//! Critical strikes through the real blow path, generated records only.
//! Engine owns a renderer, so these need a headless GPU adapter.

use super::*;
use crate::engine::CellRuntime;
use esp::writer::{Node, RecordBuilder, write_plugin};

const SWORD: FormId = FormId(0x800);
const EFFECT_SWORD: FormId = FormId(0x801);
const CHANCE: FormId = FormId(0x810);
const DAMAGE: FormId = FormId(0x811);
const ENEMY: FormId = FormId(0x812);
const SPRINT: FormId = FormId(0x813);
const MGEF: FormId = FormId(0x820);
const CRIT_SPELL: FormId = FormId(0x821);
const BOW: FormId = FormId(0x802);
const NPC: FormId = FormId(0x830);
const ACTOR: FormId = FormId(0x831);

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
    let f =
        Fixture(std::env::temp_dir().join(format!("vrm-critical-{}-{stamp}", std::process::id())));
    std::fs::create_dir(&f.0).unwrap();
    // xEdit's SSE `CRDT`: damage, unused, percent mult, flags, unused, effect.
    let weapon = |id, anim: u8, crit: u16, effect: FormId, flags: u8| {
        let mut dnam = [0; 100];
        dnam[0] = anim;
        let mut crdt = [0; 24];
        crdt[..2].copy_from_slice(&crit.to_le_bytes());
        crdt[4..8].copy_from_slice(&1.0f32.to_le_bytes());
        crdt[8] = flags;
        crdt[16..20].copy_from_slice(&effect.0.to_le_bytes());
        RecordBuilder::new(b"WEAP", id)
            .field(b"DNAM", &dnam)
            .field(b"CRDT", &crdt)
    };
    // An entry point section: `point`, `function`, one float; `owner`
    // conditions (tab 0) are `IsSprinting` when `sprint`.
    let perk = |id, point: u8, function: u8, value: f32, sprint: bool| {
        let mut p = RecordBuilder::new(b"PERK", id)
            .field(b"PRKE", &[2, 0, 0])
            .field(b"DATA", &[point, function, 3]);
        if sprint {
            let mut c = [0; 32];
            c[4..8].copy_from_slice(&1.0f32.to_le_bytes());
            c[8..10].copy_from_slice(&568u16.to_le_bytes());
            p = p.field(b"PRKC", &[0]).field(b"CTDA", &c);
        }
        p.field(b"EPFT", &[1])
            .f32(b"EPFD", value)
            .field(b"PRKF", &[])
    };
    let mut mgef = [0; 0x6c];
    mgef[0x40..0x44].copy_from_slice(&crate::magic::archetype::VALUE_MODIFIER.to_le_bytes());
    mgef[0x44..0x48].copy_from_slice(&esp::actor_value::SNEAK.to_le_bytes());
    mgef[0x10..0x14].copy_from_slice(&u32::MAX.to_le_bytes());
    let mut spit = [0; 36];
    spit[8..12].copy_from_slice(&crate::magic::spell_type::SPELL.to_le_bytes());
    let mut efit = [0; 12];
    efit[..4].copy_from_slice(&7.0f32.to_le_bytes());
    efit[8..12].copy_from_slice(&30u32.to_le_bytes());
    let records = [
        Node::top(
            b"WEAP",
            vec![
                weapon(SWORD, 1, 5, FormId::NULL, 0),
                weapon(EFFECT_SWORD, 1, 5, CRIT_SPELL, 0),
                weapon(BOW, 7, 5, FormId::NULL, 0),
            ],
        ),
        Node::top(
            b"PERK",
            vec![
                perk(
                    CHANCE,
                    ep::CALCULATE_MY_CRITICAL_HIT_CHANCE,
                    1,
                    100.0,
                    false,
                ),
                perk(DAMAGE, ep::CALCULATE_MY_CRITICAL_HIT_DAMAGE, 3, 1.5, false),
                perk(ENEMY, ep::MOD_ENEMY_CRITICAL_HIT_CHANCE, 2, 100.0, false),
                perk(SPRINT, ep::CALCULATE_MY_CRITICAL_HIT_CHANCE, 1, 100.0, true),
            ],
        ),
        Node::top(
            b"MGEF",
            vec![RecordBuilder::new(b"MGEF", MGEF).field(b"DATA", &mgef)],
        ),
        Node::top(
            b"SPEL",
            vec![
                RecordBuilder::new(b"SPEL", CRIT_SPELL)
                    .field(b"SPIT", &spit)
                    .u32(b"EFID", MGEF.0)
                    .field(b"EFIT", &efit),
            ],
        ),
        Node::top(b"NPC_", vec![RecordBuilder::new(b"NPC_", NPC)]),
    ];
    std::fs::write(
        f.0.join("Critical.esp"),
        write_plugin(
            0,
            &[],
            "test",
            "synthetic critical strikes",
            0x900,
            &records,
        ),
    )
    .unwrap();
    let mut e = crate::stats::action_tests::headless_engine(&f.0, "Critical.esp");
    let key = crate::render::CellKey::Interior(FormId(0x850));
    let mut actor = ActorRuntime::new(
        ACTOR,
        NPC,
        Arc::new(crate::world::skeleton::Skeleton::default()),
        Mat4::IDENTITY,
        Vec::new(),
        0.0,
    );
    actor.health = 1000.0;
    e.actor_cells.insert(ACTOR, key);
    e.cells
        .entry(key)
        .or_insert_with(CellRuntime::default)
        .actors
        .push(actor);
    (f, e)
}

fn wield(e: &mut Engine, weapon: Option<FormId>) {
    let mut inv = Inventory::default();
    if let Some(w) = weapon {
        inv.add(w, 1);
        inv.equip(&e.lo, w);
    }
    e.inventories.insert(PLAYER_REF, inv);
}

/// The health the player's blow of `damage` takes from the actor.
fn strike(e: &mut Engine, damage: f32) -> f32 {
    let before = e.actor_health(ACTOR).unwrap().0;
    e.hit(ACTOR, PLAYER_REF, damage, false, 0.0, Struck::Hand(RIGHT));
    before - e.actor_health(ACTOR).unwrap().0
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn weapon_critical_data_reads_both_layouts() {
    let (_f, e) = fixture();
    let c = weapon_critical(&e.lo, EFFECT_SWORD).unwrap();
    assert_eq!(c.damage, 5.0);
    assert_eq!(c.effect, Some(CRIT_SPELL));
    assert!(!c.on_death);
    assert_eq!(weapon_critical(&e.lo, SWORD).unwrap().effect, None);
    assert!(weapon_critical(&e.lo, ACTOR).is_none());
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn critical_chance_and_damage_come_from_perk_entry_points() {
    let (_f, mut e) = fixture();
    wield(&mut e, Some(SWORD));
    // No perk, no chance: an ordinary blow.
    assert_eq!(strike(&mut e, 10.0), 10.0);
    assert!(!e.critically_hit.contains(&ACTOR));
    assert_eq!(e.query_stat("Critical Strikes"), 0);

    // A sure chance adds the weapon's critical damage.
    assert!(e.add_perk(PLAYER_REF, CHANCE));
    assert_eq!(strike(&mut e, 10.0), 15.0);
    assert!(e.critically_hit.contains(&ACTOR));
    assert_eq!(e.query_stat("Critical Strikes"), 1);

    // "Calculate My Critical Hit Damage" scales only the extra damage.
    assert!(e.add_perk(PLAYER_REF, DAMAGE));
    assert_eq!(strike(&mut e, 10.0), 17.5);
    assert_eq!(e.query_stat("Critical Strikes"), 2);

    // Without the chance the latest hit is no longer critical.
    e.remove_perk(PLAYER_REF, CHANCE);
    assert_eq!(strike(&mut e, 10.0), 10.0);
    assert!(!e.critically_hit.contains(&ACTOR));

    // The target's perks can make blows against it critical.
    assert!(e.add_perk(ACTOR, ENEMY));
    assert_eq!(strike(&mut e, 10.0), 17.5);
    assert_eq!(e.query_stat("Critical Strikes"), 3);
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn bare_hands_critical_strikes_use_the_blow() {
    let (_f, mut e) = fixture();
    wield(&mut e, None);
    assert!(e.add_perk(PLAYER_REF, CHANCE));
    // Bare hands: the blow's own damage times fCombatUnarmedCritDamageMult
    // (1 without the setting).
    assert_eq!(strike(&mut e, 10.0), 20.0);
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn sprinting_perk_conditions_and_critical_effects() {
    let (_f, mut e) = fixture();
    wield(&mut e, Some(EFFECT_SWORD));
    assert!(e.add_perk(PLAYER_REF, SPRINT));
    // The perk's owner condition is IsSprinting.
    assert_eq!(strike(&mut e, 10.0), 10.0);
    assert!(e.magic.effects.iter().all(|x| x.item != CRIT_SPELL));
    e.player.sprinting = true;
    assert_eq!(strike(&mut e, 10.0), 15.0);
    // The weapon's critical effect landed with the strike.
    assert!(
        e.magic
            .effects
            .iter()
            .any(|x| x.item == CRIT_SPELL && x.target == ACTOR)
    );
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn backstabs_are_one_handed_sneak_attacks_from_behind() {
    let (_f, mut e) = fixture();
    // The actor faces +Y (heading 0).
    e.player.position = Vec3::new(0.0, 100.0, 0.0);
    assert!(!e.is_backstab(ACTOR, PLAYER_REF, Some(SWORD))); // in front of it
    e.player.position = Vec3::new(0.0, -100.0, 0.0);
    assert!(e.is_backstab(ACTOR, PLAYER_REF, Some(SWORD)));
    assert!(!e.is_backstab(ACTOR, PLAYER_REF, None)); // bare hands
    assert!(!e.is_backstab(ACTOR, PLAYER_REF, Some(BOW)));

    // Unseen behind it, a sword's sneak attack counts as both.
    wield(&mut e, Some(SWORD));
    e.player.sneaking = true;
    strike(&mut e, 10.0);
    assert_eq!(e.query_stat("Sneak Attacks"), 1);
    assert_eq!(e.query_stat("Backstabs"), 1);
}
