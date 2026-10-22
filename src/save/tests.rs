//! Saving and loading through a real `Engine` (a headless GPU adapter) with
//! generated records only.

use esp::FormId;
use esp::group_type;
use esp::writer::{Node, RecordBuilder, write_plugin};
use glam::{Mat4, Vec3};

use super::codec::Writer;
use super::file::SaveFile;
use super::state::{Snapshot, Spot};
use crate::ai::schedule::Place;
use crate::engine::{Engine, Location, PLAYER_REF};
use crate::weather::SavedWeather;
use crate::world::destruction::DestructionState;
use crate::world::inventory::{Inventory, LEFT, RIGHT};
use crate::world_state::{Moved, Wounds};

const SWORD: FormId = FormId(0x800);
const SHIELD: FormId = FormId(0x801);
const GEM: FormId = FormId(0x802);
const WEATHER: FormId = FormId(0x890);
const WEATHER_OTHER: FormId = FormId(0x891);
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
const MESSAGE_WAIT_A: FormId = FormId(0x8A0);
const MESSAGE_WAIT_B: FormId = FormId(0x8A1);
const ANIMATION_WAIT_OBJECT: FormId = FormId(0x8A2);
const GREATER_POWER_A: FormId = FormId(0x8A4);
const GREATER_POWER_B: FormId = FormId(0x8A5);
const LESSER_POWER: FormId = FormId(0x8A6);

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
    let spell_record = |id: FormId, spell_type: u32, name: &str| {
        let mut spit = [0; 36];
        spit[8..12].copy_from_slice(&spell_type.to_le_bytes());
        spit[16..20].copy_from_slice(&1u32.to_le_bytes()); // fire and forget
        RecordBuilder::new(b"SPEL", id)
            .zstring(b"FULL", name)
            .field(b"SPIT", &spit)
    };
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
            b"SPEL",
            vec![
                spell_record(
                    GREATER_POWER_A,
                    crate::magic::spell_type::POWER,
                    "Greater A",
                ),
                spell_record(
                    GREATER_POWER_B,
                    crate::magic::spell_type::POWER,
                    "Greater B",
                ),
                spell_record(
                    LESSER_POWER,
                    crate::magic::spell_type::LESSER_POWER,
                    "Lesser",
                ),
            ],
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
            vec![{
                let mut dnam = [0; 100];
                dnam[0] = 1;
                RecordBuilder::new(b"WEAP", SWORD)
                    .zstring(b"FULL", "Save Test Sword")
                    .field(b"DNAM", &dnam)
            }],
        ),
        Node::top(
            b"WTHR",
            [WEATHER, WEATHER_OTHER]
                .into_iter()
                .map(|id| RecordBuilder::new(b"WTHR", id))
                .collect(),
        ),
        Node::top(
            b"MESG",
            vec![
                RecordBuilder::new(b"MESG", MESSAGE_WAIT_A)
                    .zstring(b"FULL", "First title")
                    .zstring(b"DESC", "First question")
                    .field(b"DNAM", &[1])
                    .zstring(b"ITXT", "No")
                    .zstring(b"ITXT", "Yes"),
                RecordBuilder::new(b"MESG", MESSAGE_WAIT_B)
                    .zstring(b"FULL", "Second title")
                    .zstring(b"DESC", "Second question")
                    .field(b"DNAM", &[1])
                    .zstring(b"ITXT", "A")
                    .zstring(b"ITXT", "B"),
            ],
        ),
        Node::top(b"ARMO", vec![RecordBuilder::new(b"ARMO", SHIELD)]),
        Node::top(
            b"SLGM",
            vec![
                RecordBuilder::new(b"SLGM", GEM)
                    .field(b"SOUL", &[0])
                    .field(b"SLCP", &[1]),
            ],
        ),
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
    let mut message_script = papyrus::build::ScriptBuilder::new("VrmMessageWaitTest", "Message");
    let result = message_script.variable("result", "Int", papyrus::build::Arg::None);
    let done = message_script.variable("done", "Int", papyrus::build::Arg::None);
    let mut on_init = papyrus::build::FunctionBuilder::new("None");
    on_init.call_method(papyrus::build::id("self"), "Show", result, []);
    on_init.assign(done, 1.into());
    on_init.ret(papyrus::build::Arg::None);
    message_script.function("OnInit", on_init);
    std::fs::write(
        f.0.join("scripts/VrmMessageWaitTest.pex"),
        papyrus::pex::write(&message_script.build().unwrap()).unwrap(),
    )
    .unwrap();
    std::fs::write(
        f.0.join("scripts/Message.pex"),
        papyrus::build::native_class("Message", "Form", &[("Show", "Int", &[], false)]).unwrap(),
    )
    .unwrap();
    std::fs::write(
        f.0.join("scripts/ObjectReference.pex"),
        papyrus::build::native_class(
            "ObjectReference",
            "Form",
            &[(
                "PlayAnimationAndWait",
                "Bool",
                &[("asEventName", "String"), ("asWaitForEvent", "String")],
                false,
            )],
        )
        .unwrap(),
    )
    .unwrap();
    let mut animation_script =
        papyrus::build::ScriptBuilder::new("VrmAnimationWaitTest", "ObjectReference");
    let started = animation_script.variable("started", "Bool", papyrus::build::Arg::None);
    let done = animation_script.variable("done", "Bool", papyrus::build::Arg::None);
    let mut on_init = papyrus::build::FunctionBuilder::new("None");
    on_init.assign(started, true.into());
    on_init.call_method(
        papyrus::build::id("self"),
        "PlayAnimationAndWait",
        papyrus::build::discard(),
        [
            papyrus::build::string("Start"),
            papyrus::build::string("Done"),
        ],
    );
    on_init.assign(done, true.into());
    on_init.ret(papyrus::build::Arg::None);
    animation_script.function("OnInit", on_init);
    std::fs::write(
        f.0.join("scripts/VrmAnimationWaitTest.pex"),
        papyrus::pex::write(&animation_script.build().unwrap()).unwrap(),
    )
    .unwrap();
    let e = crate::stats::action_tests::headless_engine(&f.0, "Save.esp");
    (f, e)
}

fn use_power(e: &mut Engine, power: FormId) {
    e.equip_spell(PLAYER_REF, power, crate::world::inventory::VOICE);
    e.player_use_power();
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
    e.scripts.weapon_usage.record(SWORD);
    e.scripts.weapon_usage.record(SWORD);
    e.set_weather(WEATHER, false, false);
    e.force_weather(WEATHER_OTHER, true);
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
    ws.actor_open_doors.insert(FormId(0x9A0));
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
            origin: Some(crate::created::Origin {
                position: Vec3::new(7.0, 8.0, 9.0),
                rotation: Vec3::new(0.2, 0.3, 0.4),
            }),
            place: Some(Place::Interior(CELL)),
            container: None,
            location: Some(FormId(0x90D)),
            actor: false,
            count: 3,
            owner: Some(FormId(0x91A)),
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
    assert_eq!(
        b.stat("Favorite Weapon"),
        crate::stats::StatValue::Name("Save Test Sword".into())
    );
    assert_eq!(b.query_stat("Favorite Weapon"), 0);
    // The forced weather is on, overriding; releasing it brings back the one
    // it replaced.
    assert_eq!(b.current_weather(), Some(WEATHER_OTHER));
    assert!(b.weather.overridden);
    b.release_weather_override();
    assert_eq!(b.current_weather(), Some(WEATHER));
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
    assert!(b.world_state.actor_open_doors.contains(&FormId(0x9A0)));
    assert_eq!(b.created_refs.refs.len(), 1);
    assert_eq!(b.created_refs.next, 0xFF00_0801);
    assert_eq!(
        b.created_refs.refs[&FormId(0xFF00_0800)].owner,
        Some(FormId(0x91A))
    );
    assert_eq!(
        b.created_refs.refs[&FormId(0xFF00_0800)].origin,
        a.created_refs.refs[&FormId(0xFF00_0800)].origin
    );
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
fn quicksaves_choose_the_next_append_only_number() {
    let dir = std::env::temp_dir().join(format!(
        "vrm-quicksave-names-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("quicksave 1.vrs"), b"one").unwrap();
    std::fs::write(dir.join("quicksave 4.vrs"), b"four").unwrap();
    std::fs::write(dir.join("quicksave latest.vrs"), b"malformed").unwrap();
    std::fs::write(dir.join("autosave.vrs"), b"auto").unwrap();
    std::fs::write(dir.join("quicksave.vrs"), b"legacy").unwrap();
    let unreadable_dir = dir.join("not-a-directory");
    std::fs::write(&unreadable_dir, b"file").unwrap();

    assert_eq!(super::next_quicksave_name_in(&dir).unwrap(), "quicksave 5");
    assert!(super::next_quicksave_name_in(&unreadable_dir).is_err());

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn quicksave_number_overflow_is_reported() {
    let dir = std::env::temp_dir().join(format!(
        "vrm-quicksave-overflow-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("quicksave 18446744073709551615.vrs"), b"max").unwrap();

    assert!(super::next_quicksave_name_in(&dir).is_err());

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn failed_quicksave_write_leaves_the_slot_available() {
    let dir = std::env::temp_dir().join(format!(
        "vrm-quicksave-failure-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let name = super::next_quicksave_name_in(&dir).unwrap();
    let path = super::save_path_in(&dir, &name).unwrap();
    std::fs::create_dir(path.with_extension("tmp")).unwrap();

    assert!(super::write_save_bytes(&path, b"snapshot").is_err());
    assert!(!path.exists());
    assert_eq!(super::next_quicksave_name_in(&dir).unwrap(), name);

    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn flushed_quicksaves_keep_snapshots_and_other_save_slots() {
    let (fixture, mut engine) = engine("flush");
    let dir = fixture.0.join("saves");
    std::fs::create_dir(&dir).unwrap();
    let read_day = |name: &str| {
        let bytes = std::fs::read(super::save_path_in(&dir, name).unwrap()).unwrap();
        Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap())
            .unwrap()
            .day
    };

    engine.day = 11;
    engine.request_save(false);
    engine.flush_save_requests_in(&dir);
    engine.day = 12;
    engine.request_save(false);
    engine.flush_save_requests_in(&dir);
    assert_eq!(read_day("quicksave 1"), 11);
    assert_eq!(read_day("quicksave 2"), 12);

    engine.day = 20;
    engine.save_game_to(&dir.join("manual.vrs")).unwrap();
    engine.day = 21;
    engine.request_save(true);
    engine.flush_save_requests_in(&dir);
    engine.day = 22;
    engine.request_save(true);
    engine.flush_save_requests_in(&dir);
    assert_eq!(read_day("autosave"), 22);
    assert_eq!(read_day("manual"), 20);
    assert_eq!(read_day("quicksave 1"), 11);
    assert_eq!(read_day("quicksave 2"), 12);
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

fn destruction_payload(entries: &[(FormId, f64, i32)]) -> Vec<u8> {
    let mut w = Writer::default();
    w.len(entries.len());
    for (reference, health, stage) in entries {
        w.form(*reference);
        w.f64(*health);
        w.i32(*stage);
    }
    w.0
}

fn file_with_destruction_payload(payload: Vec<u8>) -> SaveFile {
    let mut file = Snapshot::default().write();
    let (_, data) = file
        .sections
        .iter_mut()
        .find(|(tag, _)| tag == b"DSTR")
        .expect("Snapshot::write emits DSTR");
    *data = payload;
    file
}

#[test]
fn destructible_health_save_round_trip_is_stable_and_legacy_section_is_optional() {
    let untouched_threshold = FormId(0xA10);
    let destroyed = FormId(0xA11);
    let mut a = Snapshot::default();
    a.destruction.insert(
        untouched_threshold,
        DestructionState {
            health: 99.5,
            stage: -1,
        },
    );
    a.destruction.insert(
        destroyed,
        DestructionState {
            health: 0.0,
            stage: 0,
        },
    );
    let mut b = Snapshot::default();
    b.destruction.insert(
        destroyed,
        DestructionState {
            health: 0.0,
            stage: 0,
        },
    );
    b.destruction.insert(
        untouched_threshold,
        DestructionState {
            health: 99.5,
            stage: -1,
        },
    );
    let bytes = a.write().to_bytes();
    assert_eq!(
        bytes,
        b.write().to_bytes(),
        "DSTR entries sort by reference"
    );
    let read = Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap()).unwrap();
    assert_eq!(read.destruction, a.destruction);

    let mut old = Snapshot::default().write();
    old.sections.retain(|(tag, _)| tag != b"DSTR");
    assert!(Snapshot::read(&old).unwrap().destruction.is_empty());
}

#[test]
fn malformed_destructible_health_sections_are_refused() {
    let valid = destruction_payload(&[(FormId(0xA10), 50.0, 0)]);
    let mut truncated = valid.clone();
    truncated.pop();
    let malformed = [
        truncated,
        destruction_payload(&[(FormId(0xA10), f64::NAN, 0)]),
        destruction_payload(&[(FormId(0xA10), f64::INFINITY, 0)]),
        destruction_payload(&[(FormId(0xA10), -1.0, 0)]),
        destruction_payload(&[(FormId(0xA10), u32::MAX as f64 + 1.0, 0)]),
        destruction_payload(&[(FormId(0xA10), 50.0, -2)]),
        destruction_payload(&[(FormId(0xA10), 50.0, 256)]),
        destruction_payload(&[(FormId::NULL, 50.0, 0)]),
        destruction_payload(&[(FormId(0xA10), 50.0, 0), (FormId(0xA10), 40.0, 1)]),
    ];
    for payload in malformed {
        assert!(
            Snapshot::read(&file_with_destruction_payload(payload)).is_err(),
            "malformed DSTR payload should be rejected"
        );
    }

    let mut trailing = destruction_payload(&[]);
    trailing.push(0);
    assert!(Snapshot::read(&file_with_destruction_payload(trailing)).is_err());
}

fn origin_payload(entries: &[(FormId, Vec3, Vec3)]) -> Vec<u8> {
    let mut w = Writer::default();
    w.len(entries.len());
    for (id, position, rotation) in entries {
        w.form(*id);
        w.vec3(*position);
        w.vec3(*rotation);
    }
    w.0
}

#[test]
fn created_reference_origins_round_trip_in_an_optional_section() {
    let id = FormId(0xFF00_0800);
    let origin = crate::created::Origin {
        position: Vec3::new(1.0, 2.0, 3.0),
        rotation: Vec3::new(0.1, 0.2, 0.3),
    };
    let mut s = Snapshot::default();
    s.created.insert(
        id,
        crate::created::Created {
            base: SWORD,
            position: Vec3::new(10.0, 20.0, 30.0),
            rotation: Vec3::new(1.0, 2.0, 3.0),
            origin: Some(origin),
            place: Some(Place::Interior(CELL)),
            container: None,
            location: None,
            actor: false,
            count: 1,
            owner: None,
        },
    );

    let file = s.write();
    assert!(file.section(b"ORIG").is_some());
    // Current WRLD data retains its existing layout and current transform.
    let back = Snapshot::read(&file).unwrap();
    let restored = &back.created[&id];
    assert_eq!(restored.position, Vec3::new(10.0, 20.0, 30.0));
    assert_eq!(restored.rotation, Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(restored.origin, Some(origin));

    // Older saves have no ORIG section: the created reference still loads,
    // but its original transform remains unknown instead of being guessed.
    let mut old = file;
    old.sections.retain(|(tag, _)| tag != b"ORIG");
    let old = Snapshot::read(&old).unwrap();
    assert_eq!(old.created[&id].position, restored.position);
    assert_eq!(old.created[&id].origin, None);
}

#[test]
fn malformed_created_reference_origin_sections_are_refused() {
    let id = FormId(0xFF00_0800);
    let mut s = Snapshot::default();
    s.created.insert(
        id,
        crate::created::Created {
            base: SWORD,
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            origin: None,
            place: Some(Place::Interior(CELL)),
            container: None,
            location: None,
            actor: false,
            count: 1,
            owner: None,
        },
    );
    let valid = s.write();

    for payload in [
        vec![1, 0, 0, 0, 0], // one entry, cut short
        origin_payload(&[(id, Vec3::ZERO, Vec3::ZERO), (id, Vec3::ONE, Vec3::ONE)]),
        origin_payload(&[(FormId(0xDEAD), Vec3::ZERO, Vec3::ZERO)]),
        origin_payload(&[(id, Vec3::splat(f32::NAN), Vec3::ZERO)]),
    ] {
        let mut file = valid.clone();
        file.add(b"ORIG", payload);
        assert!(Snapshot::read(&file).is_err());
    }
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

fn run_message_vm(e: &mut Engine, now: f64, messages: &[FormId]) {
    let mut vm = std::mem::take(&mut e.vm);
    {
        let mut host = crate::script::EngineHost { engine: e };
        for message in messages {
            let obj = papyrus::ObjectId::Form(message.0);
            assert!(vm.attach(&mut host, obj, "VrmMessageWaitTest", &[]));
            vm.send_event(&mut host, obj, "OnInit", vec![]);
        }
        vm.run(&mut host, now, 10_000);
    }
    e.vm = vm;
}

fn install_animation_wait_graph(e: &mut Engine, sender: FormId) {
    use crate::world::behavior::{GraphAnim, ProjectRuntime};
    use havok::behavior::runtime::Shared;
    use havok::behavior::{BehaviorGraph, ClipMode, Generator, Project};
    use std::sync::Arc;

    let graph = BehaviorGraph::new(
        "SyntheticWait",
        Some(0),
        vec![Generator::Clip {
            name: "Start".into(),
            animation: "Start".into(),
            mode: ClipMode::Looping,
            speed: 1.0,
            enforced_duration: 0.0,
            user_fraction: 0.0,
            triggers: vec![],
            crop_start: 0.0,
            crop_end: 0.0,
            start_time: 0.0,
        }],
        vec!["Start".into(), "Done".into()],
    );
    let project = Arc::new(ProjectRuntime {
        shared: Shared::new(Arc::new(Project {
            graphs: vec![("SyntheticWait".into(), graph)],
            character: None,
        })),
        dir: "synthetic".into(),
        name: "wait".into(),
        files: vec![],
    });
    let skeleton = Arc::new(crate::world::skeleton::Skeleton::default());
    let graph = GraphAnim::new(project, "synthetic", false, &skeleton, 1);
    let cell = crate::render::CellKey::Interior(CELL);
    e.cells
        .entry(cell)
        .or_default()
        .animated
        .push(crate::world::animated::test_graph_object(sender, graph));
}

fn run_animation_wait_vm(e: &mut Engine, now: f64, start_count: usize) {
    let mut vm = std::mem::take(&mut e.vm);
    {
        let mut host = crate::script::EngineHost { engine: e };
        let obj = papyrus::ObjectId::Form(ANIMATION_WAIT_OBJECT.0);
        assert!(vm.attach(&mut host, obj, "VrmAnimationWaitTest", &[]));
        for _ in 0..start_count {
            assert!(vm.send_event(&mut host, obj, "OnInit", vec![]) > 0);
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

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records and PEX only"]
fn message_show_waits_restore_and_answer_on_the_original_filtered_thread() {
    use crate::messages::MessageBox;

    let (_a, mut a) = engine("message-wait-a");
    run_message_vm(&mut a, 0.0, &[MESSAGE_WAIT_A, MESSAGE_WAIT_B]);
    assert_eq!(a.messages.boxes.len(), 2);
    assert_eq!(a.messages.boxes[0].title, "First title");
    assert_eq!(a.messages.boxes[1].title, "Second title");
    assert_eq!(a.messages.next_key, 2);
    assert_eq!(a.vm.thread_count(), 2);
    assert_eq!(
        a.vm.get_var(papyrus::ObjectId::Form(MESSAGE_WAIT_A.0), "done"),
        Some(papyrus::Value::Int(0))
    );
    let bytes = Snapshot::capture(&a).write().to_bytes();

    let (_b, mut b) = engine("message-wait-b");
    b.messages.boxes.push_back(MessageBox {
        title: "stale".into(),
        text: "stale".into(),
        buttons: vec![(0, "Okay".into())],
        key: None,
        action: None,
    });
    Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap())
        .unwrap()
        .apply(&mut b)
        .unwrap();
    assert_eq!(
        b.messages.boxes.len(),
        2,
        "saved boxes replace stale UI state"
    );
    assert_eq!(
        b.messages.boxes[0].buttons,
        vec![(0, "No".into()), (1, "Yes".into())]
    );
    assert_eq!(b.messages.next_key, 2);
    run_vm(&mut b, 0.0, None);
    assert_eq!(
        b.vm.get_var(papyrus::ObjectId::Form(MESSAGE_WAIT_A.0), "done"),
        Some(papyrus::Value::Int(0)),
        "restored waits must not run until a button is answered"
    );
    assert_eq!(
        b.vm.get_var(papyrus::ObjectId::Form(MESSAGE_WAIT_B.0), "done"),
        Some(papyrus::Value::Int(0))
    );

    b.choose_message_button(1);
    run_vm(&mut b, 0.0, None);
    assert_eq!(
        b.vm.get_var(papyrus::ObjectId::Form(MESSAGE_WAIT_A.0), "result"),
        Some(papyrus::Value::Int(1))
    );
    assert_eq!(
        b.vm.get_var(papyrus::ObjectId::Form(MESSAGE_WAIT_A.0), "done"),
        Some(papyrus::Value::Int(1))
    );
    assert_eq!(
        b.vm.get_var(papyrus::ObjectId::Form(MESSAGE_WAIT_B.0), "done"),
        Some(papyrus::Value::Int(0)),
        "the second filtered thread remains suspended"
    );
    b.choose_message_button(0);
    run_vm(&mut b, 0.0, None);
    assert_eq!(
        b.vm.get_var(papyrus::ObjectId::Form(MESSAGE_WAIT_B.0), "done"),
        Some(papyrus::Value::Int(1))
    );
    assert!(b.messages.boxes.is_empty());
    assert_eq!(b.messages.next_key, 2);
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic graph and PEX only"]
fn animation_event_waits_restore_semantically_and_resume_on_matching_event() {
    let (_a, mut a) = engine("animation-wait-a");
    install_animation_wait_graph(&mut a, ANIMATION_WAIT_OBJECT);
    run_animation_wait_vm(&mut a, 0.0, 2);
    let object = papyrus::ObjectId::Form(ANIMATION_WAIT_OBJECT.0);
    assert_eq!(a.vm.thread_count(), 2);
    assert_eq!(
        a.vm.get_var(object, "started"),
        Some(papyrus::Value::Bool(true))
    );
    assert_eq!(
        a.vm.get_var(object, "done"),
        Some(papyrus::Value::Bool(false))
    );
    assert_eq!(a.anim_events.waiting().len(), 1);

    let bytes = Snapshot::capture(&a).write().to_bytes();
    let (_b, mut b) = engine("animation-wait-b");
    install_animation_wait_graph(&mut b, ANIMATION_WAIT_OBJECT);
    b.anim_events.wait_for(FormId(0x8A3), "Stale");
    Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap())
        .unwrap()
        .apply(&mut b)
        .unwrap();
    assert!(
        b.anim_events
            .waiting()
            .iter()
            .all(|(_, sender, _)| *sender == ANIMATION_WAIT_OBJECT)
    );
    assert_eq!(b.vm.thread_count(), 2);

    // A second roundtrip must retain semantic metadata after importing the VM.
    let bytes = Snapshot::capture(&b).write().to_bytes();
    let (_c, mut c) = engine("animation-wait-c");
    install_animation_wait_graph(&mut c, ANIMATION_WAIT_OBJECT);
    Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap())
        .unwrap()
        .apply(&mut c)
        .unwrap();
    assert_eq!(c.anim_events.waiting().len(), 1);
    assert_eq!(c.vm.thread_count(), 2);
    run_vm(&mut c, 0.0, None);
    assert_eq!(
        c.vm.get_var(object, "done"),
        Some(papyrus::Value::Bool(false))
    );

    c.raise_anim_events(FormId(0x8A3), &["Done".into()]);
    c.raise_anim_events(ANIMATION_WAIT_OBJECT, &["Wrong".into()]);
    run_vm(&mut c, 0.0, None);
    assert_eq!(c.vm.thread_count(), 2);
    assert_eq!(
        c.vm.get_var(object, "done"),
        Some(papyrus::Value::Bool(false))
    );

    c.raise_anim_events(ANIMATION_WAIT_OBJECT, &["dOnE".into()]);
    run_vm(&mut c, 0.0, None);
    assert_eq!(c.vm.thread_count(), 0);
    assert_eq!(
        c.vm.get_var(object, "done"),
        Some(papyrus::Value::Bool(true))
    );
    assert!(c.anim_events.waiting().is_empty());

    // The existing timed fallback remains the result when the event never comes.
    let (_d, mut d) = engine("animation-wait-timeout");
    install_animation_wait_graph(&mut d, ANIMATION_WAIT_OBJECT);
    Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap())
        .unwrap()
        .apply(&mut d)
        .unwrap();
    run_vm(&mut d, 10.5, None);
    assert_eq!(d.vm.thread_count(), 0);
    assert_eq!(
        d.vm.get_var(object, "done"),
        Some(papyrus::Value::Bool(true))
    );
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn greater_power_cooldowns_survive_load_and_expire_after_a_game_day() {
    let (_a, mut a) = engine("power-cooldown-a");
    a.day = 5;
    a.hour = 6.0;
    use_power(&mut a, GREATER_POWER_A);
    use_power(&mut a, LESSER_POWER);
    let saved = Snapshot::capture(&a);
    assert_eq!(saved.power_uses.len(), 1);
    assert!(saved.power_uses.contains_key(&GREATER_POWER_A));
    assert!(!saved.power_uses.contains_key(&LESSER_POWER));
    let bytes = saved.write().to_bytes();

    let (_b, mut b) = engine("power-cooldown-b");
    Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap())
        .unwrap()
        .apply(&mut b)
        .unwrap();
    b.scripts.notifications.clear();
    use_power(&mut b, GREATER_POWER_B);
    assert!(
        b.scripts.notifications.is_empty(),
        "an unused power is independent"
    );
    use_power(&mut b, GREATER_POWER_A);
    use_power(&mut b, GREATER_POWER_B);
    assert_eq!(
        b.scripts.notifications.len(),
        2,
        "both daily powers are blocked"
    );
    use_power(&mut b, LESSER_POWER);
    use_power(&mut b, LESSER_POWER);
    assert_eq!(
        b.scripts.notifications.len(),
        2,
        "lesser powers have no daily cooldown"
    );
    assert!(!b.casting.power_uses().contains_key(&LESSER_POWER));

    b.day = 6;
    b.hour = 6.0;
    use_power(&mut b, GREATER_POWER_A);
    assert_eq!(
        b.scripts.notifications.len(),
        2,
        "the cooldown expires at 24 hours"
    );
    use_power(&mut b, GREATER_POWER_A);
    assert_eq!(
        b.scripts.notifications.len(),
        3,
        "use starts a new daily cooldown"
    );

    let mut legacy = SaveFile::from_bytes(&bytes).unwrap();
    legacy.sections.retain(|(tag, _)| tag != b"PWCD");
    let (_c, mut c) = engine("power-cooldown-legacy");
    c.day = 5;
    c.hour = 6.0;
    use_power(&mut c, GREATER_POWER_A);
    c.scripts.notifications.clear();
    Snapshot::read(&legacy).unwrap().apply(&mut c).unwrap();
    c.scripts.notifications.clear();
    use_power(&mut c, GREATER_POWER_A);
    assert!(
        c.scripts.notifications.is_empty(),
        "legacy save absence replaces stale destination cooldowns"
    );

    let mut bad = Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap()).unwrap();
    bad.power_uses.clear();
    bad.power_uses.insert(SWORD, 5.0);
    let (_d, mut d) = engine("power-cooldown-invalid-form");
    d.day = 99;
    assert!(
        bad.apply(&mut d).is_err(),
        "a non-power record cannot own a cooldown"
    );
    assert_eq!(
        d.day, 99,
        "validation rejects the save before mutating the engine"
    );
}

#[test]
#[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
fn spent_souls_restore_across_load_and_legacy_saves_clear_them() {
    let (_a, mut a) = engine("spent-soul-a");
    a.world_state
        .dead
        .insert(ACTOR, (Place::Interior(CELL), Vec3::ZERO));
    a.world_state
        .dead
        .insert(FormId(7), (Place::Interior(CELL), Vec3::ONE));
    assert!(
        !a.trap_soul(PLAYER_REF, FormId(7)),
        "failed capture with no gem is retryable"
    );
    a.inventory_mut(PLAYER_REF).add(GEM, 4);
    assert!(a.trap_soul(PLAYER_REF, ACTOR));
    assert!(a.trap_soul(PLAYER_REF, FormId(7)));
    assert_eq!(a.magic.souls_taken, [ACTOR, FormId(7)].into());
    let bytes = Snapshot::capture(&a).write().to_bytes();

    let (_b, mut b) = engine("spent-soul-b");
    Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap())
        .unwrap()
        .apply(&mut b)
        .unwrap();
    b.inventory_mut(PLAYER_REF).add(GEM, 1);
    let gems = b.inventory_mut(PLAYER_REF).count(GEM);
    assert!(
        !b.trap_soul(PLAYER_REF, ACTOR),
        "a spent body remains spent"
    );
    assert_eq!(b.inventory_mut(PLAYER_REF).count(GEM), gems);
    assert!(!b.trap_soul(PLAYER_REF, FormId(7)));
    assert!(b.magic.souls_taken.contains(&FormId(7)));

    let mut legacy = SaveFile::from_bytes(&bytes).unwrap();
    legacy.sections.retain(|(tag, _)| tag != b"SOUL");
    let (_c, mut c) = engine("spent-soul-legacy");
    c.magic.souls_taken.insert(ACTOR);
    Snapshot::read(&legacy).unwrap().apply(&mut c).unwrap();
    assert!(
        c.magic.souls_taken.is_empty(),
        "legacy absence replaces stale state"
    );
    assert!(c.trap_soul(PLAYER_REF, ACTOR));
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
fn greater_power_cooldown_section_round_trips_and_rejects_bad_entries() {
    let mut s = Snapshot::default();
    s.power_uses.insert(GREATER_POWER_A, 17.25);
    s.power_uses.insert(GREATER_POWER_B, 22.0);
    let valid = s.write();
    let bytes = valid.to_bytes();
    let back = Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap()).unwrap();
    assert_eq!(back.power_uses, s.power_uses);
    assert_eq!(back.write().to_bytes(), bytes);

    let mut legacy = valid.clone();
    legacy.sections.retain(|(tag, _)| tag != b"PWCD");
    assert!(Snapshot::read(&legacy).unwrap().power_uses.is_empty());

    let mut malformed = Vec::new();
    let mut duplicate = valid.clone();
    let payload = &mut duplicate
        .sections
        .iter_mut()
        .find(|(tag, _)| tag == b"PWCD")
        .unwrap()
        .1;
    let first = payload[4..12].to_vec();
    payload[0..4].copy_from_slice(&3u32.to_le_bytes());
    payload.extend_from_slice(&first);
    malformed.push(duplicate);

    let mut truncated = valid.clone();
    truncated
        .sections
        .iter_mut()
        .find(|(tag, _)| tag == b"PWCD")
        .unwrap()
        .1
        .pop();
    malformed.push(truncated);
    let mut trailing = valid.clone();
    trailing
        .sections
        .iter_mut()
        .find(|(tag, _)| tag == b"PWCD")
        .unwrap()
        .1
        .push(0);
    malformed.push(trailing);
    let mut unsorted = valid.clone();
    let payload = &mut unsorted
        .sections
        .iter_mut()
        .find(|(tag, _)| tag == b"PWCD")
        .unwrap()
        .1;
    let first = payload[4..12].to_vec();
    let second = payload[12..20].to_vec();
    payload[4..12].copy_from_slice(&second);
    payload[12..20].copy_from_slice(&first);
    malformed.push(unsorted);
    for (offset, value) in [(0, 0u32), (8, f32::NAN.to_bits()), (8, (-1.0f32).to_bits())] {
        let mut bad = valid.clone();
        let payload = &mut bad
            .sections
            .iter_mut()
            .find(|(tag, _)| tag == b"PWCD")
            .unwrap()
            .1;
        if offset == 0 {
            payload[4..8].copy_from_slice(&value.to_le_bytes());
        } else {
            payload[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        malformed.push(bad);
    }
    for file in malformed {
        assert!(Snapshot::read(&file).is_err());
    }
}

#[test]
fn spent_soul_section_is_deterministic_and_rejects_bad_entries() {
    let mut s = Snapshot::default();
    s.spent_souls.extend([ACTOR, FormId(7)]);
    let valid = s.write();
    let bytes = valid.to_bytes();
    let back = Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap()).unwrap();
    assert_eq!(back.spent_souls, s.spent_souls);
    assert_eq!(back.write().to_bytes(), bytes);

    let mut legacy = valid.clone();
    legacy.sections.retain(|(tag, _)| tag != b"SOUL");
    assert!(Snapshot::read(&legacy).unwrap().spent_souls.is_empty());

    let mut malformed = Vec::new();
    let mut null = valid.clone();
    null.sections
        .iter_mut()
        .find(|(tag, _)| tag == b"SOUL")
        .unwrap()
        .1[4..8]
        .copy_from_slice(&0u32.to_le_bytes());
    malformed.push(null);
    let mut duplicate = valid.clone();
    let payload = &mut duplicate
        .sections
        .iter_mut()
        .find(|(tag, _)| tag == b"SOUL")
        .unwrap()
        .1;
    let first = payload[4..8].to_vec();
    payload[0..4].copy_from_slice(&3u32.to_le_bytes());
    payload.extend_from_slice(&first);
    malformed.push(duplicate);
    let mut truncated = valid.clone();
    truncated
        .sections
        .iter_mut()
        .find(|(tag, _)| tag == b"SOUL")
        .unwrap()
        .1
        .pop();
    malformed.push(truncated);
    let mut trailing = valid;
    trailing
        .sections
        .iter_mut()
        .find(|(tag, _)| tag == b"SOUL")
        .unwrap()
        .1
        .push(0);
    malformed.push(trailing);
    for file in malformed {
        assert!(Snapshot::read(&file).is_err());
    }
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
fn favorite_weapon_usage_section_is_optional_deterministic_and_validated() {
    let mut s = Snapshot::default();
    s.weapon_usage = vec![(FormId(0x901), 2), (FormId(0x902), 1)];
    s.favorite_weapon = Some(FormId(0x901));
    let bytes = s.write().to_bytes();
    let mut reversed = Snapshot::default();
    reversed.weapon_usage = vec![(FormId(0x902), 1), (FormId(0x901), 2)];
    reversed.favorite_weapon = Some(FormId(0x901));
    assert_eq!(reversed.write().to_bytes(), bytes);
    assert_eq!(
        Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap())
            .unwrap()
            .write()
            .to_bytes(),
        bytes
    );

    let mut old = s.write();
    old.sections.retain(|(tag, _)| tag != b"WUSE");
    let read = Snapshot::read(&old).unwrap();
    assert!(read.weapon_usage.is_empty());
    assert_eq!(read.favorite_weapon, None);

    for (counts, favorite) in [
        (vec![(FormId::NULL, 1)], Some(FormId::NULL)),
        (vec![(FormId(0x901), 0)], Some(FormId(0x901))),
        (
            vec![(FormId(0x901), 1), (FormId(0x901), 1)],
            Some(FormId(0x901)),
        ),
        (vec![(FormId(0x901), 1)], Some(FormId(0x902))),
    ] {
        let mut malformed = Snapshot::default();
        malformed.weapon_usage = counts;
        malformed.favorite_weapon = favorite;
        assert!(Snapshot::read(&malformed.write()).is_err());
    }
}

#[test]
fn weather_section_is_optional_and_validated() {
    let mut s = Snapshot::default();
    s.weather = SavedWeather {
        current: Some(WEATHER),
        outgoing: Some(WEATHER_OTHER),
        pct: 0.25,
        overridden: true,
        before_override: Some(FormId(0x892)),
        next_roll: 3.5,
    };
    let bytes = s.write().to_bytes();
    let read = Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap()).unwrap();
    assert_eq!(read.weather, s.weather);
    assert_eq!(read.write().to_bytes(), bytes);

    // Saves from before the section keep no weather: the place picks one.
    let mut old = s.write();
    old.sections.retain(|(tag, _)| tag != b"WTHR");
    assert_eq!(
        Snapshot::read(&old).unwrap().weather,
        SavedWeather::default()
    );

    for weather in [
        SavedWeather {
            pct: 1.5,
            ..s.weather
        },
        SavedWeather {
            pct: f32::NAN,
            ..s.weather
        },
        SavedWeather {
            next_roll: f32::INFINITY,
            ..s.weather
        },
        SavedWeather {
            current: None,
            ..s.weather
        },
    ] {
        let mut malformed = Snapshot::default();
        malformed.weather = weather;
        assert!(Snapshot::read(&malformed.write()).is_err());
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

#[test]
fn deaths_section_keeps_killers_and_times_and_is_optional() {
    use crate::world_state::Death;
    let mut s = Snapshot::default();
    s.deaths.insert(
        FormId(0x8a0),
        Death {
            killer: Some(crate::engine::PLAYER_REF),
            at: 1234.5,
        },
    );
    s.deaths.insert(
        FormId(0x8a1),
        Death {
            killer: None,
            at: 7.25,
        },
    );
    let bytes = s.write().to_bytes();
    let read = Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap()).unwrap();
    assert_eq!(read.deaths, s.deaths);
    assert_eq!(read.write().to_bytes(), bytes);

    // Saves from before the section know only the bodies.
    let mut old = s.write();
    old.sections.retain(|(tag, _)| tag != b"DETH");
    assert!(Snapshot::read(&old).unwrap().deaths.is_empty());

    let mut malformed = Snapshot::default();
    malformed.deaths.insert(
        FormId(0x8a0),
        Death {
            killer: None,
            at: f64::NAN,
        },
    );
    assert!(Snapshot::read(&malformed.write()).is_err());
}

fn stealth_points_payload(entries: &[(FormId, f32, f32)]) -> Vec<u8> {
    let mut w = Writer::default();
    w.len(entries.len());
    for (actor, points, regen_wait) in entries {
        w.form(*actor);
        w.f32(*points);
        w.f32(*regen_wait);
    }
    w.0
}

#[test]
fn stealth_point_section_is_sorted_optional_and_rejects_invalid_state() {
    let mut s = Snapshot::default();
    s.stealth_points.insert(FormId(0x8b1), (37.5, 4.0));
    s.stealth_points.insert(PLAYER_REF, (62.25, 0.0));
    let file = s.write();
    let bytes = file.to_bytes();
    assert_eq!(
        s.write().to_bytes(),
        bytes,
        "stealth entries sort deterministically"
    );
    let read = Snapshot::read(&SaveFile::from_bytes(&bytes).unwrap()).unwrap();
    assert_eq!(read.stealth_points, s.stealth_points);
    assert_eq!(read.write().to_bytes(), bytes);

    let mut old = file.clone();
    old.sections.retain(|(tag, _)| tag != b"STPT");
    assert!(Snapshot::read(&old).unwrap().stealth_points.is_empty());

    let malformed = [
        stealth_points_payload(&[(FormId(0x8b0), f32::NAN, 0.0)]),
        stealth_points_payload(&[(FormId(0x8b0), 100.1, 0.0)]),
        stealth_points_payload(&[(FormId(0x8b0), -0.1, 0.0)]),
        stealth_points_payload(&[(FormId(0x8b0), 50.0, f32::NAN)]),
        stealth_points_payload(&[(FormId(0x8b0), 50.0, f32::INFINITY)]),
        stealth_points_payload(&[(FormId(0x8b0), 50.0, -0.1)]),
        stealth_points_payload(&[(FormId::NULL, 50.0, 0.0)]),
        stealth_points_payload(&[(FormId(0x8b0), 50.0, 0.0), (FormId(0x8b0), 40.0, 1.0)]),
    ];
    for payload in malformed {
        let mut file = Snapshot::default().write();
        file.sections.retain(|(tag, _)| tag != b"STPT");
        file.add(b"STPT", payload);
        assert!(Snapshot::read(&file).is_err());
    }
    for cut in 0..stealth_points_payload(&[(FormId(0x8b0), 50.0, 0.0)]).len() {
        let mut file = Snapshot::default().write();
        file.sections.retain(|(tag, _)| tag != b"STPT");
        let mut payload = stealth_points_payload(&[(FormId(0x8b0), 50.0, 0.0)]);
        payload.truncate(cut);
        file.add(b"STPT", payload);
        assert!(Snapshot::read(&file).is_err());
    }
    let mut trailing = Snapshot::default().write();
    trailing
        .sections
        .iter_mut()
        .find(|(tag, _)| tag == b"STPT")
        .unwrap()
        .1
        .push(0);
    assert!(Snapshot::read(&trailing).is_err());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic plugin only"]
fn loading_clears_sleep_wait_and_owned_fade_transients() {
    let (_fixture, mut e) = engine("sleep-transients");
    let saved = Snapshot::capture(&e);
    e.menu = Some(crate::items::Menu::SleepWait { sleeping: true });
    e.start_waiting(4);
    e.update(Default::default(), 0.25, 20.0);
    assert!(e.waiting.is_some());
    assert!(e.imagespace.sleep_fade_amount() > 0.0);

    saved.apply(&mut e).unwrap();
    assert!(e.waiting.is_none());
    assert_ne!(
        e.menu,
        Some(crate::items::Menu::SleepWait { sleeping: true })
    );
    assert_eq!(e.imagespace.sleep_fade_amount(), 0.0);
    assert_eq!(e.scene.post.fade[3], 0.0);
}
