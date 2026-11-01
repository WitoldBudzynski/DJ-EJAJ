use crate::{BLOCK, Frame};
use crossbeam_channel::{Receiver, Sender};
use crossbeam_queue::ArrayQueue;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::Duration,
};

pub enum RecordRequest {
    Start(PathBuf),
    Stop,
    Shutdown,
}
pub enum RecordEvent {
    Started(PathBuf),
    Finished(PathBuf, u64),
    Error(String),
}
#[derive(Clone)]
pub struct RecordBlock {
    pub samples: [Frame; BLOCK],
    pub len: usize,
}
impl Default for RecordBlock {
    fn default() -> Self {
        Self {
            samples: [[0.0; 2]; BLOCK],
            len: 0,
        }
    }
}
pub struct Recorder {
    pub requests: Sender<RecordRequest>,
    pub events: Receiver<RecordEvent>,
    pub active: Arc<AtomicBool>,
    pub dropped: Arc<AtomicU64>,
    pub capture_done: Arc<AtomicBool>,
    pub queue: Arc<ArrayQueue<RecordBlock>>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Recorder {
    pub fn new(sr: u32) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        let (etx, erx) = crossbeam_channel::unbounded();
        let active = Arc::new(AtomicBool::new(false));
        let dropped = Arc::new(AtomicU64::new(0));
        let capture_done = Arc::new(AtomicBool::new(true));
        let done = capture_done.clone();
        let queue = Arc::new(ArrayQueue::<RecordBlock>::new(512));
        let a = active.clone();
        let q = queue.clone();
        let d = dropped.clone();
        let worker = thread::Builder::new()
            .name("dj-recording".into())
            .spawn(move || {
                let mut writer: Option<hound::WavWriter<std::io::BufWriter<std::fs::File>>> = None;
                let mut path = PathBuf::new();
                let mut frames = 0_u64;
                let mut stopping = false;
                let mut shutdown = false;
                let mut failed = false;
                let mut stop_time = std::time::Instant::now();
                let mut rng = 0x9EA1_7713_u32;
                while !shutdown || writer.is_some() {
                    while let Ok(cmd) = rx.try_recv() {
                        match cmd {
                            RecordRequest::Start(next) => {
                                if writer.is_none() {
                                    let spec = hound::WavSpec {
                                        channels: 2,
                                        sample_rate: sr,
                                        bits_per_sample: 24,
                                        sample_format: hound::SampleFormat::Int,
                                    };
                                    match std::fs::OpenOptions::new()
                                        .write(true)
                                        .create_new(true)
                                        .open(&next)
                                        .map_err(anyhow::Error::from)
                                        .and_then(|f| {
                                            Ok(hound::WavWriter::new(
                                                std::io::BufWriter::new(f),
                                                spec,
                                            )?)
                                        }) {
                                        Ok(w) => {
                                            while q.pop().is_some() {}
                                            writer = Some(w);
                                            path = next;
                                            frames = 0;
                                            stopping = false;
                                            failed = false;
                                            d.store(0, Ordering::Release);
                                            a.store(true, Ordering::Release);
                                            let _ = etx.send(RecordEvent::Started(path.clone()));
                                        }
                                        Err(e) => {
                                            let _ = etx.send(RecordEvent::Error(e.to_string()));
                                        }
                                    }
                                }
                            }
                            RecordRequest::Stop => {
                                stop_time = std::time::Instant::now();
                                a.store(false, Ordering::Release);
                                stopping = true;
                            }
                            RecordRequest::Shutdown => {
                                stop_time = std::time::Instant::now();
                                a.store(false, Ordering::Release);
                                stopping = true;
                                shutdown = true;
                            }
                        }
                    }
                    if d.load(Ordering::Acquire) > 0 {
                        failed = true;
                        stop_time = std::time::Instant::now();
                        a.store(false, Ordering::Release);
                        stopping = true;
                        let _ = etx.send(RecordEvent::Error(
                            "Nagrywanie przerwane: kolejka zapisu została przepełniona".into(),
                        ));
                        d.store(0, Ordering::Release);
                    }
                    if let Some(w) = writer.as_mut() {
                        while let Some(block) = q.pop() {
                            let mut error = None;
                            for frame in &block.samples[..block.len] {
                                for x in frame {
                                    rng ^= rng << 13;
                                    rng ^= rng >> 17;
                                    rng ^= rng << 5;
                                    let r1 = rng as f64 / u32::MAX as f64;
                                    rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
                                    let r2 = rng as f64 / u32::MAX as f64;
                                    let value = (f64::from(*x) * 8_388_607.0 + r1 - r2)
                                        .round()
                                        .clamp(-8_388_608.0, 8_388_607.0)
                                        as i32;
                                    if let Err(e) = w.write_sample(value) {
                                        error = Some(e.to_string());
                                        break;
                                    }
                                }
                                if error.is_some() {
                                    break;
                                }
                            }
                            frames += block.len as u64;
                            if let Some(e) = error {
                                failed = true;
                                stop_time = std::time::Instant::now();
                                a.store(false, Ordering::Release);
                                stopping = true;
                                let _ = etx.send(RecordEvent::Error(e));
                                break;
                            }
                        }
                    }
                    if stopping && q.is_empty() && (done.load(Ordering::Acquire) || stop_time.elapsed().as_secs_f32() > 1.0) {
                        if !done.load(Ordering::Acquire) { failed = true; let _ = etx.send(RecordEvent::Error("Nagranie zakończone bez potwierdzenia strumienia audio; sprawdź końcówkę pliku".into())); }
                        if let Some(w) = writer.take() {
                            match w.finalize() {
                                Ok(()) => {
                                    if !failed { let _ = etx.send(RecordEvent::Finished(path.clone(), frames)); }
                                }
                                Err(e) => {
                                    let _ = etx.send(RecordEvent::Error(e.to_string()));
                                }
                            }
                        }
                        stopping = false;
                    }
                    thread::sleep(Duration::from_millis(2));
                }
            })
            .expect("recording thread");
        Self {
            requests: tx,
            events: erx,
            active,
            dropped,
            capture_done,
            queue,
            worker: Some(worker),
        }
    }
    pub fn stop(&self) {
        // Clear before sending: stream destruction may acknowledge and flush
        // before the writer receives Stop. Do not erase that acknowledgement.
        self.capture_done.store(false, Ordering::Release);
        let _ = self.requests.send(RecordRequest::Stop);
    }
}
impl Drop for Recorder {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
        let _ = self.requests.send(RecordRequest::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
