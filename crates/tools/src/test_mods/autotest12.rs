//! `test-mod autotest12`: what an owner away from home and the player inside
//! change about a home locking for the night (`known_ambiguities/locks.md`,
//! "Which doors a package locks"). autotest11 settled that Arcadia's shop,
//! she unloaded in it, is locked exactly while her sleep package
//! (`DefaultSleepEditorLoc24x8`, 0:00-8:00) runs. Belethor departed from it
//! twice: away in the city all night, his doors (already locked) stayed
//! locked all day; with the player inside, his sleep didn't lock them. This
//! tests the same on Arcadia, whose day package has no unreachable target.
//!
//! The script picks its test by where the player is when it starts:
//!
//! **Away** (the player anywhere but Arcadia's Cauldron, e.g. the Bannered
//! Mare, so she is unloaded): at 23:57 her door is unlocked and she is moved
//! to Riverwood (the Riverwood Trader's door); sampled as her sleep starts.
//! At about 0:15 she is moved back inside her shop and sampled again. Then,
//! if her door is still unlocked, the script locks it; the clock is set to
//! 7:57 and sampled past 8:00.
//!
//! | reading                                 | means                                          |
//! | --------------------------------------- | ---------------------------------------------- |
//! | locks at 0:00, she in Riverwood         | locking doesn't need the owner home            |
//! | locks only once she is moved home       | locking waits on the owner being home          |
//! | never locks that night                  | only the package's start locks, the owner home |
//! | the script's lock opens at 8:00         | the end opens the home whoever locked it       |
//! | the script's lock stays at 8:00         | the end opens only what the package locked (Belethor's day) |
//!
//! **Inside** (the player in Arcadia's Cauldron before 23:57): her door is
//! unlocked at 23:57 and sampled with the player inside until about 1:00,
//! then the player is moved out of the front door and it is sampled to about
//! 1:20. Each sample also reads whether Arcadia detects the player
//! (`IsDetectedBy`), her distance, the player's Whiterun bounty, and shows
//! a notification with the hour and the door, so voiced warnings ("You need
//! to leave") can be put beside the readings. Run twice: sneaking unseen,
//! then in plain view.
//!
//! | reading                                 | means                                          |
//! | --------------------------------------- | ---------------------------------------------- |
//! | locks at 0:00 with the player inside    | the player's presence doesn't matter           |
//! | locks only once the player is moved out | sleepers don't lock a home with the player in it |
//! | sneaking and plain view differ          | being detected (a warning?) matters            |
//! | a bounty                                | staying is trespassing                          |
//!
//! Run from a save you can throw away; it takes about 4 minutes each, and
//! leave the game alone (note the hour in the notification when she speaks).
//! The plugin name doesn't matter.

use anyhow::{Context, Result, ensure};
use esp::writer::{AttachedScript, Node, write_plugin};
use esp::{FormId, LoadOrder};
use papyrus::build::{Arg, FunctionBuilder, ScriptBuilder, id, string};
use papyrus::pex::op;

use super::TestMod;
use super::autotest2::{Vanilla, as_form, line, put};
use super::autotest4::report;
use super::autotest11::{flag, hour, set_hour, vanilla};
use super::records::Quest;
use super::script::{Code, num, on_update};

const QUEST: u32 = 0x800;
const NEXT_OBJECT_ID: u32 = 0x801;
const SCRIPT: &str = "VrmAutoTest12Script";

/// Skyrim.esm object ids: Arcadia, her shop's front door (inside and
/// outside halves) and cell, and the Riverwood Trader's outside door.
const ARCADIA: u32 = 0x1A66D;
const DOOR_IN: u32 = 0x165C9;
const DOOR_OUT: u32 = 0x1A6F3;
const SHOP: u32 = 0x165B6;
const RIVERWOOD_DOOR: u32 = 0x1341F;

/// The hour each night starts at, and samples per page (5 s, 1:40 game
/// minutes, apart).
const NIGHT: f32 = 23.95;
const MORNING: f32 = 7.95;
const SAMPLES: usize = 11;
/// Pages of the player inside: 23:57 to about 1:00.
const INSIDE_PAGES: usize = 3;
const INSIDE_SAMPLES: usize = 13;

const TESTS: &[(&str, &str)] = &[
    ("Start", "Where everyone is as the test starts"),
    ("Away", "Away: Arcadia in Riverwood as her sleep starts"),
    ("Home", "Away: Arcadia moved home, asleep"),
    ("Wake", "Away: her sleep ends, the door as left"),
    ("Inside1", "Inside: the player in the shop from 23:57"),
    ("Inside2", "Inside: the player in the shop, on"),
    ("Inside3", "Inside: the player in the shop, on"),
    ("Out", "Inside: the player moved out of the shop"),
];

fn arcadia(c: &mut Code) -> Arg {
    vanilla(c, ARCADIA, "Actor")
}

fn door_locked(c: &mut Code, door: u32) -> Arg {
    let d = vanilla(c, door, "ObjectReference");
    c.method("ObjectReference", d, "IsLocked", [])
}

/// The page's start when it isn't this run's test: a line saying so, and a
/// jump to `skip` (bound before the report's end).
fn only_if(c: &mut Code, r: &Arg, inside: bool) -> papyrus::build::Label {
    let skip = c.label();
    let go = c.label();
    if inside {
        c.jmpt(id("Inside"), go);
    } else {
        c.jmpf(id("Inside"), go);
    }
    line(
        c,
        r,
        if inside {
            "(not this run: for the player inside Arcadia's Cauldron)\n"
        } else {
            "(not this run: for the player away from Arcadia's Cauldron)\n"
        },
    );
    c.jmp(skip);
    c.bind(go);
    skip
}

/// `Away()`: a line, the hour, the door (inside, outside), Arcadia's cell
/// and package.
fn away_sample() -> FunctionBuilder {
    let (mut c, r) = report();
    let h = hour(&mut c);
    c.numbers(&r, [("  ", h)]);
    line(&mut c, &r, " ");
    for door in [DOOR_IN, DOOR_OUT] {
        let locked = door_locked(&mut c, door);
        flag(&mut c, &r, locked);
    }
    let a = arcadia(&mut c);
    let cell = c.method("ObjectReference", a.clone(), "GetParentCell", []);
    put(&mut c, &r, " ", cell);
    let p = c.method("Actor", a, "GetCurrentPackage", []);
    put(&mut c, &r, " ", p);
    line(&mut c, &r, "\n");
    c.finish(r)
}

/// `Inside(Int crimeFaction)`: a line, the hour, the door, whether Arcadia
/// detects the player and how far she is, the player's bounty; and a
/// notification of the hour and the door.
fn inside_sample(crime_faction: u32) -> FunctionBuilder {
    let (mut c, r) = report();
    let note = c.local("note", "String");
    let h = hour(&mut c);
    c.numbers(&r, [("  ", h.clone())]);
    c.assign(note.clone(), string("autotest12 "));
    c.numbers(&note, [("", h)]);
    line(&mut c, &r, " ");
    c.append(note.clone(), [string(" door ")]);
    for door in [DOOR_IN, DOOR_OUT] {
        let locked = door_locked(&mut c, door);
        flag(&mut c, &r, locked.clone());
        flag(&mut c, &note, locked);
    }
    let a = arcadia(&mut c);
    let player = c.global("Game", "GetPlayer", []);
    let seen = c.method("Actor", player.clone(), "IsDetectedBy", [a.clone()]);
    put(&mut c, &r, " detected ", seen);
    let d = c.method("ObjectReference", a.clone(), "GetDistance", [player]);
    c.numbers(&r, [(" at ", d)]);
    let f = vanilla(&mut c, crime_faction, "Faction");
    let gold = c.method("Faction", f, "GetCrimeGold", []);
    put(&mut c, &r, " bounty ", gold);
    let p = c.method("Actor", a, "GetCurrentPackage", []);
    put(&mut c, &r, " ", p);
    line(&mut c, &r, "\n");
    c.global("Debug", "Notification", [note]);
    c.finish(r)
}

fn samples(c: &mut Code, r: &Arg, function: &str, n: usize) {
    for i in 0..n {
        if i > 0 {
            c.wait(5.0);
        }
        let s = c.own(function, "String", []);
        c.append(r.clone(), [s]);
    }
}

/// Unlock her door (both halves share the lock) and set the night.
fn night(c: &mut Code) {
    let d = vanilla(c, DOOR_IN, "ObjectReference");
    c.method("ObjectReference", d, "Lock", [false.into(), false.into()]);
    set_hour(c, NIGHT);
    c.wait(1.0);
}

fn move_to(c: &mut Code, who: Arg, target: u32) {
    let t = vanilla(c, target, "ObjectReference");
    c.method(
        "ObjectReference",
        who,
        "MoveTo",
        [t, 0.0f32.into(), 0.0f32.into(), 0.0f32.into(), true.into()],
    );
}

/// `forced`: the run to take whatever the player's cell (for the dry runs).
fn test_start(forced: Option<bool>) -> FunctionBuilder {
    let (mut c, r) = report();
    let player = c.global("Game", "GetPlayer", []);
    let cell = c.method("ObjectReference", player, "GetParentCell", []);
    put(&mut c, &r, "player in ", cell.clone());
    let shop = vanilla(&mut c, SHOP, "Cell");
    let (a, b) = (as_form(&mut c, cell), as_form(&mut c, shop));
    let inside = match forced {
        Some(f) => Arg::from(f),
        None => c.calc(op::CMP_EQ, "Bool", a, b),
    };
    c.assign(id("Inside"), inside.clone());
    let h = hour(&mut c);
    c.numbers(&r, [(", hour ", h.clone())]);
    let late = c.calc(op::CMP_GT, "Bool", h, NIGHT.into());
    let early = c.label();
    c.jmpf(late, early);
    line(&mut c, &r, " (after 23:57: the night moves the clock back)");
    c.bind(early);
    let (inside_run, next) = (c.label(), c.label());
    c.jmpt(inside, inside_run);
    line(&mut c, &r, "\nrunning: Away (Arcadia sent to Riverwood)\n");
    c.jmp(next);
    c.bind(inside_run);
    line(&mut c, &r, "\nrunning: Inside (the player in her shop)\n");
    c.bind(next);
    line(&mut c, &r, "now:\n");
    let s = c.own("Away", "String", []);
    c.append(r.clone(), [s]);
    c.finish(r)
}

fn test_away() -> FunctionBuilder {
    let (mut c, r) = report();
    let skip = only_if(&mut c, &r, false);
    night(&mut c);
    let a = arcadia(&mut c);
    move_to(&mut c, a, RIVERWOOD_DOOR);
    samples(&mut c, &r, "Away", SAMPLES);
    c.bind(skip);
    c.finish(r)
}

fn test_home() -> FunctionBuilder {
    let (mut c, r) = report();
    let skip = only_if(&mut c, &r, false);
    let a = arcadia(&mut c);
    move_to(&mut c, a, DOOR_IN);
    c.wait(1.0);
    samples(&mut c, &r, "Away", SAMPLES);
    c.bind(skip);
    c.finish(r)
}

fn test_wake() -> FunctionBuilder {
    let (mut c, r) = report();
    let skip = only_if(&mut c, &r, false);
    let locked = door_locked(&mut c, DOOR_IN);
    let (already, set) = (c.label(), c.label());
    c.jmpt(locked, already);
    let d = vanilla(&mut c, DOOR_IN, "ObjectReference");
    c.method("ObjectReference", d, "Lock", [true.into(), false.into()]);
    line(
        &mut c,
        &r,
        "unlocked after the night: locked by the script\n",
    );
    c.jmp(set);
    c.bind(already);
    line(&mut c, &r, "locked after the night (not by the script)\n");
    c.bind(set);
    set_hour(&mut c, MORNING);
    c.wait(1.0);
    samples(&mut c, &r, "Away", SAMPLES);
    c.bind(skip);
    c.finish(r)
}

/// A page of the player inside; the first sets the night.
fn test_inside(page: usize) -> FunctionBuilder {
    let (mut c, r) = report();
    let skip = only_if(&mut c, &r, true);
    if page == 0 {
        night(&mut c);
    } else {
        c.wait(5.0);
    }
    samples(&mut c, &r, "Inside", INSIDE_SAMPLES);
    c.bind(skip);
    c.finish(r)
}

fn test_out() -> FunctionBuilder {
    let (mut c, r) = report();
    let skip = only_if(&mut c, &r, true);
    let player = c.global("Game", "GetPlayer", []);
    move_to(&mut c, player, DOOR_OUT);
    c.wait(2.0);
    samples(&mut c, &r, "Inside", SAMPLES);
    c.bind(skip);
    c.finish(r)
}

fn scripts(crime_faction: u32, forced: Option<bool>) -> Result<Vec<(String, Vec<u8>)>> {
    let mut s = ScriptBuilder::new(SCRIPT, "Quest");
    s.variable("Ran", "Bool", false.into());
    s.variable("Inside", "Bool", false.into());
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
        .function("OnUpdate", on_update("autotest12", "4 minutes", TESTS))
        .function("Num", num())
        .function("Away", away_sample())
        .function("Inside", inside_sample(crime_faction))
        .function("TestStart", test_start(forced))
        .function("TestAway", test_away())
        .function("TestHome", test_home())
        .function("TestWake", test_wake())
        .function("TestOut", test_out());
    for page in 0..INSIDE_PAGES {
        s.function(&format!("TestInside{}", page + 1), test_inside(page));
    }
    Ok(vec![(SCRIPT.to_owned(), papyrus::pex::write(&s.build()?)?)])
}

/// The forms as this load order has them.
fn check(lo: &LoadOrder) -> Result<()> {
    let inside = lo.get(FormId(DOOR_IN)).context("shop door")?;
    ensure!(inside.get(b"XLOC").is_some(), "Arcadia's door unlocked");
    let to = inside
        .get(b"XTEL")
        .filter(|d| d.len() >= 4)
        .map(|d| inside.fid(FormId(u32::from_le_bytes(d[..4].try_into().unwrap()))));
    ensure!(
        to == Some(FormId(DOOR_OUT)),
        "Arcadia's door leads to {to:?}"
    );
    ensure!(
        lo.cell_of_ref(FormId(DOOR_IN)) == Some(FormId(SHOP)),
        "Arcadia's door isn't in her shop"
    );
    for (what, r) in [("Arcadia", ARCADIA), ("Riverwood door", RIVERWOOD_DOOR)] {
        ensure!(lo.get(FormId(r)).is_some(), "{what} missing");
    }
    Ok(())
}

pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    check(lo)?;
    let crime_faction = Vanilla { lo }.id("CrimeFactionWhiterun")?;
    let quests = vec![
        Quest {
            object_id: QUEST,
            edid: "VrmAutoTest12Quest",
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
        "Whether a home locks for the night with its owner away or the player inside: \
         Arcadia's shop door sampled by script across her sleep, reports saved.",
        NEXT_OBJECT_ID,
        &[Node::top(b"QUST", quests)],
    );
    Ok(TestMod {
        plugin,
        scripts: scripts(crime_faction.0, None)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_mods::own;
    use crate::test_mods::script::dry_run::run;

    fn reports(forced: Option<bool>) -> String {
        let shown = run(
            scripts(0x28170, forced).unwrap(),
            &[(own(QUEST).0, SCRIPT, Vec::new())],
            TESTS.len(),
        );
        assert_eq!(shown.boxes.len(), TESTS.len(), "every test reports");
        assert_eq!(shown.saved_at_box, Some(0), "saves before the boxes");
        for b in &shown.boxes {
            assert!(b.lines().count() <= 20, "{} lines:\n{b}", b.lines().count());
        }
        shown.boxes.join("\n")
    }

    #[test]
    fn the_away_run_reports() {
        // The dry run's player cell (None) isn't the shop's: away.
        let all = reports(None);
        for expected in [
            "running: Away (Arcadia sent to Riverwood)\n",
            "unlocked after the night: locked by the script\n",
            "  1 -- None None\n",
            "(not this run: for the player inside Arcadia's Cauldron)\n",
        ] {
            assert!(all.contains(expected), "{expected:?} not in the reports");
        }
    }

    #[test]
    fn the_inside_run_reports() {
        let all = reports(Some(true));
        for expected in [
            "running: Inside (the player in her shop)\n",
            // The dry run's player is None, so calls on it read None.
            "  1 -- detected None at 1 bounty 1 None\n",
            "(not this run: for the player away from Arcadia's Cauldron)\n",
        ] {
            assert!(all.contains(expected), "{expected:?} not in the reports");
        }
    }
}
