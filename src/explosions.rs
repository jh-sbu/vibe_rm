//! Explosions (`EXPL`): what one is, as its record gives it.
//!
//! `DATA` (xEdit's `wbDefinitionsTES5.pas`, UESP's EXPL page, CommonLibSSE's
//! `BGSExplosionData`): light, sound 1, sound 2, impact data set, placed
//! object, spawn projectile (forms), force, damage, radius, image space
//! radius, vertical offset mult (floats), flags, sound level. 273 vanilla
//! records have all 52 bytes; one has 48, without the vertical offset mult
//! (`research/explosions.md`); six shorter ones are of a layout nobody has
//! described, read for their model and enchantment only.
//! `known_ambiguities/explosions.md` holds the choices.

use esp::{FormId, LoadOrder};

/// `DATA` flags.
pub mod flag {
    pub const WORLD_ORIENTATION: u32 = 0x2;
    pub const KNOCK_DOWN_ALWAYS: u32 = 0x4;
    pub const KNOCK_DOWN_BY_FORMULA: u32 = 0x8;
    pub const IGNORE_LOS: u32 = 0x10;
    pub const PUSH_SOURCE_ONLY: u32 = 0x20;
    pub const IGNORE_IMAGE_SPACE_SWAP: u32 = 0x40;
    pub const CHAIN: u32 = 0x80;
    pub const NO_VIBRATION: u32 = 0x100;
}

/// An `EXPL` record as read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExplosionData {
    pub id: FormId,
    /// Its model (`MODL`), played as an effect where it goes off.
    pub model: Option<String>,
    /// Applied to what it reaches (`EITM`).
    pub enchantment: FormId,
    /// For the player within `image_space_radius` (`MNAM`).
    pub image_space: FormId,
    pub light: FormId,
    pub sounds: [FormId; 2],
    pub impacts: FormId,
    /// Placed where it goes off (a hazard, an activator, an actor...).
    pub placed: FormId,
    pub spawn_projectile: FormId,
    pub force: f32,
    pub damage: f32,
    /// Reach in game units.
    pub radius: f32,
    pub image_space_radius: f32,
    pub vertical_offset_mult: f32,
    pub flags: u32,
    /// The noise it makes (`SoundLevel`: 0 loud, 1 normal, 2 silent, 3 very loud).
    pub sound_level: u32,
}

impl ExplosionData {
    pub fn load(lo: &LoadOrder, id: FormId) -> Option<ExplosionData> {
        let rec = lo.get(id).filter(|r| r.tag().0 == *b"EXPL")?;
        let form_field = |tag: &[u8; 4]| {
            rec.get(tag)
                .filter(|d| d.len() >= 4)
                .map(|d| u32::from_le_bytes(d[0..4].try_into().unwrap()))
                .filter(|&v| v != 0)
                .map_or(FormId::NULL, |v| rec.fid(FormId(v)))
        };
        let mut x = ExplosionData {
            id,
            model: crate::world::records::model_path(&rec),
            enchantment: form_field(b"EITM"),
            image_space: form_field(b"MNAM"),
            ..Default::default()
        };
        let d = rec.get(b"DATA").unwrap_or_default();
        // The layouts whose order is known: 52 bytes, and 48 without the
        // vertical offset mult.
        if d.len() < 48 {
            return Some(x);
        }
        let u = |o: usize| u32::from_le_bytes(d[o..o + 4].try_into().unwrap());
        let f = |o: usize| f32::from_bits(u(o));
        let form = |o: usize| match u(o) {
            0 => FormId::NULL,
            v => rec.fid(FormId(v)),
        };
        x.light = form(0);
        x.sounds = [form(4), form(8)];
        x.impacts = form(12);
        x.placed = form(16);
        x.spawn_projectile = form(20);
        x.force = f(24);
        x.damage = f(28);
        x.radius = f(32).max(0.0);
        x.image_space_radius = f(36).max(0.0);
        let rest = if d.len() >= 52 {
            x.vertical_offset_mult = f(40);
            44
        } else {
            40
        };
        x.flags = u(rest);
        x.sound_level = u(rest + 4);
        Some(x)
    }

    pub fn has(&self, flag: u32) -> bool {
        self.flags & flag != 0
    }
}

#[cfg(test)]
mod record_tests {
    use super::*;
    use esp::writer::{Node, RecordBuilder, write_plugin};

    fn data(len: usize) -> Vec<u8> {
        let mut d = Vec::new();
        for form in [0x901u32, 0x902, 0x903, 0x904, 0x905, 0x906] {
            d.extend_from_slice(&form.to_le_bytes());
        }
        for v in [240.0f32, 5.0, 500.0, 1024.0] {
            d.extend_from_slice(&v.to_le_bytes());
        }
        if len == 52 {
            d.extend_from_slice(&0.25f32.to_le_bytes());
        }
        d.extend_from_slice(&0x141u32.to_le_bytes());
        d.extend_from_slice(&2u32.to_le_bytes());
        d.truncate(len);
        d
    }

    fn load(records: Vec<RecordBuilder>) -> (std::path::PathBuf, LoadOrder) {
        let dir = std::env::temp_dir().join(format!(
            "vrm-expl-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let nodes = [Node::top(b"EXPL", records)];
        std::fs::write(
            dir.join("Expl.esp"),
            write_plugin(0, &[], "test", "explosions", 0x910, &nodes),
        )
        .unwrap();
        let lo = LoadOrder::load(&dir, &["Expl.esp".into()]).unwrap();
        (dir, lo)
    }

    #[test]
    fn reads_the_full_and_the_short_layouts() {
        let (dir, lo) = load(vec![
            RecordBuilder::new(b"EXPL", FormId(0x800))
                .zstring(b"MODL", "Effects\\FXGasTrapBlast.nif")
                .u32(b"EITM", 0x907)
                .u32(b"MNAM", 0x908)
                .field(b"DATA", &data(52)),
            RecordBuilder::new(b"EXPL", FormId(0x801)).field(b"DATA", &data(48)),
            RecordBuilder::new(b"EXPL", FormId(0x802))
                .u32(b"EITM", 0x907)
                .field(b"DATA", &data(44)),
        ]);
        let full = ExplosionData::load(&lo, FormId(0x800)).unwrap();
        assert_eq!(
            full.model.as_deref(),
            Some("meshes/effects/fxgastrapblast.nif")
        );
        assert_eq!(full.enchantment, FormId(0x907));
        assert_eq!(full.image_space, FormId(0x908));
        assert_eq!(full.light, FormId(0x901));
        assert_eq!(full.sounds, [FormId(0x902), FormId(0x903)]);
        assert_eq!(full.impacts, FormId(0x904));
        assert_eq!(full.placed, FormId(0x905));
        assert_eq!(full.spawn_projectile, FormId(0x906));
        assert_eq!(
            (
                full.force,
                full.damage,
                full.radius,
                full.image_space_radius
            ),
            (240.0, 5.0, 500.0, 1024.0)
        );
        assert_eq!(full.vertical_offset_mult, 0.25);
        assert_eq!((full.flags, full.sound_level), (0x141, 2));
        assert!(full.has(flag::IGNORE_IMAGE_SPACE_SWAP) && !full.has(flag::IGNORE_LOS));

        // TrapPoisonGasExplosion01's 48 bytes: no vertical offset mult.
        let short = ExplosionData::load(&lo, FormId(0x801)).unwrap();
        assert_eq!(short.vertical_offset_mult, 0.0);
        assert_eq!((short.flags, short.sound_level), (0x141, 2));
        assert_eq!(short.radius, 500.0);

        // An unknown layout keeps only what lies outside DATA.
        let old = ExplosionData::load(&lo, FormId(0x802)).unwrap();
        assert_eq!(old.enchantment, FormId(0x907));
        assert_eq!((old.radius, old.damage, old.flags), (0.0, 0.0, 0));
        assert!(ExplosionData::load(&lo, FormId(0x803)).is_none());
        let _ = std::fs::remove_dir_all(dir);
    }
}
