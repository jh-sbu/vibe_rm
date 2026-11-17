//! A vanilla NPC's graph sneaking and standing, with installed assets.
use super::*;
use crate::render::CellKey;

fn graph_sneaks(e: &Engine, id: FormId) -> bool {
    e.actor_ref(id)
        .unwrap()
        .graph
        .as_ref()
        .unwrap()
        .active_states()
        .iter()
        .any(|s| s == "Sneak_Idle_State")
}

#[test]
#[ignore = "requires installed Skyrim SE assets and a headless GPU"]
fn a_standing_npc_sneaks_for_its_package_and_stands_for_start_sneaking() {
    let data = vfs::locate_data_dir().unwrap();
    let names: Vec<_> = esp::BASE_MASTERS.iter().map(|s| s.to_string()).collect();
    let mut e = crate::stats::action_tests::headless_engine_plugins(&data, &names);
    e.vfs = vfs::Vfs::new(&data, &names).into();
    let cell = FormId(0x133C9);
    e.enter_interior(cell, None).unwrap();
    let id = e.cells[&CellKey::Interior(cell)]
        .actors
        .iter()
        .find(|a| a.graph.is_some())
        .unwrap()
        .ref_id;
    // Out of its chair, holding its place on an Always Sneak package.
    {
        let a = e.actor_mut(id).unwrap();
        let mut p = Package::bare(FormId(0xFF000800), Behaviour::Hold);
        p.schedule.hour = -1;
        p.schedule.day_of_week = -1;
        p.sneak = true;
        a.exec.packages = vec![p];
        a.exec.current = None;
        a.exec.run = None;
        a.exec.next_eval = 0.;
        a.seat = None;
        a.state = State::Idle(5.0);
        a.graph
            .as_mut()
            .unwrap()
            .send_event("IdleForceDefaultState");
    }
    e.evaluate_packages(0.);
    for _ in 0..30 {
        e.update_actors(1.0 / 30.0);
    }
    let a = e.actor_ref(id).unwrap();
    assert!(a.wants_sneak(), "the package asks it to sneak");
    assert!(graph_sneaks(&e, id), "the graph took SneakStart");
    assert!(e.is_sneaking(id), "IsSneaking reads the stance");
    let g = a.graph.as_ref().unwrap();
    assert_eq!(g.variable("IsSneaking"), Some(1.0), "the engine writes it");

    // `StartSneaking` toggles it back to standing.
    let this = e.object_value(id);
    crate::script::natives::call(&mut e, "actor", "startsneaking", Some(&this), &[]);
    for _ in 0..30 {
        e.update_actors(1.0 / 30.0);
    }
    assert!(!graph_sneaks(&e, id), "the graph took SneakStop");
    assert!(!e.is_sneaking(id));
    let g = e.actor_ref(id).unwrap().graph.as_ref().unwrap();
    assert_eq!(g.variable("IsSneaking"), Some(0.0));
}
