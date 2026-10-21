//! Death conditions (`GetTimeDead`, `IsKiller`, `IsKillerObject`) over deaths
//! recorded in play, on generated records only. `GetCauseofDeath` and
//! `GetKillingBlowLimb` stay unsupported.
use super::*;
use crate::world_state::Death;
use esp::writer::{Node, RecordBuilder, write_plugin};

/// The base both victims and the killer are made from.
const BASE: FormId = FormId(0x850);
const VICTIM: FormId = FormId(0x851);
const OTHER_VICTIM: FormId = FormId(0x852);
const KILLER: FormId = FormId(0x853);
/// A form list naming the base; another naming nothing.
const LIST: FormId = FormId(0x854);
const EMPTY_LIST: FormId = FormId(0x855);
const QUEST: FormId = FormId(0x856);

const GET_TIME_DEAD: u16 = 361;
const GET_CAUSE_OF_DEATH: u16 = 396;
const IS_KILLER: u16 = 408;
const IS_KILLER_OBJECT: u16 = 409;
const GET_KILLING_BLOW_LIMB: u16 = 495;

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
    let dir =
        Fixture(std::env::temp_dir().join(format!("vrm-death-{}-{stamp}", std::process::id())));
    std::fs::create_dir(&dir.0).unwrap();
    let mut acbs = [0; 24];
    acbs[8..10].copy_from_slice(&20u16.to_le_bytes());
    let actor = |id| RecordBuilder::new(b"ACHR", id).form(b"NAME", BASE);
    let records = [
        Node::top(
            b"NPC_",
            vec![RecordBuilder::new(b"NPC_", BASE).field(b"ACBS", &acbs)],
        ),
        Node::top(
            b"ACHR",
            vec![actor(VICTIM), actor(OTHER_VICTIM), actor(KILLER)],
        ),
        Node::top(
            b"FLST",
            vec![
                RecordBuilder::new(b"FLST", LIST).form(b"LNAM", BASE),
                RecordBuilder::new(b"FLST", EMPTY_LIST),
            ],
        ),
        Node::top(b"QUST", vec![RecordBuilder::new(b"QUST", QUEST)]),
    ];
    std::fs::write(
        dir.0.join("Death.esp"),
        write_plugin(0, &[], "test", "synthetic deaths", 0x900, &records),
    )
    .unwrap();
    let engine = crate::stats::action_tests::headless_engine(&dir.0, "Death.esp");
    (dir, engine)
}

/// The condition `func(p1) == value` as a CTDA record would hold it.
fn ctda(func: u16, p1: u32, value: f32) -> [u8; 32] {
    let mut d = [0; 32];
    d[4..8].copy_from_slice(&value.to_le_bytes());
    d[8..10].copy_from_slice(&func.to_le_bytes());
    d[12..16].copy_from_slice(&p1.to_le_bytes());
    d
}

/// Whether `func(p1) == value` holds for `subject` (run on the subject).
fn holds(e: &Engine, subject: FormId, func: u16, p1: u32, value: f32) -> bool {
    let rec = e.lo.get(QUEST).unwrap();
    let c = parse(&rec, &ctda(func, p1, value)).unwrap();
    evaluate(
        e,
        &[c],
        Context {
            subject: Some(subject),
            ..Default::default()
        },
    )
}

/// `VICTIM` died at game hour 10, killed by `KILLER`.
fn kill_victim(e: &mut Engine) {
    e.world_state.deaths.insert(
        VICTIM,
        Death {
            killer: Some(KILLER),
            at: 10.0,
        },
    );
}

#[test]
fn get_time_dead_is_the_game_hours_since_the_death() {
    let (_dir, mut e) = fixture();
    kill_victim(&mut e);
    // Day 1, 6 o'clock: 30 game hours in.
    e.day = 1;
    e.hour = 6.0;
    assert!(holds(&e, VICTIM, GET_TIME_DEAD, 0, 20.0));
    assert!(!holds(&e, VICTIM, GET_TIME_DEAD, 0, 30.0));
    // The living, and the dead before the game (no record), have no value.
    assert!(!holds(&e, KILLER, GET_TIME_DEAD, 0, 0.0));
    assert!(!holds(&e, OTHER_VICTIM, GET_TIME_DEAD, 0, 0.0));
}

#[test]
fn is_killer_asks_whether_the_subject_killed_the_actor_named() {
    let (_dir, mut e) = fixture();
    kill_victim(&mut e);
    assert!(holds(&e, KILLER, IS_KILLER, VICTIM.0, 1.0));
    // Not the other way round, nor for one still alive.
    assert!(!holds(&e, VICTIM, IS_KILLER, KILLER.0, 1.0));
    assert!(!holds(&e, KILLER, IS_KILLER, OTHER_VICTIM.0, 1.0));
    // A null actor names nobody.
    assert!(!holds(&e, KILLER, IS_KILLER, 0, 1.0));
}

#[test]
fn is_killer_object_asks_whether_the_subject_killed_a_listed_actor() {
    let (_dir, mut e) = fixture();
    kill_victim(&mut e);
    // The victim's base is on the list.
    assert!(holds(&e, KILLER, IS_KILLER_OBJECT, LIST.0, 1.0));
    assert!(!holds(&e, KILLER, IS_KILLER_OBJECT, EMPTY_LIST.0, 1.0));
    assert!(!holds(&e, VICTIM, IS_KILLER_OBJECT, LIST.0, 1.0));
    // Nothing killed by the other actor: not a killer of anything listed.
    assert!(!holds(&e, OTHER_VICTIM, IS_KILLER_OBJECT, LIST.0, 1.0));
}

#[test]
fn cause_of_death_and_killing_blow_limb_stay_unsupported() {
    let (_dir, mut e) = fixture();
    kill_victim(&mut e);
    // Neither the cause's meaning nor the blow's limb is known: no value.
    for value in [0.0, 1.0] {
        assert!(!holds(&e, VICTIM, GET_CAUSE_OF_DEATH, 0, value));
        assert!(!holds(&e, VICTIM, GET_KILLING_BLOW_LIMB, 0, value));
    }
}
