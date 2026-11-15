//! Banish (archetype 42) and Command Daedra (archetype 10) on generated
//! records. Engine needs a headless adapter.

use super::*;
use crate::engine::CellRuntime;
use crate::magic::{ActiveEffect, flags, spell_type};
use esp::writer::{Node, RecordBuilder, write_plugin};
use glam::Mat4;
use std::sync::Arc;

/// Banish and Command at magnitude 10 (hostile, Target Actor).
const BANISH: FormId = FormId(0x810);
const COMMAND: FormId = FormId(0x811);
/// The summoner's summon spell (its effect is put on by hand).
const SUMMON: FormId = FormId(0x812);
const BANISH_EFFECT: FormId = FormId(0x800);
const COMMAND_EFFECT: FormId = FormId(0x801);
const SUMMON_EFFECT: FormId = FormId(0x802);
const NPC_LOW: FormId = FormId(0x830);
const NPC_HIGH: FormId = FormId(0x831);
const SUMMONER: FormId = FormId(0x840);
const ATRONACH: FormId = FormId(0x841);
const STRONG_ATRONACH: FormId = FormId(0x842);

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
        Fixture(std::env::temp_dir().join(format!("vrm-banish-{}-{stamp}", std::process::id())));
    std::fs::create_dir(&dir.0).unwrap();
    let effect = |id, archetype: u32, flag: u32| {
        let mut data = [0; 0x6c];
        data[..4].copy_from_slice(&flag.to_le_bytes());
        data[0x10..0x14].copy_from_slice(&u32::MAX.to_le_bytes());
        data[0x40..0x44].copy_from_slice(&archetype.to_le_bytes());
        data[0x44..0x48].copy_from_slice(&u32::MAX.to_le_bytes());
        RecordBuilder::new(b"MGEF", id).field(b"DATA", &data)
    };
    let spell = |id: FormId, effect: FormId, duration: u32| {
        let mut spit = [0; 36];
        spit[..4].copy_from_slice(&10u32.to_le_bytes());
        spit[8..12].copy_from_slice(&spell_type::SPELL.to_le_bytes());
        spit[16..20].copy_from_slice(&1u32.to_le_bytes());
        let mut efit = [0; 12];
        efit[..4].copy_from_slice(&10.0f32.to_le_bytes());
        efit[8..12].copy_from_slice(&duration.to_le_bytes());
        RecordBuilder::new(b"SPEL", id)
            .field(b"SPIT", &spit)
            .u32(b"EFID", effect.0)
            .field(b"EFIT", &efit)
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
                effect(BANISH_EFFECT, archetype::BANISH, flags::HOSTILE),
                effect(COMMAND_EFFECT, archetype::COMMAND_SUMMONED, flags::HOSTILE),
                effect(SUMMON_EFFECT, archetype::SUMMON_CREATURE, 0),
            ],
        ),
        Node::top(
            b"SPEL",
            vec![
                spell(BANISH, BANISH_EFFECT, 0),
                spell(COMMAND, COMMAND_EFFECT, 0),
                spell(SUMMON, SUMMON_EFFECT, 60),
            ],
        ),
        Node::top(b"NPC_", vec![npc(NPC_LOW, 5), npc(NPC_HIGH, 30)]),
        Node::top(
            b"ACHR",
            vec![
                RecordBuilder::new(b"ACHR", SUMMONER).u32(b"NAME", NPC_LOW.0),
                RecordBuilder::new(b"ACHR", ATRONACH).u32(b"NAME", NPC_LOW.0),
                RecordBuilder::new(b"ACHR", STRONG_ATRONACH).u32(b"NAME", NPC_HIGH.0),
            ],
        ),
    ];
    std::fs::write(
        dir.0.join("Banish.esp"),
        write_plugin(0, &[], "test", "synthetic banishing", 0x900, &records),
    )
    .unwrap();
    let mut engine = crate::stats::action_tests::headless_engine(&dir.0, "Banish.esp");
    for (r, base) in [
        (SUMMONER, NPC_LOW),
        (ATRONACH, NPC_LOW),
        (STRONG_ATRONACH, NPC_HIGH),
    ] {
        load(&mut engine, r, base);
    }
    for summon in [ATRONACH, STRONG_ATRONACH] {
        summoned(&mut engine, summon);
    }
    (dir, engine)
}

fn load(engine: &mut Engine, r: FormId, base: FormId) {
    let key = crate::render::CellKey::Interior(FormId(0x860));
    let mut actor = crate::ai::ActorRuntime::new(
        r,
        base,
        Arc::new(crate::world::skeleton::Skeleton::default()),
        Mat4::IDENTITY,
        Vec::new(),
        0.0,
    );
    actor.exec.health = 50.0;
    engine.actor_cells.insert(r, key);
    engine
        .cells
        .entry(key)
        .or_insert_with(CellRuntime::default)
        .actors
        .push(actor);
}

/// `SUMMONER`'s summon effect keeping `summon`, as `Engine::summon` leaves it.
fn summoned(engine: &mut Engine, summon: FormId) {
    let id = engine.magic.effects.iter().map(|x| x.id).max().unwrap_or(0) + 100;
    engine.magic.effects.push(ActiveEffect {
        id,
        target: SUMMONER,
        caster: Some(SUMMONER),
        item: SUMMON,
        equipment: None,
        index: 0,
        effect: SUMMON_EFFECT,
        magnitude: 0.0,
        duration: 60.0,
        elapsed: 0.0,
        active: true,
        held: [0.0; 2],
        scripted: false,
        done: false,
        quiet: false,
        at: None,
        summoned: Some(summon),
        dual: false,
        channelled: false,
    });
    engine.magic.commanded.insert(summon, SUMMONER);
}

fn summon_effect(engine: &Engine, summon: FormId) -> &ActiveEffect {
    engine
        .magic
        .effects
        .iter()
        .find(|x| x.summoned == Some(summon) || (x.item == SUMMON && x.done))
        .unwrap()
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn banish_ends_a_summon_within_its_level() {
    let (_dir, mut e) = fixture();
    e.apply_cast(BANISH, PLAYER_REF, STRONG_ATRONACH, None, false);
    assert_eq!(
        e.commander_of(STRONG_ATRONACH),
        Some(SUMMONER),
        "too strong"
    );
    assert!(!summon_effect(&e, STRONG_ATRONACH).done);

    e.apply_cast(BANISH, PLAYER_REF, ATRONACH, None, false);
    assert_eq!(e.commander_of(ATRONACH), None);
    // Only summons: the summoner itself stays.
    e.apply_cast(BANISH, PLAYER_REF, SUMMONER, None, false);
    assert!(!e.is_dead(SUMMONER));
    assert!(e.actor_ref(SUMMONER).is_some());
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn command_takes_a_summon_for_what_is_left_of_it() {
    let (_dir, mut e) = fixture();
    e.apply_cast(COMMAND, PLAYER_REF, STRONG_ATRONACH, None, false);
    assert_eq!(
        e.commander_of(STRONG_ATRONACH),
        Some(SUMMONER),
        "too strong"
    );

    e.apply_cast(COMMAND, PLAYER_REF, ATRONACH, None, false);
    assert_eq!(e.commander_of(ATRONACH), Some(PLAYER_REF));
    let x = summon_effect(&e, ATRONACH);
    assert_eq!(x.caster, Some(PLAYER_REF));
    assert_eq!(x.duration, 60.0, "the summon's own time");
    assert_eq!(x.target, PLAYER_REF);
    // Taken over, it doesn't turn on its new commander.
    assert!(
        e.actor_ref(ATRONACH)
            .unwrap()
            .exec
            .combat
            .as_ref()
            .is_none_or(|c| c.target != PLAYER_REF)
    );
    // Its old summoner's death no longer sends it away.
    e.magic
        .effects
        .retain(|x| x.summoned != Some(STRONG_ATRONACH));
    e.actor_mut(SUMMONER).unwrap().dead = true;
    e.update_magic(0.1);
    assert_eq!(e.commander_of(ATRONACH), Some(PLAYER_REF));
    // A save keeps it the player's.
    let saved = e.magic.effects.clone();
    let next = saved.iter().map(|x| x.id).max().unwrap_or(0);
    e.magic.commanded.clear();
    e.restore_active_effects(saved, next);
    assert_eq!(e.commander_of(ATRONACH), Some(PLAYER_REF));
    // Not a summon: nothing to take.
    e.apply_cast(COMMAND, PLAYER_REF, SUMMONER, None, false);
    assert_eq!(e.commander_of(SUMMONER), None);
}
