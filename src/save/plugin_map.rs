//! Where each plugin a save was made with lives in the load order now: form
//! ids are saved as loaded (their top byte, or the light slot under `FE`,
//! names the plugin), so a save made with other plugins, or the same ones
//! in another order, is read through a [`FormMap`] from its slots to the
//! current ones (`plans/saves-remaining.md`, Part B).
//!
//! `PLGS` keeps each plugin's slot as saved. Saves without it have only
//! `HEAD`'s names, in load order; their slots are worked out as the load
//! order assigns them, each plugin's lightness taken from the plugin if it
//! is loaded now, else from an `.esl` extension
//! (`known_ambiguities/saves.md`).
//!
//! A plugin the save needs that isn't loaded (missing from Data, or there
//! but inactive: the user's choice, `deviations/main-menu.md`) is gone: its
//! forms are moved to a full-plugin index the current load order doesn't
//! use, one index for each such plugin, so they stay distinct and resolve
//! to nothing.

use std::collections::HashMap;

use esp::Slot;
use esp::{FormId, LoadOrder};

use super::codec::{Corrupt, Reader, Result, Writer};

/// The name the load order gives the runtime records' plugin (`FF`): it is
/// in every load order and its ids never move.
const RUNTIME: &str = "<runtime>";

/// The plugins a save was made with, in load order, with their slots then.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SavedSlots(pub Vec<(String, Slot)>);

fn is_light_name(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".esl")
}

impl SavedSlots {
    /// The slots of a running load order (the runtime plugin left out).
    pub fn of(lo: &LoadOrder) -> Self {
        SavedSlots(
            lo.plugins()
                .iter()
                .filter(|p| p.plugin.name() != RUNTIME)
                .map(|p| (p.plugin.name().to_string(), p.slot))
                .collect(),
        )
    }

    /// `HEAD`'s names alone (a save without `PLGS`): slots as the load
    /// order assigns them, in order, light as the plugin loaded now is (or
    /// by its `.esl` extension when it isn't loaded).
    pub fn assumed(names: &[String], lo: &LoadOrder) -> Self {
        let (mut full, mut light) = (0u16, 0u16);
        let mut slots = Vec::new();
        for name in names.iter().filter(|n| n.as_str() != RUNTIME) {
            let is_light = lo
                .plugins()
                .iter()
                .find(|p| p.plugin.name().eq_ignore_ascii_case(name))
                .map_or_else(|| is_light_name(name), |p| matches!(p.slot, Slot::Light(_)));
            let slot = if is_light {
                light += 1;
                Slot::Light(light - 1)
            } else {
                full += 1;
                Slot::Full((full - 1).min(0xFE) as u8)
            };
            slots.push((name.clone(), slot));
        }
        SavedSlots(slots)
    }

    /// `PLGS`: a kind byte (0 full, 1 light) and the index for each plugin.
    pub fn write(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.len(self.0.len());
        for (_, slot) in &self.0 {
            match *slot {
                Slot::Full(i) => {
                    w.u8(0);
                    w.u16(i as u16);
                }
                Slot::Light(j) => {
                    w.u8(1);
                    w.u16(j);
                }
            }
        }
        w.0
    }

    /// `PLGS` read against `HEAD`'s names (`names`, the runtime plugin
    /// among them as saved): one slot for each, none used twice.
    pub fn read(data: &[u8], names: &[String]) -> Result<Self> {
        let names: Vec<&String> = names.iter().filter(|n| n.as_str() != RUNTIME).collect();
        let mut r = Reader::new(data);
        let n = r.count(3)?;
        if n != names.len() {
            return Err(Corrupt("plugin slots don't match the plugin list".into()));
        }
        let mut slots = Vec::with_capacity(n);
        for name in names {
            let slot = match (r.u8()?, r.u16()?) {
                (0, i) if i < 0xFE => Slot::Full(i as u8),
                (1, j) if j < 0x1000 => Slot::Light(j),
                _ => return Err(Corrupt("invalid plugin slot".into())),
            };
            if slots.iter().any(|(_, s)| *s == slot) {
                return Err(Corrupt("plugin slot used twice".into()));
            }
            slots.push((name.clone(), slot));
        }
        r.finish()?;
        Ok(SavedSlots(slots))
    }
}

/// What becomes of the forms of one saved slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    To(Slot),
    /// Gone (the plugin isn't loaded): moved to this unused full index.
    Gone(u8),
}

/// A save needs more plugins the load order lacks than there are unused
/// full indices to hold their forms.
#[derive(Debug)]
pub struct TooManyPlugins;

impl std::error::Error for TooManyPlugins {}

impl std::fmt::Display for TooManyPlugins {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the save needs more missing plugins than there are free plugin slots")
    }
}

/// From a save's form ids to the running load order's.
#[derive(Debug, Clone)]
pub struct FormMap {
    full: HashMap<u8, Target>,
    light: HashMap<u16, Target>,
    /// Ids from a slot the save didn't list (none in a sound save).
    unlisted: u8,
    gone: Vec<u8>,
    missing: Vec<String>,
    identity: bool,
}

impl FormMap {
    /// Map `saved` onto `current` (both without the runtime plugin), by
    /// name, ignoring case.
    pub fn new(
        saved: &SavedSlots,
        current: &SavedSlots,
    ) -> std::result::Result<FormMap, TooManyPlugins> {
        let used: Vec<u8> = current
            .0
            .iter()
            .filter_map(|(_, s)| match s {
                Slot::Full(i) => Some(*i),
                Slot::Light(_) => None,
            })
            .collect();
        let mut free = (0u8..0xFE).rev().filter(|i| !used.contains(i));
        let mut map = FormMap {
            full: HashMap::new(),
            light: HashMap::new(),
            unlisted: 0,
            gone: Vec::new(),
            missing: Vec::new(),
            identity: true,
        };
        for (name, slot) in &saved.0 {
            let now = current
                .0
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(name))
                .map(|(_, s)| *s);
            let target = match now {
                Some(s) => Target::To(s),
                None => {
                    let g = free.next().ok_or(TooManyPlugins)?;
                    map.gone.push(g);
                    map.missing.push(name.clone());
                    Target::Gone(g)
                }
            };
            map.identity &= target == Target::To(*slot);
            match *slot {
                Slot::Full(i) => map.full.insert(i, target),
                Slot::Light(j) => map.light.insert(j, target),
            };
        }
        map.unlisted = free.next().ok_or(TooManyPlugins)?;
        map.gone.push(map.unlisted);
        Ok(map)
    }

    /// Whether every saved form keeps its id (the same plugins, or more
    /// loaded after them): the save reads as it was written.
    pub fn is_identity(&self) -> bool {
        self.identity
    }

    /// The plugins the save needs that aren't loaded, as saved.
    pub fn missing(&self) -> &[String] {
        &self.missing
    }

    /// Whether `id` (mapped) belonged to a plugin that isn't loaded.
    pub fn is_gone(&self, id: FormId) -> bool {
        let top = (id.0 >> 24) as u8;
        top != 0xFE && top != 0xFF && !id.is_null() && self.gone.contains(&top)
    }

    /// A runtime base record's bytes (`FORM`: only `ALCH` and `ENCH`, as
    /// `LoadOrder::insert_runtime_record` takes), with the form ids in its
    /// fields mapped. Which fields hold form ids is from the builders
    /// (`crate::crafting`, `crate::created`) and UESP's record pages (*Mod
    /// File Format/ALCH*,
    /// [revision 2308919](https://en.uesp.net/w/index.php?oldid=2308919);
    /// */ENCH*, [revision 2652910](https://en.uesp.net/w/index.php?oldid=2652910)):
    /// `EFID`, `YNAM`, `ZNAM`, `KWDA`; `ALCH`'s `ENIT` addiction (8) and use
    /// sound (16); `ENCH`'s `ENIT` base enchantment (28) and worn
    /// restrictions (32). Its own id (`FF`) doesn't move. Bytes that don't
    /// parse are kept as they are, for `insert_runtime_record` to refuse.
    pub fn map_runtime_record(&self, bytes: &[u8]) -> Vec<u8> {
        const HEADER: usize = 24;
        let mut out = bytes.to_vec();
        if self.identity || out.len() < HEADER {
            return out;
        }
        let tag: [u8; 4] = out[..4].try_into().unwrap();
        let mut at = HEADER;
        while at + 6 <= out.len() {
            let field: [u8; 4] = out[at..at + 4].try_into().unwrap();
            let size = u16::from_le_bytes([out[at + 4], out[at + 5]]) as usize;
            let data = at + 6;
            if data + size > out.len() {
                break;
            }
            let offsets: Vec<usize> = match (&tag, &field) {
                (_, b"EFID" | b"YNAM" | b"ZNAM") => vec![0],
                (_, b"KWDA") => (0..size / 4).map(|i| i * 4).collect(),
                (b"ALCH", b"ENIT") => vec![8, 16],
                (b"ENCH", b"ENIT") => vec![28, 32],
                _ => vec![],
            };
            for o in offsets.into_iter().filter(|o| o + 4 <= size) {
                let p = data + o;
                let id = u32::from_le_bytes(out[p..p + 4].try_into().unwrap());
                out[p..p + 4].copy_from_slice(&self.map(FormId(id)).0.to_le_bytes());
            }
            at = data + size;
        }
        out
    }

    /// The current id of a form the save names as `id`.
    pub fn map(&self, id: FormId) -> FormId {
        if self.identity || id.is_null() {
            return id;
        }
        let top = (id.0 >> 24) as u8;
        let (target, local) = match top {
            0xFF => return id,
            0xFE => (
                self.light.get(&(((id.0 >> 12) & 0xFFF) as u16)).copied(),
                id.0 & 0xFFF,
            ),
            i => (self.full.get(&i).copied(), id.0 & 0x00FF_FFFF),
        };
        let gone = |g: u8| FormId(((g as u32) << 24) | local);
        match target {
            Some(Target::To(Slot::Full(i))) => FormId(((i as u32) << 24) | local),
            // A plugin light now and full when saved: its ids above the
            // light range can't be told apart, so they're gone.
            Some(Target::To(Slot::Light(j))) if local <= 0xFFF => {
                FormId(0xFE00_0000 | ((j as u32) << 12) | local)
            }
            Some(Target::To(Slot::Light(_))) => gone(self.unlisted),
            Some(Target::Gone(g)) => gone(g),
            None => gone(self.unlisted),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slots(list: &[(&str, Slot)]) -> SavedSlots {
        SavedSlots(list.iter().map(|(n, s)| (n.to_string(), *s)).collect())
    }

    #[test]
    fn the_same_plugins_or_more_after_them_map_to_themselves() {
        let saved = slots(&[("Skyrim.esm", Slot::Full(0)), ("A.esp", Slot::Full(1))]);
        let map = FormMap::new(&saved, &saved).unwrap();
        assert!(map.is_identity() && map.missing().is_empty());
        let more = slots(&[
            ("Skyrim.esm", Slot::Full(0)),
            ("a.ESP", Slot::Full(1)),
            ("B.esp", Slot::Full(2)),
            ("C.esl", Slot::Light(0)),
        ]);
        let map = FormMap::new(&saved, &more).unwrap();
        assert!(map.is_identity());
        assert_eq!(map.map(FormId(0x0100_0ABC)), FormId(0x0100_0ABC));
    }

    #[test]
    fn reordered_full_and_light_plugins_move_their_forms() {
        let saved = slots(&[
            ("Skyrim.esm", Slot::Full(0)),
            ("A.esp", Slot::Full(1)),
            ("B.esp", Slot::Full(2)),
            ("L1.esl", Slot::Light(0)),
            ("L2.esl", Slot::Light(1)),
        ]);
        let now = slots(&[
            ("Skyrim.esm", Slot::Full(0)),
            ("B.esp", Slot::Full(1)),
            ("A.esp", Slot::Full(2)),
            ("L2.esl", Slot::Light(0)),
            ("L1.esl", Slot::Light(1)),
        ]);
        let map = FormMap::new(&saved, &now).unwrap();
        assert!(!map.is_identity() && map.missing().is_empty());
        assert_eq!(map.map(FormId(0x0001_2345)), FormId(0x0001_2345));
        assert_eq!(map.map(FormId(0x0100_0800)), FormId(0x0200_0800));
        assert_eq!(map.map(FormId(0x0200_0801)), FormId(0x0100_0801));
        assert_eq!(map.map(FormId(0xFE00_0805)), FormId(0xFE00_1805));
        assert_eq!(map.map(FormId(0xFE00_1806)), FormId(0xFE00_0806));
        // Runtime forms and none stay.
        assert_eq!(map.map(FormId(0xFF00_0900)), FormId(0xFF00_0900));
        assert_eq!(map.map(FormId::NULL), FormId::NULL);
    }

    #[test]
    fn missing_plugins_forms_are_gone_and_kept_apart() {
        let saved = slots(&[
            ("Skyrim.esm", Slot::Full(0)),
            ("Gone.esp", Slot::Full(1)),
            ("A.esp", Slot::Full(2)),
            ("GoneLight.esl", Slot::Light(0)),
        ]);
        let now = slots(&[("Skyrim.esm", Slot::Full(0)), ("A.esp", Slot::Full(1))]);
        let map = FormMap::new(&saved, &now).unwrap();
        assert_eq!(map.missing(), ["Gone.esp", "GoneLight.esl"]);
        assert_eq!(map.map(FormId(0x0200_0800)), FormId(0x0100_0800));
        let full = map.map(FormId(0x0100_0800));
        let light = map.map(FormId(0xFE00_0800));
        let unlisted = map.map(FormId(0x0700_0800));
        for id in [full, light, unlisted] {
            assert!(map.is_gone(id), "{id}");
            assert!(!matches!(id.0 >> 24, 0 | 1 | 0xFE | 0xFF));
        }
        assert!(full != light && light != unlisted && full != unlisted);
        assert!(!map.is_gone(FormId(0x0100_0800)) && !map.is_gone(FormId::NULL));
    }

    #[test]
    fn a_plugin_light_now_keeps_only_ids_that_fit() {
        let saved = slots(&[("Skyrim.esm", Slot::Full(0)), ("A.esp", Slot::Full(1))]);
        let now = slots(&[("Skyrim.esm", Slot::Full(0)), ("A.esp", Slot::Light(0))]);
        let map = FormMap::new(&saved, &now).unwrap();
        assert_eq!(map.map(FormId(0x0100_0801)), FormId(0xFE00_0801));
        assert!(map.is_gone(map.map(FormId(0x0100_1801))));
    }

    #[test]
    fn runtime_records_have_their_form_fields_mapped() {
        use esp::writer::RecordBuilder;
        let saved = slots(&[("Skyrim.esm", Slot::Full(0)), ("A.esp", Slot::Full(1))]);
        let now = slots(&[
            ("Skyrim.esm", Slot::Full(0)),
            ("B.esp", Slot::Full(1)),
            ("A.esp", Slot::Full(2)),
        ]);
        let map = FormMap::new(&saved, &now).unwrap();
        let mut alch_enit = [0u8; 20];
        alch_enit[8..12].copy_from_slice(&0x0100_0901u32.to_le_bytes());
        alch_enit[16..20].copy_from_slice(&0x0000_0902u32.to_le_bytes());
        let alch = RecordBuilder::new(b"ALCH", FormId(0xFF00_0800))
            .zstring(b"FULL", "Potion")
            .form(b"YNAM", FormId(0x0100_0903))
            .field(
                b"KWDA",
                &[0x0100_0904u32.to_le_bytes(), 0x0000_0905u32.to_le_bytes()].concat(),
            )
            .field(b"ENIT", &alch_enit)
            .form(b"EFID", FormId(0x0100_0906))
            .field(b"EFIT", &[0u8; 12])
            .to_bytes();
        let bytes = map.map_runtime_record(&alch);
        let mapped = esp::Record::from_raw(&bytes).unwrap();
        assert_eq!(mapped.form_id(), FormId(0xFF00_0800));
        let u32_at = |d: &[u8], o: usize| u32::from_le_bytes(d[o..o + 4].try_into().unwrap());
        assert_eq!(u32_at(mapped.get(b"YNAM").unwrap(), 0), 0x0200_0903);
        let kwda = mapped.get(b"KWDA").unwrap();
        assert_eq!(
            (u32_at(kwda, 0), u32_at(kwda, 4)),
            (0x0200_0904, 0x0000_0905)
        );
        let enit = mapped.get(b"ENIT").unwrap();
        assert_eq!(
            (u32_at(enit, 8), u32_at(enit, 16)),
            (0x0200_0901, 0x0000_0902)
        );
        assert_eq!(u32_at(mapped.get(b"EFID").unwrap(), 0), 0x0200_0906);
        assert_eq!(mapped.get(b"FULL").unwrap(), b"Potion\0");

        let mut ench_enit = [0u8; 36];
        ench_enit[28..32].copy_from_slice(&0x0100_0907u32.to_le_bytes());
        ench_enit[32..36].copy_from_slice(&0x0100_0908u32.to_le_bytes());
        let ench = RecordBuilder::new(b"ENCH", FormId(0xFF00_0801))
            .field(b"ENIT", &ench_enit)
            .to_bytes();
        let bytes = map.map_runtime_record(&ench);
        let mapped = esp::Record::from_raw(&bytes).unwrap();
        let enit = mapped.get(b"ENIT").unwrap();
        assert_eq!(
            (u32_at(enit, 28), u32_at(enit, 32)),
            (0x0200_0907, 0x0200_0908)
        );
    }

    #[test]
    fn plugin_slots_round_trip_against_the_names_and_refuse_damage() {
        let names: Vec<String> = ["Skyrim.esm", "A.esl", "B.esp", RUNTIME]
            .map(String::from)
            .into();
        let saved = slots(&[
            ("Skyrim.esm", Slot::Full(0)),
            ("A.esl", Slot::Light(0)),
            ("B.esp", Slot::Full(1)),
        ]);
        let bytes = saved.write();
        assert_eq!(SavedSlots::read(&bytes, &names).unwrap(), saved);
        assert!(SavedSlots::read(&bytes, &names[..2]).is_err());
        let twice = slots(&[
            ("Skyrim.esm", Slot::Full(0)),
            ("A.esl", Slot::Full(0)),
            ("B.esp", Slot::Full(1)),
        ]);
        assert!(SavedSlots::read(&twice.write(), &names).is_err());
    }
}
