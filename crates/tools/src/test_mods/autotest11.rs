//! `test-mod autotest11`: when shops' doors lock and unlock
//! (`known_ambiguities/locks.md`, "Which doors a package locks"). The engine
//! locks an owner's home doors when its sleep package with "Lock Doors?"
//! starts, and unlocks them only when a package with `PKDT` 0x40 ("At
//! Package Start Unlock Doors") starts or one with 0x80 ("On Package Change
//! Unlock Doors") ends. Only 50 of Skyrim.esm's 5961 packages carry either,
//! none of Belethor's or Arcadia's, so their shops never open; UESP has
//! Belethor opening his shop at 8am and the shops open 8am to 8pm.
//!
//! Three shops, their load door pairs locked (adept) on the inside half:
//!
//! | shop              | owner    | packages                                             |
//! | ----------------- | -------- | ---------------------------------------------------- |
//! | Belethor's        | Belethor | sell 8:00-20:00, patrol 20:00-24:00, sleep (locks) the rest |
//! | Arcadia's Cauldron| Arcadia  | none flagged to unlock                               |
//! | Riverwood Trader  | Lucan    | `MS13LucanValeriusTraderServices` unlocks at start (the control) |
//!
//! The script sets the hour (`GameHour`) a few game minutes before each of
//! Belethor's package changes and samples both halves of each shop's door
//! (`IsLocked`) every 5 s (1:40 game minutes) while the clock runs past it,
//! with the owners' packages and cells before and after. Then it jumps the
//! clock to 3:00, 12:00 and 21:00 (no package's start passed on the way,
//! only time) and samples each.
//!
//! | reading                                         | means                                             |
//! | ----------------------------------------------- | ------------------------------------------------- |
//! | Belethor's and Arcadia's unlock at about 8:00   | an owner's doors open when its lock-doors sleep ends (or its day package starts) |
//! | they lock at 20:00                              | they lock when the day package ends, not only at sleep |
//! | they lock only at 0:00                          | the sleep package locks them, as the engine does   |
//! | the jumps leave them as they were               | only a package's start or end changes them        |
//! | the jumps set them by the hour                  | their state follows the owner's current package   |
//! | none change                                     | unloaded owners don't move them; run it inside a shop |
//!
//! Run from a save you can throw away, in Whiterun outside the shops (the
//! owners unloaded), starting before 7:30 (wait with T if needed) so the
//! clock only moves forward. A second run inside Belethor's shop (him
//! loaded) tells whether that matters. It takes about 5 minutes; leave the
//! game alone. The plugin name doesn't matter.
//!
//! Run in Skyrim SE (2026-10-22) twice, from 6:52 outside the shops in the
//! Plains District and from 7:16 inside Belethor's shop, sneaking unseen:
//!
//! - **Arcadia** (unloaded, in her shop throughout): locked exactly while
//!   her sleep package (`DefaultSleepEditorLoc24x8`) ran. Unlocked within
//!   1:40 game minutes of 8:00 as `WhiterunArcadiaStorePackage` took over,
//!   and at the jump to 12:00; locked as the sleep package started at 0:00
//!   (inside run; outside, her evening package still ran at 0:15) and at
//!   the jump to 3:00. Not at 20:00 (`DefaultSandboxHomeowner`). Each
//!   change came as her package switched, the clock running or jumped.
//! - **Lucan** stayed on `MS13LucanValeriusTraderServices` (unlocks at
//!   start) throughout, his doors unlocked: no change to read.
//! - **Belethor**, outside run: in the city at the start (sleep package,
//!   doors locked) and through the night; his doors stayed locked all day,
//!   through his sell package and his being moved into the shop at 12:00.
//!   Inside run: locked at the start (asleep), unlocked at 8:13, then not
//!   locked again by his sleep at 0:00 or at the jump to 3:00.
//!
//! So a lock-doors package's end opens the home (the engine now unlocks it
//! then); Belethor's runs leave open what an owner away from home or the
//! player inside changes (`known_ambiguities/locks.md`).

use anyhow::{Context, Result, ensure};
use esp::writer::{AttachedScript, Node, write_plugin};
use esp::{FormId, LoadOrder};
use papyrus::build::{Arg, FunctionBuilder, ScriptBuilder, id, string};
use papyrus::pex::op;

use super::TestMod;
use super::autotest2::{line, put};
use super::autotest4::report;
use super::records::Quest;
use super::script::{Code, num, on_update};

const QUEST: u32 = 0x800;
const NEXT_OBJECT_ID: u32 = 0x801;
const SCRIPT: &str = "VrmAutoTest11Script";
/// `GameHour` in Skyrim.esm.
pub(super) const GAME_HOUR: u32 = 0x38;

/// A shop: its label, the inside and outside halves of its front door, its
/// owner's reference (Skyrim.esm object ids).
struct Shop {
    label: &'static str,
    inside: u32,
    outside: u32,
    owner: u32,
}

const SHOPS: &[Shop] = &[
    Shop {
        label: "Belethor",
        inside: 0x16607,
        outside: 0x1A6F5,
        owner: 0x1A672,
    },
    Shop {
        label: "Arcadia",
        inside: 0x165C9,
        outside: 0x1A6F3,
        owner: 0x1A66D,
    },
    Shop {
        label: "Lucan",
        inside: 0x133E9,
        outside: 0x1341F,
        owner: 0x13487,
    },
];

/// The clock run past a package change: the test ([`TESTS`]), the hour set.
const RUNS: &[(&str, f32)] = &[("Open", 7.95), ("Close", 19.95), ("Night", 23.95)];
/// Samples in each run, 5 s apart (a page: title, owners, these, owners).
const SAMPLES: usize = 11;
/// Hours jumped to, and the samples after each (seconds after the jump).
const JUMPS: &[f32] = &[3.0, 12.0, 21.0];
const JUMP_SAMPLES: &[f32] = &[1.0, 5.0, 10.0];

const TESTS: &[(&str, &str)] = &[
    ("Now", "The shops' doors and owners as the test starts"),
    ("Open", "08:00, sleep -> day packages"),
    ("Close", "20:00, Belethor's sell -> patrol"),
    ("Night", "00:00, Belethor's patrol -> sleep"),
    ("Jumps", "The clock jumped to 3:00, 12:00, 21:00"),
];

/// Skyrim.esm's form `object_id`, cast to `class`.
pub(super) fn vanilla(c: &mut Code, object_id: u32, class: &str) -> Arg {
    let f = c.global(
        "Game",
        "GetFormFromFile",
        [(object_id as i32).into(), string("Skyrim.esm")],
    );
    let t = c.temp(class);
    c.cast(t.clone(), f);
    t
}

pub(super) fn hour(c: &mut Code) -> Arg {
    let g = vanilla(c, GAME_HOUR, "GlobalVariable");
    c.method("GlobalVariable", g, "GetValue", [])
}

pub(super) fn set_hour(c: &mut Code, h: f32) {
    let g = vanilla(c, GAME_HOUR, "GlobalVariable");
    c.method("GlobalVariable", g, "SetValue", [h.into()]);
}

/// `L` locked, `-` not.
pub(super) fn flag(c: &mut Code, r: &Arg, locked: Arg) {
    let (open, next) = (c.label(), c.label());
    c.jmpf(locked, open);
    line(c, r, "L");
    c.jmp(next);
    c.bind(open);
    line(c, r, "-");
    c.bind(next);
}

/// `Sample()`: one line, the hour and each shop's door halves (inside,
/// outside), `L` locked.
fn sample() -> FunctionBuilder {
    let (mut c, r) = report();
    let h = hour(&mut c);
    c.numbers(&r, [("  ", h)]);
    for s in SHOPS {
        line(&mut c, &r, &format!("  {} ", s.label));
        for door in [s.inside, s.outside] {
            let d = vanilla(&mut c, door, "ObjectReference");
            let locked = c.method("ObjectReference", d, "IsLocked", []);
            flag(&mut c, &r, locked);
        }
    }
    line(&mut c, &r, "\n");
    c.finish(r)
}

/// `Owners()`: each owner's package and cell, a line each.
fn owners() -> FunctionBuilder {
    let (mut c, r) = report();
    for s in SHOPS {
        let a = vanilla(&mut c, s.owner, "Actor");
        let p = c.method("Actor", a.clone(), "GetCurrentPackage", []);
        put(&mut c, &r, &format!("  {}: ", s.label), p);
        let cell = c.method("ObjectReference", a.clone(), "GetParentCell", []);
        put(&mut c, &r, " in ", cell);
        let loaded = c.method("ObjectReference", a, "Is3DLoaded", []);
        put(&mut c, &r, ", 3D loaded ", loaded);
        line(&mut c, &r, "\n");
    }
    c.finish(r)
}

fn test_now() -> FunctionBuilder {
    let (mut c, r) = report();
    let player = c.global("Game", "GetPlayer", []);
    let cell = c.method("ObjectReference", player, "GetParentCell", []);
    put(&mut c, &r, "player in ", cell);
    let h = hour(&mut c);
    c.numbers(&r, [(", hour ", h.clone())]);
    let late = c.calc(op::CMP_GT, "Bool", h, RUNS[0].1.into());
    let early = c.label();
    c.jmpf(late, early);
    line(
        &mut c,
        &r,
        " (after 7:57: the first run moves the clock back)",
    );
    c.bind(early);
    line(&mut c, &r, "\ndoors (inside, outside):\n");
    for s in SHOPS {
        line(&mut c, &r, &format!("  {}: ", s.label));
        for (half, door) in [("inside", s.inside), ("outside", s.outside)] {
            let d = vanilla(&mut c, door, "ObjectReference");
            let locked = c.method("ObjectReference", d.clone(), "IsLocked", []);
            put(&mut c, &r, &format!("{half} locked "), locked);
            let level = c.method("ObjectReference", d, "GetLockLevel", []);
            put(&mut c, &r, " level ", level);
            line(&mut c, &r, "; ");
        }
        line(&mut c, &r, "\n");
    }
    let o = c.own("Owners", "String", []);
    c.append(r.clone(), [o]);
    let s = c.own("Sample", "String", []);
    c.append(r.clone(), [s]);
    c.finish(r)
}

/// A run: the hour set to `at`, then a sample every 5 s, the owners before
/// and after.
fn test_run(at: f32) -> FunctionBuilder {
    let (mut c, r) = report();
    set_hour(&mut c, at);
    c.wait(1.0);
    let o = c.own("Owners", "String", []);
    c.append(r.clone(), [o]);
    for i in 0..SAMPLES {
        if i > 0 {
            c.wait(5.0);
        }
        let s = c.own("Sample", "String", []);
        c.append(r.clone(), [s]);
    }
    line(&mut c, &r, "after:\n");
    let o = c.own("Owners", "String", []);
    c.append(r.clone(), [o]);
    c.finish(r)
}

fn test_jumps() -> FunctionBuilder {
    let (mut c, r) = report();
    for &h in JUMPS {
        set_hour(&mut c, h);
        let mut waited = 0.0;
        for &t in JUMP_SAMPLES {
            c.wait(t - waited);
            waited = t;
            let s = c.own("Sample", "String", []);
            c.append(r.clone(), [s]);
        }
        let o = c.own("Owners", "String", []);
        c.append(r.clone(), [o]);
    }
    c.finish(r)
}

fn scripts() -> Result<Vec<(String, Vec<u8>)>> {
    let mut s = ScriptBuilder::new(SCRIPT, "Quest");
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
        .function("OnUpdate", on_update("autotest11", "5 minutes", TESTS))
        .function("Num", num())
        .function("Sample", sample())
        .function("Owners", owners())
        .function("TestNow", test_now())
        .function("TestJumps", test_jumps());
    for &(name, at) in RUNS {
        s.function(&format!("Test{name}"), test_run(at));
    }
    Ok(vec![(SCRIPT.to_owned(), papyrus::pex::write(&s.build()?)?)])
}

/// The shops' doors and owners as this load order has them: each inside
/// half locked and leading to the outside one, the owner a placed actor.
fn check(lo: &LoadOrder) -> Result<()> {
    for s in SHOPS {
        let inside = lo.get(FormId(s.inside)).context("inside door")?;
        ensure!(
            inside.get(b"XLOC").is_some(),
            "{}: inside door unlocked",
            s.label
        );
        let to = inside
            .get(b"XTEL")
            .filter(|d| d.len() >= 4)
            .map(|d| inside.fid(FormId(u32::from_le_bytes(d[..4].try_into().unwrap()))));
        ensure!(
            to == Some(FormId(s.outside)),
            "{}: the inside door leads to {to:?}",
            s.label
        );
        let owner = lo.get(FormId(s.owner)).context("owner")?;
        ensure!(owner.get(b"NAME").is_some(), "{}: owner unplaced", s.label);
    }
    Ok(())
}

pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    check(lo)?;
    let quests = vec![
        Quest {
            object_id: QUEST,
            edid: "VrmAutoTest11Quest",
            flags: 0x0011,
            priority: 0,
            kind: 0,
            script: Some(AttachedScript {
                name: SCRIPT.into(),
                properties: Vec::new(),
            }),
            stages: &[],
            aliases: Vec::new(),
        }
        .record(),
    ];
    let plugin = write_plugin(
        0,
        &["Skyrim.esm"],
        "vibe_rm",
        "When shops' doors lock and unlock: the clock set just before their owners' package \
         changes and jumped, both halves of each door sampled by script, reports saved.",
        NEXT_OBJECT_ID,
        &[Node::top(b"QUST", quests)],
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
    use crate::test_mods::script::dry_run::run;

    #[test]
    fn the_tests_run_and_report() {
        let shown = run(
            scripts().unwrap(),
            &[(own(QUEST).0, SCRIPT, Vec::new())],
            TESTS.len(),
        );
        assert_eq!(shown.boxes.len(), TESTS.len(), "every test reports");
        assert_eq!(shown.saved_at_box, Some(0), "saves before the boxes");
        let all = shown.boxes.join("\n");
        for expected in [
            "  Belethor: inside locked False level 1; outside locked False level 1; \n",
            "  1  Belethor --  Arcadia --  Lucan --\n",
            "  Lucan: None in None, 3D loaded False\n",
        ] {
            assert!(all.contains(expected), "{expected:?} not in the reports");
        }
        for b in &shown.boxes {
            assert!(b.lines().count() <= 20, "{} lines:\n{b}", b.lines().count());
        }
    }
}
