//! `test-mod autotest14`: where actors sent off while unloaded are when
//! their cell loads, and what fast travel, waiting and reloading do to them
//! (`plans/ai-actor-execution.md`, `known_ambiguities/actor-execution.md`).
//! autotest13 found that an unloaded actor outside the buffered cells stays
//! put while its package sends it somewhere in the same space. Here the
//! player goes to such actors, fast travels, waits and reloads a save, and
//! the actors are read from afar (unloaded) and as their cell loads.
//!
//! The actors are made as in autotest13: the same template NPC, a Travel
//! package to a far marker under `GetGlobalValue VrmAutoTest14Go<n> == 1`,
//! and a Travel package home below it. Each logs its events into its own
//! `ResultEvents` (autotest13's actor script). Far markers are 2500 to 3200
//! from home, about 35 s of walking, or in the next cell east. Every
//! reading gives the distance from the far marker and from home, the cell,
//! 3D and package.
//!
//! | actor          | stands in               | goes to                 | set off by |
//! | -------------- | ----------------------- | ----------------------- | ---------- |
//! | **Timed**      | the Palace of the Kings | (stays: a 90 s Fortify Health cast on it) | |
//! | **VisitEarly** | Tamriel -20, 0          | 2500-3200 away          | Go1        |
//! | **VisitEast**  | the same                | the next cell east      | Go1        |
//! | **VisitLate**  | the same                | 2500-3200 away          | Go2, 55 s after Go1 |
//! | **FerryWalk**  | Tamriel -37, 21         | 2500-3200 away          | Go3, then fast travel |
//! | **FerryEast**  | the same                | the next cell east      | Go3        |
//! | **WaitNear**   | Tamriel -28, -8 (where the player fast travels to) | 2500-3200 away | Go4, then the wait |
//! | **WaitWalk**   | Tamriel -30, 8          | 2500-3200 away          | Go4        |
//! | **ReloadWalk** | Tamriel -8, 18          | 2500-3200 away          | Go5, then the reload |
//! | **ReloadEast** | the same                | the next cell east      | Go5        |
//!
//! The run, a page each (`OnUpdate`, 5 s after the save loads):
//!
//! 1. **Effect**: the player moved into the palace, Timed given its
//!    effect, then the player moved back to where they were. Timed read
//!    every quarter second for 30 s: each change of its 3D or the effect,
//!    with the time since the player left.
//! 2. **Visit**: Go1 set, VisitEarly and VisitEast read from afar at 2 and
//!    30 s; Go2 at 55 s, VisitLate read at 60 s. Then the player moved to
//!    their cell, all three read as their 3D loads, and VisitEarly and
//!    VisitLate each second for 3 s after.
//! 3. **FastTravel**: Go3 set, the Ferry actors read; controls enabled and
//!    `Game.FastTravel` to a marker in Tamriel -28, -8 (an exterior, as the
//!    carriages' destinations are). Whether it arrived, the game hours and
//!    real seconds it took, the Ferry actors read from afar on arrival and
//!    10 s later. If it didn't arrive in 60 s the player is moved there.
//! 4. **FerryVisit**: the player moved to the Ferry actors' cell, both read
//!    as they load.
//! 5. **Wait**: the player back at the fast travel marker, Go4 set, then a
//!    box asks the user to **wait two hours** (any time over half an hour
//!    counts). The jump found, WaitNear's distance just before it, both
//!    actors read after it and 10 s later.
//! 6. **WaitVisit**: the player moved to WaitWalk's cell, it read as it
//!    loads.
//! 7. **Reload**: Go5 set, both actors read; a box asks the user to **load
//!    the save the game makes when the box closes**. The script resumes
//!    from that save: more than 15 s since the save means it was loaded.
//!    Both read from afar then and 10 s later. If no load comes in 3
//!    minutes the page says so.
//! 8. **ReloadVisit**: the player moved to their cell, both read as they
//!    load.
//!
//! | reading                                                  | means                                         |
//! | -------------------------------------------------------- | --------------------------------------------- |
//! | VisitEarly and VisitLate both on their markers (40 short) as they load | a cell's actors are put on their targets as it loads |
//! | VisitEarly on its marker, VisitLate about 400 along      | the time away is walked off as the cell loads |
//! | both at home, then walking                               | actors start from where they stood            |
//! | VisitEast in the next cell                               | (with the above) the same across cells        |
//! | the Ferry actors moved from afar after fast travel        | fast travel advances unloaded actors          |
//! | the Ferry actors only move as their cell loads           | fast travel does nothing until the cell loads |
//! | WaitNear on its marker after the wait                    | waiting advances loaded actors by the time    |
//! | WaitWalk moved from afar after the wait                  | waiting advances unloaded actors              |
//! | the Reload actors on their markers after the load        | loading a save places actors, as for actors new to a save |
//! | Timed's effect gone as its 3D goes                       | effects end on unloading                      |
//! | gone some seconds after                                  | effects run on a while, then end              |
//!
//! The plugin must be named `VrmAutoTest14.esp`. Load a save outdoors near
//! Riverwood, without the other test plugins, with a character not in
//! combat, and **stay at the keyboard**: the run takes about eight minutes
//! and twice asks for something (wait two hours with the Wait menu; load
//! the newest save). Don't save over the save it makes. As with autotest13,
//! don't keep playing with the plugin: it carries Skyrim.esm's records of
//! Tamriel, its persistent cell and the palace.
//!
//! Run in Skyrim SE (2026-10-25, once, the test character's usual save near
//! Riverwood):
//!
//! - **Visit**: every actor sent off was already 40 short of its far marker
//!   at the 2 s reading, unloaded: VisitEarly and VisitLate (2,700 and 3,200
//!   away) and VisitEast (4,100 away, in the next cell). `OnPackageEnd`
//!   followed 2.5 s after `OnPackageStart`. Loading their cell left them
//!   there (40 to 46 off, dropped to the ground); VisitEarly then stepped 4
//!   further, VisitLate not at all.
//! - **FastTravel**: it worked: 0.6 s, the clock 3.2 hours on, fast travel
//!   and its controls enabled. FerryWalk (2,700 away) had jumped to 40 short
//!   before it; FerryEast (7,600 away, 2,650 below, in the next cell) never
//!   moved: not with the package, the fast travel, nor as its cell loaded
//!   (found 3 from home).
//! - **Wait**: WaitNear, loaded, 3,230 from its marker just before the wait,
//!   was 1 from it after two hours (`OnPackageEnd` then). WaitWalk, unloaded,
//!   had jumped to 40 short before the wait and stayed there.
//! - **Effect**: no change in 30 s: Timed kept its 3D and its effect after
//!   the player left the palace for Riverwood. The palace logged
//!   `OnCellDetach` but no `OnUnload` until the player went to Tamriel -20, 0
//!   90 s later. (In autotest13 the palace's actors lost their 3D 0.2 s after
//!   the player left it for Tamriel -20, 0.)
//! - **Reload**: ReloadWalk and ReloadEast (2,900 and 3,300 away, 820 and
//!   560 above, ReloadEast in the next cell with `OnLocationChange`) were
//!   40 short of their markers 3 s after Go5, before the save. The script
//!   resumed 16 s after the save (just over the 15 s that marks a load):
//!   both unchanged then, 10 s later and as their cell loaded. Loading the
//!   save fired no package events for them.
//!
//! So an unloaded actor isn't walked: within seconds of its package
//! starting it is put 40 short of its target, or left where it is. Every
//! actor moved here and in autotest13 had its target within 4,100 and 830
//! of height; every one left had its target 4,950 or more away and 1,900 or
//! more above or below. Which decides isn't known.

use anyhow::{Context, Result};
use esp::writer::{AttachedScript, Node, PropertyValue, RecordBuilder, write_plugin};
use esp::{FormId, LoadOrder, record_flags};
use papyrus::build::{Arg, FunctionBuilder, ScriptBuilder, auto_var, id, string};
use papyrus::pex::op;

use super::autotest2::{Npc, Vanilla, line, put};
use super::autotest4::{health, now, poll, put_num, report};
use super::autotest6::ALWAYS;
use super::autotest8::{get_fn, is_none};
use super::autotest13::{WALK, actor_script, package};
use super::places::{self, Interior, Spots, World};
use super::records::{self, Effect, Quest};
use super::script::{Code, num, on_update_with};
use super::venv::placement;
use super::{TestMod, own};

// Object ids, as autotest13's.
const QUEST: u32 = 0x800;
const GLOBALS: u32 = 0x801;
const SPELL: u32 = 0x808;
const NPCS: u32 = 0x810;
const PACKAGES: u32 = 0x820;
const HOMES: u32 = 0x840;
const FARS: u32 = 0x850;
const ENTRIES: u32 = 0x860;
const ACTORS: u32 = 0x870;
const NEXT_OBJECT_ID: u32 = 0x880;

const PLUGIN: &str = "VrmAutoTest14.esp";
const SCRIPT: &str = "VrmAutoTest14Script";
const ACTOR_SCRIPT: &str = "VrmAutoTest14ActorScript";
const PLAYER_REF: FormId = FormId(0x14);

/// Where actors stand and markers go: Tamriel cells, and the palace.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Place {
    Visit,
    VisitEast,
    Ferry,
    FerryEast,
    /// Where the player fast travels to, and waits.
    Arrive,
    Waits,
    Reload,
    ReloadEast,
    Palace,
}

const PLACES: [Place; 9] = [
    Place::Visit,
    Place::VisitEast,
    Place::Ferry,
    Place::FerryEast,
    Place::Arrive,
    Place::Waits,
    Place::Reload,
    Place::ReloadEast,
    Place::Palace,
];

const PALACE: (&str, &str) = ("WindhelmPalaceoftheKings", "Palace of the Kings");

impl Place {
    fn index(self) -> usize {
        PLACES.iter().position(|p| *p == self).unwrap()
    }
    /// A Tamriel cell's grid.
    fn grid(self) -> Option<(i32, i32)> {
        Some(match self {
            Place::Visit => (-20, 0),
            Place::VisitEast => (-19, 0),
            Place::Ferry => (-37, 21),
            Place::FerryEast => (-36, 21),
            Place::Arrive => (-28, -8),
            Place::Waits => (-30, 8),
            Place::Reload => (-8, 18),
            Place::ReloadEast => (-7, 18),
            Place::Palace => return None,
        })
    }
}

/// Where an actor's far marker goes.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Far {
    None,
    /// A spot of another place.
    In(Place),
    /// Its own spot's pair.
    Pair,
}

struct Actor {
    label: &'static str,
    home: Place,
    far: Far,
    /// The global `Go<n>` sending it off (0: none).
    go: u32,
}

const fn actor(label: &'static str, home: Place, far: Far, go: u32) -> Actor {
    Actor {
        label,
        home,
        far,
        go,
    }
}

const ACTOR_LIST: &[Actor] = &[
    actor("Timed", Place::Palace, Far::None, 0),
    actor("VisitEarly", Place::Visit, Far::Pair, 1),
    actor("VisitEast", Place::Visit, Far::In(Place::VisitEast), 1),
    actor("VisitLate", Place::Visit, Far::Pair, 2),
    actor("FerryWalk", Place::Ferry, Far::Pair, 3),
    actor("FerryEast", Place::Ferry, Far::In(Place::FerryEast), 3),
    actor("WaitNear", Place::Arrive, Far::Pair, 4),
    actor("WaitWalk", Place::Waits, Far::Pair, 4),
    actor("ReloadWalk", Place::Reload, Far::Pair, 5),
    actor("ReloadEast", Place::Reload, Far::In(Place::ReloadEast), 5),
];

/// Globals `Go1` to `Go5`.
const GOES: u32 = 5;

/// Timed's effect: Fortify Health 50 for this many seconds.
const EFFECT_SECONDS: u32 = 90;

/// Seconds since the save that mean the script resumed from it after a
/// load (the live run takes 5).
const RELOADED_AFTER: f32 = 15.0;

const TESTS: &[(&str, &str)] = &[
    ("Effect", "Timed's effect as the palace unloads"),
    ("Visit", "Actors sent off unloaded, then visited"),
    ("FastTravel", "The Ferry actors sent off, then fast travel"),
    ("FerryVisit", "The Ferry actors visited"),
    ("Wait", "WaitNear and WaitWalk sent off, then a wait"),
    ("WaitVisit", "WaitWalk visited"),
    ("Reload", "The Reload actors sent off, then a reload"),
    ("ReloadVisit", "The Reload actors visited"),
];

/// The boxes asking the user to do something.
const WAIT_PROMPT: &str = "vibe_rm autotest14: open the Wait menu (T) now and wait two hours. \
     The test goes on by itself afterwards.";
const RELOAD_PROMPT: &str = "vibe_rm autotest14: when you close this box the game saves. Then \
     load that save (the newest) from the menu. The test goes on by itself after the load.";

fn k_of(label: &str) -> usize {
    ACTOR_LIST
        .iter()
        .position(|a| a.label == label)
        .unwrap_or_else(|| panic!("no actor {label}"))
}

/// `Where(Int n, ObjectReference home, ObjectReference far)`: our actor
/// `n`'s distance from its far marker (and height), from home, its cell,
/// 3D and package, or that the game has none.
pub(super) fn where_fn() -> FunctionBuilder {
    let mut c = Code::new("String");
    let n = c.param("n", "Int");
    let home = c.param("home", "ObjectReference");
    let far = c.param("far", "ObjectReference");
    let r = c.local("r", "String");
    c.assign(r.clone(), string(""));
    let a = c.own("Get", "Actor", [n]);
    let missing = is_none(&mut c, a.clone());
    let found = c.label();
    c.jmpf(missing, found)
        .ret(string("not in memory"))
        .bind(found);
    let df = c.method("ObjectReference", a.clone(), "GetDistance", [far.clone()]);
    let z = c.method("ObjectReference", a.clone(), "GetPositionZ", []);
    let fz = c.method("ObjectReference", far, "GetPositionZ", []);
    let dz = c.calc(op::FSUB, "Float", z, fz);
    let dh = c.method("ObjectReference", a.clone(), "GetDistance", [home]);
    c.numbers(&r, [("far ", df), (" (z ", dz), ("), home ", dh)]);
    let cell = c.method("ObjectReference", a.clone(), "GetParentCell", []);
    put(&mut c, &r, ", ", cell);
    let loaded = c.method("ObjectReference", a.clone(), "Is3DLoaded", []);
    put(&mut c, &r, ", 3D ", loaded);
    let package = c.method("Actor", a, "GetCurrentPackage", []);
    put(&mut c, &r, ", ", package);
    c.finish(r)
}

/// Actor `k` read into `r`, after `text`, a line.
fn put_where(c: &mut Code, r: &Arg, text: &str, k: usize) {
    let s = c.own(
        "Where",
        "String",
        [
            ((ACTORS + k as u32) as i32).into(),
            auto_var(&format!("Home{k}")),
            auto_var(&format!("Far{k}")),
        ],
    );
    put(c, r, text, s);
    line(c, r, "\n");
}

/// Each of `labels` read, a line each.
fn put_all(c: &mut Code, r: &Arg, labels: &[&str]) {
    for label in labels {
        put_where(c, r, &format!("  {label}: "), k_of(label));
    }
}

/// Actor `k`, looked up.
fn get(c: &mut Code, k: usize) -> Arg {
    c.own("Get", "Actor", [((ACTORS + k as u32) as i32).into()])
}

/// `body(c, a)` for actor `k` when the game has it.
fn with_actor(c: &mut Code, k: usize, body: impl FnOnce(&mut Code, Arg)) {
    let a = get(c, k);
    let missing = is_none(c, a.clone());
    let skip = c.label();
    c.jmpt(missing, skip);
    body(c, a);
    c.bind(skip);
}

/// Global `Go<n>` set to 1, and `EvaluatePackage` on the actors it sends.
fn set_off(c: &mut Code, n: u32) {
    c.method(
        "GlobalVariable",
        auto_var(&format!("Go{n}")),
        "SetValue",
        [1.0f32.into()],
    );
    for (k, a) in ACTOR_LIST.iter().enumerate() {
        if a.go == n {
            with_actor(c, k, |c, a| {
                c.method("Actor", a, "EvaluatePackage", []);
            });
        }
    }
}

/// The seconds since `T0`.
pub(super) fn since(c: &mut Code, t0: &str) -> Arg {
    let t = now(c);
    c.calc(op::FSUB, "Float", t, id(t0))
}

/// The game time in hours (not wrapping at midnight).
fn hours(c: &mut Code) -> Arg {
    let game = c.global("Utility", "GetCurrentGameTime", []);
    c.calc(op::FMUL, "Float", game, 24.0f32.into())
}

pub(super) fn move_player(c: &mut Code, to: Arg) {
    c.method(
        "ObjectReference",
        auto_var("Player"),
        "MoveTo",
        [to, 0.0f32.into(), 0.0f32.into(), 0.0f32.into(), true.into()],
    );
}

fn entry(p: Place) -> Arg {
    auto_var(&format!("Entry{}", p.index()))
}

/// The player moved to `p`, then up to 60 s for the 3D of every one of
/// `labels`; `r` gets how long that took.
fn visit(c: &mut Code, r: &Arg, p: Place, labels: &[&str]) {
    move_player(c, entry(p));
    let ks: Vec<usize> = labels.iter().map(|l| k_of(l)).collect();
    let waited = poll(c, "loaded", 0.2, 60.0, |c, hit| {
        let not_yet = c.label();
        for k in &ks {
            let a = get(c, *k);
            let missing = is_none(c, a.clone());
            c.jmpt(missing, not_yet);
            let l = c.method("ObjectReference", a, "Is3DLoaded", []);
            c.jmpf(l, not_yet);
        }
        c.jmp(hit);
        c.bind(not_yet);
    });
    put_num(c, r, "3D after ", waited);
    line(c, r, " s (-1: not in 60 s):\n");
}

/// `label`'s distance from its far marker each second for `seconds`.
fn read_walking(c: &mut Code, r: &Arg, label: &str, seconds: usize) {
    let k = k_of(label);
    line(
        c,
        r,
        &format!("  {label} from its far marker, each second:"),
    );
    for i in 0..=seconds {
        if i > 0 {
            c.wait(1.0);
        }
        with_actor(c, k, |c, a| {
            let d = c.method(
                "ObjectReference",
                a,
                "GetDistance",
                [auto_var(&format!("Far{k}"))],
            );
            put_num(c, r, " ", d);
        });
    }
    line(c, r, "\n");
}

/// Step 1: Timed's effect as the palace unloads. `Back` placed where the
/// player was.
fn test_effect() -> FunctionBuilder {
    let (mut c, r) = report();
    let back = c.method(
        "ObjectReference",
        auto_var("Player"),
        "PlaceAtMe",
        [auto_var("XMarker"), 1.into(), false.into(), false.into()],
    );
    c.assign(id("Back"), back);
    line(&mut c, &r, "in the palace; ");
    visit(&mut c, &r, Place::Palace, &["Timed"]);
    let timed = k_of("Timed");
    with_actor(&mut c, timed, |c, a| {
        c.method("Spell", auto_var("Spell"), "Cast", [a.clone(), a]);
    });
    c.wait(2.0);
    let (was_loaded, had) = (c.local("wasLoaded", "Bool"), c.local("had", "Bool"));
    c.assign(was_loaded.clone(), true.into());
    c.assign(had.clone(), false.into());
    with_actor(&mut c, timed, |c, a| {
        let has = c.method("Actor", a.clone(), "HasMagicEffect", [auto_var("Effect")]);
        c.assign(had.clone(), has);
        let hp = health(c, &a);
        put(c, &r, "  2 s after the cast: effect ", had.clone());
        put_num(c, &r, ", health ", hp);
        line(c, &r, "\n");
    });
    let start = now(&mut c);
    c.assign(id("TLeft"), start);
    move_player(&mut c, id("Back"));
    let moved = since(&mut c, "TLeft");
    put_num(&mut c, &r, "  the player moved out, taking ", moved);
    line(&mut c, &r, " s; then each change (s since leaving):\n");
    let ended = poll(&mut c, "ended", 0.25, 30.0, |c, hit| {
        with_actor(c, timed, |c, a| {
            let l = c.method("ObjectReference", a.clone(), "Is3DLoaded", []);
            let has = c.method("Actor", a.clone(), "HasMagicEffect", [auto_var("Effect")]);
            let (changed, same) = (c.label(), c.label());
            let same_l = c.calc(op::CMP_EQ, "Bool", l.clone(), was_loaded.clone());
            c.jmpf(same_l, changed);
            let same_e = c.calc(op::CMP_EQ, "Bool", has.clone(), had.clone());
            c.jmpf(same_e, changed);
            c.jmp(same);
            c.bind(changed);
            let t = since(c, "TLeft");
            put_num(c, &r, "  ", t);
            put(c, &r, ": 3D ", l.clone());
            put(c, &r, ", effect ", has.clone());
            let hp = health(c, &a);
            put_num(c, &r, ", health ", hp);
            line(c, &r, "\n");
            c.assign(was_loaded.clone(), l);
            c.assign(had.clone(), has.clone());
            c.jmpf(has, hit);
            c.bind(same);
        });
    });
    put_num(&mut c, &r, "  effect gone ", ended);
    line(&mut c, &r, " s after leaving (-1: not in 30 s)\n");
    c.finish(r)
}

/// Step 2: actors sent off unloaded, then visited.
fn test_visit() -> FunctionBuilder {
    let (mut c, r) = report();
    set_off(&mut c, 1);
    let start = now(&mut c);
    c.assign(id("T0"), start);
    for at in [2.0f32, 30.0] {
        let wait = if at == 2.0 { 2.0 } else { 28.0 };
        c.wait(wait);
        line(&mut c, &r, &format!("{at} s after Go1:\n"));
        put_all(&mut c, &r, &["VisitEarly", "VisitEast"]);
    }
    c.wait(25.0);
    set_off(&mut c, 2);
    c.wait(5.0);
    line(&mut c, &r, "5 s after Go2:\n");
    put_all(&mut c, &r, &["VisitLate"]);
    let t = since(&mut c, "T0");
    put_num(&mut c, &r, "moved there ", t);
    line(&mut c, &r, " s after Go1; ");
    let all = ["VisitEarly", "VisitEast", "VisitLate"];
    visit(&mut c, &r, Place::Visit, &all);
    put_all(&mut c, &r, &all);
    read_walking(&mut c, &r, "VisitEarly", 3);
    read_walking(&mut c, &r, "VisitLate", 3);
    c.finish(r)
}

const FERRY: [&str; 2] = ["FerryWalk", "FerryEast"];

/// Step 3: the Ferry actors sent off, then fast travel.
fn test_fast_travel() -> FunctionBuilder {
    let (mut c, r) = report();
    set_off(&mut c, 3);
    c.wait(3.0);
    line(&mut c, &r, "3 s after Go3:\n");
    put_all(&mut c, &r, &FERRY);
    c.enable_all();
    c.global("Game", "EnableFastTravel", [true.into()]);
    let enabled = c.global("Game", "IsFastTravelEnabled", []);
    put(&mut c, &r, "fast travel enabled ", enabled);
    let controls = c.global("Game", "IsFastTravelControlsEnabled", []);
    put(&mut c, &r, ", its controls ", controls);
    line(&mut c, &r, "\n");
    let g0 = c.local("g0", "Float");
    let h = hours(&mut c);
    c.assign(g0.clone(), h);
    let start = now(&mut c);
    c.assign(id("T0"), start);
    let there = entry(Place::Arrive);
    c.global("Game", "FastTravel", [there.clone()]);
    let arrived = poll(&mut c, "arrived", 0.5, 60.0, |c, hit| {
        let d = c.method(
            "ObjectReference",
            auto_var("Player"),
            "GetDistance",
            [there.clone()],
        );
        let near = c.calc(op::CMP_LT, "Bool", d, 2000.0f32.into());
        c.jmpt(near, hit);
    });
    let real = since(&mut c, "T0");
    let h = hours(&mut c);
    let passed = c.calc(op::FSUB, "Float", h, g0);
    put_num(&mut c, &r, "arrived after ", arrived.clone());
    put_num(
        &mut c,
        &r,
        " s (-1: not in 60 s); game hours passed ",
        passed,
    );
    put_num(&mut c, &r, ", real seconds ", real);
    line(&mut c, &r, "\n");
    let came = c.calc(op::CMP_GE, "Bool", arrived, 0.0f32.into());
    let ok = c.label();
    c.jmpt(came, ok);
    line(&mut c, &r, "NOT ARRIVED: moved there instead\n");
    move_player(&mut c, there);
    c.bind(ok);
    line(&mut c, &r, "on arrival:\n");
    put_all(&mut c, &r, &FERRY);
    c.wait(10.0);
    line(&mut c, &r, "10 s later:\n");
    put_all(&mut c, &r, &FERRY);
    c.finish(r)
}

/// Step 4: the Ferry actors visited.
fn test_ferry_visit() -> FunctionBuilder {
    let (mut c, r) = report();
    line(&mut c, &r, "at the Ferry actors' cell; ");
    visit(&mut c, &r, Place::Ferry, &FERRY);
    put_all(&mut c, &r, &FERRY);
    c.finish(r)
}

const WAITERS: [&str; 2] = ["WaitNear", "WaitWalk"];

/// Step 5: WaitNear and WaitWalk sent off, then the user waits.
fn test_wait() -> FunctionBuilder {
    let (mut c, r) = report();
    line(&mut c, &r, "at the fast travel marker; ");
    visit(&mut c, &r, Place::Arrive, &["WaitNear"]);
    set_off(&mut c, 4);
    c.wait(2.0);
    line(&mut c, &r, "2 s after Go4:\n");
    put_all(&mut c, &r, &WAITERS);
    c.global("Debug", "MessageBox", [string(WAIT_PROMPT)]);
    let near = k_of("WaitNear");
    let (last_g, last_d) = (c.local("lastG", "Float"), c.local("lastD", "Float"));
    let h = hours(&mut c);
    c.assign(last_g.clone(), h);
    c.assign(last_d.clone(), (-1.0f32).into());
    let waited = poll(&mut c, "waited", 0.5, 240.0, |c, hit| {
        let h = hours(c);
        let dh = c.calc(op::FSUB, "Float", h.clone(), last_g.clone());
        let jumped = c.calc(op::CMP_GT, "Bool", dh, 0.5f32.into());
        c.jmpt(jumped, hit);
        c.assign(last_g.clone(), h);
        with_actor(c, near, |c, a| {
            let d = c.method(
                "ObjectReference",
                a,
                "GetDistance",
                [auto_var(&format!("Far{near}"))],
            );
            c.assign(last_d.clone(), d);
        });
    });
    let h = hours(&mut c);
    let jump = c.calc(op::FSUB, "Float", h, last_g);
    put_num(&mut c, &r, "the wait seen after ", waited);
    put_num(&mut c, &r, " s (-1: not in 240 s), the clock moved ", jump);
    line(&mut c, &r, " hours\n");
    put_num(&mut c, &r, "  WaitNear just before: far ", last_d);
    line(&mut c, &r, "\nafter the wait:\n");
    put_all(&mut c, &r, &WAITERS);
    c.wait(10.0);
    line(&mut c, &r, "10 s later:\n");
    put_all(&mut c, &r, &WAITERS);
    c.finish(r)
}

/// Step 6: WaitWalk visited.
fn test_wait_visit() -> FunctionBuilder {
    let (mut c, r) = report();
    line(&mut c, &r, "at WaitWalk's cell; ");
    visit(&mut c, &r, Place::Waits, &["WaitWalk"]);
    put_all(&mut c, &r, &["WaitWalk"]);
    c.finish(r)
}

const RELOADERS: [&str; 2] = ["ReloadWalk", "ReloadEast"];

/// Step 7: the Reload actors sent off, a save, and the user loads it. The
/// script carries on in the loaded game from just after the save.
fn test_reload() -> FunctionBuilder {
    let (mut c, r) = report();
    set_off(&mut c, 5);
    c.wait(3.0);
    line(&mut c, &r, "3 s after Go5:\n");
    put_all(&mut c, &r, &RELOADERS);
    c.global("Debug", "MessageBox", [string(RELOAD_PROMPT)]);
    // Waits only run once the box is closed.
    c.wait(1.0);
    let start = now(&mut c);
    c.assign(id("TSave"), start);
    c.global("Game", "RequestSave", []);
    c.wait(5.0);
    let t = since(&mut c, "TSave");
    let loaded = c.calc(op::CMP_GT, "Bool", t.clone(), RELOADED_AFTER.into());
    let (resumed, done) = (c.label(), c.label());
    c.jmpt(loaded, resumed);
    // The live run: the user's load replaces it. Wait for that, then say
    // it never came.
    c.wait(180.0);
    line(&mut c, &r, "NOT RELOADED: no load in 3 minutes\n");
    put_all(&mut c, &r, &RELOADERS);
    c.jmp(done);
    c.bind(resumed);
    put_num(&mut c, &r, "resumed after a load, ", t);
    line(&mut c, &r, " s after the save:\n");
    put_all(&mut c, &r, &RELOADERS);
    c.wait(10.0);
    line(&mut c, &r, "10 s later:\n");
    put_all(&mut c, &r, &RELOADERS);
    c.bind(done);
    c.finish(r)
}

/// Step 8: the Reload actors visited.
fn test_reload_visit() -> FunctionBuilder {
    let (mut c, r) = report();
    line(&mut c, &r, "at the Reload actors' cell; ");
    visit(&mut c, &r, Place::Reload, &RELOADERS);
    put_all(&mut c, &r, &RELOADERS);
    c.finish(r)
}

fn scripts() -> Result<Vec<(String, Vec<u8>)>> {
    let mut s = ScriptBuilder::new(SCRIPT, "Quest");
    s.auto_property("Player", "Actor");
    s.auto_property("XMarker", "Form");
    s.auto_property("Spell", "Spell");
    s.auto_property("Effect", "MagicEffect");
    for n in 1..=GOES {
        s.auto_property(&format!("Go{n}"), "GlobalVariable");
    }
    for p in PLACES {
        s.auto_property(&format!("Entry{}", p.index()), "ObjectReference");
    }
    for (k, a) in ACTOR_LIST.iter().enumerate() {
        s.auto_property(&format!("Home{k}"), "ObjectReference");
        if a.far != Far::None {
            s.auto_property(&format!("Far{k}"), "ObjectReference");
        }
    }
    s.variable("Ran", "Bool", false.into());
    s.variable("Back", "ObjectReference", Arg::None);
    for t in ["T0", "TLeft", "TSave"] {
        s.variable(t, "Float", 0.0f32.into());
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
        .function(
            "OnUpdate",
            on_update_with(
                "autotest14",
                "vibe_rm autotest14 running: about eight minutes. Stay at the keyboard: it will \
                 ask you to wait two hours, and to load a save.",
                TESTS,
            ),
        )
        .function("Num", num())
        .function("Get", get_fn(PLUGIN))
        .function("Where", where_fn())
        .function("TestEffect", test_effect())
        .function("TestVisit", test_visit())
        .function("TestFastTravel", test_fast_travel())
        .function("TestFerryVisit", test_ferry_visit())
        .function("TestWait", test_wait())
        .function("TestWaitVisit", test_wait_visit())
        .function("TestReload", test_reload())
        .function("TestReloadVisit", test_reload_visit());
    let actors = actor_script(ACTOR_SCRIPT)?;
    Ok(vec![
        (SCRIPT.to_owned(), papyrus::pex::write(&s.build()?)?),
        (
            ACTOR_SCRIPT.to_owned(),
            papyrus::pex::write(&actors.build()?)?,
        ),
    ])
}

/// The positions: each actor's spot and far marker, each place's entry,
/// and each place's cell.
struct Layout {
    cells: Vec<FormId>,
    entries: Vec<[f32; 3]>,
    homes: Vec<[f32; 3]>,
    fars: Vec<Option<[f32; 3]>>,
}

fn layout(lo: &LoadOrder, v: &Vanilla) -> Result<Layout> {
    let tamriel = v.id("Tamriel")?;
    let cells = PLACES
        .iter()
        .map(|p| match p.grid() {
            Some(grid) => lo
                .world(tamriel)
                .and_then(|w| w.cells.get(&grid))
                .copied()
                .with_context(|| format!("no Tamriel cell at {grid:?}")),
            None => v.id(PALACE.0),
        })
        .collect::<Result<Vec<_>>>()?;
    // Spots each place needs: one per actor standing there (with its pair
    // in reach), one per far marker of an actor elsewhere.
    let mut spots: Vec<Spots> = Vec::new();
    for (i, p) in PLACES.iter().enumerate() {
        let homes = ACTOR_LIST.iter().filter(|a| a.home == *p).count();
        let visitors = ACTOR_LIST.iter().filter(|a| a.far == Far::In(*p)).count();
        let (reach, apart) = match p {
            Place::Palace => ((150.0, 400.0), 100.0),
            Place::VisitEast | Place::FerryEast | Place::ReloadEast => ((250.0, 450.0), 120.0),
            _ => ((2500.0, 3200.0), 150.0),
        };
        let s = places::spots(lo, cells[i], homes + visitors, reach, apart)
            .with_context(|| format!("spots in {p:?}"))?;
        spots.push(s);
    }
    let mut used = vec![0usize; PLACES.len()];
    let mut take = |p: Place| {
        let i = p.index();
        let pair = spots[i].pairs[used[i]];
        used[i] += 1;
        pair
    };
    let mut homes = Vec::new();
    let mut pairs = Vec::new();
    for a in ACTOR_LIST {
        let (spot, pair) = take(a.home);
        homes.push(spot);
        pairs.push(pair);
    }
    let fars = ACTOR_LIST
        .iter()
        .zip(pairs)
        .map(|(a, pair)| match a.far {
            Far::None => None,
            Far::Pair => Some(pair),
            Far::In(p) => Some(take(p).0),
        })
        .collect();
    Ok(Layout {
        cells,
        entries: spots.iter().map(|s| s.entry).collect(),
        homes,
        fars,
    })
}

pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    let v = Vanilla { lo };
    let template = v.id("WEMerchantMiscMaleCommoner")?;
    let race = v.id("ImperialRace")?;
    let class = v.id("Citizen")?;
    let heading_marker = v.id("XMarkerHeading")?;
    let effect = v.id("AlchFortifyHealth")?;
    let tamriel = v.id("Tamriel")?;
    let persistent = record_flags::PERSISTENT;
    let at = layout(lo, &v)?;

    let object = PropertyValue::Object;
    let mut properties = vec![
        ("Player".to_owned(), object(PLAYER_REF)),
        ("XMarker".to_owned(), object(v.id("XMarker")?)),
        ("Spell".to_owned(), object(own(SPELL))),
        ("Effect".to_owned(), object(effect)),
    ];
    for n in 1..=GOES {
        properties.push((format!("Go{n}"), object(own(GLOBALS + n - 1))));
    }
    for p in PLACES {
        let i = p.index();
        properties.push((format!("Entry{i}"), object(own(ENTRIES + i as u32))));
    }
    for (k, a) in ACTOR_LIST.iter().enumerate() {
        properties.push((format!("Home{k}"), object(own(HOMES + k as u32))));
        if a.far != Far::None {
            properties.push((format!("Far{k}"), object(own(FARS + k as u32))));
        }
    }

    let globals: Vec<RecordBuilder> = (1..=GOES)
        .map(|n| records::global(GLOBALS + n - 1, &format!("VrmAutoTest14Go{n}"), 0.0))
        .collect();
    let spell = records::spell(
        SPELL,
        "VrmAutoTest14Fortify",
        records::spell::SPELL,
        records::spell::FIRE_AND_FORGET,
        records::spell::SELF,
        0,
        None,
        &[Effect::new(effect, 50.0, EFFECT_SECONDS)],
    );

    // Two packages each: the far one first (once its global is set), then
    // staying home.
    let mut packages = Vec::new();
    for (k, a) in ACTOR_LIST.iter().enumerate() {
        let far_id = PACKAGES + 2 * k as u32;
        if a.far != Far::None {
            packages.push(package(
                lo,
                far_id,
                &format!("VrmAutoTest14{}Far", a.label),
                own(FARS + k as u32),
                ALWAYS,
                Some(own(GLOBALS + a.go - 1)),
                WALK,
            )?);
        }
        packages.push(package(
            lo,
            far_id + 1,
            &format!("VrmAutoTest14{}Home", a.label),
            own(HOMES + k as u32),
            ALWAYS,
            None,
            WALK,
        )?);
    }

    let actor_script = AttachedScript {
        name: ACTOR_SCRIPT.into(),
        properties: Vec::new(),
    };
    let npcs: Vec<RecordBuilder> = ACTOR_LIST
        .iter()
        .enumerate()
        .map(|(k, a)| {
            let far_id = PACKAGES + 2 * k as u32;
            let mut pkids = Vec::new();
            if a.far != Far::None {
                pkids.push(own(far_id));
            }
            pkids.push(own(far_id + 1));
            Npc {
                object_id: NPCS + k as u32,
                edid: format!("VrmAutoTest14{}Npc", a.label),
                name: Some("vibe_rm Test NPC"),
                level: (10, false, 1, 0),
                template: (template, 0x1),
                temper: (0, 0),
                factions: Vec::new(),
                script: Some(actor_script.clone()),
                assistance: 0,
                combat_style: None,
                unique: false,
            }
            .record_with_packages(race, class, &pkids)
        })
        .collect();

    let quests = vec![
        Quest {
            object_id: QUEST,
            edid: "VrmAutoTest14Quest",
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
        .record(),
    ];

    // Every reference persistent: entries, markers and actors (autotest8
    // and autotest9 found persistence makes no difference to unloaded
    // movement), Tamriel's all in its persistent cell.
    let marker = |id: FormId, at: [f32; 3]| {
        RecordBuilder::new(b"REFR", id)
            .flags(persistent)
            .form(b"NAME", heading_marker)
            .field(b"DATA", &placement(at, 0.0))
    };
    let mut by_place: Vec<Vec<RecordBuilder>> = PLACES.iter().map(|_| Vec::new()).collect();
    for p in PLACES {
        let i = p.index();
        by_place[i].push(marker(own(ENTRIES + i as u32), at.entries[i]));
    }
    for (k, a) in ACTOR_LIST.iter().enumerate() {
        let home = a.home.index();
        by_place[home].push(marker(own(HOMES + k as u32), at.homes[k]));
        if let Some(far) = at.fars[k] {
            let place = match a.far {
                Far::In(p) => p.index(),
                _ => home,
            };
            by_place[place].push(marker(own(FARS + k as u32), far));
        }
        by_place[home].push(
            RecordBuilder::new(b"ACHR", own(ACTORS + k as u32))
                .flags(persistent)
                .form(b"NAME", own(NPCS + k as u32))
                .field(b"DATA", &placement(at.homes[k], 0.0)),
        );
    }

    let mut interiors = Vec::new();
    let mut tamriel_persistent = Vec::new();
    for (p, refs) in PLACES.into_iter().zip(by_place) {
        match p.grid() {
            None => interiors.push(Interior {
                cell: at.cells[p.index()],
                name: PALACE.1,
                persistent: refs,
                temporary: Vec::new(),
            }),
            Some(_) => tamriel_persistent.extend(refs),
        }
    }
    let worlds = vec![World {
        world: tamriel,
        name: "Skyrim",
        grid: Place::Visit.grid().unwrap(),
        persistent: tamriel_persistent,
        temporary: Vec::new(),
    }];

    // Top groups in the Creation Kit's order.
    let plugin = write_plugin(
        0,
        &["Skyrim.esm"],
        "vibe_rm",
        "Where actors sent off while unloaded are when their cell loads, and what fast travel, \
         waiting and reloading a save do to them; a timed effect as its actor unloads. Read by \
         script, with the user waiting and reloading when asked; reports saved.",
        NEXT_OBJECT_ID,
        &[
            Node::top(b"GLOB", globals),
            Node::top(b"SPEL", vec![spell]),
            Node::top(b"NPC_", npcs),
            places::interiors(lo, interiors)?,
            places::worlds(lo, worlds)?,
            Node::top(b"QUST", quests),
            Node::top(b"PACK", packages),
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

    fn properties() -> Vec<(String, papyrus::Value)> {
        let mut properties = vec![
            ("Player".to_owned(), form(0x14, "Actor")),
            ("XMarker".to_owned(), form(0x3B, "Form")),
            ("Spell".to_owned(), form(own(SPELL).0, "Spell")),
            ("Effect".to_owned(), form(0x3EAF3, "MagicEffect")),
        ];
        for n in 1..=GOES {
            properties.push((
                format!("Go{n}"),
                form(own(GLOBALS + n - 1).0, "GlobalVariable"),
            ));
        }
        for p in PLACES {
            properties.push((
                format!("Entry{}", p.index()),
                form(own(ENTRIES + p.index() as u32).0, "ObjectReference"),
            ));
        }
        for (k, a) in ACTOR_LIST.iter().enumerate() {
            properties.push((
                format!("Home{k}"),
                form(own(HOMES + k as u32).0, "ObjectReference"),
            ));
            if a.far != Far::None {
                properties.push((
                    format!("Far{k}"),
                    form(own(FARS + k as u32).0, "ObjectReference"),
                ));
            }
        }
        properties
    }

    #[test]
    fn the_tests_run_and_report() {
        // The two prompts, then a box per test.
        let boxes = TESTS.len() + 2;
        let shown = run(
            scripts().unwrap(),
            &[(own(QUEST).0, SCRIPT, properties())],
            boxes,
        );
        assert_eq!(shown.boxes.len(), boxes, "every test reports");
        assert_eq!(shown.boxes[0], WAIT_PROMPT);
        assert_eq!(shown.boxes[1], RELOAD_PROMPT);
        assert_eq!(shown.saved_at_box, Some(2), "saves before the reports");
        let all = shown.boxes.join("\n");
        for expected in [
            "in the palace; 3D after -1 s (-1: not in 60 s):\n",
            "  2 s after the cast: effect False, health 1\n",
            "  effect gone ",
            "30 s after Go1:\n  VisitEarly: far 1 (z 0), home 1, ",
            "  VisitLate from its far marker, each second: 1 1 1 1\n",
            "fast travel enabled False, its controls False\n",
            "arrived after 0.500 s (-1: not in 60 s); game hours passed ",
            "  WaitNear just before: far 1\n",
            "NOT RELOADED: no load in 3 minutes\n",
            "at the Reload actors' cell; 3D after ",
        ] {
            assert!(all.contains(expected), "{expected:?} in\n{all}");
        }
        for b in &shown.boxes[2..] {
            let lines = b.lines().count();
            assert!(lines <= 20, "{lines} lines:\n{b}");
        }
    }

    #[test]
    fn object_ids_fit() {
        let n = ACTOR_LIST.len() as u32;
        const { assert!(GLOBALS + GOES <= SPELL) };
        assert!(NPCS + n <= PACKAGES);
        assert!(PACKAGES + 2 * n <= HOMES);
        assert!(HOMES + n <= FARS);
        assert!(FARS + n <= ENTRIES);
        assert!(ENTRIES + PLACES.len() as u32 <= ACTORS);
        assert!(ACTORS + n <= NEXT_OBJECT_ID);
    }

    #[test]
    fn every_go_sends_someone() {
        for n in 1..=GOES {
            assert!(ACTOR_LIST.iter().any(|a| a.go == n), "Go{n}");
        }
        for a in ACTOR_LIST {
            assert_eq!(a.go == 0, a.far == Far::None, "{}", a.label);
        }
    }
}
