//! Spawn Hazard effects (archetype 40) and hazards placed by script, on
//! generated records. Engine needs a headless adapter.

use super::*;
use crate::engine::CellRuntime;
use crate::magic::{archetype, flags, spell_type};
use esp::actor_value as av;
use esp::writer::{Node, RecordBuilder, write_plugin};
use glam::Mat4;
use std::sync::Arc;

/// The spawning spell (self, magnitude 20, area 30 ft, 7 s).
const STORM: FormId = FormId(0x810);
/// The hazard's spell: 10 damage to health at once.
const STORM_TOUCH: FormId = FormId(0x811);
const SPAWN_EFFECT: FormId = FormId(0x800);
const DAMAGE_EFFECT: FormId = FormId(0x801);
/// Inherits the spell's duration and area (record: 5 ft, 60 s).
const INHERITING: FormId = FormId(0x820);
/// Keeps its record's 5 ft and 60 s, placed by script.
const PLAIN: FormId = FormId(0x821);
const NPC: FormId = FormId(0x830);
const ACTOR: FormId = FormId(0x840);

struct Fixture(std::path::PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn hazard(id: FormId, flags: u32) -> RecordBuilder {
    let mut data = Vec::new();
    data.extend_from_slice(&0u32.to_le_bytes());
    for f in [5.0f32, 60.0, 0.0, 1.0] {
        data.extend_from_slice(&f.to_le_bytes());
    }
    for w in [flags, STORM_TOUCH.0, 0, 0, 0] {
        data.extend_from_slice(&w.to_le_bytes());
    }
    RecordBuilder::new(b"HAZD", id).field(b"DATA", &data)
}

fn fixture() -> (Fixture, Engine) {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = Fixture(
        std::env::temp_dir().join(format!("vrm-spawn-hazard-{}-{stamp}", std::process::id())),
    );
    std::fs::create_dir(&dir.0).unwrap();
    let effect = |id, archetype: u32, flag: u32, value: u32, related: u32| {
        let mut data = [0; 0x6c];
        data[..4].copy_from_slice(&flag.to_le_bytes());
        data[8..12].copy_from_slice(&related.to_le_bytes());
        data[0x10..0x14].copy_from_slice(&u32::MAX.to_le_bytes());
        data[0x40..0x44].copy_from_slice(&archetype.to_le_bytes());
        data[0x44..0x48].copy_from_slice(&value.to_le_bytes());
        RecordBuilder::new(b"MGEF", id).field(b"DATA", &data)
    };
    let spell = |id: FormId, effect: FormId, magnitude: f32, area: u32, duration: u32| {
        let mut spit = [0; 36];
        spit[8..12].copy_from_slice(&spell_type::SPELL.to_le_bytes());
        spit[16..20].copy_from_slice(&1u32.to_le_bytes());
        let mut efit = [0; 12];
        efit[..4].copy_from_slice(&magnitude.to_le_bytes());
        efit[4..8].copy_from_slice(&area.to_le_bytes());
        efit[8..12].copy_from_slice(&duration.to_le_bytes());
        RecordBuilder::new(b"SPEL", id)
            .field(b"SPIT", &spit)
            .u32(b"EFID", effect.0)
            .field(b"EFIT", &efit)
    };
    let records = [
        Node::top(
            b"MGEF",
            vec![
                effect(
                    SPAWN_EFFECT,
                    archetype::SPAWN_HAZARD,
                    0,
                    u32::MAX,
                    INHERITING.0,
                ),
                effect(
                    DAMAGE_EFFECT,
                    archetype::VALUE_MODIFIER,
                    flags::HOSTILE | flags::DETRIMENTAL,
                    av::HEALTH,
                    0,
                ),
            ],
        ),
        Node::top(
            b"SPEL",
            vec![
                spell(STORM, SPAWN_EFFECT, 20.0, 30, 7),
                spell(STORM_TOUCH, DAMAGE_EFFECT, 10.0, 0, 0),
            ],
        ),
        Node::top(
            b"HAZD",
            vec![
                hazard(INHERITING, INHERIT_DURATION | INHERIT_RADIUS),
                hazard(PLAIN, 0),
            ],
        ),
        Node::top(b"NPC_", vec![RecordBuilder::new(b"NPC_", NPC)]),
        Node::top(
            b"ACHR",
            vec![RecordBuilder::new(b"ACHR", ACTOR).u32(b"NAME", NPC.0)],
        ),
    ];
    std::fs::write(
        dir.0.join("SpawnHazard.esp"),
        write_plugin(0, &[], "test", "synthetic spawned hazards", 0x900, &records),
    )
    .unwrap();
    let mut e = crate::stats::action_tests::headless_engine(&dir.0, "SpawnHazard.esp");
    // An actor 200 units from the player: inside 30 ft, outside 5 ft.
    let player = e.ref_position(PLAYER_REF).unwrap();
    let key = crate::render::CellKey::Interior(FormId(0x860));
    let mut actor = crate::ai::ActorRuntime::new(
        ACTOR,
        NPC,
        Arc::new(crate::world::skeleton::Skeleton::default()),
        Mat4::from_translation(player + Vec3::X * 200.0),
        Vec::new(),
        0.0,
    );
    actor.exec.health = 1000.0;
    e.actor_cells.insert(ACTOR, key);
    e.cells
        .entry(key)
        .or_insert_with(CellRuntime::default)
        .actors
        .push(actor);
    (dir, e)
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn a_spawned_hazard_is_its_casters_with_the_spells_area_duration_and_gain() {
    let (_dir, mut e) = fixture();
    let player = e.ref_position(PLAYER_REF).unwrap();
    assert!(e.apply_cast(STORM, PLAYER_REF, PLAYER_REF, None, false));
    let h = e.impacts.hazards.last().unwrap();
    assert_eq!(h.base, INHERITING);
    assert_eq!(h.pos, player, "at the caster's feet");
    assert_eq!(h.radius, 30.0 * UNITS_PER_FOOT, "the spell's area");
    assert_eq!(h.life, Some(7.0), "the spell's duration");
    assert_eq!(h.owner, Some(PLAYER_REF));
    assert_eq!(h.scale, 1.0);

    let health = e.actor_value(ACTOR, av::HEALTH);
    e.update_hazards(0.016);
    assert_eq!(e.actor_value(ACTOR, av::HEALTH), health - 10.0);
    assert!(
        e.magic
            .effects
            .iter()
            .any(|x| x.target == ACTOR && x.caster == Some(PLAYER_REF)),
        "touched by its owner"
    );

    // Dual cast: the gain carries to its spell.
    e.impacts.hazards.clear();
    assert!(e.apply_cast(STORM, PLAYER_REF, PLAYER_REF, None, true));
    let scale = e.impacts.hazards.last().unwrap().scale;
    assert!((scale - 2.2).abs() < 1e-4, "{scale}");
    let health = e.actor_value(ACTOR, av::HEALTH);
    e.update_hazards(0.016);
    assert!((e.actor_value(ACTOR, av::HEALTH) - (health - 22.0)).abs() < 1e-3);

    // Its hostile spell spares its owner.
    let player_health = e.actor_value(PLAYER_REF, av::HEALTH);
    e.impacts.hazards.last_mut().unwrap().next = 0.0;
    e.update_hazards(0.016);
    assert_eq!(e.actor_value(PLAYER_REF, av::HEALTH), player_health);

    // It runs out with its duration.
    e.update_hazards(7.5);
    assert!(e.impacts.hazards.is_empty());
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn a_hazard_placed_by_script_lasts_until_it_is_disabled() {
    let (_dir, mut e) = fixture();
    let r = e.create_ref(PLAIN, ACTOR, false).unwrap();
    let h = e.impacts.hazards.last().unwrap();
    assert_eq!(h.ref_id, r);
    assert_eq!(h.owner, Some(ACTOR));
    assert_eq!(h.radius, 5.0 * UNITS_PER_FOOT);
    assert_eq!(h.life, None);
    e.update_hazards(100.0);
    assert_eq!(e.impacts.hazards.len(), 1);
    e.set_disabled(r, true);
    e.update_hazards(0.016);
    assert!(e.impacts.hazards.is_empty());
}
