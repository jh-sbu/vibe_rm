use super::*;
use crate::engine::PLAYER_REF;
use crate::item_copies::{CopyState, ItemCopy};
use esp::writer::{Node, RecordBuilder, write_plugin};
const ITEM: FormId = FormId(0x803);
const STATION: FormId = FormId(0x810);
const MATERIAL: FormId = FormId(0x805);
const ARCANE: FormId = FormId(0x807);

fn fixture() -> (std::path::PathBuf, Engine) {
    let dir = std::env::temp_dir().join(format!(
        "vrm-smithing-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let mut not_enchanted = [0u8; 32];
    not_enchanted[0] = 0x21;
    not_enchanted[4..8].copy_from_slice(&1f32.to_le_bytes());
    not_enchanted[8..10].copy_from_slice(&659u16.to_le_bytes());
    let mut arcane = [0u8; 32];
    arcane[4..8].copy_from_slice(&1f32.to_le_bytes());
    arcane[8..10].copy_from_slice(&448u16.to_le_bytes());
    arcane[12..16].copy_from_slice(&ARCANE.0.to_le_bytes());
    let item_data = [
        10u32.to_le_bytes().as_slice(),
        1f32.to_le_bytes().as_slice(),
        5u16.to_le_bytes().as_slice(),
    ]
    .concat();
    let input = [MATERIAL.0.to_le_bytes(), 1u32.to_le_bytes()].concat();
    let nodes = [
        Node::top(
            b"FURN",
            vec![
                RecordBuilder::new(b"FURN", FormId(0x800))
                    .field(b"WBDT", &[2, 10])
                    .form(b"KWDA", FormId(0x801)),
            ],
        ),
        Node::top(
            b"KYWD",
            vec![
                RecordBuilder::new(b"KYWD", FormId(0x801))
                    .zstring(b"EDID", "CraftingSmithingSharpeningWheel"),
            ],
        ),
        Node::top(
            b"REFR",
            vec![RecordBuilder::new(b"REFR", STATION).form(b"NAME", FormId(0x800))],
        ),
        Node::top(
            b"WEAP",
            vec![
                RecordBuilder::new(b"WEAP", ITEM)
                    .zstring(b"FULL", "Test Sword")
                    .field(b"DATA", &item_data),
            ],
        ),
        Node::top(
            b"MISC",
            vec![
                RecordBuilder::new(b"MISC", MATERIAL)
                    .zstring(b"FULL", "Ingot")
                    .field(b"DATA", &[0; 8]),
            ],
        ),
        Node::top(
            b"PERK",
            vec![RecordBuilder::new(b"PERK", ARCANE).zstring(b"EDID", "ArcaneBlacksmith")],
        ),
        Node::top(
            b"COBJ",
            vec![
                RecordBuilder::new(b"COBJ", FormId(0x811))
                    .field(b"CNTO", &input)
                    .form(b"CNAM", ITEM)
                    .form(b"BNAM", FormId(0x801))
                    .field(b"NAM1", &1u16.to_le_bytes())
                    .field(b"CTDA", &not_enchanted)
                    .field(b"CTDA", &arcane),
            ],
        ),
    ];
    std::fs::write(
        dir.join("Smithing.esp"),
        write_plugin(0, &[], "test", "synthetic smithing", 0x900, &nodes),
    )
    .unwrap();
    let mut e = crate::stats::action_tests::headless_engine(&dir, "Smithing.esp");
    e.inventory_mut(PLAYER_REF);
    (dir, e)
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn crafting_tempering_conditions_inspect_each_copy_enchantment() {
    let (dir, mut e) = fixture();
    e.inventory_mut(PLAYER_REF).add(ITEM, 2);
    e.inventory_mut(PLAYER_REF).add_copy_state(ItemCopy {
        item: ITEM,
        owner: None,
        state: CopyState {
            enchantment: Some(FormId(0xFF000800)),
            ..Default::default()
        },
        count: 1,
    });
    assert_eq!(e.improvements_at(STATION).len(), 1);
    assert!(e.add_perk(PLAYER_REF, ARCANE));
    assert_eq!(e.improvements_at(STATION).len(), 2);
    drop(e);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn crafting_quality_boundaries_perks_and_fortify_use_the_documented_formula() {
    for (skill, level) in [
        (13., 0),
        (14., 1),
        (30., 1),
        (31., 2),
        (64., 2),
        (65., 3),
        (99., 3),
        (100., 4),
        (133., 4),
        (134., 5),
        (167., 5),
        (168., 6),
        (202., 6),
        (203., 7),
    ] {
        assert_eq!(tempering_level(skill), level, "skill {skill}");
    }
    for (skill, level) in [
        (14., 1),
        (21., 1),
        (22., 2),
        (40., 3),
        (57., 4),
        (74., 5),
        (91., 6),
        (108., 7),
    ] {
        assert_eq!(
            tempering_level((skill - 13.29) * 2. + 13.29),
            level,
            "perked {skill}"
        );
    }
    let effective = (100. - 13.29) * 2. * 2.16 * 2.3 + 13.29;
    assert!((effective - 874.84f32).abs() < 0.01);
    assert_eq!(tempering_level(effective), 26);
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn crafting_improvement_updates_one_equipped_copy_and_never_lowers_it() {
    let (dir, mut e) = fixture();
    e.set_actor_value(PLAYER_REF, esp::actor_value::SMITHING, 50.);
    e.open_crafting_station(STATION);
    e.inventory_mut(PLAYER_REF).add(ITEM, 3);
    e.inventory_mut(PLAYER_REF).add(MATERIAL, 2);
    e.equip_in(PLAYER_REF, ITEM, None).unwrap();
    let key = CopyState::default().fingerprint();
    e.improve_item(STATION, ITEM, None, key).unwrap();
    let inv = &e.inventories[&PLAYER_REF];
    assert_eq!(inv.count(ITEM), 3);
    assert_eq!(inv.count(MATERIAL), 1);
    assert_eq!(inv.plain_copies(ITEM, None), 2);
    assert!(inv.is_equipped(ITEM));
    assert_eq!(inv.copy_state(ITEM).level(), 2);
    let state = inv.copy_state(ITEM);
    assert!(
        e.improve_item(STATION, ITEM, None, state.fingerprint())
            .is_err()
    );
    assert_eq!(e.inventories[&PLAYER_REF].count(MATERIAL), 1);
    let rows = e.listed_inventory(PLAYER_REF);
    let modified = rows
        .iter()
        .find(|r| r.item == ITEM && r.copy.level() == 2)
        .unwrap();
    assert!(modified.info.name.ends_with("(Superior)"));
    assert_eq!(modified.info.value, 13);
    drop(e);
    std::fs::remove_dir_all(dir).unwrap();
}
