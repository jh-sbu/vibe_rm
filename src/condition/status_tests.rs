//! Status conditions (platform, race, items, ownership...) on generated records
//! only. Engine owns a renderer, so these need a headless GPU adapter.

use super::*;
use esp::writer::{Node, RecordBuilder, write_plugin};

pub(super) const SWORD_REF: FormId = FormId(0x814);

struct Fixture(std::path::PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture() -> (Fixture, Engine) {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let f = Fixture(
        std::env::temp_dir().join(format!("vrm-status-cond-{}-{stamp}", std::process::id())),
    );
    std::fs::create_dir(&f.0).unwrap();
    let records = [Node::top(
        b"REFR",
        vec![RecordBuilder::new(b"REFR", SWORD_REF)],
    )];
    std::fs::write(
        f.0.join("Status.esp"),
        write_plugin(
            0,
            &[],
            "test",
            "synthetic status conditions",
            0x900,
            &records,
        ),
    )
    .unwrap();
    let e = crate::stats::action_tests::headless_engine(&f.0, "Status.esp");
    (f, e)
}

/// `func(param) == value` as a CTDA record's bytes.
fn ctda(func: u16, param: u32, value: f32) -> [u8; 32] {
    let mut d = [0; 32];
    d[4..8].copy_from_slice(&value.to_le_bytes());
    d[8..10].copy_from_slice(&func.to_le_bytes());
    d[12..16].copy_from_slice(&param.to_le_bytes());
    d
}

/// Whether `func(param) == value` holds for `subject`.
fn is(e: &Engine, subject: FormId, func: u16, param: u32, value: f32) -> bool {
    let rec = e.lo.get(SWORD_REF).unwrap();
    let c = crate::condition::parse(&rec, &ctda(func, param, value)).unwrap();
    crate::condition::evaluate(
        e,
        &[c],
        crate::condition::Context {
            subject: Some(subject),
            ..Default::default()
        },
    )
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn the_platform_is_a_pc_with_gore() {
    let (_f, e) = fixture();
    assert!(is(&e, PLAYER_REF, 521, 0, 1.0)); // IsWin32
    assert!(is(&e, PLAYER_REF, 309, 0, 0.0)); // IsXBox
    assert!(is(&e, PLAYER_REF, 520, 0, 0.0)); // IsPS3
    assert!(is(&e, PLAYER_REF, 547, 0, 0.0)); // IsGoreDisabled
}
