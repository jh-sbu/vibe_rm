//! AudioMenu-compatible preferences, independent of devices and saves.
use esp::{FormId, LoadOrder};
use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
};
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub master: f32,
    pub categories: BTreeMap<u32, f32>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            master: 1.0,
            categories: BTreeMap::new(),
        }
    }
}
fn valid(value: f32) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}
impl Settings {
    pub fn parse(text: &str) -> io::Result<Self> {
        let mut result = Self::default();
        let mut active = false;
        let mut ids = BTreeMap::new();
        let mut values = BTreeMap::new();
        for line in text.lines().map(str::trim) {
            if line.starts_with('[') {
                active = line.eq_ignore_ascii_case("[AudioMenu]");
                continue;
            }
            if !active || line.starts_with([';', '#']) {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim().to_ascii_lowercase();
            let value = value.split([';', '#']).next().unwrap_or("").trim();
            let bad = || {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid AudioMenu {key}"),
                )
            };
            if key == "faudiomastervolume" {
                result.master = value.parse().ok().filter(|v| valid(*v)).ok_or_else(bad)?;
            } else if let Some(index) = key
                .strip_prefix("uid")
                .and_then(|n| n.parse::<usize>().ok())
            {
                ids.insert(index, value.parse::<u32>().map_err(|_| bad())?);
            } else if let Some(index) = key
                .strip_prefix("fval")
                .and_then(|n| n.parse::<usize>().ok())
            {
                values.insert(
                    index,
                    value
                        .parse::<f32>()
                        .ok()
                        .filter(|v| valid(*v))
                        .ok_or_else(bad)?,
                );
            }
        }
        for (index, id) in ids {
            if let Some(&value) = values.get(&index) {
                result.categories.insert(id, value);
            }
        }
        Ok(result)
    }
    pub fn encode(&self) -> String {
        let mut text = format!("[AudioMenu]\nfAudioMasterVolume={}\n", self.master);
        for (i, (id, value)) in self.categories.iter().enumerate() {
            text.push_str(&format!("uID{i}={id}\nfVal{i}={value}\n"));
        }
        text
    }
    pub fn read(path: &Path) -> io::Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(s) => Self::parse(&s),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e),
        }
    }
    pub fn write(&self, path: &Path) -> io::Result<()> {
        if !valid(self.master) || self.categories.values().any(|v| !valid(*v)) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "audio volumes must be finite and between 0 and 1",
            ));
        }
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let mut temp = path.as_os_str().to_owned();
        temp.push(format!(".{}.tmp", std::process::id()));
        let temp = PathBuf::from(temp);
        std::fs::write(&temp, self.encode())?;
        std::fs::rename(temp, path)
    }
}

/// Record metadata is read once, not rebuilt while the engine is ticking.
#[derive(Clone, Debug)]
pub struct MenuCategory {
    pub id: FormId,
    pub name: String,
    pub default: f32,
}
#[derive(Default)]
pub struct Overrides {
    pub master: Option<f32>,
    pub categories: BTreeMap<u32, f32>,
}
pub struct Runtime {
    pub menus: Vec<MenuCategory>,
    pub overrides: Overrides,
}
impl Runtime {
    pub fn load(lo: &LoadOrder) -> Self {
        let menus = lo
            .ids_of_type(b"SNCT")
            .iter()
            .filter_map(|&id| {
                let category = crate::world::sound::category(lo, id).filter(|c| c.in_menu)?;
                Some(MenuCategory {
                    id,
                    name: lo.get(id)?.editor_id().unwrap_or_default(),
                    default: category.menu,
                })
            })
            .collect();
        Self::with_environment(menus, |name| std::env::var(name).ok())
    }
    fn with_environment(menus: Vec<MenuCategory>, get: impl Fn(&str) -> Option<String>) -> Self {
        let volume = |name: &str| get(name)?.parse::<f32>().ok().filter(|v| valid(*v));
        let mut overrides = Overrides {
            master: volume("VRM_AUDIO_MASTER"),
            ..Default::default()
        };
        for category in &menus {
            let alias = alias_for(&category.name);
            let value = volume(&format!("VRM_AUDIO_{:08X}", category.id.0)).or_else(|| {
                if alias.is_empty() {
                    None
                } else {
                    volume(&format!("VRM_AUDIO_{alias}"))
                }
            });
            if let Some(value) = value {
                overrides.categories.insert(category.id.0, value);
            }
        }
        Self { menus, overrides }
    }
    pub fn master(&self, saved: &Settings) -> f32 {
        self.overrides.master.unwrap_or(saved.master)
    }
    pub fn category(&self, saved: &Settings, id: FormId) -> Option<f32> {
        let category = self.menus.iter().find(|c| c.id == id)?;
        Some(
            self.overrides
                .categories
                .get(&id.0)
                .or_else(|| saved.categories.get(&id.0))
                .copied()
                .unwrap_or(category.default),
        )
    }
    pub fn gains(&self, saved: &Settings) -> std::collections::HashMap<u32, f32> {
        self.menus
            .iter()
            .map(|c| (c.id.0, self.category(saved, c.id).unwrap()))
            .collect()
    }
}
fn alias_for(edid: &str) -> &'static str {
    match edid.to_ascii_lowercase().as_str() {
        "audiocategorysfx" => "EFFECTS",
        "audiocategoryfst" => "FOOTSTEPS",
        "audiocategoryvocgeneral" => "VOICE",
        "audiocategorymus" => "MUSIC",
        _ => "",
    }
}
pub fn path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("VRM_AUDIO_SETTINGS") {
        return Some(PathBuf::from(path));
    }
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .map(|base| base.join("vibe_rm/audio.ini"))
}
impl crate::engine::Engine {
    pub(crate) fn refresh_audio_settings(&mut self) {
        self.audio_director.menu_gains = self.audio_preferences.gains(&self.audio_settings);
        if let Some(a) = &self.audio {
            a.set_master(self.audio_preferences.master(&self.audio_settings));
        }
    }
    /// Shared console/System-page API. Settings are separate from save games.
    pub fn set_audio_volume(&mut self, category: Option<FormId>, value: f32) -> io::Result<()> {
        if !valid(value)
            || category.is_some_and(|id| !self.audio_preferences.menus.iter().any(|c| c.id == id))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "choose master or an Audio menu category and a value from 0 to 1",
            ));
        }
        if let Some(id) = category {
            self.audio_settings.categories.insert(id.0, value);
        } else {
            self.audio_settings.master = value;
        }
        self.refresh_audio_settings();
        if let Some(path) = &self.audio_settings_path {
            self.audio_settings.write(path)?;
        }
        Ok(())
    }
    pub(crate) fn audio_command(&mut self, args: &[&str]) -> Vec<String> {
        let id = |name: &str| {
            self.audio_preferences
                .menus
                .iter()
                .find(|c| {
                    c.name.eq_ignore_ascii_case(name)
                        || (!alias_for(&c.name).is_empty()
                            && alias_for(&c.name).eq_ignore_ascii_case(name))
                })
                .map(|c| c.id)
                .or_else(|| self.resolve_form(name))
        };
        if args.is_empty() {
            let mut lines = vec![format!(
                "master = {}",
                self.audio_preferences.master(&self.audio_settings)
            )];
            for category in &self.audio_preferences.menus {
                lines.push(format!(
                    "{} {} = {}",
                    category.id,
                    category.name,
                    self.audio_preferences
                        .category(&self.audio_settings, category.id)
                        .unwrap()
                ));
            }
            return lines;
        }
        let category = if args[0].eq_ignore_ascii_case("master") {
            None
        } else {
            match id(args[0]).filter(|id| self.audio_preferences.menus.iter().any(|c| c.id == *id))
            {
                Some(id) => Some(id),
                None => return vec!["unknown Audio menu category".into()],
            }
        };
        if args.len() == 1 {
            let value = category.map_or_else(
                || self.audio_preferences.master(&self.audio_settings),
                |id| {
                    self.audio_preferences
                        .category(&self.audio_settings, id)
                        .unwrap()
                },
            );
            return vec![format!("{} = {value}", args[0])];
        }
        if args.len() != 2 {
            return vec!["usage: audio [master|category [value]]".into()];
        }
        let Some(value) = args[1].parse::<f32>().ok().filter(|v| valid(*v)) else {
            return vec!["volume must be a finite number from 0 to 1".into()];
        };
        match self.set_audio_volume(category, value) {
            Ok(()) => {
                let effective = category.map_or_else(
                    || self.audio_preferences.master(&self.audio_settings),
                    |id| {
                        self.audio_preferences
                            .category(&self.audio_settings, id)
                            .unwrap()
                    },
                );
                vec![if effective == value {
                    format!("{} = {value}", args[0])
                } else {
                    format!(
                        "{} = {value} (effective {effective}: environment override)",
                        args[0]
                    )
                }]
            }
            Err(e) => vec![format!("audio settings: {e}")],
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn environment_overrides_stay_separate_when_preferences_are_saved() {
        let prefs = Runtime::with_environment(
            vec![MenuCategory {
                id: FormId(123),
                name: "AudioCategoryMUS".into(),
                default: 0.5,
            }],
            |name| match name {
                "VRM_AUDIO_MASTER" => Some("0".into()),
                "VRM_AUDIO_MUSIC" => Some(".2".into()),
                "VRM_AUDIO_0000007B" => Some(".1".into()),
                _ => None,
            },
        );
        let mut saved = Settings {
            master: 0.8,
            categories: [(123, 0.4)].into(),
        };
        assert_eq!(prefs.master(&saved), 0.0);
        assert_eq!(prefs.category(&saved, FormId(123)), Some(0.1));
        // A subsequent console/API change must save only the user's requested preference.
        saved.categories.insert(123, 0.6);
        let roundtrip = Settings::parse(&saved.encode()).unwrap();
        assert_eq!(roundtrip.master, 0.8);
        assert_eq!(roundtrip.categories[&123], 0.6);
        assert_eq!(prefs.master(&roundtrip), 0.0);
        assert_eq!(prefs.gains(&roundtrip)[&123], 0.1);
        let next = Runtime::with_environment(prefs.menus.clone(), |_| None);
        assert_eq!(next.master(&roundtrip), 0.8);
        assert_eq!(next.gains(&roundtrip)[&123], 0.6);
    }
    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn shared_slider_api_updates_director_and_persists_without_a_device() {
        use esp::writer::{Node, RecordBuilder, write_plugin};
        let dir = std::env::temp_dir().join(format!("vrm-audio-slider-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let records = [Node::top(
            b"SNCT",
            vec![
                RecordBuilder::new(b"SNCT", FormId(0x800))
                    .u32(b"FNAM", 2)
                    .field(b"UNAM", &32767u16.to_le_bytes()),
            ],
        )];
        std::fs::write(
            dir.join("Audio.esp"),
            write_plugin(0, &[], "test", "slider", 0x900, &records),
        )
        .unwrap();
        let mut e = crate::stats::action_tests::headless_engine(&dir, "Audio.esp");
        e.audio_settings = Settings::default();
        e.audio_settings_path = Some(dir.join("audio.ini"));
        e.audio_preferences.overrides.master = Some(0.0);
        e.set_audio_volume(Some(FormId(0x800)), 0.2).unwrap();
        assert_eq!(Settings::read(&dir.join("audio.ini")).unwrap().master, 1.0);
        assert_eq!(e.audio_preferences.master(&e.audio_settings), 0.0);
        e.update_audio_director();
        assert_eq!(e.audio_director.targets[&0x800].state.volume, 0.2);
        assert_eq!(
            Settings::read(&dir.join("audio.ini")).unwrap().categories[&0x800],
            0.2
        );
        assert!(e.set_audio_volume(Some(FormId(0x999)), 0.4).is_err());
        assert!(e.set_audio_volume(None, f32::NAN).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn settings_round_trip_decimal_ids_and_reject_invalid_values() {
        let s = Settings {
            master: 0.7,
            categories: [(0x71E64, 0.3), (123, 0.0)].into(),
        };
        assert_eq!(Settings::parse(&s.encode()).unwrap(), s);
        assert_eq!(
            Settings::parse("[Other]\nfAudioMasterVolume=0\n[AudioMenu]\nfVal2=.4\nuID2=123\n")
                .unwrap()
                .categories[&123],
            0.4
        );
        for bad in ["NaN", "inf", "-1", "1.1"] {
            assert!(Settings::parse(&format!("[AudioMenu]\nfAudioMasterVolume={bad}")).is_err());
        }
        let dir = std::env::temp_dir().join(format!("vrm-audio-settings-{}", std::process::id()));
        let path = dir.join("audio.ini");
        s.write(&path).unwrap();
        assert_eq!(Settings::read(&path).unwrap(), s);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
