//! Casting-state conditions (`HasEquippedSpell`, `GetCurrentCastingType`,
//! `GetCurrentDeliveryType`, `IsCurrentSpell`, `SpellHasKeyword`) on the
//! player's hands, generated records only. Engine owns a renderer, so these
//! need a headless GPU adapter.

use super::*;
use crate::condition::{self, Context};
use crate::magic::archetype;
use crate::spells::{CONCENTRATION, LEFT, RIGHT, VOICE};
use esp::actor_value as av;
use esp::writer::{Node, RecordBuilder, write_plugin};

/// Fire-and-forget, aimed.
const FIRE: FormId = FormId(0x810);
/// Concentration, self; carries its own keyword as well as its effect's.
const CONC: FormId = FormId(0x811);
const EFFECT: FormId = FormId(0x800);
const FIRE_KW: FormId = FormId(0x820);
const OWN_KW: FormId = FormId(0x821);

/// Casting sources (`ptCastingSource`).
const SRC_LEFT: u32 = 0;
const SRC_RIGHT: u32 = 1;
const SRC_VOICE: u32 = 2;

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
        std::env::temp_dir().join(format!("vrm-casting-cond-{}-{stamp}", std::process::id())),
    );
    std::fs::create_dir(&f.0).unwrap();
    let mut mgef = [0; 0x6c];
    mgef[0x10..0x14].copy_from_slice(&u32::MAX.to_le_bytes());
    mgef[0x40..0x44].copy_from_slice(&archetype::VALUE_MODIFIER.to_le_bytes());
    mgef[0x44..0x48].copy_from_slice(&av::SNEAK.to_le_bytes());
    let effect = RecordBuilder::new(b"MGEF", EFFECT)
        .field(b"DATA", &mgef)
        .field(b"KWDA", &FIRE_KW.0.to_le_bytes());
    let spell = |id, cast_type: u32, delivery: u32, kws: &[FormId]| {
        let mut spit = [0; 36];
        spit[0..4].copy_from_slice(&10u32.to_le_bytes()); // an authored cost
        spit[4..8].copy_from_slice(&1u32.to_le_bytes());
        spit[12..16].copy_from_slice(&0.5f32.to_le_bytes()); // charge time
        spit[16..20].copy_from_slice(&cast_type.to_le_bytes());
        spit[20..24].copy_from_slice(&delivery.to_le_bytes());
        let mut efit = [0; 12];
        efit[0..4].copy_from_slice(&7.0f32.to_le_bytes());
        efit[8..12].copy_from_slice(&30u32.to_le_bytes());
        let kwda: Vec<u8> = kws.iter().flat_map(|k| k.0.to_le_bytes()).collect();
        let mut r = RecordBuilder::new(b"SPEL", id)
            .zstring(b"FULL", "Test Spell")
            .field(b"SPIT", &spit)
            .u32(b"EFID", EFFECT.0)
            .field(b"EFIT", &efit);
        if !kwda.is_empty() {
            r = r.field(b"KWDA", &kwda);
        }
        r
    };
    let records = [
        Node::top(b"MGEF", vec![effect]),
        Node::top(
            b"SPEL",
            vec![
                spell(FIRE, 1, 2, &[]),
                spell(CONC, CONCENTRATION, 0, &[OWN_KW]),
            ],
        ),
    ];
    std::fs::write(
        f.0.join("Casting.esp"),
        write_plugin(0, &[], "test", "synthetic casting", 0x900, &records),
    )
    .unwrap();
    let mut e = crate::stats::action_tests::headless_engine(&f.0, "Casting.esp");
    e.set_actor_value(PLAYER_REF, av::MAGICKA, 100.0);
    (f, e)
}

/// `func(p1, p2) == value` on the player, parsed from its CTDA bytes.
fn check(e: &Engine, func: u16, p1: u32, p2: u32, value: f32) -> bool {
    let mut data = [0; 32];
    data[4..8].copy_from_slice(&value.to_le_bytes());
    data[8..10].copy_from_slice(&func.to_le_bytes());
    data[12..16].copy_from_slice(&p1.to_le_bytes());
    data[16..20].copy_from_slice(&p2.to_le_bytes());
    let record = e.lo.get(FIRE).unwrap();
    let c = condition::parse(&record, &data).unwrap();
    condition::evaluate(
        e,
        &[c],
        Context {
            subject: Some(PLAYER_REF),
            ..Default::default()
        },
    )
}

/// Ready a spell in a hand, holding its button for `held` seconds.
fn press_and_hold(e: &mut Engine, spell: FormId, hand: usize, held: f32) {
    e.add_spell(PLAYER_REF, spell);
    e.equip_spell(PLAYER_REF, spell, hand);
    assert!(e.player_cast_press(hand));
    e.update_casting(held);
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn has_equipped_spell_reads_the_hands_and_the_voice() {
    let (_f, mut e) = fixture();
    assert!(check(&e, 570, SRC_LEFT, 0, 0.0));
    assert!(!check(&e, 570, SRC_LEFT, 0, 1.0));
    e.add_spell(PLAYER_REF, FIRE);
    e.equip_spell(PLAYER_REF, FIRE, LEFT);
    assert!(check(&e, 570, SRC_LEFT, 0, 1.0));
    assert!(!check(&e, 570, SRC_RIGHT, 0, 1.0));
    assert!(check(&e, 570, SRC_RIGHT, 0, 0.0));
    // A power in the voice slot.
    e.hold_magic(PLAYER_REF, CONC, &[VOICE]);
    assert!(check(&e, 570, SRC_VOICE, 0, 1.0));
    // A source that isn't one: unsupported, so the condition fails.
    assert!(!check(&e, 570, 7, 0, 0.0));
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn casting_type_and_delivery_follow_the_spell_in_the_hand() {
    let (_f, mut e) = fixture();
    // Nothing cast from either hand: -1.
    assert!(check(&e, 571, SRC_LEFT, 0, -1.0));
    assert!(check(&e, 572, SRC_RIGHT, 0, -1.0));

    press_and_hold(&mut e, FIRE, LEFT, 0.1);
    assert!(check(&e, 571, SRC_LEFT, 0, 1.0)); // fire and forget
    assert!(check(&e, 572, SRC_LEFT, 0, 2.0)); // aimed
    assert!(check(&e, 571, SRC_RIGHT, 0, -1.0));

    press_and_hold(&mut e, CONC, RIGHT, 0.1);
    assert!(check(&e, 571, SRC_RIGHT, 0, CONCENTRATION as f32));
    assert!(check(&e, 572, SRC_RIGHT, 0, 0.0)); // self
    // The left hand's spell is still its own.
    assert!(check(&e, 571, SRC_LEFT, 0, 1.0));

    // Let go before it charged: the hand casts nothing again.
    assert!(e.player_cast_release(LEFT));
    assert!(check(&e, 571, SRC_LEFT, 0, -1.0));
    assert!(check(&e, 572, SRC_LEFT, 0, -1.0));

    // The voice has no casting state: unsupported.
    assert!(!check(&e, 571, SRC_VOICE, 0, -1.0));
    assert!(!check(&e, 572, SRC_VOICE, 0, -1.0));
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn is_current_spell_compares_the_spell_in_the_source() {
    let (_f, mut e) = fixture();
    assert!(!check(&e, 595, FIRE.0, SRC_LEFT, 1.0));
    press_and_hold(&mut e, FIRE, LEFT, 0.1);
    assert!(check(&e, 595, FIRE.0, SRC_LEFT, 1.0));
    assert!(!check(&e, 595, CONC.0, SRC_LEFT, 1.0));
    assert!(!check(&e, 595, FIRE.0, SRC_RIGHT, 1.0));
    assert!(!check(&e, 595, FIRE.0, SRC_LEFT, 0.0));
    // The voice has no casting state: unsupported.
    assert!(!check(&e, 595, FIRE.0, SRC_VOICE, 0.0));
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn spell_has_keyword_checks_the_casting_spell_and_its_effects() {
    let (_f, mut e) = fixture();
    press_and_hold(&mut e, FIRE, LEFT, 0.1);
    press_and_hold(&mut e, CONC, RIGHT, 0.1);
    // A flames spell has no keyword of its own; its effect carries it.
    assert!(check(&e, 596, SRC_LEFT, FIRE_KW.0, 1.0));
    assert!(!check(&e, 596, SRC_LEFT, OWN_KW.0, 1.0));
    // The concentration spell has its own keyword, and its effect's.
    assert!(check(&e, 596, SRC_RIGHT, OWN_KW.0, 1.0));
    assert!(check(&e, 596, SRC_RIGHT, FIRE_KW.0, 1.0));
    // Nothing cast from the voice: unsupported.
    assert!(!check(&e, 596, SRC_VOICE, FIRE_KW.0, 1.0));
}
