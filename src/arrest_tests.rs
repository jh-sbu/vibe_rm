//! Arrest greeting conditions over generated records only; no game assets.

use super::*;
use esp::writer::{Node, RecordBuilder, write_plugin};

const GUARD: FormId = FormId(0x801);
const OTHER: FormId = FormId(0x802);
const FACTION: FormId = FormId(0x803);
const TOPIC: FormId = FormId(0x810);
const INFO: FormId = FormId(0x811);
const QUEST: FormId = FormId(0x820);

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
    let dir = Fixture(std::env::temp_dir().join(format!(
        "vrm-arrest-greeting-{}-{stamp}",
        std::process::id()
    )));
    std::fs::create_dir(&dir.0).unwrap();
    let mut condition = [0; 32];
    condition[4..8].copy_from_slice(&1.0f32.to_le_bytes());
    condition[8..10].copy_from_slice(&657u16.to_le_bytes()); // GetArrestingActor == 1
    let records = [
        Node::top(b"NPC_", vec![RecordBuilder::new(b"NPC_", FormId(0x800))]),
        Node::top(
            b"ACHR",
            vec![RecordBuilder::new(b"ACHR", GUARD).form(b"NAME", FormId(0x800))],
        ),
        Node::top(
            b"DLBR",
            vec![
                RecordBuilder::new(b"DLBR", FormId(0x812))
                    .u32(b"DNAM", 2)
                    .form(b"SNAM", TOPIC),
            ],
        ),
        Node::Group {
            label: *b"DIAL",
            group_type: 0,
            children: vec![
                Node::Record(
                    RecordBuilder::new(b"DIAL", TOPIC)
                        .field(b"SNAM", b"PFGT")
                        .form(b"QNAM", QUEST),
                ),
                Node::Group {
                    label: TOPIC.0.to_le_bytes(),
                    group_type: 7,
                    children: vec![Node::Record(
                        RecordBuilder::new(b"INFO", INFO)
                            .field(b"CTDA", &condition)
                            .field(b"TRDT", &[0; 24])
                            .zstring(b"NAM1", "Stop right there."),
                    )],
                },
            ],
        },
    ];
    std::fs::write(
        dir.0.join("Arrest.esp"),
        write_plugin(0, &[], "test", "synthetic arrest", 0x900, &records),
    )
    .unwrap();
    let mut engine = crate::stats::action_tests::headless_engine(&dir.0, "Arrest.esp");
    engine.scripts.quests.entry(QUEST).or_default().running = true;
    (dir, engine)
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn arrest_greetings_see_the_prospective_guard_and_failures_restore_context() {
    let (_dir, mut engine) = fixture();
    assert!(engine.blocking_greeting(GUARD).is_none());
    assert!(engine.start_arrest_conversation(GUARD, FACTION));
    assert_eq!(engine.crime.arrests.arresting, Some((GUARD, FACTION)));
    assert_eq!(
        engine
            .conversation
            .as_ref()
            .unwrap()
            .info
            .as_ref()
            .unwrap()
            .1
            .id,
        INFO
    );
    engine.arrest_conversation_ended(GUARD, true);
    engine.conversation = None;
    assert!(engine.crime.arrests.arresting.is_none());

    // Selection passes for OTHER, but it has no base and cannot speak.
    let previous = Some((GUARD, FACTION));
    engine.crime.arrests.arresting = previous;
    assert!(engine.blocking_greeting(OTHER).is_none());
    assert!(!engine.start_arrest_conversation(OTHER, FACTION));
    assert_eq!(engine.crime.arrests.arresting, previous);
    assert!(engine.conversation.is_none());

    // Missing greetings also restore the prior state.
    // Stop eligibility without modifying the generated plugin.
    engine.scripts.quests.clear();
    assert!(!engine.start_arrest_conversation(GUARD, FACTION));
    assert_eq!(engine.crime.arrests.arresting, previous);
    engine.crime.arrests.arresting = None;
    assert!(!engine.start_arrest_conversation(GUARD, FACTION));
    assert!(engine.crime.arrests.arresting.is_none());
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn talking_to_a_pursuing_guard_sets_context_before_greeting_selection() {
    let (_dir, mut engine) = fixture();
    engine.crime.arrests.alarmed.insert(
        GUARD,
        Pursuit {
            faction: FACTION,
            bark_in: 0.0,
        },
    );
    engine.start_conversation(GUARD);
    assert_eq!(engine.crime.arrests.arresting, Some((GUARD, FACTION)));
    assert_eq!(
        engine
            .conversation
            .as_ref()
            .unwrap()
            .info
            .as_ref()
            .unwrap()
            .1
            .id,
        INFO
    );
    engine.conversation = None;
    engine.start_conversation(OTHER);
    assert!(engine.conversation.is_none());
    assert_eq!(engine.crime.arrests.arresting, Some((GUARD, FACTION)));

    // An ordinary conversation with the same speaker does not invent an arrest.
    engine.crime.arrests.alarmed.clear();
    engine.crime.arrests.arresting = None;
    engine.start_conversation(GUARD);
    assert!(engine.crime.arrests.arresting.is_none());
    assert!(engine.conversation.as_ref().unwrap().info.is_none());
}
