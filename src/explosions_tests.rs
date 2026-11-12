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

/// A free ball of `mass` kg owned by `owner` at `at`.
fn ball(e: &mut Engine, owner: FormId, at: Vec3, mass: f32) -> rapier3d::prelude::RigidBodyHandle {
    use rapier3d::prelude::*;
    let h = e
        .physics
        .world
        .insert_body(RigidBodyBuilder::dynamic().translation(at));
    let c = e
        .physics
        .world
        .insert_collider(ColliderBuilder::ball(5.0).mass(mass).build(), Some(h));
    e.physics.owners.insert(c, owner);
    h
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn a_blast_pushes_what_moves_away_from_it_and_lifts_it_by_its_offset() {
    let extra = vec![Node::top(
        b"EXPL",
        vec![RecordBuilder::new(b"EXPL", FormId(0x812)).field(b"DATA", &{
            let mut d = expl_data(5.0, 0.0, 300.0, 0x11);
            d[40..44].copy_from_slice(&0.5f32.to_le_bytes());
            d
        })],
    )];
    let (_f, mut e) = fixture("force", extra);
    let near = ball(&mut e, FormId(0x850), Vec3::new(0.0, 100.0, 0.0), 2.0);
    let far = ball(&mut e, FormId(0x851), Vec3::new(0.0, 400.0, 0.0), 2.0);
    e.explode(Detonation::new(FormId(0x812), Vec3::ZERO));
    let v = |e: &Engine, h| e.physics.world.bodies.get(h).unwrap().linvel();
    let pushed = v(&e, near);
    // Away along +Y, lifted by the vertical offset mult, no faster than the cap.
    assert!(
        pushed.y > 0.0 && pushed.z > 0.0 && pushed.x.abs() < 1e-3,
        "{pushed:?}"
    );
    assert!((pushed.z / pushed.y - 0.5).abs() < 1e-3, "{pushed:?}");
    assert!(pushed.length() <= MAX_PUSH_SPEED + 1e-2, "{pushed:?}");
    assert_eq!(v(&e, far), Vec3::ZERO);
}

#[test]
fn knockdowns_always_or_by_the_health_left() {
    use flag::{KNOCK_DOWN_ALWAYS as ALWAYS, KNOCK_DOWN_BY_FORMULA as FORMULA};
    assert!(knocks_down(ALWAYS, 50.0, 50.0, 0.5));
    assert!(knocks_down(FORMULA, 25.0, 50.0, 0.5));
    assert!(!knocks_down(FORMULA, 26.0, 50.0, 0.5));
    assert!(!knocks_down(0, 0.0, 50.0, 0.5));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn a_blasts_light_fades_and_its_image_space_reaches_the_player_within_its_radius() {
    const FLASH: FormId = FormId(0x813);
    const LIGHT: FormId = FormId(0x860);
    const IMOD: FormId = FormId(0x861);
    let mut light = vec![0u8; 24];
    light[4..8].copy_from_slice(&256u32.to_le_bytes());
    light[8..11].copy_from_slice(&[255, 128, 0]);
    let mut d = expl_data(0.0, 0.0, 100.0, 0x1);
    d[0..4].copy_from_slice(&LIGHT.0.to_le_bytes());
    d[36..40].copy_from_slice(&1024.0f32.to_le_bytes());
    let extra = vec![
        Node::top(
            b"LIGH",
            vec![RecordBuilder::new(b"LIGH", LIGHT).field(b"DATA", &light)],
        ),
        Node::top(b"IMAD", vec![RecordBuilder::new(b"IMAD", IMOD)]),
        Node::top(
            b"EXPL",
            vec![
                RecordBuilder::new(b"EXPL", FLASH)
                    .u32(b"MNAM", IMOD.0)
                    .field(b"DATA", &d),
            ],
        ),
    ];
    let (_f, mut e) = fixture("show", extra);
    let modified = |e: &Engine| e.imagespace.active.iter().any(|a| a.imad == IMOD);
    // The player 5000 away: outside its IS radius.
    e.explode(Detonation::new(FLASH, Vec3::ZERO));
    assert!(!modified(&e));
    let lights: Vec<_> = e.explosion_lights().collect();
    assert_eq!(lights.len(), 1);
    assert_eq!(lights[0].pos_radius[3], 256.0);
    e.update_explosions(MIN_FLASH / 2.0);
    let half = e.explosion_lights().next().unwrap().color[0];
    assert!((half - lights[0].color[0] / 2.0).abs() < 1e-3, "{half}");
    e.update_explosions(MIN_FLASH);
    assert_eq!(e.explosion_lights().count(), 0);
    // Within it.
    e.explode(Detonation::new(FLASH, Vec3::new(0.0, 4500.0, 0.0)));
    assert!(modified(&e));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn place_at_me_sets_an_explosion_off_and_its_reference_goes_once_it_is_over() {
    const CHAIN: FormId = FormId(0x814);
    let mut d = expl_data(0.0, 20.0, 300.0, 0x11);
    d[16..20].copy_from_slice(&GAS.0.to_le_bytes());
    let extra = vec![Node::top(
        b"EXPL",
        vec![RecordBuilder::new(b"EXPL", CHAIN).field(b"DATA", &d)],
    )];
    let (_f, mut e) = fixture("placeatme", extra);
    let at = e.object_value(NEAR);
    let chain = e.object_value(CHAIN);
    let made = match crate::script::natives::call(
        &mut e,
        "objectreference",
        "placeatme",
        Some(&at),
        &[chain],
    ) {
        papyrus::NativeResult::Value(v) => v,
        _ => panic!("PlaceAtMe gave no value"),
    };
    let r = FormId(made.as_form().expect("a reference"));
    assert_eq!(e.base_of(r), Some(CHAIN));
    // Placed at the actor: it takes the blast (no reference is spared).
    assert_eq!(health(&e, NEAR), 30.0);
    // The gas it leaves goes off next frame.
    let poisoned = |e: &Engine| {
        e.magic
            .effects
            .iter()
            .any(|x| x.target == NEAR && x.item == POISON)
    };
    assert!(!poisoned(&e));
    e.update_explosions(0.01);
    assert!(poisoned(&e));
    assert!(e.created(r).is_some());
    e.update_explosions(MIN_FLASH);
    assert!(e.created(r).is_none());
}
