use super::*;
use crate::ai::{
    exec::tests::{ACTOR, BASE, CELL, MARKER, fixture, loaded_actor},
    package::{Location, LocationKind},
};

fn created(e: &mut Engine, id: u32, x: f32) -> FormId {
    let id = FormId(id);
    e.created_refs.refs.insert(
        id,
        crate::created::Created {
            base: e.base_of(MARKER).unwrap(),
            position: Vec3::X * x,
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
    id
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn object_search_honors_space_radius_disabled_contained_and_stable_distance_order() {
    let (_f, mut e) = fixture();
    e.location = crate::engine::Location::Interior(CELL);
    loaded_actor(&mut e);
    e.actor_mut(ACTOR).unwrap().exec.pos = Vec3::ZERO;
    let a = created(&mut e, 0xFF000810, 600.);
    let b = created(&mut e, 0xFF000811, 600.);
    let hidden = created(&mut e, 0xFF000812, 50.);
    e.scripts.disabled.insert(hidden, true);
    let contained = created(&mut e, 0xFF000813, 1.);
    e.created_refs.refs.get_mut(&contained).unwrap().container = Some(ACTOR);
    let elsewhere = created(&mut e, 0xFF000814, 2.);
    e.created_refs.refs.get_mut(&elsewhere).unwrap().place = Some(Place::Interior(FormId(0x999)));
    let mut p = Package::bare(FormId(0x900), Behaviour::Hold);
    p.location = Some(Location {
        kind: LocationKind::InCell(CELL),
        radius: 0.,
    });
    let t = Target::Object(e.base_of(MARKER).unwrap());
    assert_eq!(e.package_objects(ACTOR, &p, t), [a, b, MARKER]);
    p.location = Some(Location {
        kind: LocationKind::NearSelf,
        radius: 400.,
    });
    assert!(e.package_objects(ACTOR, &p, t).is_empty());
    p.location.as_mut().unwrap().radius = 700.;
    assert_eq!(e.package_target_of(ACTOR, &p, Some(t)), Some(a));
    assert!(
        e.package_objects(ACTOR, &p, Target::ObjectType(0))
            .is_empty()
    );
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn bed_search_preserves_own_bed_priority_and_respects_reservations_and_ownership() {
    let (_f, mut e) = fixture();
    e.location = crate::engine::Location::Interior(CELL);
    loaded_actor(&mut e);
    e.actor_mut(ACTOR).unwrap().exec.pos = Vec3::ZERO;
    let own = created(&mut e, 0xFF000810, 1000.);
    let other = created(&mut e, 0xFF000811, 600.);
    for (r, x, owner) in [(own, 1000., Some(BASE)), (other, 600., None)] {
        e.furniture.items.push(super::super::furniture::Furniture {
            ref_id: r,
            cell: crate::render::CellKey::Interior(CELL),
            owner,
            markers: vec![super::super::furniture::Marker {
                pos: Vec3::X * x,
                heading: 0.,
                kind: Use::Sleep,
                entries: 0,
                anim_type: 2,
            }],
            events: vec![],
            idle_time: 0.,
            ways: vec![],
        });
    }
    let mut p = Package::bare(FormId(0x900), Behaviour::Sleep);
    p.location = Some(Location {
        kind: LocationKind::InCell(CELL),
        radius: 0.,
    });
    let t = Target::ObjectType(26);
    assert_eq!(e.package_objects(ACTOR, &p, t), [own, other]);
    e.furniture.reserve(own, 0, FormId(0x999));
    assert_eq!(e.package_objects(ACTOR, &p, t), [other]);
    e.furniture.items[1].owner = Some(FormId(0x998));
    assert!(e.package_objects(ACTOR, &p, t).is_empty());
}

#[test]
fn type_matching_shares_food_weapon_furniture_and_spell_classification() {
    use esp::writer::{Node, RecordBuilder, write_plugin};
    let dir = std::env::temp_dir().join(format!("vrm-target-types-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let food = FormId(0x800);
    let bow = FormId(0x801);
    let sword = FormId(0x802);
    let bed = FormId(0x803);
    let spell = FormId(0x804);
    let mut enit = [0; 8];
    enit[4] = 2;
    let mut spit = [0; 24];
    spit[20] = 2;
    let groups = vec![
        Node::top(
            b"ALCH",
            vec![RecordBuilder::new(b"ALCH", food).field(b"ENIT", &enit)],
        ),
        Node::top(
            b"WEAP",
            vec![
                RecordBuilder::new(b"WEAP", bow).field(b"DNAM", &[7]),
                RecordBuilder::new(b"WEAP", sword).field(b"DNAM", &[1]),
            ],
        ),
        Node::top(
            b"FURN",
            vec![RecordBuilder::new(b"FURN", bed).field(b"FNPR", &[2, 0, 0, 0])],
        ),
        Node::top(
            b"SPEL",
            vec![RecordBuilder::new(b"SPEL", spell).field(b"SPIT", &spit)],
        ),
    ];
    std::fs::write(
        dir.join("Types.esp"),
        write_plugin(0, &[], "test", "", 0x900, &groups),
    )
    .unwrap();
    let lo = LoadOrder::load(&dir, &["Types.esp".into()]).unwrap();
    for (base, yes, no) in [
        (food, 15, 6),
        (bow, 20, 19),
        (sword, 19, 20),
        (bed, 26, 27),
        (spell, 22, 24),
    ] {
        assert!(matches_base(&lo, base, Target::ObjectType(yes)));
        assert!(!matches_base(&lo, base, Target::ObjectType(no)));
        assert!(matches_base(&lo, base, Target::Object(base)));
    }
    std::fs::remove_dir_all(dir).unwrap();
}
