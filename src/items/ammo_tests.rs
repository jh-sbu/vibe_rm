//! Ammunition put on with a bow or crossbow, generated records only. Engine
//! owns a renderer, so these need a headless GPU adapter.

use super::*;
use esp::writer::{Node, RecordBuilder, write_plugin};

const IRON: FormId = FormId(0x800);
const STEEL: FormId = FormId(0x801);
const ELVEN: FormId = FormId(0x802);
const BOLT: FormId = FormId(0x803);
const BOW: FormId = FormId(0x810);
const OTHER_BOW: FormId = FormId(0x811);
const CROSSBOW: FormId = FormId(0x812);
const SWORD: FormId = FormId(0x813);

struct Fixture(std::path::PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture() -> (Fixture, Engine) {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let f = Fixture(std::env::temp_dir().join(format!(
        "vrm-equip-ammo-{}-{stamp}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )));
    std::fs::create_dir(&f.0).unwrap();
    let ammo = |id: FormId, name: &str, arrow: bool, damage: f32| {
        let mut data = [0u8; 20];
        let flags: u32 = if arrow { 0x4 } else { 0 };
        data[4..8].copy_from_slice(&flags.to_le_bytes());
        data[8..12].copy_from_slice(&damage.to_le_bytes());
        RecordBuilder::new(b"AMMO", id)
            .zstring(b"FULL", name)
            .field(b"DATA", &data)
    };
    let weapon = |id: FormId, name: &str, anim: u8| {
        let mut dnam = [0u8; 100];
        dnam[0] = anim;
        dnam[4..8].copy_from_slice(&1.0f32.to_le_bytes());
        RecordBuilder::new(b"WEAP", id)
            .zstring(b"FULL", name)
            .field(b"DATA", &[0; 10])
            .field(b"DNAM", &dnam)
    };
    let records = [
        Node::top(
            b"AMMO",
            vec![
                ammo(IRON, "Iron Arrow", true, 8.0),
                ammo(STEEL, "Steel Arrow", true, 10.0),
                ammo(ELVEN, "Elven Arrow", true, 16.0),
                ammo(BOLT, "Steel Bolt", false, 10.0),
            ],
        ),
        Node::top(
            b"WEAP",
            vec![
                weapon(BOW, "Bow", 7),
                weapon(OTHER_BOW, "Other Bow", 7),
                weapon(CROSSBOW, "Crossbow", 9),
                weapon(SWORD, "Sword", 1),
            ],
        ),
    ];
    std::fs::write(
        f.0.join("Ammo.esp"),
        write_plugin(0, &[], "test", "synthetic ammunition", 0x900, &records),
    )
    .unwrap();
    let e = crate::stats::action_tests::headless_engine(&f.0, "Ammo.esp");
    (f, e)
}

fn worn(e: &Engine, item: FormId) -> bool {
    e.inventories
        .get(&PLAYER_REF)
        .is_some_and(|i| i.is_equipped(item))
}

fn give(e: &mut Engine, items: &[FormId]) {
    for &i in items {
        e.add_item(
            PLAYER_REF,
            i,
            if i == BOW || i == CROSSBOW { 1 } else { 10 },
        );
    }
}

fn row(e: &mut Engine, item: FormId) -> ListedItem {
    e.listed_inventory(PLAYER_REF)
        .into_iter()
        .find(|r| r.item == item)
        .expect("listed")
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn a_bow_from_the_menu_puts_on_the_weakest_arrows() {
    let (_f, mut e) = fixture();
    give(&mut e, &[STEEL, ELVEN, IRON, BOLT, BOW]);
    let bow = row(&mut e, BOW);
    e.use_listed_item(&bow, None);
    assert!(worn(&e, BOW) && worn(&e, IRON));
    assert!(!worn(&e, STEEL) && !worn(&e, ELVEN) && !worn(&e, BOLT));
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn arrows_chosen_stay_through_the_bow_coming_off_and_on() {
    let (_f, mut e) = fixture();
    give(&mut e, &[IRON, STEEL, BOW, OTHER_BOW]);
    // As Papyrus EquipItem does.
    e.equip_item(PLAYER_REF, BOW, true).unwrap();
    e.equip_item(PLAYER_REF, STEEL, true).unwrap();
    assert!(worn(&e, STEEL) && !worn(&e, IRON));
    e.equip_item(PLAYER_REF, BOW, false).unwrap();
    assert!(
        !worn(&e, BOW) && worn(&e, STEEL),
        "arrows stay without the bow"
    );
    e.equip_item(PLAYER_REF, BOW, true).unwrap();
    assert!(worn(&e, STEEL) && !worn(&e, IRON));
    e.equip_item(PLAYER_REF, OTHER_BOW, true).unwrap();
    assert!(worn(&e, OTHER_BOW) && worn(&e, STEEL));
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn bolts_go_with_crossbows_and_arrows_with_bows() {
    let (_f, mut e) = fixture();
    give(&mut e, &[IRON, BOLT, BOW, CROSSBOW]);
    e.equip_item(PLAYER_REF, BOLT, true).unwrap();
    e.equip_item(PLAYER_REF, BOW, true).unwrap();
    assert!(
        worn(&e, IRON) && !worn(&e, BOLT),
        "worn bolts give way to arrows"
    );
    e.equip_item(PLAYER_REF, CROSSBOW, true).unwrap();
    assert!(worn(&e, BOLT) && !worn(&e, IRON));
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn nothing_is_put_on_without_ammunition_or_for_other_weapons_or_npcs() {
    let (_f, mut e) = fixture();
    give(&mut e, &[BOLT, BOW]);
    e.equip_item(PLAYER_REF, BOW, true).unwrap();
    assert!(worn(&e, BOW) && !worn(&e, BOLT), "a bow doesn't take bolts");

    let (_f, mut e) = fixture();
    give(&mut e, &[IRON]);
    e.add_item(PLAYER_REF, SWORD, 1);
    e.equip_item(PLAYER_REF, SWORD, true).unwrap();
    assert!(!worn(&e, IRON));

    let (_f, mut e) = fixture();
    let npc = FormId(0xFF00_0001);
    for i in [IRON, BOW] {
        e.add_item(npc, i, 1);
    }
    e.equip_item(npc, BOW, true).unwrap();
    assert!(!e.inventories[&npc].is_equipped(IRON), "only the player's");
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn shots_take_the_worn_arrows_until_they_run_out() {
    let (_f, mut e) = fixture();
    give(&mut e, &[ELVEN, BOW]);
    e.add_item(PLAYER_REF, IRON, 1);
    e.equip_item(PLAYER_REF, BOW, true).unwrap();
    assert!(worn(&e, IRON));
    e.player_loose(2.0);
    assert_eq!(e.projectiles.len(), 1);
    assert_eq!(e.inventories[&PLAYER_REF].count(IRON), 0);
    assert_eq!(e.inventories[&PLAYER_REF].count(ELVEN), 10);
    // None worn now: nothing flies, and the game's "No arrows equipped."
    assert!(!worn(&e, IRON) && !worn(&e, ELVEN));
    e.player_loose(2.0);
    assert_eq!(e.projectiles.len(), 1);
    assert_eq!(e.inventories[&PLAYER_REF].count(ELVEN), 10);
    let said = e.scripts.notifications.last().map(|n| n.text.clone());
    assert_eq!(said.as_deref(), Some("No arrows equipped."));
}
