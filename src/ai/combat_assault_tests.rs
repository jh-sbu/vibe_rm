//! Story Manager assault events for blows to the player, using generated
//! records only. Engine owns a renderer, so these need a headless GPU
//! adapter.

use super::*;
use esp::writer::{Node, RecordBuilder, write_plugin};

const ASSAULT_QUEST: FormId = FormId(0x810);
const ATTACKER: FormId = FormId(0x901);

struct Fixture(std::path::PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn actor(r: FormId) -> ActorRuntime {
    ActorRuntime::new(
        r,
        FormId(2),
        Arc::new(crate::world::skeleton::Skeleton::default()),
        Mat4::IDENTITY,
        Vec::new(),
        0.0,
    )
}

fn fixture() -> (Fixture, Engine) {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let f = Fixture(
        std::env::temp_dir().join(format!("vrm-story-assault-{}-{stamp}", std::process::id())),
    );
    std::fs::create_dir(&f.0).unwrap();
    let story = |code: &[u8; 4], menu: u32, node: u32, quest: FormId| {
        [
            Node::top(
                b"SMEN",
                vec![RecordBuilder::new(b"SMEN", FormId(menu)).field(b"ENAM", code)],
            ),
            Node::top(
                b"SMQN",
                vec![
                    RecordBuilder::new(b"SMQN", FormId(node))
                        .u32(b"PNAM", menu)
                        .u32(b"NNAM", quest.0),
                ],
            ),
        ]
    };
    let mut records = Vec::new();
    records.extend(story(b"ASSU", 0x830, 0x831, ASSAULT_QUEST));
    records.push(Node::top(
        b"QUST",
        vec![RecordBuilder::new(b"QUST", ASSAULT_QUEST)],
    ));
    std::fs::write(
        f.0.join("Assault.esp"),
        write_plugin(0, &[], "test", "synthetic assault events", 0x900, &records),
    )
    .unwrap();
    let mut e = crate::stats::action_tests::headless_engine(&f.0, "Assault.esp");
    let key = crate::render::CellKey::Interior(FormId(0x820));
    e.actor_cells.insert(ATTACKER, key);
    e.cells.entry(key).or_default().actors.push(actor(ATTACKER));
    (f, e)
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn the_first_blow_to_the_player_from_a_stranger_is_an_assault_that_is_no_crime() {
    let (_f, mut e) = fixture();
    e.damage(PLAYER_REF, 5.0, Some(ATTACKER), 0.0);
    let ev = e.scripts.quests[&ASSAULT_QUEST].event.clone().unwrap();
    assert_eq!(ev.refs, [PLAYER_REF, ATTACKER]);
    assert_eq!(ev.values[0], 0.0);
    // The blow doesn't turn the attacker on the player, but the next ones
    // are not assaults.
    assert!(e.actor_ref(ATTACKER).unwrap().exec.combat.is_none());
    e.stop_quest(ASSAULT_QUEST);
    e.damage(PLAYER_REF, 5.0, Some(ATTACKER), 0.0);
    assert!(!e.scripts.quests[&ASSAULT_QUEST].running);
    // Nor are the player's own wounds, or those with no attacker.
    e.damage(PLAYER_REF, 5.0, Some(PLAYER_REF), 0.0);
    e.damage(PLAYER_REF, 5.0, None, 0.0);
    assert!(!e.scripts.quests[&ASSAULT_QUEST].running);
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn a_teammate_told_to_ignore_friendly_hits_does_not_turn_on_the_player() {
    let (_f, mut e) = fixture();
    let victim = FormId(0x902);
    let key = crate::render::CellKey::Interior(FormId(0x820));
    e.actor_cells.insert(victim, key);
    let mut a = actor(victim);
    a.exec.health = 100.0;
    e.cells.entry(key).or_default().actors.push(a);
    e.set_player_teammate(victim, true, true);
    e.set_ignore_friendly_hits(victim, true);
    e.damage(victim, 5.0, Some(PLAYER_REF), 0.0);
    let a = e.actor_ref(victim).unwrap();
    assert_eq!(a.exec.health, 95.0);
    assert!(a.exec.combat.is_none());
    // Without the flag the same blow starts a fight.
    e.set_ignore_friendly_hits(victim, false);
    e.damage(victim, 5.0, Some(PLAYER_REF), 0.0);
    assert!(e.actor_ref(victim).unwrap().exec.combat.is_some());
}
