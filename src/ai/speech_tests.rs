use super::*;
use crate::ai::{
    exec::tests::{ACTOR, BASE, CELL, MARKER, PACKAGE, fixture, loaded_actor},
    package::{Behaviour, Location, LocationKind},
};
use glam::Vec3;
use std::sync::Arc;

const TOPIC: FormId = FormId(0x900);
const OTHER: FormId = FormId(0x805);
const QUEST: FormId = FormId(0x910);

fn leaf(name: &str, values: Vec<Input>) -> Node {
    Node {
        kind: "Procedure".into(),
        flags: 0,
        conditions: vec![],
        children: vec![],
        procedure: name.into(),
        args: vec![],
        values,
    }
}
fn say(finish: bool) -> Node {
    leaf(
        "Say",
        vec![
            Input::Topic(GreetTopic::Topic(TOPIC)),
            Input::Target(super::super::package::Target::Ref(PLAYER_REF)),
            Input::Bool(finish),
            Input::Bool(true),
            Input::Bool(true),
        ],
    )
}

fn engine(n: Node) -> Engine {
    engine_with_player_keyword(n, false)
}

fn engine_with_player_keyword(n: Node, riding: bool) -> Engine {
    use esp::writer::{Node as RecordNode, RecordBuilder, write_plugin};
    let (_f, mut e) = fixture();
    let dir = std::env::temp_dir().join(format!(
        "vrm-say-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let mut response = [0; 24];
    response[12] = 1;
    let info = RecordBuilder::new(b"INFO", FormId(0x901))
        .field(b"TRDT", &response)
        .zstring(b"NAM1", "First response.");
    response[12] = 2;
    let info = info
        .field(b"TRDT", &response)
        .zstring(b"NAM1", "Second response.");
    let mut at = [0; 24];
    at[..4].copy_from_slice(&3000f32.to_le_bytes());
    let groups = vec![
        RecordNode::top(
            b"NPC_",
            vec![
                RecordBuilder::new(b"NPC_", BASE),
                if riding {
                    RecordBuilder::new(b"NPC_", FormId(7)).field(b"KWDA", &0xDD631u32.to_le_bytes())
                } else {
                    RecordBuilder::new(b"NPC_", FormId(7))
                },
            ],
        ),
        RecordNode::top(
            b"KYWD",
            vec![RecordBuilder::new(b"KYWD", FormId(0xDD631)).zstring(b"EDID", "RidingInCarriage")],
        ),
        RecordNode::Group {
            label: *b"CELL",
            group_type: 0,
            children: vec![
                RecordNode::Record(RecordBuilder::new(b"CELL", CELL).field(b"DATA", &[1, 0])),
                RecordNode::Group {
                    label: CELL.0.to_le_bytes(),
                    group_type: esp::group_type::CELL_CHILDREN,
                    children: vec![RecordNode::Group {
                        label: CELL.0.to_le_bytes(),
                        group_type: esp::group_type::CELL_TEMPORARY,
                        children: vec![
                            RecordNode::Record(
                                RecordBuilder::new(b"ACHR", ACTOR).form(b"NAME", BASE),
                            ),
                            RecordNode::Record(
                                RecordBuilder::new(b"ACHR", OTHER).form(b"NAME", BASE),
                            ),
                            RecordNode::Record(
                                RecordBuilder::new(b"REFR", MARKER).field(b"DATA", &at),
                            ),
                        ],
                    }],
                },
            ],
        },
        RecordNode::Group {
            label: *b"DIAL",
            group_type: 0,
            children: vec![
                RecordNode::Record(RecordBuilder::new(b"DIAL", TOPIC)),
                RecordNode::Group {
                    label: TOPIC.0.to_le_bytes(),
                    group_type: esp::group_type::TOPIC_CHILDREN,
                    children: vec![RecordNode::Record(info)],
                },
            ],
        },
        RecordNode::top(
            b"SMEN",
            vec![RecordBuilder::new(b"SMEN", FormId(0x911)).field(b"ENAM", b"ADIA")],
        ),
        RecordNode::top(
            b"SMQN",
            vec![
                RecordBuilder::new(b"SMQN", FormId(0x912))
                    .u32(b"PNAM", 0x911)
                    .u32(b"NNAM", QUEST.0),
            ],
        ),
        RecordNode::top(b"QUST", vec![RecordBuilder::new(b"QUST", QUEST)]),
    ];
    std::fs::write(
        dir.join("Speech.esp"),
        write_plugin(0, &[], "test", "package dialogue", 0xA00, &groups),
    )
    .unwrap();
    e.lo = esp::LoadOrder::load(&dir, &["Speech.esp".into()]).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
    e.location = crate::engine::Location::Interior(CELL);
    e.scripts.alias_gen = 7;
    loaded_actor(&mut e);
    let mut p = Package::bare(PACKAGE, Behaviour::Hold);
    p.schedule.hour = -1;
    p.schedule.day_of_week = -1;
    p.tree = Some(Arc::new(n));
    let a = e.actor_mut(ACTOR).unwrap();
    a.exec.packages = vec![p];
    a.exec.current = None;
    a.exec.run = None;
    a.exec.next_eval = 0.;
    a.exec.pos = Vec3::ZERO;
    a.fresh = false;
    e.evaluate_packages(0.);
    e
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic dialogue only"]
fn say_plays_all_responses_before_completing_and_exposes_target_and_forced_subtitle() {
    let mut e = engine(say(false));
    e.update_package_dialogue();
    assert_eq!(e.barks.packages[&ACTOR].line.text, "First response.");
    assert!(e.is_package_speaking(ACTOR));
    assert_eq!(e.package_dialogue_target(ACTOR), Some(PLAYER_REF));
    assert!(
        e.scene_lines()
            .iter()
            .any(|l| l.force_subtitle && l.speaker == ACTOR)
    );
    assert_eq!(
        e.scene_headtracking().get(&ACTOR),
        Some(&(PLAYER_REF, true))
    );
    assert!(!e.actor_ref(ACTOR).unwrap().exec.run_completed());
    e.scripts.real_time = 2.1;
    e.update_package_dialogue();
    assert_eq!(e.barks.packages[&ACTOR].line.text, "Second response.");
    e.scripts.real_time = 4.2;
    e.update_package_dialogue();
    e.advance_loaded_trees(0.);
    assert!(!e.is_package_speaking(ACTOR));
    assert!(e.actor_ref(ACTOR).unwrap().exec.run_completed());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic dialogue only"]
fn finish_speaking_retains_only_the_current_response_when_its_package_changes() {
    for finish in [false, true] {
        let mut e = engine(say(finish));
        e.update_package_dialogue();
        e.actor_mut(ACTOR).unwrap().exec.begin_run(None, 1., false);
        e.scripts.real_time = 1.;
        e.update_package_dialogue();
        assert_eq!(e.is_package_speaking(ACTOR), finish);
        e.scripts.real_time = 2.1;
        e.update_package_dialogue();
        assert!(!e.is_package_speaking(ACTOR));
        assert!(e.actor_ref(ACTOR).unwrap().exec.run.is_none());
    }
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic dialogue only"]
fn away_speech_waits_for_a_loaded_speaker_and_restarts_only_the_current_response_on_return() {
    let mut e = engine(say(false));
    e.update_package_dialogue();
    let key = crate::render::CellKey::Interior(CELL);
    let a = e.cells.get_mut(&key).unwrap().actors.remove(0);
    e.actor_cells.remove(&ACTOR);
    e.away.insert(
        ACTOR,
        crate::world_state::leaving(&a, super::super::schedule::Place::Interior(CELL), (0., 0.)),
    );
    e.update_package_dialogue();
    e.scripts.real_time = 100.;
    e.update_package_dialogue();
    assert!(!e.is_package_speaking(ACTOR));
    assert_eq!(e.barks.packages[&ACTOR].line.text, "First response.");
    e.away.remove(&ACTOR);
    e.cells.get_mut(&key).unwrap().actors.push(a);
    e.actor_cells.insert(ACTOR, key);
    e.update_package_dialogue();
    assert_eq!(e.barks.packages[&ACTOR].line.ends_at, 102.);
    e.scripts.real_time = 102.1;
    e.update_package_dialogue();
    assert_eq!(e.barks.packages[&ACTOR].line.text, "Second response.");
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic dialogue only"]
fn dialogue_activate_reaches_talk_from_before_opening_the_player_conversation() {
    let n = leaf(
        "DialogueActivate",
        vec![
            Input::Target(super::super::package::Target::Ref(PLAYER_REF)),
            Input::Location(Location {
                kind: LocationKind::NearReference(MARKER),
                radius: 0.,
            }),
            Input::Bool(true),
        ],
    );
    let mut e = engine(n);
    e.update_package_dialogue();
    assert!(e.conversation.is_none());
    e.actor_mut(ACTOR).unwrap().exec.pos = Vec3::X * 3000.;
    e.complete_arrivals();
    assert!(!e.actor_ref(ACTOR).unwrap().exec.run_completed());
    e.update_package_dialogue();
    assert_eq!(e.conversation.as_ref().map(|c| c.npc_ref), Some(ACTOR));
    e.advance_loaded_trees(0.);
    assert!(e.actor_ref(ACTOR).unwrap().exec.run_completed());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic dialogue and story records only"]
fn dialogue_activate_dispatches_the_npc_conversation_to_the_adia_story_tree() {
    let n = leaf(
        "DialogueActivate",
        vec![
            Input::Target(super::super::package::Target::Ref(OTHER)),
            Input::Location(Location {
                kind: LocationKind::NearSelf,
                radius: 0.,
            }),
            Input::Bool(true),
        ],
    );
    let mut e = engine(n);
    loaded_actor(&mut e);
    let key = crate::render::CellKey::Interior(CELL);
    let a = e.cells.get_mut(&key).unwrap().actors.last_mut().unwrap();
    a.ref_id = OTHER;
    a.exec.current = None;
    a.exec.run = None;
    a.exec.pos = Vec3::X * 100.;
    e.actor_cells.insert(OTHER, key);
    e.update_package_dialogue();
    assert!(e.conversation.is_none());
    let event = e.scripts.quests[&QUEST].event.as_ref().unwrap();
    assert_eq!(event.code, *b"ADIA");
    assert_eq!(event.refs, [ACTOR, OTHER]);
    e.advance_loaded_trees(0.);
    assert!(e.actor_ref(ACTOR).unwrap().exec.run_completed());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic dialogue only"]
fn restored_package_speech_keeps_the_selected_response_and_rejects_invalid_response_indices() {
    let mut e = engine(say(false));
    e.update_package_dialogue();
    e.scripts.real_time = 2.1;
    e.update_package_dialogue();
    let saved = e.saved_package_speech();
    assert_eq!(saved[0].response, 1);
    let snapshot = crate::save::state::Snapshot::capture(&e);
    let file = snapshot.write();
    let round_trip = crate::save::state::Snapshot::read(&file).unwrap();
    assert_eq!(round_trip.continuity.package_speech, saved);
    let mut legacy = snapshot.write();
    legacy.sections.retain(|(tag, _)| tag != b"PDLG");
    assert!(
        crate::save::state::Snapshot::read(&legacy)
            .unwrap()
            .continuity
            .package_speech
            .is_empty()
    );
    let mut duplicate = snapshot.write();
    duplicate.add(b"PDLG", file.section(b"PDLG").unwrap().to_vec());
    assert!(crate::save::state::Snapshot::read(&duplicate).is_err());
    e.validate_saved_package_speech(&saved).unwrap();
    let mut invalid = saved.clone();
    invalid[0].response = 99;
    assert!(e.validate_saved_package_speech(&invalid).is_err());
    e.discard_package_dialogue();
    e.restore_package_speech(saved);
    e.update_package_dialogue();
    assert_eq!(e.barks.packages[&ACTOR].line.text, "Second response.");
    assert!(e.barks.packages[&ACTOR].queue.is_empty());
    e.scripts.real_time = 4.2;
    e.update_package_dialogue();
    e.advance_loaded_trees(0.);
    assert!(e.actor_ref(ACTOR).unwrap().exec.run_completed());
}

fn force_greet_engine() -> Engine {
    let mut e = engine(leaf(
        "ForceGreet",
        vec![
            Input::Topic(GreetTopic::Topic(TOPIC)),
            Input::Location(Location {
                kind: LocationKind::NearSelf,
                radius: 0.,
            }),
            Input::Location(Location {
                kind: LocationKind::NearSelf,
                radius: 0.,
            }),
        ],
    ));
    let p = &mut e.actor_mut(ACTOR).unwrap().exec.packages[0];
    p.behaviour = Behaviour::ForceGreet;
    p.greet = Some(super::super::package::ForceGreet {
        topic: GreetTopic::Topic(TOPIC),
        trigger: Some(Location {
            kind: LocationKind::NearSelf,
            radius: 1000.,
        }),
        distance: 300.,
        must_detect: false,
        sandbox: false,
        seated: false,
    });
    e
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic dialogue only"]
fn force_greet_waits_for_transition_and_finishes_its_leaf_after_the_conversation() {
    let mut e = force_greet_engine();
    e.door_transition = true;
    e.update_force_greets(0.);
    e.update_force_greets(1.);
    assert!(e.conversation.is_none());
    e.door_transition = false;
    e.actor_mut(ACTOR).unwrap().exec.greet_check = 0.;
    e.update_force_greets(0.);
    e.update_force_greets(0.);
    assert_eq!(e.conversation.as_ref().unwrap().npc_ref, ACTOR);
    assert!(!e.finish_force_greet(ACTOR));
    e.conversation = None;
    e.run_loaded_finds();
    assert_eq!(e.execution(ACTOR).unwrap().done_once, Some(PACKAGE));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic dialogue only"]
fn force_greet_alias_trigger_uses_the_alias_location_and_fails_when_unfilled() {
    let mut e = force_greet_engine();
    let p = &mut e.actor_mut(ACTOR).unwrap().exec.packages[0];
    p.quest = Some(QUEST);
    p.greet.as_mut().unwrap().trigger = Some(Location {
        kind: LocationKind::NearAlias(3),
        radius: 200.,
    });
    e.update_force_greets(0.);
    assert!(!e.actor_ref(ACTOR).unwrap().exec.greeting);
    e.scripts
        .quests
        .entry(QUEST)
        .or_default()
        .aliases
        .insert(3, MARKER);
    e.actor_mut(ACTOR).unwrap().exec.greet_check = 0.;
    e.update_force_greets(0.);
    assert!(!e.actor_ref(ACTOR).unwrap().exec.greeting);
    e.player.position.x = 3000.;
    e.actor_mut(ACTOR).unwrap().exec.pos.x = 3000.;
    e.actor_mut(ACTOR).unwrap().exec.greet_check = 0.;
    e.update_force_greets(0.);
    e.update_force_greets(0.);
    assert!(e.conversation.is_some());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn force_greet_carriage_gate_reads_the_player_keyword() {
    for riding in [false, true] {
        let e = engine_with_player_keyword(leaf("Wait", vec![Input::Float(0.)]), riding);
        let c = crate::condition::Condition {
            func: 560,
            p1: 0xDD631,
            value: 0.,
            run_on: 2,
            reference: PLAYER_REF,
            ..Default::default()
        };
        assert_eq!(
            crate::condition::evaluate_one(&e, &c, Default::default()),
            !riding
        );
    }
}

#[test]
#[ignore = "requires installed Skyrim SE masters and a headless GPU"]
fn vanilla_force_greet_wait_sitting_and_say_to_player_trees_are_enabled() {
    let data = vfs::locate_data_dir().unwrap();
    let names: Vec<_> = esp::BASE_MASTERS.iter().map(|s| s.to_string()).collect();
    let e = crate::stats::action_tests::headless_engine_plugins(&data, &names);
    for id in [0x3C1C4, 0x9C220, 0xB7869] {
        let p = crate::ai::package::parse(&e.lo, FormId(id)).unwrap();
        assert!(p.tree.as_ref().unwrap().executable());
    }
    let p = crate::ai::package::parse(&e.lo, FormId(0xB7869)).unwrap();
    let tree = p.tree.as_ref().unwrap();
    assert_eq!(tree.children.last().unwrap().procedure, "Sit");
    assert!(matches!(
        tree.children.last().unwrap().values.first(),
        Some(Input::Target(_))
    ));
}
