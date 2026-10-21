//! Reference-state conditions (`GetIsObjectType`, `IsWeaponInList`,
//! `GetDefaultOpen`, `IsUnlockedDoor`) on generated records only. Engine owns a
//! renderer, so these need a headless GPU adapter.

use super::*;
use crate::world::inventory::Inventory;
use esp::writer::{Node, RecordBuilder, write_plugin};

const SWORD: FormId = FormId(0x800);
const CUIRASS: FormId = FormId(0x801);
const NPC: FormId = FormId(0x802);
const ACTOR: FormId = FormId(0x803);
const LIST: FormId = FormId(0x804);
const DOOR_BASE: FormId = FormId(0x805);
const OPEN_DOOR: FormId = FormId(0x810);
const LOCKED_DOOR: FormId = FormId(0x811);
const PLAIN_DOOR: FormId = FormId(0x812);
const ARMOR_REF: FormId = FormId(0x813);
const SWORD_REF: FormId = FormId(0x814);

const GET_IS_OBJECT_TYPE: u16 = 432;
const IS_WEAPON_IN_LIST: u16 = 398;
const GET_DEFAULT_OPEN: u16 = 215;
const IS_UNLOCKED_DOOR: u16 = 718;

/// `ptFormType` values (xEdit's `wbFormTypeEnum`).
const ARMOR: u32 = 1;
const DOOR: u32 = 4;
const LIGHT: u32 = 6;
const WEAPON: u32 = 12;
const ACTOR_TYPE: u32 = 13;

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
        std::env::temp_dir().join(format!("vrm-reference-cond-{}-{stamp}", std::process::id())),
    );
    std::fs::create_dir(&f.0).unwrap();
    let mut acbs = [0; 24];
    acbs[8..10].copy_from_slice(&20u16.to_le_bytes());
    let reference = |id, base| RecordBuilder::new(b"REFR", id).form(b"NAME", base);
    let records = [
        Node::top(b"WEAP", vec![RecordBuilder::new(b"WEAP", SWORD)]),
        Node::top(b"ARMO", vec![RecordBuilder::new(b"ARMO", CUIRASS)]),
        Node::top(
            b"NPC_",
            vec![RecordBuilder::new(b"NPC_", NPC).field(b"ACBS", &acbs)],
        ),
        Node::top(
            b"ACHR",
            vec![RecordBuilder::new(b"ACHR", ACTOR).form(b"NAME", NPC)],
        ),
        Node::top(
            b"FLST",
            vec![RecordBuilder::new(b"FLST", LIST).form(b"LNAM", SWORD)],
        ),
        Node::top(b"DOOR", vec![RecordBuilder::new(b"DOOR", DOOR_BASE)]),
        Node::top(
            b"REFR",
            vec![
                // Open by default: the `ONAM` (with `XACT`) marks it.
                reference(OPEN_DOOR, DOOR_BASE)
                    .field(b"ONAM", &[])
                    .u32(b"XACT", 0x0d),
                // Placed locked: an `XLOC` (its contents don't matter here).
                reference(LOCKED_DOOR, DOOR_BASE).field(b"XLOC", &[0; 12]),
                reference(PLAIN_DOOR, DOOR_BASE),
                reference(ARMOR_REF, CUIRASS),
                reference(SWORD_REF, SWORD),
            ],
        ),
    ];
    std::fs::write(
        f.0.join("Reference.esp"),
        write_plugin(
            0,
            &[],
            "test",
            "synthetic reference conditions",
            0x900,
            &records,
        ),
    )
    .unwrap();
    let mut e = crate::stats::action_tests::headless_engine(&f.0, "Reference.esp");
    let mut inventory = Inventory::default();
    inventory.add(SWORD, 1);
    inventory.equip(&e.lo, SWORD);
    e.inventories.insert(ACTOR, inventory);
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

/// Whether `func(param) == 1` holds for `subject`.
fn holds(e: &Engine, subject: FormId, func: u16, param: u32) -> bool {
    let rec = e.lo.get(SWORD_REF).unwrap();
    let c = crate::condition::parse(&rec, &ctda(func, param, 1.0)).unwrap();
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
fn get_is_object_type_compares_the_base_record_type() {
    let (_f, e) = fixture();
    assert!(holds(&e, SWORD_REF, GET_IS_OBJECT_TYPE, WEAPON));
    assert!(!holds(&e, SWORD_REF, GET_IS_OBJECT_TYPE, ARMOR));
    assert!(holds(&e, ARMOR_REF, GET_IS_OBJECT_TYPE, ARMOR));
    assert!(holds(&e, PLAIN_DOOR, GET_IS_OBJECT_TYPE, DOOR));
    assert!(holds(&e, ACTOR, GET_IS_OBJECT_TYPE, ACTOR_TYPE));
    assert!(!holds(&e, ACTOR, GET_IS_OBJECT_TYPE, LIGHT));
    // A number the enum doesn't list is no type at all.
    assert!(!holds(&e, SWORD_REF, GET_IS_OBJECT_TYPE, 99));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn is_weapon_in_list_asks_for_the_weapon_in_hand() {
    let (_f, e) = fixture();
    assert!(holds(&e, ACTOR, IS_WEAPON_IN_LIST, LIST.0));
    // No weapon to hold, and a form that isn't a list names nothing.
    assert!(!holds(&e, ARMOR_REF, IS_WEAPON_IN_LIST, LIST.0));
    assert!(!holds(&e, ACTOR, IS_WEAPON_IN_LIST, CUIRASS.0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn get_default_open_reads_the_placed_open_by_default_marker() {
    let (_f, e) = fixture();
    assert!(holds(&e, OPEN_DOOR, GET_DEFAULT_OPEN, 0));
    assert!(!holds(&e, PLAIN_DOOR, GET_DEFAULT_OPEN, 0));
    assert!(!holds(&e, LOCKED_DOOR, GET_DEFAULT_OPEN, 0));
    // A reference the engine made itself has no record to carry the marker.
    assert!(!holds(&e, PLAYER_REF, GET_DEFAULT_OPEN, 0));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn is_unlocked_door_is_a_door_that_is_not_locked() {
    let (_f, mut e) = fixture();
    assert!(holds(&e, PLAIN_DOOR, IS_UNLOCKED_DOOR, 0));
    assert!(!holds(&e, LOCKED_DOOR, IS_UNLOCKED_DOOR, 0));
    // A script's unlock or lock is what counts.
    e.set_locked(LOCKED_DOOR, false);
    assert!(holds(&e, LOCKED_DOOR, IS_UNLOCKED_DOOR, 0));
    e.set_locked(PLAIN_DOOR, true);
    assert!(!holds(&e, PLAIN_DOOR, IS_UNLOCKED_DOOR, 0));
    // Not doors: never unlocked doors.
    assert!(!holds(&e, ARMOR_REF, IS_UNLOCKED_DOOR, 0));
    assert!(!holds(&e, PLAYER_REF, IS_UNLOCKED_DOOR, 0));
}
