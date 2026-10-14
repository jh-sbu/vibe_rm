//! `test-mod taper`: magic effects' taper (`MGEF` `DATA` 0x1C weight, 0x34
//! curve, 0x38 duration), which `autotest4` found adding to fire damage.
//! Each test casts a spell of ours carrying one vanilla effect (copied and
//! made fire-and-forget aimed, its taper kept) from a caster at a target
//! held in place, and logs the target's health (and the second actor value
//! of a dual effect, and the caster's health for an absorb) every 0.25 s
//! from the impact on. As the other autotests, the reports are saved and
//! read with `vrm-tool test-results`.
//!
//! Load the usual save outdoors and leave the game alone for about three
//! minutes. The reading under test (`src/magic.rs`, `update_tapers`): an
//! instant change goes on at weight x magnitude a second, falling as
//! `(1 - t / duration) ^ curve`, `magnitude x weight x duration / (curve +
//! 1)` in all. What each test predicts by it, for 50 points:
//!
//! | effect | taper | extra |
//! | --- | --- | --- |
//! | fire (Firebolt's) | 0.3, 2, 1 s | 5 |
//! | fire (a trap's) | 0.2, 2, 1 s | 3.3 |
//! | fire area (Fireball's) | 0.3, 2, 2 s | 10 |
//! | fire area (Fire Storm's) | 0.3, 2, 4 s | 20 |
//! | Storm Call's bolt (health, magicka) | 1, 1, 1 s | 25 |
//! | frost (Ice Spike's: health, stamina) | none | 0 |
//! | shock (Lightning Bolt's: health, magicka half) | 0.01, 0, 0.1 s | 0.05 |
//! | poison (a trap dart's) | 0.2, 2, 1 s | 3.3 |
//! | poison (the spriggans' swarm) | 0.5, 2, 3 s | 25 |
//! | restore health (Healing Hands') | 1, 0, 0.3 s | 15 healed |
//! | fire, 10 a second for 3 s | 0.3, 2, 1 s | 1 after? |
//! | absorb health (a draugr axe's) | 0.2, 2, 1 s | 3.3, given? |
//! | frost rune (health, stamina) | 0.3, 2, 3 s | 15 |
//!
//! Run 2026-10-13: every landing effect matched the reading summed per frame
//! at 60 fps (the frame's end rate): fire 4.875, fire area 9.875, Fire
//! Storm 19.875, trap poison 3.25, Storm Call 24.583, heal 15 (65 healed),
//! shock 0.05, frost 0. Fire 10 a second for 3 s took 30.886: 30 applied a
//! fraction a frame, then a 1 s taper after. Frost's stamina and shock's
//! magicka read 0 lost (regeneration hides them). The absorb, trap fire,
//! frost rune and swarm copies never landed (no change in 6 s), so an
//! absorb's gain is still untested.

use anyhow::Result;
use esp::writer::{AttachedScript, Node, PropertyValue, RecordBuilder, write_plugin};
use esp::{FormId, LoadOrder};
use papyrus::build::{Arg, FunctionBuilder, ScriptBuilder, auto_var, id, string};
use papyrus::pex::op;

use super::autotest2::{Npc, Vanilla, line, place_fn, put, remove};
use super::autotest4::{fresh, now, put_num, report, restrain, set_av};
use super::records::{self, Effect, spell};
use super::script::{Code, num, on_update};
use super::{TestMod, own};

const QUEST: u32 = 0x800;
const DUMMY: u32 = 0x801;
/// Our copies of the effects, then the spells carrying them.
const EFFECTS: u32 = 0x810;
const SPELLS: u32 = 0x830;
const NEXT_OBJECT_ID: u32 = 0x850;

const SCRIPT: &str = "VrmTaperTestScript";
const PLAYER_REF: FormId = FormId(0x14);
/// How long each test logs after the impact (seconds), every 0.25 s.
const SPAN: f32 = 5.0;

/// The tests: name (the report's `Result<name>`), the vanilla effect, the
/// magnitude, the duration, the second actor value logged (Health for
/// none), what the target is first (0 nothing, 1 damaged by 1000 for a
/// heal), whether the caster is logged (an absorb), label.
struct Case {
    name: &'static str,
    effect: &'static str,
    magnitude: f32,
    duration: u32,
    second: &'static str,
    damaged: bool,
    caster: bool,
    label: &'static str,
}

const fn case(
    name: &'static str,
    effect: &'static str,
    magnitude: f32,
    second: &'static str,
    label: &'static str,
) -> Case {
    Case {
        name,
        effect,
        magnitude,
        duration: 0,
        second,
        damaged: false,
        caster: false,
        label,
    }
}

const CASES: &[Case] = &[
    case(
        "Fire",
        "FireDamageFFAimed",
        50.0,
        "Health",
        "fire (0.3, 2, 1 s)",
    ),
    case(
        "FireLight",
        "TrapExplGasEnchFireDamageFFContact06",
        50.0,
        "Health",
        "fire (0.2, 2, 1 s)",
    ),
    case(
        "FireArea",
        "FireDamageFFAimedArea",
        50.0,
        "Health",
        "fire area (0.3, 2, 2 s)",
    ),
    case(
        "FireStorm",
        "FireDamageFFSelfArea100",
        50.0,
        "Health",
        "fire area (0.3, 2, 4 s)",
    ),
    case(
        "StormBolt",
        "ShockDamageBoltStormAimed",
        50.0,
        "Magicka",
        "Storm Call's bolt (1, 1, 1 s; health and magicka)",
    ),
    case(
        "Frost",
        "FrostDamageFFAimed",
        50.0,
        "Stamina",
        "frost (no taper; health and stamina)",
    ),
    case(
        "Shock",
        "ShockDamageFFAimed",
        50.0,
        "Magicka",
        "shock (0.01, 0, 0.1 s; health and half to magicka)",
    ),
    case(
        "Poison",
        "TrapPoisonFFContactEffect",
        50.0,
        "Health",
        "poison (0.2, 2, 1 s)",
    ),
    case(
        "Swarm",
        "SprigganBugDamageConcAimed",
        50.0,
        "Health",
        "poison (0.5, 2, 3 s)",
    ),
    Case {
        damaged: true,
        ..case(
            "Heal",
            "RestoreHealthConcActor",
            50.0,
            "Health",
            "restore health (1, 0, 0.3 s), the target 1000 down",
        )
    },
    Case {
        duration: 3,
        ..case(
            "FireOverTime",
            "FireDamageFFAimed",
            10.0,
            "Health",
            "fire 10 a second for 3 s (0.3, 2, 1 s)",
        )
    },
    Case {
        caster: true,
        ..case(
            "Absorb",
            "DraugrMagicAxeStreak",
            50.0,
            "Health",
            "absorb health (0.2, 2, 1 s), the caster 1000 down",
        )
    },
    case(
        "FrostRune",
        "TrapRuneFrostFFLocation06",
        50.0,
        "Stamina",
        "frost rune (0.3, 2, 3 s; health and stamina)",
    ),
];

/// `Timeline(Spell sp, Actor caster, Actor t, String av, String second, Bool
/// logCaster)`: casts `sp` and logs what it does to `t`'s `av` (and
/// `second`, and the caster's `av` with `logCaster`).
pub(super) fn timeline() -> FunctionBuilder {
    let mut c = Code::new("String");
    let sp = c.param("sp", "Spell");
    let caster = c.param("caster", "Actor");
    let t = c.param("t", "Actor");
    let av = c.param("av", "String");
    let second = c.param("second", "String");
    let log_caster = c.param("logCaster", "Bool");
    let r = c.local("r", "String");
    c.assign(r.clone(), string(""));
    let keep = |c: &mut Code, var: &str, v: Arg| {
        let l = c.local(var, "Float");
        c.assign(l.clone(), v);
        l
    };
    let value = |c: &mut Code, a: &Arg, av: &Arg| {
        c.method("Actor", a.clone(), "GetActorValue", [av.clone()])
    };
    let h = value(&mut c, &t, &av);
    let h0 = keep(&mut c, "h0", h);
    let s = value(&mut c, &t, &second);
    let s0 = keep(&mut c, "s0", s);
    let hc = value(&mut c, &caster, &av);
    let c0 = keep(&mut c, "c0", hc);
    let t0 = now(&mut c);
    let start = keep(&mut c, "start", t0);
    c.method("Spell", sp, "Cast", [caster.clone(), t.clone()]);
    // The impact: the first change of the target's value.
    let (top, hit, missed) = (c.label(), c.label(), c.label());
    c.bind(top);
    c.wait(0.05);
    let tn = now(&mut c);
    let dt = c.calc(op::FSUB, "Float", tn, start.clone());
    let over = c.calc(op::CMP_GT, "Bool", dt, 6.0f32.into());
    c.jmpt(over, missed);
    let hn = value(&mut c, &t, &av);
    let same = c.calc(op::CMP_EQ, "Bool", hn, h0.clone());
    c.jmpt(same, top);
    c.jmp(hit);
    c.bind(missed);
    line(&mut c, &r, "no change in 6 s");
    c.ret(r.clone());
    c.bind(hit);
    let ti = now(&mut c);
    let at = c.calc(op::FSUB, "Float", ti, start);
    put_num(&mut c, &r, "impact after ", at);
    put(&mut c, &r, " s; ", av.clone());
    line(&mut c, &r, " lost from then on, every 0.25 s:");
    let samples = (SPAN / 0.25) as i32;
    let i = c.local("i", "Int");
    c.assign(i.clone(), 0.into());
    let (top, done) = (c.label(), c.label());
    c.bind(top);
    let lost = value(&mut c, &t, &av);
    let lost = c.calc(op::FSUB, "Float", h0.clone(), lost);
    put_num(&mut c, &r, " ", lost);
    let enough = c.calc(op::CMP_GE, "Bool", i.clone(), samples.into());
    c.jmpt(enough, done);
    c.op(op::IADD, [i.clone(), i.clone(), 1.into()]);
    c.wait(0.25);
    c.jmp(top);
    c.bind(done);
    let s1 = value(&mut c, &t, &second);
    let ds = c.calc(op::FSUB, "Float", s0, s1);
    put(&mut c, &r, "; ", second);
    put_num(&mut c, &r, " lost ", ds);
    let skip = c.label();
    c.jmpf(log_caster, skip);
    let hc = value(&mut c, &caster, &av);
    let gained = c.calc(op::FSUB, "Float", hc, c0);
    put_num(&mut c, &r, "; the caster gained ", gained);
    c.bind(skip);
    line(&mut c, &r, "\n");
    c.finish(r)
}

/// One case's test: a fresh target and caster, the target made ready, the
/// timeline.
fn test(case: &Case, index: usize) -> FunctionBuilder {
    let (mut c, r) = report();
    let t = fresh(&mut c, "t", "Dummy", 450.0, 0.0);
    let caster = fresh(&mut c, "caster", "Dummy", 150.0, 0.0);
    for a in [&t, &caster] {
        set_av(&mut c, a, "Health", 5000.0);
        for av in ["Magicka", "Stamina"] {
            set_av(&mut c, a, av, 1000.0);
        }
    }
    restrain(&mut c, &t);
    restrain(&mut c, &caster);
    c.wait(1.5);
    if case.damaged {
        c.method(
            "Actor",
            t.clone(),
            "DamageActorValue",
            [string("Health"), 1000.0f32.into()],
        );
    }
    if case.caster {
        c.method(
            "Actor",
            caster.clone(),
            "DamageActorValue",
            [string("Health"), 1000.0f32.into()],
        );
    }
    c.wait(0.5);
    put(
        &mut c,
        &r,
        &format!("{} ({}, {} points", case.label, case.effect, case.magnitude),
        string(""),
    );
    if case.duration > 0 {
        put(
            &mut c,
            &r,
            &format!(" a second for {} s", case.duration),
            string(""),
        );
    }
    line(&mut c, &r, "): ");
    let tl = c.temp("String");
    c.call_method(
        id("self"),
        "Timeline",
        tl.clone(),
        [
            auto_var(&format!("Spell{index}")),
            caster.clone(),
            t.clone(),
            string("Health"),
            string(case.second),
            case.caster.into(),
        ],
    );
    c.append(r.clone(), [tl]);
    for a in [&t, &caster] {
        c.method("Actor", a.clone(), "StopCombat", []);
        remove(&mut c, a);
    }
    c.wait(1.0);
    c.finish(r)
}

fn scripts() -> Result<Vec<(String, Vec<u8>)>> {
    let mut s = ScriptBuilder::new(SCRIPT, "Quest");
    s.auto_property("Player", "Actor");
    s.auto_property("Dummy", "ActorBase");
    for i in 0..CASES.len() {
        s.auto_property(&format!("Spell{i}"), "Spell");
    }
    s.variable("Ran", "Bool", false.into());
    let tests: Vec<(&str, &str)> = CASES.iter().map(|c| (c.name, c.label)).collect();
    for (name, _) in &tests {
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
        .function("OnUpdate", on_update("taper", "three minutes", &tests))
        .function("Num", num())
        .function("Place", place_fn())
        .function("Timeline", timeline());
    for (i, case) in CASES.iter().enumerate() {
        s.function(&format!("Test{}", case.name), test(case, i));
    }
    Ok(vec![(SCRIPT.into(), papyrus::pex::write(&s.build()?)?)])
}

pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    let v = Vanilla { lo };
    let template = v.id("WEMerchantMiscMaleCommoner")?;
    let race = v.id("ImperialRace")?;
    let class = v.id("Citizen")?;
    let either_hand = v.id("EitherHand")?;
    let projectile = v.id("FireboltProjectile01")?;

    // The effects, made fire-and-forget aimed (a projectile if they had
    // none), their tapers and everything else kept.
    let mut effects = Vec::new();
    let mut spells = Vec::new();
    for (i, case) in CASES.iter().enumerate() {
        let effect_id = EFFECTS + i as u32;
        effects.push(records::copy(
            lo,
            case.effect,
            effect_id,
            &format!("VrmTaper{}Effect", case.name),
            None,
            move |tag, d| {
                if tag == b"DATA" && d.len() >= 0x58 {
                    if d[0x48..0x4C] == [0; 4] {
                        d[0x48..0x4C].copy_from_slice(&projectile.0.to_le_bytes());
                    }
                    d[0x50..0x54].copy_from_slice(&spell::FIRE_AND_FORGET.to_le_bytes());
                    d[0x54..0x58].copy_from_slice(&spell::AIMED.to_le_bytes());
                }
            },
        )?);
        spells.push(records::spell(
            SPELLS + i as u32,
            &format!("VrmTaper{}", case.name),
            spell::SPELL,
            spell::FIRE_AND_FORGET,
            spell::AIMED,
            0,
            Some(either_hand),
            &[Effect::new(own(effect_id), case.magnitude, case.duration)],
        ));
    }

    let object = PropertyValue::Object;
    let mut properties = vec![
        ("Player".to_owned(), object(PLAYER_REF)),
        ("Dummy".to_owned(), object(own(DUMMY))),
    ];
    for i in 0..CASES.len() {
        properties.push((format!("Spell{i}"), object(own(SPELLS + i as u32))));
    }
    let quests = vec![
        records::Quest {
            object_id: QUEST,
            edid: "VrmTaperTestQuest",
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
    let npcs: Vec<RecordBuilder> = vec![
        Npc {
            object_id: DUMMY,
            edid: "VrmTaperTestDummy".into(),
            name: Some("vibe_rm Test NPC"),
            level: (10, false, 1, 0),
            template: (template, 0x1),
            temper: (0, 0),
            factions: Vec::new(),
            script: None,
            assistance: 0,
            combat_style: None,
            unique: false,
        }
        .record(race, class),
    ];
    let plugin = write_plugin(
        0,
        &["Skyrim.esm"],
        "vibe_rm",
        "Magic effects' taper: spells of ours timed on a held NPC, reports saved.",
        NEXT_OBJECT_ID,
        &[
            Node::top(b"MGEF", effects),
            Node::top(b"SPEL", spells),
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
        let mut properties = vec![
            ("Player".to_owned(), form(0x14, "Actor")),
            ("Dummy".to_owned(), form(own(DUMMY).0, "ActorBase")),
        ];
        for i in 0..CASES.len() {
            properties.push((format!("Spell{i}"), form(own(SPELLS + i as u32).0, "Spell")));
        }
        let shown = run(
            scripts().unwrap(),
            &[(own(QUEST).0, SCRIPT, properties)],
            CASES.len(),
        );
        assert_eq!(shown.boxes.len(), CASES.len(), "every test reports");
        assert_eq!(shown.saved_at_box, Some(0), "saves before the boxes");
        let all = shown.boxes.join("\n");
        for expected in [
            "fire (0.3, 2, 1 s) (FireDamageFFAimed, 50 points): no change in 6 s",
            "fire 10 a second for 3 s (0.3, 2, 1 s) (FireDamageFFAimed, 10 points a second for 3 s): ",
        ] {
            assert!(all.contains(expected), "{expected:?} not in the reports");
        }
    }
}
