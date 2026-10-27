//! `test-mod autotest19`: the questions blocking the unblocked animation
//! work (`plans/animation-unblocked.md`, step 0): which graph events reach
//! scripts, whether an unalerted NPC draws on Papyrus `DrawWeapon`, when an
//! actor swims, and when NPCs sprint and circle in combat.
//!
//! One quest, starting as the save loads; each page runs in turn (about 3
//! minutes in all), then the reports are saved and shown:
//!
//! 1. **Events**: the player (an iron sword and shield, sheathed first) and
//!    a test NPC walking to the player from 800 units east (a copy of a
//!    vanilla Travel package, near the player) are registered for
//!    `weaponDraw` (heard in autotest2), `InitiateWinBegin` (the 1HM block
//!    hit clip's, unheard in autotest17), `blockHitStop` (ends the block
//!    hit), `HitFrame` and `SoundPlay` (the swing clips'), `FootLeft` /
//!    `FootRight` (the walk clips') and `attackStart` (sent in by the
//!    script itself). The player then draws (0 s), and is sent `blockStart`
//!    (2.5), `blockHitStart` (3.1), `blockStop` (4.6) and `attackStart`
//!    (5.2); counting stops at 10. Each event gets a line: what
//!    `RegisterForAnimationEvent` returned for each, and how often each
//!    sent it, with the first time.
//! 2. **Draw**: four NPCs given an iron sword (no `SetDontMove`): a
//!    commoner, unalerted; one `SetAlert(true)`; a vanilla guard
//!    (`GuardWhiterunImperialBarracks`, placed as it is); and a brave one
//!    in combat with a commoner held (`SetRestrained`) 500 units off.
//!    Each is told `DrawWeapon` at once; `IsWeaponDrawn` is read 1, 3 and 10 s later.
//! 3. **Sprint**: a brave NPC with a sword starts combat with a commoner
//!    held 1500 units off. Its distance, then `S` sprinting, `R` running
//!    (`IsSprinting`, `IsRunning`) or `W` neither, sampled with 0.25 s
//!    waits between until it is within 200 or 10 s pass; four samples a
//!    line, the time first. Each read takes about 0.12 s itself
//!    (autotest17 / 18), so samples come about 0.6 s apart.
//! 4. **Flee**: a cowardly NPC and a brave one held 400 units off start
//!    combat with each other; the coward sampled so for 8 s.
//! 5. **Duel**, 6. **Flank**: two NPCs of a combat style 300 units apart
//!    fight each other for 16 s (1000 health each): `csHumanMeleeLvl6`
//!    (dueling; `CSCR` circle 0.73, fallback 0.41), then
//!    `csMercerFreyMelee` (flanking; circle 1, fallback 1, flank distance
//!    0.45, stalk time 0.29). A line per four samples (0.25 s waits plus
//!    the reads', about 2 s a line), its time first: the first's
//!    `Direction` at each, its mean `Speed`, then the distance between
//!    them and both headings (`GetAngleZ`).
//! 7. **SwimPlayer**, 8. **SwimNpc**, 9. **SwimBreathing**: markers on a
//!    lake's bed (the one nearest Riverwood with a straight slope from the
//!    shore, [`places::shore`]: Tamriel cell (-3, -15), `POIPineForest25`,
//!    west of Riverwood, surface 600, a line heading north from the south
//!    shore, the markers 64 units apart and 168 to the last; build output
//!    prints it) at depths 40, 60,
//!    80, 100, 120, 140 and 180 under the surface. The player is moved to
//!    each; then a test NPC is, the player waiting on the shallowest; then
//!    the player again with `AbWaterbreathing`. At each, 1 and 3 s after
//!    the move: whether a test ability's effect, conditioned on
//!    `IsSwimming == 1`, is active (`HasMagicEffect`; ability conditions
//!    are rechecked about once a second, hence two reads), the feet's
//!    height against the surface (`GetPositionZ`), and at 3 s the graph's
//!    `SwimDirection`. The bed's depth under each marker is printed as a
//!    check. The player is moved back where they started at the end.
//!
//! | page | reading | means for the plan |
//! | ---- | ------- | ------------------ |
//! | Events | registration False for some events | scripts can't hear those at all: 8a filters by name, not kind |
//! | Events | all True; clip annotations (`HitFrame`, `FootLeft`...) counted, `blockHitStop` not | only annotations reach scripts: 8a filters the rest |
//! | Events | all True, `attackStart` (sent in) counted | events sent into the graph reach scripts too |
//! | Events | the walker's `FootLeft` / `FootRight` counted, the player's draw not | registrations on the player differ: recheck autotest2 |
//! | Events | everything counted | autotest17's silence was its setup; 8a has nothing to do |
//! | Draw | unalerted False throughout, the others True | 8b: gate `DrawWeapon` on alert, guard or combat |
//! | Draw | the alerted one False too | alert alone doesn't let it draw: gate on combat (and guards) |
//! | Draw | all True | autotest17's NPC was held by `SetDontMove`, or something else: 8b gates nothing |
//! | Sprint | `S` while far, `R` as it closes | 2c: sprint past a distance, the switch distance read off |
//! | Sprint | no `S` | NPCs don't sprint to charge: 2c only for fleeing, if Flee shows it |
//! | Flee | `S` while fleeing | 2c: fleeing actors sprint |
//! | Duel / Flank | `Direction` near 0.25 / 0.75 for stretches at fighting distance, headings turning | circling, its share of the time read off for 2b |
//! | Duel / Flank | `Direction` near 0.5 with the distance growing | backing off (fallback) |
//! | Duel / Flank | `Direction` 0 throughout | no circling: 2b's placeholder goes |
//! | Swim | the effect active from depth *d* (both reads) | 3a's swim depth, against the placeholder 100 |
//! | Swim | feet settle *h* under the surface when swimming | 3b's floating height |
//! | Swim | a depth swimming at 1 s but not 3 s, or the reverse | settling, or hysteresis: look at the heights |
//! | SwimNpc differs from SwimPlayer | NPCs' depth is their own |
//! | SwimBreathing differs from SwimPlayer | Waterbreathing changes the state (3e) |
//!
//! Load a save outdoors on open ground, out of combat, and leave the game
//! alone for about 3 minutes: the player draws, blocks and swings by
//! script, NPCs fight near them (not with them), and the player is moved
//! to a lake and back. The plugin name doesn't matter.
//!
//! Run in Skyrim SE once (2026-10-27, outdoors; Survival Mode's plugin
//! loaded). Slaughterfish attacked the player during the swimming pages.
//!
//! - **Events**: every registration returned True. Heard from the player:
//!   `weaponDraw` (0.73 s), `InitiateWinBegin` (4.04, 0.61 s after
//!   `blockHitStart`), `blockHitStop` (4.33) and `HitFrame` (6.29); from
//!   the walker, `FootLeft` 9 times and `FootRight` 8 (it walked 798 >
//!   127 units). **Never heard**: `SoundPlay` from either, although the
//!   swing played (its `HitFrame` came) and the walker stepped, and
//!   `attackStart`, which the script sent in itself. So annotations without
//!   a payload and graph-raised events reach scripts. `SoundPlay` (always
//!   carrying a payload) and events sent into the graph don't.
//!   autotest17's silence came from its setup, not from SE.
//! - **Draw**: `SetAlert(true)` drew the alerted NPC's weapon before
//!   `DrawWeapon` was called. The one in combat had drawn too (combat state
//!   2). After `DrawWeapon`, all four were drawn at 1 s. At 3 and 10 s the
//!   unalerted commoner and the guard (unalerted, not in combat) had
//!   sheathed again, while the alerted and fighting ones stayed drawn. So
//!   Papyrus draws for anyone, and an NPC neither alerted nor in combat
//!   sheathes within 1-3 s. autotest17 read its NPC 3 s after drawing.
//! - **Sprint**: inconclusive. The fighter walked (`W` throughout) from
//!   1447 to about 1000 units and stopped there in combat state 2
//!   (searching): it never found its target at that range, so it never
//!   charged.
//! - **Flee**: the coward walked for about 2 s, then **ran** (`R`, never
//!   `S`) from 385 to 1519 units in about 4 s (about 290 units/s), then
//!   walked at about 1630.
//! - **Duel** (`csHumanMeleeLvl6`): the first two lines matched Flank's
//!   to within 0.001 (the same approach). `Direction` was in the strafe
//!   bands (0.15-0.35 right, 0.65-0.85 left) in 17 of 40 samples, near 0.5
//!   (backing off) in 3, and near 0 / 1 (forward) or stopped in the rest. It
//!   circled left (about 0.75) from about 5 to 11 s at 74-180 units, mean
//!   `Speed` 15-61, while both headings turned about 40 degrees the same
//!   way. Then it strafed right (about 0.25) at 13.8 s. Health lost: 27 and
//!   20.
//! - **Flank** (`csMercerFreyMelee`): strafing in 9 of 40 samples, all
//!   right (about 0.25) in the last 4 s. One of those lines had mean
//!   `Speed` 350 (a run), with the headings swinging 150-250 degrees. From
//!   5 to 9 s it stood (`Direction` 0, `Speed` 0-31) at 85-162 units.
//! - **Swimming**: the detector read True at every depth on all three
//!   pages, at 40 too, where the player's feet were on the bed
//!   (standing). That is most likely `HasMagicEffect` counting the
//!   ability's effect while its condition fails, so **`IsSwimming` stays
//!   unanswered**: the probe had no dry-land control. The heights (player
//!   page) do settle the floating height. Feet were on the bed at 40, 60
//!   and 80 (-42, -62, -82). At 100 and 120 they held at -91 / -92, though
//!   the bed was deeper. At 140 they read -139 twice, and at 180 rose from
//!   -148 to -132 (still rising at 3 s). The Waterbreathing page held at
//!   -92 from 100 to 140; its 180 (-209, below the bed) and the NPC page
//!   (feet above the surface at 60 and 100 by 3 s) are noisy. That is
//!   likely the slaughterfish, and both are left unread. A swimmer's feet
//!   float about 92 under the surface, so an actor stands where the water
//!   is shallower than that. `SwimDirection` was 0 throughout (no one moved
//!   by choice).

use anyhow::Result;
use esp::writer::{AttachedScript, Node, PropertyValue, RecordBuilder, write_plugin};
use esp::{FormId, LoadOrder, record_flags};
use papyrus::build::{Arg, FunctionBuilder, ScriptBuilder, auto_var, id, string};
use papyrus::pex::op;

use super::TestMod;
use super::autotest2::{Npc, Vanilla, line, place_fn, put, remove, sheathe};
use super::autotest4::{fresh, put_num, report, restrain, set_av};
use super::autotest6::{ALWAYS, pldt};
use super::own;
use super::places::{self, World};
use super::records::{self, Effect, Quest, ctda, op as cmp, run_on, spell};
use super::script::{Code, num, on_update, real_time};
use super::venv::placement;

const QUEST: u32 = 0x800;
const WALKER: u32 = 0x801;
const PACKAGE: u32 = 0x802;
const COMMONER: u32 = 0x803;
const BRAVE: u32 = 0x804;
const COWARD: u32 = 0x805;
const DUELER: u32 = 0x806;
const FLANKER: u32 = 0x807;
const SWIM_EFFECT: u32 = 0x808;
const SWIM_ABILITY: u32 = 0x809;
const MARKERS: u32 = 0x80A;
const NEXT_OBJECT_ID: u32 = MARKERS + DEPTHS.len() as u32;
const SCRIPT: &str = "VrmAutoTest19Script";
const PLAYER_REF: FormId = FormId(0x14);
const TAMRIEL: FormId = FormId(0x3C);
/// Where the lake is looked for from.
const RIVERWOOD: [f32; 2] = [18700.0, -46930.0];

const TESTS: &[(&str, &str)] = &[
    ("Events", "Which graph events reach OnAnimationEvent"),
    (
        "Draw",
        "DrawWeapon on NPCs: unalerted, alerted, guard, in combat",
    ),
    (
        "Sprint",
        "A fighter closing 1500 units: distance, Sprint/Run/Walk",
    ),
    ("Flee", "A coward fleeing: distance, Sprint/Run/Walk"),
    (
        "Duel",
        "Dueling style: Direction x4, Speed, distance, headings",
    ),
    (
        "Flank",
        "Flanking style: Direction x4, Speed, distance, headings",
    ),
    (
        "SwimPlayer",
        "The player by depth: swimming, feet - surface",
    ),
    ("SwimNpc", "An NPC by depth: swimming, feet - surface"),
    ("SwimBreathing", "The player with Waterbreathing, by depth"),
];

/// The events registered for, and counted.
const EVENTS: [&str; 8] = [
    "weaponDraw",
    "InitiateWinBegin",
    "blockHitStop",
    "HitFrame",
    "SoundPlay",
    "FootLeft",
    "FootRight",
    "attackStart",
];

/// The sources counted: label, the script variable holding it.
const SOURCES: [(&str, &str); 2] = [("player", "PlayerRef"), ("walker", "WalkerRef")];

/// What the events page sends the player: (seconds after it draws, event).
const SENT: [(f32, &str); 4] = [
    (2.5, "blockStart"),
    (3.1, "blockHitStart"),
    (4.6, "blockStop"),
    (5.2, "attackStart"),
];
/// When the events page stops counting.
const EVENTS_END: f32 = 10.0;

/// The bed depths of the swimming markers.
const DEPTHS: [f32; 7] = [40.0, 60.0, 80.0, 100.0, 120.0, 140.0, 180.0];

fn count_var(event: usize, source: usize) -> String {
    format!("Count{event}{}", SOURCES[source].0)
}

fn first_var(event: usize, source: usize) -> String {
    format!("First{event}{}", SOURCES[source].0)
}

fn registered_var(event: usize, source: usize) -> String {
    format!("Reg{event}{}", SOURCES[source].0)
}

/// `OnAnimationEvent`: counted per source and event, with the first time
/// since `T0`.
fn on_animation_event() -> FunctionBuilder {
    let mut c = Code::new("None");
    let source = c.param("akSource", "ObjectReference");
    let event = c.param("asEventName", "String");
    let end = c.label();
    for (s, (_, var)) in SOURCES.iter().enumerate() {
        let is = c.calc(op::CMP_EQ, "Bool", source.clone(), id(var));
        let next_source = c.label();
        c.jmpf(is, next_source);
        for (e, name) in EVENTS.iter().enumerate() {
            let same = c.calc(op::CMP_EQ, "Bool", event.clone(), string(name));
            let next = c.label();
            c.jmpf(same, next);
            let count = id(&count_var(e, s));
            let n = c.calc(op::IADD, "Int", count.clone(), 1.into());
            c.assign(count, n);
            let first = id(&first_var(e, s));
            let seen = c.calc(op::CMP_GE, "Bool", first.clone(), 0.0f32.into());
            c.jmpt(seen, end);
            let t = real_time(&mut c);
            let dt = c.calc(op::FSUB, "Float", t, id("T0"));
            c.assign(first, dt);
            c.jmp(end);
            c.bind(next);
        }
        c.jmp(end);
        c.bind(next_source);
    }
    c.bind(end);
    c.finish(Arg::None)
}

fn remove_all(c: &mut Code, actors: &[&Arg]) {
    for a in actors {
        remove(c, a);
    }
}

fn arm(c: &mut Code, a: &Arg, items: &[&str]) {
    for item in items {
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
}

/// `Sent(String what)`: a line with the seconds since `T0`.
fn sent() -> FunctionBuilder {
    let mut c = Code::new("None");
    let what = c.param("what", "String");
    let t = real_time(&mut c);
    let dt = c.calc(op::FSUB, "Float", t, id("T0"));
    let n = c.own("Num", "String", [dt]);
    c.append(id("Log"), [n, string(" "), what, string("\n")]);
    c.finish(Arg::None)
}

fn test_events() -> FunctionBuilder {
    let (mut c, r) = report();
    let player = auto_var("Player");
    arm(&mut c, &player, &["Sword", "Shield"]);
    c.wait(1.0);
    sheathe(&mut c);
    let walker = fresh(&mut c, "walker", "WalkerBase", 800.0, 0.0);
    c.cast(id("PlayerRef"), player.clone());
    c.cast(id("WalkerRef"), walker.clone());
    for (s, (_, var)) in SOURCES.iter().enumerate() {
        for (e, name) in EVENTS.iter().enumerate() {
            let ok = c.method(
                "Form",
                id("self"),
                "RegisterForAnimationEvent",
                [id(var), string(name)],
            );
            c.assign(id(&registered_var(e, s)), ok);
        }
    }
    let d = c.method(
        "ObjectReference",
        walker.clone(),
        "GetDistance",
        [player.clone()],
    );
    put_num(&mut c, &r, "walker from the player: ", d);
    let t = real_time(&mut c);
    c.assign(id("T0"), t);
    c.assign(id("Log"), string(""));
    c.own("Sent", "None", [string("DrawWeapon")]);
    c.method("Actor", player.clone(), "DrawWeapon", []);
    let mut at = 0.0;
    for (when, event) in SENT {
        c.wait(when - at);
        at = when;
        c.own("Sent", "None", [string(&format!("sent {event}"))]);
        c.global(
            "Debug",
            "SendAnimationEvent",
            [player.clone(), string(event)],
        );
    }
    c.wait(EVENTS_END - at);
    let d = c.method("ObjectReference", walker.clone(), "GetDistance", [player]);
    put_num(&mut c, &r, ", at 10 s ", d);
    line(&mut c, &r, "\n");
    // Counting stops here.
    c.assign(id("T0"), (-1.0e6f32).into());
    for (_, var) in SOURCES {
        for name in EVENTS {
            c.method(
                "Form",
                id("self"),
                "UnregisterForAnimationEvent",
                [id(var), string(name)],
            );
        }
    }
    c.append(r.clone(), [id("Log")]);
    line(
        &mut c,
        &r,
        "event: registered player/walker; heard (count, first s)\n",
    );
    for (e, name) in EVENTS.iter().enumerate() {
        put(&mut c, &r, &format!("{name}: "), id(&registered_var(e, 0)));
        put(&mut c, &r, "/", id(&registered_var(e, 1)));
        for (s, (label, _)) in SOURCES.iter().enumerate() {
            put(&mut c, &r, &format!("; {label} "), id(&count_var(e, s)));
            let first = id(&first_var(e, s));
            let seen = c.calc(op::CMP_GE, "Bool", first.clone(), 0.0f32.into());
            let skip = c.label();
            c.jmpf(seen, skip);
            let n = c.own("Num", "String", [first]);
            c.append(r.clone(), [string(" at "), n]);
            c.bind(skip);
        }
        line(&mut c, &r, "\n");
    }
    remove_all(&mut c, &[&walker]);
    c.finish(r)
}

fn test_draw() -> FunctionBuilder {
    let (mut c, r) = report();
    let unalerted = fresh(&mut c, "unalerted", "CommonerBase", 250.0, -300.0);
    let alerted = fresh(&mut c, "alerted", "CommonerBase", 250.0, -100.0);
    let guard = fresh(&mut c, "guard", "GuardBase", 250.0, 100.0);
    let fighter = fresh(&mut c, "fighter", "BraveBase", 250.0, 300.0);
    let target = fresh(&mut c, "target", "CommonerBase", 750.0, 300.0);
    let four = [
        ("unalerted", &unalerted),
        ("alerted", &alerted),
        ("guard", &guard),
        ("in combat", &fighter),
    ];
    for (_, a) in four {
        arm(&mut c, a, &["Sword"]);
    }
    for a in [&fighter, &target] {
        set_av(&mut c, a, "Health", 1000.0);
    }
    restrain(&mut c, &target);
    c.wait(1.5);
    c.method("Actor", alerted.clone(), "SetAlert", [true.into()]);
    c.method("Actor", fighter.clone(), "StartCombat", [target.clone()]);
    c.wait(1.0);
    for (label, a) in four {
        let alert = c.method("Actor", a.clone(), "IsAlerted", []);
        put(&mut c, &r, &format!("{label}: alerted "), alert);
        let is_guard = c.method("Actor", a.clone(), "IsGuard", []);
        put(&mut c, &r, ", guard ", is_guard);
        let state = c.method("Actor", a.clone(), "GetCombatState", []);
        put(&mut c, &r, ", combat state ", state);
        let drawn = c.method("Actor", a.clone(), "IsWeaponDrawn", []);
        put(&mut c, &r, ", drawn ", drawn);
        line(&mut c, &r, "\n");
    }
    line(&mut c, &r, "DrawWeapon on all four, IsWeaponDrawn:\n");
    for (_, a) in four {
        c.method("Actor", a.clone(), "DrawWeapon", []);
    }
    let mut at = 0.0;
    for when in [1.0, 3.0, 10.0] {
        c.wait(when - at);
        at = when;
        line(&mut c, &r, &format!("{when} s:"));
        for (label, a) in four {
            let drawn = c.method("Actor", a.clone(), "IsWeaponDrawn", []);
            put(&mut c, &r, &format!(" {label} "), drawn);
        }
        line(&mut c, &r, "\n");
    }
    remove_all(&mut c, &[&unalerted, &alerted, &guard, &fighter, &target]);
    c.finish(r)
}

/// `Chase(Actor a, Actor b, Float seconds, Float near)`: `a`'s distance
/// to `b` and its gait every 0.25 s, four samples a line, until it is
/// within `near` or `seconds` pass.
fn chase() -> FunctionBuilder {
    let mut c = Code::new("String");
    let a = c.param("a", "Actor");
    let b = c.param("b", "Actor");
    let seconds = c.param("seconds", "Float");
    let near = c.param("near", "Float");
    let r = c.local("r", "String");
    c.assign(r.clone(), string(""));
    let t0 = c.local("t0", "Float");
    let t = real_time(&mut c);
    c.assign(t0.clone(), t);
    let i = c.local("i", "Int");
    c.assign(i.clone(), 0.into());
    let (top, done) = (c.label(), c.label());
    c.bind(top);
    let now = real_time(&mut c);
    let dt = c.calc(op::FSUB, "Float", now, t0.clone());
    let over = c.calc(op::CMP_GE, "Bool", dt.clone(), seconds);
    c.jmpt(over, done);
    let column = c.calc(op::IMOD, "Int", i.clone(), 4.into());
    let first = c.calc(op::CMP_EQ, "Bool", column, 0.into());
    let not_first = c.label();
    c.jmpf(first, not_first);
    let n = c.own("Num", "String", [dt]);
    c.append(r.clone(), [n, string(":")]);
    c.bind(not_first);
    let d = c.method("ObjectReference", a.clone(), "GetDistance", [b]);
    let whole = c.global("Math", "Floor", [d.clone()]);
    c.append(r.clone(), [string(" "), whole]);
    let sprinting = c.method("Actor", a.clone(), "IsSprinting", []);
    let running = c.method("Actor", a, "IsRunning", []);
    let (not_sprinting, not_running, gait_done) = (c.label(), c.label(), c.label());
    c.jmpf(sprinting, not_sprinting);
    c.append(r.clone(), [string(" S")]);
    c.jmp(gait_done);
    c.bind(not_sprinting);
    c.jmpf(running, not_running);
    c.append(r.clone(), [string(" R")]);
    c.jmp(gait_done);
    c.bind(not_running);
    c.append(r.clone(), [string(" W")]);
    c.bind(gait_done);
    c.op(op::IADD, [i.clone(), i.clone(), 1.into()]);
    let column = c.calc(op::IMOD, "Int", i.clone(), 4.into());
    let end_of_line = c.calc(op::CMP_EQ, "Bool", column, 0.into());
    let mid_line = c.label();
    c.jmpf(end_of_line, mid_line);
    c.append(r.clone(), [string("\n")]);
    c.bind(mid_line);
    let close = c.calc(op::CMP_LT, "Bool", d, near);
    c.jmpt(close, done);
    c.wait(0.25);
    c.jmp(top);
    c.bind(done);
    let column = c.calc(op::IMOD, "Int", i, 4.into());
    let ragged = c.calc(op::CMP_EQ, "Bool", column, 0.into());
    let end = c.label();
    c.jmpt(ragged, end);
    c.append(r.clone(), [string("\n")]);
    c.bind(end);
    c.finish(r)
}

fn test_sprint() -> FunctionBuilder {
    let (mut c, r) = report();
    let fighter = fresh(&mut c, "fighter", "BraveBase", 250.0, 0.0);
    let target = fresh(&mut c, "target", "CommonerBase", 1750.0, 0.0);
    arm(&mut c, &fighter, &["Sword"]);
    for a in [&fighter, &target] {
        set_av(&mut c, a, "Health", 1000.0);
    }
    restrain(&mut c, &target);
    c.wait(1.5);
    c.method("Actor", fighter.clone(), "StartCombat", [target.clone()]);
    let log = c.own(
        "Chase",
        "String",
        [
            fighter.clone(),
            target.clone(),
            10.0f32.into(),
            200.0f32.into(),
        ],
    );
    c.append(r.clone(), [log]);
    let state = c.method("Actor", fighter.clone(), "GetCombatState", []);
    put(&mut c, &r, "combat state ", state);
    line(&mut c, &r, "\n");
    remove_all(&mut c, &[&fighter, &target]);
    c.finish(r)
}

fn test_flee() -> FunctionBuilder {
    let (mut c, r) = report();
    let coward = fresh(&mut c, "coward", "CowardBase", 250.0, 0.0);
    let brave = fresh(&mut c, "brave", "BraveBase", 650.0, 0.0);
    arm(&mut c, &brave, &["Sword"]);
    for a in [&coward, &brave] {
        set_av(&mut c, a, "Health", 1000.0);
    }
    restrain(&mut c, &brave);
    c.wait(1.5);
    c.method("Actor", coward.clone(), "StartCombat", [brave.clone()]);
    c.method("Actor", brave.clone(), "StartCombat", [coward.clone()]);
    let log = c.own(
        "Chase",
        "String",
        [coward.clone(), brave.clone(), 8.0f32.into(), 0.0f32.into()],
    );
    c.append(r.clone(), [log]);
    let state = c.method("Actor", coward.clone(), "GetCombatState", []);
    put(&mut c, &r, "combat state ", state);
    line(&mut c, &r, "\n");
    remove_all(&mut c, &[&coward, &brave]);
    c.finish(r)
}

fn anim_float(c: &mut Code, a: &Arg, var: &str) -> Arg {
    c.method(
        "ObjectReference",
        a.clone(),
        "GetAnimationVariableFloat",
        [string(var)],
    )
}

/// `Circle(Actor a, Actor b, Float seconds)`: lines of `a`'s `Direction`
/// at four samples 0.25 s apart, its mean `Speed`, the distance and both
/// headings.
fn circle() -> FunctionBuilder {
    let mut c = Code::new("String");
    let a = c.param("a", "Actor");
    let b = c.param("b", "Actor");
    let seconds = c.param("seconds", "Float");
    let r = c.local("r", "String");
    c.assign(r.clone(), string(""));
    let t0 = c.local("t0", "Float");
    let t = real_time(&mut c);
    c.assign(t0.clone(), t);
    let speed = c.local("speed", "Float");
    let (top, done) = (c.label(), c.label());
    c.bind(top);
    let now = real_time(&mut c);
    let dt = c.calc(op::FSUB, "Float", now, t0.clone());
    let over = c.calc(op::CMP_GE, "Bool", dt.clone(), seconds);
    c.jmpt(over, done);
    let n = c.own("Num", "String", [dt]);
    c.append(r.clone(), [n, string(":")]);
    c.assign(speed.clone(), 0.0f32.into());
    for k in 0..4 {
        if k > 0 {
            c.wait(0.25);
        }
        let dir = anim_float(&mut c, &a, "Direction");
        c.numbers(&r, [(" ", dir)]);
        let s = anim_float(&mut c, &a, "Speed");
        c.op(op::FADD, [speed.clone(), speed.clone(), s]);
    }
    let mean = c.calc(op::FDIV, "Float", speed.clone(), 4.0f32.into());
    let mean = c.global("Math", "Floor", [mean]);
    c.append(r.clone(), [string(", speed "), mean]);
    let d = c.method("ObjectReference", a.clone(), "GetDistance", [b.clone()]);
    let d = c.global("Math", "Floor", [d]);
    c.append(r.clone(), [string(", d "), d]);
    for (label, x) in [(", headings ", &a), (" ", &b)] {
        let angle = c.method("ObjectReference", x.clone(), "GetAngleZ", []);
        let angle = c.global("Math", "Floor", [angle]);
        c.append(r.clone(), [string(label), angle]);
    }
    c.append(r.clone(), [string("\n")]);
    c.wait(0.25);
    c.jmp(top);
    c.bind(done);
    c.finish(r)
}

fn test_fight(base: &str) -> FunctionBuilder {
    let (mut c, r) = report();
    let a = fresh(&mut c, "a", base, 250.0, 0.0);
    let b = fresh(&mut c, "b", base, 550.0, 0.0);
    for x in [&a, &b] {
        arm(&mut c, x, &["Sword"]);
        set_av(&mut c, x, "Health", 1000.0);
    }
    c.wait(1.5);
    c.method("Actor", a.clone(), "StartCombat", [b.clone()]);
    c.method("Actor", b.clone(), "StartCombat", [a.clone()]);
    c.wait(1.0);
    let log = c.own("Circle", "String", [a.clone(), b.clone(), 16.0f32.into()]);
    c.append(r.clone(), [log]);
    for (label, x) in [("health: a ", &a), (", b ", &b)] {
        let h = c.method("Actor", x.clone(), "GetActorValue", [string("Health")]);
        let h = c.global("Math", "Floor", [h]);
        put(&mut c, &r, label, h);
    }
    line(&mut c, &r, "\n");
    remove_all(&mut c, &[&a, &b]);
    c.finish(r)
}

fn marker(k: usize) -> Arg {
    auto_var(&format!("Marker{k}"))
}

/// `SwimAt(Actor a, ObjectReference m, String label)`: `a` moved to `m`,
/// then its swimming and height against the surface 1 and 3 s later.
fn swim_at() -> FunctionBuilder {
    let mut c = Code::new("String");
    let a = c.param("a", "Actor");
    let m = c.param("m", "ObjectReference");
    let label = c.param("label", "String");
    let r = c.local("r", "String");
    c.assign(r.clone(), label);
    let bed = c.method("ObjectReference", m.clone(), "GetPositionZ", []);
    let bed = c.calc(op::FSUB, "Float", auto_var("Surface"), bed);
    let bed = c.global("Math", "Floor", [bed]);
    c.append(r.clone(), [string(" (bed "), bed, string("):")]);
    c.method(
        "ObjectReference",
        a.clone(),
        "MoveTo",
        [m, 0.0f32.into(), 0.0f32.into(), 0.0f32.into(), true.into()],
    );
    let mut at = 0.0;
    for when in [1.0, 3.0] {
        c.wait(when - at);
        at = when;
        let swimming = c.method(
            "Actor",
            a.clone(),
            "HasMagicEffect",
            [auto_var("SwimEffect")],
        );
        let z = c.method("ObjectReference", a.clone(), "GetPositionZ", []);
        let z = c.calc(op::FSUB, "Float", z, auto_var("Surface"));
        let z = c.global("Math", "Floor", [z]);
        c.append(
            r.clone(),
            [string(&format!(" {when} s ")), swimming, string(" "), z],
        );
    }
    let dir = anim_float(&mut c, &a, "SwimDirection");
    c.numbers(&r, [(", SwimDirection ", dir)]);
    c.append(r.clone(), [string("\n")]);
    c.finish(r)
}

/// A swimming page: `a` (the player unless `npc`) to each marker, the
/// player back home at the end of the last.
fn test_swim(npc: bool, breathing: bool, last: bool) -> FunctionBuilder {
    let (mut c, r) = report();
    let player = auto_var("Player");
    let first = !npc && !breathing;
    if first {
        // Where the player goes back to.
        let home = c.method(
            "ObjectReference",
            player.clone(),
            "PlaceAtMe",
            [auto_var("XMarker"), 1.into(), false.into(), false.into()],
        );
        c.assign(id("Home"), home);
        sheathe(&mut c);
    }
    c.method(
        "ObjectReference",
        player.clone(),
        "MoveTo",
        [
            marker(0),
            0.0f32.into(),
            0.0f32.into(),
            0.0f32.into(),
            true.into(),
        ],
    );
    c.wait(5.0);
    let subject = if npc {
        let a = c.method(
            "ObjectReference",
            marker(0),
            "PlaceActorAtMe",
            [auto_var("CommonerBase"), 4.into(), Arg::None],
        );
        let v = c.local("swimmer", "Actor");
        c.assign(v.clone(), a);
        c.wait(2.0);
        v
    } else {
        player.clone()
    };
    c.method(
        "Actor",
        subject.clone(),
        "AddSpell",
        [auto_var("SwimAbility"), false.into()],
    );
    if breathing {
        c.method(
            "Actor",
            player.clone(),
            "AddSpell",
            [auto_var("Waterbreathing"), false.into()],
        );
    }
    c.wait(1.0);
    put_num(&mut c, &r, "surface ", auto_var("Surface"));
    let pos = c.method("ObjectReference", player.clone(), "GetPositionZ", []);
    let pos = c.calc(op::FSUB, "Float", pos, auto_var("Surface"));
    put_num(&mut c, &r, ", the player's feet ", pos);
    line(
        &mut c,
        &r,
        "\ndepth (bed): 1 s swimming, feet - surface; 3 s the same\n",
    );
    for (k, depth) in DEPTHS.iter().enumerate() {
        let l = c.own(
            "SwimAt",
            "String",
            [subject.clone(), marker(k), string(&format!("{depth}"))],
        );
        c.append(r.clone(), [l]);
    }
    c.method(
        "Actor",
        subject.clone(),
        "RemoveSpell",
        [auto_var("SwimAbility")],
    );
    if breathing {
        c.method(
            "Actor",
            player.clone(),
            "RemoveSpell",
            [auto_var("Waterbreathing")],
        );
    }
    if npc {
        remove_all(&mut c, &[&subject]);
    }
    if last {
        c.method(
            "ObjectReference",
            player,
            "MoveTo",
            [
                id("Home"),
                0.0f32.into(),
                0.0f32.into(),
                0.0f32.into(),
                true.into(),
            ],
        );
        c.method("ObjectReference", id("Home"), "Delete", []);
    }
    c.finish(r)
}

fn scripts() -> Result<Vec<(String, Vec<u8>)>> {
    let mut s = ScriptBuilder::new(SCRIPT, "Quest");
    for (name, class) in [
        ("Player", "Actor"),
        ("Sword", "Weapon"),
        ("Shield", "Armor"),
        ("XMarker", "Form"),
        ("WalkerBase", "ActorBase"),
        ("CommonerBase", "ActorBase"),
        ("BraveBase", "ActorBase"),
        ("CowardBase", "ActorBase"),
        ("DuelerBase", "ActorBase"),
        ("FlankerBase", "ActorBase"),
        ("GuardBase", "ActorBase"),
        ("SwimAbility", "Spell"),
        ("SwimEffect", "MagicEffect"),
        ("Waterbreathing", "Spell"),
        ("Surface", "Float"),
    ] {
        s.auto_property(name, class);
    }
    for k in 0..DEPTHS.len() {
        s.auto_property(&format!("Marker{k}"), "ObjectReference");
    }
    s.variable("Ran", "Bool", false.into());
    s.variable("T0", "Float", (-1.0e6f32).into());
    s.variable("Log", "String", string(""));
    s.variable("Home", "ObjectReference", Arg::None);
    for (_, var) in SOURCES {
        s.variable(var, "ObjectReference", Arg::None);
    }
    for e in 0..EVENTS.len() {
        for src in 0..SOURCES.len() {
            s.variable(&count_var(e, src), "Int", 0.into());
            s.variable(&first_var(e, src), "Float", (-1.0f32).into());
            s.variable(&registered_var(e, src), "Bool", false.into());
        }
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
        .function("OnUpdate", on_update("autotest19", "3 minutes", TESTS))
        .function("OnAnimationEvent", on_animation_event())
        .function("Num", num())
        .function("Place", place_fn())
        .function("Sent", sent())
        .function("Chase", chase())
        .function("Circle", circle())
        .function("SwimAt", swim_at())
        .function("TestEvents", test_events())
        .function("TestDraw", test_draw())
        .function("TestSprint", test_sprint())
        .function("TestFlee", test_flee())
        .function("TestDuel", test_fight("DuelerBase"))
        .function("TestFlank", test_fight("FlankerBase"))
        .function("TestSwimPlayer", test_swim(false, false, false))
        .function("TestSwimNpc", test_swim(true, false, false))
        .function("TestSwimBreathing", test_swim(false, true, true));
    Ok(vec![(SCRIPT.to_owned(), papyrus::pex::write(&s.build()?)?)])
}

pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    let v = Vanilla { lo };
    let shore = places::shore(lo, TAMRIEL, RIVERWOOD, &DEPTHS)?;
    eprintln!(
        "autotest19: the lake at {:?} ({}), surface {}, markers {:?}",
        shore.grid, shore.cell, shore.surface, shore.points
    );
    let object = PropertyValue::Object;
    let mut properties = vec![
        ("Player".to_owned(), object(PLAYER_REF)),
        ("Sword".to_owned(), object(v.id("IronSword")?)),
        ("Shield".to_owned(), object(v.id("ArmorIronShield")?)),
        ("XMarker".to_owned(), object(v.id("XMarker")?)),
        ("WalkerBase".to_owned(), object(own(WALKER))),
        ("CommonerBase".to_owned(), object(own(COMMONER))),
        ("BraveBase".to_owned(), object(own(BRAVE))),
        ("CowardBase".to_owned(), object(own(COWARD))),
        ("DuelerBase".to_owned(), object(own(DUELER))),
        ("FlankerBase".to_owned(), object(own(FLANKER))),
        (
            "GuardBase".to_owned(),
            object(v.id("GuardWhiterunImperialBarracks")?),
        ),
        ("SwimAbility".to_owned(), object(own(SWIM_ABILITY))),
        ("SwimEffect".to_owned(), object(own(SWIM_EFFECT))),
        (
            "Waterbreathing".to_owned(),
            object(v.id("AbWaterbreathing")?),
        ),
        ("Surface".to_owned(), PropertyValue::Float(shore.surface)),
    ];
    for k in 0..DEPTHS.len() {
        properties.push((format!("Marker{k}"), object(own(MARKERS + k as u32))));
    }

    let template = v.id("WEMerchantMiscMaleCommoner")?;
    let (race, class) = (v.id("ImperialRace")?, v.id("Citizen")?);
    // (object id, editor id, aggression and confidence, combat style,
    // packages)
    let npcs: Vec<RecordBuilder> = [
        (WALKER, "Walker", (0, 2), None, vec![own(PACKAGE)]),
        (COMMONER, "Commoner", (0, 2), None, Vec::new()),
        (BRAVE, "Brave", (1, 3), None, Vec::new()),
        (COWARD, "Coward", (0, 0), None, Vec::new()),
        (
            DUELER,
            "Dueler",
            (1, 3),
            Some(v.id("csHumanMeleeLvl6")?),
            Vec::new(),
        ),
        (
            FLANKER,
            "Flanker",
            (1, 3),
            Some(v.id("csMercerFreyMelee")?),
            Vec::new(),
        ),
    ]
    .into_iter()
    .map(|(object_id, edid, temper, combat_style, packages)| {
        Npc {
            object_id,
            edid: format!("VrmAutoTest19{edid}"),
            name: Some("vibe_rm Test NPC"),
            level: (10, false, 1, 0),
            template: (template, 0x1),
            temper,
            factions: Vec::new(),
            script: None,
            assistance: 0,
            combat_style,
            unique: false,
        }
        .record_with_packages(race, class, &packages)
    })
    .collect();

    // The walker's: a vanilla Travel package, to near the player, always.
    let package = records::copy(
        lo,
        "MS13LucanHoldPositionPackage",
        PACKAGE,
        "VrmAutoTest19Walk",
        None,
        |tag, d| match tag {
            b"PLDT" => *d = pldt(0, PLAYER_REF, 128),
            b"PSDT" => *d = ALWAYS.to_vec(),
            // Flag 0x1: offers services.
            b"PKDT" if d.len() >= 4 => d[0] &= !0x1,
            _ => {}
        },
    )?;

    // The swimming detector: a harmless constant effect on self, active
    // only while swimming.
    let effect = records::copy(
        lo,
        "AlchFortifyOneHanded",
        SWIM_EFFECT,
        "VrmAutoTest19Swimming",
        None,
        |tag, d| {
            if tag == b"DATA" && d.len() >= 0x58 {
                d[0x50..0x54].copy_from_slice(&spell::CONSTANT.to_le_bytes());
                d[0x54..0x58].copy_from_slice(&spell::SELF.to_le_bytes());
            }
        },
    )?;
    const IS_SWIMMING: u16 = 185;
    let ability = records::spell(
        SWIM_ABILITY,
        "VrmAutoTest19SwimmingAbility",
        spell::ABILITY,
        spell::CONSTANT,
        spell::SELF,
        0,
        None,
        &[Effect::new(own(SWIM_EFFECT), 1.0, 0).when(ctda(
            IS_SWIMMING,
            0,
            0,
            cmp::EQ,
            1.0,
            run_on::SUBJECT,
        ))],
    );

    let quest = Quest {
        object_id: QUEST,
        edid: "VrmAutoTest19Quest",
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

    let heading_marker = v.id("XMarkerHeading")?;
    let markers = shore
        .points
        .iter()
        .enumerate()
        .map(|(k, at)| {
            RecordBuilder::new(b"REFR", own(MARKERS + k as u32))
                .flags(record_flags::PERSISTENT)
                .form(b"NAME", heading_marker)
                .field(b"DATA", &placement(*at, shore.heading))
        })
        .collect();
    let worlds = vec![World {
        world: TAMRIEL,
        name: "Skyrim",
        grid: shore.grid,
        persistent: markers,
        temporary: Vec::new(),
    }];

    // Top groups in the Creation Kit's order.
    let plugin = write_plugin(
        0,
        &["Skyrim.esm"],
        "vibe_rm",
        "Animation questions: graph events heard by scripts, DrawWeapon on NPCs, NPCs \
         sprinting and circling in combat, and swimming by depth. Run by script; reports saved.",
        NEXT_OBJECT_ID,
        &[
            Node::top(b"MGEF", vec![effect]),
            Node::top(b"SPEL", vec![ability]),
            Node::top(b"NPC_", npcs),
            places::worlds(lo, worlds)?,
            Node::top(b"QUST", vec![quest]),
            Node::top(b"PACK", vec![package]),
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
        let mut properties = vec![
            ("Player".to_owned(), form(0x14, "Actor")),
            ("Sword".to_owned(), form(0x12EB7, "Weapon")),
            ("Shield".to_owned(), form(0x12EB6, "Armor")),
            ("XMarker".to_owned(), form(0x3B, "Form")),
            ("SwimAbility".to_owned(), form(own(SWIM_ABILITY).0, "Spell")),
            (
                "SwimEffect".to_owned(),
                form(own(SWIM_EFFECT).0, "MagicEffect"),
            ),
            ("Waterbreathing".to_owned(), form(0xAA01C, "Spell")),
            ("Surface".to_owned(), papyrus::Value::Float(-200.0)),
            ("GuardBase".to_owned(), form(0x10E2BF, "ActorBase")),
        ];
        for (name, id) in [
            ("WalkerBase", WALKER),
            ("CommonerBase", COMMONER),
            ("BraveBase", BRAVE),
            ("CowardBase", COWARD),
            ("DuelerBase", DUELER),
            ("FlankerBase", FLANKER),
        ] {
            properties.push((name.to_owned(), form(own(id).0, "ActorBase")));
        }
        for k in 0..DEPTHS.len() {
            properties.push((
                format!("Marker{k}"),
                form(own(MARKERS + k as u32).0, "ObjectReference"),
            ));
        }
        let shown = run(
            scripts().unwrap(),
            &[(own(QUEST).0, SCRIPT, properties)],
            TESTS.len(),
        );
        assert_eq!(shown.boxes.len(), TESTS.len(), "every test reports");
        assert_eq!(shown.saved_at_box, Some(0), "saves before the boxes");
        let all = shown.boxes.join("\n");
        for expected in [
            "walker from the player: 1, at 10 s 1\n",
            "0 DrawWeapon\n",
            " sent blockHitStart\n",
            "weaponDraw: False/False; player 0; walker 0\n",
            "in combat: alerted False, guard False, combat state 1, drawn False\n",
            "10 s: unalerted False",
            "surface -200, the player's feet 201\n",
            "180 (bed -201): 1 s False 201 3 s False 201, SwimDirection 1\n",
        ] {
            assert!(all.contains(expected), "{expected:?} not in the reports");
        }
        for b in &shown.boxes {
            assert!(b.lines().count() <= 20, "{} lines:\n{b}", b.lines().count());
        }
    }
}
