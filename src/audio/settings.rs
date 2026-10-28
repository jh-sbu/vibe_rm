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
    pub fn apply_environment(&mut self, lo: &LoadOrder) {
        if let Some(value) = env_volume("VRM_AUDIO_MASTER") {
            self.master = value;
        }
        for &id in lo.ids_of_type(b"SNCT") {
            if !crate::world::sound::category(lo, id).is_some_and(|c| c.in_menu) {
                continue;
            }
            let alias = lo
                .get(id)
                .and_then(|r| r.editor_id().as_deref().map(alias_for))
                .unwrap_or("");
            let value = if alias.is_empty() {
                None
            } else {
                env_volume(&format!("VRM_AUDIO_{alias}"))
            };
            if let Some(value) = env_volume(&format!("VRM_AUDIO_{:08X}", id.0)).or(value) {
                self.categories.insert(id.0, value);
            }
        }
    }
}
fn env_volume(name: &str) -> Option<f32> {
    std::env::var(name).ok()?.parse().ok().filter(|v| valid(*v))
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
        self.audio_director.menu_gains.clear();
        for &id in self.lo.ids_of_type(b"SNCT") {
            if let Some(c) = crate::world::sound::category(&self.lo, id).filter(|c| c.in_menu) {
                self.audio_director.menu_gains.insert(
                    id.0,
                    self.audio_settings
                        .categories
                        .get(&id.0)
                        .copied()
                        .unwrap_or(c.menu),
                );
            }
        }
        if let Some(a) = &self.audio {
            a.set_master(self.audio_settings.master);
        }
    }
    /// Shared console/System-page API. Settings are separate from save games.
    pub fn set_audio_volume(&mut self, category: Option<FormId>, value: f32) -> io::Result<()> {
        if !valid(value)
            || category.is_some_and(|id| {
                !crate::world::sound::category(&self.lo, id).is_some_and(|c| c.in_menu)
            })
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
            self.lo
                .ids_of_type(b"SNCT")
                .iter()
                .copied()
                .find(|&id| {
                    self.lo.get(id).is_some_and(|r| {
                        r.editor_id().is_some_and(|edid| {
                            edid.eq_ignore_ascii_case(name)
                                || alias_for(&edid).eq_ignore_ascii_case(name)
                        })
                    })
                })
                .or_else(|| self.resolve_form(name))
        };
        if args.is_empty() {
            let mut lines = vec![format!("master = {}", self.audio_settings.master)];
            for &id in self.lo.ids_of_type(b"SNCT") {
                if let Some(c) = crate::world::sound::category(&self.lo, id).filter(|c| c.in_menu) {
                    lines.push(format!(
                        "{id} {} = {}",
                        self.lo
                            .get(id)
                            .and_then(|r| r.editor_id())
                            .unwrap_or_default(),
                        self.audio_settings
                            .categories
                            .get(&id.0)
                            .copied()
                            .unwrap_or(c.menu)
                    ));
                }
            }
            return lines;
        }
        let category = if args[0].eq_ignore_ascii_case("master") {
            None
        } else {
            match id(args[0]).filter(|&id| {
                crate::world::sound::category(&self.lo, id).is_some_and(|c| c.in_menu)
            }) {
                Some(id) => Some(id),
                None => return vec!["unknown Audio menu category".into()],
            }
        };
        if args.len() == 1 {
            let value = category.map_or(self.audio_settings.master, |id| {
                self.audio_settings
                    .categories
                    .get(&id.0)
                    .copied()
                    .unwrap_or_else(|| crate::world::sound::category(&self.lo, id).unwrap().menu)
            });
            return vec![format!("{} = {value}", args[0])];
        }
        if args.len() != 2 {
            return vec!["usage: audio [master|category [value]]".into()];
        }
        let Some(value) = args[1].parse::<f32>().ok().filter(|v| valid(*v)) else {
            return vec!["volume must be a finite number from 0 to 1".into()];
        };
        match self.set_audio_volume(category, value) {
            Ok(()) => vec![format!("{} = {value}", args[0])],
            Err(e) => vec![format!("audio settings: {e}")],
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
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
        e.set_audio_volume(Some(FormId(0x800)), 0.2).unwrap();
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
