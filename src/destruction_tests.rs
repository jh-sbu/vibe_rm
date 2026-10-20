//! Synthetic definitions and query-path tests; no game assets are used.
use super::*;
use esp::writer::{Node, RecordBuilder, write_plugin};
use papyrus::Value;

const BASE: FormId = FormId(0x800);
const OTHER_BASE: FormId = FormId(0x801);
const REF_A: FormId = FormId(0x810);
const REF_B: FormId = FormId(0x811);
const QUEST: FormId = FormId(0x820);

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
    let dir = Fixture(
        std::env::temp_dir().join(format!("vrm-destruction-{}-{stamp}", std::process::id())),
    );
    std::fs::create_dir(&dir.0).unwrap();

    let mut dest = Vec::new();
    dest.extend_from_slice(&100u32.to_le_bytes());
    dest.extend_from_slice(&[2, 1, 0, 0]); // two stages, VATS targetable
    let mut first = vec![99, 0, 1, 1]; // health%, index, damage stage, flags
    first.extend_from_slice(&0u32.to_le_bytes());
    first.extend_from_slice(&0x830u32.to_le_bytes()); // explosion
    first.extend_from_slice(&0x831u32.to_le_bytes()); // debris
    first.extend_from_slice(&2u32.to_le_bytes());
    let mut last = vec![0, 1, 2, 6];
    last.extend_from_slice(&4u32.to_le_bytes());
    last.extend_from_slice(&0u32.to_le_bytes());
    last.extend_from_slice(&0u32.to_le_bytes());
    last.extend_from_slice(&0u32.to_le_bytes());

    let reference = |id, base| RecordBuilder::new(b"REFR", id).form(b"NAME", base);
    let records = [
        Node::top(
            b"ACTI",
            vec![
                RecordBuilder::new(b"ACTI", BASE)
                    .field(b"DEST", &dest)
                    .field(b"DSTD", &first)
                    .field(b"DMDL", b"meshes\\broken.nif\0")
                    .field(b"DSTF", &[])
                    .field(b"DSTD", &last)
                    .field(b"DSTF", &[]),
                RecordBuilder::new(b"ACTI", OTHER_BASE),
            ],
        ),
        Node::top(
            b"REFR",
            vec![reference(REF_A, BASE), reference(REF_B, OTHER_BASE)],
        ),
        Node::top(b"EXPL", vec![RecordBuilder::new(b"EXPL", FormId(0x830))]),
        Node::top(b"DEBR", vec![RecordBuilder::new(b"DEBR", FormId(0x831))]),
        Node::top(
            b"QUST",
            vec![RecordBuilder::new(b"QUST", QUEST).field(b"CTDA", &condition(-1.0))],
        ),
    ];
    std::fs::write(
        dir.0.join("Destruction.esp"),
        write_plugin(
            0,
            &[],
            "test",
            "synthetic destructible references",
            0x900,
            &records,
        ),
    )
    .unwrap();
    let engine = crate::stats::action_tests::headless_engine(&dir.0, "Destruction.esp");
    (dir, engine)
}

fn condition(value: f32) -> [u8; 32] {
    let mut data = [0; 32];
    data[4..8].copy_from_slice(&value.to_le_bytes());
    data[8..10].copy_from_slice(&470u16.to_le_bytes()); // GetDestructionStage
    data
}

fn current_stage(e: &mut Engine, this: &Value) -> i32 {
    match crate::script::natives::call(
        e,
        "objectreference",
        "getcurrentdestructionstage",
        Some(this),
        &[],
    ) {
        papyrus::NativeResult::Value(Value::Int(stage)) => stage,
        _ => panic!("GetCurrentDestructionStage did not return an integer"),
    }
}

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn destructible_definitions_and_initial_stage_queries_are_reference_scoped() {
    let (_dir, mut e) = fixture();
    let definition = e.destructible_data(REF_A).unwrap();
    assert_eq!(definition.health, 100);
    assert!(definition.vats_targetable);
    assert_eq!(definition.stages.len(), 2);
    assert_eq!(definition.stages[0].explosion, FormId(0x830));
    assert_eq!(definition.stages[0].debris, FormId(0x831));
    assert!(
        e.lo.tag_of(definition.stages[0].explosion)
            .is_some_and(|tag| tag.0 == *b"EXPL")
    );
    assert!(
        e.lo.tag_of(definition.stages[0].debris)
            .is_some_and(|tag| tag.0 == *b"DEBR")
    );
    assert_eq!(
        definition.stages[0].replacement_model.as_deref(),
        Some("meshes\\broken.nif")
    );
    assert_eq!(definition.stages[1].index, 1);
    assert_eq!(e.destructible_data(REF_B), None);

    let this_a = e.object_value(REF_A);
    let this_b = e.object_value(REF_B);
    assert_eq!(current_stage(&mut e, &this_a), -1);
    assert_eq!(e.current_destruction_stage(REF_B), -1);

    // The later damage path mutates one placed reference independently.
    e.world_state.destruction_stages.insert(REF_A, 0);
    assert_eq!(e.current_destruction_stage(REF_A), 0);
    assert_eq!(e.current_destruction_stage(REF_B), -1);
    assert_eq!(current_stage(&mut e, &this_b), -1);

    let parsed = crate::condition::parse_all(&e.lo.get(QUEST).unwrap());
    assert_eq!(parsed.len(), 1);
    assert!(crate::condition::evaluate(
        &e,
        &parsed,
        crate::condition::Context {
            subject: Some(REF_B),
            ..Default::default()
        }
    ));
    assert!(!crate::condition::evaluate(
        &e,
        &parsed,
        crate::condition::Context {
            subject: Some(REF_A),
            ..Default::default()
        }
    ));
}
