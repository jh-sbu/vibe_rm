//! `test-mod autotest17`: when a behaviour state machine publishes its
//! current state to the graph variable bound to `currentStateId`
//! (`known_ambiguities/state-machine-outputs.md`). The humanoid graphs
//! bind it in `1HM_Behavior` (`i1HMState`: 0 ready, 1 blocking, 10
//! attacking...) and `Blocking_Behavior` (`iBlockState`: 0 blocking, 1 a
//! block hit), and Papyrus reads both with `GetAnimationVariableInt`. Our
//! choices: publish when a transition begins (not when its blend ends);
//! keep the source's ID while a transition waits for its initiate
//! interval; keep the last value when the machine deactivates, and publish
//! the start state when it activates again.
//!
//! The vanilla transitions used (`vrm-tool hkb-tree <Data>
//! meshes/actors/character 1hm_behavior.hkx 1HM_Behavior 2`):
//!
//! - `1HM_Ready_State` -> `BlockState` on `blockStart`: a 0.25 s blend for
//!   NPCs, 0.1 s for the player.
//! - `BlockState` -> `1HM_Ready_State` on `blockStop`: no blend when
//!   `iBlockState != 1`; when it is 1 (a block hit), only inside the
//!   `InitiateWinBegin` .. `InitiateWinEnd` interval, which the 1HM block
//!   hit clips mark 0.5 s in.
//! - `Blocking_Behavior` `BlockState` -> `BlockHit` on `blockHitStart`.
//!
//! One quest, starting as the save loads. A test NPC (commoner traits)
//! placed 150 units east of the player and the player are each given an
//! iron sword and shield and draw them. For each, the script sends events
//! with `Debug.SendAnimationEvent` and polls both variables as fast as
//! Papyrus allows, logging every change, the events sent and the
//! `blockHitStop` / `InitiateWinBegin` animation events, each with the
//! seconds since the page began. A page each (`OnUpdate`, 5 s after
//! loading; the NPC is deleted at the end):
//!
//! 1. **Setup**: both subjects' drawn weapon, `IsNPC`, hand types and the
//!    two variables; the polling interval over 1 s.
//! 2. **NpcBlend**: `blockStart` at 0, `blockStop` at 1.
//! 3. **NpcPending**: `blockStart` at 0, `blockHitStart` at 1, `blockStop`
//!    at 1.15, `blockStart` at 2.6, `blockStop` at 3.1.
//! 4. **PlayerBlend**, 5. **PlayerPending**: the same for the player.
//!
//! | reading | means |
//! | ------- | ----- |
//! | Blend: `i1HMState` 0 > 1 as soon after `blockStart` as 1 > 0 after `blockStop` | published as the transition begins (ours) |
//! | 0 > 1 about 0.25 s (player 0.1 s) later than 1 > 0 | published once the blend ends |
//! | Pending: `i1HMState` 1 > 0 near `InitiateWinBegin` (about 1.5) | the source's ID kept while the transition waits (ours) |
//! | 1 > 0 just after `blockStop` (1.15) | published when the event is taken, before the interval |
//! | `iBlockState` still 1 after `i1HMState` 1 > 0 | kept on deactivation (ours) |
//! | `iBlockState` 1 > 0 as `i1HMState` leaves 1 | reset on deactivation |
//! | `iBlockState` 1 > 0 after `blockStart` at 2.6 | start state published on activation (ours) |
//! | a change nobody sent, or `blockStart` ignored | the engine drives the graph too: the page is unusable for that subject |
//!
//! Load a save outdoors on open ground, out of combat, and leave the game
//! alone for about 40 s (the player's weapon is drawn and blocks by
//! script). The plugin name doesn't matter.
//!
//! Run in Skyrim SE once (2026-10-27, a save outdoors):
//!
//! - **The NPC never drew its weapon** (`IsWeaponDrawn` False after
//!   `DrawWeapon`, an unalerted commoner), so it ignored `blockStart`: both
//!   NPC pages are void. The player drew and nothing else drove its graph.
//! - **Polling** took about 0.17 s a sample (7 in 1.14 s idle): each
//!   iteration's two variable reads wait on the frame.
//! - **No animation events were heard**, not even `InitiateWinBegin` from
//!   the block hit clip that evidently played.
//! - **Blend** (player): `i1HMState` 0 > 1 0.18 s after `blockStart` (0.1 s
//!   blend), 1 > 0 0.17 s after `blockStop` (none). Inconclusive: one trial
//!   each at that resolution can't separate a 0.1 s offset.
//! - **Pending** (player): `blockHitStart` 1.149, `blockStop` 1.278;
//!   `iBlockState` 0 > 1 at 1.492, but `i1HMState` stayed 1 until 1.894,
//!   about 0.75 s after the hit began, past the clip's 0.5 s
//!   `InitiateWinBegin`. The transition waited, keeping the source's ID: as
//!   ours.
//! - **Deactivation**: `iBlockState` stayed 1 after `i1HMState` 1 > 0 until
//!   the next `blockStart` (sent 2.714): `i1HMState` 0 > 1 at 2.894,
//!   `iBlockState` 1 > 0 read at 2.987 in the same poll. Kept on
//!   deactivation, the start state published on activating again: as ours.

use anyhow::Result;
use esp::writer::{AttachedScript, Node, PropertyValue, write_plugin};
use esp::{FormId, LoadOrder};
use papyrus::build::{Arg, FunctionBuilder, ScriptBuilder, auto_var, id, string};
use papyrus::pex::op;

use super::TestMod;
use super::autotest2::{Npc, Vanilla, line, put};
use super::autotest4::report;
use super::own;
use super::records::Quest;
use super::script::{Code, num, on_update, real_time};

const QUEST: u32 = 0x800;
const NPC: u32 = 0x801;
const NEXT_OBJECT_ID: u32 = 0x802;
const SCRIPT: &str = "VrmAutoTest17Script";
const PLAYER_REF: FormId = FormId(0x14);

const TESTS: &[(&str, &str)] = &[
    ("Setup", "The subjects and the polling interval"),
    (
        "NpcBlend",
        "NPC: blockStart's 0.25 s blend against blockStop's none",
    ),
    (
        "NpcPending",
        "NPC: blockStop waiting for its interval, deactivation",
    ),
    (
        "PlayerBlend",
        "Player: blockStart's 0.1 s blend against blockStop's none",
    ),
    (
        "PlayerPending",
        "Player: blockStop waiting for its interval, deactivation",
    ),
];

/// The variables polled, and the script variable keeping each one's last
/// value.
const POLLED: [(&str, &str); 2] = [("i1HMState", "Last1HM"), ("iBlockState", "LastBlock")];

/// Changes and heard events logged per page, at most, to keep a page on
/// screen with its title, the start, five events sent and the summary.
const MAX_CHANGES: i32 = 10;

/// The animation events logged as they come.
const HEARD: [&str; 2] = ["blockHitStop", "InitiateWinBegin"];

/// What each pending page sends: (seconds since the page began, event).
const PENDING: [(f32, &str); 5] = [
    (0.0, "blockStart"),
    (1.0, "blockHitStart"),
    (1.15, "blockStop"),
    (2.6, "blockStart"),
    (3.1, "blockStop"),
];
const PENDING_END: f32 = 3.6;

fn seconds(c: &mut Code) -> Arg {
    let t = real_time(c);
    c.calc(op::FSUB, "Float", t, id("T0"))
}

fn read(c: &mut Code, a: Arg, var: &str) -> Arg {
    c.method(
        "ObjectReference",
        a,
        "GetAnimationVariableInt",
        [string(var)],
    )
}

/// `Begin(Actor a, String label)`: a page's log, its clock and the
/// variables' starting values.
fn begin() -> FunctionBuilder {
    let mut c = Code::new("None");
    let a = c.param("a", "Actor");
    let label = c.param("label", "String");
    let t = real_time(&mut c);
    c.assign(id("T0"), t);
    c.cast(id("Watched"), a.clone());
    c.assign(id("Samples"), 0.into());
    c.assign(id("Changes"), 0.into());
    c.assign(id("Log"), label);
    for (var, last) in POLLED {
        let v = read(&mut c, a.clone(), var);
        c.assign(id(last), v);
        put(&mut c, &id("Log"), &format!(" {var} "), id(last));
    }
    line(&mut c, &id("Log"), "\n");
    c.finish(Arg::None)
}

/// `Note(String text)`: a line in the log, with the time.
fn note() -> FunctionBuilder {
    let mut c = Code::new("None");
    let text = c.param("text", "String");
    let t = seconds(&mut c);
    let n = c.own("Num", "String", [t]);
    c.append(id("Log"), [n, string(" "), text, string("\n")]);
    c.finish(Arg::None)
}

/// `Change(String text)`: a change noted, while there's room on the page.
fn change() -> FunctionBuilder {
    let mut c = Code::new("None");
    let text = c.param("text", "String");
    let room = c.calc(op::CMP_LT, "Bool", id("Changes"), MAX_CHANGES.into());
    let skip = c.label();
    c.jmpf(room, skip);
    c.own("Note", "None", [text]);
    c.bind(skip);
    let n = c.calc(op::IADD, "Int", id("Changes"), 1.into());
    c.assign(id("Changes"), n);
    c.finish(Arg::None)
}

/// `Send(Actor a, String event)`.
fn send() -> FunctionBuilder {
    let mut c = Code::new("None");
    let a = c.param("a", "Actor");
    let event = c.param("event", "String");
    let text = c.temp("String");
    c.assign(text.clone(), string("sent "))
        .append(text.clone(), [event.clone()]);
    c.own("Note", "None", [text]);
    c.global("Debug", "SendAnimationEvent", [a, event]);
    c.finish(Arg::None)
}

/// `Poll(Actor a, Float until)`: reads the variables until `until` seconds
/// into the page, logging each change.
fn poll() -> FunctionBuilder {
    let mut c = Code::new("None");
    let a = c.param("a", "Actor");
    let until = c.param("until", "Float");
    let (top, done) = (c.label(), c.label());
    c.bind(top);
    let t = seconds(&mut c);
    let over = c.calc(op::CMP_GE, "Bool", t, until);
    c.jmpt(over, done);
    let n = c.calc(op::IADD, "Int", id("Samples"), 1.into());
    c.assign(id("Samples"), n);
    for (var, last) in POLLED {
        let v = read(&mut c, a.clone(), var);
        let same = c.calc(op::CMP_EQ, "Bool", v.clone(), id(last));
        let next = c.label();
        c.jmpt(same, next);
        let text = c.temp("String");
        c.assign(text.clone(), string(&format!("{var} ")))
            .append(text.clone(), [id(last), string(" > "), v.clone()]);
        c.own("Change", "None", [text]);
        c.assign(id(last), v);
        c.bind(next);
    }
    c.jmp(top);
    c.bind(done);
    c.finish(Arg::None)
}

/// `End()`: the log, with the samples taken and any changes left out.
fn end() -> FunctionBuilder {
    let mut c = Code::new("String");
    let t = seconds(&mut c);
    let n = c.own("Num", "String", [t]);
    put(&mut c, &id("Log"), "", id("Samples"));
    c.append(id("Log"), [string(" samples in "), n, string(" s")]);
    let over = c.calc(op::CMP_GT, "Bool", id("Changes"), MAX_CHANGES.into());
    let skip = c.label();
    c.jmpf(over, skip);
    put(&mut c, &id("Log"), "; changes in all ", id("Changes"));
    c.bind(skip);
    line(&mut c, &id("Log"), "\n");
    c.finish(id("Log"))
}

/// `Ready(Actor a)`: `a` holding the sword and shield, drawn.
fn ready() -> FunctionBuilder {
    let mut c = Code::new("None");
    let a = c.param("a", "Actor");
    let drawn = c.method("Actor", a.clone(), "IsWeaponDrawn", []);
    let skip = c.label();
    c.jmpt(drawn, skip);
    c.method("Actor", a, "DrawWeapon", []);
    c.wait(2.0);
    c.bind(skip);
    c.finish(Arg::None)
}

/// `OnAnimationEvent`: logged with the time, from the page's subject only.
fn on_animation_event() -> FunctionBuilder {
    let mut c = Code::new("None");
    let source = c.param("akSource", "ObjectReference");
    let event = c.param("asEventName", "String");
    let other = c.calc(op::CMP_EQ, "Bool", source, id("Watched"));
    let end = c.label();
    c.jmpf(other, end);
    let text = c.temp("String");
    c.assign(text.clone(), string("heard "))
        .append(text.clone(), [event]);
    c.own("Change", "None", [text]);
    c.bind(end);
    c.finish(Arg::None)
}

/// A subject's state for the setup page.
fn describe(c: &mut Code, r: &Arg, label: &str, a: Arg) {
    let drawn = c.method("Actor", a.clone(), "IsWeaponDrawn", []);
    put(c, r, &format!("{label}: drawn "), drawn);
    let npc = c.method(
        "ObjectReference",
        a.clone(),
        "GetAnimationVariableBool",
        [string("IsNPC")],
    );
    put(c, r, ", IsNPC ", npc);
    for var in [
        "iLeftHandType",
        "iRightHandType",
        "iWantBlock",
        "i1HMState",
        "iBlockState",
    ] {
        let v = read(c, a.clone(), var);
        put(c, r, &format!(", {var} "), v);
    }
    line(c, r, "\n");
}

fn test_setup() -> FunctionBuilder {
    let (mut c, r) = report();
    let player = auto_var("Player");
    let placed = c.method(
        "ObjectReference",
        player.clone(),
        "PlaceAtMe",
        [auto_var("NpcBase"), 1.into(), false.into(), false.into()],
    );
    let subject = c.temp("Actor");
    c.cast(subject.clone(), placed);
    c.assign(id("Subject"), subject);
    let npc = id("Subject");
    c.method(
        "ObjectReference",
        npc.clone(),
        "MoveTo",
        [
            player.clone(),
            150.0f32.into(),
            0.0f32.into(),
            0.0f32.into(),
            false.into(),
        ],
    );
    c.method("Actor", npc.clone(), "SetDontMove", [true.into()]);
    for a in [npc.clone(), player.clone()] {
        for item in ["Sword", "Shield"] {
            c.method(
                "ObjectReference",
                a.clone(),
                "AddItem",
                [auto_var(item), 1.into(), true.into()],
            );
            c.method(
                "Actor",
                a.clone(),
                "EquipItem",
                [auto_var(item), false.into(), true.into()],
            );
        }
        for event in HEARD {
            c.method(
                "Form",
                id("self"),
                "RegisterForAnimationEvent",
                [a.clone(), string(event)],
            );
        }
    }
    c.wait(1.0);
    for a in [npc.clone(), player.clone()] {
        c.method("Actor", a, "DrawWeapon", []);
    }
    c.wait(3.0);
    describe(&mut c, &r, "NPC", npc.clone());
    describe(&mut c, &r, "player", player);
    // The polling interval, with nothing sent.
    c.own("Begin", "None", [npc.clone(), string("idle NPC:")]);
    c.own("Poll", "None", [npc, 1.0f32.into()]);
    let log = c.own("End", "String", []);
    c.append(r.clone(), [log]);
    c.finish(r)
}

fn test_blend(subject: Arg) -> FunctionBuilder {
    let mut c = Code::new("String");
    c.own("Ready", "None", [subject.clone()]);
    c.own("Begin", "None", [subject.clone(), string("start:")]);
    c.own("Send", "None", [subject.clone(), string("blockStart")]);
    c.own("Poll", "None", [subject.clone(), 1.0f32.into()]);
    c.own("Send", "None", [subject.clone(), string("blockStop")]);
    c.own("Poll", "None", [subject, 2.0f32.into()]);
    let log = c.own("End", "String", []);
    c.finish(log)
}

/// The pending page; `last` also removes the NPC afterwards.
fn test_pending(subject: Arg, last: bool) -> FunctionBuilder {
    let mut c = Code::new("String");
    c.own("Ready", "None", [subject.clone()]);
    c.own("Begin", "None", [subject.clone(), string("start:")]);
    for (at, event) in PENDING {
        c.own("Poll", "None", [subject.clone(), at.into()]);
        c.own("Send", "None", [subject.clone(), string(event)]);
    }
    c.own("Poll", "None", [subject, PENDING_END.into()]);
    let log = c.own("End", "String", []);
    if last {
        c.method("ObjectReference", id("Subject"), "Delete", []);
    }
    c.finish(log)
}

fn scripts() -> Result<Vec<(String, Vec<u8>)>> {
    let mut s = ScriptBuilder::new(SCRIPT, "Quest");
    for (name, class) in [
        ("Player", "Actor"),
        ("NpcBase", "ActorBase"),
        ("Sword", "Weapon"),
        ("Shield", "Armor"),
    ] {
        s.auto_property(name, class);
    }
    s.variable("Ran", "Bool", false.into());
    s.variable("Subject", "Actor", Arg::None);
    s.variable("Watched", "ObjectReference", Arg::None);
    s.variable("T0", "Float", 0.0f32.into());
    s.variable("Log", "String", string(""));
    s.variable("Samples", "Int", 0.into());
    s.variable("Changes", "Int", 0.into());
    for (_, last) in POLLED {
        s.variable(last, "Int", 0.into());
    }
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
        .function("OnUpdate", on_update("autotest17", "40 seconds", TESTS))
        .function("OnAnimationEvent", on_animation_event())
        .function("Num", num())
        .function("Begin", begin())
        .function("Note", note())
        .function("Change", change())
        .function("Send", send())
        .function("Poll", poll())
        .function("End", end())
        .function("Ready", ready())
        .function("TestSetup", test_setup())
        .function("TestNpcBlend", test_blend(id("Subject")))
        .function("TestNpcPending", test_pending(id("Subject"), false))
        .function("TestPlayerBlend", test_blend(auto_var("Player")))
        .function("TestPlayerPending", test_pending(auto_var("Player"), true));
    Ok(vec![(SCRIPT.to_owned(), papyrus::pex::write(&s.build()?)?)])
}

pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    let v = Vanilla { lo };
    let object = PropertyValue::Object;
    let properties = vec![
        ("Player".to_owned(), object(PLAYER_REF)),
        ("NpcBase".to_owned(), object(own(NPC))),
        ("Sword".to_owned(), object(v.id("IronSword")?)),
        ("Shield".to_owned(), object(v.id("ArmorIronShield")?)),
    ];
    let npc = Npc {
        object_id: NPC,
        edid: "VrmAutoTest17Npc".into(),
        name: Some("vibe_rm Test NPC"),
        level: (10, false, 1, 0),
        template: (v.id("WEMerchantMiscMaleCommoner")?, 0x1),
        temper: (0, 0),
        factions: Vec::new(),
        script: None,
        assistance: 0,
        combat_style: None,
        unique: false,
    }
    .record(v.id("ImperialRace")?, v.id("Citizen")?);
    let quest = Quest {
        object_id: QUEST,
        edid: "VrmAutoTest17Quest",
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
        "When behaviour state machines publish their current state: blocking sent by script \
         to a test NPC and the player, i1HMState and iBlockState polled, reports saved.",
        NEXT_OBJECT_ID,
        &[
            Node::top(b"NPC_", vec![npc]),
            Node::top(b"QUST", vec![quest]),
        ],
    );
    Ok(TestMod {
        plugin,
        scripts: scripts()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_mods::script::dry_run::{form, run};

    #[test]
    fn the_tests_run_and_report() {
        let properties = vec![
            ("Player".to_owned(), form(0x14, "Actor")),
            ("NpcBase".to_owned(), form(own(NPC).0, "ActorBase")),
            ("Sword".to_owned(), form(0x12EB7, "Weapon")),
            ("Shield".to_owned(), form(0x12EB6, "Armor")),
        ];
        let shown = run(
            scripts().unwrap(),
            &[(own(QUEST).0, SCRIPT, properties)],
            TESTS.len(),
        );
        assert_eq!(shown.boxes.len(), TESTS.len(), "every test reports");
        assert_eq!(shown.saved_at_box, Some(0), "saves before the boxes");
        let all = shown.boxes.join("\n");
        for expected in [
            "NPC: drawn False, IsNPC False, iLeftHandType 1",
            "idle NPC: i1HMState 1 iBlockState 1\n",
            " sent blockStart\n",
            " sent blockHitStart\n",
            " samples in ",
        ] {
            assert!(all.contains(expected), "{expected:?} not in the reports");
        }
        for b in &shown.boxes {
            assert!(b.lines().count() <= 20, "{} lines:\n{b}", b.lines().count());
        }
    }
}
