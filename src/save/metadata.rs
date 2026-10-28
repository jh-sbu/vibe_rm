//! Independently readable journal metadata. Optional for pre-journal saves.
use super::codec::{Corrupt, Reader, Result, Writer};
use crate::engine::{Engine, PLAYER_REF};
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Metadata {
    pub character: u64,
    pub file_num: u32,
    pub name: String,
    pub race: String,
    pub level: u16,
    pub location: String,
    pub game_date: String,
    pub play_seconds: u64,
    pub written: u64,
    pub plugins_hash: u64,
}
/// Stable, order-sensitive FNV-1a hash, with separators between plugin names.
pub fn plugin_hash(plugins: &[String]) -> u64 {
    plugins
        .iter()
        .flat_map(|p| p.bytes().chain([0]))
        .fold(0xcbf29ce484222325, |h, b| {
            (h ^ b as u64).wrapping_mul(0x100000001b3)
        })
}
pub fn new_character_id() -> u64 {
    static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    now.rotate_left(13)
        ^ ((std::process::id() as u64) << 32)
        ^ SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}
impl Metadata {
    pub fn display_text(&self, name: &str) -> String {
        if name.starts_with("quicksave") {
            "[Quicksave]".into()
        } else if name.starts_with("autosave") {
            "[Autosave]".into()
        } else {
            format!("{} - {} - {}", self.name, self.location, self.game_date)
        }
    }

    pub fn capture(e: &Engine) -> Self {
        let plugins: Vec<_> =
            e.lo.plugins()
                .iter()
                .map(|p| p.plugin.name().to_string())
                .collect();
        Self {
            character: e.scripts.character_id,
            name: e
                .actor_name(PLAYER_REF)
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "Prisoner".into()),
            race: e
                .npc_race(esp::FormId(7))
                .map(|r| e.form_name(r))
                .unwrap_or_else(|| "?".into()),
            level: e.skills.level + e.skills.pending as u16,
            location: e.location_name(),
            game_date: e.date_text(),
            play_seconds: e.scripts.real_time as u64,
            written: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            plugins_hash: plugin_hash(&plugins),
            ..Default::default()
        }
    }
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.u32(1);
        w.u64(self.character);
        w.u32(self.file_num);
        w.str(&self.name);
        w.str(&self.race);
        w.u16(self.level);
        w.str(&self.location);
        w.str(&self.game_date);
        w.u64(self.play_seconds);
        w.u64(self.written);
        w.u64(self.plugins_hash);
        w.0
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let mut r = Reader::new(bytes);
        if r.u32()? != 1 {
            return Err(Corrupt("unknown META version".into()));
        }
        let result = Self {
            character: r.u64()?,
            file_num: r.u32()?,
            name: r.str()?,
            race: r.str()?,
            level: r.u16()?,
            location: r.str()?,
            game_date: r.str()?,
            play_seconds: r.u64()?,
            written: r.u64()?,
            plugins_hash: r.u64()?,
        };
        r.finish()?;
        Ok(result)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metadata_round_trip_and_version() {
        let m = Metadata {
            character: 77,
            file_num: 7,
            name: "Prisoner".into(),
            race: "Nord".into(),
            level: 9,
            location: "Riverwood".into(),
            game_date: "Sundas".into(),
            play_seconds: 123,
            written: 456,
            plugins_hash: plugin_hash(&["Skyrim.esm".into()]),
        };
        assert_eq!(Metadata::decode(&m.encode()).unwrap(), m);
        let mut b = m.encode();
        b[0] = 2;
        assert!(Metadata::decode(&b).is_err());
    }
}
#[cfg(test)]
mod listing_tests {
    use super::*;
    use crate::save::{file::SaveFile, save_list_in};
    #[test]
    fn header_listing_supports_legacy_damage_and_plugin_changes() {
        let dir = std::env::temp_dir().join(format!("vrm-meta-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let plugins = vec!["Skyrim.esm".to_string()];
        let mut old = SaveFile::default();
        let mut w = Writer::default();
        w.len(1);
        w.str(&plugins[0]);
        w.bool(false);
        old.add(b"HEAD", w.0);
        std::fs::write(dir.join("legacy.vrs"), old.to_bytes()).unwrap();
        let meta = Metadata {
            plugins_hash: plugin_hash(&plugins),
            ..Default::default()
        };
        let mut f = SaveFile::default();
        f.add(b"META", meta.encode());
        f.add(b"BODY", vec![0; 200]);
        let bytes = f.to_bytes();
        // The prefix reader does not even require the later snapshot bytes.
        assert!(SaveFile::read_prefix(&bytes[..28 + meta.encode().len()]).is_ok());
        std::fs::write(dir.join("new.vrs"), &bytes).unwrap();
        let mut bad = bytes;
        bad[30] ^= 1;
        std::fs::write(dir.join("bad.vrs"), bad).unwrap();
        std::fs::write(dir.join("Save 7 - old.vrs"), b"damaged").unwrap();
        assert_eq!(crate::save::next_file_num_in(&dir).unwrap(), 8);
        let rows = save_list_in(&dir, &plugins);
        assert_eq!(rows.len(), 4);
        assert!(rows.iter().find(|s| s.name == "bad").unwrap().corrupt);
        assert!(!rows.iter().find(|s| s.name == "legacy").unwrap().obsolete);
        assert!(
            save_list_in(&dir, &[])
                .iter()
                .filter(|s| !s.corrupt)
                .all(|s| s.obsolete)
        );
        assert!(crate::save::delete_save_in(&dir, "../outside").is_err());
        crate::save::delete_save_in(&dir, "legacy").unwrap();
        assert!(!dir.join("legacy.vrs").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
