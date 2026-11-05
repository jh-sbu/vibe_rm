//! Vanilla graph/script integration, using external assets and no game process.
use super::*;
use crate::world::{
    behavior::{GraphAnim, GraphLibrary},
    skeleton::Skeleton,
};
use std::sync::Arc;

#[test]
#[ignore = "requires installed Skyrim SE assets and a headless GPU"]
fn a_worker_load_event_and_lever_activation_run_the_sawmill_resource_cycle() {
    let data = vfs::locate_data_dir().unwrap();
    let names: Vec<_> = esp::BASE_MASTERS.iter().map(|s| s.to_string()).collect();
    let mut e = crate::stats::action_tests::headless_engine_plugins(&data, &names);
    e.vfs = vfs::Vfs::new(&data, &names).into();
    let mill = FormId(0x71DFC);
    let lever = FormId(0x1B752);
    let key = CellKey::Exterior(4, -13);
    e.location = Location::Exterior {
        world: FormId(0x3C),
        center: (4, -13),
    };
    let project = GraphLibrary::default()
        .project(
            &e.vfs,
            "meshes/furniture/clutter/milllogpileanimating/milllogpile_project.hkx",
        )
        .unwrap();
    let graph = GraphAnim::new(
        project,
        "meshes/furniture/clutter/milllogpileanimating/characterassets/milllogpile_character.nif",
        false,
        &Skeleton::default(),
        1,
    );
    e.cells
        .entry(key)
        .or_default()
        .animated
        .push(crate::world::animated::test_graph_object(mill, graph));
    e.scene.cells.entry(key).or_default();
    e.attach_cell_scripts(&[mill, lever]);
    e.load_activate_parents(&[mill]);
    e.send_script_event(mill, "OnLoad", vec![]);
    e.update_scripts(0.01);
    e.update_animated(0.01);
    let state = |e: &Engine| {
        e.vm.get_var(papyrus::ObjectId::Form(mill.0), "ResourceState")
            .map(|v| v.as_int())
    };
    assert_eq!(state(&e), Some(0));
    e.relay_furniture_events(mill, &["UnrelatedNotification".into()]);
    assert!(e.anim_events.pending.is_empty());
    let actor_project = GraphLibrary::default()
        .project(&e.vfs, "meshes/actors/character/defaultmale.hkx")
        .unwrap();
    let skeleton = Skeleton::default();
    let mut worker = GraphAnim::new(
        actor_project,
        "meshes/actors/character/character assets/skeleton.nif",
        false,
        &skeleton,
        2,
    );
    worker.set_variable("IsNPC", 1.);
    worker.update(0.1, &e.vfs, &mut e.anims, &skeleton);
    assert!(worker.handle_event("IdleMillLoadStart", &e.vfs, &mut e.anims, &skeleton));
    let events: Vec<_> = worker
        .update(0.01, &e.vfs, &mut e.anims, &skeleton)
        .raised
        .into_iter()
        .filter(crate::anim_events::script_notification)
        .map(|r| r.event)
        .collect();
    assert!(
        events
            .iter()
            .any(|e| e.eq_ignore_ascii_case("MillLogPileLoadStart"))
    );
    e.relay_furniture_events(mill, &events);
    e.update_scripts(0.01);
    assert_eq!(state(&e), Some(1), "worker loading makes the resource busy");
    for _ in 0..300 {
        e.update_animated(0.1);
        e.update_scripts(0.1);
        if state(&e) == Some(2) {
            break;
        }
    }
    assert_eq!(
        state(&e),
        Some(2),
        "the chute's clip notification makes it ready"
    );
    e.activate_ref(lever, Some(FormId(0x1348B)), false);
    for _ in 0..300 {
        e.update_scripts(0.1);
        e.update_activations();
        e.update_animated(0.1);
        if state(&e) == Some(0) {
            break;
        }
    }
    assert_eq!(
        state(&e),
        Some(0),
        "lever activation cuts the log and the reset returns it to waiting"
    );
}

#[test]
#[ignore = "requires installed vanilla Skyrim SE animation assets"]
fn wood_pickup_and_putdown_drive_the_carry_condition_in_the_vanilla_package() {
    let data = vfs::locate_data_dir().unwrap();
    let names: Vec<_> = esp::BASE_MASTERS.iter().map(|s| s.to_string()).collect();
    let lo = LoadOrder::load(&data, &names).unwrap();
    let vfs = vfs::Vfs::new(&data, &names);
    let project = GraphLibrary::default()
        .project(&vfs, "meshes/actors/character/defaultmale.hkx")
        .unwrap();
    let skeleton = Arc::new(Skeleton::default());
    let mut graph = GraphAnim::new(
        project,
        "meshes/actors/character/character assets/skeleton.nif",
        false,
        &skeleton,
        1,
    );
    graph.set_variable("IsNPC", 1.);
    graph.set_variable("i1stPerson", 0.);
    let mut anims = crate::world::animation::AnimationLibrary::default();
    graph.update(0.1, &vfs, &mut anims, &skeleton);
    let p = crate::ai::package::parse(&lo, FormId(0x79ADB)).unwrap();
    let root = p.tree.as_ref().unwrap();
    assert_eq!(
        root.children[0].conditions[0].string_p1.as_deref(),
        Some("isCarryItem")
    );
    let mut cursor = crate::ai::procedure::Cursor::default();
    cursor.step(root, 0., true, |_| true, || 0);
    assert_eq!(cursor.active, vec![vec![0]]);
    assert!(graph.handle_event("IdleWoodPickUpEnter", &vfs, &mut anims, &skeleton));
    let mut added = false;
    for _ in 0..600 {
        let frame = graph.update(1. / 60., &vfs, &mut anims, &skeleton);
        added |= frame
            .raised
            .iter()
            .any(|r| r.event.eq_ignore_ascii_case("AddToInventory"));
    }
    assert!(added);
    assert!(graph.handle_event("IdleForceDefaultState", &vfs, &mut anims, &skeleton));
    let offset = graph.handle_event("OffsetWoodStart", &vfs, &mut anims, &skeleton)
        || graph.handle_event("OffsetCarryLogStart", &vfs, &mut anims, &skeleton);
    assert!(offset);
    graph.update(0.1, &vfs, &mut anims, &skeleton);
    assert_eq!(graph.variable("isCarryItem"), Some(1.));
    cursor.finish(&[0]);
    cursor.step(
        root,
        0.,
        true,
        |cs| {
            cs.iter()
                .all(|c| c.string_p1.as_deref().and_then(|s| graph.variable(s)) == Some(c.value))
        },
        || 0,
    );
    assert_eq!(cursor.active, vec![vec![1]]);
    cursor.finish(&[1]);
    cursor.step(
        root,
        0.,
        true,
        |cs| {
            cs.iter()
                .all(|c| c.string_p1.as_deref().and_then(|s| graph.variable(s)) == Some(c.value))
        },
        || 0,
    );
    assert_eq!(
        cursor.active,
        vec![vec![2]],
        "the load goes to the package's carry end"
    );
    graph.handle_event("OffsetStop", &vfs, &mut anims, &skeleton);
    assert!(graph.handle_event("IdleWoodPutDownEnter", &vfs, &mut anims, &skeleton));
    let mut removed = false;
    for _ in 0..600 {
        let frame = graph.update(1. / 60., &vfs, &mut anims, &skeleton);
        removed |= frame
            .raised
            .iter()
            .any(|r| r.event.eq_ignore_ascii_case("RemoveFromInventory"));
    }
    assert!(removed);
    assert_eq!(graph.variable("isCarryItem"), Some(0.));
}
