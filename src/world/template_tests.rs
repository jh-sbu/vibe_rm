//! Synthetic records only; no game assets are required or retained.

use super::*;
use esp::writer::{Node, RecordBuilder, write_plugin};

struct Fixture(std::path::PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "vrm-spell-templates-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&dir).unwrap();
        Self(dir)
    }

    fn load(&self, npcs: Vec<RecordBuilder>, extra: Vec<Node>) -> LoadOrder {
        let mut groups = vec![Node::top(b"NPC_", npcs)];
        groups.extend(extra);
        std::fs::write(
            self.0.join("Empty.esp"),
            write_plugin(0, &[], "test", "synthetic", 0x800, &[]),
        )
        .unwrap();
        std::fs::write(
            self.0.join("Spells.esp"),
            write_plugin(0, &[], "test", "synthetic", 0xE00, &groups),
        )
        .unwrap();
        LoadOrder::load(&self.0, &["Empty.esp".into(), "Spells.esp".into()]).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn id(local: u32) -> FormId {
    FormId(0x0100_0000 | local)
}

fn npc(local: u32, flags: u16, template: Option<u32>) -> RecordBuilder {
    let mut acbs = [0; 24];
    acbs[18..20].copy_from_slice(&flags.to_le_bytes());
    let rec = RecordBuilder::new(b"NPC_", FormId(local)).field(b"ACBS", &acbs);
    match template {
        Some(t) => rec.form(b"TPLT", FormId(t)),
        None => rec,
    }
}

fn perk(rec: RecordBuilder, local: u32, rank: u8) -> RecordBuilder {
    let mut data = local.to_le_bytes().to_vec();
    data.extend_from_slice(&[rank, 0, 0, 0]);
    rec.field(b"PRKR", &data)
}

#[test]
fn spell_templates_do_not_fill_unflagged_empty_lists() {
    let fixture = Fixture::new();
    let lo = fixture.load(
        vec![
            npc(0x800, 0, Some(0x801)),
            perk(npc(0x801, 0, None).form(b"SPLO", FormId(0xA00)), 0xB00, 2),
            perk(
                npc(0x802, 0, Some(0x801)).form(b"SPLO", FormId(0xA01)),
                0xB01,
                1,
            ),
        ],
        vec![],
    );
    let empty = Sources::of_npc(&lo, id(0x800), 0, &Default::default());
    assert!(empty.spells(&lo).is_empty());
    assert!(empty.perks(&lo).is_empty());
    let own = Sources::of_npc(&lo, id(0x802), 0, &Default::default());
    assert_eq!(own.spells(&lo), [id(0xA01)]);
    assert_eq!(own.perks(&lo), [(id(0xB01), 1)]);
}

#[test]
fn spell_templates_replace_spells_and_perks_as_one_part() {
    let fixture = Fixture::new();
    let lo = fixture.load(
        vec![
            perk(
                npc(0x800, SPELLS, Some(0x801)).form(b"SPLO", FormId(0xA01)),
                0xB01,
                1,
            ),
            perk(
                npc(0x801, 0, None)
                    .form(b"SPLO", FormId(0xA00))
                    .form(b"SPLO", FormId(0xA02)),
                0xB00,
                2,
            ),
        ],
        vec![],
    );
    let sources = Sources::of_npc(&lo, id(0x800), 0, &Default::default());
    assert_eq!(sources.of(SPELLS), id(0x801));
    assert_eq!(sources.spells(&lo), [id(0xA00), id(0xA02)]);
    assert_eq!(sources.perks(&lo), [(id(0xB00), 2)]);
}

#[test]
fn spell_templates_stop_at_empty_selected_source_and_keep_own_race() {
    let fixture = Fixture::new();
    let lo = fixture.load(
        vec![
            npc(0x800, SPELLS, Some(0x801)).form(b"RNAM", FormId(0xC00)),
            npc(0x801, 0, Some(0x802)).form(b"RNAM", FormId(0xC01)),
            perk(npc(0x802, 0, None).form(b"SPLO", FormId(0xA00)), 0xB00, 1),
        ],
        vec![Node::top(
            b"RACE",
            vec![
                RecordBuilder::new(b"RACE", FormId(0xC00)).form(b"SPLO", FormId(0xA10)),
                RecordBuilder::new(b"RACE", FormId(0xC01)).form(b"SPLO", FormId(0xA11)),
            ],
        )],
    );
    let sources = Sources::of_npc(&lo, id(0x800), 0, &Default::default());
    assert_eq!(sources.chain(), [id(0x800), id(0x801), id(0x802)]);
    assert_eq!(sources.of(SPELLS), id(0x801));
    assert_eq!(sources.spells(&lo), [id(0xA10)]);
    assert!(sources.perks(&lo).is_empty());
}

#[test]
fn spell_templates_follow_nested_flags_and_independent_traits_race() {
    let fixture = Fixture::new();
    let lo = fixture.load(
        vec![
            npc(0x800, SPELLS | TRAITS, Some(0x801)).form(b"RNAM", FormId(0xC00)),
            npc(0x801, SPELLS, Some(0x802)).form(b"RNAM", FormId(0xC01)),
            perk(
                npc(0x802, 0, None)
                    .form(b"RNAM", FormId(0xC00))
                    .form(b"SPLO", FormId(0xA00)),
                0xB00,
                1,
            ),
        ],
        vec![Node::top(
            b"RACE",
            vec![
                RecordBuilder::new(b"RACE", FormId(0xC00)).form(b"SPLO", FormId(0xA10)),
                RecordBuilder::new(b"RACE", FormId(0xC01)).form(b"SPLO", FormId(0xA11)),
            ],
        )],
    );
    let sources = Sources::of_npc(&lo, id(0x800), 0, &Default::default());
    assert_eq!(sources.of(SPELLS), id(0x802));
    assert_eq!(sources.of(TRAITS), id(0x801));
    assert_eq!(sources.spells(&lo), [id(0xA00), id(0xA11)]);
    assert_eq!(sources.perks(&lo), [(id(0xB00), 1)]);
}

#[test]
fn spell_templates_use_the_same_seeded_leveled_pick_for_all_parts() {
    let fixture = Fixture::new();
    let entry = |local: u32| {
        let mut data = [0; 12];
        data[0..2].copy_from_slice(&1u16.to_le_bytes());
        data[4..8].copy_from_slice(&local.to_le_bytes());
        data[8..10].copy_from_slice(&1u16.to_le_bytes());
        data
    };
    let lo = fixture.load(
        vec![
            npc(0x800, SPELLS | TRAITS, Some(0xD00)),
            perk(npc(0x801, 0, None).form(b"SPLO", FormId(0xA01)), 0xB01, 1),
            perk(npc(0x802, 0, None).form(b"SPLO", FormId(0xA02)), 0xB02, 1),
        ],
        vec![Node::top(
            b"LVLN",
            vec![
                RecordBuilder::new(b"LVLN", FormId(0xD00))
                    .field(b"LVLD", &[0])
                    .field(b"LVLO", &entry(0x801))
                    .field(b"LVLO", &entry(0x802)),
            ],
        )],
    );
    let mut picked = std::collections::HashSet::new();
    for seed in 0..64 {
        let sources = Sources::for_ref(&lo, id(0x800), seed, &Default::default()).unwrap();
        assert_eq!(
            sources,
            Sources::for_ref(&lo, id(0x800), seed, &Default::default()).unwrap()
        );
        let selected = sources.of(SPELLS);
        assert_eq!(selected, sources.of(TRAITS));
        let leveled_base = Sources::for_ref(&lo, id(0xD00), seed, &Default::default()).unwrap();
        assert_eq!(leveled_base.of(SPELLS), selected);
        assert_eq!(leveled_base.spells(&lo), sources.spells(&lo));
        let suffix = selected.0 & 0xFF;
        assert_eq!(sources.spells(&lo), [id(0xA00 + suffix)]);
        assert_eq!(sources.perks(&lo), [(id(0xB00 + suffix), 1)]);
        picked.insert(selected);
    }
    assert_eq!(picked.len(), 2, "exercise both leveled template choices");
}

#[test]
fn spell_templates_remap_source_references_and_ignore_invalid_entries() {
    let fixture = Fixture::new();
    let lo = fixture.load(
        vec![perk(
            perk(
                npc(0x800, 0, None)
                    .form(b"SPLO", FormId(0))
                    .field(b"SPLO", &[1, 2, 3])
                    .form(b"SPLO", FormId(0xA00))
                    .field(b"PRKR", &[1, 2, 3, 4]),
                0,
                1,
            ),
            0xB00,
            0,
        )],
        vec![],
    );
    let sources = Sources::of_npc(&lo, id(0x800), 0, &Default::default());
    assert_eq!(sources.spells(&lo), [id(0xA00)]);
    assert_eq!(sources.perks(&lo), [(id(0xB00), 1)]);
}

#[test]
fn spell_templates_resolve_ids_in_the_selected_sources_plugin() {
    let fixture = Fixture::new();
    let _ = fixture.load(
        vec![perk(
            npc(0x800, 0, None)
                .form(b"RNAM", FormId(0xC00))
                .form(b"SPLO", FormId(0xA00)),
            0xB00,
            2,
        )],
        vec![Node::top(
            b"RACE",
            vec![RecordBuilder::new(b"RACE", FormId(0xC00)).form(b"SPLO", FormId(0xA10))],
        )],
    );
    // In Child.esp, local master 0 is Spells.esp (global slot 1), and local
    // slot 1 is Child.esp itself (global slot 2). The template's forms must
    // still be read in Spells.esp's namespace, rather than the child's.
    let child = npc(0x0100_0800, SPELLS | TRAITS, Some(0x800)).form(b"SPLO", FormId(0x0100_0A01));
    std::fs::write(
        fixture.0.join("Child.esp"),
        write_plugin(
            0,
            &["Spells.esp"],
            "test",
            "synthetic",
            0xA02,
            &[Node::top(b"NPC_", vec![child])],
        ),
    )
    .unwrap();
    let lo = LoadOrder::load(
        &fixture.0,
        &["Empty.esp".into(), "Spells.esp".into(), "Child.esp".into()],
    )
    .unwrap();
    let sources = Sources::of_npc(&lo, FormId(0x0200_0800), 0, &Default::default());
    assert_eq!(sources.of(SPELLS), id(0x800));
    assert_eq!(sources.spells(&lo), [id(0xA00), id(0xA10)]);
    assert_eq!(sources.perks(&lo), [(id(0xB00), 2)]);
}

fn lvsp(local: u32, none: u8, flags: u8, entries: &[(u16, u32)]) -> RecordBuilder {
    entries.iter().fold(
        RecordBuilder::new(b"LVSP", FormId(local))
            .field(b"LVLD", &[none])
            .field(b"LVLF", &[flags]),
        |rec, &(level, local)| {
            let mut data = [0; 12];
            data[0..2].copy_from_slice(&level.to_le_bytes());
            data[4..8].copy_from_slice(&local.to_le_bytes());
            data[8..10].copy_from_slice(&1u16.to_le_bytes());
            rec.field(b"LVLO", &data)
        },
    )
}

#[test]
fn leveled_spell_lists_pick_one_entry_per_actor_or_give_all() {
    use crate::world::actor::resolve_spells;
    let fixture = Fixture::new();
    let lo = fixture.load(
        vec![],
        vec![Node::top(
            b"LVSP",
            vec![
                lvsp(0xC00, 0, 0x03, &[(1, 0xA00), (1, 0xA01), (1, 0xA02)]),
                lvsp(0xC01, 0, 0x04, &[(1, 0xA00), (1, 0xC02)]),
                lvsp(0xC02, 0, 0x00, &[(1, 0xA03)]),
                lvsp(0xC03, 100, 0x04, &[(1, 0xA00)]),
            ],
        )],
    );
    let globals = Default::default();
    let mut picked = std::collections::HashSet::new();
    for seed in 0..64 {
        let spells = resolve_spells(&lo, id(0xC00), 1, seed, &globals, 0);
        assert_eq!(spells.len(), 1);
        assert_eq!(spells, resolve_spells(&lo, id(0xC00), 1, seed, &globals, 0));
        picked.insert(spells[0]);
    }
    assert_eq!(picked.len(), 3, "every entry is picked by some actor");
    // "Use all", through a nested list; a plain spell passes through.
    assert_eq!(
        resolve_spells(&lo, id(0xC01), 1, 0, &globals, 0),
        [id(0xA00), id(0xA03)]
    );
    assert_eq!(
        resolve_spells(&lo, id(0xA05), 1, 0, &globals, 0),
        [id(0xA05)]
    );
    assert!(resolve_spells(&lo, id(0xC03), 1, 0, &globals, 0).is_empty());
}

#[test]
fn leveled_spell_lists_use_the_level_given() {
    use crate::world::actor::resolve_spells;
    let fixture = Fixture::new();
    let lo = fixture.load(
        vec![],
        vec![Node::top(
            b"LVSP",
            vec![
                lvsp(0xC00, 0, 0x00, &[(1, 0xA00), (10, 0xA01), (20, 0xA02)]),
                lvsp(0xC01, 0, 0x05, &[(1, 0xA00), (10, 0xA01), (20, 0xA02)]),
            ],
        )],
    );
    let globals = Default::default();
    let at = |list, level| resolve_spells(&lo, id(list), level, 0, &globals, 0);
    assert_eq!(at(0xC00, 1), [id(0xA00)]);
    assert_eq!(at(0xC00, 15), [id(0xA01)]);
    assert_eq!(at(0xC00, 40), [id(0xA02)]);
    assert_eq!(at(0xC01, 15), [id(0xA00), id(0xA01)]);
}
