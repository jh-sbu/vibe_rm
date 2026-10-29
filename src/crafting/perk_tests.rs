//! Exercise the installed vanilla records, including DLC, rather than copies
//! of their entry-point data. No assets are written into the repository.
use super::*;
use crate::engine::PLAYER_REF;

#[test]
#[ignore = "requires installed Skyrim assets and a headless GPU adapter"]
fn crafting_vanilla_perks_and_fortify_effects() {
    let data = vfs::locate_data_dir().unwrap();
    let names = esp::BASE_MASTERS
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let mut e = crate::stats::action_tests::headless_engine_plugins(&data, &names);
    let form = |e: &Engine, name: &str| {
        e.lo.find_editor_id(name)
            .unwrap_or_else(|| panic!("missing {name}"))
    };
    let av = |name| esp::actor_value::index(name).unwrap();
    let steel = form(&e, "SteelSword");
    let iron = form(&e, "IronSword");
    e.set_actor_value(PLAYER_REF, av("Smithing"), 100.);
    assert_eq!(e.attainable_quality(steel).level(), 4);
    let perk = form(&e, "SteelSmithing");
    e.add_perk(PLAYER_REF, perk);
    assert_eq!(e.attainable_quality(steel).level(), 6);
    assert_eq!(e.attainable_quality(iron).level(), 4);
    e.mod_temporary_av(PLAYER_REF, av("SmithingMod"), 50.);
    e.mod_temporary_av(PLAYER_REF, av("SmithingPowerMod"), 50.);
    let expected = tempering_level((100. - 13.29) * 2. * 1.5 * 1.5 + 13.29);
    assert_eq!(e.attainable_quality(steel).level(), expected);

    let boost = form(&e, "DLC2BlackBookSeekerPerks");
    let seeker = form(&e, "DLC2BlackBookReward4");
    e.add_perk(PLAYER_REF, boost);
    e.scripts.globals.insert(seeker, 1.);
    assert_eq!(
        e.attainable_quality(steel).level(),
        tempering_level((100. - 13.29) * 2. * 1.5 * 1.5 * 1.1 + 13.29)
    );

    // Alchemist, Physician and Benefactor see the active potion query.
    e.set_actor_value(PLAYER_REF, av("Alchemy"), 100.);
    let ingredients = [form(&e, "Wheat"), form(&e, "Mushroom06")];
    let baseline = e.potion_result(&ingredients).unwrap().effects[0].magnitude;
    for name in ["Alchemist00", "Physician", "Benefactor"] {
        let perk = form(&e, name);
        e.add_perk(PLAYER_REF, perk);
    }
    let strengthened = e.potion_result(&ingredients).unwrap().effects[0].magnitude;
    assert!(strengthened > baseline * 1.7);
    e.mod_temporary_av(PLAYER_REF, av("AlchemyMod"), 50.);
    assert!(e.potion_result(&ingredients).unwrap().effects[0].magnitude > strengthened * 1.4);
    e.scripts.globals.insert(seeker, 3.);
    let before = e.perk_entry_point(
        crate::perks::ep::MOD_ALCHEMY_EFFECTIVENESS,
        PLAYER_REF,
        &[],
        1.,
    );
    e.scripts.globals.insert(seeker, 0.);
    let after = e.perk_entry_point(
        crate::perks::ep::MOD_ALCHEMY_EFFECTIVENESS,
        PLAYER_REF,
        &[],
        1.,
    );
    assert!((before / after - 1.1).abs() < 0.0001);

    // Sorcery changes power, Soul Squeezer changes recharging, and Extra Effect
    // changes the allowed count through distinct entry points.
    let effect = form(&e, "EnchFireDamageFFContact");
    let power = |e: &Engine| {
        e.perk_entry_point(
            crate::perks::ep::MOD_ENCHANTMENT_POWER,
            PLAYER_REF,
            &[Some(effect), Some(steel)],
            1.,
        )
    };
    let base = power(&e);
    for name in ["Enchanter00", "FireEnchanter"] {
        let perk = form(&e, name);
        e.add_perk(PLAYER_REF, perk);
    }
    assert!((power(&e) / base - 1.5).abs() < 0.0001);
    e.scripts.globals.insert(seeker, 2.);
    assert!((power(&e) / base - 1.65).abs() < 0.0001);
    let perk = form(&e, "SoulSqueezer");
    e.add_perk(PLAYER_REF, perk);
    assert_eq!(
        e.perk_entry_point(
            crate::perks::ep::MOD_SOUL_GEM_ENCHANTING,
            PLAYER_REF,
            &[],
            250.
        ),
        250.
    );
    assert_eq!(
        e.perk_entry_point(
            crate::perks::ep::MOD_SOUL_GEM_RECHARGE,
            PLAYER_REF,
            &[],
            250.
        ),
        500.
    );
    let perk = form(&e, "ExtraEffect");
    e.add_perk(PLAYER_REF, perk);
    assert_eq!(e.enchantment_slots(), 2);
}
