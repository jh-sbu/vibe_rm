//! Input-to-stamina regressions using generated records only. Engine owns a
//! renderer, so these action checks need a headless GPU adapter.

use super::*;
use esp::writer::{Node, RecordBuilder, write_plugin};

const PERK: FormId = FormId(0x880);
const SWORD: FormId = FormId(0x881);
const OTHER_SWORD: FormId = FormId(0x882);
const BOW: FormId = FormId(0x883);
const SHIELD: FormId = FormId(0x884);

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
        std::env::temp_dir().join(format!("vrm-player-bash-{}-{stamp}", std::process::id())),
    );
    std::fs::create_dir(&f.0).unwrap();
    let weapon = |id, anim| {
        let mut dnam = [0; 100];
        dnam[0] = anim;
        RecordBuilder::new(b"WEAP", id).field(b"DNAM", &dnam)
    };
    let condition = |func: u16, param: u32| {
        let mut d = [0; 32];
        d[4..8].copy_from_slice(&1.0f32.to_le_bytes());
        d[8..10].copy_from_slice(&func.to_le_bytes());
        d[12..16].copy_from_slice(&param.to_le_bytes());
        d
    };
    let mut biped = [0; 8];
    biped[..4].copy_from_slice(&(1u32 << 9).to_le_bytes());
    let records = [
        Node::top(b"PERK", vec![RecordBuilder::new(b"PERK", PERK)]),
        Node::top(
            b"IDLE",
            vec![
                RecordBuilder::new(b"IDLE", POWER_BASH_IDLE)
                    .field(b"CTDA", &condition(448, PERK.0))
                    .field(b"CTDA", &condition(72, 0x7)),
            ],
        ),
        Node::top(
            b"WEAP",
            vec![weapon(SWORD, 1), weapon(OTHER_SWORD, 1), weapon(BOW, 7)],
        ),
        Node::top(
            b"ARMO",
            vec![RecordBuilder::new(b"ARMO", SHIELD).field(b"BOD2", &biped)],
        ),
    ];
    std::fs::write(
        f.0.join("Bash.esp"),
        write_plugin(0, &[], "test", "synthetic bash actions", 0xF0000, &records),
    )
    .unwrap();
    let mut e = crate::stats::action_tests::headless_engine(&f.0, "Bash.esp");
    let attack = |event: &str, flags| Attack {
        event: event.into(),
        damage_mult: 1.0,
        chance: 1.0,
        flags,
        strike_angle: 35.0,
        stagger: 0.0,
        stamina_mult: 1.0,
        attack_type: FormId::NULL,
    };
    e.player_stats
        .set(Arc::new(CombatStats {
            attacks: vec![
                attack("testBash", ATK_BASH),
                attack("testPowerBash", ATK_BASH | ATK_POWER),
            ],
            ..Default::default()
        }))
        .unwrap();
    e.player_stamina = 200.0;
    equip(&mut e, SWORD);
    (f, e)
}

fn equip(e: &mut Engine, item: FormId) {
    let mut inv = Inventory::default();
    inv.add(item, 1);
    inv.equip(&e.lo, item);
    e.inventories.insert(PLAYER_REF, inv);
}

fn guard_and_press(e: &mut Engine) {
    e.player_attack_press(LEFT);
    assert!(e.player_blocking);
    e.player_attack_press(RIGHT);
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn bash_taps_and_holds_use_authored_idle_eligibility() {
    let (_f, mut e) = fixture();
    assert!(!e.player_can_power_bash());
    guard_and_press(&mut e);
    assert_eq!(e.player_stamina, 165.0); // no perk: immediate ordinary bash
    e.player_attack_release(RIGHT);
    assert_eq!(e.player_stamina, 165.0);

    assert!(e.add_perk(PLAYER_REF, PERK));
    assert!(e.player_can_power_bash()); // deliberately a different perk FormID
    e.player_attack_press(RIGHT);
    assert_eq!(e.player_stamina, 165.0);
    e.update_player_attack(0.1);
    e.player_attack_release(RIGHT);
    assert_eq!(e.player_stamina, 130.0); // early release: ordinary bash

    equip(&mut e, SHIELD); // shields work without a weapon too
    e.player_attack_press(RIGHT);
    e.update_player_attack(0.2);
    assert_eq!(e.player_stamina, 130.0);
    e.update_player_attack(0.2);
    assert_eq!(e.player_stamina, 75.0); // held: power-bash cost
    assert!(e.player_attack_held.is_none());
    e.update_player_attack(1.0);
    e.player_attack_release(RIGHT);
    assert_eq!(e.player_stamina, 75.0); // consumed once, release does not swing
    assert!(e.player_swing.is_none());

    equip(&mut e, BOW);
    assert!(!e.player_can_power_bash());
    e.player_attack_press(RIGHT);
    assert_eq!(e.player_stamina, 40.0); // bow remains an ordinary bash
    assert!(e.player_attack_held.is_none());
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn pending_bashes_cancel_when_guard_eligibility_or_equipment_is_lost() {
    let (_f, mut e) = fixture();
    e.add_perk(PLAYER_REF, PERK);
    guard_and_press(&mut e);
    e.player_attack_release(LEFT);
    e.player_attack_release(RIGHT);
    assert!(e.player_attack_held.is_none());
    assert_eq!(e.player_stamina, 200.0);

    guard_and_press(&mut e);
    equip(&mut e, OTHER_SWORD);
    e.player_attack_release(RIGHT); // no intervening simulation update
    assert_eq!(e.player_stamina, 200.0);

    e.player_attack_press(RIGHT);
    e.remove_perk(PLAYER_REF, PERK);
    e.player_attack_release(RIGHT); // removal immediately before release
    assert_eq!(e.player_stamina, 200.0);

    e.add_perk(PLAYER_REF, PERK);
    e.player_attack_press(RIGHT);
    e.disabled_controls.fighting = true;
    e.update_player_attack(1.0);
    assert!(e.player_attack_held.is_none());
    assert_eq!(e.player_stamina, 200.0);
    e.disabled_controls.fighting = false;

    e.player_attack_press(RIGHT);
    equip(&mut e, BOW);
    e.update_player_attack(1.0);
    assert!(e.player_attack_held.is_none());
    assert!(e.player_swing.is_none());
    assert_eq!(e.player_stamina, 200.0);
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn bash_holds_do_not_advance_while_paused_and_respect_stamina() {
    let (_f, mut e) = fixture();
    e.add_perk(PLAYER_REF, PERK);
    guard_and_press(&mut e);
    e.update_player_attack(0.0);
    e.update_player_attack(-1.0);
    assert_eq!(e.player_attack_held.unwrap().held, 0.0);
    e.player_stamina = 40.0;
    e.update_player_attack(1.0);
    assert_eq!(e.player_stamina, 5.0); // ordinary fallback when power is too dear
    e.player_attack_press(RIGHT);
    e.update_player_attack(1.0);
    assert_eq!(e.player_stamina, 5.0); // cannot afford either kind
    assert!(e.player_attack_held.is_none());
}
