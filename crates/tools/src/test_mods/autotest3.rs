//! `test-mod autotest3`: the follow-up to `autotest2`, for what its runs left
//! open. As there: loading a save with the plugin for the first time starts
//! `VrmAutoTest3Quest`, whose script runs the tests, keeps each report in a
//! `Result<Name>` variable, asks the game to save and shows the reports;
//! `vrm-tool test-results` reads them from that save.
//!
//! Load the same save as for `autotest2` (outdoors near Riverwood, without
//! these plugins) and leave the game alone for about eight minutes.
//!
//! - **Weather** (`known_ambiguities/weather.md`, transition speed): in
//!   `autotest2` a weather set without an override after `ReleaseOverride`
//!   never came in, so acceleration without an override couldn't be timed.
//!   Here, first thing (no override yet), Clear or Cloudy (whichever isn't
//!   on; Riverwood's region lists both) is set with `SetActive(false, false)`,
//!   then the other with `SetActive(false, true)`; then `ForceActive(false)`;
//!   then a weather `FindWeather` picks from the region; then the weather
//!   spell's way (`magicSetWeatherScript`: the weather to come back to set
//!   without an override and accelerated, another with one, then
//!   `ReleaseOverride`), timing the way back; then `SetActive(false, false)`
//!   again after that release. Each reports the current and outgoing weather
//!   and the transition after it.
//! - **Sword hits** (`combat-state.md`, `OnHit` sources): `autotest2`'s
//!   fighter landed no blow with an iron sword. Here the target is held in
//!   place (`SetRestrained`) beside the fighter, and the fighter's state,
//!   target, weapon and distance are logged every two seconds for 20.
//! - **Arrow hits**: `autotest2`'s archer's hits came with no projectile.
//!   Here an archer 800 units off, with its arrows counted before and after,
//!   each hit logged with the archer's distance.
//!
//! Run in Skyrim SE (2026-10-13):
//!
//! - Weather: without an override Clear and Cloudy came in as with one
//!   (23.0 s), accelerated or not; `ForceActive(false)` at once;
//!   `FindWeather` gave Cloudy for pleasant and None for cloudy;
//!   `ReleaseOverride` brought the weather from before back (going out at
//!   once, in after 22.3 s); after it, `SetActive(false, false)` worked.
//! - Sword: ten hits in 20 s from 60 to 160 units, the sword as the source,
//!   no projectile, one a power attack.
//! - Arrows: three hits from 580 to 1090 units, the bow as the source, no
//!   projectile; the archer's arrows weren't used up.

use anyhow::Result;
use esp::writer::{AttachedScript, Node, PropertyValue, write_plugin};
use esp::{FormId, LoadOrder};
use papyrus::build::{Arg, FunctionBuilder, ScriptBuilder, auto_var, discard, id, string};
use papyrus::pex::op;

use super::autotest2::{
    Npc, Vanilla, as_form, faction, line, outdoors_or_skip, place, place_fn, put, quest, remove,
};
use super::script::{Code, measure, num, on_update, settle};
use super::{TestMod, own};

const QUEST: u32 = 0x800;
const FACTION_A: u32 = 0x801;
const FACTION_B: u32 = 0x802;
const TARGET: u32 = 0x803;
const FIGHTER: u32 = 0x804;
const ARCHER: u32 = 0x805;
const NEXT_OBJECT_ID: u32 = 0x806;

const SCRIPT: &str = "VrmAutoTest3Script";
/// On the target: passes its hits to the test script.
const HIT_SCRIPT: &str = "VrmAutoTest3HitScript";
const PLAYER_REF: FormId = FormId(0x14);
/// How long a weather is waited for (seconds).
const WEATHER_TIMEOUT: f32 = 60.0;

/// Vanilla forms the scripts use: property, type, editor id.
const VANILLA: &[(&str, &str, &str)] = &[
    ("IronSword", "Weapon", "IronSword"),
    ("HuntingBow", "Weapon", "HuntingBow"),
    ("IronArrow", "Ammo", "IronArrow"),
    ("TimeScale", "GlobalVariable", "TimeScale"),
    ("WeatherA", "Weather", "SkyrimClear"),
    ("WeatherB", "Weather", "SkyrimCloudy"),
];

/// Our own forms: property, type, object id.
const OWN: &[(&str, &str, u32)] = &[
    ("Target", "ActorBase", TARGET),
    ("Fighter", "ActorBase", FIGHTER),
    ("Archer", "ActorBase", ARCHER),
];

const TESTS: &[(&str, &str)] = &[
    ("Weather", "Weather set without an override"),
    ("Sword", "A sword's hits"),
    ("Arrows", "An archer's hits"),
];

/// `Name(Form f)`: the property naming `f`, "none", or `f` cast.
fn name_fn() -> FunctionBuilder {
    let mut c = Code::new("String");
    let f = c.param("f", "Form");
    let none = c.temp("Form");
    c.cast(none.clone(), Arg::None);
    let is_none = c.calc(op::CMP_EQ, "Bool", f.clone(), none);
    let next = c.label();
    c.jmpf(is_none, next).ret(string("none")).bind(next);
    let names = ["Player", "TargetRef", "FighterRef", "ArcherRef"]
        .into_iter()
        .chain(VANILLA.iter().map(|v| v.0));
    for name in names {
        let var = if name.ends_with("Ref") {
            id(name)
        } else {
            auto_var(name)
        };
        let p = as_form(&mut c, var);
        let eq = c.calc(op::CMP_EQ, "Bool", f.clone(), p);
        let next = c.label();
        c.jmpf(eq, next).ret(string(name)).bind(next);
    }
    let s = c.temp("String");
    c.cast(s.clone(), f);
    c.finish(s)
}

fn name(c: &mut Code, f: Arg) -> Arg {
    let f = as_form(c, f);
    c.own("Name", "String", [f])
}

/// `WeatherState(String label)`: the current and outgoing weather and the
/// transition.
fn weather_state() -> FunctionBuilder {
    let mut c = Code::new("String");
    let label = c.param("label", "String");
    let r = c.local("r", "String");
    c.assign(r.clone(), label);
    let current = c.global("Weather", "GetCurrentWeather", []);
    let current = name(&mut c, current);
    let outgoing = c.global("Weather", "GetOutgoingWeather", []);
    let outgoing = name(&mut c, outgoing);
    let pct = c.global("Weather", "GetCurrentWeatherTransition", []);
    put(&mut c, &r, ": current ", current);
    put(&mut c, &r, ", outgoing ", outgoing);
    c.numbers(&r, [(", transition ", pct)]);
    line(&mut c, &r, "\n");
    c.finish(r)
}

/// `Other()`: whichever of Clear and Cloudy isn't on.
fn other_fn() -> FunctionBuilder {
    let mut c = Code::new("Weather");
    let current = c.global("Weather", "GetCurrentWeather", []);
    let is_a = c.calc(op::CMP_EQ, "Bool", current, auto_var("WeatherA"));
    let b = c.label();
    c.jmpt(is_a, b).ret(auto_var("WeatherA")).bind(b);
    c.finish(auto_var("WeatherB"))
}

fn test_weather() -> FunctionBuilder {
    let mut c = Code::new("String");
    let r = c.local("r", "String");
    c.assign(r.clone(), string(""));
    let end = outdoors_or_skip(&mut c, &r);
    let state = |c: &mut Code, label: &str| {
        let s = c.own("WeatherState", "String", [string(label)]);
        c.append(r.clone(), [s]);
    };
    let measure = |c: &mut Code, w: Arg, overriding: bool, accelerate: bool, label: &str| {
        let settled = c.own("Settle", "String", []);
        c.append(r.clone(), [settled]);
        let line = c.own(
            "Measure",
            "String",
            [
                w,
                overriding.into(),
                accelerate.into(),
                string(label),
                WEATHER_TIMEOUT.into(),
            ],
        );
        c.append(r.clone(), [line]);
    };
    let other = |c: &mut Code, var: &str| {
        let w = c.own("Other", "Weather", []);
        let v = c.local(var, "Weather");
        c.assign(v.clone(), w);
        v
    };
    state(&mut c, "start (no override set yet)");

    // 1, 2: without an override, before any.
    let w = other(&mut c, "w1");
    measure(
        &mut c,
        w,
        false,
        false,
        "1. SetActive(false, false), no override yet",
    );
    state(&mut c, "  then");
    let w = other(&mut c, "w2");
    measure(&mut c, w, false, true, "2. SetActive(false, true)");
    state(&mut c, "  then");

    // 3: ForceActive without an override.
    c.own("Settle", "String", []);
    let w = other(&mut c, "w3");
    let w_name = name(&mut c, w.clone());
    put(&mut c, &r, "3. ForceActive(false) of ", w_name);
    line(&mut c, &r, "\n");
    c.method("Weather", w, "ForceActive", [false.into()]);
    state(&mut c, "  at once");
    c.wait(1.0);
    state(&mut c, "  1 s");
    c.wait(10.0);
    state(&mut c, "  11 s");

    // 4: a weather the region picks (FindWeather: 0 pleasant, 1 cloudy).
    let pleasant = c.global("Weather", "FindWeather", [0.into()]);
    let pleasant_v = c.local("pleasant", "Weather");
    c.assign(pleasant_v.clone(), pleasant);
    let cloudy = c.global("Weather", "FindWeather", [1.into()]);
    let cloudy_v = c.local("cloudy", "Weather");
    c.assign(cloudy_v.clone(), cloudy);
    let p_name = name(&mut c, pleasant_v.clone());
    let c_name = name(&mut c, cloudy_v.clone());
    put(&mut c, &r, "4. FindWeather: pleasant ", p_name);
    put(&mut c, &r, ", cloudy ", c_name);
    line(&mut c, &r, "\n");
    let w4 = c.local("w4", "Weather");
    let current = c.global("Weather", "GetCurrentWeather", []);
    let is_pleasant = c.calc(op::CMP_EQ, "Bool", current, pleasant_v.clone());
    let keep = c.label();
    c.assign(w4.clone(), pleasant_v)
        .jmpf(is_pleasant, keep)
        .assign(w4.clone(), cloudy_v)
        .bind(keep);
    measure(
        &mut c,
        w4,
        false,
        false,
        "4. the region's, SetActive(false, false)",
    );
    state(&mut c, "  then");

    // 5: the weather spell's way.
    c.own("Settle", "String", []);
    let back = c.local("back", "Weather");
    let current = c.global("Weather", "GetCurrentWeather", []);
    c.assign(back.clone(), current);
    let w = other(&mut c, "w5");
    c.method(
        "Weather",
        back.clone(),
        "SetActive",
        [false.into(), true.into()],
    );
    let back_name = name(&mut c, back.clone());
    put(&mut c, &r, "5. the weather on, ", back_name);
    line(
        &mut c,
        &r,
        ", SetActive(false, true); then another with an override:\n",
    );
    measure(&mut c, w.clone(), true, true, "5. SetActive(true, true)");
    c.global("Weather", "ReleaseOverride", []);
    state(&mut c, "  ReleaseOverride, at once");
    // How long until the overriding weather goes out.
    let start = c.global("Utility", "GetCurrentRealTime", []);
    let gone = c.local("gone", "Float");
    c.assign(gone.clone(), (-1.0f32).into());
    let (top, done) = (c.label(), c.label());
    c.bind(top);
    c.wait(0.25);
    let now = c.global("Utility", "GetCurrentRealTime", []);
    let dt = c.calc(op::FSUB, "Float", now, start);
    let over = c.calc(op::CMP_GT, "Bool", dt.clone(), WEATHER_TIMEOUT.into());
    c.jmpt(over, done);
    let current = c.global("Weather", "GetCurrentWeather", []);
    let still = c.calc(op::CMP_EQ, "Bool", current, w);
    c.jmpt(still, top);
    c.assign(gone, dt);
    c.bind(done);
    c.numbers(
        &r,
        [(
            "  another weather coming in after (s; -1 not within the wait) ",
            id("gone"),
        )],
    );
    line(&mut c, &r, "\n");
    state(&mut c, "  then");
    let settled = c.own("Settle", "String", []);
    c.append(r.clone(), [settled]);
    state(&mut c, "  settled");

    // 6: as autotest2 did, after the release.
    let w = other(&mut c, "w6");
    measure(
        &mut c,
        w,
        false,
        false,
        "6. after the release, SetActive(false, false)",
    );
    state(&mut c, "  then");
    c.bind(end);
    c.finish(r)
}

/// `LogHit(...)`, called by the target's script on each `OnHit`.
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
    c.op(op::IADD, [id("HitCount"), id("HitCount"), 1.into()]);
    let now = c.global("Utility", "GetCurrentRealTime", []);
    let t = c.calc(op::FSUB, "Float", now, id("HitStart"));
    let t = c.own("Num", "String", [t]);
    let who = name(&mut c, aggressor.clone());
    let source = name(&mut c, source);
    let projectile = name(&mut c, projectile);
    let distance = c.method(
        "ObjectReference",
        id("TargetRef"),
        "GetDistance",
        [aggressor],
    );
    let distance = c.own("Num", "String", [distance]);
    c.append(
        log.clone(),
        [
            t,
            string(" s: by "),
            who,
            string(" at "),
            distance,
            string(", source "),
            source,
            string(", projectile "),
            projectile,
        ],
    );
    for (flag, label) in flags.into_iter().zip(["power", "sneak", "bash", "blocked"]) {
        c.append(log.clone(), [string(&format!(", {label} ")), flag]);
    }
    c.append(log, [string("\n")]);
    c.finish(Arg::None)
}

/// `FightState(Actor a)`: a fighter's state, target, weapon, distance.
fn fight_state() -> FunctionBuilder {
    let mut c = Code::new("String");
    let a = c.param("a", "Actor");
    let r = c.local("r", "String");
    let now = c.global("Utility", "GetCurrentRealTime", []);
    let t = c.calc(op::FSUB, "Float", now, id("HitStart"));
    c.assign(r.clone(), string(""));
    c.numbers(&r, [("", t)]);
    let state = c.method("Actor", a.clone(), "GetCombatState", []);
    put(&mut c, &r, " s: combat state ", state);
    let target = c.method("Actor", a.clone(), "GetCombatTarget", []);
    let target = name(&mut c, target);
    put(&mut c, &r, ", target ", target);
    let drawn = c.method("Actor", a.clone(), "IsWeaponDrawn", []);
    put(&mut c, &r, ", drawn ", drawn);
    let weapon = c.method("Actor", a.clone(), "GetEquippedWeapon", [false.into()]);
    let weapon = name(&mut c, weapon);
    put(&mut c, &r, ", weapon ", weapon);
    let distance = c.method("ObjectReference", id("TargetRef"), "GetDistance", [a]);
    c.numbers(&r, [(", distance ", distance)]);
    put(&mut c, &r, ", hits so far ", id("HitCount"));
    line(&mut c, &r, "\n");
    c.finish(r)
}

/// The target, held in place: placed once, kept for both tests.
fn ensure_target(c: &mut Code) {
    let none = c.temp("Actor");
    c.cast(none.clone(), Arg::None);
    let missing = c.calc(op::CMP_EQ, "Bool", id("TargetRef"), none);
    let have = c.label();
    c.jmpf(missing, have);
    let t = place(c, "Target", 400.0, 0.0);
    c.assign(id("TargetRef"), t.clone());
    c.method(
        "Actor",
        t.clone(),
        "SetActorValue",
        [string("Health"), 5000.0f32.into()],
    );
    c.method("Actor", t, "SetRestrained", [true.into()]);
    c.bind(have);
}

/// A fighting test: `base` placed at `x`, given `items` (equipped), set on
/// the target for 20 s, its state logged every 2 s. The target goes after
/// the `last` test.
fn fight(base: &str, var: &str, x: f32, items: &[(&str, i32)], last: bool) -> FunctionBuilder {
    let mut c = Code::new("String");
    let r = c.local("r", "String");
    c.assign(r.clone(), string(""));
    ensure_target(&mut c);
    let start = c.global("Utility", "GetCurrentRealTime", []);
    c.assign(id("HitStart"), start);
    c.assign(id("HitLog"), string(""))
        .assign(id("HitCount"), 0.into());
    let a = place(&mut c, base, x, 0.0);
    c.assign(id(var), a.clone());
    c.method(
        "Actor",
        a.clone(),
        "SetActorValue",
        [string("Health"), 5000.0f32.into()],
    );
    for (item, n) in items {
        c.method(
            "ObjectReference",
            a.clone(),
            "AddItem",
            [auto_var(item), (*n).into(), true.into()],
        );
        c.method(
            "Actor",
            a.clone(),
            "EquipItem",
            [auto_var(item), false.into(), true.into()],
        );
    }
    let arrows_before = c.method(
        "ObjectReference",
        a.clone(),
        "GetItemCount",
        [auto_var("IronArrow")],
    );
    let arrows_v = c.local("arrowsBefore", "Int");
    c.assign(arrows_v.clone(), arrows_before);
    c.wait(1.0);
    let log_state = |c: &mut Code| {
        let s = c.own("FightState", "String", [a.clone()]);
        c.append(r.clone(), [s]);
    };
    log_state(&mut c);
    c.method("Actor", a.clone(), "StartCombat", [id("TargetRef")]);
    for _ in 0..10 {
        c.wait(2.0);
        log_state(&mut c);
    }
    c.method("Actor", a.clone(), "StopCombat", []);
    let arrows_after = c.method(
        "ObjectReference",
        a.clone(),
        "GetItemCount",
        [auto_var("IronArrow")],
    );
    put(&mut c, &r, "arrows carried before ", arrows_v);
    put(&mut c, &r, ", after ", arrows_after);
    line(&mut c, &r, "\n");
    put(&mut c, &r, "OnHit on the target:\n", id("HitLog"));
    remove(&mut c, &a);
    if last {
        c.method("Actor", id("TargetRef"), "StopCombat", []);
        remove(&mut c, &id("TargetRef"));
    }
    c.finish(r)
}

fn scripts() -> Result<Vec<(String, Vec<u8>)>> {
    let mut s = ScriptBuilder::new(SCRIPT, "Quest");
    s.auto_property("Player", "Actor");
    for (name, ty, _) in VANILLA {
        s.auto_property(name, ty);
    }
    for (name, ty, _) in OWN {
        s.auto_property(name, ty);
    }
    s.variable("Ran", "Bool", false.into());
    for (name, ty) in [
        ("HitLog", "String"),
        ("HitStart", "Float"),
        ("HitCount", "Int"),
        ("TargetRef", "Actor"),
        ("FighterRef", "Actor"),
        ("ArcherRef", "Actor"),
    ] {
        let init = match ty {
            "String" => string(""),
            "Float" => 0.0f32.into(),
            "Int" => 0.into(),
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
        .function("OnUpdate", on_update("autotest3", "eight minutes", TESTS))
        .function("Num", num())
        .function("Name", name_fn())
        .function("Place", place_fn())
        .function("Other", other_fn())
        .function("WeatherState", weather_state())
        .function("FightState", fight_state())
        .function("LogHit", log_hit())
        .function("Settle", settle())
        .function("Measure", measure())
        .function("TestWeather", test_weather())
        .function(
            "TestSword",
            fight("Fighter", "FighterRef", 300.0, &[("IronSword", 1)], false),
        )
        .function(
            "TestArrows",
            fight(
                "Archer",
                "ArcherRef",
                -400.0,
                &[("HuntingBow", 1), ("IronArrow", 40)],
                true,
            ),
        );

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
        (HIT_SCRIPT.into(), papyrus::pex::write(&h.build()?)?),
    ])
}

pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    let v = Vanilla { lo };
    let template = v.id("WEMerchantMiscMaleCommoner")?;
    let race = v.id("ImperialRace")?;
    let class = v.id("Citizen")?;
    let object = PropertyValue::Object;
    let mut properties = vec![("Player".to_owned(), object(PLAYER_REF))];
    for (name, _, edid) in VANILLA {
        properties.push((name.to_string(), object(v.id(edid)?)));
    }
    for (name, _, object_id) in OWN {
        properties.push((name.to_string(), object(own(*object_id))));
    }
    let quests = vec![quest(
        QUEST,
        "VrmAutoTest3Quest",
        "vibe_rm Autotest 3",
        0x0011,
        0,
        Some(AttachedScript {
            name: SCRIPT.into(),
            properties,
        }),
        &[],
    )];
    let factions = vec![
        faction(FACTION_A, "VrmAutoTest3FactionA", 0, &[FACTION_B]),
        faction(FACTION_B, "VrmAutoTest3FactionB", 0, &[FACTION_A]),
    ];
    let npc = |object_id: u32, edid: &str, name: &'static str, temper, faction, script| Npc {
        object_id,
        edid: edid.to_owned(),
        name: Some(name),
        level: (10, false, 1, 0),
        template: (template, 0x1),
        temper,
        factions: vec![(own(faction), 0)],
        script,
    };
    let npcs = vec![
        npc(
            TARGET,
            "VrmAutoTest3Target",
            "vibe_rm Target",
            (0, 2),
            FACTION_B,
            Some(AttachedScript {
                name: HIT_SCRIPT.into(),
                properties: vec![("Main".into(), object(own(QUEST)))],
            }),
        ),
        npc(
            FIGHTER,
            "VrmAutoTest3Fighter",
            "vibe_rm Fighter",
            (1, 3),
            FACTION_A,
            None,
        ),
        npc(
            ARCHER,
            "VrmAutoTest3Archer",
            "vibe_rm Archer",
            (1, 3),
            FACTION_A,
            None,
        ),
    ]
    .into_iter()
    .map(|n| n.record(race, class))
    .collect();
    let plugin = write_plugin(
        0,
        &["Skyrim.esm"],
        "vibe_rm",
        "autotest2's follow-up: weather without an override, sword and arrow hits.",
        NEXT_OBJECT_ID,
        &[
            Node::top(b"FACT", factions),
            Node::top(b"NPC_", npcs),
            Node::top(b"QUST", quests),
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
        let mut properties = vec![("Player".to_owned(), form(0x14, "Actor"))];
        for (i, (name, ty, _)) in VANILLA.iter().enumerate() {
            properties.push((name.to_string(), form(0x1000 + i as u32, ty)));
        }
        for (name, ty, object_id) in OWN {
            properties.push((name.to_string(), form(own(*object_id).0, ty)));
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
            "start (no override set yet): current ",
            "1. SetActive(false, false), no override yet: current at once True",
            "3. ForceActive(false) of WeatherA\n",
            "4. FindWeather: pleasant none, cloudy none\n",
            "5. SetActive(true, true): current at once True",
            "another weather coming in after (s; -1 not within the wait) -1\n",
            "6. after the release, SetActive(false, false):",
            "s: combat state 1, target none, drawn False, weapon none, distance 1",
            "arrows carried before 1, after 1\n",
        ] {
            assert!(all.contains(expected), "{expected:?} not in the reports");
        }
    }
}
