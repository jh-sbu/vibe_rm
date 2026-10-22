//! Status conditions (platform, race, items, ownership...) on generated records
//! only. Engine owns a renderer, so these need a headless GPU adapter.

use super::*;
use crate::world::inventory::Inventory;
use esp::writer::{Node, RecordBuilder, write_plugin};

pub(super) const SWORD_REF: FormId = FormId(0x814);
const PLAYABLE_RACE: FormId = FormId(0x830);
const CREATURE_RACE: FormId = FormId(0x831);
const HUMAN: FormId = FormId(0x832);
const BEAST: FormId = FormId(0x833);
const HUMAN_REF: FormId = FormId(0x834);
const BEAST_REF: FormId = FormId(0x835);
const KEYWORD: FormId = FormId(0x840);
const OTHER_KEYWORD: FormId = FormId(0x841);
const TAGGED_A: FormId = FormId(0x842);
const TAGGED_B: FormId = FormId(0x843);
const UNTAGGED: FormId = FormId(0x844);
const SWORD: FormId = FormId(0x850);
const GREATSWORD: FormId = FormId(0x851);
const BOUND_SWORD: FormId = FormId(0x852);
const BOUND_BATTLEAXE: FormId = FormId(0x853);
const BOW: FormId = FormId(0x854);

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
    let f = Fixture(
        std::env::temp_dir().join(format!("vrm-status-cond-{}-{stamp}", std::process::id())),
    );
    std::fs::create_dir(&f.0).unwrap();
    let race = |id, flags: u32| {
        let mut data = [0u8; 36];
        data[32..36].copy_from_slice(&flags.to_le_bytes());
        RecordBuilder::new(b"RACE", id).field(b"DATA", &data)
    };
    let npc = |id, race| {
        RecordBuilder::new(b"NPC_", id)
            .field(b"ACBS", &[0; 24])
            .form(b"RNAM", race)
    };
    // `DNAM`: the animation type, and the bound weapon flag among flags2.
    let weapon = |id, anim: u8, bound: bool| {
        let mut dnam = [0u8; 100];
        dnam[0] = anim;
        dnam[40..44].copy_from_slice(&(if bound { 0x2000u32 } else { 0 }).to_le_bytes());
        RecordBuilder::new(b"WEAP", id).field(b"DNAM", &dnam)
    };
    let actor = |id, base| RecordBuilder::new(b"ACHR", id).form(b"NAME", base);
    let records = [
        Node::top(
            b"RACE",
            vec![race(PLAYABLE_RACE, 0x1), race(CREATURE_RACE, 0)],
        ),
        Node::top(
            b"NPC_",
            vec![npc(HUMAN, PLAYABLE_RACE), npc(BEAST, CREATURE_RACE)],
        ),
        Node::top(
            b"ACHR",
            vec![actor(HUMAN_REF, HUMAN), actor(BEAST_REF, BEAST)],
        ),
        Node::top(
            b"KYWD",
            vec![
                RecordBuilder::new(b"KYWD", KEYWORD),
                RecordBuilder::new(b"KYWD", OTHER_KEYWORD),
            ],
        ),
        Node::top(
            b"MISC",
            vec![
                RecordBuilder::new(b"MISC", TAGGED_A).form(b"KWDA", KEYWORD),
                RecordBuilder::new(b"MISC", TAGGED_B).form(b"KWDA", KEYWORD),
                RecordBuilder::new(b"MISC", UNTAGGED).form(b"KWDA", OTHER_KEYWORD),
            ],
        ),
        Node::top(
            b"WEAP",
            vec![
                weapon(SWORD, 1, false),
                weapon(GREATSWORD, 5, false),
                weapon(BOUND_SWORD, 1, true),
                weapon(BOUND_BATTLEAXE, 6, true),
                weapon(BOW, 7, false),
            ],
        ),
        Node::top(b"REFR", vec![RecordBuilder::new(b"REFR", SWORD_REF)]),
    ];
    std::fs::write(
        f.0.join("Status.esp"),
        write_plugin(
            0,
            &[],
            "test",
            "synthetic status conditions",
            0x900,
            &records,
        ),
    )
    .unwrap();
    let mut e = crate::stats::action_tests::headless_engine(&f.0, "Status.esp");
    let mut bag = Inventory::default();
    bag.add(TAGGED_A, 2);
    bag.add(TAGGED_B, 3);
    bag.add(UNTAGGED, 7);
    e.inventories.insert(HUMAN_REF, bag);
    (f, e)
}

/// `func(param) == value` as a CTDA record's bytes.
fn ctda(func: u16, param: u32, value: f32) -> [u8; 32] {
    let mut d = [0; 32];
    d[4..8].copy_from_slice(&value.to_le_bytes());
    d[8..10].copy_from_slice(&func.to_le_bytes());
    d[12..16].copy_from_slice(&param.to_le_bytes());
    d
}

/// Whether `func(param) == value` holds for `subject`.
fn is(e: &Engine, subject: FormId, func: u16, param: u32, value: f32) -> bool {
    let rec = e.lo.get(SWORD_REF).unwrap();
    let c = crate::condition::parse(&rec, &ctda(func, param, value)).unwrap();
    crate::condition::evaluate(
        e,
        &[c],
        crate::condition::Context {
            subject: Some(subject),
            ..Default::default()
        },
    )
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn the_platform_is_a_pc_with_gore() {
    let (_f, e) = fixture();
    assert!(is(&e, PLAYER_REF, 521, 0, 1.0)); // IsWin32
    assert!(is(&e, PLAYER_REF, 309, 0, 0.0)); // IsXBox
    assert!(is(&e, PLAYER_REF, 520, 0, 0.0)); // IsPS3
    assert!(is(&e, PLAYER_REF, 547, 0, 0.0)); // IsGoreDisabled
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn get_is_playable_race_reads_the_races_playable_flag() {
    let (_f, e) = fixture();
    assert!(is(&e, HUMAN_REF, 254, 0, 1.0));
    assert!(!is(&e, BEAST_REF, 254, 0, 1.0));
    // Nothing to ask about: no race, not playable.
    assert!(!is(&e, SWORD_REF, 254, 0, 1.0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn get_keyword_item_count_totals_the_carried_items_with_the_keyword() {
    let (_f, mut e) = fixture();
    assert!(is(&e, HUMAN_REF, 465, KEYWORD.0, 5.0));
    assert!(is(&e, HUMAN_REF, 465, OTHER_KEYWORD.0, 7.0));
    // A stack taken away stops counting; a keyword nothing has is zero.
    e.inventories
        .get_mut(&HUMAN_REF)
        .unwrap()
        .remove_split(TAGGED_A, 2, None);
    assert!(is(&e, HUMAN_REF, 465, KEYWORD.0, 3.0));
    assert!(is(&e, HUMAN_REF, 465, 0x845, 0.0));
    assert!(is(&e, BEAST_REF, 465, KEYWORD.0, 0.0));
}

/// `actor` holds `item` in `hand`.
fn hold(e: &mut Engine, actor: FormId, item: FormId, hands: &[usize]) {
    let lo = &e.lo;
    let inv = e.inventories.entry(actor).or_default();
    inv.add(item, 1);
    inv.hold(lo, item, hands);
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn two_handed_weapons_are_the_two_handed_animation_types() {
    use crate::spells::RIGHT;
    let (_f, mut e) = fixture();
    assert!(is(&e, HUMAN_REF, 636, 0, 0.0));
    hold(&mut e, HUMAN_REF, SWORD, &[RIGHT]);
    assert!(is(&e, HUMAN_REF, 636, 0, 0.0));
    hold(&mut e, HUMAN_REF, BOUND_BATTLEAXE, &[RIGHT]);
    assert!(is(&e, HUMAN_REF, 636, 0, 1.0));
    hold(&mut e, HUMAN_REF, GREATSWORD, &[RIGHT]);
    assert!(is(&e, HUMAN_REF, 636, 0, 1.0));
    hold(&mut e, HUMAN_REF, BOW, &[RIGHT]);
    assert!(is(&e, HUMAN_REF, 636, 0, 0.0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn bound_weapons_are_found_in_the_hand_the_source_names() {
    use crate::spells::{LEFT, RIGHT};
    let (_f, mut e) = fixture();
    hold(&mut e, HUMAN_REF, BOUND_SWORD, &[RIGHT]);
    hold(&mut e, HUMAN_REF, SWORD, &[LEFT]);
    // Sources: 0 the left hand, 1 the right, 2 the voice.
    assert!(is(&e, HUMAN_REF, 706, 1, 1.0));
    assert!(is(&e, HUMAN_REF, 706, 0, 0.0));
    assert!(is(&e, HUMAN_REF, 706, 2, 0.0));
    assert!(is(&e, BEAST_REF, 706, 1, 0.0));
}
