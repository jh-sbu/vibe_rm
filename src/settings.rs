//! User preferences live outside saves. Original INI names/sections are kept.
use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
};
#[derive(Clone, Copy)]
pub struct Spec {
    pub section: &'static str,
    pub key: &'static str,
    pub default: f32,
    pub min: f32,
    pub max: f32,
}
macro_rules! spec {
    ($s:literal,$k:literal,$d:expr,$min:expr,$max:expr) => {
        Spec {
            section: $s,
            key: $k,
            default: $d,
            min: $min,
            max: $max,
        }
    };
}
pub const SPECS: &[Spec] = &[
    spec!("Controls", "bInvertYValues", 0., 0., 1.),
    spec!("Controls", "fMouseHeadingSensitivity", 0.0125, 0., 0.025),
    spec!("Controls", "bGamePadRumble", 1., 0., 1.),
    spec!("Controls", "bAlwaysRunByDefault", 1., 0., 1.),
    spec!("MAIN", "bGamepadEnable", 1., 0., 1.),
    spec!("GamePlay", "iDifficulty", 2., 0., 5.),
    spec!("GamePlay", "bShowFloatingQuestMarkers", 1., 0., 1.),
    spec!("MAIN", "bSaveOnRest", 1., 0., 1.),
    spec!("MAIN", "bSaveOnWait", 1., 0., 1.),
    spec!("MAIN", "bSaveOnTravel", 1., 0., 1.),
    spec!("MAIN", "bSaveOnPause", 0., 0., 1.),
    spec!("SaveGame", "fAutosaveEveryXMins", 15., 5., 60.),
    spec!("MAIN", "fHUDOpacity", 1., 0., 1.),
    spec!("MAIN", "bCrosshairEnabled", 1., 0., 1.),
    spec!("Interface", "bDialogueSubtitles", 0., 0., 1.),
    spec!("Interface", "bGeneralSubtitles", 0., 0., 1.),
    spec!("Display", "fGamma", 1., 0.5, 1.5),
    spec!("LOD", "fLODFadeOutMultActors", 15., 1., 15.),
    spec!("LOD", "fLODFadeOutMultItems", 15., 1., 15.),
    spec!("LOD", "fLODFadeOutMultObjects", 15., 1., 15.),
    spec!("Grass", "fGrassStartFadeDistance", 7000., 0., 7000.),
    spec!("Display", "fShadowDistance", 8000., 0., 16000.),
    spec!("Display", "fLightLODStartFade", 3500., 0., 7000.),
    spec!("Display", "fSpecularLODStartFade", 2000., 0., 4000.),
    spec!("Display", "fTreesMidLODSwitchDist", 9999999., 0., 9999999.),
    spec!("Display", "fDynamicDOFBlurMultiplier", 0.8, 0., 1.),
];
pub fn spec(key: &str) -> Option<&'static Spec> {
    SPECS.iter().find(|s| s.key.eq_ignore_ascii_case(key))
}
#[derive(Clone, Debug, Default)]
pub struct Settings {
    values: BTreeMap<(String, String), String>,
    pub dirty: bool,
}
impl Settings {
    pub fn parse(text: &str) -> io::Result<Self> {
        let mut result = Self::default();
        let mut section = String::new();
        for line in text.lines().map(str::trim) {
            if line.starts_with([';', '#']) || line.is_empty() {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                section = line[1..line.len() - 1].to_string();
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.split([';', '#']).next().unwrap_or("").trim();
            if let Some(s) = spec(key).filter(|s| s.section.eq_ignore_ascii_case(&section)) {
                result.set(
                    s.key,
                    value.parse().map_err(|_| {
                        io::Error::new(io::ErrorKind::InvalidData, format!("invalid {}", s.key))
                    })?,
                )?;
            } else {
                result
                    .values
                    .insert((section.clone(), key.into()), value.into());
            }
        }
        result.dirty = false;
        Ok(result)
    }
    pub fn get(&self, key: &str) -> f32 {
        let s = spec(key).expect("known setting");
        self.values
            .get(&(s.section.into(), s.key.into()))
            .and_then(|v| v.parse().ok())
            .unwrap_or(s.default)
    }
    pub fn enabled(&self, key: &str) -> bool {
        self.get(key) != 0.
    }
    pub fn set(&mut self, key: &str, value: f32) -> io::Result<()> {
        let s = spec(key)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "unknown setting"))?;
        if !value.is_finite()
            || !(s.min..=s.max).contains(&value)
            || ((s.key.starts_with('b') || s.key.starts_with('i')) && value.fract() != 0.)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{} must be in {}..{}", s.key, s.min, s.max),
            ));
        }
        self.values
            .insert((s.section.into(), s.key.into()), value.to_string());
        self.dirty = true;
        Ok(())
    }
    pub fn encode(&self) -> String {
        let mut entries = self.values.clone();
        for s in SPECS {
            entries
                .entry((s.section.into(), s.key.into()))
                .or_insert_with(|| s.default.to_string());
        }
        let mut out = String::new();
        let mut previous = String::new();
        for ((section, key), value) in entries {
            if section != previous {
                out.push_str(&format!("\n[{section}]\n"));
                previous = section;
            }
            out.push_str(&format!("{key}={value}\n"));
        }
        out
    }
    pub fn load() -> Self {
        match path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|t| Self::parse(&t))
        {
            Some(Ok(s)) => s,
            Some(Err(e)) => {
                log::warn!("settings: {e}");
                Self::default()
            }
            None => Self::default(),
        }
    }
    pub fn write(&mut self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let temp = path.with_extension(format!("{}.tmp", std::process::id()));
        std::fs::write(&temp, self.encode())?;
        std::fs::rename(temp, path)?;
        self.dirty = false;
        Ok(())
    }
}
pub fn config_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")))
        .map(|p| p.join("vibe_rm"))
}
pub fn path() -> Option<PathBuf> {
    std::env::var_os("VRM_SETTINGS")
        .map(PathBuf::from)
        .or_else(|| config_dir().map(|p| p.join("settings.ini")))
}
impl crate::engine::Engine {
    pub fn sync_settings(&mut self) {
        self.autosave.rest = self.settings.enabled("bSaveOnRest");
        self.autosave.wait = self.settings.enabled("bSaveOnWait");
        self.autosave.travel = self.settings.enabled("bSaveOnTravel");
        self.autosave.pause_minutes = self
            .settings
            .enabled("bSaveOnPause")
            .then(|| self.settings.get("fAutosaveEveryXMins") as u32);
    }
    pub fn set_setting(&mut self, key: &str, value: f32) -> io::Result<()> {
        self.settings.set(key, value)?;
        self.sync_settings();
        Ok(())
    }
    pub fn save_settings(&mut self) -> io::Result<()> {
        if let Some(path) = path() {
            self.settings.write(&path)?;
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trip_unknown_keys_and_validation() {
        let mut s =
            Settings::parse("[Controls]\nbInvertYValues=1\n[Future]\nexample=kept\n").unwrap();
        assert!(s.enabled("bInvertYValues"));
        s.set("iDifficulty", 5.).unwrap();
        let back = Settings::parse(&s.encode()).unwrap();
        assert_eq!(back.get("iDifficulty"), 5.);
        assert!(back.encode().contains("example=kept"));
        assert!(s.set("iDifficulty", 2.5).is_err());
        assert!(s.set("fGamma", f32::NAN).is_err());
    }
}
