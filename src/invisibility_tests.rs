//! Real action paths, generated records only. Engine needs a headless adapter.

use super::*;
use crate::ai::combat::{Side, Struck, Swing};
use crate::engine::{CellRuntime, PLAYER_REF};
use crate::magic::{flags, spell_type};
use crate::spells::{CONCENTRATION, RIGHT};
use esp::actor_value as av;
use esp::writer::{Node, RecordBuilder, write_plugin};
use glam::{Mat4, Vec3};
use std::sync::Arc;

const INVIS: FormId = FormId(0x810);
const INVIS_OTHER: FormId = FormId(0x811);
const ORDINARY: FormId = FormId(0x812);
const CHANNEL: FormId = FormId(0x813);
const ABILITY: FormId = FormId(0x814);
const BOW: FormId = FormId(0x820);
const AMMO: FormId = FormId(0x821);
const NPC: FormId = FormId(0x830);
const ACTOR: FormId = FormId(0x831);
const ACTIVATE: FormId = FormId(0x841);

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
    let dir = Fixture(
        std::env::temp_dir().join(format!("vrm-invisibility-{}-{stamp}", std::process::id())),
    );
    std::fs::create_dir(&dir.0).unwrap();
    let effect = |id, archetype: u32, value: u32| {
        let mut data = [0; 0x6c];
        data[..4].copy_from_slice(&flags::RECOVER.to_le_bytes());
        data[0x10..0x14].copy_from_slice(&u32::MAX.to_le_bytes());
        data[0x40..0x44].copy_from_slice(&archetype.to_le_bytes());
        data[0x44..0x48].copy_from_slice(&value.to_le_bytes());
        RecordBuilder::new(b"MGEF", id).field(b"DATA", &data)
    };
    let spell = |id: FormId, effects: &[u32], casting: u32, kind: u32| {
        let mut spit = [0; 36];
        spit[..4].copy_from_slice(&10u32.to_le_bytes());
        spit[4..8].copy_from_slice(&1u32.to_le_bytes()); // authored cost
        spit[8..12].copy_from_slice(&kind.to_le_bytes());
        spit[12..16].copy_from_slice(&0.5f32.to_le_bytes());
        spit[16..20].copy_from_slice(&casting.to_le_bytes());
        let mut out = RecordBuilder::new(b"SPEL", id).field(b"SPIT", &spit);
        for &effect in effects {
            let mut efit = [0; 12];
            efit[..4].copy_from_slice(&7.0f32.to_le_bytes());
            efit[8..12].copy_from_slice(&30u32.to_le_bytes());
            out = out.u32(b"EFID", effect).field(b"EFIT", &efit);
        }
        out
    };
    let mut bow_data = [0; 100];
    bow_data[0] = 7;
    bow_data[4..8].copy_from_slice(&1.0f32.to_le_bytes());
    let mut ammo_data = [0; 12];
    ammo_data[4..8].copy_from_slice(&4u32.to_le_bytes()); // non-bolt
    ammo_data[8..12].copy_from_slice(&5.0f32.to_le_bytes());
    let records = [
        Node::top(
            b"MGEF",
            vec![
                effect(FormId(0x800), archetype::INVISIBILITY, av::INVISIBILITY),
                effect(FormId(0x801), archetype::VALUE_MODIFIER, av::SNEAK),
            ],
        ),
        Node::top(
            b"SPEL",
            vec![
                spell(INVIS, &[0x800, 0x801], 1, spell_type::SPELL),
                spell(INVIS_OTHER, &[0x800], 1, spell_type::SPELL),
                spell(ORDINARY, &[0x801], 1, spell_type::SPELL),
                spell(CHANNEL, &[0x801], CONCENTRATION, spell_type::SPELL),
                spell(ABILITY, &[0x800], 0, spell_type::ABILITY),
            ],
        ),
        Node::top(
            b"WEAP",
            vec![RecordBuilder::new(b"WEAP", BOW).field(b"DNAM", &bow_data)],
        ),
        Node::top(
            b"AMMO",
            vec![RecordBuilder::new(b"AMMO", AMMO).field(b"DATA", &ammo_data)],
        ),
        Node::top(b"NPC_", vec![RecordBuilder::new(b"NPC_", NPC)]),
        Node::top(
            b"ACHR",
            vec![RecordBuilder::new(b"ACHR", ACTOR).u32(b"NAME", NPC.0)],
        ),
        Node::top(b"ACTI", vec![RecordBuilder::new(b"ACTI", FormId(0x840))]),
        Node::top(
            b"REFR",
            vec![RecordBuilder::new(b"REFR", ACTIVATE).u32(b"NAME", 0x840)],
        ),
    ];
    std::fs::write(
        dir.0.join("Invisibility.esp"),
        write_plugin(
            0,
            &[],
            "test",
            "synthetic invisibility actions",
            0x900,
            &records,
        ),
    )
    .unwrap();
    let mut engine = crate::stats::action_tests::headless_engine(&dir.0, "Invisibility.esp");
    engine.set_actor_value(PLAYER_REF, av::MAGICKA, 1000.0);
    (dir, engine)
}

fn invisible(engine: &Engine, actor: FormId) -> bool {
    engine.actor_value(actor, av::INVISIBILITY) > 0.0
}

fn cloak(engine: &mut Engine, actor: FormId) {
    assert!(engine.apply_item(INVIS, Some(actor), actor));
    assert!(invisible(engine, actor));
}

fn load_actor(engine: &mut Engine, pos: Vec3) {
    let key = crate::render::CellKey::Interior(FormId(0x850));
    let mut actor = crate::ai::ActorRuntime::new(
        ACTOR,
        NPC,
        Arc::new(crate::world::skeleton::Skeleton::default()),
        Mat4::from_translation(pos),
        Vec::new(),
        0.0,
    );
    actor.health = 1000.0;
    engine.actor_cells.insert(ACTOR, key);
    engine
        .cells
        .entry(key)
        .or_insert_with(CellRuntime::default)
        .actors
        .push(actor);
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn casting_and_activation_end_only_the_actors_timed_invisibility() {
    let (_dir, mut engine) = fixture();
    cloak(&mut engine, PLAYER_REF);
    cloak(&mut engine, ACTOR);
    let sneak = engine.actor_value(PLAYER_REF, av::SNEAK);

    // Passive application and someone else's cast don't expose the player.
    assert!(engine.apply_item(ORDINARY, Some(ACTOR), PLAYER_REF));
    assert!(engine.cast_item(ORDINARY, Some(ACTOR), ACTOR));
    assert!(invisible(&engine, PLAYER_REF));
    assert!(!invisible(&engine, ACTOR));
    assert!(!engine.cast_item(FormId(0xBAD), Some(PLAYER_REF), PLAYER_REF));
    assert!(invisible(&engine, PLAYER_REF));

    // A canceled charge remains invisible. The actual cast, not pressing an
    // attack button to ready a spell, is the interruption boundary.
    engine.equip_spell(PLAYER_REF, ORDINARY, RIGHT);
    assert!(engine.player_cast_press(RIGHT));
    assert!(engine.player_cast_release(RIGHT));
    assert!(invisible(&engine, PLAYER_REF));
    engine.cast_from(PLAYER_REF, ORDINARY, 1.0, Vec3::ZERO, Vec3::Y, false);
    assert!(!invisible(&engine, PLAYER_REF));
    assert_eq!(engine.actor_value(PLAYER_REF, av::SNEAK), sneak + 7.0);
    assert!(
        engine
            .magic
            .effects
            .iter()
            .any(|x| { x.item == INVIS && x.effect == FormId(0x801) && !x.done && x.active })
    );

    cloak(&mut engine, PLAYER_REF);
    let old = engine
        .magic
        .effects
        .iter()
        .find(|x| x.item == INVIS && !x.done)
        .unwrap()
        .id;
    assert!(engine.cast_item(INVIS_OTHER, Some(PLAYER_REF), PLAYER_REF));
    assert!(invisible(&engine, PLAYER_REF));
    assert!(
        !engine
            .magic
            .effects
            .iter()
            .find(|x| x.id == old)
            .unwrap()
            .done
    );
    // Channel deliveries also interrupt invisibility acquired mid-channel.
    engine.cast_from(PLAYER_REF, CHANNEL, 0.25, Vec3::ZERO, Vec3::Y, false);
    assert!(!invisible(&engine, PLAYER_REF));
    cloak(&mut engine, PLAYER_REF);
    engine.cast_from(PLAYER_REF, CHANNEL, 0.25, Vec3::ZERO, Vec3::Y, false);
    assert!(!invisible(&engine, PLAYER_REF));

    cloak(&mut engine, PLAYER_REF);
    engine.look_target = None;
    engine.activate().unwrap();
    assert!(invisible(&engine, PLAYER_REF));
    engine.look_target = Some((ACTIVATE, "Synthetic activator".into()));
    engine.disabled_controls.activate = true;
    engine.activate().unwrap();
    assert!(invisible(&engine, PLAYER_REF));
    engine.disabled_controls.activate = false;
    engine.activate().unwrap();
    assert!(!invisible(&engine, PLAYER_REF));
    cloak(&mut engine, PLAYER_REF);
    engine.activate_ref(ACTIVATE, Some(ACTOR), true);
    assert!(invisible(&engine, PLAYER_REF));
    engine.activate_ref(ACTIVATE, Some(PLAYER_REF), true);
    assert!(!invisible(&engine, PLAYER_REF));

    // Constant sources are explicitly left to the ability reconciler.
    assert!(engine.apply_item(ABILITY, Some(PLAYER_REF), PLAYER_REF));
    cloak(&mut engine, PLAYER_REF);
    engine.break_invisibility(PLAYER_REF);
    assert!(invisible(&engine, PLAYER_REF));
    assert!(
        engine
            .magic
            .effects
            .iter()
            .any(|x| x.item == ABILITY && !x.done)
    );
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn attacks_break_on_a_miss_and_arrows_break_at_release_not_impact() {
    let (_dir, mut engine) = fixture();
    cloak(&mut engine, PLAYER_REF);
    engine.disabled_controls.fighting = true;
    engine.player_attack(Side::Right, false);
    assert!(invisible(&engine, PLAYER_REF));
    engine.disabled_controls.fighting = false;
    engine.player_attack(Side::Right, false); // no body graph, no target: a miss
    assert!(!invisible(&engine, PLAYER_REF));

    load_actor(&mut engine, Vec3::new(0.0, 500.0, 0.0));
    cloak(&mut engine, ACTOR);
    engine.resolve_swings(vec![Swing {
        attacker: ACTOR,
        target: PLAYER_REF,
        attack: None,
        hand: RIGHT,
        pos: Vec3::new(0.0, 500.0, 0.0),
        chest: 60.0,
        heading: 0.0,
    }]);
    assert!(!invisible(&engine, ACTOR)); // NPC also misses, out of reach

    let mut inventory = crate::world::inventory::Inventory::default();
    inventory.add(BOW, 1);
    inventory.add(AMMO, 3);
    inventory.equip(&engine.lo, BOW);
    engine.inventories.insert(PLAYER_REF, inventory);
    cloak(&mut engine, PLAYER_REF);
    assert!(engine.player_begin_bow_draw()); // no graph fallback
    assert!(invisible(&engine, PLAYER_REF));
    engine.player_loose(0.01); // not nocked
    assert!(invisible(&engine, PLAYER_REF));
    engine.player_loose(2.0);
    assert_eq!(engine.projectiles.len(), 1);
    assert!(!invisible(&engine, PLAYER_REF));
    cloak(&mut engine, PLAYER_REF);
    engine.hit(
        ACTOR,
        PLAYER_REF,
        1.0,
        false,
        0.0,
        Struck::Shot(FormId::NULL, None),
    );
    assert!(invisible(&engine, PLAYER_REF)); // delayed old shot can't dispel the recast
}
