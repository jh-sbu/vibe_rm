//! `test-mod autotest18`: whether a state machine publishes its new state
//! to the `currentStateId` variable when a transition's blend begins or
//! when it ends (`known_ambiguities/state-machine-outputs.md`). autotest17
//! settled the rest but saw the player's `blockStart` once, at Papyrus's
//! 0.17 s polling resolution, against a 0.1 s blend.
//!
//! Here the player alone (autotest17's NPC never drew), with an iron sword
//! and shield drawn, runs 8 trials of four `1HM_Behavior` transitions,
//! sent with `Debug.SendAnimationEvent`, each timed from the send until
//! `i1HMState` reads the destination (the only variable read, so a sample
//! is one read):
//!
//! | event | transition | blend |
//! | ----- | ---------- | ----- |
//! | `blockStart` | ready (0) -> blocking (1) | 0.1 s (player) |
//! | `blockStop` | blocking -> ready | none |
//! | `shoutStart` | ready -> shouting (13) | 0.2 s |
//! | `shoutStop` | shouting -> ready | 0.2 s |
//!
//! | reading (mean latencies) | means |
//! | ------------------------ | ----- |
//! | all four about the same | published as the blend begins (ours) |
//! | `blockStart` about 0.1 s and the shouts about 0.2 s over `blockStop` | published once the blend ends |
//! | a trial at -1 (not seen in 1.5 s) | the event was refused, or the engine took the graph back |
//!
//! Load a save outdoors, out of combat, and leave the game alone for about
//! 40 s (the player draws, blocks and plays the shout animation by
//! script). The plugin name doesn't matter.
//!
//! Run in Skyrim SE once (2026-10-27, a save outdoors), every trial seen:
//!
//! | step | latencies (s) | mean |
//! | ---- | ------------- | ---- |
//! | `blockStart` (0.1 s blend) | 0.129-0.138 | 0.134 |
//! | `blockStop` (none) | 0.121-0.134 | 0.125 |
//! | `shoutStart` (0.2 s) | 0.128-0.140 | 0.132 |
//! | `shoutStop` (0.2 s) | 0.132-0.143 | 0.136 |
//!
//! All within 0.011 s of each other, far under the blends: the state is
//! published as the transition begins, as ours.

use anyhow::Result;
use esp::writer::{AttachedScript, Node, PropertyValue, write_plugin};
use esp::{FormId, LoadOrder};
use papyrus::build::{Arg, FunctionBuilder, ScriptBuilder, auto_var, id, string};
use papyrus::pex::op;

use super::TestMod;
use super::autotest2::{Vanilla, line, put};
use super::autotest4::report;
use super::records::Quest;
use super::script::{Code, num, on_update, real_time};

const QUEST: u32 = 0x800;
const NEXT_OBJECT_ID: u32 = 0x801;
const SCRIPT: &str = "VrmAutoTest18Script";
const PLAYER_REF: FormId = FormId(0x14);

const TESTS: &[(&str, &str)] = &[("Trials", "i1HMState's latency after each event")];

/// The transitions: event, the state it leads to, the pause after.
const STEPS: [(&str, i32, f32); 4] = [
    ("blockStart", 1, 0.4),
    ("blockStop", 0, 0.4),
    ("shoutStart", 13, 0.6),
    ("shoutStop", 0, 0.4),
];
const TRIALS: usize = 8;
/// How long a trial waits for the destination.
const TIMEOUT: f32 = 1.5;

/// `Trial(String event, Int target)`: seconds from sending `event` to the
/// player until `i1HMState` reads `target`, or -1.
fn trial() -> FunctionBuilder {
    let mut c = Code::new("Float");
    let event = c.param("event", "String");
    let target = c.param("target", "Int");
    let t0 = c.local("t0", "Float");
    let t = real_time(&mut c);
    c.assign(t0.clone(), t);
    c.global("Debug", "SendAnimationEvent", [auto_var("Player"), event]);
    let (top, out) = (c.label(), c.label());
    c.bind(top);
    let v = c.method(
        "ObjectReference",
        auto_var("Player"),
        "GetAnimationVariableInt",
        [string("i1HMState")],
    );
    let now = real_time(&mut c);
    let dt = c.calc(op::FSUB, "Float", now, t0);
    let hit = c.calc(op::CMP_EQ, "Bool", v, target);
    c.jmpt(hit, out);
    let over = c.calc(op::CMP_GT, "Bool", dt.clone(), TIMEOUT.into());
    c.jmpf(over, top);
    c.assign(dt.clone(), (-1.0f32).into());
    c.bind(out);
    c.finish(dt)
}

fn test_trials() -> FunctionBuilder {
    let (mut c, r) = report();
    let player = auto_var("Player");
    for item in ["Sword", "Shield"] {
        c.method(
            "ObjectReference",
            player.clone(),
            "AddItem",
            [auto_var(item), 1.into(), true.into()],
        );
        c.method(
            "Actor",
            player.clone(),
            "EquipItem",
            [auto_var(item), false.into(), true.into()],
        );
    }
    c.wait(1.0);
    c.method("Actor", player.clone(), "DrawWeapon", []);
    c.wait(3.0);
    let drawn = c.method("Actor", player.clone(), "IsWeaponDrawn", []);
    put(&mut c, &r, "drawn ", drawn);
    let state = c.method(
        "ObjectReference",
        player.clone(),
        "GetAnimationVariableInt",
        [string("i1HMState")],
    );
    put(&mut c, &r, ", i1HMState ", state);
    line(&mut c, &r, "\ntrial:");
    for (event, _, _) in STEPS {
        line(&mut c, &r, &format!(" {event}"));
    }
    line(&mut c, &r, "\n");
    // Sums and counts of the latencies seen, per step.
    let sums: Vec<Arg> = (0..STEPS.len())
        .map(|i| {
            let v = c.local(&format!("sum{i}"), "Float");
            c.assign(v.clone(), 0.0f32.into());
            v
        })
        .collect();
    let counts: Vec<Arg> = (0..STEPS.len())
        .map(|i| {
            let v = c.local(&format!("n{i}"), "Int");
            c.assign(v.clone(), 0.into());
            v
        })
        .collect();
    for k in 0..TRIALS {
        line(&mut c, &r, &format!("{}:", k + 1));
        for (i, (event, target, pause)) in STEPS.into_iter().enumerate() {
            let dt = c.own("Trial", "Float", [string(event), target.into()]);
            c.numbers(&r, [(" ", dt.clone())]);
            let missed = c.calc(op::CMP_LT, "Bool", dt.clone(), 0.0f32.into());
            let skip = c.label();
            c.jmpt(missed, skip);
            let s = c.calc(op::FADD, "Float", sums[i].clone(), dt);
            c.assign(sums[i].clone(), s);
            let n = c.calc(op::IADD, "Int", counts[i].clone(), 1.into());
            c.assign(counts[i].clone(), n);
            c.bind(skip);
            c.wait(pause);
        }
        line(&mut c, &r, "\n");
    }
    line(&mut c, &r, "mean:");
    for i in 0..STEPS.len() {
        let none = c.calc(op::CMP_EQ, "Bool", counts[i].clone(), 0.into());
        let (some, next) = (c.label(), c.label());
        c.jmpf(none, some);
        line(&mut c, &r, " -");
        c.jmp(next);
        c.bind(some);
        let n = c.temp("Float");
        c.cast(n.clone(), counts[i].clone());
        let mean = c.calc(op::FDIV, "Float", sums[i].clone(), n);
        c.numbers(&r, [(" ", mean)]);
        c.bind(next);
    }
    line(&mut c, &r, "\n");
    c.finish(r)
}

fn scripts() -> Result<Vec<(String, Vec<u8>)>> {
    let mut s = ScriptBuilder::new(SCRIPT, "Quest");
    for (name, class) in [
        ("Player", "Actor"),
        ("Sword", "Weapon"),
        ("Shield", "Armor"),
    ] {
        s.auto_property(name, class);
    }
    s.variable("Ran", "Bool", false.into());
    for (name, _) in TESTS {
        s.variable(&format!("Result{name}"), "String", string(""));
    }
    let mut init = Code::new("None");
    init.method(
        "Form",
        id("self"),
        "RegisterForSingleUpdate",
        [5.0f32.into()],
    );
    s.function("OnInit", init.finish(Arg::None))
        .function("OnUpdate", on_update("autotest18", "40 seconds", TESTS))
        .function("Num", num())
        .function("Trial", trial())
        .function("TestTrials", test_trials());
    Ok(vec![(SCRIPT.to_owned(), papyrus::pex::write(&s.build()?)?)])
}

pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    let v = Vanilla { lo };
    let object = PropertyValue::Object;
    let properties = vec![
        ("Player".to_owned(), object(PLAYER_REF)),
        ("Sword".to_owned(), object(v.id("IronSword")?)),
        ("Shield".to_owned(), object(v.id("ArmorIronShield")?)),
    ];
    let quest = Quest {
        object_id: QUEST,
        edid: "VrmAutoTest18Quest",
        flags: 0x0011,
        priority: 0,
        kind: 0,
        script: Some(AttachedScript {
            name: SCRIPT.into(),
            properties,
        }),
        stages: &[],
        aliases: Vec::new(),
    }
    .record();
    let plugin = write_plugin(
        0,
        &["Skyrim.esm"],
        "vibe_rm",
        "When a behaviour state machine publishes its new state: blocking and shouting sent \
         to the player by script, i1HMState timed after each, reports saved.",
        NEXT_OBJECT_ID,
        &[Node::top(b"QUST", vec![quest])],
    );
    Ok(TestMod {
        plugin,
        scripts: scripts()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_mods::own;
    use crate::test_mods::script::dry_run::{form, run};

    #[test]
    fn the_trials_run_and_report() {
        let properties = vec![
            ("Player".to_owned(), form(0x14, "Actor")),
            ("Sword".to_owned(), form(0x12EB7, "Weapon")),
            ("Shield".to_owned(), form(0x12EB6, "Armor")),
        ];
        let shown = run(
            scripts().unwrap(),
            &[(own(QUEST).0, SCRIPT, properties)],
            TESTS.len(),
        );
        assert_eq!(shown.boxes.len(), TESTS.len());
        assert_eq!(shown.saved_at_box, Some(0), "saves before the boxes");
        let b = &shown.boxes[0];
        // The dry run's variables always read 1: only blockStart's target.
        assert!(b.contains("\n1: 0 -1 -1 -1\n"), "{b}");
        assert!(b.contains("\nmean: 0 - - -\n"), "{b}");
        assert!(b.lines().count() <= 20, "{} lines:\n{b}", b.lines().count());
    }
}
