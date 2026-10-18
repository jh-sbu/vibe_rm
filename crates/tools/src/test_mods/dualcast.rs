//! `test-mod dualcast`: how dual casting scales a spell's effects. The game
//! has `fMagicDualCastingEffectivenessBase` (2.2) and each effect a dual
//! casting scale (`MGEF` `DATA` 0x70: 1 for most, Fireball's 1.1, Ice
//! Storm's 1.2, Fire Storm's 1.4); how the two combine isn't published. Each
//! case is a spell of ours carrying a copy of the Fortify Carry Weight
//! potion's effect, made an Alteration fire-and-forget self effect (a plain
//! value modifier, no keywords), with its own scale and power flags
//! (0x200000 power affects magnitude, 0x400000 duration): 50 points for 4 s.
//!
//! Papyrus can't dual cast, so the player does. Load the usual save
//! somewhere quiet; a box explains. For each case the script readies the
//! spell in both hands (Alteration's dual casting perk given for the run)
//! and says so; cast it once with one hand, wait for "over", then dual cast
//! it (both buttons together) and wait for "over" again. The script logs
//! each cast's rise in CarryWeight and how long the effect stays on
//! (`HasMagicEffect`); a cast not made within 120 s is logged as none.
//! About five minutes; the reports are saved and read with `vrm-tool
//! test-results`.
//!
//! What each reading predicts for the dual cast (the single cast: 50 for
//! 4 s). A: power = 2.2 x scale (`src/magic.rs` now); B: 1 + 1.2 x scale;
//! C: power is 2.2 whatever the scale (the scale is for the dual casting
//! art's projectile, explosion and hit art, whose `DUAL` record has
//! "inherit scale" flags).
//!
//! | case | scale | flags | A | B | C |
//! | --- | --- | --- | --- | --- | --- |
//! | Scale1 | 1 | magnitude | 110 | 110 | 110 |
//! | Scale15 | 1.5 | magnitude | 165 | 140 | 110 |
//! | Scale3 | 3 | magnitude | 330 | 230 | 110 |
//! | Scale0 | 0 | magnitude | 0 | 50 | 110 |
//! | Duration15 | 1.5 | duration | 50 for 13.2 s | 50 for 11.2 s | 50 for 8.8 s |
//! | NoFlags | 1 | none | 50 for 4 s | 50 for 4 s | 50 for 4 s |
//! | NoDualMods | 1 | magnitude, `SPIT` 0x800000 | 50 | 50 | 50 |
//!
//! Scale1 also checks the 2.2; NoFlags and NoDualMods check that the power
//! flags and `kNoDualCastMods` gate it. A cast that rises by other than 50
//! singly means the character's perks scale it (Stability lengthens
//! Alteration spells): the dual cast's ratio to it is what counts.
//!
//! Run 2026-10-18 (a fresh level 1 character): reading C. Every single cast
//! rose 50 for 3.85 s (the polling's start after the effect is on). Dual:
//! Scale1, Scale15, Scale3 and Scale0 all +110 for 3.85 s; Duration15 +50
//! for 8.33 s (x2.16); NoFlags +110, so effects without the duration flag
//! gain magnitude, the magnitude flag or not; NoDualMods +100 for 3.86 s,
//! twice the single cast without the bonus.

use anyhow::Result;
use esp::writer::{AttachedScript, Node, PropertyValue, write_plugin};
use esp::{FormId, LoadOrder};
use papyrus::build::{Arg, FunctionBuilder, ScriptBuilder, auto_var, id, string};
use papyrus::pex::op;

use super::autotest2::{Vanilla, line};
use super::autotest4::{poll, put_num, report};
use super::records::{self, Effect, spell};
use super::script::{Code, num, on_update_with};
use super::{TestMod, own};

const QUEST: u32 = 0x800;
/// Our copies of the effect, then the spells carrying them.
const EFFECTS: u32 = 0x810;
const SPELLS: u32 = 0x830;
const NEXT_OBJECT_ID: u32 = 0x850;

const SCRIPT: &str = "VrmDualCastTestScript";
const PLAYER_REF: FormId = FormId(0x14);
/// The effect's points and seconds.
const MAGNITUDE: f32 = 50.0;
const DURATION: u32 = 4;
/// How long the script waits for a cast (seconds).
const CAST_WAIT: f32 = 120.0;

/// `MGEF` `DATA` flags: recover, power affects magnitude / duration.
const RECOVER: u32 = 0x2;
const POWER_MAGNITUDE: u32 = 0x20_0000;
const POWER_DURATION: u32 = 0x40_0000;
/// `SPIT` flag: dual casting leaves the spell's effects alone.
const NO_DUAL_CAST_MODS: u32 = 0x80_0000;
/// Alteration's skill (actor value 18).
const ALTERATION: u32 = 18;

/// A case: name (the report's `Result<name>`), the effect's dual casting
/// scale and power flags, the spell's `SPIT` flags, label.
struct Case {
    name: &'static str,
    scale: f32,
    power: u32,
    spell_flags: u32,
    label: &'static str,
}

const CASES: &[Case] = &[
    Case {
        name: "Scale1",
        scale: 1.0,
        power: POWER_MAGNITUDE,
        spell_flags: 0,
        label: "scale 1, power affects magnitude",
    },
    Case {
        name: "Scale15",
        scale: 1.5,
        power: POWER_MAGNITUDE,
        spell_flags: 0,
        label: "scale 1.5, power affects magnitude",
    },
    Case {
        name: "Scale3",
        scale: 3.0,
        power: POWER_MAGNITUDE,
        spell_flags: 0,
        label: "scale 3, power affects magnitude",
    },
    Case {
        name: "Scale0",
        scale: 0.0,
        power: POWER_MAGNITUDE,
        spell_flags: 0,
        label: "scale 0, power affects magnitude",
    },
    Case {
        name: "Duration15",
        scale: 1.5,
        power: POWER_DURATION,
        spell_flags: 0,
        label: "scale 1.5, power affects duration",
    },
    Case {
        name: "NoFlags",
        scale: 1.0,
        power: 0,
        spell_flags: 0,
        label: "scale 1, power affects neither",
    },
    Case {
        name: "NoDualMods",
        scale: 1.0,
        power: POWER_MAGNITUDE,
        spell_flags: NO_DUAL_CAST_MODS,
        label: "scale 1, power affects magnitude, spell flagged no dual cast mods",
    },
];

/// `Measure(MagicEffect fx)`: one cast by the player: waits for `fx` to
/// come on, logs CarryWeight's rise (its most while on) and how long it
/// stays on. "" if no cast came.
fn measure() -> FunctionBuilder {
    let mut c = Code::new("String");
    let fx = c.param("fx", "MagicEffect");
    let r = c.local("r", "String");
    c.assign(r.clone(), string(""));
    let player = auto_var("Player");
    let carry = |c: &mut Code| {
        c.method(
            "Actor",
            player.clone(),
            "GetActorValue",
            [string("CarryWeight")],
        )
    };
    let v = carry(&mut c);
    let base = c.local("base", "Float");
    c.assign(base.clone(), v);
    let started = poll(&mut c, "started", 0.05, CAST_WAIT, |c, hit| {
        let on = c.method("Actor", player.clone(), "HasMagicEffect", [fx.clone()]);
        c.jmpt(on, hit);
    });
    let came = c.label();
    let missed = c.calc(op::CMP_LT, "Bool", started, 0.0f32.into());
    c.jmpf(missed, came);
    c.ret(r.clone());
    c.bind(came);
    let peak = c.local("peak", "Float");
    let v = carry(&mut c);
    c.assign(peak.clone(), v);
    let lasted = poll(&mut c, "lasted", 0.05, 60.0, |c, hit| {
        let v = carry(c);
        let higher = c.calc(op::CMP_GT, "Bool", v.clone(), peak.clone());
        let keep = c.label();
        c.jmpf(higher, keep);
        c.assign(peak.clone(), v);
        c.bind(keep);
        let on = c.method("Actor", player.clone(), "HasMagicEffect", [fx.clone()]);
        c.jmpf(on, hit);
    });
    let rise = c.calc(op::FSUB, "Float", peak, base);
    put_num(&mut c, &r, "+", rise);
    put_num(&mut c, &r, " for ", lasted);
    line(&mut c, &r, " s");
    c.global("Debug", "Notification", [string("vibe_rm dualcast: over")]);
    c.finish(r)
}

/// One case: the spell readied in both hands, then two casts measured.
fn test(case: &Case, index: usize) -> FunctionBuilder {
    let (mut c, r) = report();
    let player = auto_var("Player");
    let sp = auto_var(&format!("Spell{index}"));
    let fx = auto_var(&format!("Effect{index}"));
    if index == 0 {
        c.global(
            "Debug",
            "MessageBox",
            [string(
                "vibe_rm dualcast: for each of 7 spells, readied in both hands for you, \
                 cast it once with one hand, wait for \"over\", then dual cast it \
                 (both buttons together) and wait for \"over\" again.",
            )],
        );
        let had = c.method("Actor", player.clone(), "HasPerk", [auto_var("DualPerk")]);
        c.assign(id("HadPerk"), had);
        c.method("Actor", player.clone(), "AddPerk", [auto_var("DualPerk")]);
    }
    c.method(
        "Actor",
        player.clone(),
        "AddSpell",
        [sp.clone(), false.into()],
    );
    for hand in [0, 1] {
        c.method(
            "Actor",
            player.clone(),
            "EquipSpell",
            [sp.clone(), hand.into()],
        );
    }
    c.wait(1.0);
    c.global(
        "Debug",
        "Notification",
        [string(&format!(
            "vibe_rm dualcast {} of {}: cast once, then dual cast",
            index + 1,
            CASES.len()
        ))],
    );
    line(&mut c, &r, &format!("{}: ", case.label));
    let skip = c.label();
    for n in [1, 2] {
        let m = c.own("Measure", "String", [fx.clone()]);
        let none = c.calc(op::CMP_EQ, "Bool", m.clone(), string(""));
        let ok = c.label();
        c.jmpf(none, ok);
        line(&mut c, &r, &format!("cast {n}: none in {CAST_WAIT} s"));
        c.jmp(skip);
        c.bind(ok);
        line(&mut c, &r, &format!("cast {n}: "));
        c.append(r.clone(), [m]);
        if n == 1 {
            line(&mut c, &r, "; ");
        }
    }
    c.bind(skip);
    c.method("Actor", player.clone(), "RemoveSpell", [sp]);
    if index == CASES.len() - 1 {
        let keep = c.label();
        c.jmpt(id("HadPerk"), keep);
        c.method("Actor", player, "RemovePerk", [auto_var("DualPerk")]);
        c.bind(keep);
    }
    c.finish(r)
}

fn scripts() -> Result<Vec<(String, Vec<u8>)>> {
    let mut s = ScriptBuilder::new(SCRIPT, "Quest");
    s.auto_property("Player", "Actor");
    s.auto_property("DualPerk", "Perk");
    for i in 0..CASES.len() {
        s.auto_property(&format!("Spell{i}"), "Spell");
        s.auto_property(&format!("Effect{i}"), "MagicEffect");
    }
    s.variable("Ran", "Bool", false.into());
    s.variable("HadPerk", "Bool", false.into());
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
        .function(
            "OnUpdate",
            on_update_with(
                "dualcast",
                "vibe_rm dualcast running: cast as the notifications say.",
                &tests,
            ),
        )
        .function("Num", num())
        .function("Measure", measure());
    for (i, case) in CASES.iter().enumerate() {
        s.function(&format!("Test{}", case.name), test(case, i));
    }
    Ok(vec![(SCRIPT.into(), papyrus::pex::write(&s.build()?)?)])
}

pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    let v = Vanilla { lo };
    let either_hand = v.id("EitherHand")?;
    let perk = v.id("AlterationDualCasting")?;

    let mut effects = Vec::new();
    let mut spells = Vec::new();
    for (i, case) in CASES.iter().enumerate() {
        let effect_id = EFFECTS + i as u32;
        let (scale, power) = (case.scale, case.power);
        effects.push(records::copy(
            lo,
            "AlchFortifyCarryWeight",
            effect_id,
            &format!("VrmDualCast{}Effect", case.name),
            None,
            move |tag, d| match tag {
                b"KSIZ" | b"KWDA" => d.clear(),
                b"DATA" if d.len() >= 0x74 => {
                    let put = |d: &mut Vec<u8>, o: usize, v: u32| {
                        d[o..o + 4].copy_from_slice(&v.to_le_bytes())
                    };
                    let flags = u32::from_le_bytes(d[0..4].try_into().unwrap());
                    put(
                        d,
                        0,
                        (flags & !(POWER_MAGNITUDE | POWER_DURATION)) | RECOVER | power,
                    );
                    // No keyword (a peak value modifier's), Alteration, a
                    // plain value modifier.
                    put(d, 0x08, 0);
                    put(d, 0x0C, ALTERATION);
                    put(d, 0x40, 0);
                    put(d, 0x50, spell::FIRE_AND_FORGET);
                    put(d, 0x54, spell::SELF);
                    // No dual casting art; the scale.
                    put(d, 0x6C, 0);
                    put(d, 0x70, scale.to_bits());
                }
                _ => {}
            },
        )?);
        spells.push(records::spell(
            SPELLS + i as u32,
            &format!("VrmDualCast{}", case.name),
            spell::SPELL,
            spell::FIRE_AND_FORGET,
            spell::SELF,
            case.spell_flags,
            Some(either_hand),
            &[Effect::new(own(effect_id), MAGNITUDE, DURATION)],
        ));
    }

    let object = PropertyValue::Object;
    let mut properties = vec![
        ("Player".to_owned(), object(PLAYER_REF)),
        ("DualPerk".to_owned(), object(perk)),
    ];
    for i in 0..CASES.len() {
        properties.push((format!("Spell{i}"), object(own(SPELLS + i as u32))));
        properties.push((format!("Effect{i}"), object(own(EFFECTS + i as u32))));
    }
    let quests = vec![
        records::Quest {
            object_id: QUEST,
            edid: "VrmDualCastTestQuest",
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
    let plugin = write_plugin(
        0,
        &["Skyrim.esm"],
        "vibe_rm",
        "Dual casting's power: spells of ours the player casts singly and dual, reports saved.",
        NEXT_OBJECT_ID,
        &[
            Node::top(b"MGEF", effects),
            Node::top(b"SPEL", spells),
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
            ("DualPerk".to_owned(), form(0x153CD, "Perk")),
        ];
        for i in 0..CASES.len() {
            properties.push((format!("Spell{i}"), form(own(SPELLS + i as u32).0, "Spell")));
            properties.push((
                format!("Effect{i}"),
                form(own(EFFECTS + i as u32).0, "MagicEffect"),
            ));
        }
        // The instructions, then a report a case.
        let shown = run(
            scripts().unwrap(),
            &[(own(QUEST).0, SCRIPT, properties)],
            CASES.len() + 1,
        );
        assert_eq!(shown.boxes.len(), CASES.len() + 1, "every test reports");
        assert_eq!(shown.saved_at_box, Some(1), "saves before the reports");
        let all = shown.boxes.join("\n");
        let expected = "scale 1.5, power affects duration: cast 1: none in 120 s";
        assert!(all.contains(expected), "{expected:?} not in the reports");
    }
}
