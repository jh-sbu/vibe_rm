//! Explosions going off among synthetic actors and references; generated
//! records only. Engine owns a renderer, so these need a headless GPU adapter.

use std::sync::Arc;

use esp::FormId;
use esp::actor_value as av;
use esp::writer::{Node, RecordBuilder, write_plugin};
use glam::{Mat4, Vec3};
use papyrus::{ObjectId, Value};

use super::*;
use crate::ai::ActorRuntime;
use crate::ai::combat::CombatStats;
use crate::magic::{archetype, flags};

const NPC: FormId = FormId(0x800);
const NEAR: FormId = FormId(0x801);
const FAR: FormId = FormId(0x802);
const SOURCE: FormId = FormId(0x803);
const BLAST: FormId = FormId(0x810);
const GAS: FormId = FormId(0x811);
const POISON: FormId = FormId(0x820);
const POISON_EFFECT: FormId = FormId(0x821);
const URN: FormId = FormId(0x830);
const URN_REF: FormId = FormId(0x831);
const CELL: FormId = FormId(0x840);

pub(super) struct Fixture(std::path::PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `EXPL` `DATA`: no forms, then force, damage, radius, IS radius, vertical
/// offset mult, flags, sound level.
pub(super) fn expl_data(force: f32, damage: f32, radius: f32, flags: u32) -> Vec<u8> {
    let mut d = vec![0u8; 24];
    for v in [force, damage, radius, 0.0, 0.0] {
        d.extend_from_slice(&v.to_le_bytes());
    }
    d.extend_from_slice(&flags.to_le_bytes());
    d.extend_from_slice(&1u32.to_le_bytes());
    d
}

/// A Value Modifier on Health with the given effect flags (`MGEF` `DATA`).
fn health_effect(effect_flags: u32) -> [u8; 0x6c] {
    let mut data = [0; 0x6c];
    data[0..4].copy_from_slice(&effect_flags.to_le_bytes());
    data[0x40..0x44].copy_from_slice(&archetype::VALUE_MODIFIER.to_le_bytes());
    data[0x44..0x48].copy_from_slice(&av::HEALTH.to_le_bytes());
    data
}

pub(super) fn fixture(name: &str, extra: Vec<Node>) -> (Fixture, Engine) {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let f = Fixture(std::env::temp_dir().join(format!(
        "vrm-explosions-{name}-{}-{stamp}",
        std::process::id()
    )));
    std::fs::create_dir(&f.0).unwrap();
    let mut efit = [0u8; 12];
    efit[0..4].copy_from_slice(&7.0f32.to_le_bytes());
    // A destructible activator: 10 health, one stage at 0% that destroys it.
    let mut dest = vec![0u8; 8];
    dest[0..4].copy_from_slice(&10u32.to_le_bytes());
    dest[4] = 1;
    let mut dstd = vec![0u8; 20];
    dstd[1] = 0;
    dstd[2] = 1;
    dstd[3] = 0x04;
    let mut records = vec![
        Node::top(
            b"NPC_",
            vec![RecordBuilder::new(b"NPC_", NPC).field(b"ACBS", &[0u8; 24])],
        ),
        Node::top(
            b"ACHR",
            vec![
                RecordBuilder::new(b"ACHR", NEAR).form(b"NAME", NPC),
                RecordBuilder::new(b"ACHR", FAR).form(b"NAME", NPC),
                RecordBuilder::new(b"ACHR", SOURCE).form(b"NAME", NPC),
            ],
        ),
        Node::top(
            b"MGEF",
            vec![
                RecordBuilder::new(b"MGEF", POISON_EFFECT)
                    .field(b"DATA", &health_effect(flags::HOSTILE | flags::DETRIMENTAL)),
            ],
        ),
        Node::top(
            b"ENCH",
            vec![
                RecordBuilder::new(b"ENCH", POISON)
                    .field(b"ENIT", &[0u8; 36])
                    .u32(b"EFID", POISON_EFFECT.0)
                    .field(b"EFIT", &efit),
            ],
        ),
        Node::top(
            b"EXPL",
            vec![
                RecordBuilder::new(b"EXPL", BLAST)
                    .field(b"DATA", &expl_data(0.0, 20.0, 300.0, 0x11)),
                RecordBuilder::new(b"EXPL", GAS)
                    .u32(b"EITM", POISON.0)
                    .field(b"DATA", &expl_data(0.0, 0.0, 300.0, 0x11)),
            ],
        ),
        Node::top(
            b"ACTI",
            vec![
                RecordBuilder::new(b"ACTI", URN)
                    .field(b"DEST", &dest)
                    .field(b"DSTD", &dstd)
                    .field(b"DSTF", &[]),
            ],
        ),
        Node::top(
            b"REFR",
            vec![
                RecordBuilder::new(b"REFR", URN_REF)
                    .form(b"NAME", URN)
                    .field(b"DATA", &{
                        let mut d = vec![0u8; 24];
                        d[0..4].copy_from_slice(&150.0f32.to_le_bytes());
                        d
                    }),
            ],
        ),
    ];
    records.extend(extra);
    std::fs::write(
        f.0.join("Explosions.esp"),
        write_plugin(0, &[], "test", "synthetic explosions", 0x900, &records),
    )
    .unwrap();
    let mut e = crate::stats::action_tests::headless_engine(&f.0, "Explosions.esp");
    // The player far away, out of every blast.
    e.player.position = Vec3::new(0.0, 5000.0, 0.0);
    place(&mut e, NEAR, Vec3::new(100.0, 0.0, 0.0));
    place(&mut e, FAR, Vec3::new(1000.0, 0.0, 0.0));
    place(&mut e, SOURCE, Vec3::new(-50.0, 0.0, 0.0));
    let key = crate::render::CellKey::Interior(CELL);
    e.cells.entry(key).or_default().refs.push(URN_REF);
    (f, e)
}

/// Put an actor of 50 health in the loaded test interior.
pub(super) fn place(e: &mut Engine, r: FormId, at: Vec3) {
    let key = crate::render::CellKey::Interior(CELL);
    let mut a = ActorRuntime::new(
        r,
        NPC,
        Arc::new(crate::world::skeleton::Skeleton::default()),
        Mat4::from_translation(at),
        Vec::new(),
        0.0,
    );
    a.stats = Arc::new(CombatStats {
        max_health: 50.0,
        ..CombatStats::default()
    });
    a.exec.health = 50.0;
    e.actor_cells.insert(r, key);
    e.cells.entry(key).or_default().actors.push(a);
}

pub(super) fn health(e: &Engine, r: FormId) -> f32 {
    e.actor_ref(r).unwrap().exec.health
}

fn hits_on(e: &Engine, r: FormId) -> Vec<Vec<Value>> {
    e.scripts
        .pending_events
        .iter()
        .filter(|(obj, event, _)| *obj == ObjectId::Form(r.0) && event == "OnHit")
        .map(|(_, _, args)| args.clone())
        .collect()
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn a_blast_hurts_and_hits_what_is_within_its_radius_but_its_source() {
    let (_f, mut e) = fixture("damage", vec![]);
    let mut d = Detonation::new(BLAST, Vec3::ZERO);
    d.source = Some(SOURCE);
    e.explode(d);
    assert_eq!(health(&e, NEAR), 30.0);
    assert_eq!(health(&e, FAR), 50.0);
    assert_eq!(health(&e, SOURCE), 50.0);
    let hits = hits_on(&e, NEAR);
    assert_eq!(hits.len(), 1);
    // No owner: nobody's the aggressor; the explosion is the source.
    assert!(matches!(hits[0][0], Value::None));
    assert_eq!(hits[0][1], e.object_value(BLAST));
    assert!(hits_on(&e, FAR).is_empty());
    // The urn 150 away takes the damage and is destroyed.
    assert!(e.is_destroyed(URN_REF));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn an_owners_blast_is_their_blow_and_their_noise() {
    let (_f, mut e) = fixture("owner", vec![]);
    let mut d = Detonation::new(BLAST, Vec3::ZERO);
    d.owner = Some(SOURCE);
    d.source = Some(SOURCE);
    e.explode(d);
    assert_eq!(hits_on(&e, NEAR)[0][0], e.object_value(SOURCE));
    assert_eq!(
        e.actor_ref(NEAR)
            .and_then(|a| a.exec.combat.as_ref())
            .map(|c| c.target),
        Some(SOURCE)
    );
    assert!(e.noise_of(SOURCE).is_some_and(|n| n.at == Some(Vec3::ZERO)));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn a_blasts_enchantment_lands_on_those_it_reaches() {
    let (_f, mut e) = fixture("enchantment", vec![]);
    e.explode(Detonation::new(GAS, Vec3::ZERO));
    let on = |e: &Engine, r: FormId| {
        e.magic
            .effects
            .iter()
            .any(|x| x.target == r && x.item == POISON)
    };
    assert!(on(&e, NEAR));
    assert!(!on(&e, FAR));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn blasts_set_off_by_a_blast_wait_for_the_next_frame() {
    let (_f, mut e) = fixture("queue", vec![]);
    e.explosions.going_off = true;
    e.explode(Detonation::new(BLAST, Vec3::ZERO));
    assert_eq!(health(&e, NEAR), 50.0);
    e.explosions.going_off = false;
    e.update_explosions(0.1);
    assert_eq!(health(&e, NEAR), 30.0);
    assert!(e.explosions.queue.is_empty());
}
