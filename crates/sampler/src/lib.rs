//! Deterministic original sounds and sample-clock-based step sequencing.
use anyhow::Result;
use djejaj_dsp::Frame;
use serde::{Deserialize, Serialize};
use std::{f32::consts::PI, path::Path};

pub const NAMES: [&str; 8] = [
    "Kick",
    "Clap",
    "Closed hat",
    "Open hat",
    "Perc",
    "Bass",
    "Stab",
    "FX",
];
pub const KEYS: [&str; 8] = ["Z", "X", "C", "V", "B", "N", "M", ","];
pub const VOICES: usize = 16;

fn noise(state: &mut u32) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 17;
    *state ^= *state << 5;
    (*state as f64 / u32::MAX as f64 * 2.0 - 1.0) as f32
}
pub fn generate(kind: usize, sr: u32) -> Vec<Frame> {
    let seconds = [0.7, 0.32, 0.12, 0.6, 0.35, 1.0, 0.8, 2.0][kind.min(7)];
    let mut seed = 0xE1A4_0001 + kind as u32;
    let mut phase = 0.0;
    let mut high = 0.0;
    (0..(seconds * sr as f32) as usize)
        .map(|i| {
            let t = i as f32 / sr as f32;
            let n = noise(&mut seed);
            high += 0.06 * (n - high);
            let sample = match kind {
                0 => {
                    phase += 2.0 * PI * (48.0 + 115.0 * (-t * 38.0).exp()) / sr as f32;
                    phase.sin() * (-t * 9.0).exp() + n * 0.15 * (-t * 180.0).exp()
                }
                1 => {
                    let burst = if t < 0.035 {
                        (t * 350.0).sin().abs()
                    } else {
                        0.4
                    };
                    (n - high) * burst * (-t * 16.0).exp()
                }
                2 => (n - high) * (-t * 60.0).exp(),
                3 => (n - high) * (-t * 8.0).exp(),
                4 => {
                    ((t * 2.0 * PI * 185.0 + 2.0 * (t * 2.0 * PI * 280.0).sin()).sin())
                        * (-t * 16.0).exp()
                }
                5 => (t * 2.0 * PI * 55.0).sin() * (-t * 5.0).exp(),
                6 => {
                    ((t * 2.0 * PI * 220.0).sin()
                        + (t * 2.0 * PI * 261.626).sin()
                        + (t * 2.0 * PI * 329.628).sin())
                        / 3.0
                        * (-t * 7.0).exp()
                }
                _ => {
                    let env = (t / seconds).clamp(0.0, 1.0);
                    (n - high) * env * env * (1.0 - (env - 0.96).max(0.0) * 25.0)
                }
            };
            let fade = (t * 1000.0).min(1.0) * ((seconds - t) * 100.0).clamp(0.0, 1.0);
            let x = (sample * 0.55 * fade).clamp(-0.9, 0.9);
            // Stable centered transients; subtle decorrelation only on texture effects.
            [
                x,
                if kind == 7 {
                    x * 0.8 + n * 0.04 * fade * (t / seconds)
                } else {
                    x
                },
            ]
        })
        .collect()
}

pub fn export_bank(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)?;
    for (i, name) in NAMES.iter().enumerate() {
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 48000,
            bits_per_sample: 24,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(
            path.join(format!(
                "{:02}-{}.wav",
                i + 1,
                name.to_lowercase().replace(' ', "-")
            )),
            spec,
        )?;
        for frame in generate(i, 48000) {
            for x in frame {
                writer.write_sample((x * 8_388_607.0) as i32)?;
            }
        }
        writer.finalize()?;
    }
    std::fs::write(
        path.join("manifest.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"schema":1,"creator":"DJ EJAJ / NT4.pl","sample_rate":48000,"channels":2,"bits":24,"origin":"Original deterministic DSP synthesis; no third-party recordings","seed":"0xE1A40001 + instrument index","recipe_source":"crates/sampler/src/lib.rs::generate","sounds":NAMES}),
        )?,
    )?;
    Ok(())
}

/// Original short demonstration tracks; generated offline, never at application startup without --demo.
pub fn export_demo(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)?;
    for (i, name) in [
        "Vector Field - Warehouse Signal",
        "Phase Motion - Afterglow",
        "Ion Circuit - Acid Transit",
        "Neon District - Pulse Driver",
        "Polar Echo - Northern Lights",
        "Mono Sector - Concrete Waves",
        "Red Shift - Rave Protocol",
        "Sub Assembly - Deep Structure",
    ]
    .iter()
    .enumerate()
    {
        let target = path.join(format!("{name}.wav"));
        if target.exists() {
            continue;
        }
        let sr = 48000;
        let bpm = if i % 3 == 0 {
            140.0
        } else if i % 3 == 1 {
            138.0
        } else {
            142.0
        };
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: sr,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(target, spec)?;
        let mut sampler = Sampler::new(sr);
        let mut seq = Sequencer {
            running: true,
            ..Sequencer::default()
        };
        let frames = (sr as f64 * 60.0 / bpm * 32.0) as usize;
        for frame in 0..frames {
            let beat = frame as f64 / sr as f64 * bpm / 60.0;
            seq.tick(beat, bpm as f32, sr as f32, &mut sampler);
            let x = sampler.frame();
            let fade = (frame as f32 / (sr as f32 * 0.015)).min(1.0)
                * ((frames - frame) as f32 / (sr as f32 * 0.05)).min(1.0);
            for v in x {
                writer.write_sample((v * fade * 0.6 * 32767.0).clamp(-32768.0, 32767.0) as i16)?;
            }
        }
        writer.finalize()?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Step {
    pub on: bool,
    pub velocity: f32,
    pub note: u8,
    pub gate: f32,
    pub accent: bool,
    pub slide: bool,
}
impl Default for Step {
    fn default() -> Self {
        Self {
            on: false,
            velocity: 0.8,
            note: 33,
            gate: 0.5,
            accent: false,
            slide: false,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pattern {
    pub length: usize,
    pub steps: Vec<Step>,
}
impl Default for Pattern {
    fn default() -> Self {
        Self {
            length: 16,
            steps: vec![Step::default(); 64],
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scene {
    pub name: String,
    pub patterns: [usize; 8],
    pub muted: [bool; 8],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sequence {
    pub patterns: Vec<Vec<Pattern>>,
    pub scenes: Vec<Scene>,
    pub swing: f32,
    pub volume: f32,
}
impl Default for Sequence {
    fn default() -> Self {
        let mut patterns = vec![vec![Pattern::default(); 8]; 8];
        for (track, steps) in [
            (0, vec![0, 4, 8, 12]),
            (1, vec![4, 12]),
            (2, vec![2, 6, 10, 14]),
            (3, vec![2, 6, 10, 14]),
            (5, vec![2, 3, 6, 7, 10, 11, 14, 15]),
        ] {
            for pattern in &mut patterns[track] {
                for &step in &steps {
                    pattern.steps[step].on = true;
                    pattern.steps[step].velocity = if track == 0 { 0.85 } else { 0.45 };
                }
            }
        }
        Self {
            patterns,
            scenes: (0..32)
                .map(|i| Scene {
                    name: match i {
                        0 => "INTRO",
                        1 => "GROOVE",
                        2 => "BREAK",
                        3 => "BUILD",
                        4 => "DROP",
                        _ => "SCENA",
                    }
                    .to_owned(),
                    patterns: [i % 8; 8],
                    muted: std::array::from_fn(|t| match i {
                        0 => t != 2 && t != 4,
                        2 => t == 0 || t == 5,
                        3 => t == 5,
                        _ => false,
                    }),
                })
                .collect(),
            swing: 0.0,
            volume: 0.55,
        }
    }
}
impl Sequence {
    pub fn validate(&self) -> Result<()> {
        anyhow::ensure!(
            self.patterns.len() == 8 && self.scenes.len() == 32,
            "Nieprawidłowa liczba ścieżek/scen"
        );
        anyhow::ensure!(
            self.swing.is_finite()
                && (0.0..=0.45).contains(&self.swing)
                && self.volume.is_finite()
                && (0.0..=1.0).contains(&self.volume),
            "Nieprawidłowe parametry sekwencera"
        );
        for track in &self.patterns {
            anyhow::ensure!(track.len() == 8, "Nieprawidłowy bank wzorców");
            for p in track {
                anyhow::ensure!(
                    [16, 32, 64].contains(&p.length) && p.steps.len() == 64,
                    "Nieprawidłowa długość wzorca"
                );
                for s in &p.steps {
                    anyhow::ensure!(
                        s.velocity.is_finite()
                            && (0.0..=1.0).contains(&s.velocity)
                            && s.gate.is_finite()
                            && (0.01..=1.0).contains(&s.gate)
                            && s.note <= 127,
                        "Nieprawidłowy krok"
                    );
                }
            }
        }
        for scene in &self.scenes {
            anyhow::ensure!(
                scene.patterns.iter().all(|p| *p < 8),
                "Nieprawidłowy wzorzec sceny"
            );
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum SampleMode {
    #[default]
    OneShot,
    Gate,
    Loop,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SampleConfig {
    pub mode: SampleMode,
    pub volume: f32,
    pub pan: f32,
    pub pitch: f32,
    pub synth: bool,
}
impl Default for SampleConfig {
    fn default() -> Self {
        Self {
            mode: SampleMode::OneShot,
            volume: 1.0,
            pan: 0.0,
            pitch: 1.0,
            synth: false,
        }
    }
}
pub fn default_sample_configs() -> [SampleConfig; 8] {
    std::array::from_fn(|i| SampleConfig {
        synth: i == 5,
        ..SampleConfig::default()
    })
}
#[derive(Clone, Copy)]
struct Voice {
    kind: usize,
    position: f64,
    velocity: f32,
    age: u64,
    active: bool,
    release: f32,
}
impl Default for Voice {
    fn default() -> Self {
        Self {
            kind: 0,
            position: 0.0,
            velocity: 0.0,
            age: 0,
            active: false,
            release: 1.0,
        }
    }
}
pub struct Sampler {
    samples: [Vec<Frame>; 8],
    pub configs: [SampleConfig; 8],
    tail: Frame,
    voices: [Voice; VOICES],
    age: u64,
    bass_phase: f32,
    bass_freq: f32,
    bass_target: f32,
    bass_env: f32,
    bass_gate: u64,
    bass_slide: bool,
    sr: f32,
}
impl Sampler {
    pub fn replace_sample(&mut self, kind: usize, data: Vec<Frame>) -> Vec<Frame> {
        if kind >= 8 || data.is_empty() {
            return data;
        }
        for v in &mut self.voices {
            if v.kind == kind {
                v.active = false;
            }
        }
        std::mem::replace(&mut self.samples[kind], data)
    }
    pub fn active_count(&self) -> usize {
        self.voices.iter().filter(|v| v.active).count() + usize::from(self.bass_env > 0.00001)
    }
    pub fn release(&mut self, kind: usize) {
        if kind < 8 && self.configs[kind].mode != SampleMode::OneShot {
            for voice in &mut self.voices {
                if voice.kind == kind {
                    voice.release = 0.985;
                }
            }
        }
    }
    pub fn release_gates(&mut self) {
        for kind in 0..8 {
            self.release(kind);
        }
    }
    pub fn clear(&mut self) {
        for voice in &mut self.voices {
            voice.active = false;
        }
        self.bass_gate = 0;
        self.bass_env = 0.0;
        self.tail = [0.0; 2];
    }
    pub fn new(sr: u32) -> Self {
        Self {
            samples: std::array::from_fn(|i| generate(i, sr)),
            configs: default_sample_configs(),
            tail: [0.0; 2],
            voices: [Voice::default(); VOICES],
            age: 0,
            bass_phase: 0.0,
            bass_freq: 55.0,
            bass_target: 55.0,
            bass_env: 0.0,
            bass_gate: 0,
            bass_slide: false,
            sr: sr as f32,
        }
    }
    pub fn trigger(&mut self, kind: usize, velocity: f32) {
        if kind >= 8 {
            return;
        }
        if kind == 2 {
            for voice in &mut self.voices {
                if voice.kind == 3 {
                    voice.release = 0.985;
                }
            }
        }
        // Reserve the sixteenth slot for the monophonic bass.
        let slot = self.voices[..VOICES - 1]
            .iter()
            .position(|v| !v.active)
            .unwrap_or_else(|| {
                self.voices[..VOICES - 1]
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, v)| v.age)
                    .map_or(0, |(i, _)| i)
            });
        if self.voices[slot].active {
            let old = self.voices[slot];
            if let Some(x) = self.samples[old.kind].get(old.position as usize) {
                self.tail[0] += x[0] * old.velocity;
                self.tail[1] += x[1] * old.velocity;
            }
        }
        self.age += 1;
        self.voices[slot] = Voice {
            kind,
            position: 0.0,
            velocity: velocity.clamp(0.0, 1.0),
            age: self.age,
            active: true,
            release: 1.0,
        };
    }
    pub fn bass(&mut self, step: Step, duration: u64) {
        self.bass_target = 440.0 * 2.0_f32.powf((step.note as f32 - 69.0) / 12.0);
        self.bass_slide = step.slide;
        if !step.slide || self.bass_env < 0.0001 {
            self.bass_env = step.velocity * (if step.accent { 1.0 } else { 0.7 });
            self.bass_freq = self.bass_target;
        }
        self.bass_gate = duration;
    }
    pub fn release_all(&mut self) {
        for v in &mut self.voices {
            v.release = 0.985;
        }
        self.bass_gate = 0;
    }
    pub fn frame(&mut self) -> Frame {
        let mut out = [0.0; 2];
        for voice in &mut self.voices {
            if !voice.active {
                continue;
            }
            let config = self.configs[voice.kind];
            let sample = &self.samples[voice.kind];
            let index = voice.position as usize;
            if index < sample.len() {
                let frac = (voice.position - index as f64) as f32;
                let next = (index + 1).min(sample.len() - 1);
                let edge = ((sample.len() - index) as f32 / (self.sr * 0.002)).min(1.0);
                let start = (voice.position as f32 / (self.sr * 0.002)).min(1.0);
                let gains = [
                    ((config.pan + 1.0) * PI / 4.0).cos() * std::f32::consts::SQRT_2,
                    ((config.pan + 1.0) * PI / 4.0).sin() * std::f32::consts::SQRT_2,
                ];
                for c in 0..2 {
                    out[c] += (sample[index][c] * (1.0 - frac) + sample[next][c] * frac)
                        * voice.velocity
                        * config.volume
                        * gains[c]
                        * edge
                        * start;
                }
                voice.position += config.pitch as f64;
                voice.velocity *= voice.release;
                if voice.velocity < 1e-5 {
                    voice.active = false;
                }
                if voice.position >= sample.len() as f64 {
                    if config.mode == SampleMode::Loop {
                        voice.position = voice.position.rem_euclid(sample.len() as f64);
                    } else {
                        voice.active = false;
                    }
                }
            } else {
                voice.active = false;
            }
        }
        for (c, x) in out.iter_mut().enumerate() {
            *x += self.tail[c];
            self.tail[c] *= 0.98;
        }
        self.bass_freq +=
            (self.bass_target - self.bass_freq) * if self.bass_slide { 0.0015 } else { 1.0 };
        self.bass_env *= if self.bass_gate > 0 { 0.9997 } else { 0.995 };
        self.bass_gate = self.bass_gate.saturating_sub(1);
        self.bass_phase = (self.bass_phase
            + 2.0 * PI * self.bass_freq * self.configs[5].pitch / self.sr)
            % (2.0 * PI);
        let bass =
            (self.bass_phase.sin() + 0.25 * (self.bass_phase * 2.0).sin()) * self.bass_env * 0.45;
        let pan = self.configs[5].pan;
        out[0] += bass
            * self.configs[5].volume
            * ((pan + 1.0) * PI / 4.0).cos()
            * std::f32::consts::SQRT_2;
        out[1] += bass
            * self.configs[5].volume
            * ((pan + 1.0) * PI / 4.0).sin()
            * std::f32::consts::SQRT_2;
        out
    }
}

pub struct Sequencer {
    pub sequence: Sequence,
    pub running: bool,
    pub scene: usize,
    pub pending: Option<usize>,
    pub muted: [bool; 8],
    pub step: usize,
    last_tick: i64,
    pending_steps: Vec<Option<Step>>,
    pending_lengths: [Option<usize>; 64],
}
impl Default for Sequencer {
    fn default() -> Self {
        Self {
            sequence: Sequence::default(),
            running: false,
            scene: 1,
            pending: None,
            muted: [false; 8],
            step: 0,
            last_tick: -1,
            pending_steps: vec![None; 4096],
            pending_lengths: [None; 64],
        }
    }
}
impl Sequencer {
    pub fn edit_step(&mut self, track: usize, pattern: usize, index: usize, value: Step) {
        if track >= 8 || pattern >= 8 || index >= 64 {
            return;
        }
        if self.running && self.sequence.scenes[self.scene].patterns[track] == pattern {
            self.pending_steps[(track * 8 + pattern) * 64 + index] = Some(value);
        } else {
            self.sequence.patterns[track][pattern].steps[index] = value;
        }
    }
    pub fn set_length(&mut self, track: usize, pattern: usize, length: usize) {
        if track >= 8 || pattern >= 8 || ![16, 32, 64].contains(&length) {
            return;
        }
        if self.running {
            self.pending_lengths[track * 8 + pattern] = Some(length);
        } else {
            self.sequence.patterns[track][pattern].length = length;
        }
    }
    pub fn tick(&mut self, beat: f64, bpm: f32, sr: f32, sampler: &mut Sampler) {
        let ticks = beat * 4.0;
        let pair = (ticks / 2.0).floor();
        let tick = (pair * 2.0) as i64
            + i64::from(ticks - pair * 2.0 >= 1.0 + f64::from(self.sequence.swing));
        if tick == self.last_tick {
            return;
        }
        self.last_tick = tick;
        self.step = tick.rem_euclid(64) as usize;
        if tick % 16 == 0 {
            for (i, step) in self.pending_steps.iter_mut().enumerate() {
                if let Some(value) = step.take() {
                    self.sequence.patterns[i / 512][(i / 64) % 8].steps[i % 64] = value;
                }
            }
            for (i, length) in self.pending_lengths.iter_mut().enumerate() {
                if let Some(length) = length.take() {
                    self.sequence.patterns[i / 8][i % 8].length = length;
                }
            }
            if let Some(scene) = self.pending.take()
                && scene < 32
            {
                self.scene = scene;
            }
        }
        if !self.running {
            return;
        }
        let scene = &self.sequence.scenes[self.scene];
        for track in 0..8 {
            if self.muted[track] || scene.muted[track] {
                continue;
            }
            let p = &self.sequence.patterns[track][scene.patterns[track]];
            let step = p.steps[tick.rem_euclid(p.length as i64) as usize];
            if step.on {
                if track == 5 && sampler.configs[5].synth {
                    sampler.bass(step, (sr * 60.0 / bpm * 0.25 * step.gate) as u64);
                } else {
                    sampler.trigger(track, step.velocity);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gate_release_stops_loop_but_leaves_one_shots() {
        let mut s = Sampler::new(48000);
        s.configs[0].mode = SampleMode::Loop;
        s.trigger(0, 0.8);
        for _ in 0..48000 {
            s.frame();
        }
        assert!(s.active_count() > 0);
        s.release(0);
        for _ in 0..2000 {
            s.frame();
        }
        assert_eq!(s.active_count(), 0);
    }
    #[test]
    fn edits_of_playing_pattern_wait_until_next_bar() {
        let mut s = Sequencer {
            running: true,
            ..Sequencer::default()
        };
        let mut sampler = Sampler::new(48000);
        s.tick(0.1, 140.0, 48000.0, &mut sampler);
        let mut value = s.sequence.patterns[0][1].steps[1];
        value.on = true;
        s.edit_step(0, 1, 1, value);
        assert!(!s.sequence.patterns[0][1].steps[1].on);
        s.tick(4.0, 140.0, 48000.0, &mut sampler);
        assert!(s.sequence.patterns[0][1].steps[1].on);
    }
    #[test]
    fn bank_is_deterministic_stereo_and_finite() {
        for k in 0..8 {
            let a = generate(k, 48000);
            assert_eq!(a, generate(k, 48000));
            assert!(a.iter().flatten().all(|x| x.is_finite() && x.abs() < 1.0));
            assert!(!a.is_empty());
        }
    }
    #[test]
    fn invalid_session_cannot_break_sequence_indexing() {
        let mut s = Sequence::default();
        assert!(s.validate().is_ok());
        s.patterns[0][0].length = 0;
        assert!(s.validate().is_err());
    }
    #[test]
    fn scene_changes_only_at_bar_boundary() {
        let mut s = Sequencer::default();
        let mut sampler = Sampler::new(48000);
        s.pending = Some(4);
        s.tick(1.0, 140.0, 48000.0, &mut sampler);
        assert_eq!(s.scene, 1);
        s.tick(4.0, 140.0, 48000.0, &mut sampler);
        assert_eq!(s.scene, 4);
    }
    #[test]
    fn voice_limit_remains_bounded_under_dense_triggers() {
        let mut s = Sampler::new(48000);
        for n in 0..1000 {
            s.trigger(n % 8, 1.0);
            assert!(s.frame().iter().all(|v| v.is_finite()));
        }
        assert!(s.voices.iter().filter(|v| v.active).count() <= 15);
    }
}
