//! Crime condition functions over the crime state of a synthetic engine; no
//! game assets.
use super::*;
use esp::writer::{Node, RecordBuilder, write_plugin};

const QUEST: FormId = FormId(0x830);
const FACTION: FormId = FormId(0x840);
const OTHER_FACTION: FormId = FormId(0x841);
const NPC: FormId = FormId(0x850);
const BYSTANDER: FormId = FormId(0x851);

const GET_CRIME_GOLD: u16 = 459;
const GET_CRIME_GOLD_VIOLENT: u16 = 375;
const GET_INFAMY: u16 = 533;
const GET_INFAMY_VIOLENT: u16 = 534;
const GET_INFAMY_NON_VIOLENT: u16 = 535;

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
    let dir = Fixture(
        std::env::temp_dir().join(format!("vrm-crime-cond-{}-{stamp}", std::process::id())),
    );
    std::fs::create_dir(&dir.0).unwrap();
    let records = [Node::top(b"QUST", vec![RecordBuilder::new(b"QUST", QUEST)])];
    std::fs::write(
        dir.0.join("Crime.esp"),
        write_plugin(0, &[], "test", "synthetic crime state", 0x900, &records),
    )
    .unwrap();
    let engine = crate::stats::action_tests::headless_engine(&dir.0, "Crime.esp");
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
fn holds(e: &Engine, subject: FormId, func: u16, p1: FormId, value: f32) -> bool {
    let rec = e.lo.get(QUEST).unwrap();
    let c = parse(&rec, &ctda(func, p1.0, value)).unwrap();
    evaluate(
        e,
        &[c],
        Context {
            subject: Some(subject),
            ..Default::default()
        },
    )
}

#[test]
fn infamy_is_what_crime_gold_rose_by_and_paying_it_off_leaves_it() {
    let (_dir, mut e) = fixture();
    e.mod_crime_gold(FACTION, 100, true);
    e.mod_crime_gold(FACTION, 50, false);
    e.mod_crime_gold(FACTION, -120, true);
    // The violent gold is paid off, the infamy stays.
    assert!(holds(&e, PLAYER_REF, GET_CRIME_GOLD_VIOLENT, FACTION, 0.0));
    assert!(holds(&e, PLAYER_REF, GET_INFAMY_VIOLENT, FACTION, 100.0));
    assert!(holds(&e, PLAYER_REF, GET_INFAMY_NON_VIOLENT, FACTION, 50.0));
    assert!(holds(&e, PLAYER_REF, GET_INFAMY, FACTION, 150.0));
    assert!(!holds(&e, PLAYER_REF, GET_INFAMY, OTHER_FACTION, 150.0));
}

#[test]
fn infamy_of_the_player_without_a_faction_sums_every_faction() {
    let (_dir, mut e) = fixture();
    e.mod_crime_gold(FACTION, 100, true);
    e.mod_crime_gold(OTHER_FACTION, 30, false);
    assert!(holds(&e, PLAYER_REF, GET_INFAMY, FormId::NULL, 130.0));
    assert!(holds(
        &e,
        PLAYER_REF,
        GET_INFAMY_VIOLENT,
        FormId::NULL,
        100.0
    ));
    assert!(holds(
        &e,
        PLAYER_REF,
        GET_INFAMY_NON_VIOLENT,
        FormId::NULL,
        30.0
    ));
}

#[test]
fn infamy_of_an_actor_without_a_faction_is_its_crime_faction_or_nothing() {
    let (_dir, mut e) = fixture();
    e.mod_crime_gold(FACTION, 40, true);
    e.set_crime_faction(NPC, Some(FACTION));
    assert!(holds(&e, NPC, GET_INFAMY_VIOLENT, FormId::NULL, 40.0));
    assert!(holds(&e, BYSTANDER, GET_INFAMY, FormId::NULL, 0.0));
}

#[test]
fn crime_functions_without_crime_state_are_unsupported() {
    let (_dir, e) = fixture();
    // Unsupported conditions fail whatever value they are compared with.
    for func in [122, 176, 193, 195, 197, 199, 458] {
        assert!(
            !holds(&e, PLAYER_REF, func, FACTION, 0.0),
            "condition {func} should be unsupported"
        );
    }
}
