//! Speech checks on generated records only. Engine owns a renderer, so these
//! need a headless GPU adapter.

use esp::FormId;
use esp::actor_value as av;
use esp::writer::{Node, RecordBuilder, write_plugin};
use papyrus::{NativeResult, Value};

use crate::engine::{Engine, PLAYER_REF};

/// A race that allows dialogue with the player, one that doesn't, and an
/// NPC (level 10) of each.
const TALKING: FormId = FormId(0x800);
const MUTE: FormId = FormId(0x801);
const NPC: FormId = FormId(0x802);
const ACTOR: FormId = FormId(0x803);
const BEAST: FormId = FormId(0x804);
const BEAST_REF: FormId = FormId(0x805);

struct Fixture(std::path::PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn race_data(flags: u32) -> Vec<u8> {
    let mut d = vec![0u8; 36];
    d[32..36].copy_from_slice(&flags.to_le_bytes());
    d
}

fn fixture(name: &str) -> (Fixture, Engine) {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let f = Fixture(std::env::temp_dir().join(format!(
        "vrm-speech-checks-{name}-{}-{stamp}",
        std::process::id()
    )));
    std::fs::create_dir(&f.0).unwrap();
    let mut acbs = [0u8; 24];
    acbs[8..10].copy_from_slice(&10u16.to_le_bytes());
    let records = [
        Node::top(
            b"RACE",
            vec![
                RecordBuilder::new(b"RACE", TALKING).field(b"DATA", &race_data(0x0020_0000)),
                RecordBuilder::new(b"RACE", MUTE).field(b"DATA", &race_data(0)),
            ],
        ),
        Node::top(
            b"NPC_",
            vec![
                RecordBuilder::new(b"NPC_", NPC)
                    .field(b"ACBS", &acbs)
                    .form(b"RNAM", TALKING),
                RecordBuilder::new(b"NPC_", BEAST)
                    .field(b"ACBS", &acbs)
                    .form(b"RNAM", MUTE),
            ],
        ),
        Node::top(
            b"ACHR",
            vec![
                RecordBuilder::new(b"ACHR", ACTOR).form(b"NAME", NPC),
                RecordBuilder::new(b"ACHR", BEAST_REF).form(b"NAME", BEAST),
            ],
        ),
    ];
    std::fs::write(
        f.0.join("SpeechChecks.esp"),
        write_plugin(0, &[], "test", "synthetic speech checks", 0x900, &records),
    )
    .unwrap();
    let mut e = crate::stats::action_tests::headless_engine(&f.0, "SpeechChecks.esp");
    e.skills.level = 5;
    e.set_actor_value(PLAYER_REF, av::SPEECHCRAFT, 50.0);
    e.set_actor_value(ACTOR, av::SPEECHCRAFT, 20.0);
    (f, e)
}

fn call(e: &mut Engine, func: &str, on: FormId, args: &[Value]) -> Value {
    let this = e.object_value(on);
    match crate::script::natives::call(e, "actor", func, Some(&this), args) {
        NativeResult::Value(v) => v,
        _ => panic!("actor.{func} didn't return"),
    }
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn a_bribe_costs_by_levels_speech_and_morality() {
    let (_f, mut e) = fixture("bribe");
    // (10 + 5 - (30 - 100) * 0.1) ^ 1.4 * 0.5 * (0 * 1 + 1).
    let expected = (22f32.powf(1.4) * 0.5).round() as i32;
    assert_eq!(e.bribe_amount(ACTOR), expected);
    assert_eq!(
        call(&mut e, "getbribeamount", ACTOR, &[]),
        Value::Int(expected)
    );
    // A more moral NPC asks more.
    e.set_actor_value(ACTOR, 3, 2.0);
    assert_eq!(
        e.bribe_amount(ACTOR),
        (22f32.powf(1.4) * 0.5 * 3.0).round() as i32
    );
    assert_eq!(
        e.dialogue_text(
            "Take this. (<BribeCost> gold) <CrimeGold>",
            ACTOR,
            FormId::NULL
        ),
        format!("Take this. ({} gold) 0", e.bribe_amount(ACTOR))
    );
    assert!(!e.bribe_succeeds(ACTOR));
    e.add_item(PLAYER_REF, crate::crime::GOLD, e.bribe_amount(ACTOR));
    assert!(e.bribe_succeeds(ACTOR));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn intimidation_weighs_the_players_level_and_speech_against_the_npcs_level() {
    let (_f, mut e) = fixture("intimidate");
    // 5 * (1 + 0.3) against 10.
    assert_eq!(
        call(&mut e, "willintimidatesucceed", ACTOR, &[]),
        Value::Bool(false)
    );
    e.skills.level = 8;
    // 8 * 1.3 = 10.4.
    assert!(e.intimidate_succeeds(ACTOR));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn bribed_and_intimidated_flags_set_and_clear() {
    let (_f, mut e) = fixture("flags");
    assert_eq!(call(&mut e, "isbribed", ACTOR, &[]), Value::Bool(false));
    call(&mut e, "setbribed", ACTOR, &[]);
    assert_eq!(call(&mut e, "isbribed", ACTOR, &[]), Value::Bool(true));
    call(&mut e, "setintimidated", ACTOR, &[Value::Bool(true)]);
    assert_eq!(call(&mut e, "isintimidated", ACTOR, &[]), Value::Bool(true));
    call(&mut e, "setbribed", ACTOR, &[Value::Bool(false)]);
    assert_eq!(call(&mut e, "isbribed", ACTOR, &[]), Value::Bool(false));
    assert_eq!(call(&mut e, "isintimidated", ACTOR, &[]), Value::Bool(true));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn the_race_allows_dialogue_unless_a_script_overrides_it() {
    let (_f, mut e) = fixture("talk");
    assert!(e.can_talk_to(ACTOR));
    assert!(!e.can_talk_to(BEAST_REF));
    call(&mut e, "allowpcdialogue", BEAST_REF, &[Value::Bool(true)]);
    call(&mut e, "allowpcdialogue", ACTOR, &[Value::Bool(false)]);
    assert!(e.can_talk_to(BEAST_REF));
    assert!(!e.can_talk_to(ACTOR));
}
