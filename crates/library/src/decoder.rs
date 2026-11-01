use anyhow::{Context, Result, bail};
use std::{fs::File, path::Path};
use symphonia::core::{
    audio::SampleBuffer,
    codecs::{Decoder as AudioDecoder, DecoderOptions},
    formats::{FormatOptions, FormatReader, SeekMode, SeekTo},
    io::MediaSourceStream,
    meta::MetadataOptions,
    probe::Hint,
    units::Time,
};

pub struct Decoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track: u32,
    pub sample_rate: u32,
    pub duration: f64,
    pending: Vec<[f32; 2]>,
    cursor: usize,
    title: String,
    artist: String,
}
impl Decoder {
    pub fn open(path: &Path) -> Result<Self> {
        let source = MediaSourceStream::new(Box::new(File::open(path)?), Default::default());
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }
        let mut probe = symphonia::default::get_probe().format(
            &hint,
            source,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )?;
        let mut title = String::new();
        let mut artist = String::new();
        if let Some(metadata) = probe.metadata.get()
            && let Some(revision) = metadata.current()
        {
            for tag in revision.tags() {
                match tag.std_key {
                    Some(symphonia::core::meta::StandardTagKey::TrackTitle) => {
                        title = tag.value.to_string()
                    }
                    Some(symphonia::core::meta::StandardTagKey::Artist) => {
                        artist = tag.value.to_string()
                    }
                    _ => {}
                }
            }
        }
        let format = probe.format;
        let track = format.default_track().context("Brak ścieżki audio")?;
        let channels = track
            .codec_params
            .channels
            .context("Nieznana liczba kanałów")?
            .count();
        anyhow::ensure!(
            (1..=2).contains(&channels),
            "Obsługiwane są pliki mono i stereo"
        );
        let sample_rate = track
            .codec_params
            .sample_rate
            .context("Brak częstotliwości próbkowania")?;
        let duration = track
            .codec_params
            .n_frames
            .map_or(0.0, |n| n as f64 / sample_rate as f64);
        let decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())?;
        let id = track.id;
        Ok(Self {
            format,
            decoder,
            track: id,
            sample_rate,
            duration,
            pending: Vec::new(),
            cursor: 0,
            title,
            artist,
        })
    }
    pub fn next_frame(&mut self) -> Result<Option<[f32; 2]>> {
        if self.cursor < self.pending.len() {
            let x = self.pending[self.cursor];
            self.cursor += 1;
            return Ok(Some(x));
        }
        for _ in 0..64 {
            let packet = match self.format.next_packet() {
                Ok(p) => p,
                Err(symphonia::core::errors::Error::IoError(e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    return Ok(None);
                }
                Err(e) => return Err(e.into()),
            };
            if packet.track_id() != self.track {
                continue;
            }
            let decoded = match self.decoder.decode(&packet) {
                Ok(d) => d,
                Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
                Err(e) => return Err(e.into()),
            };
            let count = decoded.spec().channels.count();
            anyhow::ensure!(
                (1..=2).contains(&count),
                "Zmiana liczby kanałów poza mono/stereo"
            );
            let mut buffer = SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
            buffer.copy_interleaved_ref(decoded);
            self.pending.clear();
            self.pending.extend(
                buffer
                    .samples()
                    .chunks_exact(count)
                    .map(|f| [f[0], if count == 1 { f[0] } else { f[1] }]),
            );
            self.cursor = 0;
            if !self.pending.is_empty() {
                let x = self.pending[0];
                self.cursor = 1;
                return Ok(Some(x));
            }
        }
        bail!("Nie można odczytać kolejnego pakietu audio")
    }
    pub fn seek(&mut self, seconds: f64) -> Result<()> {
        let seconds = seconds.max(0.0);
        let result = self.format.seek(
            SeekMode::Accurate,
            SeekTo::Time {
                time: Time::from(seconds),
                track_id: Some(self.track),
            },
        )?;
        self.decoder.reset();
        self.pending.clear();
        self.cursor = 0;
        let time = self
            .format
            .default_track()
            .and_then(|t| t.codec_params.time_base)
            .map(|tb| tb.calc_time(result.actual_ts));
        let actual = time.map_or(seconds, |t| t.seconds as f64 + t.frac);
        let discard = ((seconds - actual).max(0.0) * self.sample_rate as f64) as usize;
        for _ in 0..discard {
            if self.next_frame()?.is_none() {
                break;
            }
        }
        Ok(())
    }
    pub fn tags(&mut self) -> (String, String) {
        let mut title = self.title.clone();
        let mut artist = self.artist.clone();
        if let Some(meta) = self.format.metadata().current() {
            for tag in meta.tags() {
                match tag.std_key {
                    Some(symphonia::core::meta::StandardTagKey::TrackTitle) => {
                        title = tag.value.to_string()
                    }
                    Some(symphonia::core::meta::StandardTagKey::Artist) => {
                        artist = tag.value.to_string()
                    }
                    _ => {}
                }
            }
        }
        (title, artist)
    }
}
