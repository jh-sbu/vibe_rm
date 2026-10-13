//! `test-mod autotest`: several ambiguities tested by one script, with no
//! one playing. Loading a save with the plugin for the first time starts
//! `VrmAutoTestQuest`; a few seconds later its script runs each test in
//! turn, then shows each one's report in a message box (and traces it to
//! the Papyrus log). The reports are also kept in the script's variables
//! (`ResultLevels`, `ResultActorValues`, `ResultLocks`, `ResultControls`,
//! `ResultWeather`), and once they're done the script asks the game to
//! save (`Game.RequestSave`, a new numbered save) before showing the boxes:
//! `vrm-tool test-results` reads the reports from that save.
//!
//! Load a save made outdoors, without the plugin, and leave the game
//! alone (no menus: they stop `Utility.Wait`) for about two minutes until
//! the boxes come up. The tests change the save (a door's lock, the
//! weather, spawned actors deleted again), so don't play on from the save
//! they make: it's for reading the reports from.
//!
//! What each test does and the readings it tells apart:
//!
//! - **Levels** (`known_ambiguities/skills.md`, NPC levels): spawns NPCs of
//!   our own with the PC level mult flag, multipliers that leave fractions
//!   (a tie at .5 for any player level not a multiple of 8), and calc min /
//!   max set, and reports `GetLevel` beside the player's level times the
//!   multiplier: whether the game rounds, floors or rounds up, whether calc
//!   max 0 means no maximum, and that min and max clamp.
//! - **Actor values** (`known_ambiguities/actor-values.md`): base, current
//!   and percentage of a spawned NPC's OneHanded and Health after each of
//!   `SetActorValue`, `ModActorValue`, `ForceActorValue`,
//!   `DamageActorValue`, `RestoreActorValue`: which part each changes,
//!   whether restoring stops at the undamaged value, and whether
//!   `GetActorValuePercentage` is 0..1.
//! - **Locks** (`known_ambiguities/locks.md`, gap 3): Sven and Hilde's
//!   house's door pair, the lock authored on the inside only. Both doors'
//!   `IsLocked` / `GetLockLevel` as each side is locked and unlocked: shared,
//!   the other side follows; separate, it doesn't. Restored after.
//! - **Controls** (`known_ambiguities/player-controls.md`): whether a false
//!   argument to `DisablePlayerControls` / `EnablePlayerControls` leaves
//!   that control alone or sets it; whether one script's enabling undoes
//!   another's disabling (`VrmAutoTestHelperQuest`'s script is the other
//!   caller); whether disabling fighting sheathes a drawn weapon (drawing
//!   it first with `DrawWeapon`).
//! - **Weather** (`known_ambiguities/weather.md`, transition speed): sets a
//!   weather active (override), normally, accelerated, and one with
//!   another trans delta, timing `GetCurrentWeatherTransition` to half and
//!   full in real seconds and game minutes. Before: trans delta 125 was 8
//!   game minutes (24 s at timescale 20), lower slower, accelerated 16
//!   times faster. Indoors it reports only the current weather and sky
//!   mode.
//!
//! Results in Skyrim SE (2026-10-12, player level 1, outdoors):
//!
//! - Levels: rounded down (x1.5 and x1.667 gave 1); calc min clamps (100).
//!   Ties and calc max can't show at level 1.
//! - Actor values: as `src/actor_values.rs` had it, except a percentage
//!   whose base plus modifier is 0 reads 1, not 0.
//! - Locks: shared; the outside door reads locked from the start and
//!   follows either side's `Lock`. `GetLockLevel` 0 both sides.
//! - Controls: false arguments leave a control alone; one shared state for
//!   all scripts; disabling fighting sheathes the weapon (`DrawWeapon`
//!   works on the player).
//! - The message boxes didn't come up in the order called (Locks came
//!   last), so a batch's reports are best read from the save.
//!
//! Rerun 2026-10-13, reports read from the save it made (`test-results`):
//! the same for the four above, and
//!
//! - Weather (timescale 20, all overrides): 125 half at 11.4 s, full at
//!   22.8 s / 7.7 game minutes; accelerated the same (22.9 s); 82 (the
//!   other weather here) full at 15.7 s / 5.3 game minutes. Longer with a
//!   higher delta, about delta / 16 game minutes; acceleration does
//!   nothing to an override.

use anyhow::{Context, Result, ensure};
use esp::writer::{AttachedScript, Node, PropertyValue, RecordBuilder, vmad, write_plugin};
use esp::{FormId, LoadOrder};
use papyrus::build::{Arg, FunctionBuilder, ScriptBuilder, auto_var, discard, id, string};
use papyrus::pex::op;

use super::script::{Code, bytes, measure, num, on_update, settle, spawn_level};
use super::{TestMod, own};

const QUEST: u32 = 0x800;
const HELPER_QUEST: u32 = 0x801;
const DUMMY: u32 = 0x802;
/// The levels test's NPCs, one per [`LEVELS`] entry.
const LEVEL_NPCS: u32 = 0x803;
const NEXT_OBJECT_ID: u32 = LEVEL_NPCS + LEVELS.len() as u32;
const SCRIPT: &str = "VrmAutoTestScript";
const HELPER: &str = "VrmAutoTestHelperScript";
/// The player's reference: a fixed form every game has.
const PLAYER_REF: FormId = FormId(0x14);
/// The door pair's inside cell; the lock is on its door.
const DOOR_CELL: &str = "RiverwoodSvensHouse";
/// The weathers set by the transition test, both trans delta 125.
const WEATHER_A: &str = "SkyrimClear";
const WEATHER_B: &str = "SkyrimCloudy";

/// The levels test's NPCs: label, `ACBS` level (the multiplier x 1000 with
/// the PC level mult flag), the flag, calc min and calc max.
const LEVELS: &[(&str, u16, bool, u16, u16)] = &[
    ("level 7, no mult", 7, false, 1, 0),
    ("x1", 1000, true, 1, 0),
    ("x1.125", 1125, true, 1, 0),
    ("x1.25", 1250, true, 1, 0),
    ("x1.333", 1333, true, 1, 0),
    ("x1.5", 1500, true, 1, 0),
    ("x1.667", 1667, true, 1, 0),
    ("x1, max 2", 1000, true, 1, 2),
    ("x1, min 100", 1000, true, 100, 0),
];

fn test_levels() -> FunctionBuilder {
    let mut c = Code::new("String");
    let r = c.local("r", "String");
    let player_level = c.method("Actor", auto_var("Player"), "GetLevel", []);
    let level_f = c.temp("Float");
    c.assign(r.clone(), string(""))
        .append(r.clone(), [string("player level "), player_level.clone()])
        .append(r.clone(), [string("\n")])
        .cast(level_f.clone(), player_level);
    for (i, &(label, level, mult, _, _)) in LEVELS.iter().enumerate() {
        let got = c.own("SpawnLevel", "Int", [auto_var(&format!("LevelNpc{i}"))]);
        c.append(r.clone(), [string(label), string(": "), got]);
        if mult {
            let exact = c.calc(
                op::FMUL,
                "Float",
                level_f.clone(),
                (f32::from(level) / 1000.0).into(),
            );
            c.numbers(&r, [(" for ", exact)]);
        }
        c.append(r.clone(), [string("\n")]);
    }
    c.finish(r)
}

/// `Snap(Actor a, String av, String label)`: a line of `av`'s base, current
/// value and percentage.
fn snap() -> FunctionBuilder {
    let mut c = Code::new("String");
    let a = c.param("a", "Actor");
    let av = c.param("av", "String");
    let label = c.param("label", "String");
    let r = c.local("r", "String");
    let base = c.method("Actor", a.clone(), "GetBaseActorValue", [av.clone()]);
    let current = c.method("Actor", a.clone(), "GetActorValue", [av.clone()]);
    let pct = c.method("Actor", a, "GetActorValuePercentage", [av]);
    c.assign(r.clone(), label);
    c.numbers(
        &r,
        [
            (": base ", base),
            (", current ", current),
            (", percent ", pct),
        ],
    );
    c.append(r.clone(), [string("\n")]);
    c.finish(r)
}

fn test_actor_values() -> FunctionBuilder {
    let mut c = Code::new("String");
    let r = c.local("r", "String");
    let a = c.local("a", "Actor");
    c.assign(r.clone(), string("OneHanded\n"));
    let placed = c.method(
        "Actor",
        auto_var("Player"),
        "PlaceActorAtMe",
        [auto_var("Dummy"), 4.into(), Arg::None],
    );
    c.assign(a.clone(), placed);
    let step = |c: &mut Code, av: &str, call: Option<(&str, f32)>, label: &str| {
        if let Some((name, value)) = call {
            c.method("Actor", a.clone(), name, [string(av), value.into()]);
        }
        let line = c.own("Snap", "String", [a.clone(), string(av), string(label)]);
        c.append(r.clone(), [line]);
    };
    let one_handed = [
        (None, "start"),
        (Some(("SetActorValue", 40.0)), "SetAV 40"),
        (Some(("ModActorValue", 5.0)), "ModAV +5"),
        (Some(("ForceActorValue", 70.0)), "ForceAV 70"),
        (Some(("DamageActorValue", 10.0)), "DamageAV 10"),
        (Some(("RestoreActorValue", 100.0)), "RestoreAV 100"),
        (Some(("SetActorValue", 20.0)), "SetAV 20"),
        (Some(("ModActorValue", -50.0)), "ModAV -50"),
    ];
    for (call, label) in one_handed {
        step(&mut c, "OneHanded", call, label);
    }
    c.append(r.clone(), [string("Health\n")]);
    step(&mut c, "Health", None, "start");
    let base = c.method("Actor", a.clone(), "GetBaseActorValue", [string("Health")]);
    let half = c.calc(op::FMUL, "Float", base, 0.5f32.into());
    c.method(
        "Actor",
        a.clone(),
        "DamageActorValue",
        [string("Health"), half],
    );
    step(&mut c, "Health", None, "DamageAV half the base");
    let health = [
        (Some(("ModActorValue", 100.0)), "ModAV +100"),
        (Some(("ForceActorValue", 50.0)), "ForceAV 50"),
        (Some(("RestoreActorValue", 10000.0)), "RestoreAV 10000"),
    ];
    for (call, label) in health {
        step(&mut c, "Health", call, label);
    }
    c.method("Actor", a.clone(), "Disable", [false.into()]);
    c.method("Actor", a, "Delete", []);
    c.finish(r)
}

/// `DoorState(String label)`: both doors' lock state.
fn door_state() -> FunctionBuilder {
    let mut c = Code::new("String");
    let label = c.param("label", "String");
    let r = c.local("r", "String");
    c.assign(r.clone(), label);
    for (name, door) in [(": inside ", "DoorInside"), (", outside ", "DoorOutside")] {
        let locked = c.method("ObjectReference", auto_var(door), "IsLocked", []);
        let level = c.method("ObjectReference", auto_var(door), "GetLockLevel", []);
        c.append(
            r.clone(),
            [
                string(name),
                string("locked "),
                locked,
                string(" level "),
                level,
            ],
        );
    }
    c.append(r.clone(), [string("\n")]);
    c.finish(r)
}

fn test_locks() -> FunctionBuilder {
    let mut c = Code::new("String");
    let r = c.local("r", "String");
    c.assign(r.clone(), string(""));
    let inside = auto_var("DoorInside");
    let outside = auto_var("DoorOutside");
    let inside_was = c.method("ObjectReference", inside.clone(), "IsLocked", []);
    let outside_was = c.method("ObjectReference", outside.clone(), "IsLocked", []);
    let steps: [(Option<(&Arg, bool)>, &str); 6] = [
        (None, "start"),
        (Some((&inside, true)), "lock inside"),
        (Some((&inside, false)), "unlock inside"),
        (Some((&outside, true)), "lock outside"),
        (Some((&outside, false)), "unlock outside"),
        (None, "restored"),
    ];
    for (call, label) in steps {
        if let Some((door, lock)) = call {
            c.method(
                "ObjectReference",
                door.clone(),
                "Lock",
                [lock.into(), false.into()],
            );
        }
        if label == "restored" {
            c.method(
                "ObjectReference",
                inside.clone(),
                "Lock",
                [inside_was.clone(), false.into()],
            );
            c.method(
                "ObjectReference",
                outside.clone(),
                "Lock",
                [outside_was.clone(), false.into()],
            );
        }
        c.wait(0.1);
        let line = c.own("DoorState", "String", [string(label)]);
        c.append(r.clone(), [line]);
    }
    c.finish(r)
}

/// `ControlState(String label)`: whether movement and fighting are enabled.
fn control_state() -> FunctionBuilder {
    let mut c = Code::new("String");
    let label = c.param("label", "String");
    let r = c.local("r", "String");
    let movement = c.global("Game", "IsMovementControlsEnabled", []);
    let fighting = c.global("Game", "IsFightingControlsEnabled", []);
    c.assign(r.clone(), label).append(
        r.clone(),
        [
            string(": movement "),
            movement,
            string(", fighting "),
            fighting,
            string("\n"),
        ],
    );
    c.finish(r)
}

fn test_controls() -> FunctionBuilder {
    let mut c = Code::new("String");
    let r = c.local("r", "String");
    c.assign(r.clone(), string(""));
    let state = |c: &mut Code, label: &str| {
        c.wait(0.2);
        let line = c.own("ControlState", "String", [string(label)]);
        c.append(r.clone(), [line]);
    };
    let helper = |c: &mut Code, name: &str| {
        c.call_method(auto_var("Helper"), name, discard(), []);
    };
    state(&mut c, "at start");
    c.enable_all();
    // False arguments: left alone, or set?
    c.controls(false, true, false);
    state(&mut c, "Disable(movement)");
    c.controls(false, false, true);
    state(&mut c, "then Disable(false, fighting)");
    c.enable_all();
    c.controls(false, true, true);
    c.controls(true, false, true);
    state(&mut c, "Disable(both), then Enable(false, fighting)");
    c.enable_all();
    // Two callers: this script and the helper quest's.
    c.controls(false, true, false);
    helper(&mut c, "DisableMovement");
    helper(&mut c, "EnableMovement");
    state(&mut c, "both disable movement, the helper enables it");
    c.enable_all();
    state(&mut c, "then this script enables all");
    c.controls(false, true, false);
    helper(&mut c, "EnableMovement");
    state(
        &mut c,
        "this script disables movement, the helper enables it",
    );
    c.enable_all();

    // Does disabling fighting sheathe a drawn weapon?
    let player = auto_var("Player");
    let drawn_at_start = c.method("Actor", player.clone(), "IsWeaponDrawn", []);
    let drawn = c.label();
    c.jmpt(drawn_at_start.clone(), drawn);
    c.method("Actor", player.clone(), "DrawWeapon", []);
    c.wait(3.0);
    c.bind(drawn);
    let drawn_before = c.method("Actor", player.clone(), "IsWeaponDrawn", []);
    c.append(
        r.clone(),
        [
            string("weapon drawn at start "),
            drawn_at_start,
            string(", before disabling fighting "),
            drawn_before.clone(),
        ],
    );
    let (skip, end) = (c.label(), c.label());
    c.jmpf(drawn_before, skip);
    c.controls(false, false, true);
    c.wait(3.0);
    let drawn_after = c.method("Actor", player, "IsWeaponDrawn", []);
    c.append(r.clone(), [string(", after "), drawn_after, string("\n")]);
    c.enable_all();
    c.jmp(end);
    c.bind(skip);
    c.append(r.clone(), [string(" (couldn't draw it: skipped)\n")]);
    c.bind(end);
    c.finish(r)
}

fn test_weather(slow_label: &str) -> FunctionBuilder {
    let mut c = Code::new("String");
    let r = c.local("r", "String");
    let first = c.local("first", "Weather");
    let second = c.local("second", "Weather");
    let inside = c.method("Actor", auto_var("Player"), "IsInInterior", []);
    let current = c.global("Weather", "GetCurrentWeather", []);
    let sky = c.global("Weather", "GetSkyMode", []);
    let time_scale = c.method("GlobalVariable", auto_var("TimeScale"), "GetValue", []);
    c.assign(r.clone(), string("")).append(
        r.clone(),
        [
            string("indoors "),
            inside.clone(),
            string(", sky mode "),
            sky,
            string(", weather "),
            current,
        ],
    );
    c.numbers(&r, [(", timescale ", time_scale)]);
    c.append(r.clone(), [string("\n")]);
    let (outdoors, end) = (c.label(), c.label());
    c.jmpf(inside, outdoors);
    c.append(r.clone(), [string("transitions skipped: run outdoors\n")]);
    c.jmp(end);
    c.bind(outdoors);

    let settle = |c: &mut Code| {
        let line = c.own("Settle", "String", []);
        c.append(r.clone(), [line]);
    };
    settle(&mut c);
    // Each measurement changes the weather: whichever of A and B isn't on.
    let (a, b) = (auto_var("WeatherA"), auto_var("WeatherB"));
    let current = c.global("Weather", "GetCurrentWeather", []);
    let is_a = c.calc(op::CMP_EQ, "Bool", current, a.clone());
    let keep = c.label();
    c.assign(first.clone(), a.clone())
        .assign(second.clone(), b.clone())
        .jmpf(is_a, keep)
        .assign(first.clone(), b)
        .assign(second.clone(), a)
        .bind(keep);
    let measure = |c: &mut Code, w: Arg, accelerate: bool, label: &str| {
        let line = c.own(
            "Measure",
            "String",
            [w, true.into(), accelerate.into(), string(label)],
        );
        c.append(r.clone(), [line]);
    };
    measure(&mut c, first, false, "delta 125");
    settle(&mut c);
    measure(&mut c, second, true, "delta 125, accelerated");
    let slow = auto_var("WeatherSlow");
    let none = c.temp("Weather");
    c.cast(none.clone(), Arg::None);
    let missing = c.calc(op::CMP_EQ, "Bool", slow.clone(), none);
    let released = c.label();
    c.jmpt(missing, released);
    settle(&mut c);
    measure(&mut c, slow, false, slow_label);
    c.bind(released);
    c.global("Weather", "ReleaseOverride", []);
    c.bind(end);
    c.finish(r)
}

/// The tests in the order run: name (the report's variable is
/// `Result<name>`), function, title.
const TESTS: &[(&str, &str)] = &[
    ("Levels", "NPC levels (PC level mult)"),
    ("ActorValues", "Actor value calls"),
    ("Locks", "Load door pair locks"),
    ("Controls", "Player controls"),
    ("Weather", "Weather transitions"),
];

/// The scripts: the tests' and the helper's. `slow_label` names the weather
/// with another trans delta.
fn scripts(slow_label: &str) -> Result<Vec<(String, Vec<u8>)>> {
    let mut s = ScriptBuilder::new(SCRIPT, "Quest");
    for (name, ty) in PROPERTIES {
        s.auto_property(name, ty);
    }
    for i in 0..LEVELS.len() {
        s.auto_property(&format!("LevelNpc{i}"), "ActorBase");
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
        .function("OnUpdate", on_update("autotest", "two minutes", TESTS))
        .function("Num", num())
        .function("SpawnLevel", spawn_level())
        .function("Snap", snap())
        .function("DoorState", door_state())
        .function("ControlState", control_state())
        .function("Settle", settle())
        .function("Measure", measure())
        .function("TestLevels", test_levels())
        .function("TestActorValues", test_actor_values())
        .function("TestLocks", test_locks())
        .function("TestControls", test_controls())
        .function("TestWeather", test_weather(slow_label));

    let mut h = ScriptBuilder::new(HELPER, "Quest");
    for (name, enable) in [("DisableMovement", false), ("EnableMovement", true)] {
        let mut c = Code::new("None");
        c.controls(enable, true, false);
        h.function(name, c.finish(Arg::None));
    }
    Ok(vec![
        (SCRIPT.into(), papyrus::pex::write(&s.build()?)?),
        (HELPER.into(), papyrus::pex::write(&h.build()?)?),
    ])
}

/// The test script's properties other than the levels test's NPCs.
const PROPERTIES: &[(&str, &str)] = &[
    ("Player", "Actor"),
    ("Dummy", "ActorBase"),
    ("DoorInside", "ObjectReference"),
    ("DoorOutside", "ObjectReference"),
    ("WeatherA", "Weather"),
    ("WeatherB", "Weather"),
    ("WeatherSlow", "Weather"),
    ("TimeScale", "GlobalVariable"),
    ("Helper", HELPER),
];

/// The vanilla forms used (all from Skyrim.esm).
struct Vanilla {
    template: FormId,
    race: FormId,
    class: FormId,
    weather_a: FormId,
    weather_b: FormId,
    /// A weather of another trans delta, and that delta.
    weather_slow: Option<(FormId, u8)>,
    time_scale: FormId,
    door_inside: FormId,
    door_outside: FormId,
}

impl Vanilla {
    fn resolve(lo: &LoadOrder) -> Result<Self> {
        let id = |edid: &str| -> Result<FormId> {
            let id = lo
                .find_editor_id(edid)
                .with_context(|| format!("{edid} not found"))?;
            ensure!(id.0 >> 24 == 0, "{edid} ({id}) isn't from Skyrim.esm");
            Ok(id)
        };
        // The door pair: the inside cell's load door, and where it leads.
        let fid = |rec: &esp::LoadedRecord<'_>, tag: &[u8; 4]| {
            rec.get(tag)
                .filter(|d| d.len() >= 4)
                .map(|d| rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap()))))
        };
        let cell = id(DOOR_CELL)?;
        let idx = lo.cell(cell).context("door cell not indexed")?;
        let (door_inside, door_outside) = idx
            .persistent
            .iter()
            .find_map(|&r| {
                let rec = lo.get(r)?;
                rec.get(b"XLOC")?;
                Some((r, fid(&rec, b"XTEL")?))
            })
            .with_context(|| format!("no locked persistent load door in {DOOR_CELL}"))?;
        let outside = lo.get(door_outside).context("far door missing")?;
        ensure!(
            outside.get(b"XLOC").is_none(),
            "{door_outside}: the outside door has a lock of its own"
        );
        // The first of the game's weathers with another trans delta (DATA
        // byte 3), not 0 (at once).
        let weather_slow = lo.ids_of_type(b"WTHR").iter().find_map(|&w| {
            let rec = lo.get(w)?;
            let delta = *rec.get(b"DATA")?.get(3)?;
            let edid = rec.editor_id()?;
            (w.0 >> 24 == 0 && edid.starts_with("Skyrim") && delta != 125 && delta != 0)
                .then_some((w, delta))
        });
        Ok(Vanilla {
            // A world encounter merchant: its FaceGen ships with the game,
            // and a "Use Traits" template takes the face from it.
            template: id("WEMerchantMiscMaleCommoner")?,
            race: id("ImperialRace")?,
            class: id("Citizen")?,
            weather_a: id(WEATHER_A)?,
            weather_b: id(WEATHER_B)?,
            weather_slow,
            time_scale: id("TimeScale")?,
            door_inside,
            door_outside,
        })
    }
}

/// An unaggressive NPC of our own: Traits from the template, our stats.
fn npc(
    object_id: u32,
    edid: &str,
    level: u16,
    pc_mult: bool,
    min: u16,
    max: u16,
    v: &Vanilla,
) -> RecordBuilder {
    // ACBS: flags (PC level mult 0x80), magicka and stamina offsets, level
    // (or mult x 1000), calc min / max, speed 100, disposition 35, template
    // flags (Use Traits), health offset, bleedout override.
    let flags: u32 = if pc_mult { 0x80 } else { 0 };
    let acbs: [u8; 24] = bytes(&[
        &flags.to_le_bytes(),
        &0i16.to_le_bytes(),
        &0i16.to_le_bytes(),
        &level.to_le_bytes(),
        &min.to_le_bytes(),
        &max.to_le_bytes(),
        &100u16.to_le_bytes(),
        &35i16.to_le_bytes(),
        &0x0001u16.to_le_bytes(),
        &0i16.to_le_bytes(),
        &0u16.to_le_bytes(),
    ]);
    // AIDT: unaggressive, cowardly, energy 50, no crime, neutral, helps
    // nobody.
    let aidt = [
        0u8, 0, 50, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    // DNAM: 18 skills at 5 and no offsets, health 30, magicka 25, stamina 70,
    // far away model distance 0, geared up weapons 0.
    let dnam: [u8; 52] = bytes(&[
        &[5; 18],
        &[0; 18],
        &30u16.to_le_bytes(),
        &25u16.to_le_bytes(),
        &70u16.to_le_bytes(),
        &[0; 10],
    ]);
    let bounds: Vec<u8> = [-41i16, -25, 0, 41, 72, 86]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    RecordBuilder::new(b"NPC_", own(object_id))
        .zstring(b"EDID", edid)
        .field(b"OBND", &bounds)
        .field(b"ACBS", &acbs)
        .form(b"TPLT", v.template)
        .form(b"RNAM", v.race)
        .field(b"AIDT", &aidt)
        .form(b"CNAM", v.class)
        .zstring(b"FULL", "vibe_rm Test NPC")
        .field(b"DATA", &[])
        .field(b"DNAM", &dnam)
        .field(b"NAM5", &[0xFF, 0])
        .f32(b"NAM6", 1.0)
        .f32(b"NAM7", 50.0)
        .u32(b"NAM8", 1) // sound level: normal
}

/// A start game enabled quest carrying `script`.
fn quest(object_id: u32, edid: &str, name: &str, script: AttachedScript) -> RecordBuilder {
    // DNAM: flags (start game enabled and starts enabled), priority, form
    // version, unknown, type (none).
    let mut dnam = 0x0011u16.to_le_bytes().to_vec();
    dnam.extend_from_slice(&[0, 0]);
    dnam.extend_from_slice(&0u32.to_le_bytes());
    dnam.extend_from_slice(&0u32.to_le_bytes());
    RecordBuilder::new(b"QUST", own(object_id))
        .zstring(b"EDID", edid)
        .field(b"VMAD", &vmad(&[script]))
        .zstring(b"FULL", name)
        .field(b"DNAM", &dnam)
        .field(b"NEXT", &[])
        .u32(b"ANAM", 0) // next alias id
}

fn slow_label(v: &Vanilla) -> String {
    match v.weather_slow {
        Some((_, delta)) => format!("delta {delta}"),
        None => "(no other delta)".into(),
    }
}

pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    let v = Vanilla::resolve(lo)?;
    let object = |id: FormId| PropertyValue::Object(id);
    let mut properties = vec![
        ("Player".to_owned(), object(PLAYER_REF)),
        ("Dummy".into(), object(own(DUMMY))),
        ("DoorInside".into(), object(v.door_inside)),
        ("DoorOutside".into(), object(v.door_outside)),
        ("WeatherA".into(), object(v.weather_a)),
        ("WeatherB".into(), object(v.weather_b)),
        (
            "WeatherSlow".into(),
            object(v.weather_slow.map_or(FormId(0), |(w, _)| w)),
        ),
        ("TimeScale".into(), object(v.time_scale)),
        ("Helper".into(), object(own(HELPER_QUEST))),
    ];
    for i in 0..LEVELS.len() {
        properties.push((format!("LevelNpc{i}"), object(own(LEVEL_NPCS + i as u32))));
    }
    let quests = vec![
        quest(
            QUEST,
            "VrmAutoTestQuest",
            "vibe_rm Autotest",
            AttachedScript {
                name: SCRIPT.into(),
                properties,
            },
        ),
        quest(
            HELPER_QUEST,
            "VrmAutoTestHelperQuest",
            "vibe_rm Autotest Helper",
            AttachedScript {
                name: HELPER.into(),
                properties: Vec::new(),
            },
        ),
    ];
    let mut npcs = vec![npc(DUMMY, "VrmAutoTestDummy", 10, false, 1, 0, &v)];
    for (i, &(_, level, mult, min, max)) in LEVELS.iter().enumerate() {
        npcs.push(npc(
            LEVEL_NPCS + i as u32,
            &format!("VrmAutoTestLevel{i}"),
            level,
            mult,
            min,
            max,
            &v,
        ));
    }
    let plugin = write_plugin(
        0,
        &["Skyrim.esm"],
        "vibe_rm",
        "Tests run by script on loading a save; the reports come up in message boxes.",
        NEXT_OBJECT_ID,
        &[Node::top(b"NPC_", npcs), Node::top(b"QUST", quests)],
    );
    eprintln!(
        "door pair {} (inside) / {} (outside); other weather: {}",
        v.door_inside,
        v.door_outside,
        v.weather_slow
            .map_or("none".to_owned(), |(w, d)| format!("{w}, delta {d}"))
    );
    Ok(TestMod {
        plugin,
        scripts: scripts(&slow_label(&v))?,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use papyrus::vm::{Host, NativeResult, Vm};
    use papyrus::{ObjectId, Value};

    use super::*;
    use crate::test_mods::script::{mistyped_calls, natives};

    /// The game, as far as the tests ask it: our scripts and the natives'
    /// declarations, with made-up answers.
    struct Game {
        scripts: HashMap<String, Vec<u8>>,
        now: f64,
        weather: Value,
        weather_set_at: f64,
        boxes: Vec<String>,
        spawned: u32,
        /// How many boxes had come up when the save was requested.
        saved_at_box: Option<usize>,
    }

    impl Host for Game {
        fn load_script(&mut self, name: &str) -> Option<Vec<u8>> {
            self.scripts.get(&name.to_ascii_lowercase()).cloned()
        }

        fn call_native(
            &mut self,
            _class: &str,
            func: &str,
            this: Option<&Value>,
            args: &[Value],
        ) -> NativeResult {
            let v = match func.to_ascii_lowercase().as_str() {
                "wait" => {
                    let Some(Value::Float(s)) = args.first() else {
                        panic!("Wait({args:?})")
                    };
                    return NativeResult::Wait(*s);
                }
                "placeactoratme" => {
                    self.spawned += 1;
                    Value::Object(ObjectId::Form(0xFF00_0800 + self.spawned), "Actor".into())
                }
                "requestsave" => {
                    self.saved_at_box = Some(self.boxes.len());
                    Value::None
                }
                "getlevel" => Value::Int(7),
                "getbaseactorvalue" | "getactorvalue" => Value::Float(50.0),
                "getactorvaluepercentage" => Value::Float(0.5),
                "islocked" | "isininterior" => Value::Bool(false),
                "getlocklevel" => Value::Int(1),
                "isweapondrawn" | "ismovementcontrolsenabled" | "isfightingcontrolsenabled" => {
                    Value::Bool(true)
                }
                "getskymode" => Value::Int(3),
                "getvalue" => Value::Float(20.0),
                "getcurrentrealtime" => Value::Float(self.now as f32),
                "getcurrentgametime" => Value::Float((self.now / 72.0 / 60.0) as f32),
                "getcurrentweather" => self.weather.clone(),
                "setactive" => {
                    self.weather = this.cloned().unwrap();
                    self.weather_set_at = self.now;
                    Value::None
                }
                "getcurrentweathertransition" => {
                    Value::Float(((self.now - self.weather_set_at) / 10.0).min(1.0) as f32)
                }
                "floor" => {
                    let Some(Value::Float(f)) = args.first() else {
                        panic!("Floor({args:?})")
                    };
                    Value::Int(f.floor() as i32)
                }
                "messagebox" => {
                    let Some(Value::String(s)) = args.first() else {
                        panic!("MessageBox({args:?})")
                    };
                    self.boxes.push(s.to_string());
                    Value::None
                }
                _ => Value::None,
            };
            NativeResult::Value(v)
        }

        fn is_native_type(&self, _obj: ObjectId, _class: &str) -> bool {
            true
        }
    }

    #[test]
    fn the_tests_run_and_report() {
        let built = scripts("delta 75").unwrap();
        for (name, bytes) in &built {
            let pex = papyrus::pex::parse(bytes).unwrap();
            let bad = mistyped_calls(&pex);
            assert!(bad.is_empty(), "{name}: {bad:#?}");
        }
        let mut scripts: HashMap<String, Vec<u8>> = built
            .into_iter()
            .map(|(n, b)| (n.to_ascii_lowercase(), b))
            .collect();
        for c in natives() {
            scripts.insert(
                c.name.to_ascii_lowercase(),
                papyrus::build::native_class(c.name, c.parent, &c.functions).unwrap(),
            );
        }
        let weather = |id: u32| Value::Object(ObjectId::Form(id), "Weather".into());
        let form = |id: u32, class: &str| Value::Object(ObjectId::Form(id), class.into());
        let mut game = Game {
            scripts,
            now: 0.0,
            weather: weather(0x81A),
            weather_set_at: -100.0,
            boxes: Vec::new(),
            spawned: 0,
            saved_at_box: None,
        };
        let (quest, helper) = (
            ObjectId::Form(own(QUEST).0),
            ObjectId::Form(own(HELPER_QUEST).0),
        );
        let mut properties = vec![
            ("Player".to_owned(), form(0x14, "Actor")),
            ("Dummy".into(), form(own(DUMMY).0, "ActorBase")),
            ("DoorInside".into(), form(0x1CBAF, "ObjectReference")),
            ("DoorOutside".into(), form(0x1CBB0, "ObjectReference")),
            ("WeatherA".into(), weather(0x81A)),
            ("WeatherB".into(), weather(0x81B)),
            ("WeatherSlow".into(), weather(0x81C)),
            ("TimeScale".into(), form(0x3A, "GlobalVariable")),
            ("Helper".into(), form(own(HELPER_QUEST).0, HELPER)),
        ];
        for i in 0..LEVELS.len() {
            properties.push((
                format!("LevelNpc{i}"),
                form(own(LEVEL_NPCS).0 + i as u32, "ActorBase"),
            ));
        }
        let mut vm = Vm::new();
        assert!(vm.attach(&mut game, quest, SCRIPT, &properties));
        assert!(vm.attach(&mut game, helper, HELPER, &[]));
        assert_eq!(vm.send_event(&mut game, quest, "OnInit", vec![]), 1);
        assert_eq!(vm.send_event(&mut game, quest, "OnUpdate", vec![]), 1);
        while game.boxes.len() < TESTS.len() && game.now < 1000.0 {
            let now = game.now;
            vm.run(&mut game, now, 100_000);
            game.now += 0.05;
        }
        for b in &game.boxes {
            println!("{b}\n");
        }
        assert_eq!(game.boxes.len(), TESTS.len(), "every test reports");
        assert_eq!(game.saved_at_box, Some(0), "saves before the boxes");
        let all = game.boxes.join("\n");
        for expected in [
            "player level 7\n",
            "x1.5: 7 for 10.500\n",
            "SetAV 40: base 50, current 50, percent 0.5",
            "lock outside: inside locked False level 1",
            "this script disables movement, the helper enables it: movement True",
            "weapon drawn at start True, before disabling fighting True, after True",
            "delta 125: current at once True",
            "delta 125, accelerated:",
            "delta 75:",
        ] {
            assert!(all.contains(expected), "{expected:?} not in the reports");
        }
        // A second update doesn't run the tests again.
        vm.send_event(&mut game, quest, "OnUpdate", vec![]);
        let now = game.now;
        vm.run(&mut game, now, 100_000);
        assert_eq!(game.boxes.len(), TESTS.len());
    }
}
