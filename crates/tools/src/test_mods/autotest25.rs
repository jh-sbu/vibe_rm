//! `test-mod autotest25`: swimming in authored water, and a lake's current
//! (`research/autotest20.md`, "Faults and open items"). autotest20's lake
//! had a current, a wading floor higher than its terrain, and creatures;
//! these cells have none of them. The charge is left for a later batch.
//!
//! **Two interior cells of ours**, laid out alike, the water's surface at
//! z 0 in both:
//!
//! - `VrmAt25LevelWater`: a cell water level (`DATA` has-water flag,
//!   `XCLW` 0, `XCWT` `DefaultIntWaterWaistHeightStill`).
//! - `VrmAt25PlacedWater`: six `Water2048DefaultIntWaistStill` (a placed
//!   still water of that type) at z 0, the cell flagged as having water
//!   with no level (`XCLW` the "none" value), as vanilla's placed-water
//!   interiors are (`FortNeugrad02`).
//!
//! In each: **steps**, sixteen 1024-square floor pieces
//! (`WRIntFloorSTMid01Large`) in a 4 × 4 grid, their tops [`STEPS`] under
//! the surface (the first 64 above it, dry), each with a marker. A
//! **ramp** south of them: 64 overlapping 256-square pieces
//! (`WRIntFloorSTMid01`) every 32 units along x, each [`RISE`] higher, from
//! 220 under the surface to 80 above. They make stairs rather than a tilted
//! piece, so nothing hangs on SE's rotation order. A pad at each end: 220
//! deep, and 80 up.
//!
//! The detector is autotest20's: an ability whose `IsSwimming == 1` and `== 0`
//! effects report `OnEffectStart` / `OnEffectFinish` and give +10
//! `OneHanded` / `TwoHanded` (`ev` and `av`, `S` swimming, `D` dry).
//!
//! Per cell (1–6 the level-water cell, 7–12 the placed-water one):
//!
//! 1. **Dry**: the detector on the player and an NPC on the dry step.
//! 2. **StepsUp**, 3. **StepsDown**: the player moved to each step: the
//!    detector at 1 and 3 s, the feet's height, the events with their times.
//!    The steps between 100 and 105 are a unit apart (autotest20: between
//!    100.4 and 105.1).
//! 4. **Ramp**: the player moved by `TranslateTo` at [`RAMP_SPEED`] units a
//!    second from the dry pad down the stairs into the deep (in), the feet
//!    3 above the treads; then, from floating 93 under the surface over the
//!    deep end (autotest20's swimmers floated 92.7 to 93.9 under), level
//!    until the treads come up to that, and on up and out.
//!    Each detector event gives the bed's depth under the player and its
//!    height then, and there's a sample every 4 s. Entry and exit depths that
//!    differ are hysteresis. The state lagged a move by about 0.7 s in
//!    autotest20, that is about 4 units of depth here.
//! 5. **Npc**: an NPC to each step, then back to the dry one.
//! 6. **Float**: the player moved to the 300 step and an NPC to the 180
//!    step; their feet every second for 20 s (autotest20's swimmers placed
//!    deep stayed deep for 3 s).
//!
//! **The current** (13, 14): [`places::current_site`] picks the water
//! current marker with the deepest water under it and no actor or critter
//! spawner within 4096: Update.esm's `01002B8A` in Tamriel (-23, -12), a
//! mountain lake 372 deep at surface 6000 with a waterfall, heading 5.7°
//! (build output prints it). Markers at the marker's spot and 512 and 1024
//! along its heading each way and 512 across, those over water 150 deep
//! (all seven: 180 to 372), 92 under the surface. **CurrentPlayer** leaves the player at each for
//! 10 s, reading every second how far they drifted along the heading and
//! across it, and the feet against the surface. **CurrentNpc** does the
//! same with an NPC, the player waiting near the marker. Hostiles within
//! 6000 are disabled first and enabled again at the end.
//!
//! | test | reading | ours predicts |
//! | ---- | ------- | ------------- |
//! | Dry | `-D` | `-D` (ours sends no `OnEffectStart` as the ability is added: `av` only); else the detector is void |
//! | Steps (level) | `S` from depth d | strictly over 100 (`src/water.rs`): `S` from 101 |
//! | Steps (placed) | `-D` throughout | ours has no placed water (`plans/ROADMAP.md`) |
//! | Steps, Float | feet -92 when swimming | -92 at every depth |
//! | Ramp out | `S` ends at bed b | ours: where the bed is shallower than 96 |
//! | Ramp in | `S` begins at bed b' | ours: the feet deeper than 100 |
//! | Current | drift along the heading | ours has no currents: none |
//!
//! **Dry run in our engine** (2026-11-13; `vibe_rm --plugin
//! tests/assets/VrmAutoTest25.esp --loose tests/assets --cell Riverwood
//! --screenshot <png> --wait 100000`, reports from
//! `RUST_LOG=vibe_rm::script::natives=debug`'s `papyrus trace` lines;
//! `VRM_SAVES` set aside): every test reports. Ours: level water swims from
//! the 100 step (its feet sank just past 100), floating at -92 throughout;
//! placed water reads dry everywhere; no current. On the ramp ours began
//! swimming at bed 105.7 on the way in, and left the state at once on the
//! way out (bed 215, feet held 90 to 91 under by `TranslateTo`), which is
//! ours to look into. Ours sends no `OnEffectStart` as the ability is
//! added, and its NPC sometimes jumped to another step (no navmesh here).
//!
//! Run: a throwaway save (anywhere), Survival Mode off (the lake is cold).
//! Wait for the "running" notification and leave the game alone until the
//! "done" box (14 tests, about 20 minutes). The player is moved into the
//! cells, to the mountain lake and back. After a crash, load the newest
//! save, open the console at once and `set VrmAt25SkipTest to <n>` to step
//! past test n (see the module doc of `test_mods`). Read with `vrm-tool
//! test-results`.
//!
//! **Run 2026-11-13** (`research/autotest25.md`): the level-water cell had
//! no water (SE makes none from an interior's `XCLW`). In placed water,
//! swimming began with the feet about 90 under, for the player and the NPC
//! alike; moving out continuously it lasted until the feet were 55 to 68
//! under; a still swimmer placed deep stayed deep. The current tests were
//! faults: `MoveTo` to the lake markers left the player in the cell
//! (`test-mod autotest26` repeats them).

use anyhow::Result;
use esp::writer::{AttachedScript, Node, PropertyValue, RecordBuilder, write_plugin};
use esp::{FormId, LoadOrder, group_type, record_flags};

use super::autotest2::{Npc, Vanilla};
use super::places::{self, World};
use super::records::{self, Effect, Quest, ctda, op as cmp, run_on, spell};
use super::venv::{placement, xcll};
use super::{TestMod, own, script};

const QUEST: u32 = 0x800;
const COMMONER: u32 = 0x801;
const SWIM_EFFECT: u32 = 0x802;
const DRY_EFFECT: u32 = 0x803;
const SWIM_ABILITY: u32 = 0x804;
const SKIP_TEST: u32 = 0x805;
/// The two cells: level water, placed water.
const CELLS: [u32; 2] = [0x806, 0x807];
/// Each cell's references from here: step markers, the ramp marker, then
/// floor pieces and water.
const CELL_REFS: [u32; 2] = [0x900, 0xA00];
const CURRENT_MARKERS: u32 = 0xB00;
const NEXT_OBJECT_ID: u32 = 0xB10;

const SCRIPT: &str = "VrmAt25Script";
const LOG_SCRIPT: &str = "VrmAt25SwimLog";
const SOURCE: &str = include_str!("autotest25/autotest25.psc");
const LOG_SOURCE: &str = include_str!("autotest25/swimlog.psc");
const PLAYER_REF: FormId = FormId(0x14);
const TAMRIEL: FormId = FormId(0x3C);

/// The steps' depths under the surface; the first is 64 above it.
const STEPS: [f32; 16] = [
    -64.0, 40.0, 60.0, 80.0, 90.0, 95.0, 100.0, 101.0, 102.0, 103.0, 104.0, 105.0, 110.0, 120.0,
    180.0, 300.0,
];
/// The ramp: its stairs' treads along x, their count, its depth at the deep
/// end and height at the top.
const TREAD: f32 = 32.0;
const TREADS: usize = 64;
const RAMP_DEEP: f32 = 220.0;
const RAMP_HIGH: f32 = 80.0;
const RISE: f32 = (RAMP_DEEP + RAMP_HIGH) / (TREADS as f32 - 1.0);
/// The ramp's line, y; its first tread is centred on x 0.
const RAMP_Y: f32 = -1024.0;
/// Half a small floor piece: the tread line starts this far before x 0.
const HALF_SMALL: f32 = 128.0;
const RAMP_SPEED: f32 = 40.0;

/// The tests' names, in running order.
const TESTS: [&str; 14] = [
    "LevelDry",
    "LevelStepsUp",
    "LevelStepsDown",
    "LevelRamp",
    "LevelNpc",
    "LevelFloat",
    "PlacedDry",
    "PlacedStepsUp",
    "PlacedStepsDown",
    "PlacedRamp",
    "PlacedNpc",
    "PlacedFloat",
    "CurrentPlayer",
    "CurrentNpc",
];

/// The main script's properties of forms: name, type, our own form or a
/// Skyrim.esm editor id.
const PROPERTIES: &[(&str, &str, Source)] = &[
    ("XMarker", "Form", Source::Vanilla("XMarker")),
    ("FishRace", "Race", Source::Vanilla("SlaughterfishRace")),
    ("SwimmerBase", "ActorBase", Source::Own(COMMONER)),
    ("SwimAbility", "Spell", Source::Own(SWIM_ABILITY)),
    ("SkipTest", "GlobalVariable", Source::Own(SKIP_TEST)),
];

enum Source {
    Vanilla(&'static str),
    Own(u32),
}

/// The current measurement's points, as the header lists them.
struct CurrentPoints<'a>(&'a [(&'static str, f32)]);

/// The generated header: properties, the marker arrays, the ramp's
/// geometry and the reports.
fn header(current: &CurrentPoints) -> String {
    let mut s = format!("Scriptname {SCRIPT} extends Quest\n");
    s += "Actor Property Player Auto\n";
    s += "Float Property CurrentSurface Auto\nFloat Property CurrentHeading Auto\n";
    for (name, ty, _) in PROPERTIES {
        s += &format!("{ty} Property {name} Auto\n");
    }
    for c in 0..2 {
        for k in 0..STEPS.len() {
            s += &format!("ObjectReference Property Step{c}_{k} Auto\n");
        }
        s += &format!("ObjectReference Property Ramp{c} Auto\n");
    }
    for i in 0..current.0.len() {
        s += &format!("ObjectReference Property Current{i} Auto\n");
    }
    s += &format!("Int TestCount = {}\n", TESTS.len());
    s += &format!("Int StepCount = {}\n", STEPS.len());
    s += &format!("Int CurrentCount = {}\n", current.0.len());
    s += &format!("Float RampTread = {TREAD:.1}\nInt RampTreads = {TREADS}\n");
    s += &format!("Float RampHalfTread = {HALF_SMALL:.1}\n");
    s += &format!("Float RampRise = {RISE}\nFloat RampDeep = {RAMP_DEEP:.1}\n");
    s += &format!("Float RampHigh = {RAMP_HIGH:.1}\nFloat RampSpeed = {RAMP_SPEED:.1}\n");
    // Where the tread line meets the top: its height RAMP_HIGH.
    let top = (RAMP_DEEP + RAMP_HIGH) * TREAD / RISE - HALF_SMALL;
    s += &format!("Float RampTop = {top:.1}\n");
    s += "ObjectReference[] Steps0\nObjectReference[] Steps1\nObjectReference[] Currents\n";
    s += "String[] CurrentLabels\nFloat[] CurrentDepths\n";
    for t in TESTS {
        s += &format!("String Result{t}\n");
    }
    s += "\nFunction FillMarkers()\n";
    for c in 0..2 {
        s += &format!("    Steps{c} = new ObjectReference[{}]\n", STEPS.len());
        for k in 0..STEPS.len() {
            s += &format!("    Steps{c}[{k}] = Step{c}_{k}\n");
        }
    }
    // Arrays of length 0 aren't allowed: keep one slot.
    let n = current.0.len().max(1);
    s += &format!("    Currents = new ObjectReference[{n}]\n");
    s += &format!("    CurrentLabels = new String[{n}]\n    CurrentDepths = new Float[{n}]\n");
    for (i, (label, depth)) in current.0.iter().enumerate() {
        s += &format!("    Currents[{i}] = Current{i}\n");
        s +=
            &format!("    CurrentLabels[{i}] = \"{label}\"\n    CurrentDepths[{i}] = {depth:.1}\n");
    }
    s += "EndFunction\n\n";
    s += "ObjectReference Function Step(Int c, Int k)\n    If c == 0\n        return Steps0[k]\n    EndIf\n    return Steps1[k]\nEndFunction\n\n";
    s += "ObjectReference Function Ramp(Int c)\n    If c == 0\n        return Ramp0\n    EndIf\n    return Ramp1\nEndFunction\n\n";
    s += "ObjectReference Function Current(Int i)\n    return Currents[i]\nEndFunction\n\n";
    s += "String Function CurrentLabel(Int i)\n    return CurrentLabels[i]\nEndFunction\n\n";
    s += "Float Function CurrentDepth(Int i)\n    return CurrentDepths[i]\nEndFunction\n\n";
    s += "String Function TestName(Int n)\n";
    for (i, t) in TESTS.iter().enumerate() {
        s += &format!("    If n == {}\n        return \"{t}\"\n    EndIf\n", i + 1);
    }
    s += "    return \"\"\nEndFunction\n\n";
    s += "Function Keep(Int n, String r)\n";
    for (i, t) in TESTS.iter().enumerate() {
        s += &format!("    If n == {}\n        Result{t} = r\n    EndIf\n", i + 1);
    }
    s += "EndFunction\n\n";
    s
}

fn scripts(current: &CurrentPoints) -> Result<Vec<(String, Vec<u8>)>> {
    let main = header(current) + SOURCE;
    script::compile(&[&main, LOG_SOURCE])
}

/// A persistent marker.
fn marker(object_id: u32, base: FormId, pos: [f32; 3], heading: f32) -> RecordBuilder {
    RecordBuilder::new(b"REFR", own(object_id))
        .flags(record_flags::PERSISTENT)
        .form(b"NAME", base)
        .field(b"DATA", &placement(pos, heading))
}

/// A temporary object.
fn object(object_id: u32, base: FormId, pos: [f32; 3]) -> RecordBuilder {
    RecordBuilder::new(b"REFR", own(object_id))
        .form(b"NAME", base)
        .field(b"DATA", &placement(pos, 0.0))
}

/// One cell's records: the cell, then its children.
fn cell(v: &Vanilla, c: usize) -> Result<(RecordBuilder, Node)> {
    let (large, small) = (v.id("WRIntFloorSTMid01Large")?, v.id("WRIntFloorSTMid01")?);
    let heading = v.id("XMarkerHeading")?;
    let water_type = v.id("DefaultIntWaterWaistHeightStill")?;
    let base = CELL_REFS[c];
    let mut persistent = Vec::new();
    let mut temporary = Vec::new();
    let mut next = base + 0x20;
    let mut add = |temporary: &mut Vec<RecordBuilder>, form: FormId, pos: [f32; 3]| {
        temporary.push(object(next, form, pos));
        next += 1;
    };
    for (k, depth) in STEPS.iter().enumerate() {
        let at = [(k % 4) as f32 * 1024.0, (k / 4) as f32 * 1024.0, -depth];
        persistent.push(marker(base + k as u32, heading, at, 0.0));
        add(&mut temporary, large, at);
    }
    persistent.push(marker(
        base + STEPS.len() as u32,
        heading,
        [0.0, RAMP_Y, -RAMP_DEEP],
        std::f32::consts::FRAC_PI_2,
    ));
    for j in 0..TREADS {
        let z = -RAMP_DEEP + j as f32 * RISE;
        add(&mut temporary, small, [j as f32 * TREAD, RAMP_Y, z]);
    }
    let last = (TREADS - 1) as f32 * TREAD + HALF_SMALL;
    add(
        &mut temporary,
        large,
        [-HALF_SMALL - 512.0, RAMP_Y, -RAMP_DEEP],
    );
    add(&mut temporary, large, [last + 512.0, RAMP_Y, RAMP_HIGH]);
    // DATA: interior (0x1), has water (0x2).
    let mut record = RecordBuilder::new(b"CELL", own(CELLS[c]))
        .zstring(b"EDID", ["VrmAt25LevelWater", "VrmAt25PlacedWater"][c])
        .zstring(b"FULL", ["vibe_rm Level Water", "vibe_rm Placed Water"][c])
        .field(b"DATA", &3u16.to_le_bytes())
        .field(b"XCLL", &xcll())
        .u32(b"LTMP", 0);
    if c == 0 {
        record = record
            .field(b"XCLW", &0f32.to_le_bytes())
            .form(b"XCWT", water_type);
    } else {
        // The "no water" level vanilla's placed-water interiors carry.
        record = record.field(b"XCLW", &(-2147483648.0f32).to_le_bytes());
        let plane = v.id("Water2048DefaultIntWaistStill")?;
        for (x, y) in [
            (512.0, 512.0),
            (2560.0, 512.0),
            (512.0, 2560.0),
            (2560.0, 2560.0),
            (-HALF_SMALL, RAMP_Y - 512.0),
            (2048.0 - HALF_SMALL, RAMP_Y - 512.0),
        ] {
            add(&mut temporary, plane, [x, y, 0.0]);
        }
    }
    Ok((
        record,
        places::cell_children(own(CELLS[c]), persistent, temporary),
    ))
}

pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    let v = Vanilla { lo };
    let site = places::current_site(lo, TAMRIEL, 150.0)?;
    eprintln!(
        "autotest25: current marker {} in {:?}, surface {}, heading {:.1}°, points {:?}",
        site.marker,
        site.grid,
        site.surface,
        site.heading.to_degrees(),
        site.points
    );
    let current: Vec<(&'static str, f32)> = site.points.iter().map(|p| (p.0, p.2)).collect();

    let object_value = PropertyValue::Object;
    let mut properties = vec![
        ("Player".to_owned(), object_value(PLAYER_REF)),
        (
            "CurrentSurface".to_owned(),
            PropertyValue::Float(site.surface),
        ),
        (
            "CurrentHeading".to_owned(),
            PropertyValue::Float(site.heading.to_degrees()),
        ),
    ];
    for (name, _, source) in PROPERTIES {
        let id = match source {
            Source::Vanilla(edid) => v.id(edid)?,
            Source::Own(id) => own(*id),
        };
        properties.push((name.to_string(), object_value(id)));
    }
    for c in 0..2 {
        for k in 0..STEPS.len() {
            properties.push((
                format!("Step{c}_{k}"),
                object_value(own(CELL_REFS[c] + k as u32)),
            ));
        }
        properties.push((
            format!("Ramp{c}"),
            object_value(own(CELL_REFS[c] + STEPS.len() as u32)),
        ));
    }
    for i in 0..current.len() {
        properties.push((
            format!("Current{i}"),
            object_value(own(CURRENT_MARKERS + i as u32)),
        ));
    }

    let template = v.id("WEMerchantMiscMaleCommoner")?;
    let (race, class) = (v.id("ImperialRace")?, v.id("Citizen")?);
    let npc = Npc {
        object_id: COMMONER,
        edid: "VrmAt25Commoner".into(),
        name: Some("vibe_rm Test NPC"),
        level: (10, false, 1, 0),
        template: (template, 0x1),
        temper: (0, 2),
        factions: Vec::new(),
        script: None,
        assistance: 0,
        combat_style: None,
        unique: false,
    }
    .record(race, class);

    // autotest20's detector.
    let effect = |object_id: u32, edid: &str, kind: &str, actor_value: u32| {
        let script = AttachedScript {
            name: LOG_SCRIPT.into(),
            properties: vec![
                ("Owner".into(), object_value(own(QUEST))),
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
        effect(SWIM_EFFECT, "VrmAt25Swimming", "swim", 6)?,
        effect(DRY_EFFECT, "VrmAt25Dry", "dry", 7)?,
    ];
    const IS_SWIMMING: u16 = 185;
    let swimming = |value: f32| ctda(IS_SWIMMING, 0, 0, cmp::EQ, value, run_on::SUBJECT);
    let ability = records::spell(
        SWIM_ABILITY,
        "VrmAt25SwimmingAbility",
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
        edid: "VrmAt25Quest",
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

    // Our cells, each in its block and sub-block by object id.
    let mut blocks: std::collections::BTreeMap<i32, std::collections::BTreeMap<i32, Vec<Node>>> =
        Default::default();
    for c in 0..2 {
        let (record, children) = cell(&v, c)?;
        let id = CELLS[c] as i32;
        blocks
            .entry(id % 10)
            .or_default()
            .entry(id / 10 % 10)
            .or_default()
            .extend([Node::Record(record), children]);
    }
    let cells = Node::Group {
        label: *b"CELL",
        group_type: group_type::TOP,
        children: blocks
            .into_iter()
            .map(|(block, subs)| Node::Group {
                label: block.to_le_bytes(),
                group_type: group_type::INTERIOR_BLOCK,
                children: subs
                    .into_iter()
                    .map(|(sub, children)| Node::Group {
                        label: sub.to_le_bytes(),
                        group_type: group_type::INTERIOR_SUB_BLOCK,
                        children,
                    })
                    .collect(),
            })
            .collect(),
    };

    // The current's markers, floating 92 under the surface.
    let heading = v.id("XMarkerHeading")?;
    let markers = site
        .points
        .iter()
        .enumerate()
        .map(|(i, (_, [x, y], _))| {
            marker(
                CURRENT_MARKERS + i as u32,
                heading,
                [*x, *y, site.surface - 92.0],
                site.heading,
            )
        })
        .collect();
    let worlds = places::worlds(
        lo,
        vec![World {
            world: TAMRIEL,
            name: "Skyrim",
            grid: site.grid,
            persistent: markers,
            temporary: Vec::new(),
        }],
    )?;

    // Top groups in the Creation Kit's order.
    let plugin = write_plugin(
        0,
        &["Skyrim.esm"],
        "vibe_rm",
        "Swimming in authored water (a cell water level, placed water): steps, a ramp, \
         floating; and a lake's current. Run by script; reports saved.",
        NEXT_OBJECT_ID,
        &[
            Node::top(
                b"GLOB",
                vec![records::global(SKIP_TEST, "VrmAt25SkipTest", 0.0)],
            ),
            Node::top(b"MGEF", effects),
            Node::top(b"SPEL", vec![ability]),
            Node::top(b"NPC_", vec![npc]),
            cells,
            worlds,
            Node::top(b"QUST", vec![quest]),
        ],
    );
    Ok(TestMod {
        plugin,
        scripts: scripts(&CurrentPoints(&current))?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_mods::script::dry_run::{form, run};

    #[test]
    fn the_tests_run_and_report() {
        let current = [("marker", 372.0), ("512 along", 300.0)];
        let mut properties = vec![
            ("Player".to_owned(), form(0x14, "Actor")),
            ("CurrentSurface".to_owned(), papyrus::Value::Float(6000.0)),
            ("CurrentHeading".to_owned(), papyrus::Value::Float(5.7)),
        ];
        for (i, (name, ty, source)) in PROPERTIES.iter().enumerate() {
            let id = match source {
                Source::Vanilla(_) => 0x1000 + i as u32,
                Source::Own(id) => own(*id).0,
            };
            properties.push((name.to_string(), form(id, ty)));
        }
        for c in 0..2 {
            for k in 0..STEPS.len() {
                properties.push((
                    format!("Step{c}_{k}"),
                    form(own(CELL_REFS[c] + k as u32).0, "ObjectReference"),
                ));
            }
            properties.push((
                format!("Ramp{c}"),
                form(own(CELL_REFS[c] + STEPS.len() as u32).0, "ObjectReference"),
            ));
        }
        for i in 0..current.len() {
            properties.push((
                format!("Current{i}"),
                form(own(CURRENT_MARKERS + i as u32).0, "ObjectReference"),
            ));
        }
        // Only the done box.
        let shown = run(
            scripts(&CurrentPoints(&current)).unwrap(),
            &[(own(QUEST).0, SCRIPT, properties)],
            1,
        );
        assert_eq!(shown.boxes.len(), 1);
        assert_eq!(shown.saved_at_box, Some(0), "saved before the done box");
    }
}
