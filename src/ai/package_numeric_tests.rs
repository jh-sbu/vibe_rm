//! Numeric package conditions over synthetic records; no game assets.

use super::*;
use esp::writer::{Node, RecordBuilder, write_plugin};

struct Fixture(std::path::PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "vrm-numeric-package-{}-{}",
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
    // Tree UNAMs name inputs rather than introducing more data values.
    record = record.field(b"XNAM", &[]).field(b"UNAM", &[99]);
    let lo = fixture.load(vec![record]);
    for (index, expected) in [(201, 1.0), (7, 0.0), (31, -42.0), (17, 12.5)] {
        assert_eq!(numeric_data(&lo, id(0x800), index), Some(expected));
    }
    for index in [0, 9, 10, 11, 12, 99, 256, 263, u32::MAX] {
        assert_eq!(numeric_data(&lo, id(0x800), index), None, "index {index}");
    }
    assert_eq!(numeric_data(&lo, id(0x999), 201), None);
}

fn branched_packages() -> Vec<RecordBuilder> {
    let template = RecordBuilder::new(b"PACK", FormId(0x800))
        .field(b"XNAM", &[])
        .zstring(b"ANAM", "Stacked")
        .u32(b"PRCB", 2)
        .zstring(b"ANAM", "Procedure")
        .field(b"CTDA", &numeric_condition(201, 1.0, 0))
        .zstring(b"PNAM", "Travel")
        .zstring(b"ANAM", "Procedure")
        .field(b"CTDA", &numeric_condition(201, 0.0, 0x08))
        .zstring(b"PNAM", "Sandbox");
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
    std::iter::once(template).chain(packages).collect()
}

#[test]
fn numeric_package_data_in_template_branches_keeps_the_concrete_input_index() {
    let fixture = Fixture::new();
    let lo = fixture.load(branched_packages());
    for (package, expected) in [(id(0x801), 1.0), (id(0x802), 0.0)] {
        let branches = expand(&lo, package);
        assert_eq!(branches.len(), 2);
        assert_eq!(branches[0].behaviour, Behaviour::Travel);
        assert_eq!(branches[1].behaviour, Behaviour::Sandbox);
        for branch in branches {
            assert_eq!(branch.id, package);
            let c = &branch.conditions[0];
            assert_eq!(c.p1, 201, "input indices must not be FormIDs");
            assert_eq!(c.run_on, 0, "numeric inputs must not resolve as references");
            assert_eq!(numeric_data(&lo, branch.id, c.p1), Some(expected));
        }
    }
}

#[test]
#[ignore = "needs headless GPU with BC textures and >=17 sampled textures per shader stage; synthetic records only"]
fn numeric_package_conditions_select_branches_and_fail_without_valid_data() {
    let fixture = Fixture::new();
    let mut packages = branched_packages();
    // Missing inputs must not make == 0 or != 1 pass by default.
    packages.push(
        RecordBuilder::new(b"PACK", FormId(0x803)).field(b"CTDA", &numeric_condition(201, 0.0, 0)),
    );
    packages.push(
        RecordBuilder::new(b"PACK", FormId(0x804))
            .field(b"CTDA", &numeric_condition(201, 1.0, 0x20)),
    );
    let lo = fixture.load(packages);
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        compatible_surface: None,
        apply_limit_buckets: false,
        ..Default::default()
    }))
    .expect("headless GPU must support BC textures and >=17 sampled textures per shader stage");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::TEXTURE_COMPRESSION_BC,
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .unwrap();
    let renderer =
        crate::render::Renderer::new(device, queue, wgpu::TextureFormat::Rgba8Unorm, 16, 16);
    let engine =
        crate::engine::Engine::new(lo, vfs::Vfs::new(&fixture.0, &[]), renderer, 12.0, None, 0);
    for (package, expected_branch) in [(id(0x801), 0), (id(0x802), 1)] {
        let branches = expand(&engine.lo, package);
        let ctx = condition::Context {
            package: Some(package),
            ..Default::default()
        };
        assert_eq!(
            branches
                .iter()
                .position(|p| condition::evaluate(&engine, &p.conditions, ctx)),
            Some(expected_branch)
        );
        assert!(!condition::evaluate(
            &engine,
            &branches[expected_branch].conditions,
            condition::Context::default()
        ));
    }
    for package in [id(0x803), id(0x804)] {
        let p = parse(&engine.lo, package).unwrap();
        assert!(!condition::evaluate(
            &engine,
            &p.conditions,
            condition::Context {
                package: Some(package),
                ..Default::default()
            }
        ));
    }
}
