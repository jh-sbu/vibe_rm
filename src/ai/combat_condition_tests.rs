//! Combat-state condition functions on a synthetic actor: `GetCombatState`,
//! `IsCombatTarget`, `IsPowerAttacking`, `IsShieldOut` and `IsTorchOut`. Engine
//! owns a renderer, so these need a headless GPU adapter.

use super::*;
use crate::engine::CellRuntime;
use crate::world::inventory::Inventory;
use esp::writer::{Node, RecordBuilder, write_plugin};
use glam::Vec3;

const NPC: FormId = FormId(0x830);
const ACTOR: FormId = FormId(0x831);
const OTHER: FormId = FormId(0x832);
const SHIELD: FormId = FormId(0x840);
const TORCH: FormId = FormId(0x841);
const CELL: FormId = FormId(0x850);

const GET_COMBAT_STATE: u16 = 323;
const IS_COMBAT_TARGET: u16 = 513;
const IS_POWER_ATTACKING: u16 = 673;
const IS_SHIELD_OUT: u16 = 103;
const IS_TORCH_OUT: u16 = 102;

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
        std::env::temp_dir().join(format!("vrm-combat-cond-{}-{stamp}", std::process::id())),
    );
    std::fs::create_dir(&f.0).unwrap();
    let mut acbs = [0; 24];
    acbs[8..10].copy_from_slice(&20u16.to_le_bytes());
    let records = [
        Node::top(
            b"NPC_",
            vec![RecordBuilder::new(b"NPC_", NPC).field(b"ACBS", &acbs)],
        ),
        Node::top(
            b"ACHR",
            vec![RecordBuilder::new(b"ACHR", ACTOR).u32(b"NAME", NPC.0)],
        ),
        Node::top(b"ARMO", vec![RecordBuilder::new(b"ARMO", SHIELD)]),
        Node::top(b"LIGH", vec![RecordBuilder::new(b"LIGH", TORCH)]),
    ];
    std::fs::write(
        f.0.join("CombatCondition.esp"),
        write_plugin(
            0,
            &[],
            "test",
            "synthetic combat conditions",
            0x900,
            &records,
        ),
    )
    .unwrap();
    let mut e = crate::stats::action_tests::headless_engine(&f.0, "CombatCondition.esp");
    e.location = crate::engine::Location::Interior(CELL);
    let mut actor = ActorRuntime::new(
        ACTOR,
        NPC,
        Arc::new(crate::world::skeleton::Skeleton::default()),
        Mat4::IDENTITY,
        Vec::new(),
        0.0,
    );
    actor.health = 100.0;
    actor.stats = Arc::new(CombatStats {
        attacks: vec![attack("testBasic", 0), attack("testPower", ATK_POWER)],
        ..Default::default()
    });
    let key = crate::render::CellKey::Interior(CELL);
    e.actor_cells.insert(ACTOR, key);
    e.cells
        .entry(key)
        .or_insert_with(CellRuntime::default)
        .actors
        .push(actor);
    (f, e)
}

fn attack(event: &str, flags: u32) -> Attack {
    Attack {
        event: event.into(),
        damage_mult: 1.0,
        chance: 1.0,
        flags,
        strike_angle: 35.0,
        stagger: 0.0,
        stamina_mult: 1.0,
        attack_type: FormId::NULL,
    }
}

/// `func(param) == value` as a CTDA record's bytes.
fn ctda(func: u16, param: u32, value: f32) -> [u8; 32] {
    let mut d = [0; 32];
    d[4..8].copy_from_slice(&value.to_le_bytes());
    d[8..10].copy_from_slice(&func.to_le_bytes());
    d[12..16].copy_from_slice(&param.to_le_bytes());
    d
}

/// Whether `func(param) == value` holds for `subject`.
fn holds(e: &Engine, subject: FormId, func: u16, param: u32, value: f32) -> bool {
    let rec = e.lo.get(NPC).unwrap();
    let c = crate::condition::parse(&rec, &ctda(func, param, value)).unwrap();
    crate::condition::evaluate(
        e,
        &[c],
        crate::condition::Context {
            subject: Some(subject),
            ..Default::default()
        },
    )
}

fn actor_mut(e: &mut Engine) -> &mut ActorRuntime {
    e.actor_mut(ACTOR).unwrap()
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn combat_state_and_target_follow_the_actors_fight() {
    let (_f, mut e) = fixture();
    assert!(holds(&e, ACTOR, GET_COMBAT_STATE, 0, 0.0));
    assert!(!holds(&e, ACTOR, IS_COMBAT_TARGET, PLAYER_REF.0, 1.0));
    actor_mut(&mut e).combat = Some(Combat::new(PLAYER_REF));
    // Fighting (1), its target the player and no one else.
    assert!(holds(&e, ACTOR, GET_COMBAT_STATE, 0, 1.0));
    assert!(!holds(&e, ACTOR, GET_COMBAT_STATE, 0, 2.0));
    assert!(holds(&e, ACTOR, IS_COMBAT_TARGET, PLAYER_REF.0, 1.0));
    assert!(!holds(&e, ACTOR, IS_COMBAT_TARGET, OTHER.0, 1.0));
    // Searching (2) for a target it lost.
    actor_mut(&mut e).combat = None;
    e.start_search(ACTOR, PLAYER_REF, Vec3::ZERO, true);
    assert!(holds(&e, ACTOR, GET_COMBAT_STATE, 0, 2.0));
    // The target it lost is still the one `GetCombatTarget` reports (see
    // `known_ambiguities/combat-conditions.md`).
    assert!(holds(&e, ACTOR, IS_COMBAT_TARGET, PLAYER_REF.0, 1.0));
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn power_attacking_is_the_swing_being_made_while_it_runs() {
    let (_f, mut e) = fixture();
    let mut combat = Combat::new(PLAYER_REF);
    combat.attack = Some(1);
    actor_mut(&mut e).combat = Some(combat);
    // No swing running: the attack it last chose isn't being made.
    assert!(!holds(&e, ACTOR, IS_POWER_ATTACKING, 0, 1.0));
    actor_mut(&mut e).combat.as_mut().unwrap().swing = 0.5;
    assert!(holds(&e, ACTOR, IS_POWER_ATTACKING, 0, 1.0));
    // A basic attack running is not one.
    actor_mut(&mut e).combat.as_mut().unwrap().attack = Some(0);
    assert!(!holds(&e, ACTOR, IS_POWER_ATTACKING, 0, 1.0));
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn shield_is_out_only_drawn_and_torch_only_in_the_left_hand() {
    let (_f, mut e) = fixture();
    let hold = |e: &mut Engine, form: FormId| {
        let mut inv = Inventory::default();
        inv.add(form, 1);
        inv.hold(&e.lo, form, &[LEFT]);
        e.inventories.insert(ACTOR, inv);
    };
    hold(&mut e, SHIELD);
    actor_mut(&mut e).drawn = false;
    assert!(!holds(&e, ACTOR, IS_SHIELD_OUT, 0, 1.0));
    assert!(!holds(&e, ACTOR, IS_TORCH_OUT, 0, 1.0));
    actor_mut(&mut e).drawn = true;
    assert!(holds(&e, ACTOR, IS_SHIELD_OUT, 0, 1.0));
    assert!(!holds(&e, ACTOR, IS_TORCH_OUT, 0, 1.0));
    // A torch in the left hand is out, drawn or not; the shield is then gone.
    hold(&mut e, TORCH);
    assert!(holds(&e, ACTOR, IS_TORCH_OUT, 0, 1.0));
    assert!(!holds(&e, ACTOR, IS_SHIELD_OUT, 0, 1.0));
    actor_mut(&mut e).drawn = false;
    assert!(holds(&e, ACTOR, IS_TORCH_OUT, 0, 1.0));
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn a_null_reference_in_positional_conditions_is_the_combat_target() {
    const GET_LINE_OF_SIGHT: u16 = 27;
    const GET_HEADING_ANGLE: u16 = 99;
    let (_f, mut e) = fixture();
    actor_mut(&mut e).pos = Vec3::ZERO;
    let z = e.physics.player_half_height + e.physics.player_radius;
    e.player.position = Vec3::new(100.0, 0.0, z);
    // Nobody fought: the null names nothing.
    assert!(!holds(&e, ACTOR, GET_HEADING_ANGLE, 0, 90.0));
    assert!(!holds(&e, ACTOR, GET_LINE_OF_SIGHT, 0, 1.0));
    // Fighting the player, to the east of an actor facing north.
    actor_mut(&mut e).combat = Some(Combat::new(PLAYER_REF));
    assert!(holds(&e, ACTOR, GET_HEADING_ANGLE, 0, 90.0));
    assert!(holds(&e, ACTOR, GET_HEADING_ANGLE, PLAYER_REF.0, 90.0));
    assert!(holds(&e, ACTOR, GET_LINE_OF_SIGHT, 0, 1.0));
}
