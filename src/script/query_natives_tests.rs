//! Synthetic plugin fixtures only; these tests require a headless GPU adapter.
use crate::engine::Engine;
use esp::FormId;
use esp::writer::{Node, RecordBuilder, write_plugin};
use papyrus::{NativeResult, Value};

struct Fixture(std::path::PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn fixture(records: Vec<Node>) -> (Fixture, Engine) {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let f = Fixture(
        std::env::temp_dir().join(format!("vrm-query-natives-{}-{stamp}", std::process::id())),
    );
    std::fs::create_dir(&f.0).unwrap();
    std::fs::write(
        f.0.join("Queries.esp"),
        write_plugin(0, &[], "test", "synthetic queries", 0x900, &records),
    )
    .unwrap();
    let e = crate::stats::action_tests::headless_engine(&f.0, "Queries.esp");
    (f, e)
}
fn call(e: &mut Engine, class: &str, func: &str, on: FormId, args: &[Value]) -> Value {
    let this = e.object_value(on);
    match super::natives::call(e, class, func, Some(&this), args) {
        NativeResult::Value(v) => v,
        _ => panic!("{class}.{func} did not return a value"),
    }
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn is_unique_reads_the_base_flag() {
    let (_f, mut e) = fixture(vec![Node::top(
        b"NPC_",
        vec![
            RecordBuilder::new(b"NPC_", FormId(0x800)).u32(b"ACBS", 0x20),
            RecordBuilder::new(b"NPC_", FormId(0x801)).u32(b"ACBS", 0x2),
            RecordBuilder::new(b"NPC_", FormId(0x802)).field(b"ACBS", &[0x20]),
        ],
    )]);
    assert_eq!(
        call(&mut e, "actorbase", "isunique", FormId(0x800), &[]),
        Value::Bool(true)
    );
    for id in [0x801, 0x802, 0] {
        assert_eq!(
            call(&mut e, "actorbase", "isunique", FormId(id), &[]),
            Value::Bool(false)
        );
    }
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn get_class_follows_only_the_stats_template() {
    let mut stats = [0u8; 24];
    stats[18..20].copy_from_slice(&2u16.to_le_bytes());
    let (_f, mut e) = fixture(vec![
        Node::top(
            b"CLAS",
            vec![
                RecordBuilder::new(b"CLAS", FormId(0x810)),
                RecordBuilder::new(b"CLAS", FormId(0x811)),
            ],
        ),
        Node::top(
            b"NPC_",
            vec![
                RecordBuilder::new(b"NPC_", FormId(0x800)).form(b"CNAM", FormId(0x810)),
                RecordBuilder::new(b"NPC_", FormId(0x801))
                    .field(b"ACBS", &stats)
                    .form(b"TPLT", FormId(0x800))
                    .form(b"CNAM", FormId(0x811)),
                RecordBuilder::new(b"NPC_", FormId(0x802))
                    .form(b"TPLT", FormId(0x800))
                    .form(b"CNAM", FormId(0x811)),
                RecordBuilder::new(b"NPC_", FormId(0x803)).field(b"CNAM", &[0]),
            ],
        ),
    ]);
    let a = e.object_value(FormId(0x810));
    let b = e.object_value(FormId(0x811));
    assert_eq!(call(&mut e, "actorbase", "getclass", FormId(0x800), &[]), a);
    assert_eq!(call(&mut e, "actorbase", "getclass", FormId(0x801), &[]), a);
    assert_eq!(call(&mut e, "actorbase", "getclass", FormId(0x802), &[]), b);
    assert_eq!(
        call(&mut e, "actorbase", "getclass", FormId(0x803), &[]),
        Value::None
    );
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn is_interior_queries_the_cell_not_the_player() {
    let (_f, mut e) = fixture(vec![Node::top(
        b"CELL",
        vec![
            RecordBuilder::new(b"CELL", FormId(0x800)).field(b"DATA", &[1]),
            RecordBuilder::new(b"CELL", FormId(0x801)).field(b"DATA", &[2, 0]),
            RecordBuilder::new(b"CELL", FormId(0x802)).field(b"DATA", &[]),
        ],
    )]);
    for location in [
        crate::engine::Location::Interior(FormId(0x801)),
        crate::engine::Location::Exterior {
            world: FormId(0x900),
            center: (0, 0),
        },
    ] {
        e.location = location;
        assert_eq!(
            call(&mut e, "cell", "isinterior", FormId(0x800), &[]),
            Value::Bool(true)
        );
        for id in [0x801, 0x802, 0] {
            assert_eq!(
                call(&mut e, "cell", "isinterior", FormId(id), &[]),
                Value::Bool(false)
            );
        }
    }
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn geteditorlocation_uses_reference_location() {
    let (_f, mut e) = fixture(vec![
        Node::top(
            b"LCTN",
            vec![
                RecordBuilder::new(b"LCTN", FormId(0x810)),
                RecordBuilder::new(b"LCTN", FormId(0x811)),
            ],
        ),
        Node::top(
            b"CELL",
            vec![
                RecordBuilder::new(b"CELL", FormId(0x820)).form(b"XLCN", FormId(0x811)),
                RecordBuilder::new(b"CELL", FormId(0x821)),
            ],
        ),
        Node::top(
            b"REFR",
            vec![
                RecordBuilder::new(b"REFR", FormId(0x800)).form(b"XLCN", FormId(0x810)),
                RecordBuilder::new(b"REFR", FormId(0x801)),
            ],
        ),
    ]);
    let original = e.object_value(FormId(0x810));
    assert_eq!(
        call(
            &mut e,
            "objectreference",
            "geteditorlocation",
            FormId(0x800),
            &[]
        ),
        original
    );
    assert_eq!(
        call(
            &mut e,
            "objectreference",
            "geteditorlocation",
            FormId(0x801),
            &[]
        ),
        Value::None
    );
    e.actor_cells.insert(
        FormId(0x800),
        crate::render::CellKey::Interior(FormId(0x820)),
    );
    let expected = e.object_value(FormId(0x810));
    assert_eq!(
        call(
            &mut e,
            "objectreference",
            "geteditorlocation",
            FormId(0x800),
            &[]
        ),
        expected
    );
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn getcurrentlocation_uses_reference_location() {
    let (_f, mut e) = fixture(vec![
        Node::top(
            b"LCTN",
            vec![
                RecordBuilder::new(b"LCTN", FormId(0x810)),
                RecordBuilder::new(b"LCTN", FormId(0x811)),
            ],
        ),
        Node::top(
            b"CELL",
            vec![
                RecordBuilder::new(b"CELL", FormId(0x820)).form(b"XLCN", FormId(0x811)),
                RecordBuilder::new(b"CELL", FormId(0x821)),
            ],
        ),
        Node::top(
            b"REFR",
            vec![
                RecordBuilder::new(b"REFR", FormId(0x800)).form(b"XLCN", FormId(0x810)),
                RecordBuilder::new(b"REFR", FormId(0x801)),
            ],
        ),
    ]);
    let original = e.object_value(FormId(0x810));
    assert_eq!(
        call(
            &mut e,
            "objectreference",
            "getcurrentlocation",
            FormId(0x800),
            &[]
        ),
        original
    );
    assert_eq!(
        call(
            &mut e,
            "objectreference",
            "getcurrentlocation",
            FormId(0x801),
            &[]
        ),
        Value::None
    );
    e.actor_cells.insert(
        FormId(0x800),
        crate::render::CellKey::Interior(FormId(0x820)),
    );
    let expected = e.object_value(FormId(0x811));
    assert_eq!(
        call(
            &mut e,
            "objectreference",
            "getcurrentlocation",
            FormId(0x800),
            &[]
        ),
        expected
    );

    e.actor_cells.insert(
        FormId(0x800),
        crate::render::CellKey::Interior(FormId(0x821)),
    );
    assert_eq!(
        call(
            &mut e,
            "objectreference",
            "getcurrentlocation",
            FormId(0x800),
            &[]
        ),
        Value::None
    );

    e.actor_cells.remove(&FormId(0x800));
    e.world_state.moved.insert(
        FormId(0x800),
        crate::world_state::Moved {
            place: crate::ai::schedule::Place::Interior(FormId(0x820)),
            pos: glam::Vec3::ZERO,
            rot: glam::Vec3::ZERO,
        },
    );
    let moved = e.object_value(FormId(0x811));
    assert_eq!(
        call(
            &mut e,
            "objectreference",
            "getcurrentlocation",
            FormId(0x800),
            &[]
        ),
        moved
    );
    let editor = e.object_value(FormId(0x810));
    assert_eq!(
        call(
            &mut e,
            "objectreference",
            "geteditorlocation",
            FormId(0x800),
            &[]
        ),
        editor
    );
}

fn population_fixture() -> (Fixture, Engine) {
    let entries: Vec<u8> = [
        0x830u32, 0x800, 0, 0, 0x830, 0x801, 0, 0, 0x830, 0x800, 0, 0,
    ]
    .into_iter()
    .flat_map(u32::to_le_bytes)
    .collect();
    fixture(vec![
        Node::top(b"LCRT", vec![RecordBuilder::new(b"LCRT", FormId(0x830))]),
        Node::top(
            b"LCTN",
            vec![
                RecordBuilder::new(b"LCTN", FormId(0x810)).field(b"ACSR", &entries),
                RecordBuilder::new(b"LCTN", FormId(0x811)),
            ],
        ),
        Node::top(
            b"ACHR",
            vec![
                RecordBuilder::new(b"ACHR", FormId(0x800))
                    .form(b"XLCN", FormId(0x810))
                    .form(b"XLRT", FormId(0x830)),
                RecordBuilder::new(b"ACHR", FormId(0x801)),
            ],
        ),
    ])
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn getreftypealivecount_counts_enabled_editor_references_once() {
    let (_f, mut e) = population_fixture();
    let ty = e.object_value(FormId(0x830));
    assert_eq!(
        call(
            &mut e,
            "location",
            "getreftypealivecount",
            FormId(0x810),
            &[ty.clone()]
        ),
        Value::Int(2)
    );
    e.world_state.dead.insert(
        FormId(0x800),
        (
            crate::ai::schedule::Place::Interior(FormId(0x820)),
            glam::Vec3::ZERO,
        ),
    );
    assert_eq!(
        call(
            &mut e,
            "location",
            "getreftypealivecount",
            FormId(0x810),
            &[ty.clone()]
        ),
        Value::Int(1)
    );
    e.scripts.disabled.insert(FormId(0x801), true);
    assert_eq!(
        call(
            &mut e,
            "location",
            "getreftypealivecount",
            FormId(0x810),
            &[ty.clone()]
        ),
        Value::Int(0)
    );
    assert_eq!(
        call(
            &mut e,
            "location",
            "getreftypealivecount",
            FormId(0x811),
            &[ty]
        ),
        Value::Int(0)
    );
    assert_eq!(
        call(
            &mut e,
            "location",
            "getreftypealivecount",
            FormId(0x810),
            &[Value::None]
        ),
        Value::Int(0)
    );
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn getreftypedeadcount_counts_enabled_editor_references_once() {
    let (_f, mut e) = population_fixture();
    let ty = e.object_value(FormId(0x830));
    assert_eq!(
        call(
            &mut e,
            "location",
            "getreftypedeadcount",
            FormId(0x810),
            &[ty.clone()]
        ),
        Value::Int(0)
    );
    e.world_state.dead.insert(
        FormId(0x800),
        (
            crate::ai::schedule::Place::Interior(FormId(0x820)),
            glam::Vec3::ZERO,
        ),
    );
    assert_eq!(
        call(
            &mut e,
            "location",
            "getreftypedeadcount",
            FormId(0x810),
            &[ty.clone()]
        ),
        Value::Int(1)
    );
    e.scripts.disabled.insert(FormId(0x800), true);
    assert_eq!(
        call(
            &mut e,
            "location",
            "getreftypedeadcount",
            FormId(0x810),
            &[ty.clone()]
        ),
        Value::Int(0)
    );
    assert_eq!(
        call(
            &mut e,
            "location",
            "getreftypedeadcount",
            FormId(0x811),
            &[ty]
        ),
        Value::Int(0)
    );
    assert_eq!(
        call(
            &mut e,
            "location",
            "getreftypedeadcount",
            FormId(0x810),
            &[Value::None]
        ),
        Value::Int(0)
    );
}
