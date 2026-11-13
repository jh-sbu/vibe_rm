//! Scrolls held in a hand and cast, generated records only. Engine owns a
//! renderer, so these need a headless GPU adapter.

use super::*;
use crate::items::ListedItem;
use crate::magic::archetype;
use crate::spells::{LEFT, RIGHT};
use esp::actor_value as av;
use esp::writer::{Node, RecordBuilder, write_plugin};

const SCROLL: FormId = FormId(0x810);
const OTHER: FormId = FormId(0x811);
const TWO_HAND: FormId = FormId(0x813);
const BOTH_SLOT: FormId = FormId(0x820);
const SPELL: FormId = FormId(0x812);
const CONC_EFFECT: FormId = FormId(0x801);
const RELEASE_SCROLL: FormId = FormId(0x814);
const AUTO_SCROLL: FormId = FormId(0x815);
const EFFECT: FormId = FormId(0x800);

struct Fixture(std::path::PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture() -> (Fixture, Engine) {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let f =
        Fixture(std::env::temp_dir().join(format!("vrm-scrolls-{}-{stamp}", std::process::id())));
    std::fs::create_dir(&f.0).unwrap();
    let mut mgef = [0; 0x6c];
    mgef[0x10..0x14].copy_from_slice(&u32::MAX.to_le_bytes());
    mgef[0x40..0x44].copy_from_slice(&archetype::VALUE_MODIFIER.to_le_bytes());
    mgef[0x44..0x48].copy_from_slice(&av::SNEAK.to_le_bytes());
    mgef[0x30..0x34].copy_from_slice(&0.5f32.to_le_bytes());
    mgef[0x50..0x54].copy_from_slice(&1u32.to_le_bytes());
    let mut conc_effect = mgef;
    conc_effect[0x50..0x54].copy_from_slice(&2u32.to_le_bytes());
    let scroll = |id| {
        let mut spit = [0; 36];
        spit[0..4].copy_from_slice(&50u32.to_le_bytes()); // an authored cost
        spit[4..8].copy_from_slice(&u32::from(id != AUTO_SCROLL).to_le_bytes());
        let charge: f32 = if id == AUTO_SCROLL { 0.0 } else { 0.5 };
        spit[12..16].copy_from_slice(&charge.to_le_bytes());
        let cast_type: u32 = if id == OTHER || id == RELEASE_SCROLL {
            2
        } else {
            3
        };
        spit[16..20].copy_from_slice(&cast_type.to_le_bytes());
        let mut efit = [0; 12];
        efit[0..4].copy_from_slice(&7.0f32.to_le_bytes());
        efit[8..12].copy_from_slice(&30u32.to_le_bytes());
        let record = RecordBuilder::new(b"SCRL", id)
            .zstring(b"FULL", "Test Scroll")
            .field(b"DATA", &[0; 8])
            .field(b"SPIT", &spit)
            .u32(b"EFID", if id == OTHER { CONC_EFFECT.0 } else { EFFECT.0 })
            .field(b"EFIT", &efit);
        if id == TWO_HAND {
            record.u32(b"ETYP", BOTH_SLOT.0)
        } else {
            record
        }
    };
    let records = [
        Node::top(
            b"MGEF",
            vec![
                RecordBuilder::new(b"MGEF", EFFECT).field(b"DATA", &mgef),
                RecordBuilder::new(b"MGEF", CONC_EFFECT).field(b"DATA", &conc_effect),
            ],
        ),
        Node::top(
            b"SCRL",
            vec![
                scroll(SCROLL),
                scroll(OTHER),
                scroll(TWO_HAND),
                scroll(RELEASE_SCROLL),
                scroll(AUTO_SCROLL),
            ],
        ),
        Node::top(
            b"EQUP",
            vec![
                RecordBuilder::new(b"EQUP", BOTH_SLOT)
                    .u32(b"DATA", 1) // use all parent slots
                    .field(b"PNAM", &[0x42, 0x3f, 1, 0, 0x43, 0x3f, 1, 0]),
            ],
        ),
        Node::top(
            b"SPEL",
            vec![{
                let mut spit = [0; 36];
                spit[12..16].copy_from_slice(&0.5f32.to_le_bytes());
                spit[16..20].copy_from_slice(&1u32.to_le_bytes());
                RecordBuilder::new(b"SPEL", SPELL).field(b"SPIT", &spit)
            }],
        ),
    ];
    std::fs::write(
        f.0.join("Scrolls.esp"),
        write_plugin(0, &[], "test", "synthetic scrolls", 0x900, &records),
    )
    .unwrap();
    let mut e = crate::stats::action_tests::headless_engine(&f.0, "Scrolls.esp");
    e.set_actor_value(PLAYER_REF, av::MAGICKA, 100.0);
    e.player.grounded = true;
    (f, e)
}

fn row(e: &mut Engine, item: FormId) -> ListedItem {
    e.listed_inventory(PLAYER_REF)
        .into_iter()
        .find(|r| r.item == item)
        .expect("listed")
}

/// Cast the scroll in a hand, holding the button for `held` seconds.
fn cast(e: &mut Engine, hand: usize, held: f32) {
    assert!(e.player_cast_press(hand));
    e.update_casting(held);
    e.player_cast_release(hand);
}

fn effects_of(e: &Engine, spell: FormId) -> usize {
    e.magic
        .effects
        .iter()
        .filter(|x| x.item == spell && !x.done)
        .count()
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn a_scroll_goes_to_the_hand_asked_for_and_comes_out_again() {
    let (_f, mut e) = fixture();
    e.add_item(PLAYER_REF, SCROLL, 2);
    e.add_item(PLAYER_REF, OTHER, 1);
    let r = row(&mut e, SCROLL);
    e.use_listed_item(&r, Some(LEFT));
    assert_eq!(e.equipped_spell(PLAYER_REF, LEFT), Some(SCROLL));
    assert_eq!(e.equipped_spell(PLAYER_REF, RIGHT), None);
    assert!(e.inventories[&PLAYER_REF].is_equipped(SCROLL));
    // Another scroll takes the hand over; the same one again puts it away.
    let other = row(&mut e, OTHER);
    e.use_listed_item(&other, Some(LEFT));
    assert_eq!(e.equipped_spell(PLAYER_REF, LEFT), Some(OTHER));
    assert!(!e.inventories[&PLAYER_REF].is_equipped(SCROLL));
    e.use_listed_item(&other, Some(LEFT));
    assert_eq!(e.equipped_spell(PLAYER_REF, LEFT), None);
    // A scroll isn't a spell to scripts.
    e.use_listed_item(&r, Some(RIGHT));
    let me = e.object_value(PLAYER_REF);
    let got = crate::script::natives::call(
        &mut e,
        "actor",
        "getequippedspell",
        Some(&me),
        &[papyrus::Value::Int(1)],
    );
    assert!(matches!(
        got,
        papyrus::NativeResult::Value(papyrus::Value::None)
    ));
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn equipitem_holds_a_scroll_instead_of_using_it_up() {
    let (_f, mut e) = fixture();
    e.add_item(PLAYER_REF, SCROLL, 2);
    crate::console::execute(&mut e, &format!("player.equipitem {SCROLL} left"));
    assert_eq!(e.equipped_spell(PLAYER_REF, LEFT), Some(SCROLL));
    assert_eq!(e.inventories[&PLAYER_REF].count(SCROLL), 2);
    // Papyrus `EquipItem` too, in the hand its equip slot gives it.
    let me = e.object_value(PLAYER_REF);
    let scroll = e.object_value(SCROLL);
    crate::script::natives::call(&mut e, "actor", "equipitem", Some(&me), &[scroll]);
    assert_eq!(e.equipped_spell(PLAYER_REF, RIGHT), Some(SCROLL));
    assert_eq!(e.inventories[&PLAYER_REF].count(SCROLL), 2);
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn casting_a_scroll_uses_it_up_without_magicka() {
    let (_f, mut e) = fixture();
    e.add_item(PLAYER_REF, SCROLL, 2);
    let r = row(&mut e, SCROLL);
    e.use_listed_item(&r, Some(RIGHT));

    // Let go before it charged: nothing is used.
    cast(&mut e, RIGHT, 0.1);
    assert_eq!(e.item_count(PLAYER_REF, SCROLL), 2);
    assert_eq!(effects_of(&e, SCROLL), 0);

    // Magicka isn't touched, with none at all too.
    e.set_actor_value(PLAYER_REF, av::MAGICKA, 0.0);
    cast(&mut e, RIGHT, 0.6);
    assert_eq!(e.actor_value(PLAYER_REF, av::MAGICKA), 0.0);
    assert_eq!(e.item_count(PLAYER_REF, SCROLL), 1);
    assert_eq!(effects_of(&e, SCROLL), 1);
    assert_eq!(
        e.equipped_spell(PLAYER_REF, RIGHT),
        Some(SCROLL),
        "one is left to cast"
    );

    // The last one is used up and the hand lets go.
    cast(&mut e, RIGHT, 0.6);
    assert_eq!(e.item_count(PLAYER_REF, SCROLL), 0);
    assert_eq!(e.equipped_spell(PLAYER_REF, RIGHT), None);
    assert!(!e.inventories[&PLAYER_REF].is_equipped(SCROLL));
    assert_eq!(
        e.stat("Favorite Spell"),
        crate::stats::StatValue::Name(String::new())
    );
}

/// Exercise the actual movement update without needing world geometry.
fn walk(e: &mut Engine, seconds: f32) {
    let frames = (seconds * 60.0).round() as usize;
    for _ in 0..frames {
        e.update(
            crate::player::MoveInput {
                forward: 1.0,
                right: 1.0,
                sprint: true,
                jump: true,
                ..Default::default()
            },
            1.0 / 60.0,
            0.0,
        );
    }
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn ordinary_scroll_allows_walking_but_blocks_sprint_until_release() {
    let (_f, mut e) = fixture();
    e.player.noclip = true;
    for hand in [LEFT, RIGHT] {
        e.add_item(PLAYER_REF, SCROLL, 2);
        let r = row(&mut e, SCROLL);
        e.use_listed_item(&r, Some(hand));
        let before = e.player.position;
        assert!(e.player_cast_press(hand));
        walk(&mut e, 0.6);
        assert!(e.player_scroll_casting());
        assert!(!e.player_scroll_held());
        assert_ne!(e.player.position, before);
        assert!(!e.player.sprinting);
        assert!(e.player_cast_release(hand));
        assert!(!e.player_scroll_held());
        walk(&mut e, 0.1);
        assert_ne!(e.player.position, before);

        let count = e.item_count(PLAYER_REF, SCROLL);
        assert!(e.player_cast_press(hand));
        walk(&mut e, 0.1);
        assert!(e.player_cast_release(hand));
        assert!(!e.player_scroll_held());
        assert_eq!(e.item_count(PLAYER_REF, SCROLL), count);
        let before = e.player.position;
        walk(&mut e, 0.1);
        assert_ne!(e.player.position, before);

        assert!(e.player_cast_press(hand));
        e.use_listed_item(&r, Some(hand)); // unequip during charging
        walk(&mut e, 0.1);
        assert!(!e.player_scroll_held());
    }
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn ritual_holds_through_release_exit_but_spells_allow_movement() {
    let (_f, mut e) = fixture();
    e.player.noclip = true;
    // Model the animation's release interval without requiring game assets.
    for hand in [LEFT, RIGHT] {
        e.casting.firing[hand] = Some(Firing {
            spell: TWO_HAND,
            dual: false,
            fired: false,
            waited: 0.0,
        });
        e.add_item(PLAYER_REF, TWO_HAND, 2);
        e.equip_in(PLAYER_REF, TWO_HAND, Some(hand)).unwrap();
        let before = e.player.position;
        walk(&mut e, 0.1);
        assert_eq!(e.player.position, before);
        let mut fire = [false; 2];
        fire[hand] = true;
        e.player_spell_events(fire, [false; 2], 0.0);
        assert!(e.player_scroll_held());
        walk(&mut e, 0.1);
        assert_eq!(e.player.position, before);
        e.player_spell_events([false; 2], [true; 2], 0.0);
        assert!(!e.player_scroll_held());
        walk(&mut e, 0.1);
        assert_ne!(e.player.position, before);
        e.casting.firing[hand] = None;
    }
    e.equip_spell(PLAYER_REF, SPELL, RIGHT);
    assert!(e.player_cast_press(RIGHT));
    assert!(!e.player_scroll_held());
    let before = e.player.position;
    walk(&mut e, 0.6);
    assert_ne!(e.player.position, before);
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn concentration_scroll_allows_walking_but_blocks_jump_and_sprint() {
    let (_f, mut e) = fixture();
    e.add_item(PLAYER_REF, OTHER, 1);
    let r = row(&mut e, OTHER);
    e.use_listed_item(&r, Some(LEFT));
    assert!(e.player_cast_press(LEFT));
    assert!(e.casting.hands[LEFT].as_ref().unwrap().flowing);
    e.player.grounded = true;
    e.test_jump = true;
    e.player.sneaking = true;
    e.update(
        crate::player::MoveInput {
            forward: 1.0,
            sprint: true,
            jump: true,
            ..Default::default()
        },
        1.0 / 60.0,
        0.0,
    );
    assert!(e.player_scroll_casting());
    assert!(!e.player_scroll_held());
    assert!(!e.player.jumped);
    assert!(!e.test_jump);
    assert!(e.player.sneaking);
    assert!(e.player_cast_release(LEFT));
    assert!(!e.player_scroll_held());
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn two_handed_scroll_holds_movement() {
    let (_f, mut e) = fixture();
    e.player.noclip = true;
    e.add_item(PLAYER_REF, TWO_HAND, 1);
    let r = row(&mut e, TWO_HAND);
    e.use_listed_item(&r, Some(RIGHT));
    for hand in [LEFT, RIGHT] {
        assert_eq!(e.equipped_spell(PLAYER_REF, hand), Some(TWO_HAND));
    }
    assert!(e.player_cast_press(RIGHT));
    assert!(e.player_cast_press(LEFT));
    assert_eq!(e.casting.hands.iter().flatten().count(), 1);
    assert!(e.player_scroll_held());
    let before = e.player.position;
    walk(&mut e, 0.6);
    assert_eq!(e.player.position, before);
    assert!(
        !e.player_draw_weapon(false),
        "ritual cast refuses sheathing"
    );
    assert!(e.player_cast_release(RIGHT));
    assert!(e.player_scroll_held(), "brief release recovery");
    walk(&mut e, 0.4);
    assert!(!e.player_scroll_held());
    assert_ne!(e.player.position, before);
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn scroll_assets_supply_charge_time_and_release_cast_type() {
    let (_f, mut e) = fixture();
    assert_eq!(e.magic_item(AUTO_SCROLL).unwrap().charge_time, 0.5);
    assert_eq!(e.magic_item(RELEASE_SCROLL).unwrap().cast_type, 1);
    e.add_item(PLAYER_REF, RELEASE_SCROLL, 1);
    let r = row(&mut e, RELEASE_SCROLL);
    e.use_listed_item(&r, Some(RIGHT));
    assert!(e.player_cast_press(RIGHT));
    e.update_casting(0.6);
    assert_eq!(effects_of(&e, RELEASE_SCROLL), 0, "wait for release");
    assert!(e.player_cast_release(RIGHT));
    assert_eq!(effects_of(&e, RELEASE_SCROLL), 1);
    assert_eq!(e.item_count(PLAYER_REF, RELEASE_SCROLL), 0);
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn scroll_cannot_start_airborne_or_swimming_but_can_start_wading() {
    let (_f, mut e) = fixture();
    e.add_item(PLAYER_REF, SCROLL, 2);
    let r = row(&mut e, SCROLL);
    e.use_listed_item(&r, Some(RIGHT));
    e.player.grounded = false;
    e.player_cast_press(RIGHT);
    assert!(!e.player_scroll_casting());
    e.player.grounded = true;
    e.player.swimming = true;
    e.player_cast_press(RIGHT);
    assert!(!e.player_scroll_casting());
    e.player.swimming = false;
    e.player_cast_press(RIGHT);
    assert!(e.player_scroll_casting());
    assert_eq!(e.item_count(PLAYER_REF, SCROLL), 2);
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn ritual_early_release_recovers_and_switching_items_cancels_without_consumption() {
    let (_f, mut e) = fixture();
    e.player.noclip = true;
    e.add_item(PLAYER_REF, TWO_HAND, 1);
    let r = row(&mut e, TWO_HAND);
    e.use_listed_item(&r, Some(RIGHT));
    e.player_cast_press(RIGHT);
    e.update_casting(0.1);
    e.player_cast_release(RIGHT);
    assert!(e.player_scroll_held());
    e.update_casting(0.31);
    assert!(!e.player_scroll_held());
    assert_eq!(e.item_count(PLAYER_REF, TWO_HAND), 1);
    e.player_cast_press(RIGHT);
    e.equip_spell(PLAYER_REF, SPELL, RIGHT);
    e.update_casting(0.01);
    assert!(!e.player_scroll_held());
    assert_eq!(e.item_count(PLAYER_REF, TWO_HAND), 1);
}
