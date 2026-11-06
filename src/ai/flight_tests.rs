use super::*;
use crate::ai::exec::tests::{ACTOR, BASE, CELL, MARKER, PACKAGE, fixture, loaded_actor};
use crate::ai::package::{Location, LocationKind};
use esp::writer::{Node as RecordNode, RecordBuilder, write_plugin};

fn leaf(name: &str, values: Vec<Input>) -> Node {
    Node {
        kind: "Procedure".into(),
        procedure: name.into(),
        flags: 0,
        conditions: vec![],
        args: vec![],
        children: vec![],
        values,
    }
}
fn location() -> Input {
    Input::Location(Location {
        kind: LocationKind::NearCurrent,
        radius: 0.,
    })
}
fn install(e: &mut Engine, tree: Node) {
    let dir = std::env::temp_dir().join(format!(
        "vrm-flight-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&dir).unwrap();
    let speeds = [0., 0., 0., 0., 1000f32, 7400., 0., 0., 1.57, 1.57]
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect::<Vec<_>>();
    let mut spit = [0; 36];
    spit[16..20].copy_from_slice(&1u32.to_le_bytes());
    spit[20..24].copy_from_slice(&2u32.to_le_bytes());
    let mut effect = [0; 152];
    effect[0] = 1;
    effect[0x44..0x48].copy_from_slice(&esp::actor_value::HEALTH.to_le_bytes());
    std::fs::write(
        dir.join("Flight.esp"),
        write_plugin(
            0,
            &[],
            "test",
            "flight",
            0x900,
            &[
                RecordNode::top(
                    b"NPC_",
                    vec![
                        RecordBuilder::new(b"NPC_", BASE)
                            .form(b"RNAM", FormId(0x810))
                            .form(b"SPLO", FormId(0x830))
                            .field(b"KWDA", &0x35D59u32.to_le_bytes()),
                    ],
                ),
                RecordNode::top(
                    b"RACE",
                    vec![RecordBuilder::new(b"RACE", FormId(0x810)).form(b"FLMV", FormId(0x811))],
                ),
                RecordNode::top(
                    b"MOVT",
                    vec![
                        RecordBuilder::new(b"MOVT", FormId(0x811))
                            .zstring(b"MNAM", "DragonFlying")
                            .field(b"SPED", &speeds),
                    ],
                ),
                RecordNode::top(
                    b"ACHR",
                    vec![RecordBuilder::new(b"ACHR", ACTOR).form(b"NAME", BASE)],
                ),
                RecordNode::top(b"PACK", vec![RecordBuilder::new(b"PACK", PACKAGE)]),
                RecordNode::top(
                    b"CELL",
                    vec![RecordBuilder::new(b"CELL", CELL).field(b"DATA", &[1, 0])],
                ),
                RecordNode::top(
                    b"SHOU",
                    vec![
                        RecordBuilder::new(b"SHOU", FormId(0x830))
                            .field(b"SNAM", &[0x31, 8, 0, 0, 0x32, 8, 0, 0, 0, 0, 0x20, 0x41]),
                    ],
                ),
                RecordNode::top(
                    b"SPEL",
                    vec![
                        RecordBuilder::new(b"SPEL", FormId(0x832))
                            .field(b"SPIT", &spit)
                            .form(b"EFID", FormId(0x833))
                            .field(b"EFIT", &[0; 12]),
                    ],
                ),
                RecordNode::top(
                    b"MGEF",
                    vec![RecordBuilder::new(b"MGEF", FormId(0x833)).field(b"DATA", &effect)],
                ),
                RecordNode::top(b"STAT", vec![RecordBuilder::new(b"STAT", FormId(0x822))]),
            ],
        ),
    )
    .unwrap();
    e.lo = esp::LoadOrder::load(&dir, &["Flight.esp".into()]).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    loaded_actor(e);
    e.scripts.alias_gen = 7;
    e.location = crate::engine::Location::Interior(CELL);
    let mut p = Package::bare(PACKAGE, Behaviour::Hold);
    p.tree = Some(Arc::new(tree));
    let a = e.actor_mut(ACTOR).unwrap();
    a.exec.pos = Vec3::ZERO;
    a.exec.place = Some(Place::Interior(CELL));
    a.exec.packages = vec![p];
    a.exec.current = Some(0);
    a.exec.begin_run(Some(0), 0., false);
    let run = a.exec.run.as_mut().unwrap();
    run.origin = Some(Vec3::ZERO);
    run.origin_place = Some(Place::Interior(CELL));
    let mut c = procedure::Cursor {
        tree_key: a.exec.packages[0].editor_id.clone(),
        ..Default::default()
    };
    c.step(
        a.exec.packages[0].tree.as_ref().unwrap(),
        0.,
        false,
        |_| true,
        || 0,
    );
    run.cursor = Some(c);
    e.created_refs.refs.insert(
        MARKER,
        crate::created::Created {
            base: FormId(0x822),
            position: Vec3::new(200., 0., 0.),
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
}

#[test]
fn orbit_joins_a_bounded_ring_and_increments_angle_at_authored_height() {
    let center = Vec3::new(100., 200., 30.);
    let target = orbit_target(center + Vec3::X * 3000., center, 1000., 2000., 500., 100.);
    assert!(((target - center).truncate().length() - 1500.).abs() < 0.1);
    assert_eq!(target.z, 530.);
    assert!(target.y > center.y);
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn orbit_takeoff_hover_forced_landing_and_short_movement_record_work() {
    let (_f, mut e) = fixture();
    install(
        &mut e,
        leaf(
            "Orbit",
            vec![
                location(),
                Input::Float(1000.),
                Input::Float(2000.),
                Input::Float(500.),
            ],
        ),
    );
    assert_eq!(e.flight_speed(ACTOR), (1000., 1.57));
    assert_eq!(
        crate::world::movement::movement_types(&e.lo)["dragonflying"].run,
        7400.
    );
    assert!(e.step_flight(ACTOR, 0.1, false));
    assert_eq!(e.flying_state(ACTOR), TAKEOFF);
    for _ in 0..50 {
        assert!(e.step_flight(ACTOR, 0.1, false));
    }
    let pos = e.actor_ref(ACTOR).unwrap().exec.pos;
    assert_eq!(e.flying_state(ACTOR), CRUISE);
    assert!((pos.z - 500.).abs() < 1.);
    assert!((1000. ..=2000.).contains(&pos.truncate().length()));
    let run = e.actor_ref(ACTOR).unwrap().exec.run.clone();
    e.set_landing_marker(ACTOR, Some(MARKER));
    e.set_allow_flying(ACTOR, false, true, true);
    for _ in 0..30 {
        e.step_flight(ACTOR, 0.1, false);
    }
    assert_eq!(e.flying_state(ACTOR), GROUND);
    assert_eq!(e.actor_ref(ACTOR).unwrap().exec.pos, Vec3::X * 200.);
    assert_eq!(e.actor_ref(ACTOR).unwrap().exec.run, run);
    let save = crate::save::state::Snapshot::capture(&e).write();
    assert_eq!(
        crate::save::state::Snapshot::read(&save)
            .unwrap()
            .continuity
            .flights,
        e.flights
    );
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn hovering_sequence_advances_and_away_flight_uses_three_dimensions() {
    let (_f, mut e) = fixture();
    let mut tree = leaf("", vec![]);
    tree.kind = "Sequence".into();
    tree.children = vec![
        leaf("Hover", vec![location(), Input::Float(500.)]),
        leaf("Wait", vec![Input::Float(0.)]),
    ];
    install(&mut e, tree);
    for _ in 0..20 {
        e.step_flight(ACTOR, 0.1, false);
    }
    e.advance_loaded_trees(0.);
    let x = e.execution(ACTOR).unwrap();
    assert!(
        x.run
            .as_ref()
            .unwrap()
            .cursor
            .as_ref()
            .unwrap()
            .body(x.packages[0].tree.as_ref().unwrap())
            .is_none()
    );
    assert_eq!(e.flying_state(ACTOR), HOVER);
    e.unload_all();
    e.location = crate::engine::Location::Nowhere;
    let before = e.away[&ACTOR].pos;
    assert!(e.step_flight(ACTOR, 1., true));
    assert_eq!(e.away[&ACTOR].pos, before);
}

#[test]
#[ignore = "requires installed Skyrim SE masters and a headless GPU"]
fn vanilla_dragon_and_alduin_trees_execute_and_flight_speed_is_authored() {
    let data = vfs::locate_data_dir().unwrap();
    let names = esp::BASE_MASTERS
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let e = crate::stats::action_tests::headless_engine_plugins(&data, &names);
    for name in [
        "Orbit",
        "Hover",
        "Shout",
        "ShoutFromOrbit",
        "ShoutFromHover",
        "ShoutFromPerch",
        "DragonPerchAndShout",
        "DefaultDragonMasterPackageTemplate",
        "dunCGAlduinFlightGrabTemplate",
        "dunCGAlduinFlightGrabNoCollisionCheckTemplate",
        "MQ304AlduinFlightGrabTemplate",
    ] {
        let id = e.lo.find_editor_id(name).unwrap();
        for p in package::expand(&e.lo, id) {
            assert!(p.tree.as_ref().unwrap().executable(), "{name}");
        }
    }
    let movement = crate::world::movement::movement_types(&e.lo);
    assert_eq!(movement["dragonflying"].walk, 2000.);
    assert_eq!(movement["dragonflying"].run, 7400.);
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn shout_procedure_charges_casts_once_saves_recovery_and_completes_its_tree() {
    let (_f, mut e) = fixture();
    install(
        &mut e,
        leaf(
            "Shout",
            vec![
                location(),
                Input::Target(package::Target::Object(FormId(0x830))),
                Input::Target(package::Target::Ref(MARKER)),
                Input::Bool(false),
            ],
        ),
    );
    e.update_package_shouts();
    let cursor = || {
        e.execution(ACTOR)
            .unwrap()
            .run
            .as_ref()
            .unwrap()
            .cursor
            .as_ref()
            .unwrap()
    };
    assert!(cursor().states[&vec![]].spoken);
    assert_eq!(cursor().states[&vec![]].count, 0);
    e.advance_loaded_trees(2.);
    e.update_package_shouts();
    let x = e.execution(ACTOR).unwrap();
    let c = x.run.as_ref().unwrap().cursor.as_ref().unwrap();
    assert_eq!(c.states[&vec![]].count, 1);
    assert!(c.states[&vec![]].finished);
    assert!(e.voice_recovery_time(ACTOR) > 0.);
    let file = crate::save::state::Snapshot::capture(&e).write();
    let loaded = crate::save::state::Snapshot::read(&file).unwrap();
    assert_eq!(
        loaded.continuity.actor_voice_recovery[&ACTOR],
        e.voice_recovery_time(ACTOR)
    );
    e.update_package_shouts();
    assert_eq!(
        e.execution(ACTOR)
            .unwrap()
            .run
            .as_ref()
            .unwrap()
            .cursor
            .as_ref()
            .unwrap()
            .states[&vec![]]
            .count,
        1
    );
    e.advance_loaded_trees(0.);
    assert!(e.execution(ACTOR).unwrap().run_completed());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn flight_grab_attaches_then_releases_target_and_advances_without_restarting_patrol() {
    let (_f, mut e) = fixture();
    install(
        &mut e,
        leaf(
            "FlightGrab",
            vec![
                Input::Target(package::Target::Ref(MARKER)),
                Input::Bool(true),
            ],
        ),
    );
    // A real actor is required for SetVehicle; use an externally placed copy.
    let victim = FormId(0xFF000820);
    let mut created = e.created_refs.refs[&MARKER].clone();
    created.actor = true;
    created.base = BASE;
    e.created_refs.refs.insert(victim, created);
    let p = &mut e.actor_mut(ACTOR).unwrap().exec.packages[0];
    Arc::make_mut(p.tree.as_mut().unwrap()).values[0] = Input::Target(package::Target::Ref(victim));
    e.step_flight(ACTOR, 0.1, false);
    assert_eq!(e.flights[&ACTOR].grabbed, Some(victim));
    assert_eq!(e.passengers[&victim].vehicle, ACTOR);
    for _ in 0..40 {
        e.step_flight(ACTOR, 0.1, false);
    }
    assert!(!e.passengers.contains_key(&victim));
    assert!(
        e.execution(ACTOR)
            .unwrap()
            .run
            .as_ref()
            .unwrap()
            .cursor
            .as_ref()
            .unwrap()
            .states[&vec![]]
            .finished
    );
}

#[test]
#[ignore = "requires installed Skyrim SE animation assets"]
fn vanilla_flight_graph_accepts_controller_events_without_furniture_root_motion() {
    use crate::world::behavior::{GraphAnim, GraphLibrary};
    use crate::world::{animation::AnimationLibrary, skeleton::Skeleton};
    let data = vfs::locate_data_dir().unwrap();
    let names = esp::BASE_MASTERS
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let vfs = vfs::Vfs::new(&data, &names);
    let skeleton = Skeleton::from_nif(
        &nif::Nif::parse(
            &vfs.read("meshes/actors/dragon/character assets/skeleton.nif")
                .unwrap(),
        )
        .unwrap(),
    );
    let mut graphs = GraphLibrary::default();
    let project = graphs
        .project(&vfs, "meshes/actors/dragon/dragonproject.hkx")
        .unwrap();
    let mut g = GraphAnim::new(
        project,
        "meshes/actors/dragon/character assets/skeleton.hkx",
        false,
        &skeleton,
        1,
    );
    let mut clips = AnimationLibrary::default();
    g.update(0., &vfs, &mut clips, &skeleton);
    for (event, state) in [
        ("TakeOff", "ST_TakeOff"),
        ("FlyStartCruise", "ST_Flight"),
        ("to_Flight_Kill_Grab_Action", "ST_Flight_Kill_Grab"),
        ("FlyStartHover", "ST_Hover"),
    ] {
        assert!(
            g.handle_event(event, &vfs, &mut clips, &skeleton),
            "{event}"
        );
        g.update(0.01, &vfs, &mut clips, &skeleton);
        assert!(
            g.active_states().iter().any(|s| s == state),
            "{event}: {:?}",
            g.active_states()
        );
        if event == "to_Flight_Kill_Grab_Action" {
            let frame = g.update(1.1, &vfs, &mut clips, &skeleton);
            let release = frame
                .raised
                .iter()
                .find(|r| r.event.eq_ignore_ascii_case("FlightActionRelease"))
                .expect("asset release trigger");
            assert_eq!(release.payload.as_deref(), Some("NPC RLegFoot"));
            assert!(!crate::anim_events::script_notification(release));
        }
    }
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn combat_shouts_select_known_hostile_words_obey_recovery_and_cancel_with_combat() {
    let (_f, mut e) = fixture();
    install(&mut e, leaf("Wait", vec![Input::Float(0.)]));
    e.actor_mut(ACTOR).unwrap().exec.combat = Some(combat::Combat::new(MARKER));
    e.update_combat_shouts(0.5);
    assert!(e.actor_ref(ACTOR).unwrap().combat_voice.is_some());
    e.update_combat_shouts(0.6);
    assert!(e.actor_ref(ACTOR).unwrap().combat_voice.is_none());
    assert!(e.voice_recovery_time(ACTOR) > 0.);
    e.update_combat_shouts(0.5);
    assert!(e.actor_ref(ACTOR).unwrap().combat_voice.is_none());
    e.set_voice_recovery_time(ACTOR, 0.);
    e.update_combat_shouts(0.1);
    assert!(e.actor_ref(ACTOR).unwrap().combat_voice.is_some());
    e.actor_mut(ACTOR).unwrap().exec.combat = None;
    e.update_combat_shouts(0.1);
    assert!(e.actor_ref(ACTOR).unwrap().combat_voice.is_none());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn a_finite_perch_leaf_waits_for_touchdown_before_the_next_leaf() {
    let (_f, mut e) = fixture();
    let mut tree = leaf("", vec![]);
    tree.kind = "Sequence".into();
    tree.children = vec![
        leaf("Sit", vec![Input::Target(package::Target::Ref(MARKER))]),
        leaf("Wait", vec![Input::Float(0.)]),
    ];
    install(&mut e, tree);
    e.actor_mut(ACTOR).unwrap().exec.pos = Vec3::Z * 1000.;
    e.step_flight(ACTOR, 0.1, false);
    assert!(
        !e.execution(ACTOR)
            .unwrap()
            .run
            .as_ref()
            .unwrap()
            .cursor
            .as_ref()
            .unwrap()
            .states[&vec![0]]
            .finished
    );
    for _ in 0..40 {
        e.step_flight(ACTOR, 0.1, false);
    }
    assert_eq!(e.flying_state(ACTOR), GROUND);
    e.advance_loaded_trees(0.);
    let x = e.execution(ACTOR).unwrap();
    assert!(
        x.run
            .as_ref()
            .unwrap()
            .cursor
            .as_ref()
            .unwrap()
            .waits_in_place(x.packages[0].tree.as_ref().unwrap())
    );
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn menu_time_advances_flight_without_ground_paths_or_restarting_the_run() {
    let (_f, mut e) = fixture();
    install(
        &mut e,
        leaf(
            "Orbit",
            vec![
                location(),
                Input::Float(1000.),
                Input::Float(2000.),
                Input::Float(500.),
            ],
        ),
    );
    // Keep this synthetic selected tree during the menu's package evaluation.
    e.actor_mut(ACTOR).unwrap().exec.next_eval = 10000.;
    e.actor_mut(ACTOR).unwrap().exec.eval_slot = Some((e.game_hours_total() * 2.).floor() as i64);
    e.update_flights(0.1);
    let before = e.actor_ref(ACTOR).unwrap().exec.pos;
    let run = e.execution(ACTOR).unwrap().run.as_ref().unwrap().started_at;
    e.advance_menu_time(0.5);
    assert!(
        e.actor_ref(ACTOR).unwrap().exec.pos.distance(before) > 100.,
        "{before:?} -> {:?}, package {:?}, run {:?}",
        e.actor_ref(ACTOR).unwrap().exec.pos,
        e.execution(ACTOR).unwrap().package(),
        e.execution(ACTOR).unwrap().run
    );
    assert!(e.actor_ref(ACTOR).unwrap().exec.pos.z > before.z);
    assert_eq!(
        e.execution(ACTOR).unwrap().run.as_ref().unwrap().started_at,
        run
    );
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn combat_without_a_landing_surface_stays_in_flight_and_disallowed_ground_stays_grounded() {
    let (_f, mut e) = fixture();
    install(&mut e, leaf("Wait", vec![Input::Float(0.)]));
    e.actor_mut(ACTOR).unwrap().exec.combat = Some(combat::Combat::new(MARKER));
    e.flights.insert(
        ACTOR,
        Flight {
            phase: CRUISE,
            combat_mode: 2,
            mode_time: 100.,
            ..Default::default()
        },
    );
    assert!(e.step_flight(ACTOR, 0.1, false));
    assert_eq!(e.flights[&ACTOR].combat_mode, 0);
    assert_eq!(e.flying_state(ACTOR), CRUISE);
    e.flights.get_mut(&ACTOR).unwrap().phase = GROUND;
    e.set_allow_flying(ACTOR, false, true, true);
    assert!(!e.step_flight(ACTOR, 1., false));
    assert_eq!(e.flying_state(ACTOR), GROUND);
}

#[test]
fn combat_phase_time_scales_vanilla_settings_and_caps_at_the_saved_clock_limit() {
    let style = [0.5, 1., 0.5, 0.25, 1., 0.5, 0.5, 0.75];
    // Vanilla fCombatHoverTimeMin (5.5): hover x 0.25, ground x 1, flying x 1.
    assert_eq!(combat_mode_time(5.5, 1, &style), 2.75);
    assert_eq!(combat_mode_time(5.5, 2, &style), 11.);
    assert_eq!(combat_mode_time(5.5, 0, &style), 11.);
    // A modded setting can't make a phase the clock never finishes or a save
    // that FLGT validation rejects.
    for huge in [1e6, f32::INFINITY] {
        for mode in 0..4 {
            assert_eq!(combat_mode_time(huge, mode, &style), MAX_CLOCK);
        }
    }
}
