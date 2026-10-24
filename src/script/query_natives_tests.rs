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
