//! Script variable conditions (`GetScriptVariable`, which reads a reference's
//! script variable named in CIS2) over synthetic references and Papyrus
//! instances; no game assets.
use super::*;
use esp::writer::{Node, RecordBuilder, write_plugin};
use papyrus::{ObjectId, SavedVm, Value};

const OBJ: FormId = FormId(0x850);
const QUEST: FormId = FormId(0x851);

const GET_SCRIPT_VARIABLE: u16 = 53;
const GET_GRAPH_VARIABLE_FLOAT: u16 = 447;

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
        std::env::temp_dir().join(format!("vrm-variable-cond-{}-{stamp}", std::process::id())),
    );
    std::fs::create_dir(&dir.0).unwrap();
    let records = [Node::top(b"QUST", vec![RecordBuilder::new(b"QUST", QUEST)])];
    std::fs::write(
        dir.0.join("Variable.esp"),
        write_plugin(
            0,
            &[],
            "test",
            "synthetic script variables",
            0x900,
            &records,
        ),
    )
    .unwrap();
    let engine = crate::stats::action_tests::headless_engine(&dir.0, "Variable.esp");
    (dir, engine)
}

/// Gives `obj` an attached Papyrus script holding `vars` (compiled names).
fn attach_vars(e: &mut Engine, obj: FormId, script: &str, vars: &[(&str, Value)]) {
    let saved = SavedVm {
        instances: vec![(
            ObjectId::Form(obj.0),
            script.into(),
            vars.iter().map(|(n, v)| ((*n).into(), v.clone())).collect(),
        )],
        attached: vec![(ObjectId::Form(obj.0), vec![script.into()])],
        threads: Vec::new(),
    };
    let mut vm = std::mem::take(&mut e.vm);
    let mut host = crate::script::EngineHost { engine: e };
    vm.import(&mut host, saved);
    e.vm = vm;
}

/// The condition `func(p1, "name") == value` as its CTDA and CIS1 / CIS2 would hold it
/// (the name's slot is the parameter's index, so either may carry it).
fn parsed(e: &Engine, func: u16, p1: FormId, name: Option<&str>, value: f32) -> Condition {
    let mut d = [0u8; 32];
    d[4..8].copy_from_slice(&value.to_le_bytes());
    d[8..10].copy_from_slice(&func.to_le_bytes());
    d[12..16].copy_from_slice(&p1.0.to_le_bytes());
    let rec = e.lo.get(QUEST).unwrap();
    let mut c = parse(&rec, &d).unwrap();
    c.string_p1 = name.map(Into::into);
    c.string_p2 = name.map(Into::into);
    c
}

fn holds(e: &Engine, c: Condition, subject: FormId) -> bool {
    evaluate(
        e,
        &[c],
        Context {
            subject: Some(subject),
            ..Default::default()
        },
    )
}

#[test]
fn get_script_variable_reads_the_named_variable_of_the_reference() {
    let (_dir, mut e) = fixture();
    attach_vars(
        &mut e,
        OBJ,
        "MyScript",
        &[("::speed_var", Value::Float(2.5))],
    );
    // The reference named in the condition, not the subject, holds the script.
    assert!(holds(
        &e,
        parsed(&e, GET_SCRIPT_VARIABLE, OBJ, Some("Speed"), 2.5),
        QUEST,
    ));
    assert!(!holds(
        &e,
        parsed(&e, GET_SCRIPT_VARIABLE, OBJ, Some("speed"), 1.0),
        QUEST,
    ));
    // A variable the reference's scripts lack is 0.
    assert!(holds(
        &e,
        parsed(&e, GET_SCRIPT_VARIABLE, OBJ, Some("missing"), 0.0),
        QUEST,
    ));
    // A reference without scripts: the same.
    assert!(holds(
        &e,
        parsed(&e, GET_SCRIPT_VARIABLE, QUEST, Some("speed"), 0.0),
        OBJ,
    ));
}

#[test]
fn get_script_variable_without_its_name_is_unsupported() {
    let (_dir, mut e) = fixture();
    attach_vars(
        &mut e,
        OBJ,
        "MyScript",
        &[("::speed_var", Value::Float(0.0))],
    );
    // No CIS2 subrecord names a variable: the condition fails, even comparing to 0.
    assert!(!holds(
        &e,
        parsed(&e, GET_SCRIPT_VARIABLE, OBJ, None, 0.0),
        QUEST,
    ));
}

#[test]
fn graph_variable_needs_a_running_graph() {
    let (_dir, mut e) = fixture();
    // An object with no behaviour graph (and no actor): unsupported, the condition fails.
    assert!(!holds(
        &e,
        parsed(&e, GET_GRAPH_VARIABLE_FLOAT, FormId(0), Some("speed"), 0.0),
        OBJ,
    ));
    attach_vars(
        &mut e,
        OBJ,
        "MyScript",
        &[("::speed_var", Value::Float(0.0))],
    );
    assert!(!holds(
        &e,
        parsed(&e, GET_GRAPH_VARIABLE_FLOAT, FormId(0), Some("speed"), 0.0),
        OBJ,
    ));
}
