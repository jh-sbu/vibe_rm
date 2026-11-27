//! Creature diseases delivered by blows (`Engine::deliver_attack_spell`),
//! generated records only: a Bone Break Fever look-alike (a disease, a
//! constant contact spell whose effect lowers max Stamina by 25) with its
//! roll as a condition. Engine owns a renderer, so these need a headless
//! GPU adapter.

use super::*;
use crate::condition::{self, Context};
use esp::actor_value as av;
use esp::writer::{Node, RecordBuilder, write_plugin};

/// The disease whose roll always passes, and one whose roll never does.
const CAUGHT: FormId = FormId(0x810);
const MISSED: FormId = FormId(0x811);
const ALWAYS: FormId = FormId(0x800);
const NEVER: FormId = FormId(0x801);
const BEAR: FormId = FormId(0x0FF0_0001);

struct Fixture(std::path::PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `GetRandomPercent <= value` (function 77, operator 5).
fn roll(value: f32) -> [u8; 32] {
    let mut c = [0; 32];
    c[0] = 5 << 5;
    c[4..8].copy_from_slice(&value.to_le_bytes());
    c[8..10].copy_from_slice(&77u16.to_le_bytes());
    c
}

fn fixture() -> (Fixture, Engine) {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let f =
        Fixture(std::env::temp_dir().join(format!("vrm-disease-{}-{stamp}", std::process::id())));
    std::fs::create_dir(&f.0).unwrap();
    // As `DisDamageStamina`: Recover | Detrimental, a peak value modifier on
    // Stamina, constant, contact.
    let effect = |id: FormId, condition: [u8; 32]| {
        let mut data = [0; 0x98];
        data[0..4].copy_from_slice(&0x6u32.to_le_bytes());
        data[0x0c..0x10].copy_from_slice(&u32::MAX.to_le_bytes());
        data[0x10..0x14].copy_from_slice(&u32::MAX.to_le_bytes());
        data[0x40..0x44].copy_from_slice(&archetype::PEAK_VALUE_MODIFIER.to_le_bytes());
        data[0x44..0x48].copy_from_slice(&av::STAMINA.to_le_bytes());
        data[0x50..0x54].copy_from_slice(&0u32.to_le_bytes());
        data[0x54..0x58].copy_from_slice(&1u32.to_le_bytes());
        data[0x58..0x5c].copy_from_slice(&u32::MAX.to_le_bytes());
        RecordBuilder::new(b"MGEF", id)
            .field(b"DATA", &data)
            .field(b"CTDA", &condition)
    };
    let disease = |id: FormId, effect: FormId| {
        let mut spit = [0; 36];
        spit[8..12].copy_from_slice(&1u32.to_le_bytes()); // a disease
        spit[20..24].copy_from_slice(&1u32.to_le_bytes()); // contact
        let mut efit = [0; 12];
        efit[0..4].copy_from_slice(&25.0f32.to_le_bytes());
        RecordBuilder::new(b"SPEL", id)
            .zstring(b"FULL", "Test Fever")
            .field(b"SPIT", &spit)
            .u32(b"EFID", effect.0)
            .field(b"EFIT", &efit)
    };
    let records = [
        Node::top(
            b"MGEF",
            vec![effect(ALWAYS, roll(100.0)), effect(NEVER, roll(-1.0))],
        ),
        Node::top(
            b"SPEL",
            vec![disease(CAUGHT, ALWAYS), disease(MISSED, NEVER)],
        ),
    ];
    std::fs::write(
        f.0.join("Disease.esp"),
        write_plugin(0, &[], "test", "synthetic diseases", 0x900, &records),
    )
    .unwrap();
    let e = crate::stats::action_tests::headless_engine(&f.0, "Disease.esp");
    (f, e)
}

/// `GetDisease == 1` on the player.
fn get_disease(e: &Engine) -> bool {
    let mut data = [0; 32];
    data[4..8].copy_from_slice(&1.0f32.to_le_bytes());
    data[8..10].copy_from_slice(&39u16.to_le_bytes());
    let record = e.lo.get(CAUGHT).unwrap();
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

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn a_disease_caught_from_a_blow_stays_until_cured_and_counts_for_get_disease() {
    let (_f, mut e) = fixture();
    let stamina = e.av_max(PLAYER_REF, av::STAMINA);
    e.deliver_attack_spell(CAUGHT, BEAR, PLAYER_REF);
    assert!(get_disease(&e), "caught");
    assert!(
        !e.has_spell(PLAYER_REF, CAUGHT),
        "not a spell the player has"
    );
    // The ability sync and the condition checks leave it on: its roll was
    // made as it landed.
    for _ in 0..5 {
        e.update_magic(1.0);
    }
    assert!(get_disease(&e), "kept");
    assert_eq!(e.av_max(PLAYER_REF, av::STAMINA), stamina - 25.0);
    // `RemoveSpell` cures it, as in the game (autotest28).
    e.remove_spell(PLAYER_REF, CAUGHT);
    e.update_magic(1.0);
    assert!(!get_disease(&e), "cured");
    assert_eq!(e.av_max(PLAYER_REF, av::STAMINA), stamina);
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn a_failed_roll_gives_nothing_and_an_added_disease_isnt_caught() {
    let (_f, mut e) = fixture();
    e.deliver_attack_spell(MISSED, BEAR, PLAYER_REF);
    e.update_magic(1.0);
    assert!(!e.magic.effects.iter().any(|x| x.item == MISSED && !x.done));
    // Added as a spell (Skyrim SE: `GetDisease` 0, autotest22 and 23).
    e.add_spell(PLAYER_REF, CAUGHT);
    e.update_magic(1.0);
    assert!(e.has_spell(PLAYER_REF, CAUGHT));
    assert!(!get_disease(&e));
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn full_disease_resistance_keeps_it_off() {
    let (_f, mut e) = fixture();
    e.set_actor_value(PLAYER_REF, av::DISEASE_RESIST, 100.0);
    for _ in 0..20 {
        e.deliver_attack_spell(CAUGHT, BEAR, PLAYER_REF);
    }
    assert!(!get_disease(&e));
}
