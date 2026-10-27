//! Manual spell visual probes: things only looking at the game settles. No
//! Papyrus query exposes a membrane, a particle or an impact, so the user
//! watches, takes screenshots and puts them in `tests/results/`. Builds
//! references to vanilla records, and exports no vanilla meshes, textures or
//! scripts.
//!
//! Build `vrm-tools` first, then generate with:
//! `target/debug/vrm-tool test-mod spell-visuals "<Skyrim SE>/Data" tests/assets/VrmSpellVisuals.esp`.
//! The PEX is emitted beside the plugin in `tests/assets/Scripts/`. Do not install
//! or run the game here; the user performs these probes on a disposable SE save.
//! The protocol, step by step, is in `plans/spell-visuals.md` (Manual SE probe).
//!
//! On its first update the quest puts the player at the spot our offscreen
//! captures use (Tamriel 18432, -47104, facing 140 degrees: a rock about
//! 350 units ahead), gives them Flames, Frostbite and Fireball with magicka
//! to spare, plays Oakflesh's shader on them once and asks for a save.
//! A test caster stands 250 units out towards open ground (heading 50 from
//! the spot) and casts by a UseMagic package (copies of Onmund's practice
//! Flames) chosen by `VrmFxNpcMode`, at a marker on the rock where the
//! player's Flames lands or at one in open air beyond Flames' range. Neither
//! line passes the player. `cqf VrmFx <function>` drives it:
//!
//! | Function | What it does |
//! | --- | --- |
//! | `Rock` / `Air` | Player to the spot facing the rock / open ground |
//! | `Side` / `AirSide` | Player where the caster's rock / open-air stream is seen side-on |
//! | `NpcFlames` / `NpcFlamesAir` | Caster holds Flames at the rock / into open air |
//! | `NpcFrost` / `NpcFrostAir` | Caster holds Frostbite at the rock / into open air |
//! | `NpcFireball` | Caster casts Fireball at the rock every few seconds |
//! | `NpcStop` | Caster stops and holds its place |
//! | `Night` | Game hour 23, where additive fire reads best |
//! | `StopVisual` | Stops the Oakflesh shader |
//! | `Help` | Shows the steps again |
//!
//! | Question | Predicted outcome (not an observed SE result) |
//! | --- | --- |
//! | Oakflesh after reloading the requested save | Absent: supports our transient script shaders. Present: contradicts it. |
//! | Flames held on the rock | Our engine: a white-hot ball of stacked impacts. One steady impact means SE doesn't stack them. |
//! | Flames stream density | Our engine: sparse (about 5 fire particles per mover node). |
//! | Flames' jet | Our engine: faint red heat along the spray. |
//! | Frostbite / Fireball | Our engine: spray and fire trail forward along the projectile. |
//!
//! A missing initial Oakflesh sheen invalidates the persistence comparison.
//! The caster is new for this probe: if it doesn't cast, or casts at the
//! wrong place, note what it does instead; it's meant for later tests too.
use std::cell::Cell;

use super::autotest2::{Npc, Vanilla};
use super::autotest6::{ALWAYS, pldt};
use super::places::{self, World};
use super::records::{self, Quest};
use super::venv::placement;
use super::{TestMod, own, script::Code};
use anyhow::Result;
use esp::{
    FormId, LoadOrder, record_flags,
    writer::{AttachedScript, Node, PropertyValue, RecordBuilder, write_plugin},
};
use papyrus::build::{Arg, ScriptBuilder, auto_var, id, string};

const SCRIPT: &str = "VrmSpellVisualsProbe";
const QUEST: u32 = 0x800;
const MODE: u32 = 0x801;
/// Markers: the spot facing the rock, facing open ground, seeing the rock
/// stream side-on, seeing the open-air stream side-on, the caster's place,
/// the rock target and the open-air target.
const SPOT_ROCK: u32 = 0x810;
const SPOT_AIR: u32 = 0x811;
const SPOT_SIDE: u32 = 0x812;
const SPOT_AIR_SIDE: u32 = 0x813;
const CASTER_SPOT: u32 = 0x814;
const ROCK_TARGET: u32 = 0x815;
const AIR_TARGET: u32 = 0x816;
const CASTER: u32 = 0x818;
const CASTER_NPC: u32 = 0x820;
const PACKAGES: u32 = 0x830;
const NEXT_OBJECT_ID: u32 = 0x840;

/// The spot (where `vibe_rm` starts: the middle of Tamriel 4,-12), the rock
/// point the player's Flames lands on from there facing 140 degrees, and the
/// caster's place, 250 units towards heading 50. Heights are the ground's
/// in vibe_rm, which the markers stand on.
const SPOT: [f32; 3] = [18432.0, -47104.0, -94.0];
const ROCK: [f32; 3] = [18654.8, -47369.6, -18.0];
const CASTER_AT: [f32; 3] = [18623.5, -46943.3, -118.0];
/// 1200 units on from the caster at heading 50, chest high: past Flames'
/// 768-unit range.
const AIR: [f32; 3] = [19542.8, -46172.0, 6.0];
/// 400 units off the open-air stream's first 300, facing it.
const AIR_SIDE: [f32; 3] = [19110.4, -47056.9, -78.0];

/// The caster's packages: (name, mode, spell, rock target or open air).
const CASTS: [(&str, u8, &str, bool); 5] = [
    ("Flames", 1, "Flames", true),
    ("FlamesAir", 2, "Flames", false),
    ("Frost", 3, "Frostbite", true),
    ("FrostAir", 4, "Frostbite", false),
    ("Fireball", 5, "Fireball", true),
];

const HELP: &str = "vibe_rm spell visuals (cqf VrmFx <function>)\n\
0. Console fw 81a (clear weather) to match vibe_rm's captures.\n\
1. Oakflesh: note the sheen now. Reload the save this asks for,\n\
   note whether the sheen is still there, then StopVisual.\n\
2. Your Flames: Rock, hold Flames on the rock 5 s; screenshot\n\
   at 1 s and at 5 s. Air, hold it into open air; screenshot.\n\
3. Caster: NpcFlames, then Side; watch and screenshot. NpcFlamesAir,\n\
   then AirSide. Walk around it as you like.\n\
4. NpcFrost / NpcFrostAir, the same. NpcFireball: catch the trail.\n\
5. Night, then repeat 2 and 3 for Flames. NpcStop ends it.\n\
Screenshots go in tests/results/spell-visuals/, named by step.\n\
Help shows this again.";

fn script() -> Result<Vec<u8>> {
    let mut s = ScriptBuilder::new(SCRIPT, "Quest");
    for (name, class) in [
        ("Player", "Actor"),
        ("Shader", "EffectShader"),
        ("Caster", "Actor"),
        ("Mode", "GlobalVariable"),
        ("GameHour", "GlobalVariable"),
        ("Flames", "Spell"),
        ("Frostbite", "Spell"),
        ("Fireball", "Spell"),
        ("SpotRock", "ObjectReference"),
        ("SpotAir", "ObjectReference"),
        ("SpotSide", "ObjectReference"),
        ("SpotAirSide", "ObjectReference"),
    ] {
        s.auto_property(name, class);
    }
    s.variable("Ran", "Bool", false.into());

    let mut init = Code::new("None");
    init.method(
        "Form",
        id("self"),
        "RegisterForSingleUpdate",
        [3.0f32.into()],
    );
    s.function("OnInit", init.finish(Arg::None));

    let mut run = Code::new("None");
    let go = run.label();
    run.jmpf(id("Ran"), go)
        .ret(Arg::None)
        .bind(go)
        .assign(id("Ran"), true.into());
    run.global("Game", "ForceThirdPerson", []);
    move_to(&mut run, "SpotRock");
    for actor in ["Player", "Caster"] {
        for spell in ["Flames", "Frostbite", "Fireball"] {
            run.method(
                "Actor",
                auto_var(actor),
                "AddSpell",
                [auto_var(spell), false.into()],
            );
        }
    }
    run.method(
        "Actor",
        auto_var("Player"),
        "SetActorValue",
        [string("Magicka"), 1000.0f32.into()],
    );
    run.method(
        "Actor",
        auto_var("Caster"),
        "SetActorValue",
        [string("Magicka"), 100_000.0f32.into()],
    );
    run.method(
        "EffectShader",
        auto_var("Shader"),
        "Play",
        [auto_var("Player"), (-1.0f32).into()],
    );
    run.global("Debug", "MessageBox", [string(HELP)]);
    run.wait(3.0);
    run.global("Game", "RequestSave", []);
    s.function("OnUpdate", run.finish(Arg::None));

    let mut stop = Code::new("None");
    stop.method(
        "EffectShader",
        auto_var("Shader"),
        "Stop",
        [auto_var("Player")],
    );
    s.function("StopVisual", stop.finish(Arg::None));

    for (name, spot) in [
        ("Rock", "SpotRock"),
        ("Air", "SpotAir"),
        ("Side", "SpotSide"),
        ("AirSide", "SpotAirSide"),
    ] {
        let mut c = Code::new("None");
        move_to(&mut c, spot);
        s.function(name, c.finish(Arg::None));
    }
    let modes =
        std::iter::once(("NpcStop", 0u8)).chain(CASTS.iter().map(|&(name, mode, ..)| (name, mode)));
    for (name, mode) in modes {
        let mut c = Code::new("None");
        c.method(
            "GlobalVariable",
            auto_var("Mode"),
            "SetValue",
            [(mode as f32).into()],
        );
        c.method("Actor", auto_var("Caster"), "EvaluatePackage", []);
        let function = if name == "NpcStop" {
            name.to_owned()
        } else {
            format!("Npc{name}")
        };
        s.function(&function, c.finish(Arg::None));
    }
    let mut night = Code::new("None");
    night.method(
        "GlobalVariable",
        auto_var("GameHour"),
        "SetValue",
        [23.0f32.into()],
    );
    s.function("Night", night.finish(Arg::None));
    let mut help = Code::new("None");
    help.global("Debug", "MessageBox", [string(HELP)]);
    s.function("Help", help.finish(Arg::None));
    Ok(papyrus::pex::write(&s.build()?)?)
}

/// The player to `spot`, turned as it is.
fn move_to(c: &mut Code, spot: &str) {
    c.method(
        "ObjectReference",
        auto_var("Player"),
        "MoveTo",
        [
            auto_var(spot),
            0.0f32.into(),
            0.0f32.into(),
            0.0f32.into(),
            true.into(),
        ],
    );
}

/// Heading (radians clockwise from north, 0 to 2 pi) from `a` to `b`.
fn heading(a: [f32; 3], b: [f32; 3]) -> f32 {
    (b[0] - a[0])
        .atan2(b[1] - a[1])
        .rem_euclid(std::f32::consts::TAU)
}

/// A copy of Onmund's practice Flames casting `spell` at `target` from the
/// caster's place, held 6 s every 8 s, while `VrmFxNpcMode` is `mode`.
fn cast_package(
    lo: &LoadOrder,
    object_id: u32,
    name: &str,
    mode: u8,
    spell: FormId,
    target: FormId,
) -> Result<RecordBuilder> {
    // UseMagic's values after the location, spell and target: hold when
    // blocked, cast time min / max, cooldown min / max, casts min / max, dual.
    let values: [&[u8]; 8] = [
        &[0],
        &6.0f32.to_le_bytes(),
        &6.0f32.to_le_bytes(),
        &2.0f32.to_le_bytes(),
        &2.0f32.to_le_bytes(),
        &1i32.to_le_bytes(),
        &1000i32.to_le_bytes(),
        &[0],
    ];
    let value = Cell::new(0);
    records::copy_inserting(
        lo,
        "WCollegeOnmundPracticeFlames12x4",
        object_id,
        &format!("VrmFxCast{name}"),
        None,
        |tag, d| match tag {
            b"PLDT" => *d = pldt(0, own(CASTER_SPOT), 32),
            b"PSDT" => *d = ALWAYS.to_vec(),
            // Onmund's conditions (the College's) give way to the mode's.
            b"CTDA" | b"CIS2" => d.clear(),
            // Target selectors: type 1 a specific form (the spell), type 0
            // a reference (the target).
            b"PTDA" if d.len() >= 8 => {
                let form = if d[0] == 1 { spell } else { target };
                d[4..8].copy_from_slice(&form.0.to_le_bytes());
            }
            b"CNAM" => {
                let i = value.get();
                if let Some(v) = values.get(i) {
                    *d = v.to_vec();
                }
                value.set(i + 1);
            }
            _ => {}
        },
        |tag| match tag {
            b"PSDT" => vec![(
                *b"CTDA",
                records::ctda(
                    records::func::GET_GLOBAL_VALUE,
                    own(MODE).0,
                    0,
                    records::op::EQ,
                    mode as f32,
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
    let spell = |edid: &str| v.id(edid);
    let marker = v.id("XMarker")?;
    let heading_marker = v.id("XMarkerHeading")?;
    let persistent = record_flags::PERSISTENT;
    let object = PropertyValue::Object;

    let quest = Quest {
        object_id: QUEST,
        edid: "VrmFx",
        flags: 0x11,
        priority: 0,
        kind: 0,
        script: Some(AttachedScript {
            name: SCRIPT.into(),
            properties: vec![
                ("Player".into(), object(FormId(0x14))),
                ("Shader".into(), object(v.id("MagicArmorOakFleshFXS")?)),
                ("Caster".into(), object(own(CASTER))),
                ("Mode".into(), object(own(MODE))),
                ("GameHour".into(), object(v.id("GameHour")?)),
                ("Flames".into(), object(spell("Flames")?)),
                ("Frostbite".into(), object(spell("Frostbite")?)),
                ("Fireball".into(), object(spell("Fireball")?)),
                ("SpotRock".into(), object(own(SPOT_ROCK))),
                ("SpotAir".into(), object(own(SPOT_AIR))),
                ("SpotSide".into(), object(own(SPOT_SIDE))),
                ("SpotAirSide".into(), object(own(SPOT_AIR_SIDE))),
            ],
        }),
        stages: &[],
        aliases: Vec::new(),
    }
    .record();

    let mut packages = Vec::new();
    let mut ids = Vec::new();
    for (i, &(name, mode, edid, rock)) in CASTS.iter().enumerate() {
        let target = own(if rock { ROCK_TARGET } else { AIR_TARGET });
        let id = PACKAGES + i as u32;
        packages.push(cast_package(lo, id, name, mode, spell(edid)?, target)?);
        ids.push(own(id));
    }
    // Otherwise it holds its place.
    let hold = PACKAGES + CASTS.len() as u32;
    packages.push(records::copy(
        lo,
        "MS13LucanHoldPositionPackage",
        hold,
        "VrmFxHold",
        None,
        |tag, d| match tag {
            b"PLDT" => *d = pldt(0, own(CASTER_SPOT), 0),
            b"PSDT" => *d = ALWAYS.to_vec(),
            b"CTDA" | b"CIS2" => d.clear(),
            // Flag 0x1: offers services.
            b"PKDT" if d.len() >= 4 => d[0] &= !0x1,
            _ => {}
        },
    )?);
    ids.push(own(hold));

    let npc = Npc {
        object_id: CASTER_NPC,
        edid: "VrmFxCasterNpc".into(),
        name: Some("vibe_rm Test Caster"),
        level: (10, false, 1, 0),
        template: (v.id("WEMerchantMiscMaleCommoner")?, 0x1),
        temper: (0, 0),
        factions: Vec::new(),
        script: None,
        assistance: 0,
        combat_style: None,
        unique: true,
    }
    .record_with_packages(v.id("ImperialRace")?, v.id("Citizen")?, &ids);

    let at_rock = heading(SPOT, ROCK);
    let air = 50f32.to_radians();
    let side_mid = [
        (CASTER_AT[0] + ROCK[0]) / 2.0,
        (CASTER_AT[1] + ROCK[1]) / 2.0,
        0.0,
    ];
    let air_mid = [
        CASTER_AT[0] + 300.0 * air.sin(),
        CASTER_AT[1] + 300.0 * air.cos(),
        0.0,
    ];
    let reference = |object_id: u32, base: FormId, at: [f32; 3], rot_z: f32| {
        RecordBuilder::new(b"REFR", own(object_id))
            .flags(persistent)
            .form(b"NAME", base)
            .field(b"DATA", &placement(at, rot_z))
    };
    let refs = vec![
        reference(SPOT_ROCK, heading_marker, SPOT, at_rock),
        reference(SPOT_AIR, heading_marker, SPOT, air),
        reference(SPOT_SIDE, heading_marker, SPOT, heading(SPOT, side_mid)),
        reference(
            SPOT_AIR_SIDE,
            heading_marker,
            AIR_SIDE,
            heading(AIR_SIDE, air_mid),
        ),
        reference(
            CASTER_SPOT,
            heading_marker,
            CASTER_AT,
            heading(CASTER_AT, ROCK),
        ),
        reference(ROCK_TARGET, marker, ROCK, 0.0),
        reference(AIR_TARGET, marker, AIR, 0.0),
        RecordBuilder::new(b"ACHR", own(CASTER))
            .flags(persistent)
            .form(b"NAME", own(CASTER_NPC))
            .field(b"DATA", &placement(CASTER_AT, heading(CASTER_AT, ROCK))),
    ];
    let worlds = vec![World {
        world: v.id("Tamriel")?,
        name: "Skyrim",
        grid: (4, -12),
        persistent: refs,
        temporary: Vec::new(),
    }];

    // Top groups in the Creation Kit's order.
    Ok(TestMod {
        plugin: write_plugin(
            0,
            &["Skyrim.esm"],
            "vibe_rm",
            "Spell visuals by eye: Oakflesh's shader after a reload, Flames, Frostbite and \
             Fireball cast by the player and by a test caster at a rock and into open air, \
             for screenshots compared with vibe_rm's.",
            NEXT_OBJECT_ID,
            &[
                Node::top(b"GLOB", vec![records::global(MODE, "VrmFxNpcMode", 0.0)]),
                Node::top(b"NPC_", vec![npc]),
                places::worlds(lo, worlds)?,
                Node::top(b"QUST", vec![quest]),
                Node::top(b"PACK", packages),
            ],
        ),
        scripts: vec![(SCRIPT.into(), script()?)],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_mods::script::dry_run::{form, run};

    #[test]
    fn probe_script_uses_declared_native_signatures() {
        let bytes = super::script().unwrap();
        let p = papyrus::pex::parse(&bytes).unwrap();
        assert!(super::super::script::mistyped_calls(&p).is_empty());
    }

    #[test]
    fn first_update_shows_the_steps_on_one_screen() {
        let object = |name: &str, id: u32, class: &str| (name.to_owned(), form(id, class));
        let properties = vec![
            object("Player", 0x14, "Actor"),
            object("Shader", 0x1, "EffectShader"),
            object("Caster", own(CASTER).0, "Actor"),
            object("Mode", own(MODE).0, "GlobalVariable"),
            object("GameHour", 0x38, "GlobalVariable"),
            object("Flames", 0x12FCD, "Spell"),
            object("Frostbite", 0x2B96B, "Spell"),
            object("Fireball", 0x1C789, "Spell"),
            object("SpotRock", own(SPOT_ROCK).0, "ObjectReference"),
            object("SpotAir", own(SPOT_AIR).0, "ObjectReference"),
            object("SpotSide", own(SPOT_SIDE).0, "ObjectReference"),
            object("SpotAirSide", own(SPOT_AIR_SIDE).0, "ObjectReference"),
        ];
        let shown = run(
            vec![(SCRIPT.into(), script().unwrap())],
            &[(own(QUEST).0, SCRIPT, properties)],
            1,
        );
        assert_eq!(shown.boxes.len(), 1);
        assert!(shown.boxes[0].lines().count() <= 20, "{}", shown.boxes[0]);
    }

    #[test]
    fn neither_cast_line_passes_the_player() {
        // The player's distance from each line the caster casts along.
        let off_line = |to: [f32; 3]| {
            let (dx, dy) = (to[0] - CASTER_AT[0], to[1] - CASTER_AT[1]);
            let (px, py) = (SPOT[0] - CASTER_AT[0], SPOT[1] - CASTER_AT[1]);
            let t = ((px * dx + py * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
            ((px - t * dx).powi(2) + (py - t * dy).powi(2)).sqrt()
        };
        assert!(off_line(ROCK) > 150.0, "{}", off_line(ROCK));
        assert!(off_line(AIR) > 150.0, "{}", off_line(AIR));
        assert!(NEXT_OBJECT_ID > PACKAGES + CASTS.len() as u32);
    }
}
