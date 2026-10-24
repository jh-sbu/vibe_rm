//! Factions' runtime standing, using generated records only. Engine owns a
//! renderer, so these need a headless GPU adapter.

use std::sync::Arc;

use esp::FormId;
use esp::writer::{Node, RecordBuilder, write_plugin};

use crate::ai::ActorRuntime;
use crate::ai::combat::CombatStats;
use crate::engine::Engine;

/// Bandits list the guards as enemies (modifier -10); the guards list nobody.
const BANDITS: FormId = FormId(0x810);
const GUARDS: FormId = FormId(0x811);
const TOWN: FormId = FormId(0x812);

struct Fixture(std::path::PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn relation(other: FormId, modifier: i32, combat: u32) -> [u8; 12] {
    let mut x = [0u8; 12];
    x[0..4].copy_from_slice(&other.0.to_le_bytes());
    x[4..8].copy_from_slice(&modifier.to_le_bytes());
    x[8..12].copy_from_slice(&combat.to_le_bytes());
    x
}

fn fixture(name: &str) -> (Fixture, Engine) {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let f = Fixture(std::env::temp_dir().join(format!(
        "vrm-factions-{name}-{}-{stamp}",
        std::process::id()
    )));
    std::fs::create_dir(&f.0).unwrap();
    let records = [Node::top(
        b"FACT",
        vec![
            RecordBuilder::new(b"FACT", BANDITS).field(b"XNAM", &relation(GUARDS, -10, 1)),
            RecordBuilder::new(b"FACT", GUARDS).field(b"DATA", &0x40u32.to_le_bytes()),
            RecordBuilder::new(b"FACT", TOWN).field(b"DATA", &0x40u32.to_le_bytes()),
        ],
    )];
    std::fs::write(
        f.0.join("Factions.esp"),
        write_plugin(0, &[], "test", "synthetic factions", 0x900, &records),
    )
    .unwrap();
    let e = crate::stats::action_tests::headless_engine(&f.0, "Factions.esp");
    (f, e)
}

fn member(faction: FormId, aggression: u8) -> CombatStats {
    CombatStats {
        aggression,
        factions: vec![faction],
        ..CombatStats::default()
    }
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn set_enemy_and_set_ally_change_both_factions_reactions() {
    let (_f, mut e) = fixture("reactions");
    let bandit = Arc::new(member(BANDITS, 1));
    let guard = Arc::new(member(GUARDS, 1));
    assert_eq!(e.faction_combat_reaction(BANDITS, GUARDS), 1);
    assert_eq!(e.faction_combat_reaction(GUARDS, BANDITS), 0);
    // Read once so the cached lists must follow the change.
    assert!(e.hostile_to(&bandit, &[GUARDS], false));
    assert!(!e.hostile_to(&guard, &[BANDITS], false));

    e.set_factions_allies(BANDITS, GUARDS, false, true);
    assert_eq!(e.faction_combat_reaction(BANDITS, GUARDS), 2);
    assert_eq!(e.faction_combat_reaction(GUARDS, BANDITS), 3);
    assert!(!e.hostile_to(&bandit, &[GUARDS], false));
    assert_eq!(
        e.reaction_towards(BANDITS, GUARDS).modifier,
        -10,
        "the plugin's modifier stays"
    );

    e.set_factions_enemies(BANDITS, GUARDS, false, false);
    assert!(e.hostile_to(&bandit, &[GUARDS], false));
    assert!(e.hostile_to(&guard, &[BANDITS], false));
    e.set_factions_enemies(BANDITS, GUARDS, true, false);
    assert_eq!(e.faction_combat_reaction(BANDITS, GUARDS), 0);
    assert!(!e.hostile_to(&bandit, &[GUARDS], false), "neutral now");
    assert!(e.hostile_to(&guard, &[BANDITS], false));
    // Factions nothing named are untouched.
    assert_eq!(e.faction_combat_reaction(TOWN, BANDITS), 0);
    assert_eq!(e.factions.reactions.len(), 2);
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn reaction_modifiers_change_without_touching_combat_reactions() {
    let (_f, mut e) = fixture("modifiers");
    assert_eq!(e.reaction_towards(BANDITS, GUARDS).modifier, -10);
    e.set_reaction_modifier(BANDITS, GUARDS, 25, true);
    assert_eq!(
        e.reaction_towards(BANDITS, GUARDS),
        crate::factions::Reaction {
            modifier: 15,
            combat: 1
        }
    );
    e.set_reaction_modifier(BANDITS, GUARDS, -3, false);
    assert_eq!(e.reaction_towards(BANDITS, GUARDS).modifier, -3);
    assert_eq!(e.reaction_towards(GUARDS, BANDITS).modifier, 0, "one way");
    e.set_reaction_modifier(TOWN, GUARDS, 7, true);
    assert_eq!(e.reaction_towards(TOWN, GUARDS).modifier, 7);
    assert_eq!(e.faction_combat_reaction(TOWN, GUARDS), 0, "still neutral");
}

/// Put an actor with these stats in a loaded interior, near the player.
fn place(e: &mut Engine, r: FormId, stats: CombatStats) {
    let key = crate::render::CellKey::Interior(FormId(0x820));
    let mut a = ActorRuntime::new(
        r,
        FormId(2),
        Arc::new(crate::world::skeleton::Skeleton::default()),
        glam::Mat4::from_translation(glam::Vec3::X * 100.0),
        Vec::new(),
        0.0,
    );
    a.stats = Arc::new(stats);
    e.actor_cells.insert(r, key);
    e.cells.entry(key).or_default().actors.push(a);
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn members_of_factions_the_player_is_an_enemy_of_attack_on_sight() {
    const GUARD: FormId = FormId(0x901);
    const CITIZEN: FormId = FormId(0x902);
    const RESIDENT: FormId = FormId(0x903);
    const FOLLOWER: FormId = FormId(0x904);
    let (_f, mut e) = fixture("player-enemy");
    place(&mut e, GUARD, member(GUARDS, 1));
    place(&mut e, CITIZEN, member(GUARDS, 0));
    // A guard whose crime faction is the town's.
    place(&mut e, RESIDENT, member(GUARDS, 1));
    e.set_crime_faction(RESIDENT, Some(TOWN));
    place(&mut e, FOLLOWER, member(GUARDS, 1));
    e.set_player_teammate(FOLLOWER, true, false);
    for r in [GUARD, CITIZEN, RESIDENT, FOLLOWER] {
        assert!(!e.would_attack_player(r), "{r} keeps the law");
    }
    e.set_faction_player_flag(GUARDS, crate::factions::PLAYER_ENEMY, true);
    e.set_faction_player_flag(TOWN, crate::factions::PLAYER_ENEMY, true);
    assert!(e.would_attack_player(GUARD));
    assert!(e.would_attack_player(RESIDENT));
    assert!(!e.would_attack_player(CITIZEN), "unaggressive");
    assert!(!e.would_attack_player(FOLLOWER), "a teammate");
    e.set_faction_player_flag(GUARDS, crate::factions::PLAYER_ENEMY, false);
    assert!(!e.would_attack_player(GUARD));
    assert!(e.would_attack_player(RESIDENT), "by its crime faction");
}
