//! Actor and reference natives, using generated records only. Engine owns a
//! renderer, so these need a headless GPU adapter.

use std::sync::Arc;

use esp::FormId;
use esp::writer::{Node, RecordBuilder, write_plugin};
use papyrus::{NativeResult, Value};

use crate::ai::ActorRuntime;
use crate::engine::Engine;

const RACE: FormId = FormId(0x800);
const NPC: FormId = FormId(0x801);
const ACTOR: FormId = FormId(0x802);
/// An NPC taking its traits (race) from `NPC` (template flag 0x1).
const TEMPLATED: FormId = FormId(0x803);
const TEMPLATED_REF: FormId = FormId(0x804);
/// Markers linked in a chain: `FIRST` -> `SECOND` -> `THIRD`.
const STATIC: FormId = FormId(0x805);
const FIRST: FormId = FormId(0x806);
const SECOND: FormId = FormId(0x807);
const THIRD: FormId = FormId(0x808);

struct Fixture(std::path::PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture(name: &str) -> (Fixture, Engine) {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let f = Fixture(std::env::temp_dir().join(format!(
        "vrm-actor-natives-{name}-{}-{stamp}",
        std::process::id()
    )));
    std::fs::create_dir(&f.0).unwrap();
    let acbs = [0u8; 24];
    let mut templated = acbs;
    templated[16..18].copy_from_slice(&1u16.to_le_bytes());
    let records = [
        Node::top(b"RACE", vec![RecordBuilder::new(b"RACE", RACE)]),
        Node::top(
            b"NPC_",
            vec![
                RecordBuilder::new(b"NPC_", NPC)
                    .field(b"ACBS", &acbs)
                    .form(b"RNAM", RACE),
                RecordBuilder::new(b"NPC_", TEMPLATED)
                    .field(b"ACBS", &templated)
                    .form(b"TPLT", NPC),
            ],
        ),
        Node::top(
            b"ACHR",
            vec![
                RecordBuilder::new(b"ACHR", ACTOR).form(b"NAME", NPC),
                RecordBuilder::new(b"ACHR", TEMPLATED_REF).form(b"NAME", TEMPLATED),
            ],
        ),
        Node::top(b"STAT", vec![RecordBuilder::new(b"STAT", STATIC)]),
        Node::top(
            b"REFR",
            vec![
                RecordBuilder::new(b"REFR", FIRST)
                    .form(b"NAME", STATIC)
                    .form(b"XLKR", SECOND),
                RecordBuilder::new(b"REFR", SECOND)
                    .form(b"NAME", STATIC)
                    .form(b"XLKR", THIRD),
                RecordBuilder::new(b"REFR", THIRD).form(b"NAME", STATIC),
            ],
        ),
    ];
    std::fs::write(
        f.0.join("ActorNatives.esp"),
        write_plugin(0, &[], "test", "synthetic actor natives", 0x900, &records),
    )
    .unwrap();
    let e = crate::stats::action_tests::headless_engine(&f.0, "ActorNatives.esp");
    (f, e)
}

/// Put an actor in a loaded interior.
fn place(e: &mut Engine, r: FormId, base: FormId) {
    let key = crate::render::CellKey::Interior(FormId(0x820));
    let a = ActorRuntime::new(
        r,
        base,
        Arc::new(crate::world::skeleton::Skeleton::default()),
        glam::Mat4::IDENTITY,
        Vec::new(),
        0.0,
    );
    e.actor_cells.insert(r, key);
    e.cells.entry(key).or_default().actors.push(a);
}

fn call(e: &mut Engine, class: &str, func: &str, on: FormId, args: &[Value]) -> Value {
    let this = e.object_value(on);
    match crate::script::natives::call(e, class, func, Some(&this), args) {
        NativeResult::Value(v) => v,
        _ => panic!("{class}.{func} didn't return"),
    }
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn get_race_reads_the_race_through_templates() {
    let (_f, mut e) = fixture("race");
    let race = e.object_value(RACE);
    assert_eq!(call(&mut e, "actor", "getrace", ACTOR, &[]), race);
    assert_eq!(call(&mut e, "actorbase", "getrace", NPC, &[]), race);
    assert_eq!(call(&mut e, "actor", "getrace", TEMPLATED_REF, &[]), race);
    assert_eq!(call(&mut e, "actorbase", "getrace", RACE, &[]), Value::None);
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn get_nth_linked_ref_follows_the_chain() {
    let (_f, mut e) = fixture("linked");
    for (n, want) in [
        (0, Some(FIRST)),
        (1, Some(SECOND)),
        (2, Some(THIRD)),
        (3, None),
    ] {
        let want = want.map_or(Value::None, |r| e.object_value(r));
        assert_eq!(
            call(
                &mut e,
                "objectreference",
                "getnthlinkedref",
                FIRST,
                &[Value::Int(n)]
            ),
            want,
            "{n}"
        );
    }
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn stop_combat_alarm_ends_fights_and_pursuits_of_the_actor() {
    use crate::ai::combat::Combat;
    use crate::engine::PLAYER_REF;
    const ATTACKER: FormId = FormId(0x901);
    const BRAWLER: FormId = FormId(0x902);
    // A reference with a record, so scripts can name it.
    const VICTIM: FormId = ACTOR;
    const GUARD: FormId = FormId(0x904);
    let (_f, mut e) = fixture("alarm");
    for r in [ATTACKER, BRAWLER, VICTIM] {
        place(&mut e, r, NPC);
    }
    e.actor_mut(ATTACKER).unwrap().combat = Some(Combat::new(PLAYER_REF));
    e.actor_mut(BRAWLER).unwrap().combat = Some(Combat::new(VICTIM));
    e.crime.arrests.alarmed.insert(
        GUARD,
        crate::arrest::Pursuit {
            faction: FormId(0x905),
            bark_in: 0.0,
        },
    );
    call(&mut e, "actor", "stopcombatalarm", PLAYER_REF, &[]);
    assert!(e.actor_ref(ATTACKER).unwrap().combat.is_none());
    assert!(e.crime.arrests.alarmed.is_empty(), "pursuers give up");
    assert!(
        e.actor_ref(BRAWLER).unwrap().combat.is_some(),
        "not the player's"
    );
    call(&mut e, "actor", "stopcombatalarm", VICTIM, &[]);
    assert!(e.actor_ref(BRAWLER).unwrap().combat.is_none());
}
