//! Bow ammunition selection over generated plugin records; no game assets.

use super::*;
use crate::world::inventory::Inventory;
use esp::writer::{Node, RecordBuilder, write_plugin};

const WEAK: FormId = FormId(0x800);
const STRONG: FormId = FormId(0x801);
const BOLT: FormId = FormId(0x802);
const BROKEN: FormId = FormId(0x803);
const OTHER: FormId = FormId(0x804);
const WEAK_BOLT: FormId = FormId(0x805);
const HIDDEN: FormId = FormId(0x806);
const TIE: FormId = FormId(0x807);
const BOW: FormId = FormId(0x808);
const CROSSBOW: FormId = FormId(0x809);
const SWORD: FormId = FormId(0x80A);

struct Fixture(std::path::PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture(iron_is_arrow: bool) -> (Fixture, LoadOrder) {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let f = Fixture(std::env::temp_dir().join(format!(
        "vrm-bow-ammo-{}-{stamp}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )));
    std::fs::create_dir(&f.0).unwrap();
    let ammo = |id: FormId, flags: u32, damage: f32| {
        let mut data = [0u8; 20];
        data[4..8].copy_from_slice(&flags.to_le_bytes());
        data[8..12].copy_from_slice(&damage.to_le_bytes());
        RecordBuilder::new(b"AMMO", id).field(b"DATA", &data)
    };
    let weapon = |id: FormId, anim: u8| {
        let mut dnam = [0u8; 100];
        dnam[0] = anim;
        RecordBuilder::new(b"WEAP", id).field(b"DNAM", &dnam)
    };
    let records = vec![
        Node::top(
            b"AMMO",
            vec![
                ammo(WEAK, AMMO_NON_BOLT, 8.0),
                // Other AMMO flags do not change arrow/bolt compatibility.
                ammo(STRONG, AMMO_NON_BOLT | 1, 20.0),
                ammo(BOLT, 0, 100.0),
                ammo(WEAK_BOLT, 0, 5.0),
                ammo(HIDDEN, AMMO_NON_BOLT | AMMO_NON_PLAYABLE, 1.0),
                ammo(TIE, AMMO_NON_BOLT, 8.0),
                RecordBuilder::new(b"AMMO", BROKEN).field(b"DATA", &[0; 8]),
                ammo(
                    IRON_ARROW,
                    if iron_is_arrow { AMMO_NON_BOLT } else { 0 },
                    8.0,
                ),
            ],
        ),
        Node::top(b"MISC", vec![RecordBuilder::new(b"MISC", OTHER)]),
        Node::top(
            b"WEAP",
            vec![
                weapon(BOW, ANIM_BOW),
                weapon(CROSSBOW, ANIM_CROSSBOW),
                weapon(SWORD, 1),
            ],
        ),
    ];
    std::fs::write(
        f.0.join("Ammo.esp"),
        write_plugin(
            0,
            &[],
            "test",
            "synthetic bow ammunition",
            0x14000,
            &records,
        ),
    )
    .unwrap();
    let lo = LoadOrder::load(&f.0, &["Ammo.esp".into()]).unwrap();
    (f, lo)
}

fn inventory() -> Inventory {
    let mut inv = Inventory::default();
    for item in [WEAK, STRONG, BOLT] {
        inv.add(item, 10);
    }
    inv
}

#[test]
fn worn_bolts_never_supply_a_bow() {
    let (_fixture, lo) = fixture(true);
    let mut inv = inventory();
    inv.wear(BOLT);
    assert_eq!(arrows_in(&lo, Some(&inv), true).unwrap().ammo, STRONG);
    assert!(
        arrows_in(&lo, Some(&inv), false).is_none(),
        "the player's bolts"
    );
    // The general projectile reader still supports bolt records.
    assert_eq!(arrow(&lo, BOLT).unwrap().damage, 100.0);
}

#[test]
fn a_carried_equipped_arrow_wins_even_after_incompatible_equipment() {
    let (_fixture, lo) = fixture(true);
    let mut inv = inventory();
    inv.wear(BOLT);
    inv.wear(WEAK);
    assert_eq!(arrows_in(&lo, Some(&inv), false).unwrap().ammo, WEAK);
}

#[test]
fn invalid_or_exhausted_equipment_does_not_hide_usable_arrows() {
    let (_fixture, lo) = fixture(true);
    let mut inv = inventory();
    inv.items.retain(|&(id, _)| id != STRONG);
    inv.items.push((STRONG, 0));
    inv.add(BROKEN, 1);
    inv.add(OTHER, 1);
    for item in [BROKEN, OTHER, STRONG, FormId(0x999)] {
        inv.wear(item);
    }
    assert_eq!(arrows_in(&lo, Some(&inv), true).unwrap().ammo, WEAK);
    assert!(arrows_in(&lo, Some(&inv), false).is_none());
}

#[test]
fn only_npcs_get_default_arrows_when_no_compatible_ammo_remains() {
    let (_fixture, lo) = fixture(true);
    let mut inv = Inventory::default();
    inv.add(BOLT, 10);
    inv.wear(BOLT);
    assert!(arrows_in(&lo, Some(&inv), false).is_none());
    assert_eq!(arrows_in(&lo, Some(&inv), true).unwrap().ammo, IRON_ARROW);
    assert!(arrows_in(&lo, None, false).is_none());
    assert_eq!(arrows_in(&lo, None, true).unwrap().ammo, IRON_ARROW);

    // A mod overriding the fallback record as a bolt cannot bypass the rule.
    let (_fixture, lo) = fixture(false);
    assert!(arrows_in(&lo, Some(&inv), true).is_none());
}

#[test]
fn bows_shoot_arrows_and_crossbows_bolts() {
    let (_fixture, lo) = fixture(true);
    assert_eq!(ammo_for(&lo, BOW, WEAK), Some(8.0));
    assert_eq!(ammo_for(&lo, BOW, BOLT), None);
    assert_eq!(ammo_for(&lo, CROSSBOW, BOLT), Some(100.0));
    assert_eq!(ammo_for(&lo, CROSSBOW, WEAK), None);
    assert_eq!(ammo_for(&lo, SWORD, WEAK), None);
    assert_eq!(ammo_for(&lo, BOW, BROKEN), None);
    assert_eq!(ammo_for(&lo, BOW, OTHER), None);
    assert!(shoots_ammo(&lo, BOW) && shoots_ammo(&lo, CROSSBOW));
    assert!(!shoots_ammo(&lo, SWORD) && !shoots_ammo(&lo, WEAK));
}

#[test]
fn the_weakest_compatible_playable_ammunition_is_picked() {
    let (_fixture, lo) = fixture(true);
    let mut inv = inventory();
    inv.add(WEAK_BOLT, 3);
    inv.add(HIDDEN, 3);
    assert_eq!(weakest_in(&lo, &inv, BOW), Some(WEAK));
    assert_eq!(weakest_in(&lo, &inv, CROSSBOW), Some(WEAK_BOLT));
    assert_eq!(weakest_in(&lo, &inv, SWORD), None);
    // None left of a stack: the next weakest.
    inv.items.retain(|&(f, _)| f != WEAK);
    inv.items.push((WEAK, 0));
    assert_eq!(weakest_in(&lo, &inv, BOW), Some(STRONG));
    let mut bolts = Inventory::default();
    bolts.add(BOLT, 1);
    assert_eq!(weakest_in(&lo, &bolts, BOW), None);
}

#[test]
fn equal_damage_goes_to_the_first_listed() {
    let (_fixture, lo) = fixture(true);
    let mut inv = Inventory::default();
    inv.add(TIE, 1);
    inv.add(WEAK, 1);
    assert_eq!(weakest_in(&lo, &inv, BOW), Some(TIE));
    let mut inv = Inventory::default();
    inv.add(WEAK, 1);
    inv.add(TIE, 1);
    assert_eq!(weakest_in(&lo, &inv, BOW), Some(WEAK));
}

#[test]
fn worn_ammunition_counts_only_for_its_weapon() {
    let (_fixture, lo) = fixture(true);
    let mut inv = inventory();
    inv.wear(BOLT);
    assert!(!wears_ammo_for(&lo, &inv, BOW));
    assert!(wears_ammo_for(&lo, &inv, CROSSBOW));
    inv.wear(STRONG);
    assert!(wears_ammo_for(&lo, &inv, BOW));
}

#[test]
fn the_player_shoots_only_equipped_arrows() {
    let (_fixture, lo) = fixture(true);
    let mut inv = inventory();
    assert!(arrows_in(&lo, Some(&inv), false).is_none());
    assert_eq!(arrows_in(&lo, Some(&inv), true).unwrap().ammo, STRONG);
    inv.wear(WEAK);
    assert_eq!(arrows_in(&lo, Some(&inv), false).unwrap().ammo, WEAK);
}
