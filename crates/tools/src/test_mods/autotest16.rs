//! `test-mod autotest16`: what autotest13 to autotest15 left open about
//! unloaded actors (`plans/ai-actor-execution.md` step 9,
//! `known_ambiguities/actor-execution.md` "Open questions"): what decides
//! whether an actor sent off unloaded is put at its target, at once, late
//! or never; whether actors walk while unloaded; what happens to a journey
//! as its cell unloads; which cells keep running after the player leaves;
//! furniture and sandboxing through a wait and on return; waiting and
//! sleeping with actors out of range; when a timed effect ends as its
//! actor unloads; and a dead actor's package events.
//!
//! Actors are made as in autotest13 (the same template NPC, its events
//! logged into its own `ResultEvents`). One sent somewhere has a Travel
//! package to its far marker under `GetGlobalValue VrmAutoTest16<label>Go
//! == 1` (a global each) and a Travel package home below it. **Asked**
//! ones also get `EvaluatePackage` as they're sent; unasked ones only see
//! their global change. Sending one home sets its global back to 0,
//! unasked. Every reading gives the distance from the far marker (and
//! height), from home, cell, 3D and package (autotest14's `Where`).
//! Tracking samples actors every quarter second: each change of more than
//! 2 in the distance from home is a move (its count, the first and last,
//! the largest), with the first change of 3D for some.
//!
//! As built against Skyrim.esm (2026-10-25), far markers from home, in XY
//! and height:
//!
//! | actor          | stands in               | sent to                      | asked |
//! | -------------- | ----------------------- | ---------------------------- | ----- |
//! | **Timed**      | Tamriel -37, 21         | (stays: a 90 s Fortify Health cast on it) | |
//! | **PassA/B/C**  | Tamriel -20, 0          | 2,710 / 3,009 / 3,161, level (-68 / +133 / +92) | no |
//! | **FarAsked**   | the same                | -18, 0: 7,565, -162          | yes   |
//! | **FarUnasked** | the same                | -18, 0: 7,107, -293          | no    |
//! | **High**       | the hillsides around -10, -10 | 3,762, 2,281 above     | yes   |
//! | **MidHigh**    | the same                | 3,761, 1,453 below           | yes   |
//! | **MidLow**     | the same                | 3,782, 1,077 below           | yes   |
//! | **Leaving**    | Tamriel -8, 18          | 2,976, +542                  | yes, loaded |
//! | **LeavingFar** | the same                | -6, 18: 7,365, +245          | yes, loaded |
//! | **BufExt**     | Tamriel -30, 8          | 3,436, +131                  | yes, loaded |
//! | **BufIn1**     | the Palace of the Kings | 1,123 across it              | yes, loaded |
//! | **BufIn2**     | the Blue Palace         | 1,124 across it              | yes, loaded |
//! | **Sitter**     | the Sleeping Giant Inn  | (sits: a chair of ours 285 away, `MS13CamillaWorkTrader` copied) | |
//! | **Sandboxer**  | the same                | (sandboxes: `DefaultSandboxEditorLocation512`) | |
//! | **WaitFar**    | Tamriel -20, 0          | -18, 0: 7,747, -283          | no    |
//! | **SleepNear**  | Tamriel -28, -8 (the hub) | 2,677, -57                 | yes, loaded |
//! | **SleepFar**   | Tamriel -20, 0          | -18, 0: 7,494, -243          | no    |
//! | **Dead**       | the same                | 2,525, -129 (killed first)   | yes   |
//!
//! Every reference is persistent, Tamriel's in its persistent cell; no
//! alias holds an actor. The hub, where the player waits while others are
//! read, is 8 or more cells from every other place.
//!
//! The run, a page each (`OnUpdate`, 5 s after the save loads):
//!
//! 1. **Timed**: Dead killed unloaded. The player moved to Timed's cell,
//!    Timed given its effect, then the player moved to the hub (another
//!    exterior, far off): each change of Timed's 3D or effect for 30 s.
//! 2. **JumpsBefore**, **Jumps**, **JumpsAfter**: from the hub, the eight
//!    jumpers sent off together but PassB (10 s later) and PassC (20 s
//!    later), tracked for 125 s from the first sending, then read.
//! 3. **Home**: FarAsked, FarUnasked and PassA sent home, tracked for
//!    120 s; the far two read at 30, 60 and 90 s.
//! 4. **Leaving**: the player moved to Leaving's cell, Leaving and
//!    LeavingFar sent off loaded, read 5 s on; then the player moved to the
//!    hub and both tracked (with 3D) for 60 s.
//! 5. **Buffer1** to **Buffer3**: the player moved to BufExt's cell, BufExt
//!    sent off; 3 s on the player moved into the Palace of the Kings and
//!    BufIn1 sent off; then into the Blue Palace and BufIn2 sent off; then
//!    out to the hub. Each move is followed by 25 s of tracking (with 3D)
//!    of every actor left behind so far, read at 10 and 25 s.
//! 6. **Wait**: the player moved into the inn, up to 60 s for Sitter to sit
//!    (`GetSitState` 3), both inn actors read (sit state, from the chair or
//!    home, moved since then, 3D); WaitFar sent off; a box asks the user to
//!    **wait two hours**. Then the inn actors, WaitFar, High and MidHigh
//!    read, and 10 s later.
//! 7. **ReEntry**: the inn actors read; the player moved to the hub, then
//!    Timed's cell, Sitter's 3D watched at each (autotest14: a left
//!    interior kept its 3D until the player went on to a second exterior);
//!    10 s there, then back into the inn: both read as they load, 3 and
//!    13 s later.
//! 8. **Sleep**: at the hub, SleepNear and SleepFar sent off, a bedroll put
//!    beside the player, and a box asks the user to **sleep two hours** in
//!    it. Then both, High and MidHigh read, and 10 s later.
//! 9. **VisitLeaving**: the player moved to Leaving's cell, both read as
//!    they load and 3 s later.
//! 10. **VisitSteep**: the hillside actors given `EvaluatePackage` again
//!     from afar and read 5 s later, then visited (read as they load and 3 s
//!     later).
//! 11. **VisitJump**: Dead sent off, then the jumpers' cell visited: every
//!     actor standing there read as it loads.
//!
//! | reading                                                       | means                                         |
//! | ------------------------------------------------------------- | --------------------------------------------- |
//! | PassA/B/C each moved once, about 34-36 s after its own sending | late jumps are a delay per run (ours)         |
//! | all three moved in the same sample                             | late jumps come from a periodic pass          |
//! | FarAsked moved once, late; FarUnasked the same                 | asking only matters within range              |
//! | High never moved, MidLow moved at once                        | height stops the jump; MidHigh says where     |
//! | High moved at once                                            | height doesn't decide (a path may)            |
//! | the far ones walking home after `Home` (many small moves, about 80 a second) | unloaded actors walk past some distance |
//! | the far ones put home once (late)                             | Beyond's walk in autotest15 was something else |
//! | Leaving moved once soon after the player left                 | a journey under way jumps on unloading (ours) |
//! | Leaving walking on unloaded                                   | a cell just left (exterior to exterior) keeps running |
//! | BufExt's 3D kept through Buffer2, lost at Buffer3             | the exterior left stays loaded until the next exterior |
//! | BufIn1's 3D lost as the player goes into the Blue Palace but walking on | interiors left for interiors run without 3D (autotest13) |
//! | Sitter sitting after the wait, moved 0                        | furniture holds through a wait                |
//! | Sitter sitting in the chair as it loads after ReEntry          | an actor unloaded seated comes back seated (ours) |
//! | Sandboxer where it was after ReEntry                          | sandboxers come back where they left (ours)   |
//! | WaitFar / SleepFar on their markers right after the time passed | waiting / sleeping moves unloaded actors out of range |
//! | High moved by the wait or the sleep, or by `EvaluatePackage` again | something does reach far targets above or below |
//! | Timed's effect gone as its 3D goes                            | effects are dropped on unloading              |
//! | Dead logging `OnPackageStart` after VisitJump's sending        | dead actors still take up packages            |
//!
//! The plugin must be named `VrmAutoTest16.esp`. Load a save outdoors near
//! Riverwood, without the other test plugins, with a character not in
//! combat, and **stay at the keyboard**: the run takes about fifteen
//! minutes and asks for two things. When the first box comes up, close it
//! and wait two hours with the Wait menu straight away. When the second
//! does, walk to the bedroll beside you and sleep two hours in it, also
//! straight away: actors sent off just before may be put at their markers
//! about 35 s later anyway. As with autotest13, don't keep playing with the
//! plugin: it carries Skyrim.esm's records of Tamriel, its persistent cell,
//! the two palaces and the inn.
//!
//! Run in Skyrim SE (2026-10-25, once, the test character's usual save near
//! Riverwood). Times are from each package's `OnPackageStart` in the
//! actors' logs unless said otherwise.
//!
//! - **Jumps**, **Home**: unloaded travellers with a path **walk**. Each
//!   move was a step of about 218 (largest 217 to 221), about every 2.6 s,
//!   about 84 a second. PassA, PassB and PassC (unasked, level) walked from
//!   their first sample and arrived 40 short 33.6, 36.2 and 38.8 s after
//!   their own starts (2,710, 3,012 and 3,162 off), with `OnPackageEnd`:
//!   started 10 s apart, they arrived apart, so there's no common pass.
//!   FarUnasked walked its 7,113 in 93 s. Sent home unasked, FarAsked and
//!   FarUnasked walked 7,500 in 91 s and PassA 2,670 in 31 s, as
//!   autotest15's Beyond did.
//! - Asked: MidHigh and MidLow (4,032 and 3,933 off in 3D, 1,453 and 1,077
//!   below) were put 40 short in the first sample, `OnPackageEnd` 2.5 s
//!   after the start. High (4,399 off in 3D, 2,281 above; 3,762 in XY, as
//!   MidHigh's 3,761) walked about 1,540, then was put at its target 28.7 s
//!   in (`OnPackageEnd` at 31.0 s). FarAsked (7,566) walked about 3,000,
//!   then was put 36.6 s in (`OnPackageEnd` at 38.8 s). So height doesn't
//!   stop a jump; the 3D distance decides, between 4,032 and 4,399.
//! - **Leaving**: sent off loaded, both lost their 3D 0.35 s after the
//!   player left for the hub and walked on in the same steps: Leaving
//!   arrived 36.3 s later (2,822 left), LeavingFar walked on (98.5 s for
//!   7,331, `OnLocationChange` 5 s after leaving). **VisitLeaving** found
//!   both on their markers.
//! - **Buffer1**, **Buffer2**: BufExt, its exterior left for the Palace of
//!   the Kings and then the Blue Palace, kept its 3D and walked on, but a
//!   mudcrab killed it 21 s into Buffer1 (`OnDeath`), so its later
//!   readings are its body's. BufIn1, the palace left for the Blue Palace,
//!   kept its 3D (`OnCellDetach`, no `OnUnload`) and walked on, arriving
//!   10.8 s after the move (autotest13: the palace's actors lost their 3D
//!   leaving it for Ysolda's House). **Buffer3**: out to the hub, every
//!   cell left lost its 3D 0.8 s after the move (`OnCellDetach` and
//!   `OnUnload` together); BufIn2 walked on unloaded in steps of 217 and
//!   arrived 13 s after.
//! - **Wait** (2.00 hours): Sitter, seated as the inn loaded, stayed seated
//!   (moved 0). Sandboxer got up and wandered (575 off 10 s later).
//!   WaitFar, sent unasked 2 s before the box and 7,537 off then, was 40
//!   short of its marker after the wait, `OnPackageEnd` 2.6 s after it.
//!   The hillside actors had already arrived.
//! - **ReEntry**: Sitter's 3D stayed 20 s with the player at the hub
//!   (`OnCellDetach`; `OnUnload` only as the player went on to Timed's
//!   cell, gone 0.3 s after), as autotest14's palace did, unlike Buffer3's
//!   interiors. Back in the inn, Sitter was standing 18 from its chair
//!   (sit state 0, 15 from where it sat) and still not sitting 13 s later
//!   (33 off). Sandboxer was sitting in something 438 from where it was.
//! - **Sleep** (2.00 hours): SleepNear, loaded, 2,595 off before, was 4
//!   from its marker after (`OnPackageEnd` then). SleepFar, unloaded,
//!   unasked and 7,288 off before, was 3,605 short after: about 4,000
//!   along, not put there, and walking on (183 in the next 10 s, 2,052
//!   short at VisitJump).
//! - **Timed**: its effect was gone 0.43 s after the move began, with its
//!   3D (health 100 to 50).
//! - **Dead**: dead at once when killed unloaded. It logged
//!   `OnPackageStart` (its home package) as the wait ended, as BufExt's
//!   body did. Sent off unloaded with `EvaluatePackage`, it logged nothing
//!   until its cell loaded 3 s later, then `OnPackageChange` and
//!   `OnPackageStart`; its body stayed home.
//! - **VisitSteep**: `EvaluatePackage` again and the visit moved no one.
//!   Sandboxer logged `OnPackageEnd` for a created package (`FF000A85`)
//!   after the wait.
//!
//! So an unloaded actor outside the cells just left walks its journey at
//! about its pace in coarse steps; only one asked to evaluate with its
//! target within about 4,200 is put there at once, and one asked with it
//! farther is put there about 30 to 40 s on. autotest13's and autotest14's
//! actors that stayed put for minutes were, we infer, without a path: each
//! had its target 1,900 or more below or above in another cell.

use std::collections::HashMap;

use anyhow::{Context, Result, bail};
use esp::writer::{AttachedScript, Node, PropertyValue, RecordBuilder, write_plugin};
use esp::{FormId, LoadOrder, record_flags};
use papyrus::build::{Arg, FunctionBuilder, ScriptBuilder, auto_var, id, string};
use papyrus::pex::op;

use super::autotest2::{Npc, Vanilla, line, put};
use super::autotest4::{health, now, poll, put_num, report};
use super::autotest6::{ALWAYS, pldt};
use super::autotest8::{get_fn, is_none};
use super::autotest13::{WALK, actor_script, package};
use super::autotest14::{move_player, since, where_fn};
use super::places::{self, Interior, World};
use super::records::{self, Effect, Quest};
use super::script::{Code, num, on_update_with};
use super::venv::placement;
use super::{TestMod, own};

// Object ids: the quest, the spell, the chair, then by actor: its global,
// NPC, two packages, home and far markers; the places' entries; the actors.
const QUEST: u32 = 0x800;
const SPELL: u32 = 0x801;
const CHAIR: u32 = 0x802;
const GLOBALS: u32 = 0x810;
const NPCS: u32 = 0x830;
const PACKAGES: u32 = 0x850;
const HOMES: u32 = 0x880;
const FARS: u32 = 0x8A0;
const ENTRIES: u32 = 0x8C0;
const ACTORS: u32 = 0x8D0;
const NEXT_OBJECT_ID: u32 = 0x8F0;

const PLUGIN: &str = "VrmAutoTest16.esp";
const SCRIPT: &str = "VrmAutoTest16Script";
const ACTOR_SCRIPT: &str = "VrmAutoTest16ActorScript";
const PLAYER_REF: FormId = FormId(0x14);

/// Where actors stand, markers go and the player is sent.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Place {
    /// Where the player waits while actors elsewhere are read.
    Hub,
    Timed,
    Jump,
    /// Two cells east of `Jump`: out of jumping range, about level.
    JumpBeyond,
    /// Hillsides: the 3 by 3 cells around its grid.
    Steep,
    Unload,
    UnloadBeyond,
    Buffer,
    Palace,
    BluePalace,
    Inn,
}

const PLACES: [Place; 11] = [
    Place::Hub,
    Place::Timed,
    Place::Jump,
    Place::JumpBeyond,
    Place::Steep,
    Place::Unload,
    Place::UnloadBeyond,
    Place::Buffer,
    Place::Palace,
    Place::BluePalace,
    Place::Inn,
];

impl Place {
    fn index(self) -> usize {
        PLACES.iter().position(|p| *p == self).unwrap()
    }
    /// A Tamriel cell's grid (for `Steep`, the middle of its cells).
    fn grid(self) -> Option<(i32, i32)> {
        Some(match self {
            Place::Hub => (-28, -8),
            Place::Timed => (-37, 21),
            Place::Jump => (-20, 0),
            Place::JumpBeyond => (-18, 0),
            Place::Steep => (-10, -10),
            Place::Unload => (-8, 18),
            Place::UnloadBeyond => (-6, 18),
            Place::Buffer => (-30, 8),
            _ => return None,
        })
    }
    /// An interior's editor id and name.
    fn interior(self) -> Option<(&'static str, &'static str)> {
        Some(match self {
            Place::Palace => ("WindhelmPalaceoftheKings", "Palace of the Kings"),
            Place::BluePalace => ("SolitudeBluePalace", "Blue Palace"),
            Place::Inn => ("RiverwoodSleepingGiantInn", "Sleeping Giant Inn"),
            _ => return None,
        })
    }
    /// The Tamriel cells an actor's home may be in.
    fn home_grids(self) -> Vec<(i32, i32)> {
        let Some((x, y)) = self.grid() else {
            return Vec::new();
        };
        match self {
            Place::Steep => (-1..=1)
                .flat_map(|dx| (-1..=1).map(move |dy| (x + dx, y + dy)))
                .collect(),
            _ => vec![(x, y)],
        }
    }
}

/// Where an actor's package sends it.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Far {
    /// Nowhere: its one package keeps it home.
    Stay,
    /// In its home cell, this far (min, max) in XY, within 600 of its
    /// height: in jumping range.
    Near(f32, f32),
    /// In this place, 7,000 to 8,500 in XY and within 300 of its height:
    /// out of range, level.
    Level(Place),
    /// In jumping range (2,500 to 3,800 in XY) but this far (min, max)
    /// above or below.
    Steep(f32, f32),
    /// Across the interior it stands in.
    Inside,
    /// Sitting in our chair.
    Chair,
    /// Sandboxing within 512 of home (`DefaultSandboxEditorLocation512`).
    Sandbox,
}

impl Far {
    /// Whether a global of ours sends it.
    fn sent(self) -> bool {
        matches!(
            self,
            Far::Near(..) | Far::Level(_) | Far::Steep(..) | Far::Inside
        )
    }
}

struct Actor {
    label: &'static str,
    home: Place,
    far: Far,
    /// `EvaluatePackage` as it's sent.
    asked: bool,
}

const fn actor(label: &'static str, home: Place, far: Far, asked: bool) -> Actor {
    Actor {
        label,
        home,
        far,
        asked,
    }
}

/// In jumping range, about level.
const NEAR: Far = Far::Near(2500.0, 3200.0);

const ACTOR_LIST: &[Actor] = &[
    actor("Timed", Place::Timed, Far::Stay, false),
    actor("PassA", Place::Jump, NEAR, false),
    actor("PassB", Place::Jump, NEAR, false),
    actor("PassC", Place::Jump, NEAR, false),
    actor("FarAsked", Place::Jump, Far::Level(Place::JumpBeyond), true),
    actor(
        "FarUnasked",
        Place::Jump,
        Far::Level(Place::JumpBeyond),
        false,
    ),
    actor("High", Place::Steep, Far::Steep(1900.0, 4000.0), true),
    actor("MidHigh", Place::Steep, Far::Steep(1450.0, 1850.0), true),
    actor("MidLow", Place::Steep, Far::Steep(1000.0, 1350.0), true),
    actor("Leaving", Place::Unload, NEAR, true),
    actor(
        "LeavingFar",
        Place::Unload,
        Far::Level(Place::UnloadBeyond),
        true,
    ),
    actor("BufExt", Place::Buffer, Far::Near(3000.0, 3500.0), true),
    actor("BufIn1", Place::Palace, Far::Inside, true),
    actor("BufIn2", Place::BluePalace, Far::Inside, true),
    actor("Sitter", Place::Inn, Far::Chair, false),
    actor("Sandboxer", Place::Inn, Far::Sandbox, false),
    actor("WaitFar", Place::Jump, Far::Level(Place::JumpBeyond), false),
    actor("SleepNear", Place::Hub, NEAR, true),
    actor(
        "SleepFar",
        Place::Jump,
        Far::Level(Place::JumpBeyond),
        false,
    ),
    actor("Dead", Place::Jump, NEAR, true),
];

/// Timed's effect: Fortify Health 50 for this many seconds.
const EFFECT_SECONDS: u32 = 90;

/// How far apart spots are kept in Tamriel.
const APART: f32 = 150.0;

const JUMPERS: [&str; 8] = [
    "PassA",
    "PassB",
    "PassC",
    "FarAsked",
    "FarUnasked",
    "High",
    "MidHigh",
    "MidLow",
];
const STEEP: [&str; 3] = ["High", "MidHigh", "MidLow"];
const GOING_HOME: [&str; 3] = ["FarAsked", "FarUnasked", "PassA"];
const LEAVERS: [&str; 2] = ["Leaving", "LeavingFar"];
const RESTING: [&str; 2] = ["Sitter", "Sandboxer"];
const VISITED: [&str; 8] = [
    "PassA",
    "PassB",
    "PassC",
    "FarAsked",
    "FarUnasked",
    "WaitFar",
    "SleepFar",
    "Dead",
];

/// Seconds after the others PassB and PassC are sent off.
const STAGGER: [(&str, f32); 2] = [("PassB", 10.0), ("PassC", 20.0)];
/// Seconds the jumpers are watched.
const JUMP_SECONDS: f32 = 125.0;

const TESTS: &[(&str, &str)] = &[
    ("Timed", "Timed's effect as its cell unloads"),
    ("JumpsBefore", "Actors sent off unloaded: before"),
    ("Jumps", "Actors sent off unloaded: their moves"),
    ("JumpsAfter", "Actors sent off unloaded: after"),
    ("Home", "Three of them sent home, unasked"),
    ("Leaving", "Walkers whose cell unloads"),
    ("Buffer1", "Into an interior: the exterior left"),
    ("Buffer2", "Into another interior: both left"),
    ("Buffer3", "Out to an exterior: all three left"),
    ("Wait", "A wait in the inn"),
    ("ReEntry", "The inn left and come back to"),
    ("Sleep", "A sleep at the hub"),
    ("VisitLeaving", "The walkers visited"),
    (
        "VisitSteep",
        "The hillside actors asked again, then visited",
    ),
    ("VisitJump", "The jumpers visited"),
];

/// The boxes asking the user to do something.
const WAIT_PROMPT: &str = "vibe_rm autotest16: open the Wait menu (T) now and wait two hours. \
     The test goes on by itself afterwards.";
const SLEEP_PROMPT: &str = "vibe_rm autotest16: a bedroll is beside you. Sleep in it for two \
     hours. The test goes on by itself afterwards.";

fn k_of(label: &str) -> usize {
    ACTOR_LIST
        .iter()
        .position(|a| a.label == label)
        .unwrap_or_else(|| panic!("no actor {label}"))
}

fn entry(p: Place) -> Arg {
    auto_var(&format!("Entry{}", p.index()))
}

fn home(k: usize) -> Arg {
    auto_var(&format!("Home{k}"))
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

/// `label` read into `r` (`Where`: from its far marker, from home, cell,
/// 3D, package), after `text`, a line.
fn put_where(c: &mut Code, r: &Arg, text: &str, label: &str) {
    let k = k_of(label);
    let s = c.own(
        "Where",
        "String",
        [
            ((ACTORS + k as u32) as i32).into(),
            home(k),
            auto_var(&format!("Far{k}")),
        ],
    );
    put(c, r, text, s);
    line(c, r, "\n");
}

/// Each of `labels` read, a line each.
fn put_all(c: &mut Code, r: &Arg, labels: &[&str]) {
    for label in labels {
        put_where(c, r, &format!("  {label}: "), label);
    }
}

/// `label`'s global set to 1 (sent off, with `EvaluatePackage` when it's
/// asked) or 0 (sent home, unasked).
fn send(c: &mut Code, label: &str, on: bool) {
    let k = k_of(label);
    let a = &ACTOR_LIST[k];
    assert!(a.far.sent(), "{label} has no global");
    let value: f32 = if on { 1.0 } else { 0.0 };
    c.method(
        "GlobalVariable",
        auto_var(&format!("Go{k}")),
        "SetValue",
        [value.into()],
    );
    if on && a.asked {
        with_actor(c, k, |c, a| {
            c.method("Actor", a, "EvaluatePackage", []);
        });
    }
}

/// The game time in hours (not wrapping at midnight).
fn hours(c: &mut Code) -> Arg {
    let game = c.global("Utility", "GetCurrentGameTime", []);
    c.calc(op::FMUL, "Float", game, 24.0f32.into())
}

/// The player moved to `p`, then up to 60 s for the 3D of every one of
/// `labels`; `r` gets how long that took, a line.
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
    line(c, r, " s (-1: not in 60 s)\n");
}

/// The per-actor variables tracking uses: distance from home at the last
/// sample, moves counted, first and last move (s since `T0`), largest
/// step, 3D at the start and when it first changed.
fn track_vars(k: usize) -> [(String, &'static str); 7] {
    [
        (format!("D{k}"), "Float"),
        (format!("Moves{k}"), "Int"),
        (format!("First{k}"), "Float"),
        (format!("Last{k}"), "Float"),
        (format!("Step{k}"), "Float"),
        (format!("Was3D{k}"), "Bool"),
        (format!("Flip{k}"), "Float"),
    ]
}

/// Tracking of `labels` started: each one's distance from home and 3D
/// now, nothing counted.
fn track_start(c: &mut Code, labels: &[&str]) {
    for label in labels {
        let k = k_of(label);
        c.assign(id(&format!("Moves{k}")), 0.into());
        for v in ["First", "Last", "Flip"] {
            c.assign(id(&format!("{v}{k}")), (-1.0f32).into());
        }
        c.assign(id(&format!("Step{k}")), 0.0f32.into());
        with_actor(c, k, |c, a| {
            let d = c.method("ObjectReference", a.clone(), "GetDistance", [home(k)]);
            c.assign(id(&format!("D{k}")), d);
            let l = c.method("ObjectReference", a, "Is3DLoaded", []);
            c.assign(id(&format!("Was3D{k}")), l);
        });
    }
}

/// One sample of `labels` at `t`: a change of more than 2 in an actor's
/// distance from home counts as a move; with `with_3d`, the first change
/// of its 3D is kept.
fn track_sample(c: &mut Code, labels: &[&str], t: &Arg, with_3d: bool) {
    for label in labels {
        let k = k_of(label);
        with_actor(c, k, |c, a| {
            let last = id(&format!("D{k}"));
            let d = c.method("ObjectReference", a.clone(), "GetDistance", [home(k)]);
            let delta = c.calc(op::FSUB, "Float", d.clone(), last.clone());
            let sq = c.calc(op::FMUL, "Float", delta.clone(), delta.clone());
            let still = c.calc(op::CMP_LT, "Bool", sq, 4.0f32.into());
            let done = c.label();
            c.jmpt(still, done);
            let moves = id(&format!("Moves{k}"));
            c.op(op::IADD, [moves.clone(), moves, 1.into()]);
            let first = id(&format!("First{k}"));
            let unset = c.calc(op::CMP_LT, "Bool", first.clone(), 0.0f32.into());
            let set = c.label();
            c.jmpf(unset, set).assign(first, t.clone()).bind(set);
            c.assign(id(&format!("Last{k}")), t.clone());
            let negative = c.calc(op::CMP_LT, "Bool", delta.clone(), 0.0f32.into());
            let positive = c.label();
            c.jmpf(negative, positive)
                .op(op::FNEG, [delta.clone(), delta.clone()])
                .bind(positive);
            let biggest = id(&format!("Step{k}"));
            let bigger = c.calc(op::CMP_GT, "Bool", delta.clone(), biggest.clone());
            let small = c.label();
            c.jmpf(bigger, small).assign(biggest, delta).bind(small);
            c.assign(last, d);
            c.bind(done);
            if with_3d {
                let flip = id(&format!("Flip{k}"));
                let seen = c.label();
                let unseen = c.calc(op::CMP_LT, "Bool", flip.clone(), 0.0f32.into());
                c.jmpf(unseen, seen);
                let l = c.method("ObjectReference", a, "Is3DLoaded", []);
                let same = c.calc(op::CMP_EQ, "Bool", l, id(&format!("Was3D{k}")));
                c.jmpt(same, seen).assign(flip, t.clone()).bind(seen);
            }
        });
    }
}

/// `label`'s tracking summed up, a line.
fn put_track(c: &mut Code, r: &Arg, label: &str, with_3d: bool) {
    let k = k_of(label);
    put(c, r, &format!("  {label}: "), id(&format!("Moves{k}")));
    put_num(c, r, " moves, ", id(&format!("First{k}")));
    put_num(c, r, " to ", id(&format!("Last{k}")));
    put_num(c, r, " s, largest ", id(&format!("Step{k}")));
    if with_3d {
        let was = id(&format!("Was3D{k}"));
        put(c, r, "; 3D ", was);
        put_num(c, r, " changed at ", id(&format!("Flip{k}")));
    }
    line(c, r, "\n");
}

/// Something done once, `at` seconds after `T0`.
type Event<'a> = (f32, Box<dyn Fn(&mut Code) + 'a>);

/// `labels` sampled every `step` until `seconds` after `T0`, `events` (in
/// order of time) done as they come due. `tag` names the loop's locals.
fn track(
    c: &mut Code,
    tag: &str,
    labels: &[&str],
    seconds: f32,
    step: f32,
    with_3d: bool,
    events: Vec<Event>,
) {
    let ev = c.local(&format!("ev{tag}"), "Int");
    c.assign(ev.clone(), 0.into());
    let (top, out) = (c.label(), c.label());
    c.bind(top);
    c.wait(step);
    let t = since(c, "T0");
    track_sample(c, labels, &t, with_3d);
    for (j, (at, event)) in events.iter().enumerate() {
        let next = c.label();
        let this = c.calc(op::CMP_EQ, "Bool", ev.clone(), (j as i32).into());
        c.jmpf(this, next);
        let due = c.calc(op::CMP_GE, "Bool", t.clone(), (*at).into());
        c.jmpf(due, next);
        event(c);
        c.op(op::IADD, [ev.clone(), ev.clone(), 1.into()]);
        c.bind(next);
    }
    let over = c.calc(op::CMP_GE, "Bool", t, seconds.into());
    c.jmpt(over, out);
    c.jmp(top);
    c.bind(out);
}

/// `T0` set to now.
fn mark(c: &mut Code) {
    let t = now(c);
    c.assign(id("T0"), t);
}

/// An event at `at` reading `labels` into `r`, under the time.
fn read_at<'a>(r: &'a Arg, at: f32, labels: &'a [&'a str]) -> Event<'a> {
    (
        at,
        Box::new(move |c: &mut Code| {
            let t = since(c, "T0");
            put_num(c, r, "at ", t);
            line(c, r, " s:\n");
            put_all(c, r, labels);
        }),
    )
}

/// Step 1: Timed given its effect, then the player away to the hub; Dead
/// killed while unloaded.
fn test_timed() -> FunctionBuilder {
    let (mut c, r) = report();
    let dead = k_of("Dead");
    with_actor(&mut c, dead, |c, a| {
        let hp = health(c, &a);
        let dmg = c.calc(op::FMUL, "Float", hp, 2.0f32.into());
        c.method(
            "Actor",
            a.clone(),
            "DamageActorValue",
            [string("Health"), dmg],
        );
        let is = c.method("Actor", a, "IsDead", []);
        put(c, &r, "Dead killed unloaded: dead ", is);
        line(c, &r, "\n");
    });
    line(&mut c, &r, "at Timed's cell; ");
    visit(&mut c, &r, Place::Timed, &["Timed"]);
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
    mark(&mut c);
    move_player(&mut c, entry(Place::Hub));
    let moved = since(&mut c, "T0");
    put_num(&mut c, &r, "  the player moved to the hub, taking ", moved);
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
            let t = since(c, "T0");
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
    line(&mut c, &r, " s after the move began (-1: not in 30 s)\n");
    c.finish(r)
}

/// Step 2: the jumpers read, tracking started, and all but PassB and
/// PassC sent off.
fn test_jumps_before() -> FunctionBuilder {
    let (mut c, r) = report();
    line(&mut c, &r, "from the hub, before:\n");
    put_all(&mut c, &r, &JUMPERS);
    track_start(&mut c, &JUMPERS);
    mark(&mut c);
    for label in JUMPERS {
        if !STAGGER.iter().any(|(l, _)| *l == label) {
            send(&mut c, label, true);
        }
    }
    let t = since(&mut c, "T0");
    put_num(&mut c, &r, "sent off (taking ", t);
    line(&mut c, &r, " s); PassB 10 s later, PassC 20 s later\n");
    c.finish(r)
}

/// Step 3: the jumpers watched for `JUMP_SECONDS`.
fn test_jumps() -> FunctionBuilder {
    let (mut c, r) = report();
    let events: Vec<Event> = STAGGER
        .iter()
        .map(|&(label, at)| -> Event { (at, Box::new(move |c: &mut Code| send(c, label, true))) })
        .collect();
    track(&mut c, "Jumps", &JUMPERS, JUMP_SECONDS, 0.25, false, events);
    line(
        &mut c,
        &r,
        &format!(
            "each one's changes of distance from home over {JUMP_SECONDS} s (s since the first sending):\n"
        ),
    );
    for label in JUMPERS {
        put_track(&mut c, &r, label, false);
    }
    c.finish(r)
}

/// Step 4: the jumpers read after.
fn test_jumps_after() -> FunctionBuilder {
    let (mut c, r) = report();
    let t = since(&mut c, "T0");
    put_num(&mut c, &r, "at ", t);
    line(&mut c, &r, " s:\n");
    put_all(&mut c, &r, &JUMPERS);
    c.finish(r)
}

/// Step 5: FarAsked, FarUnasked and PassA sent home unasked, watched for
/// 120 s.
fn test_home() -> FunctionBuilder {
    let (mut c, r) = report();
    track_start(&mut c, &GOING_HOME);
    mark(&mut c);
    for label in GOING_HOME {
        send(&mut c, label, false);
    }
    line(&mut c, &r, "sent home (globals 0, no EvaluatePackage):\n");
    let far = ["FarAsked", "FarUnasked"];
    let events = vec![
        read_at(&r, 30.0, &far),
        read_at(&r, 60.0, &far),
        read_at(&r, 90.0, &far),
    ];
    track(&mut c, "Home", &GOING_HOME, 120.0, 0.25, false, events);
    line(&mut c, &r, "changes of distance from home:\n");
    for label in GOING_HOME {
        put_track(&mut c, &r, label, false);
    }
    line(&mut c, &r, "at 120 s:\n");
    put_all(&mut c, &r, &GOING_HOME);
    c.finish(r)
}

/// Step 6: Leaving and LeavingFar sent off loaded, then the player away to
/// the hub.
fn test_leaving() -> FunctionBuilder {
    let (mut c, r) = report();
    line(&mut c, &r, "at their cell; ");
    visit(&mut c, &r, Place::Unload, &LEAVERS);
    for label in LEAVERS {
        send(&mut c, label, true);
    }
    c.wait(5.0);
    line(&mut c, &r, "5 s after they were sent:\n");
    put_all(&mut c, &r, &LEAVERS);
    track_start(&mut c, &LEAVERS);
    mark(&mut c);
    move_player(&mut c, entry(Place::Hub));
    let t = since(&mut c, "T0");
    put_num(&mut c, &r, "the player moved to the hub, taking ", t);
    line(&mut c, &r, " s; 60 s of changes (s since leaving):\n");
    track(&mut c, "Leaving", &LEAVERS, 60.0, 0.25, true, Vec::new());
    for label in LEAVERS {
        put_track(&mut c, &r, label, true);
    }
    line(&mut c, &r, "then:\n");
    put_all(&mut c, &r, &LEAVERS);
    c.finish(r)
}

/// One step of the buffer chain: the player first moved to `start`'s place
/// when given (waiting for its actor's 3D), `label` (loaded where the
/// player is) sent off, then the player moved to `to` (waiting for
/// `wait_for`'s 3D), and `watched` tracked for 25 s from the move.
fn buffer_step(
    start: Option<(Place, &str)>,
    label: &str,
    to: Place,
    wait_for: &str,
    watched: &[&str],
) -> FunctionBuilder {
    let (mut c, r) = report();
    if let Some((p, a)) = start {
        line(&mut c, &r, &format!("at {a}'s cell; "));
        visit(&mut c, &r, p, &[a]);
    }
    send(&mut c, label, true);
    c.wait(3.0);
    put_where(
        &mut c,
        &r,
        &format!("{label} 3 s after it was sent: "),
        label,
    );
    track_start(&mut c, watched);
    mark(&mut c);
    line(&mut c, &r, "the player moved on; ");
    visit(&mut c, &r, to, &[wait_for]);
    let events = vec![read_at(&r, 10.0, watched)];
    track(&mut c, "Buffer", watched, 25.0, 0.25, true, events);
    line(&mut c, &r, "changes over 25 s (s since the move began):\n");
    for w in watched {
        put_track(&mut c, &r, w, true);
    }
    line(&mut c, &r, "at 25 s:\n");
    put_all(&mut c, &r, watched);
    c.finish(r)
}

/// Step 10: the inn: Sitter seated, then the user waits two hours.
fn test_wait() -> FunctionBuilder {
    let (mut c, r) = report();
    line(&mut c, &r, "in the inn; ");
    visit(&mut c, &r, Place::Inn, &RESTING);
    let sitter = k_of("Sitter");
    let seated = poll(&mut c, "seated", 0.5, 60.0, |c, hit| {
        with_actor(c, sitter, |c, a| {
            let s = c.method("Actor", a, "GetSitState", []);
            let sat = c.calc(op::CMP_EQ, "Bool", s, 3.into());
            c.jmpt(sat, hit);
        });
    });
    put_num(&mut c, &r, "Sitter seated after ", seated);
    line(&mut c, &r, " s (-1: not in 60 s):\n");
    keep_spots(&mut c);
    put_seats(&mut c, &r);
    send(&mut c, "WaitFar", true);
    c.wait(2.0);
    put_where(&mut c, &r, "WaitFar 2 s after it was sent: ", "WaitFar");
    pass_time(&mut c, &r, WAIT_PROMPT, 240.0);
    line(&mut c, &r, "after the wait:\n");
    put_seats(&mut c, &r);
    put_all(&mut c, &r, &["WaitFar", "High", "MidHigh"]);
    c.wait(10.0);
    line(&mut c, &r, "10 s later:\n");
    put_seats(&mut c, &r);
    put_all(&mut c, &r, &["WaitFar"]);
    c.finish(r)
}

/// Sitter's and Sandboxer's positions kept, to measure moves from.
fn keep_spots(c: &mut Code) {
    for label in RESTING {
        let k = k_of(label);
        with_actor(c, k, |c, a| {
            let x = c.method("ObjectReference", a.clone(), "GetPositionX", []);
            let y = c.method("ObjectReference", a, "GetPositionY", []);
            c.assign(id(&format!("SX{k}")), x)
                .assign(id(&format!("SY{k}")), y);
        });
    }
}

/// Sitter's and Sandboxer's sit state, distance from the chair (Sitter)
/// or home (Sandboxer), move from the kept spot, and 3D, a line each.
fn put_seats(c: &mut Code, r: &Arg) {
    for label in RESTING {
        let k = k_of(label);
        line(c, r, &format!("  {label}: "));
        let a = get(c, k);
        let missing = is_none(c, a.clone());
        let (found, done) = (c.label(), c.label());
        c.jmpf(missing, found);
        line(c, r, "not in memory");
        c.jmp(done);
        c.bind(found);
        let s = c.method("Actor", a.clone(), "GetSitState", []);
        put(c, r, "sit state ", s);
        let d = c.method(
            "ObjectReference",
            a.clone(),
            "GetDistance",
            [auto_var(&format!("Far{k}"))],
        );
        let what = if label == "Sitter" { "chair" } else { "home" };
        put_num(c, r, &format!(", {what} "), d);
        let x = c.method("ObjectReference", a.clone(), "GetPositionX", []);
        let y = c.method("ObjectReference", a.clone(), "GetPositionY", []);
        let dx = c.calc(op::FSUB, "Float", x, id(&format!("SX{k}")));
        let dy = c.calc(op::FSUB, "Float", y, id(&format!("SY{k}")));
        let dx2 = c.calc(op::FMUL, "Float", dx.clone(), dx);
        let dy2 = c.calc(op::FMUL, "Float", dy.clone(), dy);
        let d2 = c.calc(op::FADD, "Float", dx2, dy2);
        let moved = c.global("Math", "sqrt", [d2]);
        put_num(c, r, ", moved ", moved);
        let l = c.method("ObjectReference", a, "Is3DLoaded", []);
        put(c, r, ", 3D ", l);
        c.bind(done);
        line(c, r, "\n");
    }
}

/// The user asked (`prompt`) to pass time; up to `seconds` for the clock
/// to jump by more than half an hour. `r` gets the jump, a line.
fn pass_time(c: &mut Code, r: &Arg, prompt: &str, seconds: f32) {
    c.global("Debug", "MessageBox", [string(prompt)]);
    let last_g = c.local("lastG", "Float");
    let h = hours(c);
    c.assign(last_g.clone(), h);
    let passed = poll(c, "passed", 0.5, seconds, |c, hit| {
        let h = hours(c);
        let dh = c.calc(op::FSUB, "Float", h.clone(), last_g.clone());
        let jumped = c.calc(op::CMP_GT, "Bool", dh, 0.5f32.into());
        c.jmpt(jumped, hit);
        c.assign(last_g.clone(), h);
    });
    let h = hours(c);
    let jump = c.calc(op::FSUB, "Float", h, last_g);
    put_num(c, r, "time passed seen after ", passed);
    put_num(
        c,
        r,
        &format!(" s (-1: not in {seconds} s), the clock moved "),
        jump,
    );
    line(c, r, " hours\n");
}

/// Step 11: the inn left for the hub and then Timed's cell, and come
/// back to.
fn test_re_entry() -> FunctionBuilder {
    let (mut c, r) = report();
    keep_spots(&mut c);
    line(&mut c, &r, "before leaving:\n");
    put_seats(&mut c, &r);
    let sitter = k_of("Sitter");
    for (p, name) in [(Place::Hub, "the hub"), (Place::Timed, "Timed's cell")] {
        move_player(&mut c, entry(p));
        let gone = poll(&mut c, "gone", 0.25, 20.0, |c, hit| {
            with_actor(c, sitter, |c, a| {
                let l = c.method("ObjectReference", a, "Is3DLoaded", []);
                c.jmpf(l, hit);
            });
        });
        put_num(
            &mut c,
            &r,
            &format!("at {name}: Sitter's 3D gone after "),
            gone,
        );
        line(&mut c, &r, " s (-1: not in 20 s)\n");
    }
    c.wait(10.0);
    line(&mut c, &r, "back in the inn; ");
    visit(&mut c, &r, Place::Inn, &RESTING);
    put_seats(&mut c, &r);
    c.wait(3.0);
    line(&mut c, &r, "3 s later:\n");
    put_seats(&mut c, &r);
    c.wait(10.0);
    line(&mut c, &r, "10 s later:\n");
    put_seats(&mut c, &r);
    c.finish(r)
}

/// Step 12: at the hub, SleepNear (loaded) and SleepFar sent off, then
/// the user sleeps two hours in a bedroll put beside them.
fn test_sleep() -> FunctionBuilder {
    let (mut c, r) = report();
    line(&mut c, &r, "at the hub; ");
    visit(&mut c, &r, Place::Hub, &["SleepNear"]);
    send(&mut c, "SleepNear", true);
    send(&mut c, "SleepFar", true);
    c.wait(2.0);
    line(&mut c, &r, "2 s after they were sent:\n");
    put_all(&mut c, &r, &["SleepNear", "SleepFar"]);
    let bed = c.method(
        "ObjectReference",
        auto_var("Player"),
        "PlaceAtMe",
        [auto_var("Bedroll"), 1.into(), false.into(), false.into()],
    );
    c.assign(id("Bed"), bed);
    c.method(
        "ObjectReference",
        id("Bed"),
        "MoveTo",
        [
            auto_var("Player"),
            120.0f32.into(),
            0.0f32.into(),
            0.0f32.into(),
            false.into(),
        ],
    );
    pass_time(&mut c, &r, SLEEP_PROMPT, 300.0);
    line(&mut c, &r, "after the sleep:\n");
    put_all(&mut c, &r, &["SleepNear", "SleepFar", "High", "MidHigh"]);
    c.wait(10.0);
    line(&mut c, &r, "10 s later:\n");
    put_all(&mut c, &r, &["SleepNear", "SleepFar"]);
    c.method("ObjectReference", id("Bed"), "Disable", [false.into()]);
    c.method("ObjectReference", id("Bed"), "Delete", []);
    c.finish(r)
}

/// Step 13: Leaving and LeavingFar visited.
fn test_visit_leaving() -> FunctionBuilder {
    let (mut c, r) = report();
    line(&mut c, &r, "at their cell; ");
    visit(&mut c, &r, Place::Unload, &LEAVERS);
    put_all(&mut c, &r, &LEAVERS);
    c.wait(3.0);
    line(&mut c, &r, "3 s later:\n");
    put_all(&mut c, &r, &LEAVERS);
    c.finish(r)
}

/// Step 14: the hillside actors asked again from afar, then visited.
fn test_visit_steep() -> FunctionBuilder {
    let (mut c, r) = report();
    for label in STEEP {
        with_actor(&mut c, k_of(label), |c, a| {
            c.method("Actor", a, "EvaluatePackage", []);
        });
    }
    c.wait(5.0);
    line(&mut c, &r, "5 s after EvaluatePackage again, from afar:\n");
    put_all(&mut c, &r, &STEEP);
    line(&mut c, &r, "at their cells; ");
    visit(&mut c, &r, Place::Steep, &STEEP);
    put_all(&mut c, &r, &STEEP);
    c.wait(3.0);
    line(&mut c, &r, "3 s later:\n");
    put_all(&mut c, &r, &STEEP);
    c.finish(r)
}

/// Step 15: Dead sent off, then the jumpers visited.
fn test_visit_jump() -> FunctionBuilder {
    let (mut c, r) = report();
    send(&mut c, "Dead", true);
    c.wait(3.0);
    line(&mut c, &r, "at the jumpers' cell; ");
    visit(&mut c, &r, Place::Jump, &VISITED);
    put_all(&mut c, &r, &VISITED);
    with_actor(&mut c, k_of("Dead"), |c, a| {
        let is = c.method("Actor", a, "IsDead", []);
        put(c, &r, "Dead dead ", is);
        line(c, &r, "\n");
    });
    c.finish(r)
}

fn scripts() -> Result<Vec<(String, Vec<u8>)>> {
    let mut s = ScriptBuilder::new(SCRIPT, "Quest");
    s.auto_property("Player", "Actor");
    s.auto_property("Spell", "Spell");
    s.auto_property("Effect", "MagicEffect");
    s.auto_property("Bedroll", "Form");
    for p in PLACES {
        s.auto_property(&format!("Entry{}", p.index()), "ObjectReference");
    }
    for (k, a) in ACTOR_LIST.iter().enumerate() {
        s.auto_property(&format!("Home{k}"), "ObjectReference");
        s.auto_property(&format!("Far{k}"), "ObjectReference");
        if a.far.sent() {
            s.auto_property(&format!("Go{k}"), "GlobalVariable");
        }
        for (name, ty) in track_vars(k) {
            let init = match ty {
                "Int" => 0.into(),
                "Bool" => false.into(),
                _ => 0.0f32.into(),
            };
            s.variable(&name, ty, init);
        }
    }
    for label in RESTING {
        let k = k_of(label);
        s.variable(&format!("SX{k}"), "Float", 0.0f32.into());
        s.variable(&format!("SY{k}"), "Float", 0.0f32.into());
    }
    s.variable("Ran", "Bool", false.into());
    s.variable("T0", "Float", 0.0f32.into());
    s.variable("Bed", "ObjectReference", Arg::None);
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
                "autotest16",
                "vibe_rm autotest16 running: about fifteen minutes. Stay at the keyboard: it \
                 will ask you to wait two hours, and to sleep two hours.",
                TESTS,
            ),
        )
        .function("Num", num())
        .function("Get", get_fn(PLUGIN))
        .function("Where", where_fn())
        .function("TestTimed", test_timed())
        .function("TestJumpsBefore", test_jumps_before())
        .function("TestJumps", test_jumps())
        .function("TestJumpsAfter", test_jumps_after())
        .function("TestHome", test_home())
        .function("TestLeaving", test_leaving())
        .function(
            "TestBuffer1",
            buffer_step(
                Some((Place::Buffer, "BufExt")),
                "BufExt",
                Place::Palace,
                "BufIn1",
                &["BufExt"],
            ),
        )
        .function(
            "TestBuffer2",
            buffer_step(
                None,
                "BufIn1",
                Place::BluePalace,
                "BufIn2",
                &["BufExt", "BufIn1"],
            ),
        )
        .function(
            "TestBuffer3",
            buffer_step(
                None,
                "BufIn2",
                Place::Hub,
                "SleepNear",
                &["BufExt", "BufIn1", "BufIn2"],
            ),
        )
        .function("TestWait", test_wait())
        .function("TestReEntry", test_re_entry())
        .function("TestSleep", test_sleep())
        .function("TestVisitLeaving", test_visit_leaving())
        .function("TestVisitSteep", test_visit_steep())
        .function("TestVisitJump", test_visit_jump());
    let actors = actor_script(ACTOR_SCRIPT)?;
    Ok(vec![
        (SCRIPT.to_owned(), papyrus::pex::write(&s.build()?)?),
        (
            ACTOR_SCRIPT.to_owned(),
            papyrus::pex::write(&actors.build()?)?,
        ),
    ])
}

fn xy_distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// The nearest of `chosen` to `p`, in XY.
fn clearance(p: [f32; 3], chosen: &[[f32; 3]]) -> f32 {
    chosen
        .iter()
        .map(|c| xy_distance(p, *c))
        .fold(f32::MAX, f32::min)
}

/// The positions: each place's cell (for `Steep`, its middle) and entry,
/// each actor's home and far marker (none for those staying or
/// sandboxing), and the chair.
struct Layout {
    cells: Vec<FormId>,
    entries: Vec<[f32; 3]>,
    homes: Vec<[f32; 3]>,
    fars: Vec<Option<[f32; 3]>>,
    chair: [f32; 3],
}

/// Tamriel's cells' floors, read as asked for.
struct Floors<'a> {
    lo: &'a LoadOrder,
    tamriel: FormId,
    by_grid: HashMap<(i32, i32), Vec<[f32; 3]>>,
}

impl Floors<'_> {
    fn cell(&self, grid: (i32, i32)) -> Result<FormId> {
        self.lo
            .world(self.tamriel)
            .and_then(|w| w.cells.get(&grid))
            .copied()
            .with_context(|| format!("no Tamriel cell at {grid:?}"))
    }

    fn of(&mut self, grids: &[(i32, i32)]) -> Result<Vec<[f32; 3]>> {
        let mut out = Vec::new();
        for &g in grids {
            if !self.by_grid.contains_key(&g) {
                let cell = self.cell(g)?;
                let f = places::floors(self.lo, cell).unwrap_or_default();
                self.by_grid.insert(g, f);
            }
            out.extend(self.by_grid[&g].iter().copied());
        }
        Ok(out)
    }
}

/// The floor of `floors` nearest their middle.
fn middle(floors: &[[f32; 3]]) -> Option<[f32; 3]> {
    let n = floors.len() as f32;
    let m = [0, 1].map(|i| floors.iter().map(|f| f[i]).sum::<f32>() / n);
    floors
        .iter()
        .min_by(|a, b| {
            xy_distance(**a, [m[0], m[1], 0.0]).total_cmp(&xy_distance(**b, [m[0], m[1], 0.0]))
        })
        .copied()
}

/// Where `cell`'s own references are (from their `DATA`), in XY.
fn references(lo: &LoadOrder, cell: FormId) -> Result<Vec<[f32; 3]>> {
    let index = lo.cell(cell).context("not a cell")?;
    let mut out = Vec::new();
    for &r in index.persistent.iter().chain(&index.temporary) {
        let Some(d) = lo
            .get(r)
            .and_then(|rec| rec.get(b"DATA").map(|d| d.to_vec()))
        else {
            continue;
        };
        if d.len() >= 12 {
            let f = |o: usize| f32::from_le_bytes(d[o..o + 4].try_into().unwrap());
            out.push([f(0), f(4), f(8)]);
        }
    }
    Ok(out)
}

fn layout(lo: &LoadOrder, v: &Vanilla) -> Result<Layout> {
    let tamriel = v.id("Tamriel")?;
    let mut floors = Floors {
        lo,
        tamriel,
        by_grid: HashMap::new(),
    };
    let mut cells = Vec::new();
    let mut entries = Vec::new();
    // Interiors' spots, by place: entry and pairs.
    let mut inside: HashMap<usize, places::Spots> = HashMap::new();
    for p in PLACES {
        match (p.grid(), p.interior()) {
            (Some(g), _) => {
                cells.push(floors.cell(g)?);
                let f = floors.of(&[g])?;
                entries.push(middle(&f).with_context(|| format!("no floor in {p:?}"))?);
            }
            (None, Some((edid, _))) => {
                let cell = v.id(edid)?;
                cells.push(cell);
                let homes = ACTOR_LIST.iter().filter(|a| a.home == p).count();
                let s = places::spots(lo, cell, homes, (600.0, 1200.0), 100.0)
                    .or_else(|_| places::spots(lo, cell, homes, (300.0, 900.0), 100.0))
                    .with_context(|| format!("spots in {p:?}"))?;
                entries.push(s.entry);
                inside.insert(p.index(), s);
            }
            _ => unreachable!(),
        }
    }
    let mut chosen: Vec<[f32; 3]> = PLACES
        .iter()
        .filter(|p| p.grid().is_some())
        .map(|p| entries[p.index()])
        .collect();
    let mut used = vec![0usize; PLACES.len()];
    let mut homes = Vec::new();
    let mut fars = Vec::new();
    for a in ACTOR_LIST {
        if a.home.grid().is_none() {
            let i = a.home.index();
            let (spot, pair) = inside[&i].pairs[used[i]];
            used[i] += 1;
            homes.push(spot);
            fars.push((a.far == Far::Inside).then_some(pair));
            continue;
        }
        let mut candidates = floors.of(&a.home.home_grids())?;
        candidates.sort_by(|x, y| clearance(*y, &chosen).total_cmp(&clearance(*x, &chosen)));
        let targets = match a.far {
            Far::Near(..) => floors.of(&a.home.home_grids())?,
            Far::Level(p) => floors.of(&[p.grid().unwrap()])?,
            Far::Steep(..) => {
                let (x, y) = a.home.grid().unwrap();
                let grids: Vec<_> = (-2..=2)
                    .flat_map(|dx| (-2..=2).map(move |dy| (x + dx, y + dy)))
                    .collect();
                floors.of(&grids)?
            }
            _ => Vec::new(),
        };
        let fits = |h: [f32; 3], t: [f32; 3]| {
            let (xy, dz) = (xy_distance(h, t), (t[2] - h[2]).abs());
            match a.far {
                Far::Near(lo, hi) => (lo..=hi).contains(&xy) && dz <= 600.0,
                Far::Level(_) => (7000.0..=8500.0).contains(&xy) && dz <= 300.0,
                Far::Steep(lo, hi) => (2500.0..=3800.0).contains(&xy) && (lo..=hi).contains(&dz),
                _ => false,
            }
        };
        let mut found = None;
        for h in candidates {
            if clearance(h, &chosen) < APART {
                break;
            }
            if !a.far.sent() {
                found = Some((h, None));
                break;
            }
            let mut with = chosen.clone();
            with.push(h);
            let t = targets
                .iter()
                .filter(|t| fits(h, **t))
                .filter(|t| clearance(**t, &with) >= APART)
                .max_by(|x, y| clearance(**x, &with).total_cmp(&clearance(**y, &with)));
            if let Some(t) = t {
                found = Some((h, Some(*t)));
                break;
            }
        }
        let Some((h, t)) = found else {
            bail!(
                "no spot for {} in {:?} with its marker {:?}",
                a.label,
                a.home,
                a.far
            );
        };
        chosen.push(h);
        if let Some(t) = t {
            chosen.push(t);
        }
        homes.push(h);
        fars.push(t);
    }
    // The chair: on the inn's floor as far from its own references as can
    // be, Sitter's home within 150 to 350 of it; Sandboxer's home 150 from
    // both.
    let inn = cells[Place::Inn.index()];
    let refs = references(lo, inn)?;
    let mut inn_floors = places::floors(lo, inn)?;
    inn_floors.sort_by(|x, y| clearance(*y, &refs).total_cmp(&clearance(*x, &refs)));
    let entry = entries[Place::Inn.index()];
    let mut seats = None;
    for chair in &inn_floors {
        if xy_distance(*chair, entry) < 150.0 {
            continue;
        }
        let sitter = inn_floors
            .iter()
            .filter(|f| (150.0..=350.0).contains(&xy_distance(**f, *chair)))
            .filter(|f| xy_distance(**f, entry) >= 100.0)
            .max_by(|x, y| clearance(**x, &refs).total_cmp(&clearance(**y, &refs)));
        let Some(sitter) = sitter else { continue };
        let taken = [*chair, *sitter, entry];
        let sandboxer = inn_floors
            .iter()
            .filter(|f| clearance(**f, &taken) >= 150.0)
            .max_by(|x, y| clearance(**x, &refs).total_cmp(&clearance(**y, &refs)));
        if let Some(sandboxer) = sandboxer {
            seats = Some((*chair, *sitter, *sandboxer));
            break;
        }
    }
    let (chair, sitter, sandboxer) = seats.context("no room in the inn for the chair")?;
    homes[k_of("Sitter")] = sitter;
    homes[k_of("Sandboxer")] = sandboxer;
    Ok(Layout {
        cells,
        entries,
        homes,
        fars,
        chair,
    })
}

pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    let v = Vanilla { lo };
    let template = v.id("WEMerchantMiscMaleCommoner")?;
    let race = v.id("ImperialRace")?;
    let class = v.id("Citizen")?;
    let heading_marker = v.id("XMarkerHeading")?;
    let effect = v.id("AlchFortifyHealth")?;
    let sandbox = v.id("DefaultSandboxEditorLocation512")?;
    let tamriel = v.id("Tamriel")?;
    let persistent = record_flags::PERSISTENT;
    let at = layout(lo, &v)?;

    let object = PropertyValue::Object;
    let mut properties = vec![
        ("Player".to_owned(), object(PLAYER_REF)),
        ("Spell".to_owned(), object(own(SPELL))),
        ("Effect".to_owned(), object(effect)),
        ("Bedroll".to_owned(), object(v.id("Bedroll01")?)),
    ];
    for p in PLACES {
        let i = p.index();
        properties.push((format!("Entry{i}"), object(own(ENTRIES + i as u32))));
    }
    // Far markers: the far marker, the chair, or home.
    let far_ref = |k: usize| match (ACTOR_LIST[k].far, at.fars[k]) {
        (Far::Chair, _) => own(CHAIR),
        (_, Some(_)) => own(FARS + k as u32),
        (_, None) => own(HOMES + k as u32),
    };
    for (k, a) in ACTOR_LIST.iter().enumerate() {
        properties.push((format!("Home{k}"), object(own(HOMES + k as u32))));
        properties.push((format!("Far{k}"), object(far_ref(k))));
        if a.far.sent() {
            properties.push((format!("Go{k}"), object(own(GLOBALS + k as u32))));
        }
    }

    let globals: Vec<RecordBuilder> = ACTOR_LIST
        .iter()
        .enumerate()
        .filter(|(_, a)| a.far.sent())
        .map(|(k, a)| {
            records::global(
                GLOBALS + k as u32,
                &format!("VrmAutoTest16{}Go", a.label),
                0.0,
            )
        })
        .collect();
    let spell = records::spell(
        SPELL,
        "VrmAutoTest16Fortify",
        records::spell::SPELL,
        records::spell::FIRE_AND_FORGET,
        records::spell::SELF,
        0,
        None,
        &[Effect::new(effect, 50.0, EFFECT_SECONDS)],
    );

    // Each actor's packages, first to last: the far one (once its global
    // is set) then staying home; sitting in the chair; sandboxing (the
    // vanilla package).
    let mut packages = Vec::new();
    let mut lists = Vec::new();
    for (k, a) in ACTOR_LIST.iter().enumerate() {
        let far_id = PACKAGES + 2 * k as u32;
        let home_id = far_id + 1;
        let home_package = |packages: &mut Vec<RecordBuilder>| -> Result<()> {
            packages.push(package(
                lo,
                home_id,
                &format!("VrmAutoTest16{}Home", a.label),
                own(HOMES + k as u32),
                ALWAYS,
                None,
                WALK,
            )?);
            Ok(())
        };
        match a.far {
            Far::Stay => {
                home_package(&mut packages)?;
                lists.push(vec![own(home_id)]);
            }
            Far::Chair => {
                let at = pldt(0, own(CHAIR), 0);
                packages.push(records::copy(
                    lo,
                    "MS13CamillaWorkTrader",
                    far_id,
                    "VrmAutoTest16SitterSit",
                    None,
                    |tag, d| match tag {
                        b"PLDT" => *d = at.clone(),
                        b"PSDT" => *d = ALWAYS.to_vec(),
                        b"PKDT" if d.len() >= 4 => {
                            // Not offering services.
                            let flags = u32::from_le_bytes(d[0..4].try_into().unwrap()) & !0x1;
                            d[0..4].copy_from_slice(&flags.to_le_bytes());
                        }
                        _ => {}
                    },
                )?);
                lists.push(vec![own(far_id)]);
            }
            Far::Sandbox => lists.push(vec![sandbox]),
            _ => {
                packages.push(package(
                    lo,
                    far_id,
                    &format!("VrmAutoTest16{}Far", a.label),
                    own(FARS + k as u32),
                    ALWAYS,
                    Some(own(GLOBALS + k as u32)),
                    WALK,
                )?);
                home_package(&mut packages)?;
                lists.push(vec![own(far_id), own(home_id)]);
            }
        }
    }

    let actor_script = AttachedScript {
        name: ACTOR_SCRIPT.into(),
        properties: Vec::new(),
    };
    let npcs: Vec<RecordBuilder> = ACTOR_LIST
        .iter()
        .enumerate()
        .map(|(k, a)| {
            Npc {
                object_id: NPCS + k as u32,
                edid: format!("VrmAutoTest16{}Npc", a.label),
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
            .record_with_packages(race, class, &lists[k])
        })
        .collect();

    let quests = vec![
        Quest {
            object_id: QUEST,
            edid: "VrmAutoTest16Quest",
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

    // Every reference persistent, Tamriel's in its persistent cell.
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
    by_place[Place::Inn.index()].push(
        RecordBuilder::new(b"REFR", own(CHAIR))
            .flags(persistent)
            .form(b"NAME", v.id("CommonChair01")?)
            .field(b"DATA", &placement(at.chair, 0.0)),
    );
    for (k, a) in ACTOR_LIST.iter().enumerate() {
        let home = a.home.index();
        by_place[home].push(marker(own(HOMES + k as u32), at.homes[k]));
        if let Some(far) = at.fars[k] {
            // Interior far markers stay in their cell; Tamriel's all go in
            // its persistent cell whichever place they're near.
            by_place[home].push(marker(own(FARS + k as u32), far));
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
        match p.interior() {
            Some((_, name)) => interiors.push(Interior {
                cell: at.cells[p.index()],
                name,
                persistent: refs,
                temporary: Vec::new(),
            }),
            None => tamriel_persistent.extend(refs),
        }
    }
    let worlds = vec![World {
        world: tamriel,
        name: "Skyrim",
        grid: Place::Hub.grid().unwrap(),
        persistent: tamriel_persistent,
        temporary: Vec::new(),
    }];

    // Top groups in the Creation Kit's order.
    let plugin = write_plugin(
        0,
        &["Skyrim.esm"],
        "vibe_rm",
        "What unloaded actors do: when and whether they are put at their targets, walking on \
         as their cell unloads, the cells kept loaded after the player leaves, furniture and \
         sandboxing through a wait and on return, waiting and sleeping, and a timed effect as \
         its actor unloads. Read by script, with the user waiting and sleeping when asked; \
         reports saved.",
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
            ("Spell".to_owned(), form(own(SPELL).0, "Spell")),
            ("Effect".to_owned(), form(0x3EAF3, "MagicEffect")),
            ("Bedroll".to_owned(), form(0x36ED3, "Form")),
        ];
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
            properties.push((
                format!("Far{k}"),
                form(own(FARS + k as u32).0, "ObjectReference"),
            ));
            if a.far.sent() {
                properties.push((
                    format!("Go{k}"),
                    form(own(GLOBALS + k as u32).0, "GlobalVariable"),
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
        assert_eq!(shown.boxes[1], SLEEP_PROMPT);
        assert_eq!(shown.saved_at_box, Some(2), "saves before the reports");
        let all = shown.boxes.join("\n");
        for expected in [
            "Dead killed unloaded: dead False\nat Timed's cell; 3D after -1 s",
            "  effect gone ",
            "from the hub, before:\n  PassA: far 1 (z 0), home 1, ",
            "  PassB: 0 moves, -1 to -1 s, largest 0\n",
            "sent home (globals 0, no EvaluatePackage):\nat 30",
            "  LeavingFar: 0 moves, -1 to -1 s, largest 0; 3D False changed at -1\n",
            "at BufExt's cell; 3D after -1 s",
            "  BufIn2: 0 moves, -1 to -1 s, largest 0; 3D False changed at -1\n",
            "Sitter seated after -1 s (-1: not in 60 s):\n  Sitter: sit state 1, chair 1, moved ",
            "time passed seen after -1 s (-1: not in 240 s)",
            "at Timed's cell: Sitter's 3D gone after ",
            "time passed seen after -1 s (-1: not in 300 s)",
            "5 s after EvaluatePackage again, from afar:\n",
            "Dead dead False\n",
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
        const { assert!(CHAIR < GLOBALS) };
        assert!(GLOBALS + n <= NPCS);
        assert!(NPCS + n <= PACKAGES);
        assert!(PACKAGES + 2 * n <= HOMES);
        assert!(HOMES + n <= FARS);
        assert!(FARS + n <= ENTRIES);
        assert!(ENTRIES + PLACES.len() as u32 <= ACTORS);
        assert!(ACTORS + n <= NEXT_OBJECT_ID);
    }

    #[test]
    fn the_lists_name_actors() {
        for label in JUMPERS
            .iter()
            .chain(&STEEP)
            .chain(&GOING_HOME)
            .chain(&LEAVERS)
            .chain(&RESTING)
            .chain(&VISITED)
        {
            k_of(label);
        }
        for (label, _) in STAGGER {
            assert!(JUMPERS.contains(&label));
        }
    }
}
