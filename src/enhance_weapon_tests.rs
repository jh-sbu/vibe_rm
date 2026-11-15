//! Enhance Weapon effects (archetype 39) on generated records. Engine needs a
//! headless adapter.

use super::*;
use crate::ai::combat::Struck;
use crate::engine::CellRuntime;
use crate::magic::{archetype, flags, spell_type};
use crate::world::inventory::RIGHT;
use esp::actor_value as av;
use esp::writer::{Node, RecordBuilder, write_plugin};
use glam::{Mat4, Vec3};
use std::sync::Arc;

/// Self, +1.3 weapon speed for 15 s, lending `FURY`.
const SHOUT: FormId = FormId(0x810);
const ENHANCE_EFFECT: FormId = FormId(0x800);
/// The lent enchantment: 10 damage to health.
const FURY: FormId = FormId(0x820);
const FURY_EFFECT: FormId = FormId(0x801);
/// An enchantment of a sword's own.
const OWN: FormId = FormId(0x821);
const SWORD: FormId = FormId(0x830);
const BOW: FormId = FormId(0x831);
const ENCHANTED_SWORD: FormId = FormId(0x832);
const NPC: FormId = FormId(0x840);
const ACTOR: FormId = FormId(0x841);

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
        Fixture(std::env::temp_dir().join(format!("vrm-enhance-{}-{stamp}", std::process::id())));
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
    let efit = |magnitude: f32, duration: u32| {
        let mut efit = [0; 12];
        efit[..4].copy_from_slice(&magnitude.to_le_bytes());
        efit[8..12].copy_from_slice(&duration.to_le_bytes());
        efit
    };
    let mut spit = [0; 36];
    spit[8..12].copy_from_slice(&spell_type::SPELL.to_le_bytes());
    spit[16..20].copy_from_slice(&1u32.to_le_bytes());
    let ench = |id| {
        let mut enit = [0; 36];
        enit[16..20].copy_from_slice(&1u32.to_le_bytes()); // fire and forget
        enit[20..24].copy_from_slice(&1u32.to_le_bytes()); // contact
        RecordBuilder::new(b"ENCH", id)
            .field(b"ENIT", &enit)
            .u32(b"EFID", FURY_EFFECT.0)
            .field(b"EFIT", &efit(10.0, 0))
    };
    let weapon = |id: FormId, anim: u8, enchantment: Option<FormId>| {
        let mut dnam = [0u8; 100];
        dnam[0] = anim;
        dnam[4..8].copy_from_slice(&1.0f32.to_le_bytes());
        let mut w = RecordBuilder::new(b"WEAP", id).field(b"DNAM", &dnam);
        if let Some(e) = enchantment {
            w = w.u32(b"EITM", e.0);
        }
        w
    };
    let records = [
        Node::top(
            b"MGEF",
            vec![
                effect(
                    ENHANCE_EFFECT,
                    archetype::ENHANCE_WEAPON,
                    flags::RECOVER,
                    av::WEAPON_SPEED_MULT,
                    FURY.0,
                ),
                effect(
                    FURY_EFFECT,
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
                RecordBuilder::new(b"SPEL", SHOUT)
                    .field(b"SPIT", &spit)
                    .u32(b"EFID", ENHANCE_EFFECT.0)
                    .field(b"EFIT", &efit(1.3, 15)),
            ],
        ),
        Node::top(b"ENCH", vec![ench(FURY), ench(OWN)]),
        Node::top(
            b"WEAP",
            vec![
                weapon(SWORD, 1, None),
                weapon(BOW, 7, None),
                weapon(ENCHANTED_SWORD, 1, Some(OWN)),
            ],
        ),
        Node::top(b"NPC_", vec![RecordBuilder::new(b"NPC_", NPC)]),
        Node::top(
            b"ACHR",
            vec![RecordBuilder::new(b"ACHR", ACTOR).u32(b"NAME", NPC.0)],
        ),
    ];
    std::fs::write(
        dir.0.join("Enhance.esp"),
        write_plugin(
            0,
            &[],
            "test",
            "synthetic enhanced weapons",
            0x900,
            &records,
        ),
    )
    .unwrap();
    let mut e = crate::stats::action_tests::headless_engine(&dir.0, "Enhance.esp");
    let key = crate::render::CellKey::Interior(FormId(0x860));
    let mut actor = crate::ai::ActorRuntime::new(
        ACTOR,
        NPC,
        Arc::new(crate::world::skeleton::Skeleton::default()),
        Mat4::from_translation(Vec3::X * 100.0),
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
fn an_enhanced_weapon_swings_faster_and_lends_its_enchantment_while_it_lasts() {
    let (_dir, mut e) = fixture();
    let speed = e.actor_value(PLAYER_REF, av::WEAPON_SPEED_MULT);
    assert_eq!(e.enhanced_weapon(PLAYER_REF, SWORD), None);
    assert!(e.apply_cast(SHOUT, PLAYER_REF, PLAYER_REF, None, false));
    assert!((e.actor_value(PLAYER_REF, av::WEAPON_SPEED_MULT) - (speed + 1.3)).abs() < 1e-5);
    assert_eq!(e.enhanced_weapon(PLAYER_REF, SWORD), Some(FURY));
    assert_eq!(e.enhanced_weapon(PLAYER_REF, BOW), None, "not bows");
    assert_eq!(e.enhanced_weapon(ACTOR, SWORD), None, "the caster's own");

    // A blow with the sword carries it; one with its own enchantment, its own.
    e.add_item(PLAYER_REF, SWORD, 1);
    e.equip_item(PLAYER_REF, SWORD, true).unwrap();
    e.hit(ACTOR, PLAYER_REF, 0.0, false, 0.0, Struck::Hand(RIGHT));
    let lent = |e: &Engine, item| {
        e.magic
            .effects
            .iter()
            .any(|x| x.target == ACTOR && x.item == item)
    };
    assert!(lent(&e, FURY));
    e.magic.effects.retain(|x| x.target != ACTOR);
    e.add_item(PLAYER_REF, ENCHANTED_SWORD, 1);
    e.equip_item(PLAYER_REF, ENCHANTED_SWORD, true).unwrap();
    e.hit(ACTOR, PLAYER_REF, 0.0, false, 0.0, Struck::Hand(RIGHT));
    assert!(!lent(&e, FURY), "an enchanted weapon keeps its own");

    e.update_magic(16.0);
    assert!((e.actor_value(PLAYER_REF, av::WEAPON_SPEED_MULT) - speed).abs() < 1e-5);
    assert_eq!(e.enhanced_weapon(PLAYER_REF, SWORD), None);
}
