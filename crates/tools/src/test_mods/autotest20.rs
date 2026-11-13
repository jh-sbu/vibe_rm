//! `test-mod autotest20`: autotest19's two unanswered questions
//! (`plans/animation-unblocked.md`, "Follow-up probe"): when an actor
//! swims, and whether a fighter sprints to charge. Papyrus source
//! (`autotest20/*.psc`), one start-game-enabled quest; each test is
//! announced and followed by a save.
//!
//! **The detector.** autotest19 read `IsSwimming` through `HasMagicEffect`,
//! which counts an ability's effect whose condition fails, so it read True
//! everywhere. Here an ability carries two effects, `IsSwimming == 1`
//! (+10 `OneHanded`) and `IsSwimming == 0` (+10 `TwoHanded`), each with a
//! script (`VrmAt20SwimLog`) reporting `OnEffectStart` / `OnEffectFinish`
//! to the quest. Each reading gives both: `ev` from the events and `av`
//! from the gains (the probe autotest22 showed works), `S` swimming, `D`
//! dry, `-` neither. A working detector reads `-D` on land.
//!
//! 1. **Dry**: the detector added on land where the run began, on the
//!    player and on an NPC 200 off, read 1 and 3 s later.
//! 2. **Charge1500**, 3. **Charge2500**: a brave NPC with a sword is put in
//!    combat with a commoner (1000 more health, `SetRestrained`) 300 units
//!    off. Once it has closed to 200 (or after 8 s) the commoner is moved
//!    1500 or 2500 past it along the same line, and the fighter is sampled
//!    for 14 s or until it is within 200: its distance, then `S`
//!    sprinting, `R` running or `W` neither (`IsSprinting`, `IsRunning`),
//!    four samples a line, the time first (samples about 0.6 s apart).
//!    autotest19's fighter never found its target 1500 off; this one
//!    has already found it.
//! 4. **SwimUp**, 5. **SwimDown**: the player moved to markers on a lake's
//!    bed (autotest19's way, [`places::shore`]) at depths 40, 60, 80, 90,
//!    95, 100, 105, 110, 120, 140 and 180 under the surface, shallow to deep,
//!    then deep to shallow without leaving the water. At each: the bed's
//!    depth, the detector 1 and 3 s after the move with the feet against
//!    the surface, and the effects' events since the move with their times.
//!    The first move there disables the slaughterfish (autotest19's
//!    attacked the player and spoilt the NPC and Waterbreathing pages) and
//!    anyone hostile within 6000; they are enabled again at the end.
//! 6. **SwimNpc**: an NPC moved shallow to deep, then back to the
//!    shallowest, the player on the shore; also its distance from the
//!    marker at 3 s (autotest19's NPC drifted).
//! 7. **SwimBreathing**: SwimUp with `AbWaterbreathing`.
//!
//! The player is moved back where the run began at the end.
//!
//! | test | reading | ours predicts (`src/water.rs`, `src/ai/combat.rs`) |
//! | ---- | ------- | ------------------------------------------------- |
//! | Dry | `-D` | `-D`; else the detector is void |
//! | Charge | `S` from the move until in reach | sprint beyond 1000 and on into reach (`known_ambiguities/combat-movement.md`) |
//! | Charge | `R` throughout, or `W` | NPCs don't sprint to charge: drop the rule |
//! | Charge | `S` only while far, then `R` | a switch distance: read it off |
//! | SwimUp | `S` from depth d | entry strictly over 100 (`SWIM_DEPTH`): `S` from 105 |
//! | SwimUp | feet - surface | -92 when swimming (autotest19), on the bed otherwise |
//! | SwimDown | `S` deeper than d' < d | hysteresis; ours leaves where the bed is under 96 (`S` at 100, `D` from 95) |
//! | SwimNpc differs from SwimUp | NPCs have their own depth; ours uses one for all |
//! | SwimBreathing differs from SwimUp | Waterbreathing changes the state; ours doesn't |
//!
//! Run: a throwaway save outdoors on open, flat ground (the charge needs
//! 2500 units of it), out of combat, Survival Mode off. Wait for the
//! "running" notification and leave the game alone until the "done" box
//! (7 tests, about 5 minutes). After a crash, load the newest save, open the
//! console at once and `set VrmAt20SkipTest to <n>` to step past test n
//! (see the module doc of `test_mods`). Read with `vrm-tool test-results`.
//!
//! **Run 2026-11-13** (`research/autotest20.md`): the detector worked
//! (`-D` on land). Swimming began between bed depths 100.4 and 105.1 for
//! the player, the NPC and with Waterbreathing alike, and teleporting back
//! showed the same bound. A swimmer placed at 105 to 120 held at about
//! -93, but one placed deeper stayed deeper. Both charges were faults: the
//! fighter never engaged the held target (searching, then out of combat).

use anyhow::Result;
use esp::writer::{AttachedScript, Node, PropertyValue, RecordBuilder, write_plugin};
use esp::{FormId, LoadOrder, record_flags};

use super::autotest2::{Npc, Vanilla};
use super::places::{self, World};
use super::records::{self, Effect, Quest, ctda, op as cmp, run_on, spell};
use super::venv::placement;
use super::{TestMod, own, script};

const QUEST: u32 = 0x800;
const FIGHTER: u32 = 0x801;
const COMMONER: u32 = 0x802;
const SWIM_EFFECT: u32 = 0x803;
const DRY_EFFECT: u32 = 0x804;
const SWIM_ABILITY: u32 = 0x805;
const SKIP_TEST: u32 = 0x806;
const MARKERS: u32 = 0x810;
const NEXT_OBJECT_ID: u32 = MARKERS + DEPTHS.len() as u32;

const SCRIPT: &str = "VrmAt20Script";
const LOG_SCRIPT: &str = "VrmAt20SwimLog";
const SOURCE: &str = include_str!("autotest20/autotest20.psc");
const LOG_SOURCE: &str = include_str!("autotest20/swimlog.psc");
const PLAYER_REF: FormId = FormId(0x14);
const TAMRIEL: FormId = FormId(0x3C);
/// Where the lake is looked for from (autotest19's).
const RIVERWOOD: [f32; 2] = [18700.0, -46930.0];
/// The bed depths of the swimming markers: autotest19's, with 90, 95,
/// 105 and 110 about our thresholds.
const DEPTHS: [f32; 11] = [
    40.0, 60.0, 80.0, 90.0, 95.0, 100.0, 105.0, 110.0, 120.0, 140.0, 180.0,
];
/// The tests' report variables, in running order.
#[cfg(test)]
const TESTS: [&str; 7] = [
    "Dry",
    "Charge1500",
    "Charge2500",
    "SwimUp",
    "SwimDown",
    "SwimNpc",
    "SwimBreathing",
];

/// The main script's properties: name, type, and our own form or a
/// Skyrim.esm editor id.
const PROPERTIES: &[(&str, &str, Source)] = &[
    ("Sword", "Weapon", Source::Vanilla("IronSword")),
    ("XMarker", "Form", Source::Vanilla("XMarker")),
    ("FishRace", "Race", Source::Vanilla("SlaughterfishRace")),
    (
        "Waterbreathing",
        "Spell",
        Source::Vanilla("AbWaterbreathing"),
    ),
    ("FighterBase", "ActorBase", Source::Own(FIGHTER)),
    ("TargetBase", "ActorBase", Source::Own(COMMONER)),
    ("SwimmerBase", "ActorBase", Source::Own(COMMONER)),
    ("SwimAbility", "Spell", Source::Own(SWIM_ABILITY)),
    ("SkipTest", "GlobalVariable", Source::Own(SKIP_TEST)),
];

enum Source {
    Vanilla(&'static str),
    Own(u32),
}

/// The generated header: properties, and the markers as an array.
fn header() -> String {
    let mut s = format!("Scriptname {SCRIPT} extends Quest\n");
    s += "Actor Property Player Auto\nFloat Property Surface Auto\n";
    for (name, ty, _) in PROPERTIES {
        s += &format!("{ty} Property {name} Auto\n");
    }
    for k in 0..DEPTHS.len() {
        s += &format!("ObjectReference Property Marker{k} Auto\n");
    }
    s += "ObjectReference[] Markers\n\nFunction FillMarkers()\n";
    s += &format!("    Markers = new ObjectReference[{}]\n", DEPTHS.len());
    for k in 0..DEPTHS.len() {
        s += &format!("    Markers[{k}] = Marker{k}\n");
    }
    s += "EndFunction\n\n";
    s
}

fn scripts() -> Result<Vec<(String, Vec<u8>)>> {
    let main = header() + SOURCE;
    script::compile(&[&main, LOG_SOURCE])
}

pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    let v = Vanilla { lo };
    let shore = places::shore(lo, TAMRIEL, RIVERWOOD, &DEPTHS)?;
    eprintln!(
        "autotest20: the lake at {:?} ({}), surface {}, markers {:?}",
        shore.grid, shore.cell, shore.surface, shore.points
    );
    let object = PropertyValue::Object;
    let mut properties = vec![
        ("Player".to_owned(), object(PLAYER_REF)),
        ("Surface".to_owned(), PropertyValue::Float(shore.surface)),
    ];
    for (name, _, source) in PROPERTIES {
        let id = match source {
            Source::Vanilla(edid) => v.id(edid)?,
            Source::Own(id) => own(*id),
        };
        properties.push((name.to_string(), object(id)));
    }
    for k in 0..DEPTHS.len() {
        properties.push((format!("Marker{k}"), object(own(MARKERS + k as u32))));
    }

    let template = v.id("WEMerchantMiscMaleCommoner")?;
    let (race, class) = (v.id("ImperialRace")?, v.id("Citizen")?);
    let npcs: Vec<RecordBuilder> = [(FIGHTER, "Fighter", (1, 3)), (COMMONER, "Commoner", (0, 2))]
        .into_iter()
        .map(|(object_id, edid, temper)| {
            Npc {
                object_id,
                edid: format!("VrmAt20{edid}"),
                name: Some("vibe_rm Test NPC"),
                level: (10, false, 1, 0),
                template: (template, 0x1),
                temper,
                factions: Vec::new(),
                script: None,
                assistance: 0,
                combat_style: None,
                unique: false,
            }
            .record(race, class)
        })
        .collect();

    // The detector's effects: a constant effect on self raising a skill
    // (`OneHanded` 6 swimming, `TwoHanded` 7 dry), its script reporting.
    let effect = |object_id: u32, edid: &str, kind: &str, actor_value: u32| {
        let script = AttachedScript {
            name: LOG_SCRIPT.into(),
            properties: vec![
                ("Owner".into(), object(own(QUEST))),
                ("Kind".into(), PropertyValue::String(kind.into())),
            ],
        };
        records::copy(
            lo,
            "AlchFortifyOneHanded",
            object_id,
            edid,
            Some(script),
            |tag, d| {
                if tag == b"DATA" && d.len() >= 0x58 {
                    d[0x44..0x48].copy_from_slice(&actor_value.to_le_bytes());
                    d[0x50..0x54].copy_from_slice(&spell::CONSTANT.to_le_bytes());
                    d[0x54..0x58].copy_from_slice(&spell::SELF.to_le_bytes());
                }
            },
        )
    };
    let effects = vec![
        effect(SWIM_EFFECT, "VrmAt20Swimming", "swim", 6)?,
        effect(DRY_EFFECT, "VrmAt20Dry", "dry", 7)?,
    ];
    const IS_SWIMMING: u16 = 185;
    let swimming = |value: f32| ctda(IS_SWIMMING, 0, 0, cmp::EQ, value, run_on::SUBJECT);
    let ability = records::spell(
        SWIM_ABILITY,
        "VrmAt20SwimmingAbility",
        spell::ABILITY,
        spell::CONSTANT,
        spell::SELF,
        0,
        None,
        &[
            Effect::new(own(SWIM_EFFECT), 10.0, 0).when(swimming(1.0)),
            Effect::new(own(DRY_EFFECT), 10.0, 0).when(swimming(0.0)),
        ],
    );

    let quest = Quest {
        object_id: QUEST,
        edid: "VrmAt20Quest",
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
        "When actors swim (by depth, in and out, with Waterbreathing, an NPC) and whether \
         a fighter sprints to charge. Run by script; reports saved.",
        NEXT_OBJECT_ID,
        &[
            Node::top(
                b"GLOB",
                vec![records::global(SKIP_TEST, "VrmAt20SkipTest", 0.0)],
            ),
            Node::top(b"MGEF", effects),
            Node::top(b"SPEL", vec![ability]),
            Node::top(b"NPC_", npcs),
            places::worlds(lo, worlds)?,
            Node::top(b"QUST", vec![quest]),
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
            ("Surface".to_owned(), papyrus::Value::Float(-200.0)),
        ];
        for (i, (name, ty, source)) in PROPERTIES.iter().enumerate() {
            let id = match source {
                Source::Vanilla(_) => 0x1000 + i as u32,
                Source::Own(id) => own(*id).0,
            };
            properties.push((name.to_string(), form(id, ty)));
        }
        for k in 0..DEPTHS.len() {
            properties.push((
                format!("Marker{k}"),
                form(own(MARKERS + k as u32).0, "ObjectReference"),
            ));
        }
        // Only the done box.
        let shown = run(scripts().unwrap(), &[(own(QUEST).0, SCRIPT, properties)], 1);
        assert_eq!(shown.boxes.len(), 1);
        assert_eq!(shown.saved_at_box, Some(0), "saved before the done box");
        assert_eq!(TESTS.len(), 7);
    }
}
