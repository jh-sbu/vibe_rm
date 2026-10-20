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
