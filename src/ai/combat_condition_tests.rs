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
const KIN: FormId = FormId(0x860);
const KIN_NPC: FormId = FormId(0x861);
const FRIEND: FormId = FormId(0x862);
const FRIEND_NPC: FormId = FormId(0x863);
const KEYWORD: FormId = FormId(0x864);
const PROTECTED_NPC: FormId = FormId(0x865);
const PROTECTED_REF: FormId = FormId(0x866);
const STRANGER_NPC: FormId = FormId(0x867);
const TAGGED_NPC: FormId = FormId(0x868);
const TAGGED_REF: FormId = FormId(0x8C4);
const KIN_REF: FormId = FormId(0x8C1);
const FRIEND_REF: FormId = FormId(0x8C2);
const STRANGER_REF: FormId = FormId(0x8C3);

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
    // `XNAM`: the other faction, a modifier and the group combat reaction.
    let reaction = |other: FormId, r: u32| {
        let mut d = other.0.to_le_bytes().to_vec();
        d.extend_from_slice(&0i32.to_le_bytes());
        d.extend_from_slice(&r.to_le_bytes());
        d
    };
    // `SNAM`: a faction and a rank.
    let member = |f: FormId| {
        let mut d = f.0.to_le_bytes().to_vec();
        d.extend_from_slice(&[0; 4]);
        d
    };
    let mut protected = acbs;
    protected[0..4].copy_from_slice(&0x800u32.to_le_bytes());
    let records = [
        Node::top(
            b"NPC_",
            vec![RecordBuilder::new(b"NPC_", NPC).field(b"ACBS", &acbs)],
        ),
        Node::top(
            b"ACHR",
            vec![RecordBuilder::new(b"ACHR", ACTOR).u32(b"NAME", NPC.0)],
        ),
        // Kin and friends: each lists the other in its group combat reaction
        // (2 ally, 3 friend), and the NPCs are members.
        Node::top(
            b"FACT",
            vec![
                RecordBuilder::new(b"FACT", KIN).field(b"XNAM", &reaction(FRIEND, 2)),
                RecordBuilder::new(b"FACT", FRIEND).field(b"XNAM", &reaction(KIN, 3)),
            ],
        ),
        Node::top(
            b"NPC_",
            vec![
                RecordBuilder::new(b"NPC_", KIN_NPC)
                    .field(b"ACBS", &acbs)
                    .field(b"SNAM", &member(KIN)),
                RecordBuilder::new(b"NPC_", FRIEND_NPC)
                    .field(b"ACBS", &acbs)
                    .field(b"SNAM", &member(FRIEND)),
                RecordBuilder::new(b"NPC_", STRANGER_NPC).field(b"ACBS", &acbs),
                RecordBuilder::new(b"NPC_", TAGGED_NPC)
                    .field(b"ACBS", &acbs)
                    .form(b"KWDA", KEYWORD),
                RecordBuilder::new(b"NPC_", PROTECTED_NPC).field(b"ACBS", &protected),
            ],
        ),
        Node::top(
            b"ACHR",
            vec![
                RecordBuilder::new(b"ACHR", PROTECTED_REF).u32(b"NAME", PROTECTED_NPC.0),
                RecordBuilder::new(b"ACHR", KIN_REF).u32(b"NAME", KIN_NPC.0),
                RecordBuilder::new(b"ACHR", FRIEND_REF).u32(b"NAME", FRIEND_NPC.0),
                RecordBuilder::new(b"ACHR", STRANGER_REF).u32(b"NAME", STRANGER_NPC.0),
                RecordBuilder::new(b"ACHR", TAGGED_REF).u32(b"NAME", TAGGED_NPC.0),
            ],
        ),
        Node::top(b"KYWD", vec![RecordBuilder::new(b"KYWD", KEYWORD)]),
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

/// Another loaded actor of `base`, standing at the origin.
fn add_actor(e: &mut Engine, id: FormId, base: FormId) {
    let mut actor = ActorRuntime::new(
        id,
        base,
        Arc::new(crate::world::skeleton::Skeleton::default()),
        Mat4::IDENTITY,
        Vec::new(),
        0.0,
    );
    actor.health = 100.0;
    let key = crate::render::CellKey::Interior(CELL);
    e.actor_cells.insert(id, key);
    e.cells
        .entry(key)
        .or_insert_with(CellRuntime::default)
        .actors
        .push(actor);
}

fn fights(e: &mut Engine, who: FormId, target: FormId) {
    e.actor_mut(who).unwrap().combat = Some(Combat::new(target));
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn combat_groups_are_the_sides_of_a_fight() {
    const GET_GROUP_TARGET_COUNT: u16 = 416;
    const IS_LAST_HOSTILE_ACTOR: u16 = 674;
    const GET_COMBAT_GROUP_MEMBER_COUNT: u16 = 709;
    let (_f, mut e) = fixture();
    let (a2, e1, e2) = (FormId(0x8A2), FormId(0x8E1), FormId(0x8E2));
    for id in [a2, e1, e2] {
        add_actor(&mut e, id, NPC);
    }
    // Nobody fights: no group.
    assert!(holds(&e, ACTOR, GET_COMBAT_GROUP_MEMBER_COUNT, 0, 0.0));
    assert!(!holds(&e, ACTOR, IS_LAST_HOSTILE_ACTOR, 0, 1.0));
    // One against one: each is the last of its side.
    fights(&mut e, ACTOR, e1);
    fights(&mut e, e1, ACTOR);
    assert!(holds(&e, ACTOR, GET_COMBAT_GROUP_MEMBER_COUNT, 0, 1.0));
    assert!(holds(&e, ACTOR, GET_GROUP_TARGET_COUNT, 0, 1.0));
    assert!(holds(&e, e1, IS_LAST_HOSTILE_ACTOR, 0, 1.0));
    // A second fights the same enemy, and a third joins the enemy's side
    // against it.
    fights(&mut e, a2, e1);
    fights(&mut e, e2, a2);
    assert!(holds(&e, ACTOR, GET_COMBAT_GROUP_MEMBER_COUNT, 0, 2.0));
    assert!(holds(&e, a2, GET_COMBAT_GROUP_MEMBER_COUNT, 0, 2.0));
    assert!(holds(&e, ACTOR, GET_GROUP_TARGET_COUNT, 0, 2.0));
    assert!(holds(&e, e1, GET_COMBAT_GROUP_MEMBER_COUNT, 0, 2.0));
    assert!(!holds(&e, ACTOR, IS_LAST_HOSTILE_ACTOR, 0, 1.0));
    // One fighting the player is the last, until another does too.
    fights(&mut e, e2, PLAYER_REF);
    assert!(holds(&e, e2, IS_LAST_HOSTILE_ACTOR, 0, 1.0));
    fights(&mut e, e1, PLAYER_REF);
    assert!(holds(&e, e2, IS_LAST_HOSTILE_ACTOR, 0, 0.0));
    assert!(holds(&e, e2, GET_COMBAT_GROUP_MEMBER_COUNT, 0, 2.0));
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn combat_target_keyword_and_lethal_intent() {
    const GET_COMBAT_TARGET_HAS_KEYWORD: u16 = 707;
    const SHOULD_ATTACK_KILL: u16 = 678;
    let (_f, mut e) = fixture();
    let victim = FormId(0x8B1);
    add_actor(&mut e, victim, NPC);
    // Nobody fought: no target to have the keyword or to kill.
    assert!(holds(
        &e,
        ACTOR,
        GET_COMBAT_TARGET_HAS_KEYWORD,
        KEYWORD.0,
        0.0
    ));
    assert!(!holds(&e, ACTOR, SHOULD_ATTACK_KILL, 0, 1.0));
    fights(&mut e, ACTOR, victim);
    assert!(holds(
        &e,
        ACTOR,
        GET_COMBAT_TARGET_HAS_KEYWORD,
        KEYWORD.0,
        0.0
    ));
    add_actor(&mut e, TAGGED_REF, TAGGED_NPC);
    fights(&mut e, ACTOR, TAGGED_REF);
    assert!(holds(
        &e,
        ACTOR,
        GET_COMBAT_TARGET_HAS_KEYWORD,
        KEYWORD.0,
        1.0
    ));
    fights(&mut e, ACTOR, victim);
    assert!(holds(&e, ACTOR, SHOULD_ATTACK_KILL, 0, 1.0));
    assert!(holds(&e, ACTOR, SHOULD_ATTACK_KILL, victim.0, 1.0));
    // An essential victim bleeds out, a protected one but to the player.
    let mut stats = (*e.actor_ref(victim).unwrap().stats).clone();
    stats.essential = true;
    e.actor_mut(victim).unwrap().stats = Arc::new(stats.clone());
    assert!(holds(&e, ACTOR, SHOULD_ATTACK_KILL, 0, 0.0));
    stats.essential = false;
    stats.protected = true;
    e.actor_mut(victim).unwrap().stats = Arc::new(stats);
    assert!(holds(&e, ACTOR, SHOULD_ATTACK_KILL, 0, 0.0));
    // The player has no combat target: the context's is the victim (an idle's).
    let rec = e.lo.get(NPC).unwrap();
    let c = crate::condition::parse(&rec, &ctda(SHOULD_ATTACK_KILL, 0, 1.0)).unwrap();
    let towards = |subject: FormId, target: FormId| {
        crate::condition::evaluate(
            &e,
            &[c.clone()],
            crate::condition::Context {
                subject: Some(subject),
                target: Some(target),
                ..Default::default()
            },
        )
    };
    assert!(towards(PLAYER_REF, victim));
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn protected_actors_loaded_or_not_and_aggro_radius() {
    const IS_PROTECTED: u16 = 476;
    const AGGRO_RADIUS_VIOLATED: u16 = 118;
    let (_f, mut e) = fixture();
    assert!(holds(&e, ACTOR, IS_PROTECTED, 0, 0.0));
    // Not loaded: the base's flag.
    assert!(holds(&e, PROTECTED_REF, IS_PROTECTED, 0, 1.0));
    let mut stats = (*e.actor_ref(ACTOR).unwrap().stats).clone();
    stats.protected = true;
    e.actor_mut(ACTOR).unwrap().stats = Arc::new(stats.clone());
    assert!(holds(&e, ACTOR, IS_PROTECTED, 0, 1.0));
    // The player inside the attack radius.
    stats.aggro_attack = Some(500.0);
    e.actor_mut(ACTOR).unwrap().stats = Arc::new(stats);
    actor_mut(&mut e).pos = Vec3::ZERO;
    e.player.position = Vec3::new(300.0, 0.0, 0.0);
    assert!(holds(&e, ACTOR, AGGRO_RADIUS_VIOLATED, 0, 1.0));
    e.player.position = Vec3::new(900.0, 0.0, 0.0);
    assert!(holds(&e, ACTOR, AGGRO_RADIUS_VIOLATED, 0, 0.0));
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn should_help_follows_the_factions_and_assistance() {
    const GET_SHOULD_HELP: u16 = 705;
    let (_f, mut e) = fixture();
    let (kin, friend, stranger) = (KIN_REF, FRIEND_REF, STRANGER_REF);
    add_actor(&mut e, kin, KIN_NPC);
    add_actor(&mut e, friend, FRIEND_NPC);
    add_actor(&mut e, stranger, STRANGER_NPC);
    let assist = |e: &mut Engine, who: FormId, level: u8| {
        let mut stats = (*e.actor_ref(who).unwrap().stats).clone();
        stats.assistance = level;
        e.actor_mut(who).unwrap().stats = Arc::new(stats);
    };
    // Kin lists the friends as allies (2), who list kin as friends (3).
    assist(&mut e, kin, 0);
    assert!(holds(&e, kin, GET_SHOULD_HELP, friend.0, 0.0));
    assist(&mut e, kin, 1);
    assert!(holds(&e, kin, GET_SHOULD_HELP, friend.0, 1.0));
    assert!(holds(&e, kin, GET_SHOULD_HELP, stranger.0, 0.0));
    // Helping a friend takes the highest assistance.
    assist(&mut e, friend, 1);
    assert!(holds(&e, friend, GET_SHOULD_HELP, kin.0, 0.0));
    assist(&mut e, friend, 2);
    assert!(holds(&e, friend, GET_SHOULD_HELP, kin.0, 1.0));
}
