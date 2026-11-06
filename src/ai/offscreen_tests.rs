use super::*;
use crate::ai::{
    exec::{
        ActorExec,
        tests::{ACTOR, BASE, CELL, PACKAGE, fixture},
    },
    package::{Behaviour, Location, LocationKind, Package},
};
use esp::writer::{Node, RecordBuilder, write_plugin};

const OTHER: FormId = FormId(0x821);
const NAV_A: FormId = FormId(0x900);
const NAV_B: FormId = FormId(0x901);
const DOOR_A: FormId = FormId(0x830);
const DOOR_B: FormId = FormId(0x831);
const GOAL: FormId = FormId(0x840);
const LOC_A: FormId = FormId(0x860);
const LOC_B: FormId = FormId(0x861);
const WORLD: FormId = FormId(0x870);
const KEY: FormId = FormId(0x880);

fn nvmi(mesh: FormId, cell: FormId, world: bool, door: Option<FormId>) -> Vec<u8> {
    let mut d = vec![];
    for v in [
        mesh.0,
        0,
        300f32.to_bits(),
        300f32.to_bits(),
        0,
        0,
        0,
        0,
        if door.is_some() { 1 } else { 0 },
    ] {
        d.extend(v.to_le_bytes());
    }
    if let Some(door) = door {
        d.extend(0u32.to_le_bytes());
        d.extend(door.0.to_le_bytes());
    }
    d.push(0);
    for v in [
        0,
        if world { WORLD.0 } else { 0 },
        if world { 0 } else { cell.0 },
    ] {
        d.extend(v.to_le_bytes());
    }
    d
}
fn nvnm(cell: FormId, world: bool, door: FormId) -> Vec<u8> {
    let mut d = vec![];
    for v in [
        12,
        0xA5E9A03C,
        if world { WORLD.0 } else { 0 },
        if world { 0 } else { cell.0 },
        3,
    ] {
        d.extend(v.to_le_bytes());
    }
    for v in [[0f32, 0., 0.], [1000., 0., 0.], [0., 1000., 0.]] {
        for c in v {
            d.extend(c.to_le_bytes());
        }
    }
    d.extend(1u32.to_le_bytes());
    for v in [0i16, 1, 2, -1, -1, -1, 0, 0] {
        d.extend(v.to_le_bytes());
    }
    d.extend(0u32.to_le_bytes());
    d.extend(1u32.to_le_bytes());
    d.extend(0u16.to_le_bytes());
    d.extend(0u32.to_le_bytes());
    d.extend(door.0.to_le_bytes());
    d
}
fn at(pos: Vec3) -> Vec<u8> {
    pos.to_array()
        .into_iter()
        .chain([0.; 3])
        .flat_map(|v| v.to_le_bytes())
        .collect()
}
fn engine(connected: bool, world: bool, height: f32) -> Engine {
    let (_f, mut e) = fixture();
    let dir = std::env::temp_dir().join(format!(
        "vrm-offscreen-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&dir).unwrap();
    let door = |id: FormId, partner: FormId| {
        let mut r = RecordBuilder::new(b"REFR", id)
            .form(b"NAME", FormId(0x832))
            .field(b"DATA", &at(Vec3::new(300., 300., 0.)));
        if connected {
            let mut xtel = partner.0.to_le_bytes().to_vec();
            xtel.extend(at(Vec3::new(300., 300., 0.)));
            r = r.field(b"XTEL", &xtel);
        }
        if id == DOOR_A {
            let mut xloc = vec![0u8; 12];
            xloc[4..8].copy_from_slice(&KEY.0.to_le_bytes());
            r = r.field(b"XLOC", &xloc);
        }
        r
    };
    let cell = |id: FormId,
                location: FormId,
                world: bool,
                nav: FormId,
                door: RecordBuilder,
                goal: bool| {
        let mut rec = RecordBuilder::new(b"CELL", id)
            .field(b"DATA", &[if world { 0 } else { 1 }, 0])
            .form(b"XLCN", location);
        if world {
            rec = rec.field(b"XCLC", &[0; 8]);
        }
        let mut refs = vec![
            Node::Record(door),
            Node::Record(RecordBuilder::new(b"NAVM", nav).field(
                b"NVNM",
                &nvnm(id, world, if id == CELL { DOOR_A } else { DOOR_B }),
            )),
        ];
        if goal {
            refs.push(Node::Record(
                RecordBuilder::new(b"REFR", GOAL)
                    .field(b"DATA", &at(Vec3::new(500., 100., height))),
            ));
        } else {
            refs.push(Node::Record(
                RecordBuilder::new(b"ACHR", ACTOR)
                    .form(b"NAME", BASE)
                    .field(b"DATA", &at(Vec3::new(100., 100., 0.))),
            ));
        }
        vec![
            Node::Record(rec),
            Node::Group {
                label: id.0.to_le_bytes(),
                group_type: esp::group_type::CELL_CHILDREN,
                children: vec![Node::Group {
                    label: id.0.to_le_bytes(),
                    group_type: esp::group_type::CELL_TEMPORARY,
                    children: refs,
                }],
            },
        ]
    };
    let mut groups = vec![
        Node::top(b"KEYM", vec![RecordBuilder::new(b"KEYM", KEY)]),
        Node::top(b"NPC_", vec![RecordBuilder::new(b"NPC_", BASE)]),
        Node::top(b"DOOR", vec![RecordBuilder::new(b"DOOR", FormId(0x832))]),
        Node::top(
            b"LCTN",
            vec![
                RecordBuilder::new(b"LCTN", LOC_A),
                RecordBuilder::new(b"LCTN", LOC_B),
            ],
        ),
        Node::top(b"PACK", vec![RecordBuilder::new(b"PACK", PACKAGE)]),
        Node::top(
            b"NAVI",
            vec![
                RecordBuilder::new(b"NAVI", FormId(0x12FB4))
                    .field(b"NVMI", &nvmi(NAV_A, CELL, false, Some(DOOR_A)))
                    .field(b"NVMI", &nvmi(NAV_B, OTHER, world, Some(DOOR_B))),
            ],
        ),
        Node::Group {
            label: *b"CELL",
            group_type: 0,
            children: cell(CELL, LOC_A, false, NAV_A, door(DOOR_A, DOOR_B), false),
        },
    ];
    let children = cell(OTHER, LOC_B, world, NAV_B, door(DOOR_B, DOOR_A), true);
    if world {
        groups.push(Node::Group {
            label: *b"WRLD",
            group_type: 0,
            children: vec![
                Node::Record(RecordBuilder::new(b"WRLD", WORLD)),
                Node::Group {
                    label: WORLD.0.to_le_bytes(),
                    group_type: esp::group_type::WORLD_CHILDREN,
                    children,
                },
            ],
        });
    } else {
        groups.push(Node::Group {
            label: *b"CELL",
            group_type: 0,
            children,
        });
    }
    std::fs::write(
        dir.join("Route.esp"),
        write_plugin(0, &[], "test", "coarse routes", 0xA00, &groups),
    )
    .unwrap();
    e.lo = esp::LoadOrder::load(&dir, &["Route.esp".into()]).unwrap();
    e.set_locked(DOOR_A, false);
    std::fs::remove_dir_all(dir).unwrap();
    e.location = crate::engine::Location::Nowhere;
    e.scripts.alias_gen = 7;
    let mut p = Package::bare(PACKAGE, Behaviour::Travel);
    p.schedule.hour = -1;
    p.schedule.day_of_week = -1;
    p.location = Some(Location {
        kind: LocationKind::NearReference(GOAL),
        radius: 0.,
    });
    let mut x = ActorExec::new(vec![p], Vec3::new(100., 100., 0.), 0., 0.);
    x.place = Some(Place::Interior(CELL));
    x.npc = BASE;
    x.alias_gen = 7;
    x.current = Some(0);
    x.begin_run(Some(0), 0., false);
    e.away.insert(ACTOR, x);
    e
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn away_routes_walk_through_doors_and_worldspaces_and_keep_location_events() {
    for exterior in [false, true] {
        let mut e = engine(true, exterior, 0.);
        e.step_away_at(ACTOR, 1., 1.);
        e.step_away_at(ACTOR, 5., 5.);
        let there = if exterior {
            Place::Exterior(WORLD, (0, 0))
        } else {
            Place::Interior(OTHER)
        };
        assert_eq!(e.away[&ACTOR].place, Some(there));
        assert!(!e.away[&ACTOR].run_completed());
        assert!(
            e.scripts
                .pending_events
                .iter()
                .any(|(_, name, args)| name == "OnLocationChange"
                    && args.iter().filter_map(|v| v.as_form()).collect::<Vec<_>>()
                        == vec![LOC_A.0, LOC_B.0])
        );
        e.offscreen_routes.clear();
        e.step_away_at(ACTOR, 20., 20.);
        assert!(e.away[&ACTOR].run_completed());
        assert!(e.away[&ACTOR].pos.distance(Vec3::new(500., 100., 0.)) < 45.);
    }
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn unasked_no_route_stays_put_but_an_explicit_cross_space_request_keeps_its_jump() {
    let mut e = engine(false, false, 0.);
    let start = e.away[&ACTOR].pos;
    e.step_away_at(ACTOR, 1., 1.);
    e.step_away_at(ACTOR, 100., 100.);
    assert_eq!(e.away[&ACTOR].pos, start);
    assert!(!e.away[&ACTOR].run_completed());
    e.away.get_mut(&ACTOR).unwrap().prompted = true;
    e.step_away_at(ACTOR, 101., 101.);
    assert_eq!(e.away[&ACTOR].place, Some(Place::Interior(OTHER)));
    e.step_away_at(ACTOR, 104., 104.);
    assert!(e.away[&ACTOR].run_completed());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn unreachable_height_does_not_turn_into_an_unasked_late_jump() {
    let mut e = engine(true, false, 1900.);
    let start = e.away[&ACTOR].pos;
    e.step_away_at(ACTOR, 1., 1.);
    e.step_away_at(ACTOR, 100., 100.);
    assert_eq!(e.away[&ACTOR].pos, start);
    assert!(!e.away[&ACTOR].run_completed());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn locked_route_stops_then_replans_and_unlocks_when_the_actor_has_the_key() {
    let mut e = engine(true, false, 0.);
    e.set_locked(DOOR_A, true);
    let start = e.away[&ACTOR].pos;
    e.step_away_at(ACTOR, 1., 1.);
    e.step_away_at(ACTOR, 20., 20.);
    assert_eq!(e.away[&ACTOR].pos, start);
    // Gaining the key makes the retained package travel without a prompt.
    e.inventory_mut(ACTOR).add(KEY, 1);
    e.step_away_at(ACTOR, 40., 40.);
    assert_eq!(e.away[&ACTOR].place, Some(Place::Interior(OTHER)));
    assert!(e.away[&ACTOR].run_completed());
    assert!(!e.is_locked(DOOR_A));
}
