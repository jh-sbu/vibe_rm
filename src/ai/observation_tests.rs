use super::*;
use crate::ai::{
    exec::tests::{ACTOR, CELL, MARKER, PACKAGE, fixture, loaded_actor},
    package::{Location, LocationKind, Node, Target},
    procedure::Cursor,
};
use glam::Vec3;
use std::sync::Arc;

fn leaf(name: &str, values: Vec<Input>) -> Node {
    Node {
        kind: "Procedure".into(),
        procedure: name.into(),
        values,
        flags: 0,
        conditions: vec![],
        args: vec![],
        children: vec![],
    }
}
fn install(e: &mut Engine, tree: Node) {
    loaded_actor(e);
    let mut p = Package::bare(PACKAGE, Behaviour::Hold);
    p.schedule.hour = -1;
    p.schedule.day_of_week = -1;
    p.tree = Some(Arc::new(tree));
    e.location = crate::engine::Location::Interior(CELL);
    e.scripts.alias_gen = 7;
    let a = e.actor_mut(ACTOR).unwrap();
    a.exec.packages = vec![p];
    a.exec.current = None;
    a.exec.run = None;
    a.exec.next_eval = 0.;
    a.exec.pos = Vec3::ZERO;
    a.fresh = false;
    e.evaluate_packages(0.);
}
fn loc(kind: LocationKind, radius: f32) -> Input {
    Input::Location(Location { kind, radius })
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn observation_is_limited_to_its_area_and_explicit_script_requests_override_it() {
    let (_f, mut e) = fixture();
    install(
        &mut e,
        leaf(
            "KeepAnEyeOn",
            vec![
                Input::Target(Target::Ref(MARKER)),
                loc(LocationKind::NearSelf, 4000.),
                loc(LocationKind::NearSelf, 6000.),
            ],
        ),
    );
    assert_eq!(e.observation_targets().get(&ACTOR), Some(&(MARKER, true)));
    let a = e.actor_mut(ACTOR).unwrap();
    Arc::make_mut(a.exec.packages[0].tree.as_mut().unwrap()).values[1] =
        loc(LocationKind::NearSelf, 10.);
    assert!(e.observation_targets().is_empty());
    let this = e.object_value(ACTOR);
    let target = e.object_value(MARKER);
    crate::script::natives::call(
        &mut e,
        "actor",
        "setlookat",
        Some(&this),
        &[target, papyrus::Value::Bool(false)],
    );
    assert_eq!(e.observation_targets().get(&ACTOR), Some(&(MARKER, false)));
    let saved = crate::save::state::Snapshot::capture(&e).write();
    assert_eq!(
        crate::save::state::Snapshot::read(&saved)
            .unwrap()
            .continuity
            .look_at,
        e.scripts.look_at
    );
    crate::script::natives::call(&mut e, "actor", "clearlookat", Some(&this), &[]);
    assert!(e.observation_targets().is_empty());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn follow_to_waits_for_the_followed_actor_then_enters_travel() {
    let (_f, mut e) = fixture();
    let mut root = leaf("", vec![]);
    root.kind = "Sequence".into();
    root.children = vec![
        leaf(
            "FollowTo",
            vec![
                Input::Target(Target::Ref(MARKER)),
                loc(LocationKind::NearSelf, 100.),
                Input::Float(50.),
                Input::Float(200.),
            ],
        ),
        leaf("Travel", vec![loc(LocationKind::NearSelf, 100.)]),
    ];
    install(&mut e, root);
    let goal = e.actor_ref(ACTOR).unwrap().exec.goal.unwrap();
    assert_eq!(goal.behaviour, Behaviour::Follow);
    assert_eq!(goal.follow_radius, (50., 200.));
    assert!(!e.run_follow_to(ACTOR));
    e.actor_mut(ACTOR).unwrap().exec.pos.x = 2950.;
    assert!(e.run_follow_to(ACTOR));
    e.advance_loaded_trees(0.);
    e.evaluate_packages(0.);
    assert_eq!(
        e.actor_ref(ACTOR).unwrap().exec.goal.unwrap().behaviour,
        Behaviour::Travel
    );
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn missing_fixed_idle_marker_holds_without_searching_nearby_chairs() {
    let (_f, mut e) = fixture();
    install(
        &mut e,
        leaf("UseIdleMarker", vec![Input::Target(Target::Ref(MARKER))]),
    );
    assert_eq!(
        e.actor_ref(ACTOR).unwrap().exec.goal.unwrap().behaviour,
        Behaviour::Hold
    );
    assert!(!e.execution(ACTOR).unwrap().run_completed());
}

#[test]
#[ignore = "requires installed Skyrim SE masters and a headless GPU"]
fn vanilla_group_follow_idle_and_crouch_trees_retain_the_authored_roles() {
    let data = vfs::locate_data_dir().unwrap();
    let names: Vec<_> = esp::BASE_MASTERS.iter().map(|s| s.to_string()).collect();
    let e = crate::stats::action_tests::headless_engine_plugins(&data, &names);
    for id in [
        0x6DE44, 0xC5811, 0x25E6D, 0x750BE, 0xD530D, 0x283F0, 0x6C872,
    ] {
        let p = crate::ai::package::parse(&e.lo, FormId(id)).unwrap();
        assert!(p.tree.as_ref().unwrap().executable(), "{}", p.editor_id);
    }
    let p = crate::ai::package::parse(&e.lo, FormId(0x54868)).unwrap();
    let tree = p.tree.as_ref().unwrap();
    assert!(!p.for_procedure(Some(&tree.children[0])).sneak);
    assert!(p.for_procedure(Some(&tree.children[1])).sneak);
    let mut c = Cursor::default();
    c.step(tree, 0., true, |_| true, || 0);
    assert_eq!(c.active, vec![vec![0]]);
    c.finish(&[0]);
    c.step(tree, 0., true, |_| true, || 0);
    assert_eq!(c.active, vec![vec![1]]);
    assert_eq!(
        crate::ai::combat::gmst_f32(&e.lo, "fFollowSpaceBetweenFollowers", 0.),
        192.
    );
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn group_followers_keep_separate_distance_bands_and_single_file_keeps_one_spacing() {
    let (_f, mut e) = fixture();
    install(
        &mut e,
        leaf(
            "Follow",
            vec![
                Input::Target(Target::Ref(MARKER)),
                Input::Float(100.),
                Input::Float(200.),
            ],
        ),
    );
    e.actor_mut(ACTOR).unwrap().exec.packages[0].template = "GroupMovement".into();
    let first = FormId(ACTOR.0 + 1);
    let second = FormId(ACTOR.0 + 2);
    let mut x = e.actor_ref(ACTOR).unwrap().exec.clone();
    x.place = Some(crate::ai::schedule::Place::Interior(CELL));
    e.away.insert(first, x.clone());
    e.away.insert(second, x);
    let p = &e.away[&second].packages[0];
    assert_eq!(
        e.group_follow_radius(second, p, Some(MARKER), (100., 200.)),
        (484., 584.)
    );
    let mut single = p.clone();
    single.template = "SingleFileFollow".into();
    assert_eq!(
        e.group_follow_radius(second, &single, Some(MARKER), (64., 128.)),
        (192., 192.)
    );
}
