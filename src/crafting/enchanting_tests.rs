use super::*;
use esp::writer::{Node, RecordBuilder, write_plugin};
const BASE: FormId = FormId(0x820);
const LOOT: FormId = FormId(0x821);
const ARMOR_ENCH: FormId = FormId(0x822);
const SWORD: FormId = FormId(0x830);
const MAGIC_SWORD: FormId = FormId(0x831);
const FORBIDDEN: FormId = FormId(0x832);
const ARMOR: FormId = FormId(0x833);
const GEM: FormId = FormId(0x840);
const STAR: FormId = FormId(0x842);
const STATION: FormId = FormId(0x871);
fn fixture() -> (std::path::PathBuf, Engine) {
    let dir = std::env::temp_dir().join(format!(
        "vrm-enchanting-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let effect = |id: u32, flags: u32, skill: u32, name: &str| {
        let mut data = [0u8; 0x98];
        data[..4].copy_from_slice(&flags.to_le_bytes());
        data[4..8].copy_from_slice(&1f32.to_le_bytes());
        data[0x0c..0x10].copy_from_slice(&skill.to_le_bytes());
        data[0x10..0x14].copy_from_slice(&u32::MAX.to_le_bytes());
        data[0x44..0x48].copy_from_slice(&esp::actor_value::HEALTH.to_le_bytes());
        RecordBuilder::new(b"MGEF", FormId(id))
            .zstring(b"FULL", name)
            .field(b"DATA", &data)
    };
    let enchantment = |id: FormId, base: u32, armor: bool, mag: f32| {
        let mut enit = [0u8; 36];
        enit[..4].copy_from_slice(&11u32.to_le_bytes());
        enit[8..12].copy_from_slice(&if armor { 0u32 } else { 1 }.to_le_bytes());
        enit[12..16].copy_from_slice(&11u32.to_le_bytes());
        enit[16..20].copy_from_slice(&if armor { 0u32 } else { 1 }.to_le_bytes());
        enit[20..24].copy_from_slice(&6u32.to_le_bytes());
        enit[28..32].copy_from_slice(&base.to_le_bytes());
        if armor {
            enit[32..36].copy_from_slice(&0x850u32.to_le_bytes());
        }
        RecordBuilder::new(b"ENCH", id)
            .zstring(
                b"FULL",
                if armor {
                    "Fortify Health"
                } else {
                    "Fire Damage"
                },
            )
            .field(b"ENIT", &enit)
            .form(b"EFID", FormId(if armor { 0x801 } else { 0x800 }))
            .field(
                b"EFIT",
                &[mag.to_le_bytes(), 0u32.to_le_bytes(), 0u32.to_le_bytes()].concat(),
            )
    };
    let data = [
        10u32.to_le_bytes().as_slice(),
        1f32.to_le_bytes().as_slice(),
        5u16.to_le_bytes().as_slice(),
    ]
    .concat();
    let weapon = |id: FormId, name: &str| {
        RecordBuilder::new(b"WEAP", id)
            .zstring(b"FULL", name)
            .field(b"DATA", &data)
    };
    let nodes = [
        Node::top(
            b"MGEF",
            vec![
                effect(
                    0x800,
                    crate::magic::flags::HOSTILE
                        | crate::magic::flags::POWER_AFFECTS_MAGNITUDE
                        | crate::magic::flags::NO_DURATION,
                    20,
                    "Fire Damage",
                ),
                effect(
                    0x801,
                    crate::magic::flags::RECOVER
                        | crate::magic::flags::POWER_AFFECTS_MAGNITUDE
                        | crate::magic::flags::NO_DURATION,
                    u32::MAX,
                    "Fortify Health",
                ),
            ],
        ),
        Node::top(
            b"ENCH",
            vec![
                enchantment(BASE, 0, false, 10.),
                enchantment(LOOT, BASE.0, false, 20.),
                enchantment(ARMOR_ENCH, 0, true, 20.),
            ],
        ),
        Node::top(
            b"WEAP",
            vec![
                weapon(SWORD, "Sword"),
                weapon(MAGIC_SWORD, "Fire Sword")
                    .form(b"EITM", LOOT)
                    .field(b"EAMT", &1000u16.to_le_bytes()),
                weapon(FORBIDDEN, "Artifact")
                    .form(b"EITM", LOOT)
                    .form(b"KWDA", FormId(0x860)),
            ],
        ),
        Node::top(
            b"ARMO",
            vec![
                RecordBuilder::new(b"ARMO", ARMOR)
                    .zstring(b"FULL", "Armor")
                    .field(b"DATA", &data[..8])
                    .form(b"KWDA", FormId(0x861))
                    .field(b"BOD2", &[4u32.to_le_bytes(), 1u32.to_le_bytes()].concat()),
            ],
        ),
        Node::top(
            b"SLGM",
            vec![
                RecordBuilder::new(b"SLGM", GEM)
                    .zstring(b"FULL", "Grand Soul Gem")
                    .field(b"DATA", &data[..8])
                    .field(b"SOUL", &[5])
                    .field(b"SLCP", &[5]),
                RecordBuilder::new(b"SLGM", STAR)
                    .zstring(b"FULL", "Star")
                    .field(b"DATA", &data[..8])
                    .field(b"SOUL", &[0])
                    .field(b"SLCP", &[5])
                    .form(b"KWDA", FormId(0x862)),
            ],
        ),
        Node::top(
            b"FLST",
            vec![RecordBuilder::new(b"FLST", FormId(0x850)).form(b"LNAM", FormId(0x861))],
        ),
        Node::top(
            b"KYWD",
            vec![
                RecordBuilder::new(b"KYWD", FormId(0x860))
                    .zstring(b"EDID", "MagicDisallowEnchanting"),
                RecordBuilder::new(b"KYWD", FormId(0x861)).zstring(b"EDID", "ArmorCuirass"),
                RecordBuilder::new(b"KYWD", FormId(0x862)).zstring(b"EDID", "ReusableSoulGem"),
            ],
        ),
        Node::top(
            b"FURN",
            vec![RecordBuilder::new(b"FURN", FormId(0x870)).field(b"WBDT", &[3, 23])],
        ),
        Node::top(
            b"REFR",
            vec![RecordBuilder::new(b"REFR", STATION).form(b"NAME", FormId(0x870))],
        ),
        Node::top(
            b"PERK",
            vec![
                RecordBuilder::new(b"PERK", FormId(0x880))
                    .field(b"PRKE", &[2, 0, 0])
                    .field(b"DATA", &[80, 1, 1])
                    .field(b"EPFT", &[1])
                    .f32(b"EPFD", 2.)
                    .field(b"PRKF", &[]),
                RecordBuilder::new(b"PERK", FormId(0x881))
                    .field(b"PRKE", &[2, 0, 0])
                    .field(b"DATA", &[77, 3, 3])
                    .field(b"EPFT", &[1])
                    .f32(b"EPFD", 2.)
                    .field(b"PRKF", &[]),
            ],
        ),
    ];
    std::fs::write(
        dir.join("Enchanting.esp"),
        write_plugin(0, &[], "test", "synthetic enchanting", 0x900, &nodes),
    )
    .unwrap();
    let mut e = crate::stats::action_tests::headless_engine(&dir, "Enchanting.esp");
    for (f, n) in [
        (SWORD, 2),
        (MAGIC_SWORD, 2),
        (FORBIDDEN, 1),
        (ARMOR, 1),
        (GEM, 5),
        (STAR, 1),
    ] {
        e.inventory_mut(PLAYER_REF).add(f, n);
    }
    e.inventory_mut(PLAYER_REF).put_soul(STAR, 5);
    e.open_crafting_station(STATION);
    (dir, e)
}
#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn crafting_disenchant_learns_the_base_once_and_refuses_forbidden_items() {
    let (dir, mut e) = fixture();
    assert_eq!(
        e.disenchantable()
            .iter()
            .map(|r| r.item)
            .collect::<Vec<_>>(),
        vec![MAGIC_SWORD]
    );
    let key = e.disenchantable()[0].copy.fingerprint();
    e.disenchant_item(STATION, MAGIC_SWORD, None, key).unwrap();
    assert_eq!(e.inventories[&PLAYER_REF].count(MAGIC_SWORD), 1);
    assert!(e.player_knows(BASE));
    assert!(e.player_knows(LOOT));
    assert!(e.disenchantable().is_empty());
    assert!(e.disenchant_item(STATION, FORBIDDEN, None, key).is_err());
    let snapshot =
        crate::save::state::Snapshot::read(&crate::save::state::Snapshot::capture(&e).write())
            .unwrap();
    assert!(snapshot.enchantments.contains(&BASE));
    drop(e);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn crafting_enchanting_categories_filter_item_types_restrictions_and_souls() {
    let (dir, mut e) = fixture();
    e.knowledge.enchantments.extend([BASE, ARMOR_ENCH]);
    let items = e
        .enchantable_items()
        .iter()
        .map(|r| r.item)
        .collect::<Vec<_>>();
    assert!(items.contains(&SWORD));
    assert!(items.contains(&ARMOR));
    assert!(!items.contains(&MAGIC_SWORD));
    assert!(!items.contains(&FORBIDDEN));
    assert_eq!(e.known_enchantments_for(Some(SWORD)), vec![BASE]);
    assert_eq!(e.known_enchantments_for(Some(ARMOR)), vec![ARMOR_ENCH]);
    assert!(!e.enchantment_compatible(ARMOR_ENCH, SWORD));
    assert!(!e.enchantment_compatible(BASE, ARMOR));
    let souls = e.enchanting_souls();
    assert!(souls.iter().any(|(r, s)| r.item == GEM && *s == 5));
    assert!(souls.iter().any(|(r, s)| r.item == STAR && *s == 5));
    assert_eq!(e.enchantment_slots(), 1);
    e.add_perk(PLAYER_REF, FormId(0x880));
    assert_eq!(e.enchantment_slots(), 2);
    drop(e);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn crafting_enchantment_skill_and_weapon_charge_formulas() {
    assert!((skill_strength(100.) - 1.2529411).abs() < 0.00001);
    assert!((skill_strength(15.) - 1.0004412).abs() < 0.00001);
    let full = weapon_charge_cost(10., 100., 1.);
    let half = weapon_charge_cost(10., 100., 0.5);
    assert!(half < full);
    assert!((half / full - 0.5f32.powf(1.1)).abs() < 0.00001);
    for (soul, charge) in [(1, 250.), (2, 500.), (3, 1000.), (4, 2000.), (5, 3000.)] {
        assert_eq!(crate::recharge::soul_charge(soul), charge);
    }
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn crafting_enchant_transaction_preserves_copies_empties_stars_and_saves() {
    let (dir, mut e) = fixture();
    e.knowledge.enchantments.extend([BASE, ARMOR_ENCH]);
    e.set_actor_value(PLAYER_REF, esp::actor_value::ENCHANTING, 100.);
    let quality = crate::item_copies::CopyState::at_level(3);
    assert!(e.inventory_mut(PLAYER_REF).replace_copy_state(
        SWORD,
        None,
        &Default::default(),
        quality.clone()
    ));
    let row = e
        .enchantable_items()
        .into_iter()
        .find(|r| r.item == SWORD && r.copy == quality)
        .unwrap();
    e.use_listed_item(&row, Some(1));
    let mut request = EnchantRequest {
        station: STATION,
        item: SWORD,
        owner: None,
        copy: quality.clone(),
        effects: vec![BASE],
        gem: GEM,
        soul: 5,
        name: Some("Éclair".into()),
        slider: 1.,
    };
    e.enchant_item(request.clone()).unwrap();
    assert_eq!(e.item_count(PLAYER_REF, SWORD), 2);
    assert_eq!(e.item_count(PLAYER_REF, GEM), 4);
    let state = e.equipped_copy(PLAYER_REF, SWORD);
    assert_eq!(state.quality, quality.quality);
    assert_eq!(state.name.as_deref(), Some("Éclair"));
    assert_eq!(state.charge, 3000.);
    let enchantment = state.enchantment.unwrap();
    assert!(e.pay_weapon_charge(PLAYER_REF, SWORD, enchantment));
    assert!(e.weapon_charge(PLAYER_REF, SWORD) < 3000.);
    assert!(e.enchant_item(request.clone()).is_err()); // Stale confirmation.
    assert_eq!(e.item_count(PLAYER_REF, GEM), 4);

    assert!(e.inventory_mut(PLAYER_REF).spend_soul(STAR, 5, 0, true));
    e.inventory_mut(PLAYER_REF).put_soul(STAR, 1);
    request.copy = Default::default();
    request.name = None;
    request.gem = STAR;
    request.soul = 1;
    e.enchant_item(request).unwrap();
    assert_eq!(e.item_count(PLAYER_REF, STAR), 1);
    assert!(!e.charging_souls(PLAYER_REF).iter().any(|(f, _)| *f == STAR));
    let states: Vec<_> = e.inventories[&PLAYER_REF]
        .copies
        .iter()
        .filter(|c| c.item == SWORD)
        .collect();
    assert_eq!(states.len(), 2);
    assert!(
        states
            .iter()
            .any(|c| c.state.capacity == 250. && c.state.enchantment == Some(enchantment))
    );
    assert_eq!(
        e.equipped_copy(PLAYER_REF, SWORD).name.as_deref(),
        Some("Éclair")
    );

    let armor = e
        .enchantable_items()
        .into_iter()
        .find(|r| r.item == ARMOR)
        .unwrap();
    e.use_listed_item(&armor, None);
    let health = e.av_max(PLAYER_REF, esp::actor_value::HEALTH);
    e.enchant_item(EnchantRequest {
        station: STATION,
        item: ARMOR,
        owner: None,
        copy: armor.copy,
        effects: vec![ARMOR_ENCH],
        gem: GEM,
        soul: 5,
        name: None,
        slider: 1.,
    })
    .unwrap();
    e.update_magic(0.);
    assert_eq!(e.av_max(PLAYER_REF, esp::actor_value::HEALTH), health + 25.);
    let saved =
        crate::save::state::Snapshot::read(&crate::save::state::Snapshot::capture(&e).write())
            .unwrap();
    let expected = e.inventories[&PLAYER_REF].copies.clone();
    drop(e);
    let mut restored = crate::stats::action_tests::headless_engine(&dir, "Enchanting.esp");
    saved.apply(&mut restored).unwrap();
    assert_eq!(restored.inventories[&PLAYER_REF].copies, expected);
    assert!(restored.magic_item(enchantment).is_some());
    assert_eq!(
        restored.equipped_copy(PLAYER_REF, SWORD).name.as_deref(),
        Some("Éclair")
    );
    drop(restored);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn crafting_enchantment_strength_souls_slider_and_perks() {
    let (dir, mut e) = fixture();
    e.knowledge.enchantments.extend([BASE, ARMOR_ENCH]);
    e.set_actor_value(PLAYER_REF, esp::actor_value::ENCHANTING, 100.);
    let sword = e
        .enchantable_items()
        .into_iter()
        .find(|r| r.item == SWORD)
        .unwrap();
    let mut selection = EnchantSelection {
        item: Some(sword),
        effects: vec![BASE],
        gem: Some((GEM, 5)),
        ..Default::default()
    };
    let full = e.enchant_preview(&selection).unwrap();
    assert_eq!(full.effects[0].magnitude, 12.);
    assert_eq!(full.capacity, 3000.);
    selection.slider = 0.5;
    let half = e.enchant_preview(&selection).unwrap();
    assert_eq!(half.effects[0].magnitude, 6.);
    assert!(half.uses > full.uses);
    selection.slider = 1.;
    selection.gem = Some((GEM, 1));
    assert_eq!(
        e.enchant_preview(&selection).unwrap().effects[0].magnitude,
        12.
    );
    selection.item = e.enchantable_items().into_iter().find(|r| r.item == ARMOR);
    selection.effects = vec![ARMOR_ENCH];
    selection.gem = Some((GEM, 5));
    assert_eq!(
        e.enchant_preview(&selection).unwrap().effects[0].magnitude,
        25.
    );
    selection.gem = Some((GEM, 1));
    assert_eq!(
        e.enchant_preview(&selection).unwrap().effects[0].magnitude,
        2.
    );
    e.add_perk(PLAYER_REF, FormId(0x881));
    selection.gem = Some((GEM, 5));
    assert_eq!(
        e.enchant_preview(&selection).unwrap().effects[0].magnitude,
        50.
    );
    drop(e);
    std::fs::remove_dir_all(dir).unwrap();
}
