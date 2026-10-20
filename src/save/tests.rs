//! Saving and loading through a real `Engine` (a headless GPU adapter) with
//! generated records only.

use esp::FormId;
use esp::group_type;
use esp::writer::{Node, RecordBuilder, write_plugin};
use glam::{Mat4, Vec3};

use super::file::SaveFile;
use super::state::{Snapshot, Spot};
use crate::ai::schedule::Place;
use crate::engine::{Engine, Location, PLAYER_REF};
use crate::world::inventory::{Inventory, LEFT, RIGHT};
use crate::world_state::{Moved, Wounds};

const SWORD: FormId = FormId(0x800);
const SHIELD: FormId = FormId(0x801);
const GEM: FormId = FormId(0x802);
const GLOBAL: FormId = FormId(0x810);
const QUEST: FormId = FormId(0x820);
const PERK: FormId = FormId(0x830);
const ACTOR: FormId = FormId(0x840);
const CELL: FormId = FormId(0x850);
const STORY_FIRST: FormId = FormId(0x860);
const STORY_SECOND: FormId = FormId(0x861);
const COOLDOWN: FormId = FormId(0x862);
const RANK_EVENT: FormId = FormId(0x863);
const LOCATION_EVENT: FormId = FormId(0x864);

struct Fixture(std::path::PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn engine(name: &str) -> (Fixture, Engine) {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let f = Fixture(
        std::env::temp_dir().join(format!("vrm-save-{name}-{}-{stamp}", std::process::id())),
    );
    std::fs::create_dir(&f.0).unwrap();
    let records = [
        Node::top(
            b"NPC_",
            vec![
                RecordBuilder::new(b"NPC_", ACTOR),
                RecordBuilder::new(b"NPC_", FormId(7)),
            ],
        ),
        Node::top(
            b"QUST",
            [
                STORY_FIRST,
                STORY_SECOND,
                COOLDOWN,
                RANK_EVENT,
                LOCATION_EVENT,
            ]
            .into_iter()
            .map(|id| RecordBuilder::new(b"QUST", id))
            .collect(),
        ),
        Node::top(
            b"SMEN",
            vec![
                RecordBuilder::new(b"SMEN", FormId(0x870)).field(b"ENAM", b"LOCK"),
                RecordBuilder::new(b"SMEN", FormId(0x871)).field(b"ENAM", b"SCPT"),
                RecordBuilder::new(b"SMEN", FormId(0x872)).field(b"ENAM", b"CHRR"),
                RecordBuilder::new(b"SMEN", FormId(0x873)).field(b"ENAM", b"CLOC"),
            ],
        ),
        Node::top(
            b"SMQN",
            vec![
                RecordBuilder::new(b"SMQN", FormId(0x880))
                    .u32(b"PNAM", 0x870)
                    .field(b"DNAM", &[0, 0, 1, 0])
                    .u32(b"NNAM", STORY_FIRST.0)
                    .u32(b"NNAM", STORY_SECOND.0),
                RecordBuilder::new(b"SMQN", FormId(0x881))
                    .u32(b"PNAM", 0x871)
                    .u32(b"NNAM", COOLDOWN.0)
                    .field(b"RNAM", &24.0f32.to_le_bytes()),
                RecordBuilder::new(b"SMQN", FormId(0x882))
                    .u32(b"PNAM", 0x872)
                    .u32(b"NNAM", RANK_EVENT.0),
                RecordBuilder::new(b"SMQN", FormId(0x883))
                    .u32(b"PNAM", 0x873)
                    .u32(b"NNAM", LOCATION_EVENT.0),
            ],
        ),
        Node::top(
            b"WEAP",
            vec![RecordBuilder::new(b"WEAP", SWORD).field(b"DNAM", &[0; 100])],
        ),
        Node::top(b"ARMO", vec![RecordBuilder::new(b"ARMO", SHIELD)]),
        Node::top(b"SLGM", vec![RecordBuilder::new(b"SLGM", GEM)]),
        // An empty interior cell (object id 0x850: block 8, sub-block 2).
        Node::Group {
            label: *b"CELL",
            group_type: group_type::TOP,
            children: vec![Node::Group {
                label: 8i32.to_le_bytes(),
                group_type: group_type::INTERIOR_BLOCK,
                children: vec![Node::Group {
                    label: 2i32.to_le_bytes(),
                    group_type: group_type::INTERIOR_SUB_BLOCK,
                    children: vec![Node::Record(
                        RecordBuilder::new(b"CELL", CELL)
                            .zstring(b"EDID", "SaveCell")
                            .field(b"DATA", &[1, 0]),
                    )],
                }],
            }],
        },
    ];
    std::fs::write(
        f.0.join("Save.esp"),
        write_plugin(0, &[], "test", "synthetic saves", 0x900, &records),
    )
    .unwrap();
    // A script that sets `count` to 1, waits ten seconds and sets it to 2.
    std::fs::create_dir(f.0.join("scripts")).unwrap();
    let mut script = papyrus::build::ScriptBuilder::new("VrmSaveTest", "Quest");
    let count = script.variable("count", "Int", papyrus::build::Arg::None);
    let mut on_init = papyrus::build::FunctionBuilder::new("None");
    on_init.assign(count.clone(), papyrus::build::Arg::Int(1));
    on_init.call_static(
        "Utility",
        "Wait",
        papyrus::build::discard(),
        [papyrus::build::Arg::Float(10.0)],
    );
    on_init.assign(count, papyrus::build::Arg::Int(2));
    on_init.ret(papyrus::build::Arg::None);
    script.function("OnInit", on_init);
    std::fs::write(
        f.0.join("scripts/VrmSaveTest.pex"),
        papyrus::pex::write(&script.build().unwrap()).unwrap(),
    )
    .unwrap();
    std::fs::write(
        f.0.join("scripts/Utility.pex"),
        papyrus::build::native_class(
            "Utility",
            "",
            &[("Wait", "None", &[("afSeconds", "Float")], true)],
        )
        .unwrap(),
    )
    .unwrap();
    let e = crate::stats::action_tests::headless_engine(&f.0, "Save.esp");
    (f, e)
}

/// Change a bit of everything saves keep.
fn play(e: &mut Engine) {
    e.day = 12;
    e.hour = 17.5;
    e.scripts.real_time = 321.5;
    e.scripts.wall_time = 400.25;
    e.scripts.globals.insert(GLOBAL, 3.5);
    let q = e.scripts.quests.entry(QUEST).or_default();
    q.running = true;
    q.stage = 30;
    q.done.extend([10, 20, 30]);
    q.objectives_displayed.insert(1);
    q.objectives_completed.insert(0);
    q.objective_texts.insert(1, "Find the sword".into());
    q.log.push("It was lost.".into());
    q.active = true;
    q.aliases.insert(2, ACTOR);
    q.filled_at.insert(2, 7);
    q.event = Some(crate::story::StoryEvent {
        code: *b"CLOC",
        refs: [ACTOR, FormId::NULL],
        values: [1.0, 2.0],
        ..Default::default()
    });
    e.scripts.stats.insert("Locks Picked".into(), 4);
    let quest = papyrus::ObjectId::Form(QUEST.0);
    e.scripts.initialized.insert(quest);
    e.scripts.timers.push(crate::script::Timer {
        at: 340.0,
        game_time: false,
        obj: quest,
        script: "VrmSaveTest".into(),
        event: "OnUpdate",
        repeat: None,
    });
    e.scripts.sleep_listeners.push(quest);
    e.scripts
        .inventory_filters
        .insert(quest, [SWORD].into_iter().collect());
    e.scripts.actor_values.insert(
        (PLAYER_REF, 7),
        crate::actor_values::Modifiers {
            base: Some(40.0),
            permanent: 2.0,
            temporary: -1.0,
            damage: -3.0,
        },
    );
    e.scripts
        .other_actor_values
        .insert((ACTOR, "Aggression".into()), 2.0);
    e.scripts.looted.insert(FormId(0x900));
    e.scripts.disabled.insert(FormId(0x901), true);
    e.scripts.locked.insert(FormId(0x902), false);
    e.scripts.lock_levels.insert(FormId(0x902), 50);
    e.scripts.owners.insert(FormId(0x903), None);
    e.scripts.owners.insert(FormId(0x904), Some(ACTOR));
    e.scripts
        .faction_changes
        .entry(ACTOR)
        .or_default()
        .insert(FormId(0x905), -1);

    let mut inv = Inventory::default();
    inv.add(SWORD, 2);
    inv.add(SHIELD, 1);
    inv.add_owned(GEM, Some(ACTOR), 3);
    inv.put_soul(GEM, 4);
    inv.set_charge(SWORD, 12.0, 50.0);
    inv.poison(SWORD, FormId(0x906), 2);
    inv.hold(&e.lo, SWORD, &[RIGHT]);
    inv.hold(&e.lo, SWORD, &[LEFT]);
    e.inventories.insert(PLAYER_REF, inv);
    let mut chest = Inventory::default();
    chest.add(SWORD, 1);
    e.inventories.insert(FormId(0x907), chest);

    let ws = &mut e.world_state;
    ws.alerted.insert(ACTOR);
    ws.dead.insert(
        ACTOR,
        (
            Place::Exterior(FormId(0x3C), (-2, 7)),
            Vec3::new(1.0, 2.0, 3.0),
        ),
    );
    ws.open_doors.insert(FormId(0x908));
    ws.body_poses.insert(
        ACTOR,
        vec![Mat4::from_translation(Vec3::new(5.0, 6.0, 7.0))],
    );
    ws.wounds.insert(
        ACTOR,
        Wounds {
            health: 10.0,
            max: 50.0,
            rate: 0.01,
            at: 99.0,
        },
    );
    ws.moved.insert(
        FormId(0x909),
        Moved {
            place: Place::Interior(CELL),
            pos: Vec3::ONE,
            rot: Vec3::new(0.0, 0.0, 1.5),
        },
    );
    ws.item_charges.insert(FormId(0x90A), vec![1.0, 2.0]);
    ws.broken_joints.insert(FormId(0x90B), vec![0, 3]);
    ws.learned_effects.insert((FormId(0x90C), 2));
    e.created_refs.refs.insert(
        FormId(0xFF00_0800),
        crate::created::Created {
            base: SWORD,
            position: Vec3::new(9.0, 8.0, 7.0),
            rotation: Vec3::ZERO,
            place: Some(Place::Interior(CELL)),
            container: None,
            location: Some(FormId(0x90D)),
            actor: false,
            count: 3,
        },
    );
    e.created_refs.next = 0xFF00_0801;

    e.player_health = 66.0;
    e.player_stamina = 55.0;
    e.skills.xp[3] = 12.5;
    e.skills.legendary[1] = 2;
    e.skills.level = 9;
    e.skills.perk_points = 4;
    e.skills.read_books.insert(FormId(0x90E));
    e.perks.added.insert(PLAYER_REF, vec![(PERK, 2)]);
    e.perks.removed.entry(ACTOR).or_default().insert(PERK);
    e.magic.added.insert(PLAYER_REF, vec![FormId(0x90F)]);
    e.shouts.known.insert(FormId(0x910));
    e.shouts.unlocked.insert(FormId(0x910));
    e.favorites = vec![SWORD, GEM];
    e.hotkeys[2] = Some(SWORD);
    e.infos_said.insert(FormId(0x911));
    e.talked_to_pc.insert(ACTOR);
    e.crime.bounties.insert(
        FormId(0x912),
        crate::crime::Bounty {
            violent: 25,
            nonviolent: 15,
            infamy_violent: 25,
            infamy_nonviolent: 20,
        },
    );
    e.crime.stolen_value.insert(FormId(0x912), (5, 6));
    e.restocks.stocked.insert(FormId(0x907), 48.0);
    e.story.restore(crate::story::StoryState {
        last_run: [(STORY_FIRST, 200.0)].into(),
        ran: [(FormId(0x880), [STORY_FIRST].into())].into(),
        next_social: [(ACTOR, 333.0)].into(),
        last_location: Some(None),
        found_bodies: [(ACTOR, FormId(0x999))].into(),
        next_body_check: 322.0,
    });
    e.relationships
        .restore_ranks([((ACTOR, FormId(0x999)), -3)].into());
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn a_saved_game_loads_into_another_engine_unchanged() {
    let (_a, mut a) = engine("a");
    play(&mut a);
    let saved = Snapshot::capture(&a).write();
    let bytes = saved.to_bytes();
    assert_eq!(
        Snapshot::capture(&a).write().to_bytes(),
        bytes,
        "deterministic"
    );

    let (_b, mut b) = engine("b");
    Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap())
        .unwrap()
        .apply(&mut b)
        .unwrap();
    // Everything captured again is byte for byte the same file.
    assert_eq!(Snapshot::capture(&b).write().to_bytes(), bytes);

    assert_eq!((b.day, b.hour), (12, 17.5));
    assert_eq!(b.scripts.globals[&GLOBAL], 3.5);
    let q = &b.scripts.quests[&QUEST];
    assert_eq!(q.stage, 30);
    assert_eq!(q.aliases[&2], ACTOR);
    assert_eq!(q.event.as_ref().unwrap().code, *b"CLOC");
    assert_eq!(b.query_stat("Locks Picked"), 4);
    let inv = &b.inventories[&PLAYER_REF];
    assert_eq!(inv.count(SWORD), 2);
    assert_eq!(inv.hands().0, [Some(SWORD), Some(SWORD), None]);
    assert!(inv.is_equipped(SWORD));
    assert_eq!(inv.souls, vec![(GEM, 4, 1)]);
    assert_eq!(inv.charges.len(), 1);
    assert!(b.has_perk(PLAYER_REF, PERK));
    assert_eq!(b.skills.level, 9);
    assert_eq!(b.hotkeys[2], Some(SWORD));
    assert_eq!(b.crime.bounties[&FormId(0x912)].total(), 40);
    assert!(b.world_state.dead.contains_key(&ACTOR));
    assert_eq!(b.created_refs.refs.len(), 1);
    assert_eq!(b.created_refs.next, 0xFF00_0801);
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn loading_replaces_the_state_it_finds_and_refuses_other_plugins() {
    let (_a, mut a) = engine("a");
    play(&mut a);
    let dir = std::env::temp_dir().join(format!("vrm-save-files-{}", std::process::id()));
    let path = dir.join("one.vrs");
    a.save_game_to(&path).unwrap();
    assert!(!path.with_extension("tmp").exists());

    // Loading drops what the other game had.
    let (_b, mut b) = engine("b");
    b.scripts.globals.insert(FormId(0x999), 1.0);
    b.inventories.insert(FormId(0x998), Inventory::default());
    b.load_game_from(&path).unwrap();
    assert!(!b.scripts.globals.contains_key(&FormId(0x999)));
    assert!(!b.inventories.contains_key(&FormId(0x998)));
    assert_eq!(b.scripts.globals[&GLOBAL], 3.5);

    // A damaged file changes nothing.
    let mut bytes = std::fs::read(&path).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0xFF;
    std::fs::write(&path, &bytes).unwrap();
    let (_c, mut c) = engine("c");
    assert!(c.load_game_from(&path).is_err());
    assert!(c.scripts.globals.is_empty());

    // Another load order is refused.
    let mut snapshot = Snapshot::capture(&a);
    snapshot.plugins.push("Other.esp".into());
    let err = snapshot.apply(&mut c).unwrap_err();
    assert!(err.to_string().contains("other plugins"));
    assert!(c.scripts.globals.is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn script_save_requests_are_written_on_the_next_update_and_deduplicated() {
    let (_a, mut a) = engine("a");
    a.request_save(false);
    a.request_save(false);
    a.request_save(true);
    assert_eq!(a.save_requests.len(), 2);
}

#[test]
fn where_the_player_stood_survives_the_file() {
    let mut s = Snapshot::default();
    s.spot = Some(Spot {
        location: Location::Exterior {
            world: FormId(0x3C),
            center: (-3, 4),
        },
        feet: Vec3::new(10.0, 20.0, 30.0),
        yaw: 1.25,
    });
    s.hotkeys[7] = Some(FormId(5));
    let back = Snapshot::read(&s.write()).unwrap();
    assert_eq!(back.spot, s.spot);
    assert_eq!(back.hotkeys[7], Some(FormId(5)));
    assert!(back.favorites.is_empty());
}

#[test]
fn a_file_missing_a_section_is_refused() {
    let mut file = Snapshot::default().write();
    file.sections.retain(|(t, _)| t != b"GLOB");
    let err = Snapshot::read(&file).err().unwrap();
    assert!(err.0.contains("GLOB"), "{err}");
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn loading_goes_to_where_the_player_stood() {
    let (_a, mut a) = engine("a");
    a.enter_interior(CELL, Some((Vec3::new(100.0, 200.0, 0.0), 1.5)))
        .unwrap();
    let feet = a.player_feet();
    let bytes = Snapshot::capture(&a).write().to_bytes();

    let (_b, mut b) = engine("b");
    assert_eq!(b.location, Location::Nowhere);
    Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap())
        .unwrap()
        .apply(&mut b)
        .unwrap();
    assert_eq!(b.location, Location::Interior(CELL));
    assert!(
        (b.player_feet() - feet).length() < 1.0,
        "{:?} {:?}",
        b.player_feet(),
        feet
    );
    assert!((b.camera.yaw - 1.5).abs() < 1e-4);
}

/// Run the engine's VM to `now`.
fn run_vm(e: &mut Engine, now: f64, start: Option<papyrus::ObjectId>) {
    let mut vm = std::mem::take(&mut e.vm);
    {
        let mut host = crate::script::EngineHost { engine: e };
        if let Some(obj) = start {
            assert!(vm.attach(&mut host, obj, "VrmSaveTest", &[]));
            vm.send_event(&mut host, obj, "OnInit", vec![]);
        }
        vm.run(&mut host, now, 10_000);
    }
    e.vm = vm;
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn a_script_waiting_when_saved_finishes_after_loading() {
    let (_a, mut a) = engine("a");
    let quest = papyrus::ObjectId::Form(QUEST.0);
    run_vm(&mut a, 0.0, Some(quest));
    a.scripts.real_time = 3.0;
    run_vm(&mut a, 3.0, None);
    assert_eq!(a.vm.get_var(quest, "count"), Some(papyrus::Value::Int(1)));
    let bytes = Snapshot::capture(&a).write().to_bytes();

    let (_b, mut b) = engine("b");
    Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap())
        .unwrap()
        .apply(&mut b)
        .unwrap();
    assert_eq!(b.vm.thread_count(), 1);
    assert_eq!(b.vm.get_var(quest, "count"), Some(papyrus::Value::Int(1)));
    run_vm(&mut b, 9.0, None);
    assert_eq!(b.vm.get_var(quest, "count"), Some(papyrus::Value::Int(1)));
    run_vm(&mut b, 10.5, None);
    assert_eq!(b.vm.get_var(quest, "count"), Some(papyrus::Value::Int(2)));
    // The same script attached again (a cell loading) keeps what it had.
    let mut vm = std::mem::take(&mut b.vm);
    {
        let mut host = crate::script::EngineHost { engine: &mut b };
        assert!(vm.attach(&mut host, quest, "VrmSaveTest", &[]));
    }
    b.vm = vm;
    assert_eq!(b.vm.get_var(quest, "count"), Some(papyrus::Value::Int(2)));
}

/// Every runtime field is encoded without requiring a renderer or game assets.
#[test]
fn story_and_relationship_sections_are_deterministic_and_optional() {
    let mut s = Snapshot::default();
    s.story.last_run.insert(STORY_FIRST, 42.5);
    s.story
        .ran
        .insert(FormId(0x880), [STORY_FIRST, STORY_SECOND].into());
    s.story.next_social.insert(ACTOR, 100.25);
    s.story.last_location = Some(Some(CELL));
    s.story.found_bodies.insert((ACTOR, FormId(0x999)));
    s.story.found_bodies.insert((FormId(0x999), ACTOR));
    s.story.next_body_check = 101.5;
    s.relationship_ranks.insert((FormId(7), ACTOR), -4);
    s.relationship_ranks.insert((ACTOR, FormId(0x999)), 4);
    let bytes = s.write().to_bytes();
    let read = Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap()).unwrap();
    assert_eq!(read.story, s.story);
    assert_eq!(read.relationship_ranks, s.relationship_ranks);
    assert_eq!(read.write().to_bytes(), bytes);
    // Distinguish never observed, observed no location, and observed location.
    for location in [None, Some(None), Some(Some(CELL))] {
        s.story.last_location = location;
        assert_eq!(
            Snapshot::read(&s.write()).unwrap().story.last_location,
            location
        );
    }
    let mut old = s.write();
    old.sections
        .retain(|(tag, _)| tag != b"STOR" && tag != b"RELA");
    let read = Snapshot::read(&old).unwrap();
    assert_eq!(read.story, Default::default());
    assert!(read.relationship_ranks.is_empty());
}

#[test]
fn malformed_story_and_relationship_sections_are_refused() {
    let mut s = Snapshot::default();
    s.story.last_run.insert(STORY_FIRST, 42.5);
    s.story.ran.insert(FormId(0x880), [STORY_FIRST].into());
    s.story.next_social.insert(ACTOR, 100.25);
    s.story.last_location = Some(Some(CELL));
    s.story.found_bodies.insert((ACTOR, FormId(0x999)));
    s.relationship_ranks.insert((FormId(7), ACTOR), -4);
    for tag in [b"STOR", b"RELA"] {
        let file = s.write();
        let payload = file.section(tag).unwrap();
        for len in 0..payload.len() {
            let mut cut = file.clone();
            cut.sections
                .iter_mut()
                .find(|(t, _)| t == tag)
                .unwrap()
                .1
                .truncate(len);
            assert!(Snapshot::read(&cut).is_err(), "{tag:?} truncated at {len}");
        }
        let mut trailing = file;
        trailing
            .sections
            .iter_mut()
            .find(|(t, _)| t == tag)
            .unwrap()
            .1
            .push(0);
        assert!(Snapshot::read(&trailing).is_err());
    }
    for (a, b, rank) in [
        (ACTOR, FormId(7), 0),
        (FormId(7), ACTOR, -5),
        (FormId(7), ACTOR, 5),
    ] {
        let mut s = Snapshot::default();
        s.relationship_ranks.insert((a, b), rank);
        assert!(Snapshot::read(&s.write()).is_err());
    }
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn loading_preserves_story_selection_and_ranks_without_dispatching_events() {
    use crate::story::StoryEvent;
    let (_fixture, mut e) = engine("story");
    e.ai_enabled = false;
    e.update_story(); // Establish the no-location baseline.
    e.set_relationship_rank(ACTOR, FormId(7), -2);
    e.stop_quest(RANK_EVENT);
    assert!(e.send_story_event(StoryEvent::new(b"LOCK")));
    assert!(e.scripts.quests[&STORY_FIRST].running);
    e.stop_quest(STORY_FIRST);
    assert!(e.send_story_event(StoryEvent::new(b"SCPT")));
    e.stop_quest(COOLDOWN);
    let saved = Snapshot::capture(&e).write();
    // Pollute the destination's rank, rotation, cooldown and dispatch context.
    e.set_relationship_rank(ACTOR, FormId(7), 4);
    assert!(e.send_story_event(StoryEvent::new(b"LOCK")));
    e.story.active = Some(StoryEvent::new(b"TRES"));
    Snapshot::read(&saved).unwrap().apply(&mut e).unwrap();
    assert_eq!(e.relationship_rank(FormId(7), ACTOR), -2);
    assert_eq!(e.relationship_rank(ACTOR, PLAYER_REF), -2);
    assert!(!e.scripts.quests[&RANK_EVENT].running);
    assert!(e.story.active.is_none());
    e.update_story();
    assert!(
        !e.scripts
            .quests
            .get(&LOCATION_EVENT)
            .is_some_and(|q| q.running)
    );
    assert!(
        !e.send_story_event(StoryEvent::new(b"SCPT")),
        "cooldown survives load"
    );
    assert!(e.send_story_event(StoryEvent::new(b"LOCK")));
    assert!(
        e.scripts.quests[&STORY_SECOND].running,
        "rotation survives load"
    );
    assert!(!e.scripts.quests[&STORY_FIRST].running);
    e.hour += 24.0;
    assert!(
        e.send_story_event(StoryEvent::new(b"SCPT")),
        "cooldown expires normally"
    );
    // Old saves also replace stale state, using the default empty memory.
    let mut old = saved;
    old.sections
        .retain(|(tag, _)| tag != b"STOR" && tag != b"RELA");
    Snapshot::read(&old).unwrap().apply(&mut e).unwrap();
    assert_eq!(e.relationship_rank(ACTOR, FormId(7)), 0);
    assert_eq!(e.story.saved_state(), Default::default());
    assert!(e.send_story_event(StoryEvent::new(b"LOCK")));
    assert!(e.scripts.quests[&STORY_FIRST].running);
    assert!(e.send_story_event(StoryEvent::new(b"SCPT")));
}
