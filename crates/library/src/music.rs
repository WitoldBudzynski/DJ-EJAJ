//! Musical hints are explainable heuristics; uncertain keys remain unknown.
use crate::Track;
fn key(value: &str) -> Option<(i32, bool)> {
    let minor = value.ends_with('m');
    let name = value.trim_end_matches('m');
    Some((
        match name {
            "C" => 0,
            "C#" | "Db" => 1,
            "D" => 2,
            "D#" | "Eb" => 3,
            "E" => 4,
            "F" => 5,
            "F#" | "Gb" => 6,
            "G" => 7,
            "G#" | "Ab" => 8,
            "A" => 9,
            "A#" | "Bb" => 10,
            "B" => 11,
            _ => return None,
        },
        minor,
    ))
}
pub fn harmonic(a: &str, b: &str, source_speed: f32, key_lock: bool) -> Option<&'static str> {
    let (mut ar, am) = key(a)?;
    let (br, bm) = key(b)?;
    if !key_lock {
        let semitones = 12.0 * source_speed.log2();
        if !semitones.is_finite() || (semitones - semitones.round()).abs() > 0.12 {
            return None;
        }
        ar = (ar + semitones.round() as i32).rem_euclid(12);
    }
    if ar == br && am == bm {
        Some("ta sama tonacja")
    } else if am == bm && [5, 7].contains(&(br - ar).rem_euclid(12)) {
        Some("sąsiednia tonacja")
    } else if am != bm
        && (if am {
            (br - ar).rem_euclid(12) == 3
        } else {
            (ar - br).rem_euclid(12) == 3
        })
    {
        Some("tonacja paralelna")
    } else {
        Some("różne tonacje — odsłuch")
    }
}
pub fn suggestion(
    reference: &Track,
    candidate: &Track,
    bpm: f32,
    speed: f32,
    key_lock: bool,
) -> (f32, String) {
    let delta = candidate.bpm.map(|b| b - bpm);
    let relation = reference
        .key
        .as_deref()
        .zip(candidate.key.as_deref())
        .and_then(|(a, b)| harmonic(a, b, speed, key_lock));
    let mut score = -delta.map_or(20.0, |d| d.abs() * 2.0);
    score += match relation {
        Some("ta sama tonacja") => 40.0,
        Some("sąsiednia tonacja") | Some("tonacja paralelna") => 25.0,
        _ => 0.0,
    };
    score -= (reference.energy as f32 - candidate.energy as f32).abs() * 4.0;
    if candidate.played {
        score -= 20.0;
    }
    if candidate.missing || candidate.error.is_some() {
        score -= 1000.0;
    }
    (
        score,
        format!(
            "{} · {} · energia {}→{}{}",
            delta.map_or("BPM nieznane".into(), |d| format!("{d:+.1} BPM")),
            relation.unwrap_or("tonacja niepewna / przestrojenie"),
            reference.energy,
            candidate.energy,
            if candidate.played {
                " · zagrany"
            } else {
                " · niezagrany"
            }
        ),
    )
}
pub fn scale_note(note: u8, root: u8, minor: bool) -> u8 {
    let scale = if minor {
        [0, 2, 3, 5, 7, 8, 10]
    } else {
        [0, 2, 4, 5, 7, 9, 11]
    };
    (0..=127)
        .filter(|n| scale.contains(&((*n as i32 - root as i32).rem_euclid(12) as u8)))
        .min_by_key(|n| (*n as i32 - note as i32).abs())
        .unwrap_or(note)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn harmonic_hints_respect_detuning_and_relative_keys() {
        assert_eq!(harmonic("Am", "C", 1.0, true), Some("tonacja paralelna"));
        assert_eq!(harmonic("Am", "Em", 1.0, true), Some("sąsiednia tonacja"));
        assert_eq!(harmonic("Am", "Am", 1.03, false), None);
        assert_eq!(harmonic("?", "C", 1.0, true), None);
    }
    #[test]
    fn quantized_notes_stay_in_selected_scale() {
        for note in 0..127 {
            assert!(
                [0, 2, 3, 5, 7, 8, 10]
                    .contains(&((scale_note(note, 9, true) as i32 - 9).rem_euclid(12) as u8))
            );
        }
    }
}
