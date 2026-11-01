use anyhow::{Context, Result};
use djejaj_audio::Settings;
use djejaj_library::Track;
use djejaj_sampler::Sequence;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Binding {
    pub name: String,
    pub less: String,
    pub more: String,
}
pub fn default_bindings() -> Vec<Binding> {
    [
        ("Filter", "Q", "A"),
        ("Delay", "W", "S"),
        ("Reverb", "E", "D"),
        ("Drive", "R", "F"),
        ("Tempo", "T", "G"),
        ("Gain", "O", "L"),
        ("Low", "Y", "H"),
        ("Mid", "U", "J"),
        ("High", "I", "K"),
    ]
    .iter()
    .map(|(name, less, more)| Binding {
        name: name.to_string(),
        less: less.to_string(),
        more: more.to_string(),
    })
    .collect()
}
pub fn validate_bindings(bindings: &[Binding]) -> Result<()> {
    anyhow::ensure!(
        bindings.len() == 9,
        "Profil wymaga dziewięciu par regulatorów"
    );
    let mut used = std::collections::HashSet::new();
    for b in bindings {
        for k in [&b.less, &b.more] {
            let key = k.to_uppercase();
            anyhow::ensure!(
                key.len() == 1 && key.as_bytes()[0].is_ascii_uppercase(),
                "Użyj pojedynczej litery A–Z"
            );
            anyhow::ensure!(
                !["Z", "X", "C", "V", "B", "N", "M", "P"].contains(&key.as_str()),
                "{key} jest przypisany do pada"
            );
            anyhow::ensure!(
                used.insert(key.clone()),
                "Klawisz {key} jest użyty dwukrotnie"
            );
        }
    }
    Ok(())
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    pub schema: u32,
    pub settings: Settings,
    pub sequence: Sequence,
    pub tracks: [Option<Track>; 2],
    pub queue: Vec<Track>,
    pub bindings: Vec<Binding>,
    pub folder: String,
    pub sample_paths: [Option<String>; 8],
    pub sample_configs: [djejaj_sampler::SampleConfig; 8],
    pub scale_root: u8,
    pub scale_minor: bool,
}
impl Default for Session {
    fn default() -> Self {
        Self {
            schema: 1,
            settings: Settings::default(),
            sequence: Sequence::default(),
            tracks: [None, None],
            queue: Vec::new(),
            bindings: default_bindings(),
            folder: String::new(),
            sample_paths: std::array::from_fn(|_| None),
            sample_configs: djejaj_sampler::default_sample_configs(),
            scale_root: 9,
            scale_minor: true,
        }
    }
}
impl Session {
    pub fn validate(&self) -> Result<()> {
        anyhow::ensure!(self.schema == 1, "Nieobsługiwana wersja sesji");
        self.settings.validate()?;
        self.sequence.validate()?;
        anyhow::ensure!(self.scale_root < 12, "Nieprawidłowa tonika skali");
        for c in self.sample_configs {
            anyhow::ensure!(
                c.volume.is_finite()
                    && (0.0..=1.0).contains(&c.volume)
                    && c.pan.is_finite()
                    && (-1.0..=1.0).contains(&c.pan)
                    && c.pitch.is_finite()
                    && (0.5..=2.0).contains(&c.pitch),
                "Nieprawidłowe parametry sampla"
            );
        }
        validate_bindings(&self.bindings)?;
        Ok(())
    }
}
pub fn data_dir() -> PathBuf {
    std::env::var_os("DJEJAJ_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                .join("Library/Application Support/DJejaj")
        })
}
pub fn load(path: &Path) -> Result<Session> {
    let data = std::fs::read(path)?;
    let s: Session = serde_json::from_slice(&data)?;
    s.validate()?;
    Ok(s)
}
pub fn save(path: &Path, session: &Session) -> Result<()> {
    session.validate()?;
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(session)?)?;
    if path.exists() {
        std::fs::copy(path, path.with_extension("json.bak"))
            .context("Nie można utworzyć kopii poprzedniej sesji")?;
    }
    std::fs::rename(tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn binding_conflicts_are_rejected() {
        let mut b = default_bindings();
        assert!(validate_bindings(&b).is_ok());
        b[0].less = "Z".into();
        assert!(validate_bindings(&b).is_err());
        b[0].less = "W".into();
        assert!(validate_bindings(&b).is_err());
    }
    #[test]
    fn future_session_version_is_not_silently_reset() {
        let s = Session {
            schema: 2,
            ..Session::default()
        };
        assert!(s.validate().is_err());
    }
}
