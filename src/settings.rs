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
fn valid(s: &Spec, value: f32) -> bool {
    value.is_finite()
        && (s.min..=s.max).contains(&value)
        && (!(s.key.starts_with('b') || s.key.starts_with('i')) || value.fract() == 0.)
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
            let (section, key) =
                if let Some(s) = spec(key).filter(|s| s.section.eq_ignore_ascii_case(&section)) {
                    // Retain malformed values too. Only this setting falls back at
                    // read time, and unrelated edits must not erase the user's data.
                    if !value.parse::<f32>().is_ok_and(|v| valid(s, v)) {
                        log::warn!("settings: invalid {}; using its default", s.key);
                    }
                    (s.section.to_string(), s.key.to_string())
                } else {
                    (section.clone(), key.to_string())
                };
            result.values.insert((section, key), value.into());
        }
        result.dirty = false;
        Ok(result)
    }
    pub fn get(&self, key: &str) -> f32 {
        let s = spec(key).expect("known setting");
        self.values
            .get(&(s.section.into(), s.key.into()))
            .and_then(|v| v.parse().ok())
            .filter(|v| valid(s, *v))
            .unwrap_or(s.default)
    }
    pub fn enabled(&self, key: &str) -> bool {
        self.get(key) != 0.
    }
    pub fn set(&mut self, key: &str, value: f32) -> io::Result<()> {
        let s = spec(key)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "unknown setting"))?;
        if !valid(s, value) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{} must be in {}..{}", s.key, s.min, s.max),
            ));
        }
        let invalid = self
            .values
            .get(&(s.section.into(), s.key.into()))
            .is_some_and(|raw| !raw.parse::<f32>().is_ok_and(|v| valid(s, v)));
        if self.get(key) != value || invalid {
            self.values
                .insert((s.section.into(), s.key.into()), value.to_string());
            self.dirty = true;
        }
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
        if !self.dirty {
            return Ok(());
        }
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
    pub fn subtitles(&self, dialogue: bool) -> bool {
        self.subtitle_override.unwrap_or_else(|| {
            self.settings.enabled(if dialogue {
                "bDialogueSubtitles"
            } else {
                "bGeneralSubtitles"
            })
        })
    }
    pub fn sync_settings(&mut self) {
        self.renderer.distances.trees = self.settings.get("fTreesMidLODSwitchDist");
        self.renderer.distances.specular = self.settings.get("fSpecularLODStartFade");
        self.renderer.distances.lights = self.settings.get("fLightLODStartFade");
        self.renderer.distances.shadows = self.settings.get("fShadowDistance");
        self.renderer.distances.grass = self.settings.get("fGrassStartFadeDistance");
        self.renderer.distances.objects = self.settings.get("fLODFadeOutMultObjects");
        self.renderer.distances.items = self.settings.get("fLODFadeOutMultItems");
        self.renderer.distances.actors = self.settings.get("fLODFadeOutMultActors");
        self.renderer.gamma = self.settings.get("fGamma");
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
        // Audio preferences are persisted by set_audio_volume when changed.
        if self.settings.dirty
            && let Some(path) = path()
        {
            self.settings.write(&path)?;
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_values_are_preserved_and_clean_settings_do_not_write() {
        let dir = std::env::temp_dir().join(format!("vrm-settings-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.ini");
        let original = "[GamePlay]\niDifficulty=broken\n[Display]\nfGamma=NaN\n[Controls]\nbInvertYValues=1\n[Future]\nexample=kept\n";
        std::fs::write(&path, original).unwrap();
        let mut s = Settings::parse(original).unwrap();
        assert_eq!(s.get("iDifficulty"), 2.);
        assert_eq!(s.get("fGamma"), 1.);
        assert!(s.enabled("bInvertYValues"));
        assert!(!s.dirty);
        s.write(&path).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        s.set("bInvertYValues", 1.).unwrap();
        assert!(!s.dirty);
        s.set("bCrosshairEnabled", 0.).unwrap();
        s.write(&path).unwrap();
        let saved = std::fs::read_to_string(&path).unwrap();
        for value in ["iDifficulty=broken", "fGamma=NaN", "example=kept"] {
            assert!(saved.contains(value));
        }
        assert!(!s.dirty);
        // Explicitly selecting the default repairs the invalid value too.
        s.set("iDifficulty", 2.).unwrap();
        assert!(s.dirty);
        s.write(&path).unwrap();
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains("iDifficulty=2")
        );
        let absent = dir.join("untouched.ini");
        Settings::default().write(&absent).unwrap();
        assert!(!absent.exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

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
/// Movie labels are asset keys, independent of translated display strings.
pub fn option_key(label: &str) -> Option<&'static str> {
    Some(match label {
        "$Invert Y" => "bInvertYValues",
        "$Look Sensitivity" => "fMouseHeadingSensitivity",
        "$Difficulty" => "iDifficulty",
        "$Show Floating Markers" => "bShowFloatingQuestMarkers",
        "$Save on Rest" => "bSaveOnRest",
        "$Save on Wait" => "bSaveOnWait",
        "$Save on Travel" => "bSaveOnTravel",
        "$Save on Pause" => "bSaveOnPause",
        "$HUD Opacity" => "fHUDOpacity",
        "$Crosshair" => "bCrosshairEnabled",
        "$Dialogue Subtitles" => "bDialogueSubtitles",
        "$General Subtitles" => "bGeneralSubtitles",
        "$Brightness" => "fGamma",
        "$Actor Fade" => "fLODFadeOutMultActors",
        "$Item Fade" => "fLODFadeOutMultItems",
        "$Object Fade" => "fLODFadeOutMultObjects",
        "$Grass Fade" => "fGrassStartFadeDistance",
        "$Shadow Fade" => "fShadowDistance",
        "$Light Fade" => "fLightLODStartFade",
        "$Specularity Fade" => "fSpecularLODStartFade",
        "$Tree LOD Fade" => "fTreesMidLODSwitchDist",
        _ => return None,
    })
}
pub const PAUSE_MINUTES: [u32; 6] = [5, 10, 15, 30, 45, 60];
pub fn option_value(settings: &Settings, key: &str, defaults: bool) -> f32 {
    let s = spec(key).unwrap();
    let v = if defaults {
        s.default
    } else {
        settings.get(key)
    };
    if key == "bSaveOnPause" {
        if v == 0. {
            6.
        } else {
            PAUSE_MINUTES
                .iter()
                .position(|m| *m == settings.get("fAutosaveEveryXMins") as u32)
                .unwrap_or(2) as f32
        }
    } else if key.starts_with('f') {
        (v - s.min) / (s.max - s.min)
    } else {
        v
    }
}
impl crate::engine::Engine {
    pub fn change_option(&mut self, id: usize, value: f32) -> io::Result<()> {
        let s = SPECS
            .get(id)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "unknown option"))?;
        if !value.is_finite() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid option value",
            ));
        }
        if s.key == "bSaveOnPause" {
            if value.fract() != 0. || !(0. ..=6.).contains(&value) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "invalid pause interval",
                ));
            }
            self.set_setting("bSaveOnPause", if value == 6. { 0. } else { 1. })?;
            if value < 6. {
                self.set_setting("fAutosaveEveryXMins", PAUSE_MINUTES[value as usize] as f32)?;
            }
            Ok(())
        } else {
            self.set_setting(
                s.key,
                if s.key.starts_with('f') {
                    s.min + value.clamp(0., 1.) * (s.max - s.min)
                } else {
                    value
                },
            )
        }
    }
    pub fn setting_command(&mut self, args: &[&str]) -> Vec<String> {
        let Some(key) = args.first() else {
            return SPECS
                .iter()
                .map(|s| format!("{} = {}", s.key, self.settings.get(s.key)))
                .collect();
        };
        let Some(s) = spec(key) else {
            return vec![format!("unknown setting {key}")];
        };
        if let Some(value) = args.get(1) {
            let result = value
                .parse::<f32>()
                .map_err(|e| e.to_string())
                .and_then(|v| self.set_setting(s.key, v).map_err(|e| e.to_string()))
                .and_then(|_| self.save_settings().map_err(|e| e.to_string()));
            if let Err(e) = result {
                return vec![e];
            }
        }
        vec![format!("{} = {}", s.key, self.settings.get(s.key))]
    }
}
