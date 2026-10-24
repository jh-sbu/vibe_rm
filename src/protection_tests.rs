//! Essential and protected actors, using generated records only. Engine owns
//! a renderer, so these need a headless GPU adapter.

use std::sync::Arc;

use esp::FormId;
use esp::writer::{Node, RecordBuilder, write_plugin};

use crate::ai::ActorRuntime;
use crate::ai::combat::CombatStats;
use crate::engine::{Engine, PLAYER_REF};

const NPC: FormId = FormId(0x800);
const ACTOR: FormId = FormId(0x801);
const OTHER: FormId = FormId(0x802);
const BYSTANDER: FormId = FormId(0x803);
const QUEST: FormId = FormId(0x810);

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
        "vrm-protection-{name}-{}-{stamp}",
        std::process::id()
    )));
    std::fs::create_dir(&f.0).unwrap();
    let acbs = [0u8; 24];
    let records = [
        Node::top(
            b"NPC_",
            vec![RecordBuilder::new(b"NPC_", NPC).field(b"ACBS", &acbs)],
        ),
        Node::top(
            b"ACHR",
            vec![
                RecordBuilder::new(b"ACHR", ACTOR).form(b"NAME", NPC),
                RecordBuilder::new(b"ACHR", OTHER).form(b"NAME", NPC),
                RecordBuilder::new(b"ACHR", BYSTANDER).form(b"NAME", NPC),
            ],
        ),
        Node::top(
            b"QUST",
            vec![
                RecordBuilder::new(b"QUST", QUEST)
                    .u32(b"ALST", 0)
                    .u32(b"FNAM", crate::aliases::flags::ESSENTIAL)
                    .form(b"ALFR", ACTOR)
                    .field(b"ALED", &[])
                    .u32(b"ALST", 1)
                    .u32(b"FNAM", crate::aliases::flags::PROTECTED)
                    .form(b"ALFR", OTHER)
                    .field(b"ALED", &[]),
            ],
        ),
    ];
    std::fs::write(
        f.0.join("Protection.esp"),
        write_plugin(0, &[], "test", "synthetic protection", 0x900, &records),
    )
    .unwrap();
    let mut e = crate::stats::action_tests::headless_engine(&f.0, "Protection.esp");
    for r in [ACTOR, OTHER, BYSTANDER] {
        place(&mut e, r);
    }
    (f, e)
}

/// Put an actor in a loaded interior, near the player.
fn place(e: &mut Engine, r: FormId) {
    let key = crate::render::CellKey::Interior(FormId(0x820));
    let mut a = ActorRuntime::new(
        r,
        NPC,
        Arc::new(crate::world::skeleton::Skeleton::default()),
        glam::Mat4::from_translation(glam::Vec3::X * 100.0),
        Vec::new(),
        0.0,
    );
    a.stats = Arc::new(CombatStats {
        max_health: 50.0,
        ..CombatStats::default()
    });
    a.health = 50.0;
    e.actor_cells.insert(r, key);
    e.cells.entry(key).or_default().actors.push(a);
}

fn quest(e: &mut Engine, func: &str) {
    let q = e.object_value(QUEST);
    crate::script::natives::call(e, "quest", func, Some(&q), &[]);
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn alias_flags_protect_their_references_while_the_quest_runs() {
    let (_f, mut e) = fixture("alias");
    assert!(!e.is_essential(ACTOR));
    assert!(!e.is_protected(OTHER));
    quest(&mut e, "start");
    assert!(e.is_essential(ACTOR));
    assert!(!e.is_protected(ACTOR));
    assert!(e.is_protected(OTHER));
    assert!(!e.is_essential(OTHER));
    assert!(!e.is_essential(BYSTANDER) && !e.is_protected(BYSTANDER));
    // The essential bleed out, from anyone; the protected but to the player.
    // (Synthetic actors have no scene instance, so none can die here.)
    assert!(e.spared_by(ACTOR, PLAYER_REF));
    assert!(e.spared_by(OTHER, BYSTANDER));
    assert!(!e.spared_by(OTHER, PLAYER_REF));
    e.damage(ACTOR, 100.0, Some(PLAYER_REF), 0.0);
    assert!(e.is_bleeding_out(ACTOR) && !e.is_dead(ACTOR));
    e.damage(OTHER, 100.0, Some(BYSTANDER), 0.0);
    assert!(e.is_bleeding_out(OTHER) && !e.is_dead(OTHER));
    assert!(!e.is_bleeding_out(BYSTANDER));
    e.damage(BYSTANDER, 100.0, Some(ACTOR), 0.0);
    assert!(!e.is_bleeding_out(BYSTANDER), "not spared");
    // Conditions see the alias too: IsEssential (354), IsProtected (476).
    let holds = |e: &Engine, r: FormId, f: u16| {
        let rec = e.lo.get(NPC).unwrap();
        let mut d = [0u8; 32];
        d[4..8].copy_from_slice(&1.0f32.to_le_bytes());
        d[8..10].copy_from_slice(&f.to_le_bytes());
        let c = crate::condition::parse(&rec, &d).unwrap();
        crate::condition::evaluate(
            e,
            &[c],
            crate::condition::Context {
                subject: Some(r),
                ..Default::default()
            },
        )
    };
    assert!(holds(&e, ACTOR, 354));
    quest(&mut e, "stop");
    assert!(!e.is_essential(ACTOR));
    assert!(!holds(&e, ACTOR, 354));
}
