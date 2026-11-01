use crate::{AtomicF32, BLOCK, Frame};
use anyhow::{Result, bail};
use crossbeam_channel::{Receiver, Sender};
use crossbeam_queue::ArrayQueue;
use djejaj_library::Decoder;
use signalsmith_stretch::Stretch;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::Duration,
};

#[derive(Clone)]
pub struct AudioBlock {
    pub samples: [Frame; BLOCK],
    pub positions: [f64; BLOCK],
    pub generation: u64,
    pub seek_version: u64,
    pub len: usize,
    pub end: bool,
}
impl Default for AudioBlock {
    fn default() -> Self {
        Self {
            samples: [[0.0; 2]; BLOCK],
            positions: [0.0; BLOCK],
            generation: 0,
            seek_version: 0,
            len: 0,
            end: false,
        }
    }
}
pub enum SourceEvent {
    Ready {
        generation: u64,
        path: PathBuf,
        duration: f64,
        sample_rate: u32,
    },
    Error {
        generation: u64,
        message: String,
    },
}
struct Load {
    generation: u64,
    path: PathBuf,
}
pub struct SourceControl {
    pub playing: AtomicBool,
    pub ready: AtomicBool,
    pub generation: AtomicU64,
    pub position: AtomicU64,
    pub speed: AtomicF32,
    pub key_lock: AtomicBool,
    pub loop_start: AtomicU64,
    pub loop_end: AtomicU64,
    pub seek_version: AtomicU64,
    pub seek_position: AtomicU64,
    pub underruns: AtomicU64,
    pub shutdown: AtomicBool,
}
impl Default for SourceControl {
    fn default() -> Self {
        Self {
            playing: AtomicBool::new(false),
            ready: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            position: AtomicU64::new(0.0_f64.to_bits()),
            speed: AtomicF32::new(1.0),
            key_lock: AtomicBool::new(false),
            loop_start: AtomicU64::new(0.0_f64.to_bits()),
            loop_end: AtomicU64::new(0.0_f64.to_bits()),
            seek_version: AtomicU64::new(0),
            seek_position: AtomicU64::new(0.0_f64.to_bits()),
            underruns: AtomicU64::new(0),
            shutdown: AtomicBool::new(false),
        }
    }
}
pub struct SourceHandle {
    pub control: Arc<SourceControl>,
    pub events: Receiver<SourceEvent>,
    loads: Sender<Load>,
}
impl SourceHandle {
    pub fn load(&self, path: PathBuf, live: bool) -> Result<u64> {
        if live && self.control.playing.load(Ordering::Acquire) {
            bail!("Deck gra — zatrzymaj go lub wybierz drugi deck");
        }
        self.control.playing.store(false, Ordering::Release);
        self.control.ready.store(false, Ordering::Release);
        let generation = self.control.generation.fetch_add(1, Ordering::AcqRel) + 1;
        self.loads.send(Load { generation, path })?;
        Ok(generation)
    }
    pub fn play(&self) -> Result<()> {
        anyhow::ensure!(
            self.control.ready.load(Ordering::Acquire),
            "Najpierw załaduj utwór i poczekaj na stan Gotowy"
        );
        self.control.playing.store(true, Ordering::Release);
        Ok(())
    }
    pub fn stop(&self) {
        self.control.playing.store(false, Ordering::Release);
    }
    pub fn seek(&self, seconds: f64) {
        if !seconds.is_finite() {
            return;
        }
        self.control.ready.store(false, Ordering::Release);
        self.control
            .seek_position
            .store(seconds.max(0.0).to_bits(), Ordering::Release);
        self.control.seek_version.fetch_add(1, Ordering::AcqRel);
    }
    pub fn position(&self) -> f64 {
        f64::from_bits(self.control.position.load(Ordering::Relaxed))
    }
    pub fn set_loop(&self, start: f64, end: f64) {
        self.control
            .loop_start
            .store(start.max(0.0).to_bits(), Ordering::Release);
        self.control
            .loop_end
            .store(end.max(0.0).to_bits(), Ordering::Release);
    }
}
pub struct Source {
    pub control: Arc<SourceControl>,
    queue: Arc<ArrayQueue<AudioBlock>>,
    current: AudioBlock,
    cursor: usize,
    starved: bool,
    last: Frame,
}
impl Source {
    pub fn new(sr: u32) -> (Self, SourceHandle) {
        let control = Arc::new(SourceControl::default());
        let queue = Arc::new(ArrayQueue::new(3));
        let (tx, rx) = crossbeam_channel::unbounded();
        let (etx, erx) = crossbeam_channel::unbounded();
        let ctrl = control.clone();
        let q = queue.clone();
        thread::Builder::new()
            .name("dj-decode".into())
            .spawn(move || worker(sr, ctrl, q, rx, etx))
            .expect("decoder thread");
        (
            Self {
                control: control.clone(),
                queue,
                current: AudioBlock::default(),
                cursor: 0,
                starved: false,
                last: [0.0; 2],
            },
            SourceHandle {
                control,
                events: erx,
                loads: tx,
            },
        )
    }
    pub fn frame(&mut self) -> Frame {
        if !self.control.playing.load(Ordering::Relaxed)
            || !self.control.ready.load(Ordering::Acquire)
        {
            self.last = [self.last[0] * 0.98, self.last[1] * 0.98];
            return self.last;
        }
        let generation = self.control.generation.load(Ordering::Acquire);
        let seek_version = self.control.seek_version.load(Ordering::Acquire);
        if self.cursor >= self.current.len
            || self.current.generation != generation
            || self.current.seek_version != seek_version
        {
            let mut found = false;
            for _ in 0..4 {
                if let Some(block) = self.queue.pop() {
                    if block.generation == generation && block.seek_version == seek_version {
                        self.current = block;
                        self.cursor = 0;
                        found = true;
                        break;
                    }
                } else {
                    break;
                }
            }
            if !found {
                if !self.starved {
                    self.control.underruns.fetch_add(1, Ordering::Relaxed);
                    self.starved = true;
                }
                self.last = [self.last[0] * 0.98, self.last[1] * 0.98];
                return self.last;
            }
        }
        self.starved = false;
        if self.current.len == 0 {
            if self.current.end {
                self.control.playing.store(false, Ordering::Release);
            }
            return [0.0; 2];
        }
        self.last = self.current.samples[self.cursor];
        self.control.position.store(
            self.current.positions[self.cursor].to_bits(),
            Ordering::Relaxed,
        );
        self.cursor += 1;
        if self.cursor == self.current.len && self.current.end {
            self.control.playing.store(false, Ordering::Release);
        }
        self.last
    }
}
impl Drop for Source {
    fn drop(&mut self) {
        self.control.shutdown.store(true, Ordering::Release);
    }
}

/// Windowed-sinc resampling with a precomputed polyphase table, on the decoding worker.
struct Reader {
    decoder: Decoder,
    points: [Frame; 32],
    fraction: f64,
    position: f64,
    ended: bool,
    table: Vec<[f32; 32]>,
    last_increment: f64,
    loop_start: f64,
    loop_end: f64,
}
impl Reader {
    fn new(mut decoder: Decoder) -> Result<Self> {
        let first = decoder.next_frame()?.unwrap_or([0.0; 2]);
        let mut points = [first; 32];
        for item in points.iter_mut().skip(16) {
            *item = decoder.next_frame()?.unwrap_or([0.0; 2]);
        }
        Ok(Self {
            decoder,
            points,
            fraction: 0.0,
            position: 0.0,
            ended: false,
            table: vec![[0.0; 32]; 256],
            last_increment: 0.0,
            loop_start: 0.0,
            loop_end: 0.0,
        })
    }
    fn coefficients(&mut self, increment: f64) {
        if (increment - self.last_increment).abs() < 0.00001 {
            return;
        }
        self.last_increment = increment;
        let cutoff = (0.94 / increment.max(1.0)) as f32;
        for (phase, weights) in self.table.iter_mut().enumerate() {
            let t = phase as f32 / 256.0;
            let mut sum = 0.0;
            for (i, w) in weights.iter_mut().enumerate() {
                let x = i as f32 - 15.0 - t;
                let sinc = if x.abs() < 1e-6 {
                    cutoff
                } else {
                    (std::f32::consts::PI * x * cutoff).sin() / (std::f32::consts::PI * x)
                };
                let window = 0.5 + 0.5 * (std::f32::consts::PI * x / 16.0).cos();
                *w = sinc * window;
                sum += *w;
            }
            for w in weights {
                *w /= sum;
            }
        }
    }
    fn next(&mut self, increment: f64) -> Result<Frame> {
        if self.loop_end > self.loop_start + 0.01
            && self.position / self.decoder.sample_rate as f64 >= self.loop_end
        {
            let excess = self.position / self.decoder.sample_rate as f64 - self.loop_end;
            self.seek(self.loop_start + excess)?;
        }
        self.coefficients(increment);
        let weights = &self.table[((self.fraction * 256.0) as usize).min(255)];
        let mut out = [0.0; 2];
        for (point, w) in self.points.iter().zip(weights) {
            out[0] += point[0] * w;
            out[1] += point[1] * w;
        }
        if self.loop_end > self.loop_start + 0.01 {
            let pos = self.position / self.decoder.sample_rate as f64;
            let fade = ((pos - self.loop_start) / 0.002)
                .clamp(0.0, 1.0)
                .min(((self.loop_end - pos) / 0.002).clamp(0.0, 1.0)) as f32;
            out[0] *= fade;
            out[1] *= fade;
        }
        self.fraction += increment;
        self.position += increment;
        while self.fraction >= 1.0 {
            self.fraction -= 1.0;
            self.points.rotate_left(1);
            self.points[31] = match self.decoder.next_frame()? {
                Some(x) => x,
                None => {
                    self.ended = true;
                    [0.0; 2]
                }
            };
        }
        Ok(out)
    }
    fn seek(&mut self, seconds: f64) -> Result<()> {
        self.decoder.seek(seconds)?;
        let first = self.decoder.next_frame()?.unwrap_or([0.0; 2]);
        self.points = [first; 32];
        for item in self.points.iter_mut().skip(16) {
            *item = self.decoder.next_frame()?.unwrap_or([0.0; 2]);
        }
        self.fraction = 0.0;
        self.position = seconds * self.decoder.sample_rate as f64;
        self.ended = false;
        Ok(())
    }
}

fn worker(
    sr: u32,
    control: Arc<SourceControl>,
    queue: Arc<ArrayQueue<AudioBlock>>,
    loads: Receiver<Load>,
    events: Sender<SourceEvent>,
) {
    let mut reader: Option<Reader> = None;
    let mut generation = 0;
    let mut last_path: Option<PathBuf> = None;
    let mut seek_version = 0;
    let mut stretch = Stretch::new(2, 1024, 256);
    let mut previous_lock = false;
    let mut input = vec![0.0_f32; BLOCK * 4];
    let mut output = vec![0.0_f32; BLOCK * 2];
    let mut input_fraction = 0.0;
    let mut eof_blocks = 0;
    let mut pending: Option<AudioBlock> = None;
    while !control.shutdown.load(Ordering::Acquire) {
        let mut load = None;
        while let Ok(next) = loads.try_recv() {
            load = Some(next);
        }
        if let Some(load) = load {
            match Decoder::open(&load.path).and_then(Reader::new) {
                Ok(next) => {
                    if load.generation != control.generation.load(Ordering::Acquire) {
                        continue;
                    }
                    last_path = Some(load.path.clone());
                    generation = load.generation;
                    while queue.pop().is_some() {}
                    pending = None;
                    let duration = next.decoder.duration;
                    let sample_rate = next.decoder.sample_rate;
                    reader = Some(next);
                    stretch.reset();
                    eof_blocks = 0;
                    input_fraction = 0.0;
                    seek_version = control.seek_version.load(Ordering::Acquire);
                    control.position.store(0.0_f64.to_bits(), Ordering::Release);
                    control.loop_end.store(0.0_f64.to_bits(), Ordering::Release);
                    control.ready.store(false, Ordering::Release);
                    let _ = events.send(SourceEvent::Ready {
                        generation,
                        path: load.path,
                        duration,
                        sample_rate,
                    });
                }
                Err(e) => {
                    control.generation.store(generation, Ordering::Release);
                    control.ready.store(reader.is_some(), Ordering::Release);
                    let _ = events.send(SourceEvent::Error {
                        generation: load.generation,
                        message: e.to_string(),
                    });
                    if let (Some(path), Some(reader)) = (&last_path, &reader) {
                        let _ = events.send(SourceEvent::Ready {
                            generation,
                            path: path.clone(),
                            duration: reader.decoder.duration,
                            sample_rate: reader.decoder.sample_rate,
                        });
                    }
                }
            }
        }
        let Some(r) = reader.as_mut() else {
            thread::sleep(Duration::from_millis(3));
            continue;
        };
        let seek = control.seek_version.load(Ordering::Acquire);
        if seek != seek_version {
            seek_version = seek;
            let position = f64::from_bits(control.seek_position.load(Ordering::Acquire));
            match r.seek(position) {
                Ok(()) => {
                    while queue.pop().is_some() {}
                    pending = None;
                    stretch.reset();
                    eof_blocks = 0;
                    if !control.playing.load(Ordering::Acquire) {
                        control
                            .position
                            .store(position.to_bits(), Ordering::Release);
                    }
                }
                Err(e) => {
                    let _ = events.send(SourceEvent::Error {
                        generation,
                        message: e.to_string(),
                    });
                }
            }
        }
        if let Some(block) = pending.take() {
            if let Err(block) = queue.push(block) {
                pending = Some(block);
                thread::sleep(Duration::from_millis(1));
                continue;
            }
            control.ready.store(true, Ordering::Release);
        }
        if queue.is_full() || eof_blocks >= if previous_lock { 8 } else { 1 } {
            thread::sleep(Duration::from_millis(1));
            continue;
        }
        let key_lock = control.key_lock.load(Ordering::Relaxed);
        if key_lock != previous_lock {
            stretch.reset();
            previous_lock = key_lock;
        }
        let speed = control.speed.get().clamp(
            if key_lock { 0.8 } else { 0.5 },
            if key_lock { 1.2 } else { 1.5 },
        ) as f64;
        let ratio = r.decoder.sample_rate as f64 / sr as f64;
        let start = r.position / r.decoder.sample_rate as f64;
        let loop_start = f64::from_bits(control.loop_start.load(Ordering::Relaxed));
        let loop_end = f64::from_bits(control.loop_end.load(Ordering::Relaxed));
        r.loop_start = loop_start;
        r.loop_end = loop_end;
        let mut block = AudioBlock {
            generation,
            seek_version,
            ..AudioBlock::default()
        };
        let result: Result<()> = (|| {
            if key_lock {
                input_fraction += BLOCK as f64 * speed;
                let frames = input_fraction.floor() as usize;
                input_fraction -= frames as f64;
                for i in 0..frames {
                    let x = r.next(ratio)?;
                    input[i * 2] = x[0];
                    input[i * 2 + 1] = x[1];
                }
                stretch.process(&input[..frames * 2], &mut output);
                for i in 0..BLOCK {
                    block.samples[i] = [output[i * 2], output[i * 2 + 1]];
                    block.positions[i] = (start + (i as f64 / sr as f64) * speed).max(0.0);
                }
            } else {
                for i in 0..BLOCK {
                    block.positions[i] = r.position / r.decoder.sample_rate as f64;
                    block.samples[i] = r.next(ratio * speed)?;
                }
            }
            block.len = BLOCK;
            if r.ended {
                eof_blocks += 1;
                if !key_lock || eof_blocks >= 8 {
                    block.end = true;
                }
            }
            Ok(())
        })();
        match result {
            Ok(()) => {
                if let Err(block) = queue.push(block) {
                    pending = Some(block);
                } else {
                    control.ready.store(true, Ordering::Release);
                }
            }
            Err(e) => {
                control.playing.store(false, Ordering::Release);
                control.ready.store(false, Ordering::Release);
                reader = None;
                let _ = events.send(SourceEvent::Error {
                    generation,
                    message: e.to_string(),
                });
            }
        }
    }
}

pub fn prepare_sample(path: &std::path::Path, sr: u32) -> Result<Vec<Frame>> {
    let decoder = Decoder::open(path)?;
    let ratio = decoder.sample_rate as f64 / sr as f64;
    let mut reader = Reader::new(decoder)?;
    let mut frames = Vec::new();
    while !reader.ended {
        anyhow::ensure!(
            frames.len() < 2_000_000,
            "Sample jest zbyt długi: limit 2 mln ramek na pad (około 42 s przy 48 kHz). Skróć plik lub użyj decka."
        );
        frames.push(reader.next(ratio)?);
    }
    anyhow::ensure!(!frames.is_empty(), "Pusty sample");
    Ok(frames)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sine(path: &std::path::Path) {
        let mut w = hound::WavWriter::create(
            path,
            hound::WavSpec {
                channels: 2,
                sample_rate: 48000,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )
        .unwrap();
        for i in 0..144000 {
            w.write_sample(
                ((i as f32 * std::f32::consts::TAU * 440.0 / 48000.0).sin() * 8000.0) as i16,
            )
            .unwrap();
            w.write_sample(0_i16).unwrap();
        }
        w.finalize().unwrap();
    }
    fn wait(predicate: impl Fn() -> bool) {
        let start = std::time::Instant::now();
        while !predicate() {
            assert!(start.elapsed().as_secs_f32() < 5.0, "worker timed out");
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    fn collect(source: &mut Source, count: usize) -> Vec<Frame> {
        let mut out = Vec::new();
        while out.len() < count {
            if source.cursor >= source.current.len && source.queue.is_empty() {
                std::thread::sleep(Duration::from_millis(1));
                continue;
            }
            out.push(source.frame());
        }
        out
    }
    #[test]
    fn live_load_is_protected_and_failed_load_keeps_previous_track() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sine.wav");
        sine(&path);
        let (mut source, handle) = Source::new(48000);
        let generation = handle.load(path.clone(), true).unwrap();
        wait(|| handle.control.ready.load(Ordering::Acquire));
        handle.play().unwrap();
        assert!(handle.load(path.clone(), true).is_err());
        assert_eq!(
            handle.control.generation.load(Ordering::Relaxed),
            generation
        );
        let x = collect(&mut source, 2048);
        assert!(x.iter().any(|x| x[0].abs() > 0.05));
        assert!(x.iter().all(|x| x[1] == 0.0));
        handle.stop();
        handle.load(dir.path().join("missing.mp3"), true).unwrap();
        wait(|| handle.control.ready.load(Ordering::Acquire));
        assert_eq!(
            handle.control.generation.load(Ordering::Relaxed),
            generation
        );
    }
    #[test]
    fn seek_invalidates_buffered_audio() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("seek.wav");
        sine(&path);
        let (mut source, handle) = Source::new(48000);
        handle.load(path, true).unwrap();
        wait(|| handle.control.ready.load(Ordering::Acquire));
        handle.play().unwrap();
        collect(&mut source, 256);
        handle.seek(1.0);
        let version = handle.control.seek_version.load(Ordering::Acquire);
        let mut reached = false;
        for _ in 0..5000 {
            let _ = source.frame();
            if handle.position() >= 1.0 {
                reached = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(reached);
        assert_eq!(source.current.seek_version, version);
    }
    #[test]
    fn key_lock_preserves_test_tone_at_changed_tempo() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pitch.wav");
        sine(&path);
        let (mut source, handle) = Source::new(48000);
        handle.control.key_lock.store(true, Ordering::Release);
        handle.control.speed.set(1.2);
        handle.load(path, true).unwrap();
        wait(|| handle.control.ready.load(Ordering::Acquire));
        handle.play().unwrap();
        let frames = collect(&mut source, 60000);
        let signal = &frames[6000..54000];
        let crossings = signal
            .windows(2)
            .filter(|w| w[0][0] < 0.0 && w[1][0] >= 0.0)
            .count();
        assert!(
            (crossings as f32 - 440.0).abs() <= 2.0,
            "measured {crossings} Hz"
        );
        assert!(signal.iter().all(|f| f[1].abs() < 1e-6));
    }
}
