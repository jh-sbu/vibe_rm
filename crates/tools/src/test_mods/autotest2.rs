//! `test-mod autotest2`: the first tier of `plans/autotest-survey.md`, the
//! ambiguities a script settles with records test mods already write and
//! without travelling. As `autotest`: loading a save with the plugin for
//! the first time starts `VrmAutoTest2Quest`, whose script runs each test in
//! turn, keeps each report in a `Result<Name>` variable, asks the game to
//! save and then shows the reports. `vrm-tool test-results` reads them from
//! that save.
//!
//! Load a save made outdoors (the weather and light tests need the sky)
//! near Riverwood, without the plugin, and leave the game alone (no menus:
//! they stop `Utility.Wait`) for about six minutes until the boxes come up.
//! The tests change the save for good (stats, a cleared location, map
//! markers, crime gold paid, time and weather moved, skills raised, quests
//! completed), so don't play on from the save they make.
//!
//! The tests and the readings they tell apart:
//!
//! - **Settings** (many files): `Game.GetGameSettingFloat / Int` of the
//!   settings the engine or `known_ambiguities/` name whose values the data
//!   doesn't hold, with two from the data and two that don't exist (what a
//!   missing one reads as) for comparison.
//! - **Levels** (`skills.md`): NPCs whose PC level mult product passes calc
//!   max (does it clamp? does max 0 mean none?), x0.5 with calc min 0, and
//!   a fixed level above calc max.
//! - **Templates** (`npc-templates.md`): NPCs templated on one in our
//!   faction, with and without the factions flag: is a missing part taken
//!   from the template?
//! - **Relationships** (`story-manager.md`): set on one reference, read both
//!   ways and from another reference of the same NPC.
//! - **Quest start** (`story-manager.md`): how many times a quest's `OnInit`
//!   has run when `Start()` returns, after `Wait(0)` and later; after a stop
//!   and a second start.
//! - **Map markers** (`map-markers.md`): `IsMapMarkerVisible` and
//!   `CanFastTravelToMarker` of far markers with each `FNAM` Skyrim.esm
//!   uses (0, 1, 3; 2 and 4 are the DLCs'); `AddToMap`
//!   without and with fast travel, and Locations Discovered.
//! - **Cleared** (`cleared.md`): Dungeons Cleared across `SetCleared` true,
//!   false, true on Embershard.
//! - **Animation events** (`object-graphs.md`): `RegisterForAnimationEvent`
//!   on a placed item (no graph), a disabled one and the player; events
//!   registered in another case than the graph sends them.
//! - **Player graph** (`player-body.md`): `IsNPC` and the other variables
//!   the player's graph is driven with, sheathed, drawn, in either view.
//! - **Hands** (`hands.md`): the player equips weapons, a shield, a torch
//!   and spells in turn; after each, what each hand holds, the equipped item
//!   types and the graph's hand types.
//! - **Gear** (`crime.md`): an NPC with nothing given better armor and
//!   weapons by `AddItem`: does it wear them, and how soon; a teammate.
//! - **Combat events** (`combat-state.md`): which hits send `OnHit` (script
//!   damage, a spell cast by script, blows, arrows) and with what; after
//!   `StopCombat`, how soon a fighter is in combat again.
//! - **Crime** (`crime.md`): `PlayerPayCrimeGold` with too little gold and
//!   with enough; an assault seen by two members of the victim's crime
//!   faction and one of another (gold once or per witness; to the witness's
//!   own faction or not); the bounties as the witnesses are killed.
//! - **Stats** (`journal.md`): `QueryStat` before and after kills by the
//!   player and by no one, skill increases, food and a potion, a side quest
//!   completed and a miscellaneous quest's objective and completion.
//! - **Weather time** (`weather.md`): delta 125 timed at the timescale set
//!   and at 60, and the other delta at 60: game or real time?
//! - **Light level** (`detection.md`): the player's `GetLightLevel` at each
//!   hour, clear weather forced.
//! - **Weather acceleration** (`weather.md`): without an override, normal
//!   and accelerated.
//!
//! First run in Skyrim SE (2026-10-13, player level 1, near Riverwood; the
//! idle camera came on part way, which no test reads): the scripts passed
//! objects to natives without casting them to the parameter's class (a
//! `Weapon` for a `Form`, an `Actor` for an `ObjectReference`), and the game
//! refuses such calls, so `AddItem`, `EquipItem`, `IsEquipped`, `MoveTo`,
//! `PlaceAtMe`, `Spell.Cast` and the player's animation event registrations
//! did nothing: the hands, gear, animation events, spell and archer hits,
//! food and potions and the crime test's gold went untested. `Code::typed`
//! casts them now and the tests check every call. The rest (in
//! `known_ambiguities/`):
//!
//! - Settings: the engine's fallbacks were the game's but `fWeaponBashMin`
//!   (0.1); `fSneakLightMoveMult` 0.01, `fSneakLightRunMult` 0.2,
//!   `fSneakCombatMod` -0.4, `iCombatStealthPointSneakDetectionThreshold` 70
//!   and the flee settings; a missing setting reads 0.
//! - Levels: calc max clamps, 0 is none, calc min 0 allows 0; a fixed level
//!   ignores both. Templates: missing factions aren't the template's.
//! - Relationships: both ways, per reference. Quest start: `OnInit` before the
//!   quest started, not at `Start()` nor a restart.
//! - Map markers: `FNAM` 1 shown, 2 travel; `AddToMap` doesn't count as
//!   discovered. Cleared: `SetCleared` from afar doesn't count.
//! - Player graph: `IsNPC` 0; spells are hand type 9. Combat: no `OnHit` for
//!   script damage; fists' source `Unarmed`; back in combat 0.44 s after
//!   `StopCombat`.
//! - Crime: the victim's faction gets the gold, a witness of another none;
//!   the bounty went with the last witness. Stats: kills by type, none for
//!   `Kill(None)`; side quests' statistic is a script's.
//! - Weather: transitions run on game time. Without an override the
//!   weather set didn't come in (rerun with each target the weather not on).
//! - Light level: the hours' values (`detection.md`).

use anyhow::{Context, Result, ensure};
use esp::writer::{AttachedScript, Node, PropertyValue, RecordBuilder, vmad, write_plugin};
use esp::{FormId, LoadOrder};
use papyrus::build::{Arg, FunctionBuilder, ScriptBuilder, auto_var, discard, id, string};
use papyrus::pex::op;

use super::script::{Code, bytes, measure, num, on_update, settle, spawn_level};
use super::{TestMod, own};

const QUEST: u32 = 0x800;
const START_QUEST: u32 = 0x801;
const SIDE_QUEST: u32 = 0x802;
const MISC_QUEST: u32 = 0x803;
const FACTION_A: u32 = 0x804;
const FACTION_B: u32 = 0x805;
const CRIME_1: u32 = 0x806;
const CRIME_2: u32 = 0x807;
const TEMPLATE_FACTION: u32 = 0x808;
const DUMMY: u32 = 0x809;
const TEMPLATE: u32 = 0x80A;
const DERIVED_NONE: u32 = 0x80B;
const DERIVED_FACTIONS: u32 = 0x80C;
const TARGET: u32 = 0x80D;
const FIGHTER: u32 = 0x80E;
const ARCHER: u32 = 0x80F;
/// The levels test's NPCs, one per [`LEVELS`] entry.
const LEVEL_NPCS: u32 = 0x810;
const NEXT_OBJECT_ID: u32 = LEVEL_NPCS + LEVELS.len() as u32;

const SCRIPT: &str = "VrmAutoTest2Script";
/// On the quest started by the quest start test: counts its `OnInit`s.
const START_SCRIPT: &str = "VrmAutoTest2StartScript";
/// On the combat test's target: passes its hits to the test script.
const HIT_SCRIPT: &str = "VrmAutoTest2HitScript";
const PLAYER_REF: FormId = FormId(0x14);
/// The miscellaneous quest's objective.
const MISC_OBJECTIVE: i32 = 10;
/// Where the test character stands, near Riverwood; map markers this far
/// away (units) and more are taken as undiscovered.
const RIVERWOOD: [f32; 2] = [18700.0, -46930.0];
const MARKER_DISTANCE: f32 = 60000.0;

/// The levels test's NPCs: label, `ACBS` level (the multiplier x 1000 with
/// the PC level mult flag), the flag, calc min and calc max. The player is
/// level 1.
const LEVELS: &[(&str, u16, bool, u16, u16)] = &[
    ("x5, max 2", 5000, true, 1, 2),
    ("x5, max 0", 5000, true, 1, 0),
    ("x0.5, min 0", 500, true, 0, 0),
    ("x0.5, min 1", 500, true, 1, 0),
    ("level 7, max 2, no mult", 7, false, 1, 2),
];

/// The settings read: those the engine uses whose values the data doesn't
/// hold (it has fallbacks for them) and those `known_ambiguities/` names
/// that the data lacks; then two the data holds (16, 1000) and two no one
/// has, for what a missing one reads as.
const SETTINGS: &[&str] = &[
    "fAISocial",
    "fCombatDetectionDialogueIdleMin",
    "fCombatFleeAllyDistanceMin",
    "fCombatFleeAllyRadius",
    "fCombatFleeBoostConfidence",
    "fCombatFleeBoostConfidenceTargetRadius",
    "fCombatFleeCoverMinDistance",
    "fCombatFleeCoverSearchRadius",
    "fCombatFleeDistance",
    "fCombatFleeDistanceExterior",
    "fCombatFleeDistanceInterior",
    "fCombatFleeDoorDistanceMax",
    "fCombatFleeInitialDoorRestrictChance",
    "fCombatFleeTargetAvoidRadius",
    "fCombatFleeTargetGatherRadius",
    "fCombatFleeUseDoorChance",
    "fCombatFleeWaitTime",
    "fCombatGroupCombatStrengthUpdateTime",
    "fCombatStealthPointRegenAlertWaitTime",
    "fCombatStrengthUpdateTime",
    "fConfidenceCowardly",
    "fConfidenceFoolhardy",
    "fFleeDistance",
    "fFleeDoneDistance",
    "fFleeIsSafeTimer",
    "fGrassStartFadeDistance",
    "fOverShoulderCombatAddY",
    "fOverShoulderCombatPosX",
    "fOverShoulderPosX",
    "fPartialPick",
    "fPlayerMaxResistance",
    "fSkillUsageLockPick",
    "fSneakCombatMod",
    "fSneakDistanceAttenuationExponent",
    "fSneakEquippedWeightBase",
    "fSneakEquippedWeightMult",
    "fSneakLightMoveMult",
    "fSneakLightRunMult",
    "fSneakNoticedMin",
    "fSneakStealthboyMult",
    "fVanityModeMaxDist",
    "fVanityModeMinDist",
    "fWeaponBashMin",
    "fXPPerSkillRank",
    "iCombatStealthPointSneakDetectionThreshold",
    "iMinGrassSize",
    "iSoundLevelLoud",
    "fWeatherTransAccel",
    "iMapMarkerRevealDistance",
    "fVrmNoSuchSetting",
    "iVrmNoSuchSetting",
];

/// Vanilla forms the scripts use, all from Skyrim.esm: property, type,
/// editor id. [`name_fn`] reports these by property name.
const VANILLA: &[(&str, &str, &str)] = &[
    ("IronSword", "Weapon", "IronSword"),
    ("SteelSword", "Weapon", "SteelSword"),
    ("IronDagger", "Weapon", "IronDagger"),
    ("IronGreatsword", "Weapon", "IronGreatsword"),
    ("HuntingBow", "Weapon", "HuntingBow"),
    ("IronArrow", "Ammo", "IronArrow"),
    ("IronShield", "Armor", "ArmorIronShield"),
    ("IronCuirass", "Armor", "ArmorIronCuirass"),
    ("SteelCuirass", "Armor", "ArmorSteelCuirassA"),
    ("Torch", "Light", "Torch01"),
    ("Flames", "Spell", "Flames"),
    ("Firebolt", "Spell", "Firebolt"),
    ("IncinerateLeftHand", "Spell", "IncinerateLeftHand"),
    ("FireStorm", "Spell", "FireStorm"),
    ("Apple", "Potion", "FoodApple"),
    ("HealthPotion", "Potion", "RestoreHealth01"),
    ("Gold", "MiscObject", "Gold001"),
    ("Hare", "ActorBase", "EncHare"),
    ("Wolf", "ActorBase", "EncWolf"),
    ("Embershard", "Location", "EmbershardLocation"),
    ("TimeScale", "GlobalVariable", "TimeScale"),
    ("GameHour", "GlobalVariable", "GameHour"),
    ("WeatherA", "Weather", "SkyrimClear"),
    ("WeatherB", "Weather", "SkyrimCloudy"),
];

/// Our own forms the scripts use: property, type, object id.
const OWN: &[(&str, &str, u32)] = &[
    ("Dummy", "ActorBase", DUMMY),
    ("Template", "ActorBase", TEMPLATE),
    ("DerivedNone", "ActorBase", DERIVED_NONE),
    ("DerivedFactions", "ActorBase", DERIVED_FACTIONS),
    ("Target", "ActorBase", TARGET),
    ("Fighter", "ActorBase", FIGHTER),
    ("Archer", "ActorBase", ARCHER),
    ("TemplateFaction", "Faction", TEMPLATE_FACTION),
    ("Crime1", "Faction", CRIME_1),
    ("Crime2", "Faction", CRIME_2),
    ("StartQuest", START_SCRIPT, START_QUEST),
    ("SideQuest", "Quest", SIDE_QUEST),
    ("MiscQuest", "Quest", MISC_QUEST),
];

/// The tests in the order run: name (the report's variable is
/// `Result<name>`), title. The weather and light tests come last: they
/// change the time and the weather.
const TESTS: &[(&str, &str)] = &[
    ("Settings", "Game settings"),
    ("Levels", "NPC level caps"),
    ("Templates", "Template fallbacks"),
    ("Relationships", "Relationship ranks"),
    ("QuestStart", "Quest OnInit timing"),
    ("MapMarkers", "Map marker flags"),
    ("Cleared", "Cleared locations"),
    ("AnimEvents", "Animation event registration"),
    ("PlayerGraph", "Player graph variables"),
    ("Hands", "Hands"),
    ("Gear", "NPCs taking up gear"),
    ("CombatEvents", "OnHit and StopCombat"),
    ("Crime", "Crime gold and witnesses"),
    ("Stats", "Statistics"),
    ("WeatherTime", "Weather transitions, game or real time"),
    ("LightLevel", "Light level by hour"),
    ("WeatherAccel", "Weather acceleration without override"),
];

/// `"text"` then `value` cast, appended to `r`.
fn put(c: &mut Code, r: &Arg, text: &str, value: Arg) {
    c.append(r.clone(), [string(text), value]);
}

fn line(c: &mut Code, r: &Arg, text: &str) {
    c.append(r.clone(), [string(text)]);
}

/// `form` cast to Form, for comparing with others.
fn as_form(c: &mut Code, form: Arg) -> Arg {
    let f = c.temp("Form");
    c.cast(f.clone(), form);
    f
}

/// `Name(f)`: `f` cast to a Form first (a call given another class is
/// refused).
fn name(c: &mut Code, f: Arg) -> Arg {
    let f = as_form(c, f);
    c.own("Name", "String", [f])
}

/// `Name(Form f)`: the property naming `f` (the player or a vanilla form),
/// "none", or `f` cast.
fn name_fn() -> FunctionBuilder {
    let mut c = Code::new("String");
    let f = c.param("f", "Form");
    let none = c.temp("Form");
    c.cast(none.clone(), Arg::None);
    let is_none = c.calc(op::CMP_EQ, "Bool", f.clone(), none);
    let next = c.label();
    c.jmpf(is_none, next).ret(string("none")).bind(next);
    let names = std::iter::once("Player").chain(VANILLA.iter().map(|v| v.0));
    for name in names {
        let p = as_form(&mut c, auto_var(name));
        let eq = c.calc(op::CMP_EQ, "Bool", f.clone(), p);
        let next = c.label();
        c.jmpf(eq, next).ret(string(name)).bind(next);
    }
    let s = c.temp("String");
    c.cast(s.clone(), f);
    c.finish(s)
}

/// `Place(ActorBase base, Float x, Float y)`: one placed by the player and
/// moved `x`, `y` from them.
fn place_fn() -> FunctionBuilder {
    let mut c = Code::new("Actor");
    let base = c.param("base", "ActorBase");
    let x = c.param("x", "Float");
    let y = c.param("y", "Float");
    let player = auto_var("Player");
    let a = c.method(
        "ObjectReference",
        player.clone(),
        "PlaceActorAtMe",
        [base, 4.into(), Arg::None],
    );
    c.method(
        "ObjectReference",
        a.clone(),
        "MoveTo",
        [player, x, y, 0.0f32.into(), false.into()],
    );
    c.finish(a)
}

fn place(c: &mut Code, base: &str, x: f32, y: f32) -> Arg {
    let a = c.own("Place", "Actor", [auto_var(base), x.into(), y.into()]);
    let v = c.temp("Actor");
    c.assign(v.clone(), a);
    v
}

fn remove(c: &mut Code, a: &Arg) {
    c.method("ObjectReference", a.clone(), "Disable", [false.into()]);
    c.method("ObjectReference", a.clone(), "Delete", []);
}

fn test_settings() -> FunctionBuilder {
    let mut c = Code::new("String");
    let r = c.local("r", "String");
    c.assign(r.clone(), string(""));
    for name in SETTINGS {
        let label = format!("{name} = ");
        if name.starts_with('f') {
            let v = c.global("Game", "GetGameSettingFloat", [string(name)]);
            c.numbers(&r, [(&label, v)]);
        } else {
            let v = c.global("Game", "GetGameSettingInt", [string(name)]);
            put(&mut c, &r, &label, v);
        }
        line(&mut c, &r, "\n");
    }
    c.finish(r)
}

fn test_levels() -> FunctionBuilder {
    let mut c = Code::new("String");
    let r = c.local("r", "String");
    let player_level = c.method("Actor", auto_var("Player"), "GetLevel", []);
    c.assign(r.clone(), string(""));
    put(&mut c, &r, "player level ", player_level);
    line(&mut c, &r, "\n");
    for (i, (label, ..)) in LEVELS.iter().enumerate() {
        let got = c.own("SpawnLevel", "Int", [auto_var(&format!("LevelNpc{i}"))]);
        put(&mut c, &r, &format!("{label}: "), got);
        line(&mut c, &r, "\n");
    }
    c.finish(r)
}

fn test_templates() -> FunctionBuilder {
    let mut c = Code::new("String");
    let r = c.local("r", "String");
    c.assign(r.clone(), string(""));
    let faction = auto_var("TemplateFaction");
    for (base, label) in [
        ("Template", "the template (rank 2)"),
        (
            "DerivedNone",
            "templated on it, factions not templated, none of its own",
        ),
        ("DerivedFactions", "templated on it, factions templated"),
    ] {
        let a = place(&mut c, base, 150.0, 0.0);
        let member = c.method("Actor", a.clone(), "IsInFaction", [faction.clone()]);
        let rank = c.method("Actor", a.clone(), "GetFactionRank", [faction.clone()]);
        put(&mut c, &r, &format!("{label}: member "), member);
        put(&mut c, &r, ", rank ", rank);
        line(&mut c, &r, "\n");
        remove(&mut c, &a);
    }
    c.finish(r)
}

fn test_relationships() -> FunctionBuilder {
    let mut c = Code::new("String");
    let r = c.local("r", "String");
    c.assign(r.clone(), string(""));
    let a1 = place(&mut c, "Dummy", 150.0, 0.0);
    let a2 = place(&mut c, "Dummy", 150.0, 100.0);
    let b = place(&mut c, "Template", 150.0, -100.0);
    let ranks = |c: &mut Code, label: &str| {
        put(c, &r, label, string(""));
        for (x, y, name) in [
            (&a1, &b, ": A1 to B "),
            (&b, &a1, ", B to A1 "),
            (&a2, &b, ", A2 to B "),
            (&b, &a2, ", B to A2 "),
        ] {
            let v = c.method("Actor", x.clone(), "GetRelationshipRank", [y.clone()]);
            put(c, &r, name, v);
        }
        line(c, &r, "\n");
    };
    line(&mut c, &r, "A1, A2 two of one NPC, B another\n");
    ranks(&mut c, "start");
    c.method(
        "Actor",
        a1.clone(),
        "SetRelationshipRank",
        [b.clone(), 3.into()],
    );
    ranks(&mut c, "A1.SetRelationshipRank(B, 3)");
    c.method(
        "Actor",
        b.clone(),
        "SetRelationshipRank",
        [a2.clone(), (-2).into()],
    );
    ranks(&mut c, "then B.SetRelationshipRank(A2, -2)");
    for a in [&a1, &a2, &b] {
        remove(&mut c, a);
    }
    c.finish(r)
}

fn test_quest_start() -> FunctionBuilder {
    let mut c = Code::new("String");
    let r = c.local("r", "String");
    c.assign(r.clone(), string(""));
    let q = auto_var("StartQuest");
    let state = |c: &mut Code, label: &str| {
        let inits = c.temp("Int");
        c.call_method(q.clone(), "GetInits", inits.clone(), []);
        let running = c.method("Quest", q.clone(), "IsRunning", []);
        put(c, &r, &format!("{label}: OnInit run "), inits);
        put(c, &r, " times, running ", running);
        line(c, &r, "\n");
    };
    state(&mut c, "before Start");
    let started = c.method("Quest", q.clone(), "Start", []);
    put(&mut c, &r, "Start() returned ", started);
    line(&mut c, &r, "\n");
    state(&mut c, "at once");
    c.wait(0.0);
    state(&mut c, "after Wait(0)");
    c.wait(0.5);
    state(&mut c, "after 0.5 s more");
    c.wait(2.0);
    state(&mut c, "after 2 s more");
    c.method("Quest", q.clone(), "Stop", []);
    c.wait(1.0);
    state(&mut c, "Stop(), 1 s later");
    let again = c.method("Quest", q.clone(), "Start", []);
    put(&mut c, &r, "Start() again returned ", again);
    line(&mut c, &r, "\n");
    state(&mut c, "at once");
    c.wait(2.0);
    state(&mut c, "after 2 s");
    c.method("Quest", q, "Stop", []);
    c.finish(r)
}

/// The map marker test's markers: property, label (`FNAM` and form id).
struct Markers {
    shown: Vec<(String, String)>,
    /// For `AddToMap(false)` and `AddToMap(true)`.
    add: [(String, String); 2],
}

fn test_map_markers(m: &Markers) -> FunctionBuilder {
    let mut c = Code::new("String");
    let r = c.local("r", "String");
    c.assign(r.clone(), string(""));
    let state = |c: &mut Code, prop: &str, label: &str| {
        let visible = c.method("ObjectReference", auto_var(prop), "IsMapMarkerVisible", []);
        let travel = c.method(
            "ObjectReference",
            auto_var(prop),
            "CanFastTravelToMarker",
            [],
        );
        let found = c.global("Game", "QueryStat", [string("Locations Discovered")]);
        put(c, &r, &format!("{label}: visible "), visible);
        put(c, &r, ", fast travel ", travel);
        put(c, &r, ", Locations Discovered ", found);
        line(c, &r, "\n");
    };
    for (prop, label) in &m.shown {
        state(&mut c, prop, label);
    }
    for ((prop, label), travel) in m.add.iter().zip([false, true]) {
        state(&mut c, prop, label);
        c.method(
            "ObjectReference",
            auto_var(prop),
            "AddToMap",
            [travel.into()],
        );
        c.wait(1.0);
        state(&mut c, prop, &format!("  then AddToMap({travel})"));
    }
    c.finish(r)
}

fn test_cleared() -> FunctionBuilder {
    let mut c = Code::new("String");
    let r = c.local("r", "String");
    c.assign(r.clone(), string("Embershard\n"));
    let loc = auto_var("Embershard");
    let state = |c: &mut Code, label: &str| {
        let cleared = c.method("Location", loc.clone(), "IsCleared", []);
        let stat = c.global("Game", "QueryStat", [string("Dungeons Cleared")]);
        put(c, &r, &format!("{label}: cleared "), cleared);
        put(c, &r, ", Dungeons Cleared ", stat);
        line(c, &r, "\n");
    };
    state(&mut c, "start");
    for (set, label) in [
        (true, "SetCleared(true)"),
        (false, "SetCleared(false)"),
        (true, "SetCleared(true) again"),
    ] {
        c.method("Location", loc.clone(), "SetCleared", [set.into()]);
        c.wait(1.0);
        state(&mut c, label);
    }
    c.finish(r)
}

/// Sheathes the player's weapon if drawn: disabling fighting does it
/// (`autotest`'s controls test).
fn sheathe(c: &mut Code) {
    let drawn = c.method("Actor", auto_var("Player"), "IsWeaponDrawn", []);
    let skip = c.label();
    c.jmpf(drawn, skip);
    c.controls(false, false, true);
    c.wait(3.0);
    c.enable_all();
    c.wait(0.5);
    c.bind(skip);
}

/// `OnAnimationEvent`: logged in `AnimLog`.
fn on_animation_event() -> FunctionBuilder {
    let mut c = Code::new("None");
    let source = c.param("akSource", "ObjectReference");
    let event = c.param("asEventName", "String");
    let who = name(&mut c, source);
    c.append(id("AnimLog"), [who, string(" "), event, string("; ")]);
    c.finish(Arg::None)
}

fn test_anim_events() -> FunctionBuilder {
    let mut c = Code::new("String");
    let r = c.local("r", "String");
    c.assign(r.clone(), string(""));
    let player = auto_var("Player");
    let placed = c.method(
        "ObjectReference",
        player.clone(),
        "PlaceAtMe",
        [auto_var("Gold"), 1.into(), false.into(), false.into()],
    );
    let placed_v = c.local("placed", "ObjectReference");
    c.assign(placed_v.clone(), placed);
    let disabled = c.method(
        "ObjectReference",
        player.clone(),
        "PlaceAtMe",
        [auto_var("Gold"), 1.into(), false.into(), true.into()],
    );
    let disabled_v = c.local("disabled", "ObjectReference");
    c.assign(disabled_v.clone(), disabled);
    let registrations = [
        (
            placed_v.clone(),
            "weaponDraw",
            "a placed item (no graph), weaponDraw",
        ),
        (
            disabled_v.clone(),
            "weaponDraw",
            "a disabled placed item, weaponDraw",
        ),
        (player.clone(), "WEAPONDRAW", "the player, WEAPONDRAW"),
        (player.clone(), "weapEquip_OUT", "the player, weapEquip_OUT"),
        (player.clone(), "weaponSheathe", "the player, weaponSheathe"),
    ];
    for (sender, event, label) in &registrations {
        let ok = c.method(
            "Form",
            id("self"),
            "RegisterForAnimationEvent",
            [sender.clone(), string(event)],
        );
        put(&mut c, &r, &format!("register on {label}: "), ok);
        line(&mut c, &r, "\n");
    }
    sheathe(&mut c);
    c.assign(id("AnimLog"), string(""));
    c.method("Actor", player.clone(), "DrawWeapon", []);
    c.wait(3.0);
    put(&mut c, &r, "events while drawing: ", id("AnimLog"));
    line(&mut c, &r, "\n");
    c.assign(id("AnimLog"), string(""));
    sheathe(&mut c);
    put(&mut c, &r, "events while sheathing: ", id("AnimLog"));
    line(&mut c, &r, "\n");
    for (sender, event, _) in &registrations {
        c.method(
            "Form",
            id("self"),
            "UnregisterForAnimationEvent",
            [sender.clone(), string(event)],
        );
    }
    remove(&mut c, &placed_v);
    remove(&mut c, &disabled_v);
    c.finish(r)
}

/// The player graph's variables read, by type.
const GRAPH_BOOLS: &[&str] = &[
    "IsNPC",
    "IsPlayer",
    "IsFirstPerson",
    "IsSneaking",
    "IsEquipping",
    "IsUnequipping",
    "bInJumpState",
    "bAnimationDriven",
    "bMotionDriven",
    "bHumanoidFootIKEnable",
    "bHeadTrackSpine",
];
const GRAPH_INTS: &[&str] = &[
    "iState",
    "iEquippedItemState",
    "iLeftHandType",
    "iRightHandType",
    "iWantBlock",
];
const GRAPH_FLOATS: &[&str] = &["Speed", "Direction"];

/// `GraphState(String label)`: the player graph's variables.
fn graph_state() -> FunctionBuilder {
    let mut c = Code::new("String");
    let label = c.param("label", "String");
    let r = c.local("r", "String");
    c.assign(r.clone(), label);
    line(&mut c, &r, ":");
    let player = auto_var("Player");
    for name in GRAPH_BOOLS {
        let v = c.method(
            "ObjectReference",
            player.clone(),
            "GetAnimationVariableBool",
            [string(name)],
        );
        put(&mut c, &r, &format!(" {name} "), v);
    }
    for name in GRAPH_INTS {
        let v = c.method(
            "ObjectReference",
            player.clone(),
            "GetAnimationVariableInt",
            [string(name)],
        );
        put(&mut c, &r, &format!(" {name} "), v);
    }
    for name in GRAPH_FLOATS {
        let v = c.method(
            "ObjectReference",
            player.clone(),
            "GetAnimationVariableFloat",
            [string(name)],
        );
        c.numbers(&r, [(&format!(" {name} "), v)]);
    }
    line(&mut c, &r, "\n");
    c.finish(r)
}

fn test_player_graph() -> FunctionBuilder {
    let mut c = Code::new("String");
    let r = c.local("r", "String");
    c.assign(r.clone(), string(""));
    let state = |c: &mut Code, label: &str| {
        let s = c.own("GraphState", "String", [string(label)]);
        c.append(r.clone(), [s]);
    };
    sheathe(&mut c);
    state(&mut c, "sheathed");
    c.method("Actor", auto_var("Player"), "DrawWeapon", []);
    c.wait(3.0);
    state(&mut c, "drawn");
    sheathe(&mut c);
    state(&mut c, "sheathed again");
    c.global("Game", "ForceFirstPerson", []);
    c.wait(1.5);
    state(&mut c, "first person");
    c.global("Game", "ForceThirdPerson", []);
    c.wait(1.5);
    state(&mut c, "third person");
    c.finish(r)
}

/// `HandState(String label)`: what the player's hands hold.
fn hand_state() -> FunctionBuilder {
    let mut c = Code::new("String");
    let label = c.param("label", "String");
    let r = c.local("r", "String");
    let player = auto_var("Player");
    c.assign(r.clone(), label);
    let right = c.method("Actor", player.clone(), "GetEquippedWeapon", [false.into()]);
    let left = c.method("Actor", player.clone(), "GetEquippedWeapon", [true.into()]);
    let shield = c.method("Actor", player.clone(), "GetEquippedShield", []);
    let spell_left = c.method("Actor", player.clone(), "GetEquippedSpell", [0.into()]);
    let spell_right = c.method("Actor", player.clone(), "GetEquippedSpell", [1.into()]);
    for (text, form) in [
        (": weapon R ", right),
        (", L ", left),
        ("; shield ", shield),
        ("; spell L ", spell_left),
        (", R ", spell_right),
    ] {
        let held = name(&mut c, form);
        put(&mut c, &r, text, held);
    }
    let torch = c.method("Actor", player.clone(), "IsEquipped", [auto_var("Torch")]);
    put(&mut c, &r, "; torch ", torch);
    let type_left = c.method("Actor", player.clone(), "GetEquippedItemType", [0.into()]);
    let type_right = c.method("Actor", player.clone(), "GetEquippedItemType", [1.into()]);
    put(&mut c, &r, "; item types L ", type_left);
    put(&mut c, &r, ", R ", type_right);
    for (text, var) in [("; graph L ", "iLeftHandType"), (", R ", "iRightHandType")] {
        let v = c.method(
            "ObjectReference",
            player.clone(),
            "GetAnimationVariableInt",
            [string(var)],
        );
        put(&mut c, &r, text, v);
    }
    line(&mut c, &r, "\n");
    c.finish(r)
}

/// The hands test's items added to the player: property, count.
const HAND_ITEMS: &[(&str, i32)] = &[
    ("IronSword", 2),
    ("IronDagger", 1),
    ("IronGreatsword", 1),
    ("HuntingBow", 1),
    ("IronShield", 1),
    ("Torch", 1),
];
/// The hands test's spells, taught if not known and forgotten again.
const HAND_SPELLS: &[&str] = &["Flames", "IncinerateLeftHand", "FireStorm"];

/// One step of the hands test.
enum Hand {
    Equip(&'static str),
    Unequip(&'static str),
    /// Spell, hand (0 left, 1 right).
    Spell(&'static str, i32),
    /// One of the item leaves the inventory.
    Drop(&'static str),
}

const HAND_STEPS: &[(Hand, &str)] = &[
    (
        Hand::Equip("IronSword"),
        "EquipItem(sword), the first of two",
    ),
    (Hand::Equip("IronSword"), "EquipItem(sword) again"),
    (Hand::Equip("IronDagger"), "EquipItem(dagger)"),
    (Hand::Equip("IronShield"), "EquipItem(shield)"),
    (Hand::Equip("Torch"), "EquipItem(torch)"),
    (Hand::Spell("Flames", 0), "EquipSpell(Flames, left)"),
    (Hand::Spell("Flames", 1), "EquipSpell(Flames, right)"),
    (Hand::Equip("IronSword"), "EquipItem(sword)"),
    (Hand::Equip("IronGreatsword"), "EquipItem(greatsword)"),
    (Hand::Spell("Flames", 0), "EquipSpell(Flames, left)"),
    (
        Hand::Spell("IncinerateLeftHand", 1),
        "EquipSpell(IncinerateLeftHand, right)",
    ),
    (
        Hand::Spell("FireStorm", 0),
        "EquipSpell(FireStorm (both hands), left)",
    ),
    (Hand::Equip("IronSword"), "EquipItem(sword)"),
    (Hand::Equip("IronSword"), "EquipItem(sword) again"),
    (Hand::Drop("IronSword"), "RemoveItem(sword, 1)"),
    (Hand::Equip("HuntingBow"), "EquipItem(bow)"),
    (Hand::Unequip("HuntingBow"), "UnequipItem(bow)"),
];

fn test_hands() -> FunctionBuilder {
    let mut c = Code::new("String");
    let r = c.local("r", "String");
    c.assign(r.clone(), string(""));
    let player = auto_var("Player");
    let state = |c: &mut Code, label: &str| {
        c.wait(0.5);
        let s = c.own("HandState", "String", [string(label)]);
        c.append(r.clone(), [s]);
    };
    sheathe(&mut c);
    state(&mut c, "start");
    // What the hands held, put away for the test and back after.
    let held = [
        ("origRight", "Weapon", "GetEquippedWeapon", Some(false)),
        ("origLeft", "Weapon", "GetEquippedWeapon", Some(true)),
        ("origShield", "Armor", "GetEquippedShield", None),
    ];
    let mut kept = Vec::new();
    for (var, ty, getter, left) in held {
        let v = c.local(var, ty);
        let got = match left {
            Some(l) => c.method("Actor", player.clone(), getter, [l.into()]),
            None => c.method("Actor", player.clone(), getter, []),
        };
        c.assign(v.clone(), got);
        kept.push(v);
    }
    let mut kept_spells = Vec::new();
    for (var, hand) in [("origSpellLeft", 0), ("origSpellRight", 1)] {
        let v = c.local(var, "Spell");
        let got = c.method("Actor", player.clone(), "GetEquippedSpell", [hand.into()]);
        c.assign(v.clone(), got);
        kept_spells.push((v, hand));
    }
    let none_form = c.temp("Form");
    c.cast(none_form.clone(), Arg::None);
    let if_some = |c: &mut Code, v: &Arg, then: &mut dyn FnMut(&mut Code)| {
        let f = as_form(c, v.clone());
        let missing = c.calc(op::CMP_EQ, "Bool", f, none_form.clone());
        let skip = c.label();
        c.jmpt(missing, skip);
        then(c);
        c.bind(skip);
    };
    for v in &kept {
        if_some(&mut c, v, &mut |c| {
            c.method(
                "Actor",
                player.clone(),
                "UnequipItem",
                [v.clone(), false.into(), true.into()],
            );
        });
    }
    for (v, hand) in &kept_spells {
        if_some(&mut c, v, &mut |c| {
            c.method(
                "Actor",
                player.clone(),
                "UnequipSpell",
                [v.clone(), (*hand).into()],
            );
        });
    }
    // The items: counts before, to take away only ours after.
    let mut counts = Vec::new();
    for (item, n) in HAND_ITEMS {
        let before = c.method(
            "ObjectReference",
            player.clone(),
            "GetItemCount",
            [auto_var(item)],
        );
        let v = c.local(&format!("had{item}"), "Int");
        c.assign(v.clone(), before);
        counts.push((item, v));
        c.method(
            "ObjectReference",
            player.clone(),
            "AddItem",
            [auto_var(item), (*n).into(), true.into()],
        );
    }
    let mut known = Vec::new();
    for spell in HAND_SPELLS {
        let had = c.method("Actor", player.clone(), "HasSpell", [auto_var(spell)]);
        let v = c.local(&format!("knew{spell}"), "Bool");
        c.assign(v.clone(), had);
        known.push((spell, v.clone()));
        let skip = c.label();
        c.jmpt(v, skip);
        c.method(
            "Actor",
            player.clone(),
            "AddSpell",
            [auto_var(spell), false.into()],
        );
        c.bind(skip);
    }
    state(&mut c, "hands emptied, items given");
    for (step, label) in HAND_STEPS {
        match step {
            Hand::Equip(item) => {
                c.method(
                    "Actor",
                    player.clone(),
                    "EquipItem",
                    [auto_var(item), false.into(), true.into()],
                );
            }
            Hand::Unequip(item) => {
                c.method(
                    "Actor",
                    player.clone(),
                    "UnequipItem",
                    [auto_var(item), false.into(), true.into()],
                );
            }
            Hand::Spell(spell, hand) => {
                c.method(
                    "Actor",
                    player.clone(),
                    "EquipSpell",
                    [auto_var(spell), (*hand).into()],
                );
            }
            Hand::Drop(item) => {
                c.method(
                    "ObjectReference",
                    player.clone(),
                    "RemoveItem",
                    [auto_var(item), 1.into(), true.into(), Arg::None],
                );
            }
        }
        state(&mut c, label);
    }
    // Put back: ours taken away, spells forgotten, the hands as they were.
    for (item, had) in counts {
        c.method(
            "Actor",
            player.clone(),
            "UnequipItem",
            [auto_var(item), false.into(), true.into()],
        );
        let now = c.method(
            "ObjectReference",
            player.clone(),
            "GetItemCount",
            [auto_var(item)],
        );
        let extra = c.calc(op::ISUB, "Int", now, had);
        c.method(
            "ObjectReference",
            player.clone(),
            "RemoveItem",
            [auto_var(item), extra, true.into(), Arg::None],
        );
    }
    for (spell, knew) in known {
        for hand in [0, 1] {
            c.method(
                "Actor",
                player.clone(),
                "UnequipSpell",
                [auto_var(spell), hand.into()],
            );
        }
        let skip = c.label();
        c.jmpt(knew, skip);
        c.method("Actor", player.clone(), "RemoveSpell", [auto_var(spell)]);
        c.bind(skip);
    }
    for v in &kept {
        if_some(&mut c, v, &mut |c| {
            c.method(
                "Actor",
                player.clone(),
                "EquipItem",
                [v.clone(), false.into(), true.into()],
            );
        });
    }
    for (v, hand) in &kept_spells {
        if_some(&mut c, v, &mut |c| {
            c.method(
                "Actor",
                player.clone(),
                "EquipSpell",
                [v.clone(), (*hand).into()],
            );
        });
    }
    state(&mut c, "put back");
    c.finish(r)
}

/// `GearState(Actor a, String label)`: what an NPC wears and holds.
fn gear_state() -> FunctionBuilder {
    let mut c = Code::new("String");
    let a = c.param("a", "Actor");
    let label = c.param("label", "String");
    let r = c.local("r", "String");
    c.assign(r.clone(), label);
    for (text, item) in [
        (": iron cuirass worn ", "IronCuirass"),
        (", steel cuirass worn ", "SteelCuirass"),
    ] {
        let worn = c.method("Actor", a.clone(), "IsEquipped", [auto_var(item)]);
        put(&mut c, &r, text, worn);
    }
    let weapon = c.method("Actor", a.clone(), "GetEquippedWeapon", [false.into()]);
    let weapon = name(&mut c, weapon);
    put(&mut c, &r, "; weapon ", weapon);
    let shield = c.method("Actor", a.clone(), "GetEquippedShield", []);
    let shield = name(&mut c, shield);
    put(&mut c, &r, "; shield ", shield);
    line(&mut c, &r, "\n");
    c.finish(r)
}

fn test_gear() -> FunctionBuilder {
    let mut c = Code::new("String");
    let r = c.local("r", "String");
    c.assign(r.clone(), string(""));
    let state = |c: &mut Code, a: &Arg, label: &str| {
        let s = c.own("GearState", "String", [a.clone(), string(label)]);
        c.append(r.clone(), [s]);
    };
    let a = place(&mut c, "Dummy", 150.0, 150.0);
    c.wait(1.0);
    state(&mut c, &a, "an NPC with nothing");
    for (item, label) in [
        ("IronCuirass", "an iron cuirass"),
        ("SteelCuirass", "a steel cuirass"),
        ("IronSword", "an iron sword"),
        ("SteelSword", "a steel sword"),
        ("IronShield", "an iron shield"),
        ("IronGreatsword", "an iron greatsword"),
    ] {
        c.method(
            "ObjectReference",
            a.clone(),
            "AddItem",
            [auto_var(item), 1.into(), true.into()],
        );
        c.wait(0.5);
        state(&mut c, &a, &format!("given {label}, 0.5 s"));
        c.wait(3.0);
        state(&mut c, &a, "  3.5 s");
    }
    let b = place(&mut c, "Dummy", 150.0, -150.0);
    c.method(
        "Actor",
        b.clone(),
        "SetPlayerTeammate",
        [true.into(), false.into()],
    );
    c.wait(1.0);
    c.method(
        "ObjectReference",
        b.clone(),
        "AddItem",
        [auto_var("IronCuirass"), 1.into(), true.into()],
    );
    c.wait(3.5);
    state(&mut c, &b, "a teammate given an iron cuirass, 3.5 s");
    c.method(
        "Actor",
        b.clone(),
        "SetPlayerTeammate",
        [false.into(), false.into()],
    );
    remove(&mut c, &a);
    remove(&mut c, &b);
    c.finish(r)
}

/// `LogHit(...)`, called by the target's script on each `OnHit`: a line in
/// `HitLog`.
fn log_hit() -> FunctionBuilder {
    let mut c = Code::new("None");
    let aggressor = c.param("akAggressor", "ObjectReference");
    let source = c.param("akSource", "Form");
    let projectile = c.param("akProjectile", "Projectile");
    let flags = [
        "abPowerAttack",
        "abSneakAttack",
        "abBashAttack",
        "abHitBlocked",
    ]
    .map(|n| c.param(n, "Bool"));
    let log = id("HitLog");
    let now = c.global("Utility", "GetCurrentRealTime", []);
    let t = c.calc(op::FSUB, "Float", now, id("HitStart"));
    let t = c.own("Num", "String", [t]);
    let who = c.local("who", "String");
    let named = name(&mut c, aggressor.clone());
    c.assign(who.clone(), named);
    for (var, label) in [("FighterRef", "the fighter"), ("ArcherRef", "the archer")] {
        let a = as_form(&mut c, aggressor.clone());
        let b = as_form(&mut c, id(var));
        let eq = c.calc(op::CMP_EQ, "Bool", a, b);
        let skip = c.label();
        c.jmpf(eq, skip)
            .assign(who.clone(), string(label))
            .bind(skip);
    }
    let source = name(&mut c, source);
    c.append(
        log.clone(),
        [
            t,
            string(" s: by "),
            who,
            string(", source "),
            source,
            string(", projectile "),
            projectile,
        ],
    );
    for (flag, name) in flags.into_iter().zip(["power", "sneak", "bash", "blocked"]) {
        c.append(log.clone(), [string(&format!(", {name} ")), flag]);
    }
    c.append(log, [string("\n")]);
    c.finish(Arg::None)
}

fn test_combat_events() -> FunctionBuilder {
    let mut c = Code::new("String");
    let r = c.local("r", "String");
    c.assign(r.clone(), string(""));
    let start = c.global("Utility", "GetCurrentRealTime", []);
    c.assign(id("HitStart"), start);
    c.assign(id("HitLog"), string(""));
    let mark = |c: &mut Code, label: &str| {
        c.append(id("HitLog"), [string(&format!("-- {label}\n"))]);
    };
    let target = place(&mut c, "Target", 400.0, 0.0);
    c.method(
        "Actor",
        target.clone(),
        "SetActorValue",
        [string("Health"), 2000.0f32.into()],
    );
    c.wait(1.0);
    c.method(
        "Actor",
        target.clone(),
        "DamageActorValue",
        [string("Health"), 5.0f32.into()],
    );
    c.wait(1.0);
    mark(&mut c, "after DamageActorValue(Health, 5) by script");

    let fighter = place(&mut c, "Fighter", 250.0, 0.0);
    c.assign(id("FighterRef"), fighter.clone());
    c.method(
        "Actor",
        fighter.clone(),
        "SetActorValue",
        [string("Health"), 2000.0f32.into()],
    );
    c.method(
        "Spell",
        auto_var("Firebolt"),
        "Cast",
        [fighter.clone(), target.clone()],
    );
    c.wait(2.0);
    mark(&mut c, "after Firebolt.Cast(fighter, target)");
    c.method(
        "ObjectReference",
        fighter.clone(),
        "AddItem",
        [auto_var("IronSword"), 1.into(), true.into()],
    );
    c.method(
        "Actor",
        fighter.clone(),
        "EquipItem",
        [auto_var("IronSword"), false.into(), true.into()],
    );
    c.method("Actor", fighter.clone(), "StartCombat", [target.clone()]);
    c.wait(8.0);
    mark(&mut c, "8 s of the fighter (iron sword) attacking");

    // StopCombat: how soon is the fighter (an enemy, aggressive) fighting
    // again?
    let before = c.method("Actor", fighter.clone(), "GetCombatState", []);
    c.method("Actor", fighter.clone(), "StopCombat", []);
    let stopped = c.global("Utility", "GetCurrentRealTime", []);
    let at_once = c.method("Actor", fighter.clone(), "GetCombatState", []);
    let back = c.local("back", "Float");
    c.assign(back.clone(), (-1.0f32).into());
    let (top, done) = (c.label(), c.label());
    c.bind(top);
    c.wait(0.25);
    let now = c.global("Utility", "GetCurrentRealTime", []);
    let dt = c.calc(op::FSUB, "Float", now, stopped.clone());
    let over = c.calc(op::CMP_GT, "Bool", dt.clone(), 15.0f32.into());
    c.jmpt(over, done);
    let state = c.method("Actor", fighter.clone(), "GetCombatState", []);
    let idle = c.calc(op::CMP_EQ, "Bool", state, 0.into());
    c.jmpt(idle, top);
    c.assign(back.clone(), dt);
    c.bind(done);
    put(
        &mut c,
        &r,
        "fighter's combat state before StopCombat ",
        before,
    );
    put(&mut c, &r, ", right after ", at_once);
    c.numbers(
        &r,
        [(", fighting again after (s; -1 not within 15 s) ", back)],
    );
    line(&mut c, &r, "\n");
    c.method("Actor", fighter.clone(), "StopCombat", []);
    remove(&mut c, &fighter);
    c.method("Actor", target.clone(), "StopCombat", []);

    let archer = place(&mut c, "Archer", -200.0, 0.0);
    c.assign(id("ArcherRef"), archer.clone());
    for (item, n) in [("HuntingBow", 1), ("IronArrow", 40)] {
        c.method(
            "ObjectReference",
            archer.clone(),
            "AddItem",
            [auto_var(item), n.into(), true.into()],
        );
        c.method(
            "Actor",
            archer.clone(),
            "EquipItem",
            [auto_var(item), false.into(), true.into()],
        );
    }
    c.method("Actor", archer.clone(), "StartCombat", [target.clone()]);
    c.wait(12.0);
    mark(&mut c, "12 s of the archer (hunting bow, iron arrows)");
    c.method("Actor", archer.clone(), "StopCombat", []);
    c.method("Actor", target.clone(), "StopCombat", []);
    remove(&mut c, &archer);
    remove(&mut c, &target);
    put(&mut c, &r, "OnHit on the target:\n", id("HitLog"));
    c.finish(r)
}

/// `CrimeState(String label)`: both crime factions' bounties.
fn crime_state() -> FunctionBuilder {
    let mut c = Code::new("String");
    let label = c.param("label", "String");
    let r = c.local("r", "String");
    c.assign(r.clone(), label);
    for (faction, name) in [("Crime1", ": crime 1 "), ("Crime2", "; crime 2 ")] {
        let gold = c.method("Faction", auto_var(faction), "GetCrimeGold", []);
        let violent = c.method("Faction", auto_var(faction), "GetCrimeGoldViolent", []);
        put(&mut c, &r, name, gold);
        put(&mut c, &r, " (violent ", violent);
        line(&mut c, &r, ")");
    }
    line(&mut c, &r, "\n");
    c.finish(r)
}

/// The crime test's statistics, read at its start and end.
const CRIME_STATS: &[&str] = &[
    "Assaults",
    "Murders",
    "Fines Paid",
    "Total Lifetime Bounty",
    "Largest Bounty",
];

fn stat_line(c: &mut Code, r: &Arg, label: &str, stats: &[&str]) {
    put(c, r, label, string(":"));
    for stat in stats {
        let v = c.global("Game", "QueryStat", [string(stat)]);
        put(c, r, &format!(" {stat} "), v);
    }
    line(c, r, "\n");
}

fn test_crime() -> FunctionBuilder {
    let mut c = Code::new("String");
    let r = c.local("r", "String");
    c.assign(r.clone(), string(""));
    let player = auto_var("Player");
    let gold = auto_var("Gold");
    let crime1 = auto_var("Crime1");
    stat_line(&mut c, &r, "stats at the start", CRIME_STATS);
    let state = |c: &mut Code, label: &str| {
        let s = c.own("CrimeState", "String", [string(label)]);
        c.append(r.clone(), [s]);
    };
    // Paying with too little gold, then with enough.
    let had = c.method(
        "ObjectReference",
        player.clone(),
        "GetItemCount",
        [gold.clone()],
    );
    let had_v = c.local("hadGold", "Int");
    c.assign(had_v.clone(), had);
    c.method(
        "ObjectReference",
        player.clone(),
        "RemoveItem",
        [gold.clone(), had_v.clone(), true.into(), Arg::None],
    );
    for (give, label) in [(100, "gold 100"), (1000, "gold 1000 more")] {
        c.method(
            "ObjectReference",
            player.clone(),
            "AddItem",
            [gold.clone(), give.into(), true.into()],
        );
        c.method("Faction", crime1.clone(), "SetCrimeGold", [500.into()]);
        c.method(
            "Faction",
            crime1.clone(),
            "PlayerPayCrimeGold",
            [false.into(), false.into()],
        );
        c.wait(0.5);
        let left = c.method(
            "ObjectReference",
            player.clone(),
            "GetItemCount",
            [gold.clone()],
        );
        let bounty = c.method("Faction", crime1.clone(), "GetCrimeGold", []);
        put(
            &mut c,
            &r,
            &format!("bounty 500, {label}, PlayerPayCrimeGold(false, false): gold "),
            left,
        );
        put(&mut c, &r, ", bounty ", bounty);
        line(&mut c, &r, "\n");
    }
    let now = c.method(
        "ObjectReference",
        player.clone(),
        "GetItemCount",
        [gold.clone()],
    );
    c.method(
        "ObjectReference",
        player.clone(),
        "RemoveItem",
        [gold.clone(), now, true.into(), Arg::None],
    );
    c.method(
        "ObjectReference",
        player.clone(),
        "AddItem",
        [gold.clone(), had_v, true.into()],
    );
    c.method("Faction", crime1.clone(), "SetCrimeGold", [0.into()]);

    // An assault seen by two of the victim's crime faction and one of
    // another.
    line(
        &mut c,
        &r,
        "victim and witnesses 1, 2 in crime faction 1, witness 3 in crime faction 2\n",
    );
    let victim = place(&mut c, "Dummy", 200.0, 0.0);
    let w1 = place(&mut c, "Dummy", 150.0, 150.0);
    let w2 = place(&mut c, "Dummy", 150.0, -150.0);
    let w3 = place(&mut c, "Dummy", 250.0, 100.0);
    for (a, faction) in [
        (&victim, "Crime1"),
        (&w1, "Crime1"),
        (&w2, "Crime1"),
        (&w3, "Crime2"),
    ] {
        c.method("Actor", a.clone(), "SetCrimeFaction", [auto_var(faction)]);
    }
    c.wait(2.0);
    state(&mut c, "before");
    c.method("Actor", victim.clone(), "SendAssaultAlarm", []);
    c.wait(3.0);
    state(&mut c, "victim.SendAssaultAlarm(), 3 s");
    for (who, label) in [
        (vec![&w3], "witness 3 killed (Kill(None))"),
        (vec![&w1, &w2], "witnesses 1 and 2 killed"),
        (vec![&victim], "the victim killed"),
    ] {
        for a in who {
            c.method("Actor", a.clone(), "Kill", [Arg::None]);
        }
        c.wait(1.5);
        state(&mut c, label);
    }
    stat_line(&mut c, &r, "stats at the end", CRIME_STATS);
    for faction in ["Crime1", "Crime2"] {
        c.method("Faction", auto_var(faction), "SetCrimeGold", [0.into()]);
        c.method(
            "Faction",
            auto_var(faction),
            "SetCrimeGoldViolent",
            [0.into()],
        );
    }
    for a in [&victim, &w1, &w2, &w3] {
        remove(&mut c, a);
    }
    c.finish(r)
}

/// The kill statistics.
const KILL_STATS: &[&str] = &[
    "People Killed",
    "Animals Killed",
    "Creatures Killed",
    "Bunnies Slaughtered",
    "Murders",
];
const USE_STATS: &[&str] = &["Food Eaten", "Ingredients Eaten", "Potions Used"];
const QUEST_STATS: &[&str] = &[
    "Quests Completed",
    "Side Quests Completed",
    "Misc Objectives Completed",
];

/// Statistics before and after `action`, a line each.
fn stats_around(
    c: &mut Code,
    r: &Arg,
    label: &str,
    stats: &[&str],
    action: impl FnOnce(&mut Code),
) {
    let before: Vec<Arg> = stats
        .iter()
        .map(|s| {
            let v = c.global("Game", "QueryStat", [string(s)]);
            let keep = c.temp("Int");
            c.assign(keep.clone(), v);
            keep
        })
        .collect();
    action(c);
    c.wait(1.5);
    put(c, r, label, string(":"));
    for (stat, before) in stats.iter().zip(before) {
        let after = c.global("Game", "QueryStat", [string(stat)]);
        put(c, r, &format!(" {stat} "), before);
        put(c, r, " to ", after);
    }
    line(c, r, "\n");
}

fn test_stats() -> FunctionBuilder {
    let mut c = Code::new("String");
    let r = c.local("r", "String");
    c.assign(r.clone(), string(""));
    let player = auto_var("Player");
    for (base, killer, label) in [
        ("Hare", true, "a hare, Kill(player)"),
        ("Hare", false, "a hare, Kill(None)"),
        ("Wolf", true, "a wolf, Kill(player)"),
        (
            "Dummy",
            true,
            "an NPC of ours (no crime faction), Kill(player)",
        ),
    ] {
        let a = place(&mut c, base, 300.0, 0.0);
        let by = if killer { player.clone() } else { Arg::None };
        stats_around(&mut c, &r, label, KILL_STATS, |c| {
            c.method("Actor", a.clone(), "Kill", [by]);
        });
        remove(&mut c, &a);
    }
    stats_around(
        &mut c,
        &r,
        "IncrementSkill(Alchemy)",
        &["Skill Increases"],
        |c| {
            c.global("Game", "IncrementSkill", [string("Alchemy")]);
        },
    );
    stats_around(
        &mut c,
        &r,
        "IncrementSkillBy(Alchemy, 3)",
        &["Skill Increases"],
        |c| {
            c.global("Game", "IncrementSkillBy", [string("Alchemy"), 3.into()]);
        },
    );
    for (item, label) in [
        ("Apple", "an apple eaten (EquipItem)"),
        ("HealthPotion", "a health potion drunk (EquipItem)"),
    ] {
        stats_around(&mut c, &r, label, USE_STATS, |c| {
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
        });
    }
    stats_around(
        &mut c,
        &r,
        "a side quest started and completed (CompleteQuest)",
        QUEST_STATS,
        |c| {
            c.method("Quest", auto_var("SideQuest"), "Start", []);
            c.wait(0.5);
            c.method("Quest", auto_var("SideQuest"), "CompleteQuest", []);
        },
    );
    let misc = auto_var("MiscQuest");
    stats_around(
        &mut c,
        &r,
        "a misc quest's objective shown and completed",
        QUEST_STATS,
        |c| {
            c.method("Quest", misc.clone(), "Start", []);
            c.wait(0.5);
            c.method(
                "Quest",
                misc.clone(),
                "SetObjectiveDisplayed",
                [MISC_OBJECTIVE.into(), true.into(), false.into()],
            );
            c.wait(0.5);
            c.method(
                "Quest",
                misc.clone(),
                "SetObjectiveCompleted",
                [MISC_OBJECTIVE.into(), true.into()],
            );
        },
    );
    stats_around(
        &mut c,
        &r,
        "the misc quest completed (CompleteQuest)",
        QUEST_STATS,
        |c| {
            c.method("Quest", misc.clone(), "CompleteQuest", []);
        },
    );
    c.finish(r)
}

/// Weather tests' start: none indoors.
fn outdoors_or_skip(c: &mut Code, r: &Arg) -> papyrus::build::Label {
    let inside = c.method("Actor", auto_var("Player"), "IsInInterior", []);
    let current = c.global("Weather", "GetCurrentWeather", []);
    let time_scale = c.method("GlobalVariable", auto_var("TimeScale"), "GetValue", []);
    put(c, r, "indoors ", inside.clone());
    put(c, r, ", weather ", current);
    c.numbers(r, [(", timescale ", time_scale)]);
    line(c, r, "\n");
    let (outdoors, end) = (c.label(), c.label());
    c.jmpf(inside, outdoors);
    line(c, r, "skipped: run outdoors\n");
    c.jmp(end);
    c.bind(outdoors);
    end
}

/// Whichever of A and B isn't on, then the other.
fn pick_weathers(c: &mut Code) -> (Arg, Arg) {
    let first = c.local("first", "Weather");
    let second = c.local("second", "Weather");
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
    (first, second)
}

fn measure_line(c: &mut Code, r: &Arg, w: Arg, overriding: bool, accelerate: bool, label: &str) {
    let settled = c.own("Settle", "String", []);
    c.append(r.clone(), [settled]);
    let line = c.own(
        "Measure",
        "String",
        [w, overriding.into(), accelerate.into(), string(label)],
    );
    c.append(r.clone(), [line]);
}

fn test_weather_time(slow_label: &str) -> FunctionBuilder {
    let mut c = Code::new("String");
    let r = c.local("r", "String");
    c.assign(r.clone(), string(""));
    let end = outdoors_or_skip(&mut c, &r);
    let time_scale = auto_var("TimeScale");
    let was = c.method("GlobalVariable", time_scale.clone(), "GetValue", []);
    let was_v = c.local("timeScaleWas", "Float");
    c.assign(was_v.clone(), was);
    let (first, second) = pick_weathers(&mut c);
    measure_line(
        &mut c,
        &r,
        first,
        true,
        false,
        "timescale as set, delta 125",
    );
    c.method(
        "GlobalVariable",
        time_scale.clone(),
        "SetValue",
        [60.0f32.into()],
    );
    measure_line(&mut c, &r, second, true, false, "timescale 60, delta 125");
    let slow = auto_var("WeatherSlow");
    let none = c.temp("Weather");
    c.cast(none.clone(), Arg::None);
    let missing = c.calc(op::CMP_EQ, "Bool", slow.clone(), none);
    let skip = c.label();
    c.jmpt(missing, skip);
    measure_line(
        &mut c,
        &r,
        slow,
        true,
        false,
        &format!("timescale 60, {slow_label}"),
    );
    c.bind(skip);
    c.method("GlobalVariable", time_scale, "SetValue", [was_v]);
    c.bind(end);
    c.finish(r)
}

fn test_light_level() -> FunctionBuilder {
    let mut c = Code::new("String");
    let r = c.local("r", "String");
    c.assign(r.clone(), string(""));
    let end = outdoors_or_skip(&mut c, &r);
    let hour = auto_var("GameHour");
    let was = c.method("GlobalVariable", hour.clone(), "GetValue", []);
    let was_v = c.local("hourWas", "Float");
    c.assign(was_v.clone(), was);
    c.method(
        "Weather",
        auto_var("WeatherA"),
        "ForceActive",
        [true.into()],
    );
    line(&mut c, &r, "SkyrimClear forced; hour: light level\n");
    for h in 0..24 {
        c.method(
            "GlobalVariable",
            hour.clone(),
            "SetValue",
            [(h as f32).into()],
        );
        c.wait(1.5);
        let level = c.method("Actor", auto_var("Player"), "GetLightLevel", []);
        c.numbers(&r, [(&format!("{h}: "), level)]);
        line(&mut c, &r, "\n");
    }
    c.method("GlobalVariable", hour, "SetValue", [was_v]);
    c.bind(end);
    c.finish(r)
}

fn test_weather_accel() -> FunctionBuilder {
    let mut c = Code::new("String");
    let r = c.local("r", "String");
    c.assign(r.clone(), string(""));
    let end = outdoors_or_skip(&mut c, &r);
    c.global("Weather", "ReleaseOverride", []);
    line(&mut c, &r, "override released\n");
    for (i, (accelerate, label)) in [
        (false, "no override, delta 125"),
        (true, "no override, delta 125, accelerated"),
    ]
    .into_iter()
    .enumerate()
    {
        // Whichever of A and B isn't on now.
        let target = c.local(&format!("target{i}"), "Weather");
        let (a, b) = (auto_var("WeatherA"), auto_var("WeatherB"));
        let current = c.global("Weather", "GetCurrentWeather", []);
        let is_a = c.calc(op::CMP_EQ, "Bool", current, a.clone());
        let keep = c.label();
        c.assign(target.clone(), a)
            .jmpf(is_a, keep)
            .assign(target.clone(), b)
            .bind(keep);
        put(&mut c, &r, "setting ", target.clone());
        line(&mut c, &r, "\n");
        measure_line(&mut c, &r, target, false, accelerate, label);
        let current = c.global("Weather", "GetCurrentWeather", []);
        let outgoing = c.global("Weather", "GetOutgoingWeather", []);
        put(&mut c, &r, "  then current ", current);
        put(&mut c, &r, ", outgoing ", outgoing);
        line(&mut c, &r, "\n");
    }
    c.bind(end);
    c.finish(r)
}

/// What the scripts are built for: the other weather's label and the map
/// markers.
struct Setup {
    slow_label: String,
    markers: Markers,
}

fn scripts(setup: &Setup) -> Result<Vec<(String, Vec<u8>)>> {
    let mut s = ScriptBuilder::new(SCRIPT, "Quest");
    s.auto_property("Player", "Actor");
    s.auto_property("WeatherSlow", "Weather");
    for (name, ty, _) in VANILLA {
        s.auto_property(name, ty);
    }
    for (name, ty, _) in OWN {
        s.auto_property(name, ty);
    }
    for i in 0..LEVELS.len() {
        s.auto_property(&format!("LevelNpc{i}"), "ActorBase");
    }
    for (name, _) in setup.markers.shown.iter().chain(setup.markers.add.iter()) {
        s.auto_property(name, "ObjectReference");
    }
    s.variable("Ran", "Bool", false.into());
    for (name, ty) in [
        ("AnimLog", "String"),
        ("HitLog", "String"),
        ("HitStart", "Float"),
        ("FighterRef", "Actor"),
        ("ArcherRef", "Actor"),
    ] {
        let init = match ty {
            "String" => string(""),
            "Float" => 0.0f32.into(),
            _ => Arg::None,
        };
        s.variable(name, ty, init);
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
        .function("OnUpdate", on_update("autotest2", "six minutes", TESTS))
        .function("OnAnimationEvent", on_animation_event())
        .function("Num", num())
        .function("Name", name_fn())
        .function("Place", place_fn())
        .function("SpawnLevel", spawn_level())
        .function("GraphState", graph_state())
        .function("HandState", hand_state())
        .function("GearState", gear_state())
        .function("CrimeState", crime_state())
        .function("LogHit", log_hit())
        .function("Settle", settle())
        .function("Measure", measure())
        .function("TestSettings", test_settings())
        .function("TestLevels", test_levels())
        .function("TestTemplates", test_templates())
        .function("TestRelationships", test_relationships())
        .function("TestQuestStart", test_quest_start())
        .function("TestMapMarkers", test_map_markers(&setup.markers))
        .function("TestCleared", test_cleared())
        .function("TestAnimEvents", test_anim_events())
        .function("TestPlayerGraph", test_player_graph())
        .function("TestHands", test_hands())
        .function("TestGear", test_gear())
        .function("TestCombatEvents", test_combat_events())
        .function("TestCrime", test_crime())
        .function("TestStats", test_stats())
        .function("TestWeatherTime", test_weather_time(&setup.slow_label))
        .function("TestLightLevel", test_light_level())
        .function("TestWeatherAccel", test_weather_accel());

    // The started quest's script: counts its OnInits.
    let mut q = ScriptBuilder::new(START_SCRIPT, "Quest");
    q.variable("Inits", "Int", 0.into());
    let mut on_init = Code::new("None");
    on_init.op(op::IADD, [id("Inits"), id("Inits"), 1.into()]);
    q.function("OnInit", on_init.finish(Arg::None));
    let get = Code::new("Int");
    q.function("GetInits", get.finish(id("Inits")));

    // The combat target's script: its hits to the test script.
    let mut h = ScriptBuilder::new(HIT_SCRIPT, "Actor");
    h.auto_property("Main", SCRIPT);
    let mut on_hit = Code::new("None");
    let params = [
        ("akAggressor", "ObjectReference"),
        ("akSource", "Form"),
        ("akProjectile", "Projectile"),
        ("abPowerAttack", "Bool"),
        ("abSneakAttack", "Bool"),
        ("abBashAttack", "Bool"),
        ("abHitBlocked", "Bool"),
    ]
    .map(|(n, t)| on_hit.param(n, t));
    on_hit.call_method(auto_var("Main"), "LogHit", discard(), params);
    h.function("OnHit", on_hit.finish(Arg::None));

    Ok(vec![
        (SCRIPT.into(), papyrus::pex::write(&s.build()?)?),
        (START_SCRIPT.into(), papyrus::pex::write(&q.build()?)?),
        (HIT_SCRIPT.into(), papyrus::pex::write(&h.build()?)?),
    ])
}

/// The vanilla forms by editor id, all from Skyrim.esm.
struct Vanilla<'a> {
    lo: &'a LoadOrder,
}

impl Vanilla<'_> {
    fn id(&self, edid: &str) -> Result<FormId> {
        let id = self
            .lo
            .find_editor_id(edid)
            .with_context(|| format!("{edid} not found"))?;
        ensure!(id.0 >> 24 == 0, "{edid} ({id}) isn't from Skyrim.esm");
        Ok(id)
    }
}

/// The first of the game's weathers with a trans delta other than 125 (and
/// not 0, at once), and that delta.
fn other_weather(lo: &LoadOrder) -> Option<(FormId, u8)> {
    lo.ids_of_type(b"WTHR").iter().find_map(|&w| {
        let rec = lo.get(w)?;
        let delta = *rec.get(b"DATA")?.get(3)?;
        let edid = rec.editor_id()?;
        (w.0 >> 24 == 0 && edid.starts_with("Skyrim") && delta != 125 && delta != 0)
            .then_some((w, delta))
    })
}

/// A map marker and its label.
type Marker = (FormId, String);

/// Map markers in Tamriel far from Riverwood: the first with each `FNAM`
/// (Skyrim.esm's have 0, 1 and 3), and two more with 0 for `AddToMap`. Each as (form id,
/// label).
fn pick_markers(lo: &LoadOrder) -> Result<(Vec<Marker>, [Marker; 2])> {
    let mut by_flags: Vec<(u8, FormId)> = Vec::new();
    let mut spare = Vec::new();
    for &r in lo.ids_of_type(b"REFR") {
        if r.0 >> 24 != 0 {
            continue;
        }
        let Some(rec) = lo.get(r) else { continue };
        if rec.get(b"XMRK").is_none() {
            continue;
        }
        let world = lo
            .cell_of_ref(r)
            .and_then(|c| lo.cell(c))
            .and_then(|c| c.world);
        if world != Some(FormId(0x3C)) {
            continue;
        }
        let Some(pos) = rec.get(b"DATA").filter(|d| d.len() >= 8) else {
            continue;
        };
        let x = f32::from_le_bytes(pos[0..4].try_into().unwrap());
        let y = f32::from_le_bytes(pos[4..8].try_into().unwrap());
        if (x - RIVERWOOD[0]).hypot(y - RIVERWOOD[1]) < MARKER_DISTANCE {
            continue;
        }
        let flags = rec
            .get(b"FNAM")
            .and_then(|d| d.first().copied())
            .unwrap_or(0);
        if !by_flags.iter().any(|&(f, _)| f == flags) {
            by_flags.push((flags, r));
        } else if flags == 0 && spare.len() < 2 {
            spare.push(r);
        }
    }
    by_flags.sort();
    let shown = by_flags
        .into_iter()
        .map(|(f, r)| (r, format!("FNAM {f} ({r})")))
        .collect();
    ensure!(spare.len() == 2, "too few far map markers");
    Ok((
        shown,
        [
            (spare[0], format!("FNAM 0 ({})", spare[0])),
            (spare[1], format!("FNAM 0 ({})", spare[1])),
        ],
    ))
}

/// An NPC of our own: Traits from the template, our stats.
struct Npc {
    object_id: u32,
    edid: String,
    /// Its name; `None` for none of its own.
    name: Option<&'static str>,
    /// `ACBS` level (the multiplier x 1000 with the PC level mult flag),
    /// the flag, calc min and max.
    level: (u16, bool, u16, u16),
    /// Its template and the template flags (0x1 traits, 0x4 factions).
    template: (FormId, u16),
    /// Aggression (0 unaggressive, 1 aggressive), confidence (0 cowardly
    /// to 4 foolhardy).
    temper: (u8, u8),
    factions: Vec<(FormId, i8)>,
    script: Option<AttachedScript>,
}

impl Npc {
    fn record(self, race: FormId, class: FormId) -> RecordBuilder {
        let (level, pc_mult, min, max) = self.level;
        let flags: u32 = if pc_mult { 0x80 } else { 0 };
        // ACBS: flags, magicka and stamina offsets, level, calc min / max,
        // speed 100, disposition 35, template flags, health offset,
        // bleedout override.
        let acbs: [u8; 24] = bytes(&[
            &flags.to_le_bytes(),
            &0i16.to_le_bytes(),
            &0i16.to_le_bytes(),
            &level.to_le_bytes(),
            &min.to_le_bytes(),
            &max.to_le_bytes(),
            &100u16.to_le_bytes(),
            &35i16.to_le_bytes(),
            &self.template.1.to_le_bytes(),
            &0i16.to_le_bytes(),
            &0u16.to_le_bytes(),
        ]);
        // AIDT: aggression, confidence, energy 50, no crime, neutral mood,
        // helps nobody.
        let mut aidt = [0u8; 20];
        aidt[..4].copy_from_slice(&[self.temper.0, self.temper.1, 50, 3]);
        // DNAM: 18 skills at 5 and no offsets, health 30, magicka 25,
        // stamina 70, far away model distance 0, geared up weapons 0.
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
        let mut rec = RecordBuilder::new(b"NPC_", own(self.object_id)).zstring(b"EDID", &self.edid);
        if let Some(script) = self.script {
            rec = rec.field(b"VMAD", &vmad(&[script]));
        }
        rec = rec.field(b"OBND", &bounds).field(b"ACBS", &acbs);
        // SNAM: faction, rank, unused.
        for &(faction, rank) in &self.factions {
            let snam: [u8; 8] = bytes(&[&faction.0.to_le_bytes(), &[rank as u8, 0, 0, 0]]);
            rec = rec.field(b"SNAM", &snam);
        }
        rec = rec
            .form(b"TPLT", self.template.0)
            .form(b"RNAM", race)
            .field(b"AIDT", &aidt)
            .form(b"CNAM", class);
        if let Some(name) = self.name {
            rec = rec.zstring(b"FULL", name);
        }
        rec.field(b"DATA", &[])
            .field(b"DNAM", &dnam)
            .field(b"NAM5", &[0xFF, 0])
            .f32(b"NAM6", 1.0)
            .f32(b"NAM7", 50.0)
            .u32(b"NAM8", 1) // sound level: normal
    }
}

/// A quest: `DNAM` flags (0x1 start game enabled, 0x10 starts enabled),
/// its type (0 none, 6 miscellaneous, 8 side quest), its script and its
/// objectives (index, text).
fn quest(
    object_id: u32,
    edid: &str,
    name: &str,
    flags: u16,
    kind: u32,
    script: Option<AttachedScript>,
    objectives: &[(u16, &str)],
) -> RecordBuilder {
    // DNAM: flags, priority, unknown, unknown, type.
    let dnam: [u8; 12] = bytes(&[
        &flags.to_le_bytes(),
        &[0, 0],
        &0u32.to_le_bytes(),
        &kind.to_le_bytes(),
    ]);
    let mut rec = RecordBuilder::new(b"QUST", own(object_id)).zstring(b"EDID", edid);
    if let Some(script) = script {
        rec = rec.field(b"VMAD", &vmad(&[script]));
    }
    rec = rec
        .zstring(b"FULL", name)
        .field(b"DNAM", &dnam)
        .field(b"NEXT", &[]);
    for &(index, text) in objectives {
        rec = rec
            .field(b"QOBJ", &index.to_le_bytes())
            .u32(b"FNAM", 0)
            .zstring(b"NNAM", text);
    }
    rec.u32(b"ANAM", 0) // next alias id
}

/// A faction: `DATA` flags (0x40 tracks crime), its reactions to others
/// (faction, `XNAM` group combat reaction: 1 enemy) and crime values.
fn faction(object_id: u32, edid: &str, flags: u32, enemies: &[u32]) -> RecordBuilder {
    let mut rec = RecordBuilder::new(b"FACT", own(object_id))
        .zstring(b"EDID", edid)
        .zstring(b"FULL", edid);
    for &enemy in enemies {
        let xnam: [u8; 12] = bytes(&[
            &own(enemy).0.to_le_bytes(),
            &0i32.to_le_bytes(),
            &1u32.to_le_bytes(),
        ]);
        rec = rec.field(b"XNAM", &xnam);
    }
    // CRVA: no arrest, no attack on sight, murder 1000, assault 40,
    // trespass 5, pickpocket 25, unknown, steal mult 0.5, escape 100,
    // werewolf 1000 (the defaults UESP gives).
    let crva: [u8; 20] = bytes(&[
        &[0, 0],
        &1000u16.to_le_bytes(),
        &40u16.to_le_bytes(),
        &5u16.to_le_bytes(),
        &25u16.to_le_bytes(),
        &0u16.to_le_bytes(),
        &0.5f32.to_le_bytes(),
        &100u16.to_le_bytes(),
        &1000u16.to_le_bytes(),
    ]);
    rec.u32(b"DATA", flags).field(b"CRVA", &crva)
}

pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    let v = Vanilla { lo };
    let template = v.id("WEMerchantMiscMaleCommoner")?;
    let race = v.id("ImperialRace")?;
    let class = v.id("Citizen")?;
    let slow = other_weather(lo);
    let (shown, add) = pick_markers(lo)?;

    let object = PropertyValue::Object;
    let mut properties = vec![
        ("Player".to_owned(), object(PLAYER_REF)),
        (
            "WeatherSlow".into(),
            object(slow.map_or(FormId(0), |(w, _)| w)),
        ),
    ];
    for (name, _, edid) in VANILLA {
        properties.push((name.to_string(), object(v.id(edid)?)));
    }
    for (name, _, object_id) in OWN {
        properties.push((name.to_string(), object(own(*object_id))));
    }
    for i in 0..LEVELS.len() {
        properties.push((format!("LevelNpc{i}"), object(own(LEVEL_NPCS + i as u32))));
    }
    let markers = Markers {
        shown: shown
            .iter()
            .enumerate()
            .map(|(i, (_, label))| (format!("Marker{i}"), label.clone()))
            .collect(),
        add: [
            ("AddMarker0".into(), add[0].1.clone()),
            ("AddMarker1".into(), add[1].1.clone()),
        ],
    };
    for ((name, _), (r, _)) in markers.shown.iter().zip(&shown) {
        properties.push((name.clone(), object(*r)));
    }
    for ((name, _), (r, _)) in markers.add.iter().zip(&add) {
        properties.push((name.clone(), object(*r)));
    }

    let quests = vec![
        quest(
            QUEST,
            "VrmAutoTest2Quest",
            "vibe_rm Autotest 2",
            0x0011,
            0,
            Some(AttachedScript {
                name: SCRIPT.into(),
                properties,
            }),
            &[],
        ),
        quest(
            START_QUEST,
            "VrmAutoTest2StartQuest",
            "vibe_rm Autotest 2: started by script",
            0,
            0,
            Some(AttachedScript {
                name: START_SCRIPT.into(),
                properties: Vec::new(),
            }),
            &[],
        ),
        quest(
            SIDE_QUEST,
            "VrmAutoTest2SideQuest",
            "vibe_rm Autotest 2: side quest",
            0,
            8,
            None,
            &[],
        ),
        quest(
            MISC_QUEST,
            "VrmAutoTest2MiscQuest",
            "vibe_rm Autotest 2: misc quest",
            0,
            6,
            None,
            &[(MISC_OBJECTIVE as u16, "vibe_rm autotest objective")],
        ),
    ];
    let factions = vec![
        faction(FACTION_A, "VrmAutoTest2FactionA", 0, &[FACTION_B]),
        faction(FACTION_B, "VrmAutoTest2FactionB", 0, &[FACTION_A]),
        faction(CRIME_1, "VrmAutoTest2Crime1", 0x40, &[]),
        faction(CRIME_2, "VrmAutoTest2Crime2", 0x40, &[]),
        faction(TEMPLATE_FACTION, "VrmAutoTest2TemplateFaction", 0, &[]),
    ];
    let plain = |object_id: u32, edid: &str, name: Option<&'static str>| Npc {
        object_id,
        edid: edid.to_owned(),
        name,
        level: (10, false, 1, 0),
        template: (template, 0x1),
        temper: (0, 0),
        factions: Vec::new(),
        script: None,
    };
    let mut npcs = vec![
        plain(DUMMY, "VrmAutoTest2Dummy", Some("vibe_rm Test NPC")),
        Npc {
            factions: vec![(own(TEMPLATE_FACTION), 2)],
            ..plain(TEMPLATE, "VrmAutoTest2Template", Some("vibe_rm Template"))
        },
        Npc {
            template: (own(TEMPLATE), 0x1),
            ..plain(DERIVED_NONE, "VrmAutoTest2DerivedNone", None)
        },
        Npc {
            template: (own(TEMPLATE), 0x5),
            ..plain(DERIVED_FACTIONS, "VrmAutoTest2DerivedFactions", None)
        },
        Npc {
            temper: (0, 2),
            factions: vec![(own(FACTION_B), 0)],
            script: Some(AttachedScript {
                name: HIT_SCRIPT.into(),
                properties: vec![("Main".into(), object(own(QUEST)))],
            }),
            ..plain(TARGET, "VrmAutoTest2Target", Some("vibe_rm Target"))
        },
        Npc {
            temper: (1, 3),
            factions: vec![(own(FACTION_A), 0)],
            ..plain(FIGHTER, "VrmAutoTest2Fighter", Some("vibe_rm Fighter"))
        },
        Npc {
            temper: (1, 3),
            factions: vec![(own(FACTION_A), 0)],
            ..plain(ARCHER, "VrmAutoTest2Archer", Some("vibe_rm Archer"))
        },
    ];
    for (i, &(_, level, mult, min, max)) in LEVELS.iter().enumerate() {
        npcs.push(Npc {
            level: (level, mult, min, max),
            ..plain(
                LEVEL_NPCS + i as u32,
                &format!("VrmAutoTest2Level{i}"),
                Some("vibe_rm Test NPC"),
            )
        });
    }
    let npcs = npcs.into_iter().map(|n| n.record(race, class)).collect();
    let plugin = write_plugin(
        0,
        &["Skyrim.esm"],
        "vibe_rm",
        "Tier 1 of plans/autotest-survey.md: tests run by script on loading a save, reports saved.",
        NEXT_OBJECT_ID,
        &[
            Node::top(b"FACT", factions),
            Node::top(b"NPC_", npcs),
            Node::top(b"QUST", quests),
        ],
    );
    let slow_label = match slow {
        Some((_, delta)) => format!("delta {delta}"),
        None => "(no other delta)".into(),
    };
    eprintln!(
        "other weather: {}; map markers: {}; for AddToMap: {}, {}",
        slow.map_or("none".to_owned(), |(w, d)| format!("{w}, delta {d}")),
        shown
            .iter()
            .map(|(_, l)| l.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        add[0].1,
        add[1].1,
    );
    Ok(TestMod {
        plugin,
        scripts: scripts(&Setup {
            slow_label,
            markers,
        })?,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use papyrus::vm::{Host, NativeResult, Vm};
    use papyrus::{ObjectId, Value};

    use super::*;
    use crate::test_mods::script::{mistyped_calls, native, natives};

    /// The game, as far as the tests ask it: our scripts, the natives'
    /// declarations, and for each native an answer of its type (some
    /// made up to keep the tests going).
    struct Game {
        scripts: HashMap<String, Vec<u8>>,
        now: f64,
        weather: Value,
        weather_set_at: f64,
        boxes: Vec<String>,
        placed: u32,
        saved_at_box: Option<usize>,
    }

    impl Host for Game {
        fn load_script(&mut self, name: &str) -> Option<Vec<u8>> {
            self.scripts.get(&name.to_ascii_lowercase()).cloned()
        }

        fn call_native(
            &mut self,
            class: &str,
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
                "placeactoratme" | "placeatme" => {
                    self.placed += 1;
                    Value::Object(ObjectId::Form(0xFF00_0800 + self.placed), "Actor".into())
                }
                "getcurrentrealtime" => Value::Float(self.now as f32),
                "getcurrentgametime" => Value::Float((self.now / 72.0 / 60.0) as f32),
                "getcurrentweather" => self.weather.clone(),
                "setactive" | "forceactive" => {
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
                "requestsave" => {
                    self.saved_at_box = Some(self.boxes.len());
                    Value::None
                }
                _ => match native(class, func).1.1.to_ascii_lowercase().as_str() {
                    "int" => Value::Int(1),
                    "float" => Value::Float(1.0),
                    "bool" => Value::Bool(false),
                    "string" => Value::String("".into()),
                    _ => Value::None,
                },
            };
            NativeResult::Value(v)
        }

        fn is_native_type(&self, _obj: ObjectId, _class: &str) -> bool {
            true
        }
    }

    #[test]
    fn the_tests_run_and_report() {
        let setup = Setup {
            slow_label: "delta 82".into(),
            markers: Markers {
                shown: (0..3)
                    .map(|i| (format!("Marker{i}"), format!("FNAM {i}")))
                    .collect(),
                add: [
                    ("AddMarker0".into(), "FNAM 0 (a)".into()),
                    ("AddMarker1".into(), "FNAM 0 (b)".into()),
                ],
            },
        };
        let built = scripts(&setup).unwrap();
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
        let form = |id: u32, class: &str| Value::Object(ObjectId::Form(id), class.into());
        let mut game = Game {
            scripts,
            now: 0.0,
            weather: form(0x81A, "Weather"),
            weather_set_at: -100.0,
            boxes: Vec::new(),
            placed: 0,
            saved_at_box: None,
        };
        let mut properties = vec![
            ("Player".to_owned(), form(0x14, "Actor")),
            ("WeatherSlow".into(), form(0x105F40, "Weather")),
        ];
        for (i, (name, ty, _)) in VANILLA.iter().enumerate() {
            properties.push((name.to_string(), form(0x1000 + i as u32, ty)));
        }
        for (name, ty, object_id) in OWN {
            properties.push((name.to_string(), form(own(*object_id).0, ty)));
        }
        for i in 0..LEVELS.len() {
            properties.push((
                format!("LevelNpc{i}"),
                form(own(LEVEL_NPCS + i as u32).0, "ActorBase"),
            ));
        }
        for (i, (name, _)) in setup
            .markers
            .shown
            .iter()
            .chain(&setup.markers.add)
            .enumerate()
        {
            properties.push((name.clone(), form(0x2000 + i as u32, "ObjectReference")));
        }
        let (quest, start_quest) = (
            ObjectId::Form(own(QUEST).0),
            ObjectId::Form(own(START_QUEST).0),
        );
        let mut vm = Vm::new();
        assert!(vm.attach(&mut game, quest, SCRIPT, &properties));
        assert!(vm.attach(&mut game, start_quest, START_SCRIPT, &[]));
        assert_eq!(vm.send_event(&mut game, quest, "OnInit", vec![]), 1);
        assert_eq!(vm.send_event(&mut game, quest, "OnUpdate", vec![]), 1);
        while game.boxes.len() < TESTS.len() && game.now < 3000.0 {
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
            "fFleeIsSafeTimer = 1\n",
            "iVrmNoSuchSetting = 1\n",
            "x5, max 2: 1\n",
            "the template (rank 2): member False, rank 1\n",
            "A1.SetRelationshipRank(B, 3): A1 to B 1",
            "after Wait(0): OnInit run 0 times, running False\n",
            "FNAM 2: visible False, fast travel False, Locations Discovered 1\n",
            "  then AddToMap(true): visible",
            "SetCleared(true) again: cleared False, Dungeons Cleared 1\n",
            "register on the player, WEAPONDRAW: False\n",
            "drawn: IsNPC False IsPlayer False",
            "EquipItem(sword), the first of two: weapon R none, L none; shield none",
            "given an iron cuirass, 0.5 s: iron cuirass worn False",
            "-- after Firebolt.Cast(fighter, target)\n",
            "right after 1, fighting again after (s; -1 not within 15 s) 0.",
            "bounty 500, gold 100, PlayerPayCrimeGold(false, false): gold 1, bounty 1\n",
            "the victim killed: crime 1 1 (violent 1); crime 2 1 (violent 1)\n",
            "a hare, Kill(player): People Killed 1 to 1",
            "a misc quest's objective shown and completed: Quests Completed 1 to 1",
            "timescale 60, delta 125: current at once True",
            "timescale 60, delta 82:",
            "23: 1\n",
            "no override, delta 125, accelerated: current at once True",
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
