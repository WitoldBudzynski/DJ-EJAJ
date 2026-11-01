use rustfft::{FftPlanner, num_complex::Complex};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Analysis {
    pub waveform: Vec<[f32; 2]>,
    pub bpm: Option<f32>,
    pub bpm_confidence: f32,
    pub key: Option<String>,
    pub key_confidence: f32,
    pub seconds: f64,
}

pub fn analyze_control(
    path: &std::path::Path,
    cancel: &std::sync::atomic::AtomicBool,
    paused: Option<&std::sync::atomic::AtomicBool>,
) -> anyhow::Result<Analysis> {
    let mut decoder = crate::Decoder::open(path)?;
    let sr = decoder.sample_rate;
    let hop = (sr / 200).max(1) as usize;
    let mut peaks = Vec::new();
    let mut peak = [0.0_f32; 2];
    let mut count = 0usize;
    let mut mono = Vec::new();
    let mut energy = 0.0;
    let mut onset = Vec::new();
    let mut previous = 0.0_f32;
    while let Some(frame) = decoder.next_frame()? {
        if count.is_multiple_of(8192) {
            while paused.is_some_and(|p| p.load(std::sync::atomic::Ordering::Relaxed)) {
                if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                    anyhow::bail!("Analiza anulowana");
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
        if count.is_multiple_of(8192) && cancel.load(std::sync::atomic::Ordering::Relaxed) {
            anyhow::bail!("Analiza anulowana");
        }
        for c in 0..2 {
            peak[c] = peak[c].max(frame[c].abs());
        }
        if count % 2048 == 2047 {
            peaks.push(peak);
            peak = [0.0; 2];
        }
        if count < (sr as usize * 90) {
            energy += (frame[0] + frame[1]).abs() * 0.5;
            if count % hop == hop - 1 {
                let current = energy / hop as f32;
                onset.push((current - previous).max(0.0));
                previous = current;
                energy = 0.0;
            }
            if count.is_multiple_of((sr as usize / 4000).max(1)) {
                mono.push((frame[0] + frame[1]) * 0.5);
            }
        }
        count += 1;
    }
    if peak[0] > 0.0 || peak[1] > 0.0 {
        peaks.push(peak);
    }
    let waveform = if peaks.len() > 512 {
        (0..512)
            .map(|i| {
                let slice = &peaks[i * peaks.len() / 512
                    ..((i + 1) * peaks.len() / 512).max(i * peaks.len() / 512 + 1)];
                slice
                    .iter()
                    .fold([0.0_f32; 2], |a, b| [a[0].max(b[0]), a[1].max(b[1])])
            })
            .collect()
    } else {
        peaks
    };
    let onset_rate = sr as f32 / hop as f32;
    let (bpm, bpm_confidence) = estimate_bpm(&onset, onset_rate);
    let (key, key_confidence) = estimate_key(&mono, sr as f32 / (sr as usize / 4000).max(1) as f32);
    Ok(Analysis {
        waveform,
        bpm,
        bpm_confidence,
        key,
        key_confidence,
        seconds: count as f64 / sr as f64,
    })
}
pub fn estimate_bpm(onset: &[f32], rate: f32) -> (Option<f32>, f32) {
    if onset.len() < rate as usize * 4 {
        return (None, 0.0);
    }
    let energy = onset.iter().map(|x| x * x).sum::<f32>();
    if energy < 1e-6 {
        return (None, 0.0);
    }
    let mut best = (0.0, 0usize);
    for lag in (rate * 60.0 / 180.0) as usize..=(rate * 60.0 / 75.0) as usize {
        if lag >= onset.len() {
            continue;
        }
        let value = onset[lag..]
            .iter()
            .zip(onset)
            .map(|(a, b)| a * b)
            .sum::<f32>()
            / energy;
        if value > best.0 {
            best = (value, lag);
        }
    }
    if best.0 < 0.16 || best.1 == 0 {
        (None, best.0)
    } else {
        (Some(60.0 * rate / best.1 as f32), best.0.min(1.0))
    }
}
pub fn estimate_key(mono: &[f32], sr: f32) -> (Option<String>, f32) {
    let n = 4096;
    if mono.len() < n {
        return (None, 0.0);
    }
    let fft = FftPlanner::<f32>::new().plan_fft_forward(n);
    let mut data = vec![Complex::default(); n];
    let mut chroma = [0.0_f32; 12];
    for chunk in mono.chunks_exact(n).step_by(2) {
        for (i, x) in chunk.iter().enumerate() {
            data[i] = Complex::new(
                *x * (0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / n as f32).cos()),
                0.0,
            );
        }
        fft.process(&mut data);
        for (i, z) in data.iter().enumerate().take(n / 2).skip(1) {
            let hz = i as f32 * sr / n as f32;
            if !(55.0..=1800.0).contains(&hz) {
                continue;
            }
            let midi = (69.0 + 12.0 * (hz / 440.0).log2()).round() as i32;
            chroma[midi.rem_euclid(12) as usize] += z.norm();
        }
    }
    let major = [
        6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88,
    ];
    let minor = [
        6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17,
    ];
    let norm = chroma.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm < 1e-5 {
        return (None, 0.0);
    }
    let mean = chroma.iter().sum::<f32>() / 12.0;
    let deviation = chroma
        .iter()
        .map(|x| (x - mean).powi(2))
        .sum::<f32>()
        .sqrt();
    if deviation / norm < 0.16 {
        return (None, 0.0);
    }
    let mut scores = Vec::new();
    for (mode, profile) in [major, minor].iter().enumerate() {
        for root in 0..12 {
            let pmean = profile.iter().sum::<f32>() / 12.0;
            let pnorm = profile
                .iter()
                .map(|x| (x - pmean).powi(2))
                .sum::<f32>()
                .sqrt();
            let score = (0..12)
                .map(|i| (chroma[(i + root) % 12] - mean) * (profile[i] - pmean))
                .sum::<f32>()
                / (deviation * pnorm);
            scores.push((score, root, mode));
        }
    }
    scores.sort_by(|a, b| b.0.total_cmp(&a.0));
    let confidence = ((scores[0].0 - scores[1].0) * 4.0).clamp(0.0, 1.0);
    if scores[0].0 < 0.55 || confidence < 0.12 {
        return (None, confidence);
    }
    (
        Some(format!(
            "{}{}",
            [
                "C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"
            ][scores[0].1],
            if scores[0].2 == 1 { "m" } else { "" }
        )),
        confidence,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detects_known_pulse_and_rejects_silence() {
        let mut onset = vec![0.0; 12000];
        for i in (0..12000).step_by(100) {
            onset[i] = 1.0;
        }
        let (bpm, confidence) = estimate_bpm(&onset, 200.0);
        assert!((bpm.unwrap() - 120.0).abs() < 0.1);
        assert!(confidence > 0.8);
        assert_eq!(estimate_bpm(&vec![0.0; 12000], 200.0).0, None);
    }
    #[test]
    fn silence_has_no_forced_key() {
        assert_eq!(estimate_key(&vec![0.0; 8192], 4000.0).0, None);
    }
}
