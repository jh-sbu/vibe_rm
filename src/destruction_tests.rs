//! Synthetic definitions and query-path tests; no game assets are used.
use super::*;
use esp::writer::{Node, RecordBuilder, write_plugin};
use papyrus::Value;

const BASE: FormId = FormId(0x800);
const OTHER_BASE: FormId = FormId(0x801);
const REF_A: FormId = FormId(0x810);
const REF_B: FormId = FormId(0x811);
const REF_C: FormId = FormId(0x812);
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
            vec![
                reference(REF_A, BASE),
                reference(REF_B, OTHER_BASE),
                reference(REF_C, BASE),
            ],
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

fn damage_object(e: &mut Engine, this: &Value, damage: f32) {
    assert!(matches!(
        crate::script::natives::call(
            e,
            "objectreference",
            "damageobject",
            Some(this),
            &[Value::Float(damage)],
        ),
        papyrus::NativeResult::Value(Value::None)
    ));
}

fn stage(
    index: u8,
    health_percent: u8,
    damage_stage: u8,
    flags: u8,
) -> esp::destructible::DestructionStage {
    esp::destructible::DestructionStage {
        health_percent,
        index,
        damage_stage,
        flags,
        self_damage_per_second: 0,
        explosion: FormId::NULL,
        debris: FormId::NULL,
        debris_count: 0,
        replacement_model: None,
    }
}

#[test]
fn scripted_damage_crosses_authored_indices_and_caps_one_hit_at_first_capped_stage() {
    let data = esp::destructible::DestructibleData {
        health: 100,
        stage_count: 2,
        vats_targetable: false,
        // Deliberately distinct damage_stage values: Papyrus query state uses
        // the authored index under the current documented policy.
        stages: vec![stage(4, 90, 1, 0x01), stage(9, 0, 2, 0)],
    };

    assert_eq!(apply_damage(&data, None, f32::NAN), None);
    assert_eq!(apply_damage(&data, None, 0.0), None);
    assert_eq!(apply_damage(&data, None, -1.0), None);
    assert_eq!(apply_damage(&data, None, f32::INFINITY), None);

    let first = apply_damage(&data, None, 100.0).unwrap();
    assert_eq!(
        first,
        DestructionState {
            health: 90.0,
            stage: 4
        }
    );
    // One hit cannot pass the first CAP_DAMAGE stage.
    let second = apply_damage(&data, Some(first), 90.0).unwrap();
    assert_eq!(
        second,
        DestructionState {
            health: 0.0,
            stage: 9
        }
    );
    assert_eq!(apply_damage(&data, Some(second), 10.0), None);
}

#[test]
fn damage_cannot_heal_to_a_cap_threshold_or_reduce_health_below_zero() {
    let data = esp::destructible::DestructibleData {
        health: 100,
        stage_count: 1,
        vats_targetable: false,
        stages: vec![stage(3, 90, 1, 0x01)],
    };
    let already_below = DestructionState {
        health: 50.0,
        stage: -1,
    };
    let hit = apply_damage(&data, Some(already_below), 10.0).unwrap();
    assert_eq!(hit.health, 50.0);
    assert_eq!(hit.stage, 3);
    let lethal = apply_damage(&data, Some(hit), 100.0).unwrap();
    assert_eq!(lethal.health, 0.0);
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
    e.world_state.destruction.insert(
        REF_A,
        crate::world::destruction::DestructionState {
            health: 99.0,
            stage: 0,
        },
    );
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

#[test]
#[ignore = "needs a headless GPU adapter; synthetic records only"]
fn damageobject_is_loaded_reference_gated_and_keeps_health_per_reference() {
    let (_dir, mut e) = fixture();
    let this_a = e.object_value(REF_A);
    let this_b = e.object_value(REF_B);
    let this_c = e.object_value(REF_C);

    // A valid but unloaded reference is untouched.
    damage_object(&mut e, &this_a, 20.0);
    assert_eq!(e.current_destruction_stage(REF_A), -1);
    assert!(!e.world_state.destruction.contains_key(&REF_A));

    e.cells
        .entry(crate::render::CellKey::Interior(FormId(0x840)))
        .or_default()
        .refs
        .extend([REF_A, REF_B, REF_C]);

    damage_object(&mut e, &this_a, f32::NAN);
    damage_object(&mut e, &this_a, -5.0);
    assert!(!e.world_state.destruction.contains_key(&REF_A));

    // First cap stage prevents this hit from entering stage 1.
    damage_object(&mut e, &this_a, 100.0);
    assert_eq!(e.current_destruction_stage(REF_A), 0);
    assert_eq!(e.world_state.destruction[&REF_A].health, 99.0);
    assert_eq!(e.current_destruction_stage(REF_C), -1);

    // A loaded object without DEST is still a no-op.
    damage_object(&mut e, &this_b, 10.0);
    assert!(!e.world_state.destruction.contains_key(&REF_B));

    damage_object(&mut e, &this_c, 2.0);
    assert_eq!(e.current_destruction_stage(REF_C), 0);
    // The synthetic base's first stage is a 99% cap, so the hit stops there.
    assert_eq!(e.world_state.destruction[&REF_C].health, 99.0);
    damage_object(&mut e, &this_c, 1.0);
    assert_eq!(e.world_state.destruction[&REF_C].health, 98.0);
    assert_eq!(current_stage(&mut e, &this_c), 0);
    assert_eq!(e.world_state.destruction[&REF_A].health, 99.0);

    let parsed = crate::condition::parse_all(&e.lo.get(QUEST).unwrap());
    assert!(!crate::condition::evaluate(
        &e,
        &parsed,
        crate::condition::Context {
            subject: Some(REF_C),
            ..Default::default()
        }
    ));
}
