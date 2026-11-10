//! Followers taking the player's side, using generated records only. Engine
//! owns a renderer, so these need a headless GPU adapter.

use super::*;
use esp::writer::{Node, RecordBuilder, write_plugin};

const FOLLOWERS: FormId = FormId(0x810);
const BANDITS: FormId = FormId(0x811);
const HELPER: FormId = FormId(0x901);
const BANDIT: FormId = FormId(0x902);
const ALIAS_QUEST: FormId = FormId(0x830);
const HELPER_NPC: FormId = FormId(0x840);
const BANDIT_NPC: FormId = FormId(0x841);

/// An NPC's membership (`SNAM`) of a faction at rank 0.
fn snam(faction: FormId) -> [u8; 8] {
    let mut x = [0u8; 8];
    x[0..4].copy_from_slice(&faction.0.to_le_bytes());
    x
}
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
    let records = [
        Node::top(
            b"FACT",
            vec![
                RecordBuilder::new(b"FACT", FOLLOWERS)
                    .field(b"XNAM", &relation(PLAYER_FACTION, 2))
                    .field(b"XNAM", &relation(BANDITS, 1)),
                RecordBuilder::new(b"FACT", BANDITS).field(b"XNAM", &relation(FOLLOWERS, 1)),
            ],
        ),
        // The actors' records: the bandit's NPC is one of the bandits.
        Node::top(
            b"NPC_",
            vec![
                RecordBuilder::new(b"NPC_", HELPER_NPC),
                RecordBuilder::new(b"NPC_", BANDIT_NPC).field(b"SNAM", &snam(BANDITS)),
            ],
        ),
        Node::top(
            b"ACHR",
            vec![
                RecordBuilder::new(b"ACHR", HELPER).form(b"NAME", HELPER_NPC),
                RecordBuilder::new(b"ACHR", BANDIT).form(b"NAME", BANDIT_NPC),
            ],
        ),
        // An alias that puts whoever it holds among the followers.
        Node::top(
            b"QUST",
            vec![
                RecordBuilder::new(b"QUST", ALIAS_QUEST)
                    .u32(b"ALST", 0)
                    .form(b"ALFC", FOLLOWERS)
                    .field(b"ALED", &[]),
            ],
        ),
    ];
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

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn a_teammate_takes_the_players_side_whatever_its_factions_and_assistance() {
    let (_f, mut e) = fixture();
    e.player.position = Vec3::ZERO;
    e.actor_mut(HELPER).unwrap().stats = Arc::new(CombatStats {
        assistance: 0,
        ..CombatStats::default()
    });
    assert_eq!(joins(&mut e), None);
    e.set_player_teammate(HELPER, true, true);
    assert_eq!(joins(&mut e), Some(BANDIT));
    // Not against its own side, nor the other fights of strangers.
    e.set_faction_rank(HELPER, BANDITS, 0);
    assert_eq!(joins(&mut e), None);
    e.set_player_teammate(HELPER, false, true);
    assert_eq!(joins(&mut e), None);
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn a_teammates_blow_on_a_bystander_is_the_players_assault() {
    let (_f, mut e) = fixture();
    // The bandits keep the law here (a crime faction), the helper is theirs.
    e.send_assault(BANDIT, HELPER, true);
    assert!(!e.assaulted_by_player(BANDIT));
    e.set_player_teammate(HELPER, true, true);
    e.send_assault(BANDIT, HELPER, true);
    assert!(e.assaulted_by_player(BANDIT));
}

/// A shared faction alone doesn't make allies: Skyrim SE (autotest21)
/// members of a faction that doesn't list itself didn't help each other.
#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn a_faction_that_doesnt_list_itself_doesnt_help_its_members() {
    let (_f, mut e) = fixture();
    e.player.position = Vec3::ZERO;
    e.set_faction_rank(HELPER, BANDITS, 0);
    let stats = e.actor_ref(HELPER).unwrap().stats.clone();
    let player_factions = e.player_factions();
    let fights = [(BANDIT, Vec3::X * 300.0, PLAYER_REF)];
    let others = [(BANDIT, Vec3::X * 300.0, vec![BANDITS])];
    let join = |e: &mut Engine| {
        e.fight_to_join(
            HELPER,
            Vec3::X * 100.0,
            &stats,
            &fights,
            &others,
            &player_factions,
        )
        .map(|(_, target)| target)
    };
    assert_eq!(join(&mut e), None);
    // Listing itself as an ally, it does.
    e.set_combat_reaction(BANDITS, BANDITS, 2);
    assert_eq!(join(&mut e), Some(PLAYER_REF));
}

/// Skyrim SE (autotest21, autotest23, autotest24): a helper of allies, of no
/// faction, joined its victim's fight from relationship rank 2 (not at 1);
/// one of friends and allies at every rank from 1 to 4.
#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn relationship_ranks_bring_help() {
    let (_f, mut e) = fixture();
    e.player.position = Vec3::ZERO;
    let fights = [(BANDIT, Vec3::X * 300.0, PLAYER_REF)];
    let others = [(BANDIT, Vec3::X * 300.0, vec![BANDITS])];
    let player_factions = e.player_factions();
    let join = |e: &mut Engine| {
        let stats = e.actor_ref(HELPER).unwrap().stats.clone();
        e.fight_to_join(
            HELPER,
            Vec3::X * 100.0,
            &stats,
            &fights,
            &others,
            &player_factions,
        )
        .map(|(_, target)| target)
    };
    assert_eq!(join(&mut e), None);
    // The synthetic actors have no records for `set_relationship_rank`.
    let rank = |e: &mut Engine, r: i32| {
        e.relationships
            .changed
            .insert((HELPER.min(BANDIT), HELPER.max(BANDIT)), r);
    };
    rank(&mut e, 1);
    assert_eq!(e.relationship_rank(HELPER, BANDIT), 1);
    assert_eq!(join(&mut e), None, "a friend, and it helps only allies");
    rank(&mut e, 2);
    assert_eq!(join(&mut e), Some(PLAYER_REF), "a confidant is an ally");
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn a_loaded_actor_fights_by_its_aliases_factions_while_they_hold_it() {
    let (_f, mut e) = fixture();
    e.player.position = Vec3::ZERO;
    e.scripts.quests.entry(ALIAS_QUEST).or_default().running = true;
    // `ForceRefTo`: the alias's faction (`ALFC`) counts in its fights.
    e.force_alias_ref(ALIAS_QUEST, 0, HELPER);
    e.refresh_alias_packages();
    assert_eq!(e.actor_ref(HELPER).unwrap().stats.factions, [FOLLOWERS]);
    assert_eq!(joins(&mut e), Some(BANDIT));
    // The quest stops: the membership goes with the alias.
    e.stop_quest(ALIAS_QUEST);
    e.refresh_alias_packages();
    assert!(e.actor_ref(HELPER).unwrap().stats.factions.is_empty());
    assert_eq!(joins(&mut e), None);
}
