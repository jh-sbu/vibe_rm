use super::*;
use crate::ai::{
    exec::tests::{ACTOR, BASE, CELL, fixture, loaded_actor},
    schedule::Place,
};
use esp::writer::{Node, RecordBuilder, write_plugin};
use glam::Vec3;

const OTHER: FormId = FormId(0x803);
const LOW: FormId = FormId(0x850);
const HIGH: FormId = FormId(0x851);
const LIST: FormId = FormId(0x852);
const NESTED: FormId = FormId(0x853);
const LOC_A: FormId = FormId(0x860);
const LOC_B: FormId = FormId(0x861);
const ROOM_B: FormId = FormId(0x830);
const Q: FormId = FormId(0x870);
const EVENT_Q: FormId = FormId(0x871);
const PACK: FormId = FormId(0x880);

fn engine() -> Engine {
    let (_f, mut e) = fixture();
    let dir = std::env::temp_dir().join(format!(
        "vrm-alias-depth-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&dir).unwrap();
    let acbs = |level: u16| {
        let mut d = [0; 24];
        d[8..10].copy_from_slice(&level.to_le_bytes());
        d
    };
    let lvlo = |level: u16, item: FormId, n: u16| {
        [
            level.to_le_bytes().as_slice(),
            &[0; 2],
            &item.0.to_le_bytes(),
            &n.to_le_bytes(),
            &[0; 2],
        ]
        .concat()
    };
    let cell = |id: FormId, location: FormId, refs: Vec<RecordBuilder>| Node::Group {
        label: *b"CELL",
        group_type: 0,
        children: vec![
            Node::Record(
                RecordBuilder::new(b"CELL", id)
                    .field(b"DATA", &[1, 0])
                    .form(b"XLCN", location),
            ),
            Node::Group {
                label: id.0.to_le_bytes(),
                group_type: esp::group_type::CELL_CHILDREN,
                children: vec![Node::Group {
                    label: id.0.to_le_bytes(),
                    group_type: esp::group_type::CELL_TEMPORARY,
                    children: refs.into_iter().map(Node::Record).collect(),
                }],
            },
        ],
    };
    let mut cnto = NESTED.0.to_le_bytes().to_vec();
    cnto.extend(2i32.to_le_bytes());
    let groups = vec![
        Node::top(
            b"NPC_",
            vec![
                RecordBuilder::new(b"NPC_", BASE).field(b"ACBS", &acbs(20)),
                RecordBuilder::new(b"NPC_", FormId(0x804)).field(b"ACBS", &acbs(1)),
            ],
        ),
        Node::top(
            b"MISC",
            vec![
                RecordBuilder::new(b"MISC", LOW).field(b"DATA", &[0; 8]),
                RecordBuilder::new(b"MISC", HIGH).field(b"DATA", &[0; 8]),
            ],
        ),
        Node::top(
            b"LVLI",
            vec![
                RecordBuilder::new(b"LVLI", LIST)
                    .field(b"LVLF", &[0])
                    .field(b"LVLO", &lvlo(1, LOW, 2))
                    .field(b"LVLO", &lvlo(10, HIGH, 3)),
                RecordBuilder::new(b"LVLI", NESTED).field(b"LVLO", &lvlo(1, LIST, 1)),
            ],
        ),
        Node::top(
            b"LCTN",
            vec![
                RecordBuilder::new(b"LCTN", LOC_A),
                RecordBuilder::new(b"LCTN", LOC_B),
            ],
        ),
        Node::top(
            b"PACK",
            vec![
                RecordBuilder::new(b"PACK", PACK)
                    .field(b"XNAM", &[])
                    .zstring(b"ANAM", "Procedure")
                    .zstring(b"PNAM", "HoldPosition"),
            ],
        ),
        Node::top(
            b"QUST",
            vec![
                RecordBuilder::new(b"QUST", Q)
                    .u32(b"ALST", 1)
                    .form(b"ALFR", ACTOR)
                    .field(b"CNTO", &cnto)
                    .field(b"ALED", &[])
                    .u32(b"ALLS", 2)
                    .u32(b"ALFA", 1)
                    .field(b"ALED", &[]),
                RecordBuilder::new(b"QUST", EVENT_Q)
                    .u32(b"ALST", 1)
                    .field(b"ALFE", b"ADIA")
                    .field(b"ALFD", b"R1")
                    .form(b"ALPC", PACK)
                    .field(b"ALED", &[]),
            ],
        ),
        Node::top(
            b"SMEN",
            vec![RecordBuilder::new(b"SMEN", FormId(0x890)).field(b"ENAM", b"ADIA")],
        ),
        Node::top(
            b"SMQN",
            vec![
                RecordBuilder::new(b"SMQN", FormId(0x891))
                    .u32(b"PNAM", 0x890)
                    .form(b"NNAM", EVENT_Q),
            ],
        ),
        cell(
            CELL,
            LOC_A,
            vec![
                RecordBuilder::new(b"ACHR", ACTOR).form(b"NAME", BASE),
                RecordBuilder::new(b"ACHR", OTHER).form(b"NAME", FormId(0x804)),
            ],
        ),
        cell(ROOM_B, LOC_B, vec![]),
    ];
    std::fs::write(
        dir.join("AliasDepth.esp"),
        write_plugin(0, &[], "test", "alias depth", 0x900, &groups),
    )
    .unwrap();
    e.lo = esp::LoadOrder::load(&dir, &["AliasDepth.esp".into()]).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
    e.location = crate::engine::Location::Interior(CELL);
    e.alias_spec_cache.borrow_mut().clear();
    loaded_actor(&mut e);
    e
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn alias_leveled_inventory_uses_the_recipient_level_through_nested_lists_once_per_new_fill() {
    let mut e = engine();
    e.scripts.quests.entry(Q).or_default().running = true;
    assert!(e.fill_quest_aliases(Q));
    assert_eq!(e.item_count(ACTOR, HIGH), 6);
    assert_eq!(e.item_count(ACTOR, LOW), 0);
    e.force_alias_ref(Q, 1, ACTOR);
    assert_eq!(e.item_count(ACTOR, HIGH), 6);
    e.force_alias_ref(Q, 1, OTHER);
    assert_eq!(e.item_count(OTHER, LOW), 4);
    assert_eq!(e.item_count(OTHER, HIGH), 0);
    e.force_alias_ref(Q, 1, ACTOR);
    assert_eq!(e.item_count(ACTOR, HIGH), 12);
    let saved = crate::save::state::Snapshot::capture(&e).write();
    let copy = crate::save::state::Snapshot::read(&saved).unwrap();
    assert_eq!(
        copy.continuity.alias_location_sources,
        e.alias_location_sources
    );
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn reference_location_aliases_follow_loaded_and_away_travel_before_location_events_are_delivered() {
    let mut e = engine();
    e.scripts.quests.entry(Q).or_default().running = true;
    assert!(e.fill_quest_aliases(Q));
    assert_eq!(e.alias_ref(Q, 2), Some(LOC_A));
    let key = e.actor_cells.remove(&ACTOR).unwrap();
    let mut a = e.cells.get_mut(&key).unwrap().actors.remove(0);
    a.exec.place = Some(Place::Interior(ROOM_B));
    a.exec.pos = Vec3::ZERO;
    e.away.insert(ACTOR, a.exec);
    e.send_script_event(ACTOR, "OnLocationChange", vec![]);
    assert_eq!(e.alias_ref(Q, 2), Some(LOC_B));
    let object = papyrus::Value::Object(
        papyrus::ObjectId::Alias {
            quest: Q.0,
            alias: 2,
        },
        "LocationAlias".into(),
    );
    let forced = e.object_value(LOC_A);
    crate::script::natives::call(
        &mut e,
        "locationalias",
        "forcelocationto",
        Some(&object),
        &[forced],
    );
    e.refresh_ref_location_aliases();
    assert_eq!(e.alias_ref(Q, 2), Some(LOC_A));
    e.away.get_mut(&ACTOR).unwrap().place = Some(Place::Interior(CELL));
    e.refresh_ref_location_aliases();
    assert_eq!(e.alias_ref(Q, 2), Some(LOC_A));
    e.away.get_mut(&ACTOR).unwrap().place = Some(Place::Interior(ROOM_B));
    e.refresh_ref_location_aliases();
    assert_eq!(e.alias_ref(Q, 2), Some(LOC_B));
    let source = papyrus::Value::Object(
        papyrus::ObjectId::Alias {
            quest: Q.0,
            alias: 1,
        },
        "ReferenceAlias".into(),
    );
    crate::script::natives::call(&mut e, "referencealias", "clear", Some(&source), &[]);
    assert_eq!(e.alias_ref(Q, 2), None);
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn from_event_aliases_supply_their_packages_to_the_event_actor() {
    let mut e = engine();
    let mut event = crate::story::StoryEvent::new(b"ADIA");
    event.refs = [ACTOR, OTHER];
    e.send_story_event(event);
    assert!(e.scripts.quests[&EVENT_Q].running);
    assert_eq!(e.alias_ref(EVENT_Q, 1), Some(ACTOR));
    let packages = e.alias_packages(ACTOR);
    assert_eq!(
        packages.iter().map(|p| p.id).collect::<Vec<_>>(),
        vec![PACK]
    );
    assert_eq!(packages[0].quest, Some(EVENT_Q));
    assert!(e.alias_packages(OTHER).is_empty());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn an_alias_hears_its_reference_load_with_it() {
    let mut e = engine();
    e.scripts.quests.entry(Q).or_default().running = true;
    e.force_alias_ref(Q, 1, ACTOR);
    let alias = papyrus::ObjectId::Alias {
        quest: Q.0,
        alias: 1,
    };
    // The objects told of the cell's load (`OnCellLoad`) take in the alias.
    assert!(e.attach_cell_scripts(&[ACTOR]).contains(&alias));
    // Not once the quest has stopped.
    e.stop_quest(Q);
    assert!(!e.attach_cell_scripts(&[ACTOR]).contains(&alias));
}
