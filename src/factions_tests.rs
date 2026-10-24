//! Factions' runtime standing, using generated records only. Engine owns a
//! renderer, so these need a headless GPU adapter.

use std::sync::Arc;

use esp::FormId;
use esp::writer::{Node, RecordBuilder, write_plugin};

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
