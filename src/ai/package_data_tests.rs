//! Package data conditions (GetNumericPackageData, GetWithinPackageLocation)
//! over synthetic records; no game assets.

use super::*;
use esp::writer::{Node, RecordBuilder, write_plugin};

struct Fixture(std::path::PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "vrm-package-data-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&dir).unwrap();
        Self(dir)
    }

    fn load(&self, packages: Vec<RecordBuilder>) -> LoadOrder {
        for (name, groups) in [
            ("Empty.esp", vec![]),
            ("Numeric.esp", vec![Node::top(b"PACK", packages)]),
        ] {
            std::fs::write(
                self.0.join(name),
                write_plugin(0, &[], "test", "synthetic package data", 0xA00, &groups),
            )
            .unwrap();
        }
        LoadOrder::load(&self.0, &["Empty.esp".into(), "Numeric.esp".into()]).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn id(local: u32) -> FormId {
    FormId(0x0100_0000 | local)
}

fn input(record: RecordBuilder, index: u8, kind: &str, bytes: &[u8]) -> RecordBuilder {
    record
        .zstring(b"ANAM", kind)
        .field(b"CNAM", bytes)
        .field(b"UNAM", &[index])
}

fn location_input(
    record: RecordBuilder,
    index: u8,
    kind: u32,
    value: u32,
    radius: i32,
) -> RecordBuilder {
    let mut pldt = kind.to_le_bytes().to_vec();
    pldt.extend_from_slice(&value.to_le_bytes());
    pldt.extend_from_slice(&radius.to_le_bytes());
    record
        .zstring(b"ANAM", "Location")
        .field(b"PLDT", &pldt)
        .field(b"UNAM", &[index])
}

fn package_condition(func: u16, index: u32, value: f32, flags: u8) -> [u8; 32] {
    let mut data = [0; 32];
    data[0] = flags;
    data[4..8].copy_from_slice(&value.to_le_bytes());
    data[8..10].copy_from_slice(&func.to_le_bytes());
    data[12..16].copy_from_slice(&index.to_le_bytes());
    data
}

fn numeric_condition(index: u32, value: f32, flags: u8) -> [u8; 32] {
    package_condition(condition::GET_NUMERIC_PACKAGE_DATA, index, value, flags)
}

fn location_condition(index: u32, value: f32) -> [u8; 32] {
    package_condition(condition::GET_WITHIN_PACKAGE_LOCATION, index, value, 0)
}

fn number(c: &Condition) -> Option<f32> {
    match c.package_data {
        Some(condition::PackageData::Number(v)) => Some(v),
        _ => None,
    }
}

fn location(c: &Condition) -> Option<Location> {
    match c.package_data {
        Some(condition::PackageData::Location(l)) => Some(l),
        _ => None,
    }
}

#[test]
fn numeric_package_data_uses_sparse_indices_and_preserves_numeric_types() {
    let fixture = Fixture::new();
    let mut record = RecordBuilder::new(b"PACK", FormId(0x800));
    let cases: [(u32, Option<f32>); 13] = [
        (201, Some(1.0)),
        (7, Some(0.0)),
        (31, Some(-42.0)),
        (17, Some(12.5)),
        (0, None),
        (9, None),
        (10, None),
        (11, None),
        (12, None),
        (99, None),
        (256, None),
        (263, None),
        (u32::MAX, None),
    ];
    for (index, _) in cases {
        record = record.field(b"CTDA", &numeric_condition(index, 0.0, 0));
    }
    for (index, kind, bytes) in [
        (201, "Bool", vec![1]),
        (7, "Bool", vec![0]),
        (31, "Int", (-42i32).to_le_bytes().to_vec()),
        (17, "Float", 12.5f32.to_le_bytes().to_vec()),
        (9, "Float", vec![1, 2, 3]),
        (10, "Float", f32::NAN.to_le_bytes().to_vec()),
        (11, "Bool", vec![]),
        (12, "SingleRef", 1u32.to_le_bytes().to_vec()),
    ] {
        record = input(record, index, kind, &bytes);
    }
    // Tree UNAMs name inputs rather than introducing more data values, and
    // tree conditions are not the package's own.
    record = record
        .field(b"XNAM", &[])
        .field(b"CTDA", &numeric_condition(201, 1.0, 0))
        .field(b"UNAM", &[99]);
    let lo = fixture.load(vec![record]);
    let p = parse(&lo, id(0x800)).unwrap();
    assert_eq!(p.conditions.len(), cases.len());
    for ((index, expected), c) in cases.into_iter().zip(&p.conditions) {
        assert_eq!(c.p1, index);
        assert_eq!(number(c), expected, "index {index}");
    }
}

fn template_tree(record: RecordBuilder) -> RecordBuilder {
    record
        .field(b"XNAM", &[])
        .zstring(b"ANAM", "Stacked")
        .u32(b"PRCB", 2)
        .zstring(b"ANAM", "Procedure")
        .field(b"CTDA", &numeric_condition(201, 1.0, 0))
        .zstring(b"PNAM", "Travel")
        .zstring(b"ANAM", "Procedure")
        .field(b"CTDA", &numeric_condition(201, 0.0, 0x08))
        .zstring(b"PNAM", "Sandbox")
}

/// Each branch has only its own condition, with the concrete package's value.
fn assert_branches(lo: &LoadOrder, package: FormId, value: f32) {
    let branches = expand(lo, package);
    assert_eq!(branches.len(), 2);
    assert_eq!(branches[0].behaviour, Behaviour::Travel);
    assert_eq!(branches[1].behaviour, Behaviour::Sandbox);
    for branch in branches {
        assert_eq!(branch.id, package);
        let [c] = branch.conditions.as_slice() else {
            panic!("{}: {:?}", branch.editor_id, branch.conditions);
        };
        assert_eq!(c.p1, 201, "input indices must not be FormIDs");
        assert_eq!(c.run_on, 0, "numeric inputs must not resolve as references");
        assert_eq!(number(c), Some(value));
    }
}

#[test]
fn numeric_package_data_in_template_branches_keeps_the_concrete_input_index() {
    let fixture = Fixture::new();
    let template = template_tree(RecordBuilder::new(b"PACK", FormId(0x800)));
    let mut pkcu = 1u32.to_le_bytes().to_vec();
    pkcu.extend_from_slice(&0x800u32.to_le_bytes());
    let packages = [true, false].into_iter().enumerate().map(|(i, value)| {
        input(
            RecordBuilder::new(b"PACK", FormId(0x801 + i as u32)).field(b"PKCU", &pkcu),
            201,
            "Bool",
            &[u8::from(value)],
        )
    });
    let lo = fixture.load(std::iter::once(template).chain(packages).collect());
    assert_branches(&lo, id(0x801), 1.0);
    assert_branches(&lo, id(0x802), 0.0);
}

#[test]
fn branches_of_a_package_with_its_own_tree_have_only_their_own_conditions() {
    let fixture = Fixture::new();
    let package = template_tree(input(
        RecordBuilder::new(b"PACK", FormId(0x800)),
        201,
        "Bool",
        &[0],
    ));
    let lo = fixture.load(vec![package]);
    assert!(parse(&lo, id(0x800)).unwrap().conditions.is_empty());
    assert_branches(&lo, id(0x800), 0.0);
}

#[test]
fn within_package_location_reads_location_inputs_only() {
    let fixture = Fixture::new();
    let mut record = RecordBuilder::new(b"PACK", FormId(0x800));
    for (index, value) in [(62, 1.0), (75, 0.0), (5, 1.0), (99, 1.0)] {
        record = record.field(b"CTDA", &location_condition(index, value));
    }
    // GetNumericPackageData doesn't read a location.
    record = record.field(b"CTDA", &numeric_condition(62, 0.0, 0));
    record = location_input(record, 62, 0, 0x14, 500);
    record = location_input(record, 75, 12, 0, 300);
    record = input(record, 5, "Bool", &[1]);
    let lo = fixture.load(vec![record]);
    let p = parse(&lo, id(0x800)).unwrap();
    let [near_player, near_self, bool_input, missing, numeric] = p.conditions.as_slice() else {
        panic!("{:?}", p.conditions);
    };
    let l = location(near_player).unwrap();
    assert!(matches!(l.kind, LocationKind::NearReference(r) if r == id(0x14)));
    assert_eq!(l.radius, 500.0);
    let l = location(near_self).unwrap();
    assert!(matches!(l.kind, LocationKind::NearSelf));
    assert_eq!(l.radius, 300.0);
    for c in [bool_input, missing, numeric] {
        assert!(c.package_data.is_none(), "{c:?}");
    }
}

#[test]
fn within_package_location_in_template_branches_reads_the_concrete_package() {
    let fixture = Fixture::new();
    // TravelToRadius-like: travel until within the location, then hold position.
    let template = RecordBuilder::new(b"PACK", FormId(0x800))
        .field(b"XNAM", &[])
        .zstring(b"ANAM", "Stacked")
        .u32(b"PRCB", 2)
        .zstring(b"ANAM", "Procedure")
        .field(b"CTDA", &location_condition(0, 0.0))
        .zstring(b"PNAM", "Travel")
        .zstring(b"ANAM", "Procedure")
        .zstring(b"PNAM", "HoldPosition");
    let mut pkcu = 1u32.to_le_bytes().to_vec();
    pkcu.extend_from_slice(&0x800u32.to_le_bytes());
    let packages = [(0x801, 1000), (0x802, 250)].map(|(form, radius)| {
        location_input(
            RecordBuilder::new(b"PACK", FormId(form)).field(b"PKCU", &pkcu),
            0,
            1,
            0x3C,
            radius,
        )
    });
    let lo = fixture.load(std::iter::once(template).chain(packages).collect());
    for (package, radius) in [(id(0x801), 1000.0), (id(0x802), 250.0)] {
        let branches = expand(&lo, package);
        assert_eq!(branches.len(), 2);
        let [c] = branches[0].conditions.as_slice() else {
            panic!("{:?}", branches[0].conditions);
        };
        assert_eq!(c.p1, 0);
        assert_eq!(c.run_on, 0);
        let l = location(c).unwrap();
        assert!(matches!(l.kind, LocationKind::InCell(_)));
        assert_eq!(l.radius, radius);
        assert!(branches[1].conditions.is_empty());
    }
}

fn prcb(children: u32, flags: u32) -> [u8; 8] {
    let mut d = [0; 8];
    d[..4].copy_from_slice(&children.to_le_bytes());
    d[4..].copy_from_slice(&flags.to_le_bytes());
    d
}

/// One stop of a SandboxMultiLocation-like rotation: a branch that runs when
/// its time is above zero, sandboxing at its location while waiting its time.
fn stop_branch(record: RecordBuilder, location: u8, time: u8) -> RecordBuilder {
    record
        .zstring(b"ANAM", "Sequence")
        .field(b"CTDA", &numeric_condition(time as u32, 0.0, 0x40))
        .field(b"PRCB", &prcb(1, 0))
        .zstring(b"ANAM", "Simultaneous")
        .field(b"PRCB", &prcb(2, 0))
        .zstring(b"ANAM", "Procedure")
        .zstring(b"PNAM", "Sandbox")
        .field(b"PKC2", &[location])
        .zstring(b"ANAM", "Procedure")
        .zstring(b"PNAM", "Wait")
        .field(b"PKC2", &[time])
}

#[test]
fn multi_location_sandboxes_rotate_through_the_stops_their_data_enables() {
    let fixture = Fixture::new();
    // Root: travel, then a repeating Random (Random? true) or Sequence (false)
    // over three stops. Op 2 (`>`) is flags 0x40.
    let mut template = RecordBuilder::new(b"PACK", FormId(0x800))
        .zstring(b"EDID", "SandboxMultiLocation")
        .field(b"XNAM", &[])
        .zstring(b"ANAM", "Sequence")
        .field(b"PRCB", &prcb(3, 0))
        .zstring(b"ANAM", "Procedure")
        .zstring(b"PNAM", "Travel")
        .field(b"PKC2", &[0]);
    for (kind, random) in [("Random", 1.0), ("Sequence", 0.0)] {
        template = template
            .zstring(b"ANAM", kind)
            .field(b"CTDA", &numeric_condition(92, random, 0))
            .field(b"PRCB", &prcb(3, 1));
        for (location, time) in [(0, 32), (35, 41), (37, 43)] {
            template = stop_branch(template, location, time);
        }
    }
    let mut pkcu = 1u32.to_le_bytes().to_vec();
    pkcu.extend_from_slice(&0x800u32.to_le_bytes());
    let packages = [(0x801, false), (0x802, true)].map(|(form, random)| {
        let mut p = RecordBuilder::new(b"PACK", FormId(form)).field(b"PKCU", &pkcu);
        p = input(p, 92, "Bool", &[u8::from(random)]);
        for (location, time, reference, seconds) in [
            (0, 32, 0x10, 120.0f32),
            (35, 41, 0x11, 90.0),
            (37, 43, 0x12, 0.0),
        ] {
            p = location_input(p, location, 0, reference, 256);
            p = input(p, time, "Float", &seconds.to_le_bytes());
        }
        p
    });
    let lo = fixture.load(std::iter::once(template).chain(packages).collect());
    for (package, random) in [(id(0x801), false), (id(0x802), true)] {
        let p = parse(&lo, package).unwrap();
        assert_eq!(p.behaviour, Behaviour::Sandbox);
        assert_eq!(p.random_stops, random);
        // The third stop's time is zero: its branch doesn't run.
        let [a, b] = p.stops.as_slice() else {
            panic!("{:?}", p.stops);
        };
        assert!(matches!(a.location.kind, LocationKind::NearReference(r) if r == id(0x10)));
        assert_eq!(a.seconds, 120.0);
        assert!(matches!(b.location.kind, LocationKind::NearReference(r) if r == id(0x11)));
        assert_eq!(b.seconds, 90.0);
    }
}

#[test]
fn package_data_conditions_decide_alone_only_when_every_one_is_numeric_package_data() {
    let fixture = Fixture::new();
    let record = input(
        RecordBuilder::new(b"PACK", FormId(0x800))
            .field(b"CTDA", &numeric_condition(5, 1.0, 0))
            .field(b"CTDA", &numeric_condition(6, 0.0, 0x40)),
        5,
        "Bool",
        &[1],
    );
    let record = input(record, 6, "Float", &0.0f32.to_le_bytes());
    let lo = fixture.load(vec![record]);
    let p = parse(&lo, id(0x800)).unwrap();
    assert_eq!(
        condition::evaluate_package_data(&p.conditions[..1]),
        Some(true)
    );
    assert_eq!(condition::evaluate_package_data(&p.conditions), Some(false));
    let mut world = p.conditions[0].clone();
    world.func = 0;
    assert_eq!(condition::evaluate_package_data(&[world]), None);
}

#[test]
fn null_package_data_is_an_input_with_nothing_set() {
    let fixture = Fixture::new();
    let mut record = RecordBuilder::new(b"PACK", FormId(0x800));
    // A bool, a reference left empty, and an input the package lacks.
    for index in [1, 2, 5] {
        record = record.field(
            b"CTDA",
            &package_condition(condition::IS_NULL_PACKAGE_DATA, index, 0.0, 0),
        );
    }
    record = input(record, 1, "Bool", &[0]);
    record = input(record, 2, "SingleRef", &[]);
    let lo = fixture.load(vec![record]);
    let p = parse(&lo, id(0x800)).unwrap();
    let nulls: Vec<_> = p.conditions.iter().map(number).collect();
    assert_eq!(nulls, [Some(0.0), Some(1.0), Some(1.0)]);
}
