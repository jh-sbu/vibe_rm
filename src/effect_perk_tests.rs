//! Generated records only; no game files or engine binaries are needed.
use super::*;
use esp::writer::{Node, RecordBuilder, write_plugin};

const PERK: FormId = FormId(0x800);
const EFFECT: FormId = FormId(0x801);
const SHORT_EFFECT: FormId = FormId(0x802);
const ABILITY: FormId = FormId(0x803);

struct Fixture(std::path::PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture() -> (Fixture, Engine) {
    let dir = std::env::temp_dir().join(format!("vrm-effect-perks-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut data = vec![0; 0x98];
    data[0x40..0x44].copy_from_slice(&archetype::SCRIPT.to_le_bytes());
    data[0x88..0x8C].copy_from_slice(&PERK.0.to_le_bytes());
    let records = [
        Node::top(
            b"MGEF",
            vec![
                RecordBuilder::new(b"MGEF", EFFECT).field(b"DATA", &data),
                RecordBuilder::new(b"MGEF", SHORT_EFFECT).field(b"DATA", &data[..0x6C]),
            ],
        ),
        Node::top(
            b"PERK",
            vec![
                RecordBuilder::new(b"PERK", PERK)
                    .field(b"PRKE", &[2, 0, 0])
                    .field(b"DATA", &[crate::perks::ep::MOD_SKILL_USE, 3, 1])
                    .field(b"EPFT", &[1])
                    .f32(b"EPFD", 1.5)
                    .field(b"PRKF", &[])
                    .field(b"PRKE", &[1, 0, 0])
                    .field(b"DATA", &ABILITY.0.to_le_bytes())
                    .field(b"PRKF", &[]),
            ],
        ),
    ];
    std::fs::write(
        dir.join("EffectPerks.esp"),
        write_plugin(0, &[], "test", "effect perks", 0x900, &records),
    )
    .unwrap();
    let engine = crate::stats::action_tests::headless_engine(&dir, "EffectPerks.esp");
    (Fixture(dir), engine)
}

fn effect(id: u64, target: FormId) -> ActiveEffect {
    ActiveEffect {
        id,
        target,
        caster: None,
        item: FormId(0x900 + id as u32),
        index: 0,
        effect: EFFECT,
        magnitude: 0.0,
        duration: 20.0,
        elapsed: 0.0,
        active: false,
        held: [0.0; 2],
        scripted: false,
        done: false,
        quiet: false,
        at: None,
        summoned: None,
        dual: false,
        channelled: false,
    }
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn effect_perks_follow_active_sources_without_changing_permanent_ownership() {
    let (_dir, mut e) = fixture();
    assert_eq!(e.magic_effect(EFFECT).unwrap().perk, PERK);
    assert!(e.magic_effect(SHORT_EFFECT).unwrap().perk.is_null());
    assert!(!e.has_perk(PLAYER_REF, PERK)); // prime the permanent cache
    e.magic.effects.push(effect(1, PLAYER_REF));
    assert!(!e.has_perk(PLAYER_REF, PERK)); // inactive
    e.start_magic_effect(1);
    assert!(e.has_perk(PLAYER_REF, PERK));
    assert!(!e.permanent_perks(PLAYER_REF).iter().any(|p| p.0 == PERK));
    assert!(!e.has_spell(PLAYER_REF, ABILITY)); // avoid self-sustaining abilities
    assert_eq!(
        e.perk_entry_point(crate::perks::ep::MOD_SKILL_USE, PLAYER_REF, &[], 10.0),
        15.0
    );
    // The real skill-use hook sees the temporary entry point as well.
    e.use_skill(av::ONE_HANDED, 10.0);
    let boosted = e.skills.xp[(av::ONE_HANDED - av::FIRST_SKILL) as usize];
    e.stop_magic_effect(1);
    assert!(!e.has_perk(PLAYER_REF, PERK));
    e.skills.xp.fill(0.0);
    e.use_skill(av::ONE_HANDED, 10.0);
    let plain = e.skills.xp[(av::ONE_HANDED - av::FIRST_SKILL) as usize];
    assert!(plain > 0.0 && (boosted - plain * 1.5).abs() < 1e-3);
    e.start_magic_effect(1);
    e.magic.effects.push(effect(2, PLAYER_REF));
    e.start_magic_effect(2);
    assert_eq!(
        e.actor_perks(PLAYER_REF)
            .iter()
            .filter(|p| p.0 == PERK)
            .count(),
        1
    );
    assert_eq!(
        e.perk_entry_point(crate::perks::ep::MOD_SKILL_USE, PLAYER_REF, &[], 10.0),
        15.0
    );
    e.finish_magic_effect(1);
    assert!(e.has_perk(PLAYER_REF, PERK)); // second source remains
    // Explicit addition during a temporary grant must survive its expiration.
    assert!(e.add_perk(PLAYER_REF, PERK));
    assert!(e.has_spell(PLAYER_REF, ABILITY));
    e.finish_magic_effect(2);
    assert!(e.has_perk(PLAYER_REF, PERK));
    e.remove_perk(PLAYER_REF, PERK);
    assert!(!e.has_perk(PLAYER_REF, PERK));
    e.magic.effects.push(effect(3, PLAYER_REF));
    e.start_magic_effect(3);
    e.remove_perk(PLAYER_REF, PERK);
    assert!(e.has_perk(PLAYER_REF, PERK)); // RemovePerk affects permanent sources
    e.magic.effects.last_mut().unwrap().done = true;
    assert!(!e.has_perk(PLAYER_REF, PERK)); // done even if still active
    let npc = FormId(0x123);
    e.magic.effects.push(effect(4, npc));
    e.start_magic_effect(4);
    assert!(!e.has_perk(npc, PERK));
}
