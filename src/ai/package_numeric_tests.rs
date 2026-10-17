//! Numeric package conditions over synthetic records; no game assets.

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
            "vrm-numeric-package-{}-{stamp}-{}",
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

fn numeric_condition(index: u32, value: f32, flags: u8) -> [u8; 32] {
    let mut data = [0; 32];
    data[0] = flags;
    data[4..8].copy_from_slice(&value.to_le_bytes());
    data[8..10].copy_from_slice(&612u16.to_le_bytes());
    data[12..16].copy_from_slice(&index.to_le_bytes());
    data
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
        assert_eq!(c.package_data, expected, "index {index}");
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
        assert_eq!(c.package_data, Some(value));
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
