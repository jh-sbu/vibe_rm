//! `test-mod autotest13`: what actors do while the player is away
//! (`plans/ai-actor-execution.md` step 1, `known_ambiguities/actor-execution.md`).
//! autotest6 to autotest9 measured where an actor's package has it when its
//! cell loads, for actors new to a save. Here packages change during the run,
//! so what the readings show is the actor's own progress while unloaded
//! rather than the placement the game gives actors as a save loads.
//!
//! Every actor is ours: the same template NPC, a Travel package (a copy of
//! `MS13LucanHoldPositionPackage`, radius 0) to a far marker that applies once
//! its trigger is set, and below it a Travel package to a marker on the spot
//! where it stands. Triggers are globals of ours (`VrmAutoTest13Go<n>`, the
//! far package's condition `GetGlobalValue == 1`) or, for **Scheduled**, the
//! hour (the far package runs 12:00 to 24:00). Every actor but
//! **LongControl** carries a script that keeps a log of its events
//! (`OnPackageStart` / `Change` / `End`, `OnLoad` / `OnUnload`,
//! `OnCellAttach` / `Detach`, `OnAttachedToCell` / `OnDetachedFromCell`,
//! `OnLocationChange`, `OnDeath`, `OnMagicEffectApply`), each with the real
//! time and the game time in hours, in its own `ResultEvents`.
//! `vrm-tool test-results` prints those beside the reports (not shown in
//! the game: too long for a message box). Packages print as their form id:
//! actor `k`'s far package is `0x820 + 2k`, its home package `0x821 + 2k`.
//!
//! | actor            | stands in                         | goes to                       | set off by           |
//! | ---------------- | --------------------------------- | ----------------------------- | -------------------- |
//! | **LongWalk**     | Tamriel -37, 21 (west of Solitude) | the next cell east, walking   | Go1, `EvaluatePackage` |
//! | **LongRun**      | the same                          | the same, running             | Go1, `EvaluatePackage` |
//! | **LongNoEval**   | the same                          | the same, walking             | Go1 only             |
//! | **LongControl**  | the same (not persistent, no script) | the same, walking          | Go1, `EvaluatePackage` |
//! | **DoorOut**      | Kraldar's House, Winterhold       | outside, past its front door  | Go1, `EvaluatePackage` |
//! | **DoorIn**       | outside Kraldar's House            | inside it                     | Go1, `EvaluatePackage` |
//! | **HurtAway**     | Tamriel -37, 21                   | (stays: half its health taken while unloaded) | |
//! | **KilledAway**   | the same                          | (stays: all its health taken while unloaded)  | |
//! | **InsideWalker** | the Palace of the Kings, Windhelm | across the hall               | Go2, while watched   |
//! | **Hurt**         | the same                          | (stays: half its health taken while watched)  | |
//! | **Timed**        | the same                          | (stays: a 90 s Fortify Health cast on it while watched) | |
//! | **Watched**      | Tamriel -20, 0                    | 2500-3200 away                | Go3, while watched   |
//! | **Scheduled**    | Tamriel -37, 21                   | 600-1000 away                 | the clock past 12:00 |
//! | **Ferry**        | Tamriel -37, 21                   | the next cell east            | Go4, then fast travel |
//!
//! Every actor is flagged persistent except **LongControl**, which
//! autotest8 and autotest9 found makes no difference to unloaded movement.
//! The script finds the actors with `Game.GetFormFromFile` and holds no
//! property naming one. No quest alias holds one.
//!
//! The run, a page each (`OnUpdate`, 5 s after the save loads):
//!
//! 1. **Start**: Go1 set; HurtAway and KilledAway damaged. Then 90 s of
//!    sampling every half second, the player where the save was. For each
//!    walker: how often its position changed, when it first and last
//!    moved, the path it covered and its largest step between samples
//!    (to show whether it moves smoothly, in steps, or jumps), and where it
//!    was 5, 15, 30, 60 and 90 s in: distance and height from its far
//!    marker, its cell, 3D and package. Pages **Walk**, **NoEval**,
//!    **Doors**, **HurtAway**.
//! 2. **Inside**: the player moved into the palace. Hurt is damaged, Timed
//!    is given its effect, Go2 is set and InsideWalker read every second for
//!    5 s (its walking speed while loaded). **Watched**: the same outdoors
//!    with Go3. **Away**: the player is moved to Ysolda's House (Whiterun)
//!    for 30 s, everyone read at 0, 10, 20 and 30 s. **Return**: back
//!    outdoors (Watched read as its 3D loads), then into the palace
//!    (InsideWalker, Hurt, Timed), then Timed watched until its effect
//!    ends. The palace was the last interior the player left, so it may
//!    still be in the game's interior cell buffer.
//! 3. **Schedule**: the clock set to 11:57 and Scheduled read until 60 s
//!    after it passes 12:00.
//! 4. **FastTravel**: Go4 set, then the player fast travelled
//!    (`Game.FastTravel`) to Ysolda's House, the game time it took, and
//!    Ferry, LongWalk and Scheduled read on arrival and 10 s later.
//!
//! | reading                                                    | means                                              |
//! | ---------------------------------------------------------- | -------------------------------------------------- |
//! | walkers' positions change every sample, steady speed        | unloaded actors move continuously                  |
//! | changes every few seconds, steps of the same size          | unloaded actors are updated at a slower cadence (the step / interval is their speed) |
//! | one jump to the far marker (40 short)                      | unloaded travel is resolved at once                |
//! | LongNoEval stays where LongWalk moves                       | package conditions aren't re-read while unloaded until something asks |
//! | LongNoEval sets off later, on the hour                      | unloaded actors' packages are re-read hourly (`MiddleLowProcessData::hourPackageEvaluated`) |
//! | DoorOut's cell changes from Kraldar's House to Tamriel      | unloaded actors go through load doors              |
//! | KilledAway dead while unloaded, `OnDeath` logged then       | damage kills unloaded actors at once               |
//! | HurtAway's health rises while unloaded                      | unloaded actors heal                               |
//! | Watched on return as far along as 30 s of walking puts it   | actors left behind keep walking at their speed     |
//! | Watched where it was left                                   | actors stop when the player leaves                 |
//! | InsideWalker stopped but Watched moved                      | the buffered interior isn't processed              |
//! | Timed's effect ends 90 s after the cast                     | timed effects run while the actor is away          |
//! | Timed's effect ends later by the time away                  | timed effects stop while the actor is away         |
//! | Scheduled's package changes within seconds of 12:00         | unloaded actors' schedules are read as the hour passes |
//! | after fast travel Ferry is as far along as the game time passed allows | catch-up follows game time            |
//!
//! The plugin must be named `VrmAutoTest13.esp` (the script finds its
//! actors by that name). Load a save outdoors near Riverwood, without the
//! other test plugins, with a character not in combat and allowed to fast
//! travel, then leave the game alone for about six minutes. The player is
//! moved to Windhelm, outdoors, to Whiterun and back, with loading screens,
//! and the clock is set to 11:57. Don't keep playing with the plugin: it
//! carries Skyrim.esm's records of Tamriel, its persistent cell and the
//! cells the actors stand in (without later plugins' changes to them).
//!
//! Run in Skyrim SE (2026-10-25, once, the test character's usual save near
//! Riverwood):
//!
//! - **Walk**, **NoEval**: none of the four long walkers moved in 90 s,
//!   walking or running, with `EvaluatePackage` or without, though each
//!   changed to its far package (`OnPackageChange`, then `OnPackageStart`
//!   0.08 s later) within 1.3 s of Go1 (LongNoEval 2.6 s). They were still
//!   where they stood at the FastTravel page, 3.5 minutes later, the clock
//!   having passed two hours (13:00, then 12:00 after it was set back).
//! - **Doors**: DoorOut and DoorIn were moved in one step, by 0.5 s, exactly
//!   onto their markers (0 off) in the other cell, with `OnLocationChange`;
//!   `OnPackageEnd` followed 2.6 s later.
//! - **HurtAway**: health 25 of 50 throughout (NPCs' `HealRate` is 0).
//!   **KilledAway** died at once: dead at the next reading, `OnDeath` 1.2 s
//!   after the damage, while unloaded.
//! - **Inside**, **Watched**: loaded, InsideWalker walked 55 to 70 a second,
//!   Watched 60 to 85. **Away**: Watched's exterior stayed loaded while the
//!   player was in Ysolda's House (3D, `OnCellDetach` but no `OnUnload`) and
//!   it walked on at 87 a second, arriving 22 short of its marker.
//!   InsideWalker, in the palace the player had just left, had no 3D but
//!   walked on (811 to 346 from its marker in the 6 s before the first
//!   reading, about 78 a second), and by 10 s was exactly on its marker.
//! - **Timed**: the 90 s Fortify Health was already gone at the first Away
//!   reading, 12 s after the cast and 6 s after the palace unloaded (health
//!   back to 50). Every load logs `OnMagicEffectApply` for the template's
//!   racial ability (`RaceImperialLuckEffect`), so active effects seem to be
//!   dropped on unload and abilities applied again on load.
//! - **Return**: Watched 29 short of its marker, InsideWalker on its marker
//!   (dropped 7 to the floor).
//! - **Schedule**: setting the clock to 11:57 changed Scheduled to its home
//!   package at once, and back to the far one at 11:59:42 game time. It had
//!   no 3D and didn't move. It stood 44 from its far marker throughout, put
//!   there as the save loaded.
//! - **FastTravel**: didn't happen. `Game.FastTravel` to the entry marker in
//!   Ysolda's House left the player where they were (0.36 game hours over
//!   62 s, the normal rate), so the page answers nothing.
//! - Events: unloaded actors get `OnPackageStart` / `Change` / `End`. As the
//!   save loaded, every actor logged `OnPackageStart` then `OnPackageEnd`
//!   0.2 s later. Loading logs `OnCellAttach`, `OnMagicEffectApply` (the
//!   ability), then `OnLoad`; unloading logs `OnCellDetach`, then `OnUnload`
//!   0.2 s later. No `OnAttachedToCell` / `OnDetachedFromCell`. Dead
//!   KilledAway logged an `OnPackageStart` at the FastTravel page.
//!
//! So an actor outside the loaded and buffered cells doesn't walk: its
//! packages are re-read within seconds and its events fire, but it stays put,
//! except that a target in another cell moves it there at once. An actor in
//! a buffered cell keeps walking at about its loaded speed, 3D or not.
//! Where a frozen walker is when its cell loads, and what fast travel, waiting
//! and reloading do: `known_ambiguities/actor-execution.md`.

use anyhow::{Context, Result};
use esp::writer::{AttachedScript, Node, PropertyValue, RecordBuilder, write_plugin};
use esp::{FormId, LoadOrder, record_flags};
use papyrus::build::{Arg, FunctionBuilder, ScriptBuilder, auto_var, id, string};
use papyrus::pex::op;

use super::autotest2::{Npc, Vanilla, line, put};
use super::autotest4::{now, poll, put_num, report};
use super::autotest6::{ALWAYS, pldt};
use super::autotest8::{get_fn, is_none};
use super::autotest11::set_hour;
use super::places::{self, Interior, Spots, World};
use super::records::{self, Effect, Quest};
use super::script::{Code, num, on_update};
use super::venv::placement;
use super::{TestMod, own};

// Object ids: the quest, the globals, the spell, then by actor: NPCs, two
// packages each, home and far markers, entries (by place), actors.
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

const PLUGIN: &str = "VrmAutoTest13.esp";
const SCRIPT: &str = "VrmAutoTest13Script";
const ACTOR_SCRIPT: &str = "VrmAutoTest13ActorScript";
const PLAYER_REF: FormId = FormId(0x14);

/// Tamriel cells: where the long walkers stand, where they go (the next
/// cell east), and where Watched walks while the player looks on.
const WEST: (i32, i32) = (-37, 21);
const EAST: (i32, i32) = (-36, 21);
const OUTDOORS: (i32, i32) = (-20, 0);
/// The interiors, by editor id and name.
const KRALDAR: (&str, &str) = ("WinterholdKorirshouse", "Kraldar's House");
const PALACE: (&str, &str) = ("WindhelmPalaceoftheKings", "Palace of the Kings");
const YSOLDA: (&str, &str) = ("WhiterunYsoldasHouse", "Ysolda's House");

/// Where actors stand and markers go.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Place {
    West,
    East,
    Kraldar,
    /// The Tamriel cell outside Kraldar's House's front door.
    Winterhold,
    Palace,
    Outdoors,
    Ysolda,
}

const PLACES: [Place; 7] = [
    Place::West,
    Place::East,
    Place::Kraldar,
    Place::Winterhold,
    Place::Palace,
    Place::Outdoors,
    Place::Ysolda,
];

impl Place {
    fn index(self) -> usize {
        PLACES.iter().position(|p| *p == self).unwrap()
    }
    fn interior(self) -> Option<(&'static str, &'static str)> {
        match self {
            Place::Kraldar => Some(KRALDAR),
            Place::Palace => Some(PALACE),
            Place::Ysolda => Some(YSOLDA),
            _ => None,
        }
    }
}

/// What sends an actor to its far marker.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Trigger {
    /// Global `VrmAutoTest13Go<n>` set to 1.
    Go(u32),
    /// The hour from 12:00.
    Noon,
    /// Nothing: it stays.
    Never,
}

/// Where an actor's far marker goes.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Far {
    None,
    /// A spot of another place.
    In(Place),
    /// Its own spot's pair, in reach.
    Pair,
}

struct Actor {
    label: &'static str,
    home: Place,
    far: Far,
    trigger: Trigger,
    run: bool,
    /// `EvaluatePackage` called as its trigger is set.
    evaluate: bool,
    scripted: bool,
    persistent: bool,
}

const fn actor(label: &'static str, home: Place, far: Far, trigger: Trigger) -> Actor {
    Actor {
        label,
        home,
        far,
        trigger,
        run: false,
        evaluate: true,
        scripted: true,
        persistent: true,
    }
}

const ACTOR_LIST: &[Actor] = &[
    actor(
        "LongWalk",
        Place::West,
        Far::In(Place::East),
        Trigger::Go(1),
    ),
    Actor {
        run: true,
        ..actor("LongRun", Place::West, Far::In(Place::East), Trigger::Go(1))
    },
    Actor {
        evaluate: false,
        ..actor(
            "LongNoEval",
            Place::West,
            Far::In(Place::East),
            Trigger::Go(1),
        )
    },
    Actor {
        scripted: false,
        persistent: false,
        ..actor(
            "LongControl",
            Place::West,
            Far::In(Place::East),
            Trigger::Go(1),
        )
    },
    actor(
        "DoorOut",
        Place::Kraldar,
        Far::In(Place::Winterhold),
        Trigger::Go(1),
    ),
    actor(
        "DoorIn",
        Place::Winterhold,
        Far::In(Place::Kraldar),
        Trigger::Go(1),
    ),
    actor("HurtAway", Place::West, Far::None, Trigger::Never),
    actor("KilledAway", Place::West, Far::None, Trigger::Never),
    actor("InsideWalker", Place::Palace, Far::Pair, Trigger::Go(2)),
    actor("Hurt", Place::Palace, Far::None, Trigger::Never),
    actor("Timed", Place::Palace, Far::None, Trigger::Never),
    actor("Watched", Place::Outdoors, Far::Pair, Trigger::Go(3)),
    actor("Scheduled", Place::West, Far::Pair, Trigger::Noon),
    actor("Ferry", Place::West, Far::In(Place::East), Trigger::Go(4)),
];

/// Globals `Go1` to `Go4`.
const GOES: u32 = 4;

fn k_of(label: &str) -> usize {
    ACTOR_LIST
        .iter()
        .position(|a| a.label == label)
        .unwrap_or_else(|| panic!("no actor {label}"))
}

/// The actors sampled while they walk unloaded (step 1).
const WALKERS: [&str; 6] = [
    "LongWalk",
    "LongRun",
    "LongNoEval",
    "LongControl",
    "DoorOut",
    "DoorIn",
];
/// All of step 1's actors, read at its checkpoints.
const AWAY: [&str; 8] = [
    "LongWalk",
    "LongRun",
    "LongNoEval",
    "LongControl",
    "DoorOut",
    "DoorIn",
    "HurtAway",
    "KilledAway",
];
/// Seconds after Go1 that step 1 reads everyone.
const CHECKPOINTS: [f32; 5] = [5.0, 15.0, 30.0, 60.0, 90.0];
/// The palace's actors, and Watched.
const INSIDE: [&str; 3] = ["InsideWalker", "Hurt", "Timed"];

/// Timed's effect: Fortify Health 50 for this many seconds.
const EFFECT_SECONDS: u32 = 90;

const TESTS: &[(&str, &str)] = &[
    ("Start", "Go1 set, the unloaded actors as it was"),
    ("Walk", "LongWalk and LongRun, unloaded"),
    ("NoEval", "LongNoEval and LongControl, unloaded"),
    ("Doors", "DoorOut and DoorIn, Kraldar's House"),
    ("HurtAway", "HurtAway and KilledAway, damaged unloaded"),
    ("Inside", "The palace: InsideWalker set off, Hurt, Timed"),
    ("Watched", "Outdoors: Watched set off"),
    ("Away", "30 s in Ysolda's House"),
    ("Return", "Back outdoors and in the palace"),
    ("Schedule", "Scheduled as the clock passes 12:00"),
    ("FastTravel", "Ferry set off, then fast travel"),
];

/// Actor `k`'s marker its distance is read from: the far one, or home.
fn marker_of(k: usize) -> Arg {
    match ACTOR_LIST[k].far {
        Far::None => auto_var(&format!("Home{k}")),
        _ => auto_var(&format!("Far{k}")),
    }
}

/// `Brief(Int n, ObjectReference m)`: our actor `n`'s distance and
/// height from marker `m`, cell, 3D and package, or that the game has
/// none.
fn brief_fn() -> FunctionBuilder {
    let mut c = Code::new("String");
    let n = c.param("n", "Int");
    let m = c.param("m", "ObjectReference");
    let r = c.local("r", "String");
    c.assign(r.clone(), string(""));
    let a = c.own("Get", "Actor", [n]);
    let missing = is_none(&mut c, a.clone());
    let found = c.label();
    c.jmpf(missing, found)
        .ret(string("not in memory"))
        .bind(found);
    let d = c.method("ObjectReference", a.clone(), "GetDistance", [m.clone()]);
    let z = c.method("ObjectReference", a.clone(), "GetPositionZ", []);
    let mz = c.method("ObjectReference", m, "GetPositionZ", []);
    let dz = c.calc(op::FSUB, "Float", z, mz);
    c.numbers(&r, [("d ", d), (" (z ", dz)]);
    let cell = c.method("ObjectReference", a.clone(), "GetParentCell", []);
    put(&mut c, &r, "), ", cell);
    let loaded = c.method("ObjectReference", a.clone(), "Is3DLoaded", []);
    put(&mut c, &r, ", 3D ", loaded);
    let package = c.method("Actor", a, "GetCurrentPackage", []);
    put(&mut c, &r, ", ", package);
    c.finish(r)
}

/// Actor `k`'s brief into `r`, after `text`.
fn put_brief(c: &mut Code, r: &Arg, text: &str, k: usize) {
    let s = c.own(
        "Brief",
        "String",
        [((ACTORS + k as u32) as i32).into(), marker_of(k)],
    );
    put(c, r, text, s);
    line(c, r, "\n");
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

fn health(c: &mut Code, a: &Arg) -> Arg {
    c.method("Actor", a.clone(), "GetActorValue", [string("Health")])
}

/// `r += text + health of k (dead?)`, a line.
fn put_health(c: &mut Code, r: &Arg, text: &str, k: usize) {
    put_health_then(c, r, text, k, "\n");
}

/// [`put_health`], ending with `end`.
fn put_health_then(c: &mut Code, r: &Arg, text: &str, k: usize, end: &str) {
    line(c, r, text);
    let a = get(c, k);
    let missing = is_none(c, a.clone());
    let (found, done) = (c.label(), c.label());
    c.jmpf(missing, found);
    line(c, r, "not in memory");
    c.jmp(done);
    c.bind(found);
    let hp = health(c, &a);
    put_num(c, r, "health ", hp);
    let dead = c.method("Actor", a, "IsDead", []);
    put(c, r, ", dead ", dead);
    c.bind(done);
    line(c, r, end);
}

/// `r += text + whether Timed has its effect`.
fn put_effect(c: &mut Code, r: &Arg, text: &str) {
    line(c, r, text);
    let a = get(c, k_of("Timed"));
    let missing = is_none(c, a.clone());
    let (found, done) = (c.label(), c.label());
    c.jmpf(missing, found);
    line(c, r, "not in memory");
    c.jmp(done);
    c.bind(found);
    let has = c.method("Actor", a.clone(), "HasMagicEffect", [auto_var("Effect")]);
    put(c, r, "effect ", has);
    let hp = health(c, &a);
    put_num(c, r, ", health ", hp);
    c.bind(done);
    line(c, r, "\n");
}

/// Set global `Go<n>` to 1, and `EvaluatePackage` on the actors it
/// triggers that want it.
fn set_off(c: &mut Code, n: u32) {
    c.method(
        "GlobalVariable",
        auto_var(&format!("Go{n}")),
        "SetValue",
        [1.0f32.into()],
    );
    for (k, a) in ACTOR_LIST.iter().enumerate() {
        if a.trigger == Trigger::Go(n) && a.evaluate {
            with_actor(c, k, |c, a| {
                c.method("Actor", a, "EvaluatePackage", []);
            });
        }
    }
}

/// The seconds since `T0`.
fn since(c: &mut Code, t0: &str) -> Arg {
    let t = now(c);
    c.calc(op::FSUB, "Float", t, id(t0))
}

/// The player moved to place `p`'s entry.
fn move_player(c: &mut Code, to: Arg) {
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

/// Waits up to 60 s for the 3D of every actor of `labels`; returns the
/// seconds it took (-1: not in time).
fn wait_loaded(c: &mut Code, labels: &[&str]) -> Arg {
    let ks: Vec<usize> = labels.iter().map(|l| k_of(l)).collect();
    poll(c, "loaded", 0.2, 60.0, |c, hit| {
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
    })
}

/// `Sample(Float t)`: each walker's move since the last sample counted.
fn sample_fn() -> FunctionBuilder {
    let mut c = Code::new("None");
    let t = c.param("t", "Float");
    for label in WALKERS {
        let k = k_of(label);
        with_actor(&mut c, k, |c, a| {
            let (lx, ly) = (id(&format!("LastX{k}")), id(&format!("LastY{k}")));
            let x = c.method("ObjectReference", a.clone(), "GetPositionX", []);
            let y = c.method("ObjectReference", a, "GetPositionY", []);
            let dx = c.calc(op::FSUB, "Float", x.clone(), lx.clone());
            let dy = c.calc(op::FSUB, "Float", y.clone(), ly.clone());
            let dx2 = c.calc(op::FMUL, "Float", dx.clone(), dx);
            let dy2 = c.calc(op::FMUL, "Float", dy.clone(), dy);
            let d2 = c.calc(op::FADD, "Float", dx2, dy2);
            let d = c.global("Math", "sqrt", [d2]);
            let still = c.calc(op::CMP_LT, "Bool", d.clone(), 1.0f32.into());
            let done = c.label();
            c.jmpt(still, done);
            let moves = id(&format!("Moves{k}"));
            c.op(op::IADD, [moves.clone(), moves, 1.into()]);
            let path = id(&format!("Path{k}"));
            c.op(op::FADD, [path.clone(), path, d.clone()]);
            let first = id(&format!("FirstT{k}"));
            let unset = c.calc(op::CMP_LT, "Bool", first.clone(), 0.0f32.into());
            let set = c.label();
            c.jmpf(unset, set).assign(first, t.clone()).bind(set);
            c.assign(id(&format!("LastT{k}")), t.clone());
            let biggest = id(&format!("MaxStep{k}"));
            let bigger = c.calc(op::CMP_GT, "Bool", d.clone(), biggest.clone());
            let small = c.label();
            c.jmpf(bigger, small).assign(biggest, d).bind(small);
            c.assign(lx, x).assign(ly, y);
            c.bind(done);
        });
    }
    c.finish(Arg::None)
}

/// `Checkpoint(Float t)`: step 1's actors read into their `Check<k>`.
fn checkpoint_fn() -> FunctionBuilder {
    let mut c = Code::new("None");
    let t = c.param("t", "Float");
    let n = c.own("Num", "String", [t]);
    for label in AWAY {
        let k = k_of(label);
        let check = id(&format!("Check{k}"));
        let s = c.own(
            "Brief",
            "String",
            [((ACTORS + k as u32) as i32).into(), marker_of(k)],
        );
        c.append(check.clone(), [string("  "), n.clone(), string(" s: "), s]);
        if ACTOR_LIST[k].far == Far::None {
            let r = c.temp("String");
            c.assign(r.clone(), string(""));
            put_health(&mut c, &r, "; ", k);
            c.append(check, [r]);
        } else {
            c.append(check, [string("\n")]);
        }
    }
    c.finish(Arg::None)
}

/// Step 1's first page: the player held, Go1 set, the unloaded actors
/// damaged, then 90 s of samples.
fn test_start() -> FunctionBuilder {
    let (mut c, r) = report();
    c.controls(false, true, true);
    let cell = c.method("ObjectReference", auto_var("Player"), "GetParentCell", []);
    for p in PLACES {
        if let Some((_, name)) = p.interior() {
            let inside = c.calc(
                op::CMP_EQ,
                "Bool",
                cell.clone(),
                auto_var(&format!("Cell{}", p.index())),
            );
            let outside = c.label();
            c.jmpf(inside, outside);
            line(
                &mut c,
                &r,
                &format!("INVALID: the player was in {name} when the test began\n"),
            );
            c.bind(outside);
        }
    }
    let t = now(&mut c);
    let game = c.global("Utility", "GetCurrentGameTime", []);
    let hours = c.calc(op::FMUL, "Float", game, 24.0f32.into());
    put_num(&mut c, &r, "Go1 at real time ", t);
    put_num(&mut c, &r, ", game hour ", hours);
    line(&mut c, &r, "; as it was set:\n");
    for label in AWAY {
        let k = k_of(label);
        put_brief(&mut c, &r, &format!("  {label}: "), k);
        with_actor(&mut c, k, |c, a| {
            let x = c.method("ObjectReference", a.clone(), "GetPositionX", []);
            let y = c.method("ObjectReference", a, "GetPositionY", []);
            c.assign(id(&format!("LastX{k}")), x)
                .assign(id(&format!("LastY{k}")), y);
        });
    }
    // Damage while unloaded: half of HurtAway's health, all of KilledAway's.
    for (label, part) in [("HurtAway", 0.5f32), ("KilledAway", 2.0)] {
        let k = k_of(label);
        put_health(&mut c, &r, &format!("  {label} before: "), k);
        with_actor(&mut c, k, |c, a| {
            let hp = health(c, &a);
            let dmg = c.calc(op::FMUL, "Float", hp, part.into());
            c.method("Actor", a, "DamageActorValue", [string("Health"), dmg]);
        });
        put_health(&mut c, &r, &format!("  {label} after: "), k);
    }
    set_off(&mut c, 1);
    let start = now(&mut c);
    c.assign(id("T0"), start);
    // 90 s of samples, checkpoints as they come.
    let cp = c.local("cp", "Int");
    c.assign(cp.clone(), 0.into());
    let (top, out) = (c.label(), c.label());
    c.bind(top);
    c.wait(0.5);
    let t = since(&mut c, "T0");
    c.own("Sample", "None", [t.clone()]);
    for (j, at) in CHECKPOINTS.iter().enumerate() {
        let next = c.label();
        let this = c.calc(op::CMP_EQ, "Bool", cp.clone(), (j as i32).into());
        c.jmpf(this, next);
        let due = c.calc(op::CMP_GE, "Bool", t.clone(), (*at).into());
        c.jmpf(due, next);
        c.own("Checkpoint", "None", [(*at).into()]);
        c.op(op::IADD, [cp.clone(), cp.clone(), 1.into()]);
        c.bind(next);
    }
    let over = c.calc(
        op::CMP_GE,
        "Bool",
        cp.clone(),
        (CHECKPOINTS.len() as i32).into(),
    );
    c.jmpt(over, out);
    c.jmp(top);
    c.bind(out);
    c.finish(r)
}

/// A walker's summary and checkpoints.
fn walker_page(c: &mut Code, r: &Arg, label: &str, note: &str) {
    let k = k_of(label);
    let moves = id(&format!("Moves{k}"));
    put(c, r, &format!("{label} ({note}): position changed "), moves);
    let first = id(&format!("FirstT{k}"));
    let last = id(&format!("LastT{k}"));
    put_num(c, r, " times, from ", first.clone());
    put_num(c, r, " to ", last.clone());
    put_num(c, r, " s; path ", id(&format!("Path{k}")));
    put_num(c, r, ", largest step ", id(&format!("MaxStep{k}")));
    let span = c.calc(op::FSUB, "Float", last, first);
    let some = c.calc(op::CMP_GT, "Bool", span.clone(), 0.0f32.into());
    let skip = c.label();
    c.jmpf(some, skip);
    let speed = c.calc(op::FDIV, "Float", id(&format!("Path{k}")), span);
    put_num(c, r, ", ", speed);
    line(c, r, "/s");
    c.bind(skip);
    line(c, r, "\n");
    c.append(r.clone(), [id(&format!("Check{k}"))]);
}

fn test_walkers(labels: [(&'static str, &'static str); 2]) -> FunctionBuilder {
    let (mut c, r) = report();
    for (label, note) in labels {
        walker_page(&mut c, &r, label, note);
    }
    c.finish(r)
}

fn test_hurt_away() -> FunctionBuilder {
    let (mut c, r) = report();
    for label in ["HurtAway", "KilledAway"] {
        let k = k_of(label);
        line(&mut c, &r, &format!("{label}:\n"));
        c.append(r.clone(), [id(&format!("Check{k}"))]);
    }
    c.finish(r)
}

/// `label`'s distance from its far marker every second for `seconds`.
fn read_walking(c: &mut Code, r: &Arg, label: &str, seconds: usize) {
    let k = k_of(label);
    line(c, r, &format!("{label} from its far marker, each second:"));
    for i in 0..=seconds {
        if i > 0 {
            c.wait(1.0);
        }
        with_actor(c, k, |c, a| {
            let d = c.method("ObjectReference", a, "GetDistance", [marker_of(k)]);
            put_num(c, r, " ", d);
        });
    }
    line(c, r, "\n");
}

/// Step 2: the palace. `Back` placed where the player was.
fn test_inside() -> FunctionBuilder {
    let (mut c, r) = report();
    let back = c.method(
        "ObjectReference",
        auto_var("Player"),
        "PlaceAtMe",
        [auto_var("XMarker"), 1.into(), false.into(), false.into()],
    );
    c.assign(id("Back"), back);
    move_player(&mut c, entry(Place::Palace));
    let waited = wait_loaded(&mut c, &INSIDE);
    put_num(&mut c, &r, "in the palace; 3D after ", waited);
    line(&mut c, &r, " s:\n");
    for label in INSIDE {
        put_brief(&mut c, &r, &format!("  {label}: "), k_of(label));
    }
    let hurt = k_of("Hurt");
    put_health(&mut c, &r, "  Hurt before: ", hurt);
    with_actor(&mut c, hurt, |c, a| {
        let hp = health(c, &a);
        let dmg = c.calc(op::FMUL, "Float", hp, 0.5f32.into());
        c.method("Actor", a, "DamageActorValue", [string("Health"), dmg]);
    });
    put_health(&mut c, &r, "  Hurt after: ", hurt);
    with_actor(&mut c, k_of("Timed"), |c, a| {
        c.method("Spell", auto_var("Spell"), "Cast", [a.clone(), a]);
    });
    let cast = now(&mut c);
    c.assign(id("TCast"), cast.clone());
    put_num(&mut c, &r, "  Timed cast at ", cast);
    line(&mut c, &r, "\n");
    c.wait(1.0);
    put_effect(&mut c, &r, "  Timed 1 s later: ");
    set_off(&mut c, 2);
    read_walking(&mut c, &r, "InsideWalker", 5);
    put_brief(&mut c, &r, "  InsideWalker: ", k_of("InsideWalker"));
    put_health(&mut c, &r, "  Hurt now: ", hurt);
    c.finish(r)
}

fn test_watched() -> FunctionBuilder {
    let (mut c, r) = report();
    move_player(&mut c, entry(Place::Outdoors));
    let waited = wait_loaded(&mut c, &["Watched"]);
    put_num(&mut c, &r, "outdoors; 3D after ", waited);
    line(&mut c, &r, " s:\n");
    let k = k_of("Watched");
    put_brief(&mut c, &r, "  Watched: ", k);
    set_off(&mut c, 3);
    read_walking(&mut c, &r, "Watched", 5);
    put_brief(&mut c, &r, "  Watched: ", k);
    let left = now(&mut c);
    c.assign(id("TLeft"), left.clone());
    put_num(&mut c, &r, "left at ", left);
    line(&mut c, &r, "\n");
    c.finish(r)
}

fn test_away() -> FunctionBuilder {
    let (mut c, r) = report();
    move_player(&mut c, entry(Place::Ysolda));
    let start = now(&mut c);
    c.assign(id("TAway"), start);
    for (i, at) in [0.0f32, 10.0, 20.0, 30.0].into_iter().enumerate() {
        if i > 0 {
            c.wait(10.0);
        }
        let t = since(&mut c, "TLeft");
        put_num(&mut c, &r, &format!("{at} s in (left outdoors "), t);
        line(&mut c, &r, " s ago):\n");
        put_brief(&mut c, &r, "  InsideWalker: ", k_of("InsideWalker"));
        put_brief(&mut c, &r, "  Watched: ", k_of("Watched"));
        put_health_then(&mut c, &r, "  Hurt: ", k_of("Hurt"), "; ");
        put_effect(&mut c, &r, "Timed: ");
    }
    c.finish(r)
}

fn test_return() -> FunctionBuilder {
    let (mut c, r) = report();
    move_player(&mut c, entry(Place::Outdoors));
    let waited = wait_loaded(&mut c, &["Watched"]);
    let t = since(&mut c, "TLeft");
    put_num(&mut c, &r, "outdoors again ", t);
    put_num(&mut c, &r, " s after leaving; 3D after ", waited);
    line(&mut c, &r, " s:\n");
    put_brief(&mut c, &r, "  Watched: ", k_of("Watched"));
    c.wait(3.0);
    put_brief(&mut c, &r, "  3 s later: ", k_of("Watched"));
    move_player(&mut c, entry(Place::Palace));
    let waited = wait_loaded(&mut c, &INSIDE);
    let t = since(&mut c, "TCast");
    put_num(&mut c, &r, "in the palace again ", t);
    put_num(&mut c, &r, " s after the cast; 3D after ", waited);
    line(&mut c, &r, " s:\n");
    put_brief(&mut c, &r, "  InsideWalker: ", k_of("InsideWalker"));
    put_health(&mut c, &r, "  Hurt: ", k_of("Hurt"));
    put_effect(&mut c, &r, "  Timed: ");
    // Timed until its effect ends.
    let timed = k_of("Timed");
    let ended = poll(&mut c, "ended", 1.0, 100.0, |c, hit| {
        let a = get(c, timed);
        let missing = is_none(c, a.clone());
        c.jmpt(missing, hit);
        let has = c.method("Actor", a, "HasMagicEffect", [auto_var("Effect")]);
        c.jmpf(has, hit);
    });
    let t = since(&mut c, "TCast");
    put_num(&mut c, &r, "  Timed's effect gone ", ended);
    put_num(&mut c, &r, " s later (-1: not in 100 s), ", t);
    line(&mut c, &r, " s after the cast\n");
    put_effect(&mut c, &r, "  Timed: ");
    put_health(&mut c, &r, "  Hurt: ", k_of("Hurt"));
    move_player(&mut c, id("Back"));
    c.wait(3.0);
    c.finish(r)
}

fn hour_now(c: &mut Code) -> Arg {
    let game = c.global("Utility", "GetCurrentGameTime", []);
    let days = c.global("Math", "Floor", [game.clone()]);
    let day = c.temp("Float");
    c.cast(day.clone(), days);
    let frac = c.calc(op::FSUB, "Float", game, day);
    c.calc(op::FMUL, "Float", frac, 24.0f32.into())
}

fn test_schedule() -> FunctionBuilder {
    let (mut c, r) = report();
    let k = k_of("Scheduled");
    let h = hour_now(&mut c);
    put_num(&mut c, &r, "hour was ", h);
    line(&mut c, &r, "\n");
    put_brief(&mut c, &r, "  Scheduled: ", k);
    set_hour(&mut c, 11.95);
    let start = now(&mut c);
    c.assign(id("TClock"), start);
    // 11:57 to 12:00 is 9 real seconds at the usual time scale (20).
    for (i, at) in [0.0f32, 5.0, 10.0, 15.0, 20.0, 30.0, 45.0, 70.0]
        .into_iter()
        .enumerate()
    {
        if i > 0 {
            let wait = at - [0.0f32, 5.0, 10.0, 15.0, 20.0, 30.0, 45.0][i - 1];
            c.wait(wait);
        }
        let t = since(&mut c, "TClock");
        let h = hour_now(&mut c);
        put_num(&mut c, &r, "  ", t);
        put_num(&mut c, &r, " s, hour ", h);
        let s = c.own(
            "Brief",
            "String",
            [((ACTORS + k as u32) as i32).into(), marker_of(k)],
        );
        put(&mut c, &r, ": ", s);
        line(&mut c, &r, "\n");
    }
    c.finish(r)
}

fn test_fast_travel() -> FunctionBuilder {
    let (mut c, r) = report();
    let ferry = k_of("Ferry");
    set_off(&mut c, 4);
    c.wait(2.0);
    put_brief(&mut c, &r, "Ferry 2 s after Go4: ", ferry);
    put_brief(&mut c, &r, "LongWalk: ", k_of("LongWalk"));
    put_brief(&mut c, &r, "Scheduled: ", k_of("Scheduled"));
    let game0 = c.global("Utility", "GetCurrentGameTime", []);
    let g0 = c.temp("Float");
    c.assign(g0.clone(), game0);
    let real0 = now(&mut c);
    let r0 = c.temp("Float");
    c.assign(r0.clone(), real0);
    c.global("Game", "FastTravel", [entry(Place::Ysolda)]);
    let ysolda = auto_var(&format!("Cell{}", Place::Ysolda.index()));
    let arrived = poll(&mut c, "arrived", 0.5, 60.0, |c, hit| {
        let cell = c.method("ObjectReference", auto_var("Player"), "GetParentCell", []);
        let there = c.calc(op::CMP_EQ, "Bool", cell, ysolda.clone());
        c.jmpt(there, hit);
    });
    c.wait(2.0);
    let game1 = c.global("Utility", "GetCurrentGameTime", []);
    let days = c.calc(op::FSUB, "Float", game1, g0);
    let hours = c.calc(op::FMUL, "Float", days, 24.0f32.into());
    let real1 = now(&mut c);
    let secs = c.calc(op::FSUB, "Float", real1, r0);
    put_num(&mut c, &r, "fast travelled: arrived after ", arrived);
    put_num(
        &mut c,
        &r,
        " s (-1: not in 60 s); game hours passed ",
        hours,
    );
    put_num(&mut c, &r, ", real seconds ", secs);
    line(&mut c, &r, "\n");
    put_brief(&mut c, &r, "Ferry: ", ferry);
    put_brief(&mut c, &r, "LongWalk: ", k_of("LongWalk"));
    put_brief(&mut c, &r, "Scheduled: ", k_of("Scheduled"));
    c.wait(10.0);
    put_brief(&mut c, &r, "Ferry 10 s later: ", ferry);
    let back = id("Back");
    move_player(&mut c, back.clone());
    c.wait(3.0);
    c.method("ObjectReference", back.clone(), "Disable", [false.into()]);
    c.method("ObjectReference", back, "Delete", []);
    c.finish(r)
}

/// The actors' script: each event logged into its own `ResultEvents`.
fn actor_script() -> Result<ScriptBuilder> {
    let mut s = ScriptBuilder::new(ACTOR_SCRIPT, "Actor");
    s.variable("ResultEvents", "String", string(""));
    // `Log(String what, String detail)`.
    let mut log = Code::new("None");
    let what = log.param("what", "String");
    let detail = log.param("detail", "String");
    let events = id("ResultEvents");
    let t = now(&mut log);
    let tn = log.own("Num", "String", [t]);
    let game = log.global("Utility", "GetCurrentGameTime", []);
    let hours = log.calc(op::FMUL, "Float", game, 24.0f32.into());
    let gn = log.own("Num", "String", [hours]);
    let me = log.temp("ObjectReference");
    log.cast(me.clone(), id("self"));
    let cell = log.method("ObjectReference", me.clone(), "GetParentCell", []);
    let loaded = log.method("ObjectReference", me, "Is3DLoaded", []);
    log.append(
        events.clone(),
        [
            tn,
            string(" g"),
            gn,
            string(" "),
            what,
            string(" "),
            detail,
            string(", "),
        ],
    );
    log.append(events.clone(), [cell, string(" 3D "), loaded]);
    log.append(events, [string("\n")]);
    s.function("Log", log.finish(Arg::None));
    s.function("Num", num());
    let events: &[(&str, &[(&str, &str)])] = &[
        ("OnPackageStart", &[("akNewPackage", "Package")]),
        ("OnPackageChange", &[("akOldPackage", "Package")]),
        ("OnPackageEnd", &[("akOldPackage", "Package")]),
        ("OnLoad", &[]),
        ("OnUnload", &[]),
        ("OnCellAttach", &[]),
        ("OnCellDetach", &[]),
        ("OnAttachedToCell", &[]),
        ("OnDetachedFromCell", &[]),
        (
            "OnLocationChange",
            &[("akOldLoc", "Location"), ("akNewLoc", "Location")],
        ),
        ("OnDeath", &[("akKiller", "Actor")]),
        (
            "OnMagicEffectApply",
            &[("akCaster", "ObjectReference"), ("akEffect", "MagicEffect")],
        ),
    ];
    for (event, params) in events {
        let mut c = Code::new("None");
        let ps: Vec<Arg> = params.iter().map(|(n, t)| c.param(n, t)).collect();
        let detail = c.temp("String");
        c.assign(detail.clone(), string(""));
        for p in ps {
            c.append(detail.clone(), [p, string(" ")]);
        }
        let name = event.trim_start_matches("On");
        c.own("Log", "None", [string(name), detail]);
        s.function(event, c.finish(Arg::None));
    }
    Ok(s)
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
        if p.interior().is_some() {
            s.auto_property(&format!("Cell{}", p.index()), "Cell");
        }
    }
    for (k, a) in ACTOR_LIST.iter().enumerate() {
        s.auto_property(&format!("Home{k}"), "ObjectReference");
        if a.far != Far::None {
            s.auto_property(&format!("Far{k}"), "ObjectReference");
        }
    }
    s.variable("Ran", "Bool", false.into());
    s.variable("Back", "ObjectReference", Arg::None);
    for t in ["T0", "TCast", "TLeft", "TAway", "TClock"] {
        s.variable(t, "Float", 0.0f32.into());
    }
    for label in AWAY {
        let k = k_of(label);
        for v in ["LastX", "LastY", "Path", "MaxStep", "LastT"] {
            s.variable(&format!("{v}{k}"), "Float", 0.0f32.into());
        }
        s.variable(&format!("FirstT{k}"), "Float", (-1.0f32).into());
        s.variable(&format!("Moves{k}"), "Int", 0.into());
        s.variable(&format!("Check{k}"), "String", string(""));
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
        .function("OnUpdate", on_update("autotest13", "six minutes", TESTS))
        .function("Num", num())
        .function("Get", get_fn(PLUGIN))
        .function("Brief", brief_fn())
        .function("Sample", sample_fn())
        .function("Checkpoint", checkpoint_fn())
        .function("TestStart", test_start())
        .function(
            "TestWalk",
            test_walkers([
                ("LongWalk", "walks, EvaluatePackage"),
                ("LongRun", "runs, EvaluatePackage"),
            ]),
        )
        .function(
            "TestNoEval",
            test_walkers([
                ("LongNoEval", "walks, no EvaluatePackage"),
                ("LongControl", "not persistent, no script"),
            ]),
        )
        .function(
            "TestDoors",
            test_walkers([
                ("DoorOut", "inside, its marker outside"),
                ("DoorIn", "outside, its marker inside"),
            ]),
        )
        .function("TestHurtAway", test_hurt_away())
        .function("TestInside", test_inside())
        .function("TestWatched", test_watched())
        .function("TestAway", test_away())
        .function("TestReturn", test_return())
        .function("TestSchedule", test_schedule())
        .function("TestFastTravel", test_fast_travel());
    let actors = actor_script()?;
    Ok(vec![
        (SCRIPT.to_owned(), papyrus::pex::write(&s.build()?)?),
        (
            ACTOR_SCRIPT.to_owned(),
            papyrus::pex::write(&actors.build()?)?,
        ),
    ])
}

/// The Tamriel cell just outside Kraldar's House's front door (holding its
/// load door's teleport target).
fn outside_door(lo: &LoadOrder, interior: FormId, world: FormId) -> Result<FormId> {
    let index = lo.cell(interior).context("Kraldar's House")?;
    for &r in index.persistent.iter().chain(&index.temporary) {
        let Some(rec) = lo.get(r) else { continue };
        let Some(xtel) = rec.get(b"XTEL").filter(|d| d.len() >= 16) else {
            continue;
        };
        let dest = FormId(u32::from_le_bytes(xtel[0..4].try_into().unwrap()));
        let Some(cell) = lo.cell_of_ref(dest) else {
            continue;
        };
        if lo.cell(cell).and_then(|c| c.world) != Some(world) {
            continue;
        }
        let f = |o: usize| f32::from_le_bytes(xtel[o..o + 4].try_into().unwrap());
        let (x, y) = (f(4), f(8));
        let grid = ((x / 4096.0).floor() as i32, (y / 4096.0).floor() as i32);
        return lo
            .world(world)
            .and_then(|w| w.cells.get(&grid))
            .copied()
            .with_context(|| format!("no Tamriel cell at {grid:?}"));
    }
    anyhow::bail!("Kraldar's House has no door to Tamriel")
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
    let ext = |grid: (i32, i32)| -> Result<FormId> {
        lo.world(tamriel)
            .and_then(|w| w.cells.get(&grid))
            .copied()
            .with_context(|| format!("no Tamriel cell at {grid:?}"))
    };
    let kraldar = v.id(KRALDAR.0)?;
    let cells = PLACES
        .iter()
        .map(|p| match p {
            Place::West => ext(WEST),
            Place::East => ext(EAST),
            Place::Outdoors => ext(OUTDOORS),
            Place::Winterhold => outside_door(lo, kraldar, tamriel),
            _ => v.id(p.interior().unwrap().0),
        })
        .collect::<Result<Vec<_>>>()?;
    // Spots each place needs: one per actor standing there (with its pair
    // in reach), one per far marker of an actor elsewhere.
    let mut spots: Vec<Spots> = Vec::new();
    for (i, p) in PLACES.iter().enumerate() {
        let homes = ACTOR_LIST.iter().filter(|a| a.home == *p).count();
        let visitors = ACTOR_LIST.iter().filter(|a| a.far == Far::In(*p)).count();
        let (reach, apart) = match p {
            Place::West => ((600.0, 1000.0), 150.0),
            Place::Palace => ((600.0, 1200.0), 100.0),
            Place::Outdoors => ((2500.0, 3200.0), 150.0),
            Place::Kraldar => ((150.0, 400.0), 60.0),
            _ => ((250.0, 450.0), 120.0),
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

/// `PSDT` of a package from 12:00 for 12 hours.
const NOON: [u8; 12] = [0xFF, 0xFF, 0, 12, 0xFF, 0, 0, 0, 0xD0, 0x02, 0, 0];

/// `PKDT` general flag: use the preferred speed.
const PREFERRED_SPEED: u32 = 1 << 13;
const WALK: u8 = 0;
const RUN: u8 = 2;

fn package(
    lo: &LoadOrder,
    object_id: u32,
    edid: &str,
    marker: FormId,
    schedule: [u8; 12],
    condition: Option<FormId>,
    speed: u8,
) -> Result<RecordBuilder> {
    let at = pldt(0, marker, 0);
    records::copy_inserting(
        lo,
        "MS13LucanHoldPositionPackage",
        object_id,
        edid,
        None,
        |tag, d| match tag {
            b"PLDT" => *d = at.clone(),
            b"PSDT" => *d = schedule.to_vec(),
            b"PKDT" if d.len() >= 7 => {
                // Not offering services (0x1); the speed given.
                let flags = u32::from_le_bytes(d[0..4].try_into().unwrap());
                let flags = (flags & !0x1) | PREFERRED_SPEED;
                d[0..4].copy_from_slice(&flags.to_le_bytes());
                d[6] = speed;
            }
            _ => {}
        },
        |tag| match (tag, condition) {
            (b"PSDT", Some(g)) => vec![(
                *b"CTDA",
                records::ctda(
                    records::func::GET_GLOBAL_VALUE,
                    g.0,
                    0,
                    records::op::EQ,
                    1.0,
                    records::run_on::SUBJECT,
                )
                .to_vec(),
            )],
            _ => Vec::new(),
        },
    )
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
        if p.interior().is_some() {
            properties.push((format!("Cell{i}"), object(at.cells[i])));
        }
    }
    for (k, a) in ACTOR_LIST.iter().enumerate() {
        properties.push((format!("Home{k}"), object(own(HOMES + k as u32))));
        if a.far != Far::None {
            properties.push((format!("Far{k}"), object(own(FARS + k as u32))));
        }
    }

    let globals: Vec<RecordBuilder> = (1..=GOES)
        .map(|n| records::global(GLOBALS + n - 1, &format!("VrmAutoTest13Go{n}"), 0.0))
        .collect();
    let spell = records::spell(
        SPELL,
        "VrmAutoTest13Fortify",
        records::spell::SPELL,
        records::spell::FIRE_AND_FORGET,
        records::spell::SELF,
        0,
        None,
        &[Effect::new(effect, 50.0, EFFECT_SECONDS)],
    );

    // Two packages each: the far one first (when its trigger is set), then
    // staying home.
    let mut packages = Vec::new();
    for (k, a) in ACTOR_LIST.iter().enumerate() {
        let speed = if a.run { RUN } else { WALK };
        let far_id = PACKAGES + 2 * k as u32;
        if a.far != Far::None {
            let (schedule, condition) = match a.trigger {
                Trigger::Go(n) => (ALWAYS, Some(own(GLOBALS + n - 1))),
                Trigger::Noon => (NOON, None),
                Trigger::Never => unreachable!("{} goes nowhere", a.label),
            };
            packages.push(package(
                lo,
                far_id,
                &format!("VrmAutoTest13{}Far", a.label),
                own(FARS + k as u32),
                schedule,
                condition,
                speed,
            )?);
        }
        packages.push(package(
            lo,
            far_id + 1,
            &format!("VrmAutoTest13{}Home", a.label),
            own(HOMES + k as u32),
            ALWAYS,
            None,
            speed,
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
                edid: format!("VrmAutoTest13{}Npc", a.label),
                name: Some("vibe_rm Test NPC"),
                level: (10, false, 1, 0),
                template: (template, 0x1),
                temper: (0, 0),
                factions: Vec::new(),
                script: a.scripted.then(|| actor_script.clone()),
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
            edid: "VrmAutoTest13Quest",
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

    // References by place: persistent (entries, markers, persistent actors;
    // Tamriel's all in its persistent cell) and temporary.
    let marker = |id: FormId, at: [f32; 3]| {
        RecordBuilder::new(b"REFR", id)
            .flags(persistent)
            .form(b"NAME", heading_marker)
            .field(b"DATA", &placement(at, 0.0))
    };
    let mut by_place: Vec<(Vec<RecordBuilder>, Vec<RecordBuilder>)> =
        PLACES.iter().map(|_| (Vec::new(), Vec::new())).collect();
    for p in PLACES {
        let i = p.index();
        by_place[i]
            .0
            .push(marker(own(ENTRIES + i as u32), at.entries[i]));
    }
    for (k, a) in ACTOR_LIST.iter().enumerate() {
        let home = a.home.index();
        by_place[home]
            .0
            .push(marker(own(HOMES + k as u32), at.homes[k]));
        if let (Far::In(p), Some(far)) = (a.far, at.fars[k]) {
            by_place[p.index()]
                .0
                .push(marker(own(FARS + k as u32), far));
        } else if let Some(far) = at.fars[k] {
            by_place[home].0.push(marker(own(FARS + k as u32), far));
        }
        let actor = RecordBuilder::new(b"ACHR", own(ACTORS + k as u32))
            .form(b"NAME", own(NPCS + k as u32))
            .field(b"DATA", &placement(at.homes[k], 0.0));
        if a.persistent {
            by_place[home].0.push(actor.flags(persistent));
        } else {
            by_place[home].1.push(actor);
        }
    }

    let mut interiors = Vec::new();
    let mut tamriel_persistent = Vec::new();
    let mut west_temporary = Vec::new();
    for (p, (persistent_refs, temporary_refs)) in PLACES.into_iter().zip(by_place) {
        match p.interior() {
            Some((_, name)) => interiors.push(Interior {
                cell: at.cells[p.index()],
                name,
                persistent: persistent_refs,
                temporary: temporary_refs,
            }),
            None => {
                tamriel_persistent.extend(persistent_refs);
                anyhow::ensure!(
                    p == Place::West || temporary_refs.is_empty(),
                    "temporary references only go in the west cell"
                );
                west_temporary.extend(temporary_refs);
            }
        }
    }
    let worlds = vec![World {
        world: tamriel,
        name: "Skyrim",
        grid: WEST,
        persistent: tamriel_persistent,
        temporary: west_temporary,
    }];

    // Top groups in the Creation Kit's order.
    let plugin = write_plugin(
        0,
        &["Skyrim.esm"],
        "vibe_rm",
        "What actors do while the player is away: unloaded travel, through load doors, actors \
         left behind, a timed effect and wounds, a schedule change and fast travel, read by \
         script on loading a save, reports saved.",
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
            let i = p.index();
            properties.push((
                format!("Entry{i}"),
                form(own(ENTRIES + i as u32).0, "ObjectReference"),
            ));
            if p.interior().is_some() {
                properties.push((format!("Cell{i}"), form(0x133C9 + i as u32, "Cell")));
            }
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
        let shown = run(
            scripts().unwrap(),
            &[(own(QUEST).0, SCRIPT, properties())],
            TESTS.len(),
        );
        assert_eq!(shown.boxes.len(), TESTS.len(), "every test reports");
        assert_eq!(shown.saved_at_box, Some(0), "saves before the boxes");
        let all = shown.boxes.join("\n");
        for expected in [
            "Go1 at real time ",
            "  KilledAway after: health 1, dead False\n",
            "LongWalk (walks, EvaluatePackage): position changed ",
            "  90 s: d 1 (z 0), ",
            "InsideWalker from its far marker, each second: 1 1 1 1 1 1\n",
            "30 s in (left outdoors ",
            "  Timed's effect gone ",
            " s, hour ",
            "fast travelled: arrived after ",
            "Ferry 10 s later: d 1",
        ] {
            assert!(all.contains(expected), "{expected:?} not in the reports");
        }
        for b in &shown.boxes {
            assert!(b.lines().count() <= 20, "{} lines:\n{b}", b.lines().count());
        }
    }

    #[test]
    fn object_ids_fit() {
        let n = ACTOR_LIST.len() as u32;
        assert!(GLOBALS + GOES <= SPELL);
        assert!(NPCS + n <= PACKAGES);
        assert!(PACKAGES + 2 * n <= HOMES);
        assert!(HOMES + n <= FARS);
        assert!(FARS + n <= ENTRIES);
        assert!(ENTRIES + PLACES.len() as u32 <= ACTORS);
        assert!(ACTORS + n <= NEXT_OBJECT_ID);
    }

    #[test]
    fn triggers_match_targets() {
        for a in ACTOR_LIST {
            assert_eq!(
                a.far == Far::None,
                a.trigger == Trigger::Never,
                "{}",
                a.label
            );
        }
        for label in WALKERS.iter().chain(&AWAY).chain(&INSIDE) {
            k_of(label);
        }
    }
}
