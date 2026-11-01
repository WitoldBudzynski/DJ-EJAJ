//! Native audio output. The callback owns DSP and consumes predecoded fixed blocks.
mod recording;
#[cfg(test)]
mod rt_guard;
mod source;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_queue::ArrayQueue;
pub use djejaj_dsp::Frame;
use djejaj_dsp::{Channel, ChannelParams, Delay, Limiter, Modulation, Reverb, Smooth, crossfade};
use djejaj_sampler::{Sampler, Sequencer, Step};
pub use recording::{RecordEvent, RecordRequest};
use serde::{Deserialize, Serialize};
use source::Source;
pub use source::{SourceEvent, SourceHandle, prepare_sample};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
};
use std::time::Instant;

pub const BLOCK: usize = 256;
pub struct AtomicF32(AtomicU32);
impl AtomicF32 {
    pub fn new(x: f32) -> Self {
        Self(AtomicU32::new(x.to_bits()))
    }
    pub fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }
    pub fn set(&self, x: f32) {
        if x.is_finite() {
            self.0.store(x.to_bits(), Ordering::Relaxed);
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct DeckSettings {
    pub gain: f32,
    pub eq: [f32; 3],
    pub fader: f32,
    pub cutoff: f32,
    pub resonance: f32,
    pub filter_on: bool,
    pub highpass: bool,
    pub drive: f32,
    pub delay: f32,
    pub reverb: f32,
    pub modulation: f32,
    pub flanger: bool,
    pub cue: bool,
    pub speed: f32,
    pub key_lock: bool,
}
impl Default for DeckSettings {
    fn default() -> Self {
        Self {
            gain: 1.0,
            eq: [1.0; 3],
            fader: 0.75,
            cutoff: 20000.0,
            resonance: 0.15,
            filter_on: false,
            highpass: false,
            drive: 0.0,
            delay: 0.0,
            reverb: 0.0,
            modulation: 0.0,
            flanger: false,
            cue: false,
            speed: 1.0,
            key_lock: false,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    pub decks: [DeckSettings; 2],
    pub master: f32,
    pub crossfader: f32,
    pub thru: bool,
    pub bpm: f32,
    pub delay_beats: f32,
    pub feedback: f32,
    pub decay: f32,
    pub sampler_volume: f32,
    pub sampler_delay: f32,
    pub sampler_reverb: f32,
    pub cue_volume: f32,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            decks: [DeckSettings::default(); 2],
            master: 0.65,
            crossfader: 0.5,
            thru: true,
            bpm: 140.0,
            delay_beats: 1.0,
            feedback: 0.45,
            decay: 0.75,
            sampler_volume: 0.55,
            sampler_delay: 0.05,
            sampler_reverb: 0.08,
            cue_volume: 0.5,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> anyhow::Result<()> {
        let valid = |x: f32, a: f32, b: f32| x.is_finite() && (a..=b).contains(&x);
        anyhow::ensure!(
            valid(self.master, 0.0, 1.0)
                && valid(self.crossfader, 0.0, 1.0)
                && valid(self.bpm, 60.0, 220.0)
                && valid(self.feedback, 0.0, 0.9)
                && valid(self.decay, 0.0, 0.88)
                && valid(self.delay_beats, 0.125, 4.0),
            "Nieprawidłowe ustawienia miksera"
        );
        anyhow::ensure!(
            [
                self.sampler_volume,
                self.sampler_delay,
                self.sampler_reverb,
                self.cue_volume
            ]
            .iter()
            .all(|x| valid(*x, 0.0, 1.0)),
            "Nieprawidłowy poziom samplera lub cue"
        );
        for d in self.decks {
            anyhow::ensure!(
                !d.key_lock || valid(d.speed, 0.8, 1.2),
                "Key lock obsługuje zakres 0,8–1,2×"
            );
            anyhow::ensure!(
                valid(d.gain, 0.0, 4.0)
                    && valid(d.fader, 0.0, 1.0)
                    && valid(d.speed, 0.5, 1.5)
                    && valid(d.cutoff, 25.0, 22000.0)
                    && valid(d.resonance, 0.0, 1.0)
                    && d.eq.iter().all(|v| valid(*v, 0.0, 2.0))
                    && [d.drive, d.delay, d.reverb, d.modulation]
                        .iter()
                        .all(|v| valid(*v, 0.0, 1.0)),
                "Nieprawidłowe ustawienia decka"
            );
        }
        Ok(())
    }
}

pub enum Command {
    Settings(Settings),
    Pad(usize, f32),
    ReleasePads,
    PadOff(usize),
    SampleConfig(usize, djejaj_sampler::SampleConfig),
    ReplaceSample(usize, Vec<Frame>),
    SequenceRunning(bool),
    Step {
        track: usize,
        pattern: usize,
        index: usize,
        value: Step,
    },
    Length {
        track: usize,
        pattern: usize,
        length: usize,
    },
    Scene(usize),
    CancelScene,
    Mute(usize, bool),
    SceneMute {
        scene: usize,
        track: usize,
        muted: bool,
    },
    ScenePattern {
        scene: usize,
        track: usize,
        pattern: usize,
    },
    Swing(f32),
}
pub struct Shared {
    pub commands: ArrayQueue<Command>,
    pub panic: AtomicBool,
    pub meter: [AtomicF32; 2],
    pub deck_meter: [AtomicF32; 2],
    pub visual: ArrayQueue<[Frame; BLOCK]>,
    pub retired: ArrayQueue<Vec<Frame>>,
    pub voices: AtomicU64,
    pub buffer_min: AtomicU64,
    pub buffer_max: AtomicU64,
    pub beat: AtomicU64,
    pub callbacks: AtomicU64,
    pub late: AtomicU64,
    pub max_nanos: AtomicU64,
    pub histogram: [AtomicU64; 32],
    pub error: AtomicBool,
    pub active_scene: AtomicU64,
    pub pending_scene: AtomicU64,
    pub step: AtomicU64,
}
impl Default for Shared {
    fn default() -> Self {
        Self {
            commands: ArrayQueue::new(8192),
            panic: AtomicBool::new(false),
            meter: std::array::from_fn(|_| AtomicF32::new(0.0)),
            deck_meter: std::array::from_fn(|_| AtomicF32::new(0.0)),
            visual: ArrayQueue::new(8),
            retired: ArrayQueue::new(32),
            voices: AtomicU64::new(0),
            buffer_min: AtomicU64::new(u64::MAX),
            buffer_max: AtomicU64::new(0),
            beat: AtomicU64::new(0.0_f64.to_bits()),
            callbacks: AtomicU64::new(0),
            late: AtomicU64::new(0),
            max_nanos: AtomicU64::new(0),
            histogram: std::array::from_fn(|_| AtomicU64::new(0)),
            error: AtomicBool::new(false),
            active_scene: AtomicU64::new(1),
            pending_scene: AtomicU64::new(u64::MAX),
            step: AtomicU64::new(0),
        }
    }
}
impl Shared {
    pub fn send(&self, c: Command) -> anyhow::Result<()> {
        self.commands
            .push(c)
            .map_err(|_| anyhow::anyhow!("Kolejka sterowania jest pełna — ponów działanie"))
    }
    pub fn panic(&self) {
        self.panic.store(true, Ordering::Release);
    }
    pub fn beat(&self) -> f64 {
        f64::from_bits(self.beat.load(Ordering::Relaxed))
    }
    pub fn diagnostics(&self) -> serde_json::Value {
        let callbacks = self.callbacks.load(Ordering::Relaxed);
        let target = (callbacks as f64 * 0.99).ceil() as u64;
        let mut sum = 0;
        let mut p99 = 0.0;
        for (i, b) in self.histogram.iter().enumerate() {
            sum += b.load(Ordering::Relaxed);
            if sum >= target {
                p99 = (i + 1) as f64 / 16.0;
                break;
            }
        }
        serde_json::json!({"callbacks":callbacks,"actual_buffer_min_frames":self.buffer_min.load(Ordering::Relaxed),"actual_buffer_max_frames":self.buffer_max.load(Ordering::Relaxed),"deadline_misses":self.late.load(Ordering::Relaxed),"max_callback_ns":self.max_nanos.load(Ordering::Relaxed),"p99_callback_budget_fraction_upper_bound":p99,"stream_error":self.error.load(Ordering::Relaxed)})
    }
}
pub struct Engine {
    _stream: cpal::Stream,
    pub sources: [SourceHandle; 3],
    pub shared: Arc<Shared>,
    pub recorder: recording::Recorder,
    pub sample_rate: u32,
    pub channels: u16,
    pub buffer_requested: u32,
    pub device_name: String,
}
pub fn output_devices() -> Vec<String> {
    cpal::default_host()
        .output_devices()
        .map(|ds| ds.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default()
}
impl Engine {
    pub fn new(name: Option<&str>, buffer: u32, cue: bool) -> anyhow::Result<Self> {
        let host = cpal::default_host();
        let device = if let Some(name) = name {
            host.output_devices()?
                .find(|d| d.name().ok().as_deref() == Some(name))
                .ok_or_else(|| anyhow::anyhow!("Urządzenie jest niedostępne"))?
        } else {
            host.default_output_device()
                .ok_or_else(|| anyhow::anyhow!("Brak urządzenia audio"))?
        };
        let channels = if cue { 4 } else { 2 };
        let mut ranges: Vec<_> = device
            .supported_output_configs()?
            .filter(|c| {
                c.channels() == channels
                    && c.min_sample_rate().0 <= 48000
                    && c.max_sample_rate().0 >= 44100
            })
            .collect();
        ranges.sort_by_key(|c| usize::from(c.sample_format() != cpal::SampleFormat::F32));
        let range = ranges.first().ok_or_else(|| {
            anyhow::anyhow!(
                "Urządzenie nie obsługuje żądanego wyjścia {}-kanałowego",
                channels
            )
        })?;
        let sr = if range.max_sample_rate().0 >= 48000 {
            48000
        } else {
            44100
        };
        let supported = (*range).with_sample_rate(cpal::SampleRate(sr));
        let format = supported.sample_format();
        let mut config: cpal::StreamConfig = supported.into();
        config.buffer_size = match range.buffer_size() {
            cpal::SupportedBufferSize::Range { min, max } => {
                cpal::BufferSize::Fixed(buffer.clamp(*min, *max))
            }
            cpal::SupportedBufferSize::Unknown => cpal::BufferSize::Default,
        };
        let (a, ah) = Source::new(sr);
        let (b, bh) = Source::new(sr);
        let (p, ph) = Source::new(sr);
        let shared = Arc::new(Shared::default());
        let recorder = recording::Recorder::new(sr);
        let core = Core::new(sr, [a, b, p], shared.clone(), &recorder);
        let error = shared.clone();
        let controls = [ah.control.clone(), bh.control.clone(), ph.control.clone()];
        let err = move |_: cpal::StreamError| {
            for source in &controls {
                source.playing.store(false, Ordering::Release);
                source.ready.store(false, Ordering::Release);
            }
            error.error.store(true, Ordering::Release);
            error.panic();
        };
        let stream = match format {
            cpal::SampleFormat::F32 => build::<f32>(&device, &config, core, err)?,
            cpal::SampleFormat::I16 => build::<i16>(&device, &config, core, err)?,
            cpal::SampleFormat::U16 => build::<u16>(&device, &config, core, err)?,
            _ => anyhow::bail!("Nieobsługiwany format urządzenia: {format}"),
        };
        stream.play()?;
        Ok(Self {
            _stream: stream,
            sources: [ah, bh, ph],
            shared,
            recorder,
            sample_rate: sr,
            channels,
            buffer_requested: buffer,
            device_name: device.name()?,
        })
    }
    pub fn settings(&self, s: Settings) -> anyhow::Result<()> {
        s.validate()?;
        for (i, d) in s.decks.iter().enumerate() {
            self.sources[i].control.speed.set(d.speed);
            self.sources[i]
                .control
                .key_lock
                .store(d.key_lock, Ordering::Release);
        }
        self.shared.send(Command::Settings(s))
    }
}
fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut core: Core,
    err: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    let channels = config.channels as usize;
    let sr = config.sample_rate.0;
    device.build_output_stream(
        config,
        move |data: &mut [T], _| {
            let start = Instant::now();
            core.commands();
            core.deck_peaks = [0.0; 2];
            let mut peak = [0.0_f32; 2];
            for frame in data.chunks_mut(channels) {
                let (master, cue) = core.frame();
                for c in 0..channels {
                    frame[c] = T::from_sample(if c < 2 {
                        master[c]
                    } else if c < 4 {
                        cue[c - 2]
                    } else {
                        0.0
                    });
                }
                for c in 0..2 {
                    peak[c] = peak[c].max(master[c].abs());
                }
            }
            for (c, p) in peak.iter().enumerate() {
                core.shared.meter[c].set(*p);
                core.shared.deck_meter[c].set(core.deck_peaks[c]);
            }
            let elapsed = start.elapsed().as_nanos() as u64;
            core.shared
                .buffer_min
                .fetch_min((data.len() / channels) as u64, Ordering::Relaxed);
            core.shared
                .buffer_max
                .fetch_max((data.len() / channels) as u64, Ordering::Relaxed);
            let budget = (data.len() / channels) as f64 / sr as f64 * 1e9;
            let ratio = elapsed as f64 / budget;
            core.shared.callbacks.fetch_add(1, Ordering::Relaxed);
            core.shared.max_nanos.fetch_max(elapsed, Ordering::Relaxed);
            core.shared.histogram[((ratio * 16.0) as usize).min(31)]
                .fetch_add(1, Ordering::Relaxed);
            if ratio >= 1.0 {
                core.shared.late.fetch_add(1, Ordering::Relaxed);
            }
        },
        err,
        None,
    )
}

pub struct Core {
    sources: [Source; 3],
    shared: Arc<Shared>,
    settings: Settings,
    channels: [Channel; 2],
    modulation: [Modulation; 2],
    delay: Delay,
    reverb: Reverb,
    limiter: Limiter,
    cue_limiter: Limiter,
    sampler: Sampler,
    sequencer: Sequencer,
    sr: f32,
    beat: f64,
    master: Smooth,
    faders: [Smooth; 2],
    cross: Smooth,
    panic_gain: f32,
    panicked: bool,
    tails_cleared: bool,
    discard_remaining: usize,
    pending_return: Option<Vec<Frame>>,
    record_queue: Arc<ArrayQueue<recording::RecordBlock>>,
    record_active: Arc<AtomicBool>,
    record_capture_done: Arc<AtomicBool>,
    record_dropped: Arc<AtomicU64>,
    record_block: recording::RecordBlock,
    was_recording: bool,
    visual_block: [Frame; BLOCK],
    visual_cursor: usize,
    deck_peaks: [f32; 2],
}
impl Core {
    fn new(sr: u32, sources: [Source; 3], shared: Arc<Shared>, rec: &recording::Recorder) -> Self {
        let rate = sr as f32;
        Self {
            sources,
            shared,
            settings: Settings::default(),
            channels: std::array::from_fn(|_| Channel::new(rate)),
            modulation: std::array::from_fn(|_| Modulation::new(rate)),
            delay: Delay::new(rate),
            reverb: Reverb::new(rate),
            limiter: Limiter::new(rate),
            cue_limiter: Limiter::new(rate),
            sampler: Sampler::new(sr),
            sequencer: Sequencer::default(),
            sr: rate,
            beat: 0.0,
            master: Smooth::new(0.0, rate, 10.0),
            faders: [Smooth::new(0.0, rate, 5.0); 2],
            cross: Smooth::new(0.5, rate, 5.0),
            panic_gain: 1.0,
            panicked: false,
            tails_cleared: false,
            discard_remaining: 0,
            pending_return: None,
            record_queue: rec.queue.clone(),
            record_active: rec.active.clone(),
            record_capture_done: rec.capture_done.clone(),
            record_dropped: rec.dropped.clone(),
            record_block: recording::RecordBlock::default(),
            was_recording: false,
            visual_block: [[0.0; 2]; BLOCK],
            visual_cursor: 0,
            deck_peaks: [0.0; 2],
        }
    }
    fn commands(&mut self) {
        let panic_now = self.shared.panic.swap(false, Ordering::AcqRel);
        if panic_now {
            self.discard_remaining = self.shared.commands.len();
            self.panicked = true;
            self.tails_cleared = false;
            for source in &self.sources {
                source.control.playing.store(false, Ordering::Release);
            }
            self.sequencer.running = false;
            self.sampler.release_all();
        }
        self.shared
            .voices
            .store(self.sampler.active_count() as u64, Ordering::Relaxed);
        for _ in 0..128 {
            if let Some(data) = self.pending_return.take()
                && let Err(data) = self.shared.retired.push(data)
            {
                self.pending_return = Some(data);
                break;
            }
            let Some(cmd) = self.shared.commands.pop() else {
                break;
            };
            let stale = self.discard_remaining > 0;
            self.discard_remaining = self.discard_remaining.saturating_sub(1);
            match cmd {
                Command::Settings(s) => self.settings = s,
                Command::Pad(k, v) => {
                    if stale {
                        continue;
                    }
                    self.panicked = false;
                    self.sampler.trigger(k, v);
                }
                Command::ReleasePads => self.sampler.release_gates(),
                Command::PadOff(kind) => self.sampler.release(kind),
                Command::SampleConfig(kind, config) => {
                    if kind < 8 {
                        self.sampler.configs[kind] = config;
                    }
                }
                Command::ReplaceSample(kind, data) => {
                    let old = self.sampler.replace_sample(kind, data);
                    if let Err(old) = self.shared.retired.push(old) {
                        self.pending_return = Some(old);
                    }
                }
                Command::SequenceRunning(r) => {
                    if stale && r {
                        continue;
                    }
                    self.sequencer.running = r;
                    if r {
                        self.panicked = false;
                    }
                }
                Command::Step {
                    track,
                    pattern,
                    index,
                    value,
                } => {
                    if track < 8 && pattern < 8 && index < 64 {
                        self.sequencer.edit_step(track, pattern, index, value);
                    }
                }
                Command::Length {
                    track,
                    pattern,
                    length,
                } => {
                    if track < 8 && pattern < 8 && [16, 32, 64].contains(&length) {
                        self.sequencer.set_length(track, pattern, length);
                    }
                }
                Command::Scene(scene) => {
                    if scene < 32 {
                        self.sequencer.pending = Some(scene);
                    }
                }
                Command::CancelScene => self.sequencer.pending = None,
                Command::Mute(i, m) => {
                    if i < 8 {
                        self.sequencer.sequence.scenes[self.sequencer.scene].muted[i] = m;
                    }
                }
                Command::SceneMute {
                    scene,
                    track,
                    muted,
                } => {
                    if scene < 32 && track < 8 {
                        self.sequencer.sequence.scenes[scene].muted[track] = muted;
                    }
                }
                Command::ScenePattern {
                    scene,
                    track,
                    pattern,
                } => {
                    if scene < 32 && track < 8 && pattern < 8 {
                        self.sequencer.sequence.scenes[scene].patterns[track] = pattern;
                    }
                }
                Command::Swing(s) => self.sequencer.sequence.swing = s.clamp(0.0, 0.45),
            }
        }
        if self.panicked
            && self
                .sources
                .iter()
                .any(|s| s.control.playing.load(Ordering::Relaxed))
        {
            self.panicked = false;
        }
        self.shared
            .active_scene
            .store(self.sequencer.scene as u64, Ordering::Relaxed);
        self.shared.pending_scene.store(
            self.sequencer.pending.map_or(u64::MAX, |p| p as u64),
            Ordering::Relaxed,
        );
        self.shared
            .step
            .store(self.sequencer.step as u64, Ordering::Relaxed);
        self.shared
            .beat
            .store(self.beat.to_bits(), Ordering::Relaxed);
    }
    fn frame(&mut self) -> (Frame, Frame) {
        self.sequencer
            .tick(self.beat, self.settings.bpm, self.sr, &mut self.sampler);
        self.beat += f64::from(self.settings.bpm) / (60.0 * f64::from(self.sr));
        let cross = crossfade(self.cross.next(self.settings.crossfader));
        let mut sum = [0.0; 2];
        let mut cue = [0.0; 2];
        let mut send_delay = [0.0; 2];
        let mut send_reverb = [0.0; 2];
        for (i, cross_gain) in cross.iter().enumerate() {
            let d = self.settings.decks[i];
            let x = self.sources[i].frame();
            self.deck_peaks[i] = self.deck_peaks[i].max(x[0].abs().max(x[1].abs()));
            let p = ChannelParams {
                gain: d.gain,
                eq: d.eq,
                cutoff: d.cutoff,
                resonance: d.resonance,
                highpass: d.highpass,
                filter_on: d.filter_on,
                drive: d.drive,
            };
            let processed =
                self.modulation[i].process(self.channels[i].process(x, p), d.modulation, d.flanger);
            let gain =
                self.faders[i].next(d.fader) * if self.settings.thru { 1.0 } else { *cross_gain };
            for c in 0..2 {
                if d.cue {
                    cue[c] += processed[c];
                }
                let y = processed[c] * gain;
                sum[c] += y;
                send_delay[c] += y * d.delay;
                send_reverb[c] += y * d.reverb;
            }
        }
        let preview = self.sources[2].frame();
        for c in 0..2 {
            cue[c] += preview[c];
        }
        let sample = self.sampler.frame();
        for c in 0..2 {
            let x = sample[c] * self.settings.sampler_volume;
            sum[c] += x;
            send_delay[c] += x * self.settings.sampler_delay;
            send_reverb[c] += x * self.settings.sampler_reverb;
        }
        let echo = self.delay.process(
            send_delay,
            60.0 / self.settings.bpm * self.settings.delay_beats,
            self.settings.feedback,
        );
        let space = self.reverb.process(send_reverb, self.settings.decay);
        self.panic_gain = if self.panicked {
            (self.panic_gain - 1.0 / (self.sr * 0.005)).max(0.0)
        } else {
            (self.panic_gain + 1.0 / (self.sr * 0.01)).min(1.0)
        };
        if self.panicked && self.panic_gain == 0.0 && !self.tails_cleared {
            self.delay.clear();
            self.reverb.clear();
            for m in &mut self.modulation {
                m.clear();
            }
            for c in &mut self.channels {
                c.clear();
            }
            self.sampler.clear();
            self.tails_cleared = true;
        }
        let gain = self.master.next(self.settings.master) * self.panic_gain;
        for c in 0..2 {
            sum[c] = (sum[c] + echo[c] + space[c]) * gain;
            cue[c] *= self.settings.cue_volume * self.panic_gain;
        }
        let master = self.limiter.process(sum);
        self.visual_block[self.visual_cursor] = master;
        self.visual_cursor += 1;
        if self.visual_cursor == BLOCK {
            self.visual_cursor = 0;
            let block = std::mem::replace(&mut self.visual_block, [[0.0; 2]; BLOCK]);
            let _ = self.shared.visual.push(block);
        }
        let cue = self.cue_limiter.process(cue);
        let recording = self.record_active.load(Ordering::Relaxed);
        if recording {
            self.record_block.samples[self.record_block.len] = master;
            self.record_block.len += 1;
            if self.record_block.len == BLOCK {
                let block = std::mem::take(&mut self.record_block);
                if self.record_queue.push(block).is_err() {
                    self.record_dropped.fetch_add(1, Ordering::Relaxed);
                    self.record_active.store(false, Ordering::Release);
                }
            }
        } else if self.was_recording && self.record_block.len > 0 {
            let block = std::mem::take(&mut self.record_block);
            if self.record_queue.push(block).is_err() {
                self.record_dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
        if !recording {
            self.record_capture_done.store(true, Ordering::Release);
        }
        self.was_recording = recording;
        (master, cue)
    }
}

impl Drop for Core {
    fn drop(&mut self) {
        self.record_active.store(false, Ordering::Release);
        if self.record_block.len > 0 {
            let block = std::mem::take(&mut self.record_block);
            if self.record_queue.push(block).is_err() {
                self.record_dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
        self.record_capture_done.store(true, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recording_flushes_partial_block_and_preserves_stereo_format() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("set.wav");
        let rec = recording::Recorder::new(48000);
        let shared = Arc::new(Shared::default());
        let sources = std::array::from_fn(|_| Source::new(48000).0);
        let mut core = Core::new(48000, sources, shared, &rec);
        rec.requests
            .send(RecordRequest::Start(path.clone()))
            .unwrap();
        assert!(matches!(
            rec.events
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap(),
            RecordEvent::Started(_)
        ));
        core.sampler.trigger(0, 0.8);
        for _ in 0..1500 {
            core.frame();
        }
        rec.stop();
        let start = Instant::now();
        while rec.active.load(Ordering::Acquire) {
            assert!(start.elapsed().as_secs() < 2);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        core.frame();
        match rec
            .events
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap()
        {
            RecordEvent::Finished(_, frames) => assert_eq!(frames, 1500),
            RecordEvent::Error(e) => panic!("{e}"),
            _ => panic!("unexpected event"),
        }
        let reader = hound::WavReader::open(path).unwrap();
        assert_eq!(reader.spec().channels, 2);
        assert_eq!(reader.spec().bits_per_sample, 24);
        assert_eq!(reader.duration(), 1500);
        assert!(
            reader
                .into_samples::<i32>()
                .any(|v| v.unwrap().abs() > 1000)
        );
    }
    #[test]
    fn closing_engine_finalizes_recording_with_last_partial_buffer() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("closed.wav");
        let rec = recording::Recorder::new(48000);
        let shared = Arc::new(Shared::default());
        let sources = std::array::from_fn(|_| Source::new(48000).0);
        let mut core = Core::new(48000, sources, shared, &rec);
        rec.requests
            .send(RecordRequest::Start(path.clone()))
            .unwrap();
        assert!(matches!(
            rec.events
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap(),
            RecordEvent::Started(_)
        ));
        core.sampler.trigger(0, 0.8);
        for _ in 0..1500 {
            core.frame();
        }
        rec.stop();
        // Engine declares its stream before recorder, so Core drops first.
        drop(core);
        drop(rec);
        let reader = hound::WavReader::open(path).unwrap();
        assert_eq!(reader.duration(), 1500);
        assert_eq!(reader.spec().bits_per_sample, 24);
        assert!(
            reader
                .into_samples::<i32>()
                .any(|v| v.unwrap().abs() > 1000)
        );
    }
    #[test]
    fn panic_discards_old_pad_commands_across_multiple_callbacks() {
        let rec = recording::Recorder::new(48000);
        let shared = Arc::new(Shared::default());
        let sources = std::array::from_fn(|_| Source::new(48000).0);
        let mut c = Core::new(48000, sources, shared.clone(), &rec);
        for _ in 0..600 {
            shared.send(Command::Pad(0, 0.8)).unwrap();
        }
        shared.panic();
        for _ in 0..10 {
            c.commands();
            for _ in 0..256 {
                c.frame();
            }
        }
        assert!(c.panicked);
        assert_eq!(c.sampler.active_count(), 0);
    }
    #[test]
    fn callback_dsp_does_not_allocate_or_free() {
        let rec = recording::Recorder::new(48000);
        let shared = Arc::new(Shared::default());
        let sources = std::array::from_fn(|_| Source::new(48000).0);
        let mut core = Core::new(48000, sources, shared.clone(), &rec);
        shared.send(Command::SequenceRunning(true)).unwrap();
        shared.send(Command::Pad(0, 0.8)).unwrap();
        rt_guard::begin();
        for _ in 0..200 {
            core.commands();
            for _ in 0..256 {
                let _ = core.frame();
            }
        }
        let operations = rt_guard::end();
        assert_eq!(operations, 0, "heap operations in audio render");
    }
    #[test]
    fn settings_reject_non_finite_and_dangerous_feedback() {
        let mut s = Settings::default();
        assert!(s.validate().is_ok());
        s.feedback = 1.5;
        assert!(s.validate().is_err());
        s.feedback = 0.5;
        s.decks[0].speed = f32::NAN;
        assert!(s.validate().is_err());
    }
    #[test]
    fn panic_mutes_effect_tails() {
        let rec = recording::Recorder::new(48000);
        let shared = Arc::new(Shared::default());
        let sources = std::array::from_fn(|_| Source::new(48000).0);
        let mut c = Core::new(48000, sources, shared.clone(), &rec);
        c.settings.sampler_reverb = 0.8;
        c.sampler.trigger(0, 1.0);
        for _ in 0..24000 {
            c.frame();
        }
        shared.panic();
        c.commands();
        for _ in 0..480 {
            c.frame();
        }
        for _ in 0..1000 {
            let (a, b) = c.frame();
            assert_eq!(a, [0.0; 2]);
            assert_eq!(b, [0.0; 2]);
        }
    }
}
