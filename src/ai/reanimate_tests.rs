//! Reanimation (archetype 22) on generated records. Engine needs a headless
//! adapter.

use super::*;
use crate::engine::CellRuntime;
use crate::magic::{flags, spell_type};
use crate::render::{ActorInstance, RenderCell};
use esp::actor_value as av;
use esp::writer::{Node, RecordBuilder, write_plugin};
use glam::{Mat4, Vec3};
use std::sync::Arc;

/// Reanimate (magnitude 10, 60 s) with a companion value modifier, as
/// Raise Zombie's "Reanimate Secondary Effects".
const RAISE: FormId = FormId(0x810);
/// A plain value modifier, to show the dead take nothing else.
const ORDINARY: FormId = FormId(0x811);
const REANIMATE_EFFECT: FormId = FormId(0x800);
const COMPANION_EFFECT: FormId = FormId(0x801);
const NPC_LOW: FormId = FormId(0x830);
const NPC_HIGH: FormId = FormId(0x831);
const BODY: FormId = FormId(0x840);
const OTHER_BODY: FormId = FormId(0x841);
const STRONG_BODY: FormId = FormId(0x842);
const LOOT: FormId = FormId(0x850);

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
        Fixture(std::env::temp_dir().join(format!("vrm-reanimate-{}-{stamp}", std::process::id())));
    std::fs::create_dir(&dir.0).unwrap();
    let effect = |id, archetype: u32, value: u32| {
        let mut data = [0; 0x6c];
        data[..4].copy_from_slice(&flags::RECOVER.to_le_bytes());
        data[0x10..0x14].copy_from_slice(&u32::MAX.to_le_bytes());
        data[0x40..0x44].copy_from_slice(&archetype.to_le_bytes());
        data[0x44..0x48].copy_from_slice(&value.to_le_bytes());
        RecordBuilder::new(b"MGEF", id).field(b"DATA", &data)
    };
    let spell = |id: FormId, effects: &[(FormId, f32)]| {
        let mut spit = [0; 36];
        spit[..4].copy_from_slice(&10u32.to_le_bytes());
        spit[8..12].copy_from_slice(&spell_type::SPELL.to_le_bytes());
        spit[16..20].copy_from_slice(&1u32.to_le_bytes()); // fire and forget
        let mut out = RecordBuilder::new(b"SPEL", id).field(b"SPIT", &spit);
        for &(effect, magnitude) in effects {
            let mut efit = [0; 12];
            efit[..4].copy_from_slice(&magnitude.to_le_bytes());
            efit[8..12].copy_from_slice(&60u32.to_le_bytes());
            out = out.u32(b"EFID", effect.0).field(b"EFIT", &efit);
        }
        out
    };
    let npc = |id, level: u16| {
        let mut acbs = [0; 24];
        acbs[8..10].copy_from_slice(&level.to_le_bytes());
        RecordBuilder::new(b"NPC_", id).field(b"ACBS", &acbs)
    };
    let records = [
        Node::top(
            b"MGEF",
            vec![
                effect(REANIMATE_EFFECT, archetype::REANIMATE, u32::MAX),
                effect(COMPANION_EFFECT, archetype::VALUE_MODIFIER, av::SNEAK),
            ],
        ),
        Node::top(
            b"SPEL",
            vec![
                spell(RAISE, &[(REANIMATE_EFFECT, 10.0), (COMPANION_EFFECT, 5.0)]),
                spell(ORDINARY, &[(COMPANION_EFFECT, 5.0)]),
            ],
        ),
        Node::top(b"NPC_", vec![npc(NPC_LOW, 6), npc(NPC_HIGH, 20)]),
        Node::top(
            b"ACHR",
            vec![
                RecordBuilder::new(b"ACHR", BODY).u32(b"NAME", NPC_LOW.0),
                RecordBuilder::new(b"ACHR", OTHER_BODY).u32(b"NAME", NPC_LOW.0),
                RecordBuilder::new(b"ACHR", STRONG_BODY).u32(b"NAME", NPC_HIGH.0),
            ],
        ),
        Node::top(b"MISC", vec![RecordBuilder::new(b"MISC", LOOT)]),
    ];
    std::fs::write(
        dir.0.join("Reanimate.esp"),
        write_plugin(0, &[], "test", "synthetic reanimation", 0x900, &records),
    )
    .unwrap();
    let mut engine = crate::stats::action_tests::headless_engine(&dir.0, "Reanimate.esp");
    for (r, base, x) in [
        (BODY, NPC_LOW, 100.0),
        (OTHER_BODY, NPC_LOW, -100.0),
        (STRONG_BODY, NPC_HIGH, 300.0),
    ] {
        load_dead(&mut engine, r, base, Vec3::new(x, 200.0, 0.0));
    }
    (dir, engine)
}

/// A body lying in the loaded cell.
fn load_dead(engine: &mut Engine, r: FormId, base: FormId, pos: Vec3) {
    let key = crate::render::CellKey::Interior(FormId(0x860));
    let mut actor = crate::ai::ActorRuntime::new(
        r,
        base,
        Arc::new(crate::world::skeleton::Skeleton::default()),
        Mat4::from_translation(pos),
        Vec::new(),
        0.0,
    );
    actor.exec.health = 50.0;
    let skeleton = actor.skeleton.clone();
    engine.actor_cells.insert(r, key);
    engine
        .cells
        .entry(key)
        .or_insert_with(CellRuntime::default)
        .actors
        .push(actor);
    // Its drawing, which a death reads its pose from.
    engine
        .scene
        .cells
        .entry(key)
        .or_insert_with(RenderCell::default)
        .actors
        .push(ActorInstance {
            meshes: vec![],
            attachments: vec![],
            equipment: vec![],
            held_light: None,
            transform: Mat4::from_translation(pos),
            pose: skeleton.model_space(&skeleton.bind_locals()),
            lights: [0xffff; 8],
            radius: 120.0,
            shadow_only: false,
            skin_tone: None,
            weight: 0.0,
            alpha: 1.0,
        });
    assert!(engine.kill_actor_by(r, Some(PLAYER_REF)));
    assert!(engine.is_dead(r));
}

fn reanimation(engine: &Engine, body: FormId) -> Option<u64> {
    engine
        .magic
        .effects
        .iter()
        .find(|x| x.target == body && x.effect == REANIMATE_EFFECT && !x.done)
        .map(|x| x.id)
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn a_body_within_the_level_rises_commanded_with_what_it_carries() {
    let (_dir, mut e) = fixture();
    e.add_item(BODY, LOOT, 2);
    // Anything else misses the dead.
    assert!(!e.apply_cast(ORDINARY, PLAYER_REF, BODY, None, false));
    assert!(e.is_dead(BODY));

    assert!(e.apply_cast(RAISE, PLAYER_REF, BODY, None, false));
    assert!(!e.is_dead(BODY));
    assert_eq!(e.commander_of(BODY), Some(PLAYER_REF));
    assert_eq!(
        e.item_count(BODY, LOOT),
        2,
        "not back to its base inventory"
    );
    // The companion effect landed on the risen body.
    assert!(
        e.magic
            .effects
            .iter()
            .any(|x| x.target == BODY && x.effect == COMPANION_EFFECT && x.active && !x.done)
    );
    // Already risen: a second cast raises nothing more.
    assert!(!e.reanimates(BODY, Some(PLAYER_REF), 10.0));
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn the_level_cap_the_living_and_disintegrated_bodies_stay_as_they_are() {
    let (_dir, mut e) = fixture();
    assert!(!e.apply_cast(RAISE, PLAYER_REF, STRONG_BODY, None, false));
    assert!(e.is_dead(STRONG_BODY));
    assert!(reanimation(&e, STRONG_BODY).is_none());

    e.set_critical_stage(BODY, 3);
    assert!(!e.apply_cast(RAISE, PLAYER_REF, BODY, None, false));
    assert!(e.is_dead(BODY));

    e.resurrect(OTHER_BODY);
    assert!(!e.is_dead(OTHER_BODY));
    e.apply_cast(RAISE, PLAYER_REF, OTHER_BODY, None, false);
    assert!(
        reanimation(&e, OTHER_BODY).is_none(),
        "the living aren't raised"
    );
    assert_eq!(e.commander_of(OTHER_BODY), None);
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn expiring_kills_the_body_again_and_ends_its_companions() {
    let (_dir, mut e) = fixture();
    assert!(e.apply_cast(RAISE, PLAYER_REF, BODY, None, false));
    e.update_magic(30.0);
    assert!(!e.is_dead(BODY));
    e.update_magic(31.0);
    assert!(e.is_dead(BODY));
    assert_eq!(e.commander_of(BODY), None);
    assert!(
        e.magic
            .effects
            .iter()
            .filter(|x| x.target == BODY && x.item == RAISE)
            .all(|x| x.done)
    );
    // Killed while raised, the effect ends with no second death.
    assert!(e.apply_cast(RAISE, PLAYER_REF, OTHER_BODY, None, false));
    assert!(e.kill_actor_by(OTHER_BODY, None));
    e.update_magic(0.1);
    assert!(reanimation(&e, OTHER_BODY).is_none());
    assert_eq!(e.commander_of(OTHER_BODY), None);
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn a_second_reanimation_past_the_limit_lays_the_first_down() {
    let (_dir, mut e) = fixture();
    assert!(e.apply_cast(RAISE, PLAYER_REF, BODY, None, false));
    assert!(e.apply_cast(RAISE, PLAYER_REF, OTHER_BODY, None, false));
    assert!(e.is_dead(BODY), "the oldest goes to make room");
    assert_eq!(e.commander_of(BODY), None);
    assert!(!e.is_dead(OTHER_BODY));
    assert_eq!(e.commander_of(OTHER_BODY), Some(PLAYER_REF));
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn restored_reanimations_command_their_bodies_again() {
    let (_dir, mut e) = fixture();
    assert!(e.apply_cast(RAISE, PLAYER_REF, BODY, None, false));
    let saved = e.magic.effects.clone();
    let next = saved.iter().map(|x| x.id).max().unwrap_or(0);
    e.magic.commanded.clear();
    e.restore_active_effects(saved, next);
    assert_eq!(e.commander_of(BODY), Some(PLAYER_REF));
}
