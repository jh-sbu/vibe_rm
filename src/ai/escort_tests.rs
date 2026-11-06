use super::*;
use crate::ai::{
    exec::tests::{ACTOR, BASE, CELL, MARKER, PACKAGE, fixture, loaded_actor},
    procedure::Cursor,
};
use std::sync::Arc;

fn node(initial: bool) -> Node {
    let mut values = vec![
        Input::ObjectList(2),
        Input::Int(2),
        Input::Location(Location {
            kind: LocationKind::NearReference(MARKER),
            radius: 100.,
        }),
        Input::Float(512.),
        Input::Float(120.),
        Input::Float(256.),
        Input::Bool(false),
        Input::Bool(true),
        Input::Float(500.),
    ];
    if initial {
        values.push(Input::Location(Location {
            kind: LocationKind::NearSelf,
            radius: 20.,
        }));
    }
    Node {
        kind: "Procedure".into(),
        procedure: "Escort".into(),
        values,
        flags: 0,
        conditions: vec![],
        args: vec![],
        children: vec![],
    }
}
fn install(e: &mut Engine, initial: bool) {
    loaded_actor(e);
    e.location = crate::engine::Location::Interior(CELL);
    e.scripts.alias_gen = 7;
    let mut p = Package::bare(PACKAGE, Behaviour::Escort);
    p.schedule.hour = -1;
    p.schedule.day_of_week = -1;
    p.tree = Some(Arc::new(node(initial)));
    let a = e.actor_mut(ACTOR).unwrap();
    a.exec.packages = vec![p];
    a.exec.current = None;
    a.exec.run = None;
    a.exec.next_eval = 0.;
    a.exec.pos = Vec3::ZERO;
    a.fresh = false;
    e.evaluate_packages(0.);
}
fn follower(e: &mut Engine, id: u32, x: f32) -> FormId {
    let id = FormId(id);
    e.created_refs.refs.insert(
        id,
        crate::created::Created {
            base: BASE,
            position: Vec3::X * x,
            rotation: Vec3::ZERO,
            origin: None,
            place: Some(Place::Interior(CELL)),
            container: None,
            location: None,
            actor: true,
            count: 1,
            owner: None,
        },
    );
    id
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn object_list_escort_waits_for_the_last_follower_and_preserves_its_wait_band() {
    let (_f, mut e) = fixture();
    install(&mut e, false);
    let near = follower(&mut e, 0xFF000801, -100.);
    let far = follower(&mut e, 0xFF000802, -1000.);
    let c = e
        .actor_mut(ACTOR)
        .unwrap()
        .exec
        .run
        .as_mut()
        .unwrap()
        .cursor
        .as_mut()
        .unwrap();
    c.lists
        .insert(2, vec![Found::Reference(near), Found::Reference(far)]);
    e.actor_mut(ACTOR).unwrap().exec.next_eval = 0.;
    e.evaluate_packages(0.);
    let goal = e.actor_ref(ACTOR).unwrap().exec.goal.unwrap();
    assert_eq!(goal.target, Some(far));
    assert_eq!(goal.follow_radius, (120., 256.));
    let x = &mut e.actor_mut(ACTOR).unwrap().exec;
    assert!(x.escort_waiting(true, false));
    assert!(x.escort_waiting(false, false));
    assert!(!x.escort_waiting(false, true));
    assert!(x.path_policy().prefer);
    e.created_refs.refs.get_mut(&far).unwrap().place = Some(Place::Interior(FormId(0x999)));
    e.actor_mut(ACTOR).unwrap().exec.next_eval = 0.;
    e.evaluate_packages(0.);
    assert_eq!(
        e.actor_ref(ACTOR).unwrap().exec.goal.unwrap().behaviour,
        Behaviour::Hold
    );
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn escort_initial_leg_does_not_finish_the_run_and_resumes_its_destination_afterwards() {
    let (_f, mut e) = fixture();
    install(&mut e, true);
    assert_eq!(
        e.actor_ref(ACTOR).unwrap().exec.goal.unwrap().centre,
        Vec3::ZERO
    );
    e.complete_arrivals();
    assert!(!e.execution(ACTOR).unwrap().run_completed());
    assert!(e.advance_escort(ACTOR));
    e.advance_loaded_trees(0.);
    e.evaluate_packages(0.);
    assert_eq!(
        e.actor_ref(ACTOR).unwrap().exec.goal.unwrap().centre,
        Vec3::X * 3000.
    );
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
            .arrived
    );
}

#[test]
#[ignore = "requires installed Skyrim SE masters and a headless GPU"]
fn vanilla_hadvar_escort_switch_and_herd_lists_keep_preferred_path_and_pacing_inputs() {
    let data = vfs::locate_data_dir().unwrap();
    let names: Vec<_> = esp::BASE_MASTERS.iter().map(|s| s.to_string()).collect();
    let e = crate::stats::action_tests::headless_engine_plugins(&data, &names);
    let p = crate::ai::package::parse(&e.lo, FormId(0x5421A)).unwrap();
    let tree = p.tree.as_ref().unwrap();
    assert!(tree.executable());
    let mut c = Cursor::default();
    c.step(tree, 0., true, |_| true, || 0);
    assert_eq!(c.body(tree).unwrap().procedure, "Escort");
    let effective = p.for_procedure(c.body(tree));
    assert_eq!(effective.follow_radius, (120., 256.));
    assert_eq!(effective.escort_wait, 600.);
    assert_eq!(effective.escort_run, 500.);
    c.step(tree, 0., true, |conditions| conditions.is_empty(), || 0);
    assert_eq!(c.body(tree).unwrap().procedure, "Travel");
    let herd = crate::ai::package::parse(&e.lo, FormId(0x964CF)).unwrap();
    assert!(herd.tree.as_ref().unwrap().executable());
    fn lists(n: &Node) -> bool {
        (n.procedure == "Escort" && matches!(n.values.first(), Some(Input::ObjectList(18))))
            || n.children.iter().any(lists)
    }
    assert!(lists(herd.tree.as_ref().unwrap()));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn away_escort_paces_the_last_follower_then_resumes_when_everyone_catches_up() {
    let (_f, mut e) = fixture();
    install(&mut e, false);
    let near = follower(&mut e, 0xFF000801, -100.);
    let far = follower(&mut e, 0xFF000802, -1000.);
    let mut x = e.actor_ref(ACTOR).unwrap().exec.clone();
    x.place = Some(Place::Interior(CELL));
    x.run
        .as_mut()
        .unwrap()
        .cursor
        .as_mut()
        .unwrap()
        .lists
        .insert(2, vec![Found::Reference(near), Found::Reference(far)]);
    let key = e.actor_cells.remove(&ACTOR).unwrap();
    e.cells
        .get_mut(&key)
        .unwrap()
        .actors
        .retain(|a| a.ref_id != ACTOR);
    e.away.insert(ACTOR, x);
    e.step_away_at(ACTOR, 1., 1.);
    let waiting = e.away[&ACTOR].pos.x;
    assert!(
        waiting < 0.,
        "approach the waiting band before continuing to the destination"
    );
    e.created_refs.refs.get_mut(&far).unwrap().position.x = waiting - 50.;
    e.created_refs.refs.get_mut(&near).unwrap().position.x = waiting - 30.;
    e.step_away_at(ACTOR, 2., 2.);
    assert!(e.away[&ACTOR].pos.x > waiting);
}
