//! Synthetic two-floor geometry and package state, with vanilla animation assets.
use super::*;
use crate::render::CellKey;
use std::collections::HashMap;

fn fixture() -> (Engine, ActorRuntime, CellKey) {
    let data = vfs::locate_data_dir().expect("installed vanilla Skyrim SE assets");
    let names = esp::BASE_MASTERS
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let mut e = crate::stats::action_tests::headless_engine_plugins(&data, &names);
    e.vfs = vfs::Vfs::new(&data, &names).into();
    let cell = FormId(0x133C9);
    let key = CellKey::Interior(cell);
    e.enter_interior(cell, None).unwrap();
    let mut a = e.cells.get_mut(&key).unwrap().actors.remove(0);
    assert_eq!(a.load_cell, Some(key), "interior loading marks new spawns");
    let upstairs = Vec3::new(0., 0., 600.);
    a.exec.pos = upstairs;
    a.fresh = true;
    a.load_cell = Some(key);
    a.seat = None;
    let mut p = Package::bare(FormId(0xFF000800), Behaviour::Sandbox);
    p.location = Some(package::Location {
        kind: LocationKind::InCell(cell),
        radius: 0.,
    });
    p.allow.idle_markers = true;
    let mut goal = Goal::travel(upstairs);
    goal.behaviour = Behaviour::Sandbox;
    goal.allow = p.allow;
    goal.energy = 100.;
    a.exec.packages = vec![p];
    a.exec.current = Some(0);
    a.exec.begin_run(Some(0), 0., false);
    a.exec.goal = Some(goal);
    // Keep one upstairs chair (disallowed) and one downstairs idle marker.
    let idle = e
        .furniture
        .items
        .iter()
        .position(|f| f.markers.iter().any(|m| m.kind == Use::Idle) && !f.events.is_empty())
        .unwrap();
    let mut idle = e.furniture.items.remove(idle);
    idle.markers.retain(|m| m.kind == Use::Idle);
    idle.markers.truncate(1);
    idle.markers[0].pos = Vec3::ZERO;
    let chair = furniture::Furniture {
        ref_id: FormId(0xFF000801),
        cell: key,
        owner: None,
        markers: vec![furniture::Marker {
            pos: upstairs,
            heading: 0.,
            kind: Use::Sit,
            entries: 0,
            anim_type: 1,
        }],
        events: vec![],
        idle_time: 0.,
        ways: vec![],
    };
    e.furniture = FurnitureWorld::default();
    e.furniture.items = vec![chair, idle];
    e.nav.clear();
    for (i, z) in [0., 600.].into_iter().enumerate() {
        e.nav.insert_fixture(
            FormId(0xFF000810 + i as u32),
            esp::navmesh::NavMesh {
                version: 12,
                world: FormId(0),
                cell,
                grid: (0, 0),
                vertices: vec![[-2000., -2000., z], [2000., -2000., z], [0., 2000., z]],
                triangles: vec![esp::navmesh::NavTriangle {
                    vertices: [0, 1, 2],
                    edges: [-1; 3],
                    flags: 0,
                    cover: 0,
                }],
                edge_links: vec![],
                doors: vec![],
            },
        );
    }
    (e, a, key)
}

fn plan(e: &mut Engine, a: &mut ActorRuntime) {
    let mut animations = crate::world::animation::AnimationLibrary::default();
    let mut behaviors = crate::world::animation::BehaviorLibrary::default();
    a.plan(&mut World {
        nav: &e.nav,
        furniture: &mut e.furniture,
        clips: Clips {
            vfs: &e.vfs,
            anims: &mut animations,
            behaviors: &mut behaviors,
        },
        may_use: &|_, _, _| true,
        claimed: &HashMap::new(),
        patrols: &HashMap::new(),
        targets: &HashMap::new(),
        holds: &|_, _| false,
        // This would fail the ordinary sandbox furniture chance roll.
        rand: &mut || u64::MAX,
    });
}

#[test]
#[ignore = "requires installed Skyrim SE animation assets and a headless GPU"]
fn vanilla_stay_at_current_location_preserves_a_seated_actor() {
    let (mut e, mut a, key) = fixture();
    plan(&mut e, &mut a);
    let seat = a.seat.as_mut().unwrap();
    seat.kind = Use::Sit;
    seat.anim_type = 1;
    a.state = State::Use(0.1);
    a.leave = false;
    let template = e.lo.find_editor_id("StayAtCurrentLocation").unwrap();
    let mut ps = package::expand(&e.lo, template);
    assert_eq!(ps.len(), 2);
    for p in &mut ps {
        p.schedule.hour = -1;
        p.schedule.day_of_week = -1;
    }
    a.exec.packages = ps;
    a.exec.alias_gen = e.scripts.alias_gen;
    a.exec.next_eval = 0.;
    let id = a.ref_id;
    e.cells.get_mut(&key).unwrap().actors.insert(0, a);
    e.evaluate_packages(0.);
    let a = e.actor_ref(id).unwrap();
    assert!(!a.leave);
    assert!(matches!(a.state, State::Use(_)));
    let tree = a.exec.packages[a.exec.current.unwrap()]
        .tree
        .as_ref()
        .unwrap();
    assert!(
        a.exec
            .run
            .as_ref()
            .unwrap()
            .cursor
            .as_ref()
            .unwrap()
            .waits_in_place(tree)
    );
    e.pass_time(120.);
    let a = e.actor_ref(id).unwrap();
    assert!(!a.leave);
    assert!(matches!(a.state, State::Use(_)));
}

#[test]
#[ignore = "requires installed Skyrim SE animation assets and a headless GPU"]
fn whole_cell_load_places_upstairs_sandboxer_at_downstairs_idle() {
    let (mut e, mut a, key) = fixture();
    // Reproduce the old local search: it cannot offer the downstairs marker.
    let goal = a.exec.goal.unwrap();
    assert!(!a.seek_furniture(
        &goal,
        true,
        &mut World {
            nav: &e.nav,
            furniture: &mut e.furniture,
            clips: Clips {
                vfs: &e.vfs,
                anims: &mut Default::default(),
                behaviors: &mut Default::default()
            },
            may_use: &|_, _, _| true,
            claimed: &HashMap::new(),
            patrols: &HashMap::new(),
            targets: &HashMap::new(),
            holds: &|_, _| false,
            rand: &mut || 0,
        }
    ));
    assert_eq!(a.exec.pos.z, 600.);
    plan(&mut e, &mut a);
    let seat = a
        .seat
        .as_ref()
        .expect("whole-cell load must skip chance and height limits");
    assert_eq!(seat.kind, Use::Idle);
    assert_eq!(seat.pos, Vec3::ZERO);
    assert!(a.exec.pos.z.abs() < 48., "{:?}", a.exec.pos);
    assert!(a.load_cell.is_none());
    assert!(!a.fresh);
    assert!(
        e.furniture
            .free_in_cell(Use::Idle, key, |_, _| true)
            .is_empty()
    );
    // A second actor cannot take the reserved marker.
    a.ref_id = FormId(0xFF000899);
    a.seat = None;
    a.fresh = true;
    a.load_cell = Some(key);
    a.exec.pos.z = 600.;
    plan(&mut e, &mut a);
    assert!(a.seat.is_none());
    assert_eq!(a.exec.pos.z, 600.);
}

#[test]
#[ignore = "requires installed Skyrim SE animation assets and a headless GPU"]
fn resumed_whole_cell_sandboxer_keeps_its_upstairs_position() {
    let (mut e, mut a, _) = fixture();
    let before = a.exec.pos;
    let id = a.ref_id;
    let goal = a.exec.goal;
    let packages = a.exec.packages.clone();
    e.away.insert(
        id,
        a.exec.clone().away(
            Some(Place::Interior(FormId(0x133C9))),
            None,
            Some(exec::Stage::Standing),
            (0., 0.),
        ),
    );
    e.enter_interior(FormId(0x133C9), None).unwrap();
    let actors = &mut e
        .cells
        .get_mut(&CellKey::Interior(FormId(0x133C9)))
        .unwrap()
        .actors;
    let i = actors.iter().position(|a| a.ref_id == id).unwrap();
    let mut a = actors.remove(i);
    assert!(!a.fresh);
    assert!(a.load_cell.is_none());
    assert_eq!(a.exec.pos, before);
    a.exec.packages = packages;
    a.exec.current = Some(0);
    a.exec.goal = goal;
    plan(&mut e, &mut a);
    assert!(a.seat.is_none());
    assert_eq!(a.exec.pos, before);
}

#[test]
#[ignore = "requires installed Skyrim SE animation assets and a headless GPU"]
fn fixed_idle_marker_uses_only_its_authored_marker_and_holds_it() {
    let (mut e, mut a, _) = fixture();
    let marker = e.furniture.items[1].ref_id;
    let mut goal = Goal::travel(Vec3::ZERO);
    goal.behaviour = Behaviour::Sit;
    goal.furniture = Some(marker);
    goal.any_furniture = true;
    a.exec.goal = Some(goal);
    a.exec.pos = Vec3::ZERO;
    a.fresh = false;
    a.load_cell = None;
    plan(&mut e, &mut a);
    assert_eq!(a.seat.as_ref().unwrap().furniture, marker);
    assert!(matches!(
        a.state,
        State::Walk { to_seat: true, .. } | State::Enter(_) | State::Use(_)
    ));
    assert_eq!(a.exec.goal.unwrap().furniture, Some(marker));
    assert!(a.seat.as_ref().unwrap().duration.is_infinite());
}
