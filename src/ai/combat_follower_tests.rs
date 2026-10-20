//! Followers taking the player's side, using generated records only. Engine
//! owns a renderer, so these need a headless GPU adapter.

use super::*;
use esp::writer::{Node, RecordBuilder, write_plugin};

const FOLLOWERS: FormId = FormId(0x810);
const BANDITS: FormId = FormId(0x811);
const HELPER: FormId = FormId(0x901);
const BANDIT: FormId = FormId(0x902);
/// The player's faction (`PlayerFaction`), which `Engine::player_factions` names.
const PLAYER_FACTION: FormId = FormId(0xDB1);

struct Fixture(std::path::PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn actor(r: FormId, x: f32, factions: Vec<FormId>, assistance: u8) -> ActorRuntime {
    let mut a = ActorRuntime::new(
        r,
        FormId(2),
        Arc::new(crate::world::skeleton::Skeleton::default()),
        Mat4::from_translation(Vec3::X * x),
        Vec::new(),
        0.0,
    );
    a.stats = Arc::new(CombatStats {
        assistance,
        factions,
        ..CombatStats::default()
    });
    a
}

/// Followers are allies of the player's faction; the bandits fight both.
fn fixture() -> (Fixture, Engine) {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let f =
        Fixture(std::env::temp_dir().join(format!("vrm-followers-{}-{stamp}", std::process::id())));
    std::fs::create_dir(&f.0).unwrap();
    let relation = |other: FormId, reaction: u32| {
        let mut x = [0u8; 12];
        x[0..4].copy_from_slice(&other.0.to_le_bytes());
        x[8..12].copy_from_slice(&reaction.to_le_bytes());
        x
    };
    let records = [Node::top(
        b"FACT",
        vec![
            RecordBuilder::new(b"FACT", FOLLOWERS)
                .field(b"XNAM", &relation(PLAYER_FACTION, 2))
                .field(b"XNAM", &relation(BANDITS, 1)),
            RecordBuilder::new(b"FACT", BANDITS).field(b"XNAM", &relation(FOLLOWERS, 1)),
        ],
    )];
    std::fs::write(
        f.0.join("Followers.esp"),
        write_plugin(0, &[], "test", "synthetic followers", 0x900, &records),
    )
    .unwrap();
    let mut e = crate::stats::action_tests::headless_engine(&f.0, "Followers.esp");
    let key = crate::render::CellKey::Interior(FormId(0x820));
    e.actor_cells.insert(HELPER, key);
    e.actor_cells.insert(BANDIT, key);
    let rt = e.cells.entry(key).or_default();
    rt.actors.push(actor(HELPER, 100.0, Vec::new(), 1));
    rt.actors.push(actor(BANDIT, 300.0, vec![BANDITS], 0));
    (f, e)
}

/// The helper looks for a fight to join: the bandit fighting the player.
fn joins(e: &mut Engine) -> Option<FormId> {
    let stats = e.actor_ref(HELPER).unwrap().stats.clone();
    let player_factions = e.player_factions();
    let fights = [
        (BANDIT, Vec3::X * 300.0, PLAYER_REF),
        (PLAYER_REF, e.player.position, BANDIT),
    ];
    let others = [(BANDIT, Vec3::X * 300.0, vec![BANDITS])];
    e.fight_to_join(
        HELPER,
        Vec3::X * 100.0,
        &stats,
        &fights,
        &others,
        &player_factions,
    )
    .map(|(_, target)| target)
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn a_script_made_follower_helps_the_player_against_their_attacker() {
    let (_f, mut e) = fixture();
    e.player.position = Vec3::ZERO;
    // Not a follower yet: no side to take.
    assert_eq!(joins(&mut e), None);
    // `AddToFaction`: it is in the followers' faction at once, in combat too.
    e.set_faction_rank(HELPER, FOLLOWERS, 0);
    assert_eq!(e.actor_ref(HELPER).unwrap().stats.factions, [FOLLOWERS]);
    assert_eq!(joins(&mut e), Some(BANDIT));
    // A potential follower (rank -1) is no member.
    e.set_faction_rank(HELPER, FOLLOWERS, -1);
    assert!(e.actor_ref(HELPER).unwrap().stats.factions.is_empty());
    assert_eq!(joins(&mut e), None);
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn the_players_own_faction_changes_count_as_theirs() {
    let (_f, mut e) = fixture();
    assert_eq!(e.player_factions(), [PLAYER_FACTION]);
    e.set_faction_rank(PLAYER_REF, BANDITS, 0);
    assert_eq!(e.player_factions(), [BANDITS, PLAYER_FACTION]);
    e.set_faction_rank(PLAYER_REF, BANDITS, -1);
    assert_eq!(e.player_factions(), [PLAYER_FACTION]);
}
