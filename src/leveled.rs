//! Entries scripts add to leveled lists (`LeveledItem` / `LeveledActor` /
//! `LeveledSpell` `.AddForm`), after the plugin's (`LVLO`) in the order added;
//! `Revert` drops them. The resolvers in `world/actor.rs` read them through
//! `Globals`. Saved: `LVAD`. Choices: `known_ambiguities/remaining-natives.md`.

use std::collections::HashMap;

use esp::FormId;

use crate::engine::Engine;

/// `TESLeveledList::numEntries` is a `u8`.
pub const MAX_ENTRIES: usize = 255;

/// One entry a script added: level, form, count.
pub type Entry = (u16, FormId, u16);

#[derive(Default, Clone)]
pub struct Additions {
    pub(crate) added: HashMap<FormId, Vec<Entry>>,
}

impl Additions {
    pub fn of(&self, list: FormId) -> &[Entry] {
        self.added.get(&list).map_or(&[], |v| v.as_slice())
    }
}

impl Engine {
    /// `AddForm` on a leveled list: appends an entry unless the list is full.
    /// Duplicates are kept, as the editor allows them. Actor and spell lists
    /// take a count of 1.
    pub fn add_to_leveled(&mut self, list: FormId, form: FormId, level: i32, count: i32) {
        let Some(rec) = self.lo.get(list) else { return };
        let is_item = &rec.tag().0 == b"LVLI";
        if !matches!(&rec.tag().0, b"LVLI" | b"LVLN" | b"LVSP") || form.is_null() {
            return;
        }
        let plugin = rec
            .subrecords()
            .filter(|s| s.tag.0 == *b"LVLO" && s.data.len() >= 8)
            .count();
        let added = self.scripts.globals.leveled.added.entry(list).or_default();
        if plugin + added.len() >= MAX_ENTRIES {
            if added.is_empty() {
                self.scripts.globals.leveled.added.remove(&list);
            }
            return;
        }
        let count = if is_item {
            count.clamp(1, 0xFFFF) as u16
        } else {
            1
        };
        added.push((level.clamp(0, 0xFFFF) as u16, form, count));
    }

    /// `Revert`: drops every entry scripts added to a leveled list.
    pub fn revert_leveled(&mut self, list: FormId) {
        self.scripts.globals.leveled.added.remove(&list);
    }

    /// Restore the entries a save kept, in place of this session's.
    pub(crate) fn restore_leveled_additions(&mut self, added: HashMap<FormId, Vec<Entry>>) {
        self.scripts.globals.leveled.added = added;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::actor::{resolve_items, resolve_leveled, resolve_spells};
    use esp::writer::{Node, RecordBuilder, write_plugin};
    use papyrus::{NativeResult, Value};

    const ITEMS: FormId = FormId(0x800);
    const ACTORS: FormId = FormId(0x801);
    const SPELLS: FormId = FormId(0x802);
    const A: FormId = FormId(0x810);
    const B: FormId = FormId(0x811);
    const NPC: FormId = FormId(0x812);
    const SPEL: FormId = FormId(0x813);
    const FULL: FormId = FormId(0x803);

    struct Fixture(std::path::PathBuf);

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn lvlo(level: u16, form: FormId, count: u16) -> Vec<u8> {
        let mut d = Vec::new();
        d.extend(level.to_le_bytes());
        d.extend([0, 0]);
        d.extend(form.0.to_le_bytes());
        d.extend(count.to_le_bytes());
        d.extend([0, 0]);
        d
    }

    fn fixture() -> (Fixture, Engine) {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let f = Fixture(
            std::env::temp_dir().join(format!("vrm-leveled-{}-{stamp}", std::process::id())),
        );
        std::fs::create_dir(&f.0).unwrap();
        let list = |tag: &[u8; 4], id: FormId| {
            RecordBuilder::new(tag, id)
                .field(b"LVLD", &[0])
                .field(b"LVLF", &[0x01])
        };
        let mut full = list(b"LVLI", FULL);
        for _ in 0..MAX_ENTRIES {
            full = full.field(b"LVLO", &lvlo(1, A, 1));
        }
        let records = [
            Node::top(
                b"LVLI",
                vec![list(b"LVLI", ITEMS).field(b"LVLO", &lvlo(1, A, 2)), full],
            ),
            Node::top(b"LVLN", vec![list(b"LVLN", ACTORS)]),
            Node::top(b"LVSP", vec![list(b"LVSP", SPELLS)]),
            Node::top(
                b"MISC",
                vec![
                    RecordBuilder::new(b"MISC", A),
                    RecordBuilder::new(b"MISC", B),
                ],
            ),
            Node::top(b"NPC_", vec![RecordBuilder::new(b"NPC_", NPC)]),
            Node::top(b"SPEL", vec![RecordBuilder::new(b"SPEL", SPEL)]),
        ];
        std::fs::write(
            f.0.join("Leveled.esp"),
            write_plugin(0, &[], "test", "synthetic leveled lists", 0x900, &records),
        )
        .unwrap();
        let e = crate::stats::action_tests::headless_engine(&f.0, "Leveled.esp");
        (f, e)
    }

    fn call(e: &mut Engine, class: &str, func: &str, list: FormId, args: &[Value]) {
        let this = e.object_value(list);
        match crate::script::natives::call(e, class, func, Some(&this), args) {
            NativeResult::Value(_) => {}
            _ => panic!("{class}.{func} didn't return"),
        }
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn rolls_see_script_entries_until_reverted() {
        let (_f, mut e) = fixture();
        let b = e.object_value(B);
        let items = |e: &Engine| resolve_items(&e.lo, ITEMS, 1, 7, &e.scripts.globals, 0);
        // Calculate from all levels, so both entries are eligible at level 1.
        assert_eq!(items(&e), [(A, 2)]);
        call(
            &mut e,
            "leveleditem",
            "addform",
            ITEMS,
            &[b.clone(), Value::Int(1), Value::Int(4)],
        );
        let seen: std::collections::HashSet<_> = (0..64)
            .flat_map(|seed| resolve_items(&e.lo, ITEMS, 1, seed, &e.scripts.globals, 0))
            .collect();
        assert_eq!(seen, [(A, 2), (B, 4)].into_iter().collect());
        // A level above the player's isn't eligible yet.
        call(&mut e, "leveleditem", "revert", ITEMS, &[]);
        call(
            &mut e,
            "leveleditem",
            "addform",
            ITEMS,
            &[b.clone(), Value::Int(99), Value::Int(1)],
        );
        assert_eq!(items(&e), [(A, 2)]);
        call(&mut e, "leveleditem", "revert", ITEMS, &[]);
        assert!(e.scripts.globals.leveled.added.is_empty());
        assert_eq!(items(&e), [(A, 2)]);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn actor_and_spell_lists_take_entries_with_count_one() {
        let (_f, mut e) = fixture();
        let npc = e.object_value(NPC);
        let spel = e.object_value(SPEL);
        assert_eq!(
            resolve_leveled(&e.lo, ACTORS, b"LVLN", 3, &e.scripts.globals),
            None
        );
        call(
            &mut e,
            "leveledactor",
            "addform",
            ACTORS,
            &[npc, Value::Int(1)],
        );
        call(
            &mut e,
            "leveledspell",
            "addform",
            SPELLS,
            &[spel, Value::Int(1)],
        );
        assert_eq!(
            resolve_leveled(&e.lo, ACTORS, b"LVLN", 3, &e.scripts.globals),
            Some(NPC)
        );
        assert_eq!(
            resolve_spells(&e.lo, SPELLS, 1, 3, &e.scripts.globals, 0),
            [SPEL]
        );
        assert_eq!(e.scripts.globals.leveled.of(ACTORS), [(1, NPC, 1)]);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn a_full_list_takes_no_more_and_only_leveled_lists_take_any() {
        let (_f, mut e) = fixture();
        e.add_to_leveled(FULL, B, 1, 1);
        assert!(e.scripts.globals.leveled.added.is_empty());
        e.add_to_leveled(A, B, 1, 1); // Not a leveled list.
        e.add_to_leveled(ITEMS, FormId::NULL, 1, 1);
        assert!(e.scripts.globals.leveled.added.is_empty());
        e.add_to_leveled(ITEMS, B, 1, 1);
        e.add_to_leveled(ITEMS, B, 1, 1); // Duplicates stay.
        assert_eq!(e.scripts.globals.leveled.of(ITEMS).len(), 2);
    }
}
