//! Factions scripts change at runtime, using generated records only. Engine
//! owns a renderer, so these need a headless GPU adapter.

use super::*;
use esp::writer::{Node, RecordBuilder, write_plugin};

const FACTION: FormId = FormId(0x810);
const NPC: FormId = FormId(0x820);
const ACTOR: FormId = FormId(0x821);

struct Fixture(std::path::PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture(name: &str) -> (Fixture, Engine) {
    let f =
        Fixture(std::env::temp_dir().join(format!("vrm-factions-{name}-{}", std::process::id())));
    std::fs::create_dir_all(&f.0).unwrap();
    let records = [
        Node::top(
            b"FACT",
            vec![RecordBuilder::new(b"FACT", FACTION).u32(b"DATA", 0x8000)],
        ),
        Node::top(b"NPC_", vec![RecordBuilder::new(b"NPC_", NPC)]),
        Node::top(
            b"ACHR",
            vec![RecordBuilder::new(b"ACHR", ACTOR).u32(b"NAME", NPC.0)],
        ),
    ];
    std::fs::write(
        f.0.join("Factions.esp"),
        write_plugin(0, &[], "test", "synthetic factions", 0x900, &records),
    )
    .unwrap();
    let e = crate::stats::action_tests::headless_engine(&f.0, "Factions.esp");
    (f, e)
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn actors_scripts_put_in_a_faction_count_towards_its_favor_cap() {
    let (_f, mut e) = fixture("cap");
    e.set_relationship_rank(ACTOR, PLAYER_REF, 1);
    // Nobody belongs: no allowance.
    assert_eq!(e.faction_favor_cap(FACTION), None);
    assert_eq!(e.faction_rank(ACTOR, FACTION), -1);
    e.set_faction_rank(ACTOR, FACTION, 0);
    assert_eq!(e.faction_rank(ACTOR, FACTION), 0);
    assert_eq!(e.faction_rank(NPC, FACTION), 0);
    assert_eq!(e.faction_favor_cap(FACTION), Some(25));
    // Taken out again, they no longer count.
    e.set_faction_rank(ACTOR, FACTION, -1);
    assert_eq!(e.faction_rank(ACTOR, FACTION), -1);
    assert_eq!(e.faction_favor_cap(FACTION), None);
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn the_player_in_a_faction_below_the_rank_asked_is_outside_it() {
    let (_f, mut e) = fixture("rank");
    assert!(!e.player_in_faction(FACTION, -1));
    e.set_faction_rank(PLAYER_REF, FACTION, 2);
    assert!(e.player_in_faction(FACTION, -1));
    assert!(e.player_in_faction(FACTION, 2));
    assert!(!e.player_in_faction(FACTION, 3));
    assert!(!e.owned_by_other_ranked(FACTION, 2));
    assert!(e.owned_by_other_ranked(FACTION, 3));
}
