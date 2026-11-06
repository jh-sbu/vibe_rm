use super::*;
use crate::ai::{
    exec::{
        ActorExec,
        tests::{ACTOR, BASE, CELL, MARKER, PACKAGE, fixture, loaded_actor},
    },
    package::{Behaviour, Location, LocationKind, Node},
    procedure::Cursor,
    schedule::Place,
};
use glam::Vec3;
use std::sync::Arc;

const KEY: FormId = FormId(0x850);
const HOLDER: FormId = FormId(0x803);
const FACTION: FormId = FormId(0x860);
const LIST: FormId = FormId(0x861);
const CYCLE: FormId = FormId(0x862);

fn leaf(name: &str, values: Vec<Input>) -> Node {
    Node {
        kind: "Procedure".into(),
        flags: 0,
        conditions: vec![],
        children: vec![],
        procedure: name.into(),
        args: vec![],
        values,
    }
}
fn branch(kind: &str, children: Vec<Node>) -> Node {
    Node {
        kind: kind.into(),
        children,
        ..leaf("", vec![])
    }
}
fn package(own: bool, random: bool, wander: bool) -> Package {
    let loc = Input::Location(Location {
        kind: LocationKind::InCell(CELL),
        radius: 0.,
    });
    let find = leaf(
        "Find",
        vec![
            loc,
            Input::Target(Target::Object(KEY)),
            Input::ObjectList(2),
            Input::Bool(own),
            Input::Bool(random),
        ],
    );
    let find = if wander {
        branch("Simultaneous", vec![find, leaf("Wander", vec![loc])])
    } else {
        find
    };
    let mut p = Package::bare(PACKAGE, Behaviour::Hold);
    p.schedule.hour = -1;
    p.schedule.day_of_week = -1;
    p.tree = Some(Arc::new(branch(
        "Sequence",
        vec![
            find,
            leaf("Wait", vec![Input::Float(0.), Input::Bool(true)]),
        ],
    )));
    p
}

fn engine() -> Engine {
    use esp::writer::{Node as R, RecordBuilder, write_plugin};
    let (_f, mut e) = fixture();
    let dir = std::env::temp_dir().join(format!(
        "vrm-find-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let mut at = [0; 24];
    at[..4].copy_from_slice(&200f32.to_le_bytes());
    let mut holder_at = [0; 24];
    holder_at[..4].copy_from_slice(&100f32.to_le_bytes());
    let groups = vec![
        R::top(
            b"NPC_",
            vec![
                RecordBuilder::new(b"NPC_", BASE)
                    .field(b"SNAM", &[FACTION.0.to_le_bytes(), [0; 4]].concat()),
            ],
        ),
        R::top(b"FACT", vec![RecordBuilder::new(b"FACT", FACTION)]),
        R::top(
            b"FLST",
            vec![
                RecordBuilder::new(b"FLST", LIST).form(b"LNAM", FACTION),
                RecordBuilder::new(b"FLST", CYCLE)
                    .form(b"LNAM", LIST)
                    .form(b"LNAM", CYCLE),
            ],
        ),
        R::top(
            b"KEYM",
            vec![RecordBuilder::new(b"KEYM", KEY).field(b"DATA", &[0; 8])],
        ),
        R::Group {
            label: *b"CELL",
            group_type: 0,
            children: vec![
                R::Record(RecordBuilder::new(b"CELL", CELL).field(b"DATA", &[1, 0])),
                R::Group {
                    label: CELL.0.to_le_bytes(),
                    group_type: esp::group_type::CELL_CHILDREN,
                    children: vec![R::Group {
                        label: CELL.0.to_le_bytes(),
                        group_type: esp::group_type::CELL_TEMPORARY,
                        children: vec![
                            R::Record(RecordBuilder::new(b"ACHR", ACTOR).form(b"NAME", BASE)),
                            R::Record(
                                RecordBuilder::new(b"ACHR", HOLDER)
                                    .form(b"NAME", BASE)
                                    .field(b"DATA", &holder_at),
                            ),
                            R::Record(
                                RecordBuilder::new(b"REFR", MARKER)
                                    .form(b"NAME", KEY)
                                    .field(b"DATA", &at),
                            ),
                        ],
                    }],
                },
            ],
        },
    ];
    std::fs::write(
        dir.join("Find.esp"),
        write_plugin(0, &[], "test", "find object lists", 0x900, &groups),
    )
    .unwrap();
    e.lo = esp::LoadOrder::load(&dir, &["Find.esp".into()]).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
    e.location = crate::engine::Location::Interior(CELL);
    e.scripts.alias_gen = 7;
    e.inventory_mut(HOLDER).add(KEY, 2);
    e.inventory_mut(ACTOR).add(KEY, 3);
    e
}
fn loaded(e: &mut Engine, p: Package) {
    loaded_actor(e);
    let a = e.actor_mut(ACTOR).unwrap();
    a.exec.packages = vec![p];
    a.exec.current = None;
    a.exec.run = None;
    a.exec.next_eval = 0.;
    a.exec.pos = Vec3::ZERO;
    a.fresh = false;
    e.evaluate_packages(0.);
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn find_records_world_and_holder_items_and_adds_own_inventory_only_when_requested() {
    for own in [false, true] {
        let mut e = engine();
        loaded(&mut e, package(own, true, false));
        e.run_loaded_finds();
        let c = e
            .execution(ACTOR)
            .unwrap()
            .run
            .as_ref()
            .unwrap()
            .cursor
            .as_ref()
            .unwrap();
        assert_eq!(c.active, vec![vec![1]]);
        let list = &c.lists[&2];
        assert_eq!(list.len(), if own { 3 } else { 2 });
        assert!(list.contains(&Found::Reference(MARKER)));
        assert!(list.contains(&Found::Inventory {
            owner: HOLDER,
            item: KEY
        }));
        assert_eq!(
            list.contains(&Found::Inventory {
                owner: ACTOR,
                item: KEY
            }),
            own
        );
        let saved = crate::save::state::Snapshot::capture(&e).write();
        let copy = crate::save::state::Snapshot::read(&saved).unwrap();
        assert_eq!(
            copy.continuity.executions[&ACTOR]
                .run
                .as_ref()
                .unwrap()
                .cursor
                .as_ref()
                .unwrap()
                .lists,
            c.lists
        );
    }
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn object_id_criteria_expand_factions_and_nested_dynamic_form_lists_without_cycles() {
    let mut e = engine();
    loaded(&mut e, package(false, false, false));
    let mut p = Package::bare(PACKAGE, Behaviour::Hold);
    p.location = Some(Location {
        kind: LocationKind::InCell(CELL),
        radius: 0.,
    });
    assert_eq!(
        e.package_objects(ACTOR, &p, Target::Object(FACTION)),
        vec![HOLDER]
    );
    assert_eq!(
        e.package_objects(ACTOR, &p, Target::Object(CYCLE)),
        vec![HOLDER]
    );
    assert!(!e.matches_package_item(KEY, Target::Object(CYCLE)));
    e.add_to_formlist(LIST, KEY);
    assert!(e.matches_package_item(KEY, Target::Object(CYCLE)));
    assert_eq!(
        e.package_objects(ACTOR, &p, Target::Object(CYCLE)),
        vec![HOLDER, MARKER]
    );
    e.remove_added_form(LIST, KEY);
    assert!(!e.matches_package_item(KEY, Target::Object(CYCLE)));
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn an_empty_find_wanders_and_retries_before_advancing_when_an_item_appears() {
    let mut e = engine();
    e.scripts.disabled.insert(MARKER, true);
    e.inventory_mut(HOLDER).remove_split(KEY, 2, None);
    loaded(&mut e, package(false, false, true));
    e.run_loaded_finds();
    assert_eq!(
        e.execution(ACTOR)
            .unwrap()
            .run
            .as_ref()
            .unwrap()
            .cursor
            .as_ref()
            .unwrap()
            .active,
        vec![vec![0, 0], vec![0, 1]]
    );
    let goal = e.actor_ref(ACTOR).unwrap().exec.goal.unwrap();
    assert_eq!(goal.behaviour, Behaviour::Sandbox);
    assert!(goal.allow.wandering && !goal.allow.sitting && !goal.allow.idle_markers);
    e.inventory_mut(HOLDER).add(KEY, 1);
    assert!(
        !e.run_find(ACTOR),
        "a repeated request respects the search cadence"
    );
    e.advance_loaded_trees(3.);
    e.run_loaded_finds();
    let c = e
        .execution(ACTOR)
        .unwrap()
        .run
        .as_ref()
        .unwrap()
        .cursor
        .as_ref()
        .unwrap();
    assert_eq!(c.active, vec![vec![1]]);
    assert_eq!(
        c.lists[&2],
        vec![Found::Inventory {
            owner: HOLDER,
            item: KEY
        }]
    );
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn find_can_fill_its_object_list_away_without_an_attached_body() {
    let mut e = engine();
    let p = package(true, false, false);
    let mut cursor = Cursor {
        tree_key: p.editor_id.clone(),
        ..Default::default()
    };
    cursor.step(p.tree.as_ref().unwrap(), 0., true, |_| true, || 0);
    let mut x = ActorExec::new(vec![p], Vec3::ZERO, 0., 0.);
    x.place = Some(Place::Interior(CELL));
    x.current = Some(0);
    x.alias_gen = e.scripts.alias_gen;
    x.begin_run(Some(0), 0., false);
    x.run.as_mut().unwrap().cursor = Some(cursor);
    e.away.insert(ACTOR, x);
    assert!(e.run_find(ACTOR));
    e.advance_away_tree(ACTOR, 0.);
    let c = e
        .execution(ACTOR)
        .unwrap()
        .run
        .as_ref()
        .unwrap()
        .cursor
        .as_ref()
        .unwrap();
    assert_eq!(c.active, vec![vec![1]]);
    assert!(c.lists[&2].contains(&Found::Inventory {
        owner: ACTOR,
        item: KEY
    }));
}

#[test]
#[ignore = "requires installed Skyrim SE masters and a headless GPU"]
fn vanilla_ralof_key_search_names_the_soldiers_alias_inventory_and_a_bound_object_list() {
    let data = vfs::locate_data_dir().unwrap();
    let names: Vec<_> = esp::BASE_MASTERS.iter().map(|s| s.to_string()).collect();
    let e = crate::stats::action_tests::headless_engine_plugins(&data, &names);
    let aliases = e.alias_specs(FormId(0x3372B));
    let soldier = aliases.iter().find(|a| a.id == 173).unwrap();
    assert_eq!(soldier.name, "BarracksRoomSoldier01");
    assert!(soldier.inventory.contains(&(FormId(0xB5178), 1)));
    assert!(matches!(
        soldier.fill,
        crate::aliases::Fill::Create { inside: false, .. }
    ));
    let quest = e.lo.get(FormId(0x3372B)).unwrap();
    let vmad = crate::script::vmad::parse(&quest).unwrap();
    for alias in [173, 174] {
        let scripts = &vmad
            .alias_scripts
            .iter()
            .find(|(id, _)| *id == alias)
            .unwrap()
            .1;
        let enemy = scripts
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case("MQ101EnemyScript"))
            .unwrap();
        assert!(
            enemy
                .properties
                .iter()
                .any(|(name, v)| name.eq_ignore_ascii_case("onDeathStage")
                    && matches!(v, crate::script::vmad::PropValue::Int(280)))
        );
    }
    let p = crate::ai::package::parse(&e.lo, FormId(0x8C816)).unwrap();
    let find = &p.tree.as_ref().unwrap().children[1].children[0];
    assert!(matches!(find.values[2], Input::ObjectList(2)));
    assert!(matches!(
        find.values[1],
        Input::Target(Target::Object(FormId(0xB5178)))
    ));
    let mut consumers = 0;
    for &id in e.lo.ids_of_type(b"SCEN") {
        let rec = e.lo.get(id).unwrap();
        let Some(scene) = esp::scene::parse(&rec, id) else {
            continue;
        };
        for action in &scene.actions {
            if !matches!(&action.kind, esp::scene::ActionKind::Package { packages } if packages.contains(&p.id))
            {
                continue;
            }
            consumers += 1;
            assert_eq!(id, FormId(0x4E1E2));
            assert_eq!((action.start_phase, action.end_phase), (30, 30));
            assert!(
                scene.phases[28]
                    .completion
                    .iter()
                    .chain(&scene.phases[28].start)
                    .any(|raw| {
                        crate::condition::parse(&rec, &raw.ctda)
                            .is_some_and(|c| c.func == 59 && c.p2 == 280 && c.value == 1.)
                    })
            );
        }
    }
    assert!(consumers > 0);
}

fn acquire_package(quantity: i32, stealing: bool, pockets: bool, own: bool) -> Package {
    let mut p = package(own, false, false);
    Arc::make_mut(p.tree.as_mut().unwrap()).children[1] = leaf(
        "Acquire",
        vec![
            Input::ObjectList(2),
            Input::Int(quantity),
            Input::Bool(stealing),
            Input::Bool(pockets),
            Input::Bool(false),
        ],
    );
    p
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn acquire_approaches_a_corpse_takes_only_its_quota_and_finishes_the_package() {
    let mut e = engine();
    e.world_state
        .dead
        .insert(HOLDER, (Place::Interior(CELL), Vec3::X * 100.));
    loaded(&mut e, acquire_package(1, false, false, false));
    e.run_loaded_finds();
    assert_eq!(e.item_count(ACTOR, KEY), 3);
    assert_eq!(
        e.actor_ref(ACTOR).unwrap().exec.goal.unwrap().centre,
        Vec3::X * 100.
    );
    e.actor_mut(ACTOR).unwrap().exec.pos.x = 10.;
    e.run_loaded_finds();
    assert_eq!(e.item_count(ACTOR, KEY), 4);
    assert_eq!(e.item_count(HOLDER, KEY), 1);
    assert_eq!(e.execution(ACTOR).unwrap().done_once, Some(PACKAGE));
    assert!(e.crime.bounties.is_empty());
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn acquire_skips_living_pockets_and_owned_loose_items_even_when_permitted() {
    // Stealing and pickpocketing are blocked (plans/ai-depth.md): the
    // permission flags must not make an owned item or a living pocket eligible.
    for allow in [false, true] {
        let mut e = engine();
        e.scripts.owners.insert(MARKER, Some(FormId(0x7)));
        loaded(&mut e, acquire_package(1, allow, allow, false));
        e.run_loaded_finds();
        assert_eq!(
            e.item_count(HOLDER, KEY),
            2,
            "living inventory is never taken"
        );
        e.actor_mut(ACTOR).unwrap().exec.pos.x = 150.;
        e.run_loaded_finds();
        assert!(!e.is_disabled(MARKER), "owned loose items are never taken");
        assert_eq!(e.item_count(ACTOR, KEY), 3);
        assert!(e.crime.bounties.is_empty());
    }
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn acquire_counts_owned_inventory_without_moving_it_or_counting_it_twice_on_retry() {
    let mut e = engine();
    e.scripts.disabled.insert(MARKER, true);
    e.inventory_mut(HOLDER).remove_split(KEY, 2, None);
    loaded(&mut e, acquire_package(4, false, false, true));
    e.run_loaded_finds();
    assert_eq!(e.item_count(ACTOR, KEY), 3);
    for _ in 0..3 {
        e.advance_loaded_trees(3.);
        e.run_loaded_finds();
    }
    let c = e
        .execution(ACTOR)
        .unwrap()
        .run
        .as_ref()
        .unwrap()
        .cursor
        .as_ref()
        .unwrap();
    assert_eq!(c.states[&vec![1]].count, 3);
    assert!(!c.completed);
    assert!(
        c.valid_for(
            e.execution(ACTOR).unwrap().packages[0]
                .tree
                .as_ref()
                .unwrap(),
            &c.tree_key
        )
    );
    e.scripts.disabled.insert(MARKER, false);
    e.actor_mut(ACTOR).unwrap().exec.pos.x = 150.;
    e.advance_loaded_trees(3.);
    e.run_loaded_finds();
    assert_eq!(e.item_count(ACTOR, KEY), 4);
    assert_eq!(e.execution(ACTOR).unwrap().done_once, Some(PACKAGE));
}

fn flee_package(from: bool, dead: bool, quiet: bool, counts: bool) -> Package {
    let mut p = Package::bare(PACKAGE, Behaviour::Hold);
    p.schedule.hour = -1;
    p.schedule.day_of_week = -1;
    p.tree = Some(Arc::new(leaf(
        "Flee",
        vec![
            Input::Target(Target::Ref(HOLDER)),
            Input::Location(Location {
                kind: LocationKind::NearReference(MARKER),
                radius: 20.,
            }),
            Input::Bool(from),
            Input::Float(20.),
            Input::Float(20.),
            Input::Float(400.),
            Input::Bool(false),
            Input::Bool(dead),
            Input::Bool(quiet),
            Input::Bool(counts),
        ],
    )));
    p
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn flee_to_location_counts_as_fleeing_only_when_requested_and_finishes_at_destination() {
    for counts in [false, true] {
        let mut e = engine();
        loaded(&mut e, flee_package(false, false, true, counts));
        assert_eq!(e.is_fleeing(ACTOR), counts);
        assert_eq!(
            e.actor_ref(ACTOR).unwrap().exec.goal.unwrap().centre,
            Vec3::X * 200.
        );
        assert!(!e.run_package_flee(ACTOR));
        e.actor_mut(ACTOR).unwrap().exec.pos.x = 190.;
        e.run_loaded_finds();
        assert_eq!(e.execution(ACTOR).unwrap().done_once, Some(PACKAGE));
        assert!(!e.is_fleeing(ACTOR));
    }
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn flee_from_distance_ignores_dead_threats_only_with_the_flag() {
    for forget in [false, true] {
        let mut e = engine();
        e.world_state
            .dead
            .insert(HOLDER, (Place::Interior(CELL), Vec3::X * 100.));
        loaded(&mut e, flee_package(true, forget, true, true));
        assert_eq!(e.run_package_flee(ACTOR), forget);
        if !forget {
            e.actor_mut(ACTOR).unwrap().exec.pos.x = -310.;
            assert!(e.run_package_flee(ACTOR));
        }
    }
}

#[test]
#[ignore = "requires installed Skyrim SE masters and a headless GPU"]
fn vanilla_acquire_flee_and_weapon_preparation_trees_are_executable() {
    let data = vfs::locate_data_dir().unwrap();
    let names: Vec<_> = esp::BASE_MASTERS.iter().map(|s| s.to_string()).collect();
    let e = crate::stats::action_tests::headless_engine_plugins(&data, &names);
    for id in [
        0x18969, 0x35B6B, 0x8C816, 0xFEDCD, 0x197F1, 0xC7039, 0x46BA9, 0x1C338,
    ] {
        let p = crate::ai::package::parse(&e.lo, FormId(id)).unwrap();
        assert!(p.tree.as_ref().unwrap().executable(), "{}", p.editor_id);
    }
    assert_eq!(
        super::super::combat::gmst_f32(&e.lo, "fFleeDistanceInterior", 0.),
        3000.
    );
    assert_eq!(
        super::super::combat::gmst_f32(&e.lo, "fFleeDistanceExterior", 0.),
        5000.
    );
}
