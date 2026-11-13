//! `test-mod autotest26`: a lake's current, autotest25's current tests
//! again (`research/autotest25.md`: there `Player.MoveTo` to our markers in
//! Tamriel, made from one of our interior cells, did nothing, so both read
//! the player still indoors).
//!
//! [`places::current_site`] picks the water current marker with the
//! deepest water under it and no actor or critter spawner within 4096:
//! Update.esm's `01002B8A` in Tamriel (-23, -12), a mountain lake 372 deep
//! at surface 6000 with a waterfall, heading 5.7°. Markers at its spot and
//! 512 and 1024 along its heading each way and 512 across, 92 under the
//! surface (all seven over water 180 to 372 deep), and one on the nearest
//! dry land (a terrain vertex 32 or more above the surface). Build output
//! prints them.
//!
//! 1. **CurrentPlayer**: the player moved to the marker's spot. The report
//!    gives the target's and the player's worldspace, cell and position
//!    before and after. If the player isn't there, it goes by the dry-land
//!    marker and tries again. Hostiles within 6000 are disabled (enabled
//!    again at the end). Then the player is left at each point for 10 s,
//!    read every second: the drift along the marker's heading and across it
//!    (right positive), and the feet against the surface.
//! 2. **CurrentNpc**: the same with an NPC, the player waiting 300 off the
//!    marker.
//!
//! | reading | means |
//! | ------- | ----- |
//! | drift along the heading, growing steadily | a current along the marker's heading: its speed read off |
//! | drift against it, or across | the heading isn't the direction |
//! | drift falling off away from the marker | its reach |
//! | none anywhere | no current at this marker (or not on still swimmers) |
//!
//! Ours has no currents: none, everywhere. Its dry run (2026-11-13)
//! reached the marker at the first `MoveTo`; it reads exterior references'
//! `GetParentCell` as none, where SE gives the exterior cell (to look into).
//!
//! Run: a throwaway save, outdoors or in, Survival Mode off (the lake is
//! cold). Wait for the "running" notification and leave the game alone
//! until the "done" box (2 tests, about 3 minutes). After a crash, load the
//! newest save, open the console at once and `set VrmAt26SkipTest to <n>`
//! to step past test n (see the module doc of `test_mods`). Read with
//! `vrm-tool test-results`.

use anyhow::Result;
use esp::writer::{AttachedScript, Node, PropertyValue, RecordBuilder, write_plugin};
use esp::{FormId, LoadOrder, record_flags};

use super::autotest2::{Npc, Vanilla};
use super::places::{self, World};
use super::records::{self, Quest};
use super::venv::placement;
use super::{TestMod, own, script};

const QUEST: u32 = 0x800;
const COMMONER: u32 = 0x801;
const SKIP_TEST: u32 = 0x802;
const SHORE: u32 = 0x803;
const CURRENT_MARKERS: u32 = 0x810;
const NEXT_OBJECT_ID: u32 = 0x820;

const SCRIPT: &str = "VrmAt26Script";
const SOURCE: &str = include_str!("autotest26/autotest26.psc");
const PLAYER_REF: FormId = FormId(0x14);
const TAMRIEL: FormId = FormId(0x3C);

/// The main script's properties of forms: name, type, our own form or a
/// Skyrim.esm editor id.
const PROPERTIES: &[(&str, &str, Source)] = &[
    ("XMarker", "Form", Source::Vanilla("XMarker")),
    ("FishRace", "Race", Source::Vanilla("SlaughterfishRace")),
    ("SwimmerBase", "ActorBase", Source::Own(COMMONER)),
    ("SkipTest", "GlobalVariable", Source::Own(SKIP_TEST)),
    ("Shore", "ObjectReference", Source::Own(SHORE)),
];

enum Source {
    Vanilla(&'static str),
    Own(u32),
}

/// The generated header: properties, the points as arrays, the reports.
fn header(points: &[(&str, f32)]) -> String {
    let mut s = format!("Scriptname {SCRIPT} extends Quest\n");
    s += "Actor Property Player Auto\n";
    s += "Float Property CurrentSurface Auto\nFloat Property CurrentHeading Auto\n";
    for (name, ty, _) in PROPERTIES {
        s += &format!("{ty} Property {name} Auto\n");
    }
    for i in 0..points.len() {
        s += &format!("ObjectReference Property Current{i} Auto\n");
    }
    s += &format!("Int CurrentCount = {}\n", points.len());
    s += "ObjectReference[] Currents\nString[] CurrentLabels\nFloat[] CurrentDepths\n";
    s += "String ResultCurrentPlayer\nString ResultCurrentNpc\n";
    let n = points.len().max(1);
    s += "\nFunction FillMarkers()\n";
    s += &format!("    Currents = new ObjectReference[{n}]\n");
    s += &format!("    CurrentLabels = new String[{n}]\n    CurrentDepths = new Float[{n}]\n");
    for (i, (label, depth)) in points.iter().enumerate() {
        s += &format!("    Currents[{i}] = Current{i}\n");
        s +=
            &format!("    CurrentLabels[{i}] = \"{label}\"\n    CurrentDepths[{i}] = {depth:.1}\n");
    }
    s += "EndFunction\n\n";
    s += "ObjectReference Function Current(Int i)\n    return Currents[i]\nEndFunction\n\n";
    s += "String Function CurrentLabel(Int i)\n    return CurrentLabels[i]\nEndFunction\n\n";
    s += "Float Function CurrentDepth(Int i)\n    return CurrentDepths[i]\nEndFunction\n\n";
    s
}

fn scripts(points: &[(&str, f32)]) -> Result<Vec<(String, Vec<u8>)>> {
    let main = header(points) + SOURCE;
    script::compile(&[&main])
}

fn marker(object_id: u32, base: FormId, pos: [f32; 3], heading: f32) -> RecordBuilder {
    RecordBuilder::new(b"REFR", own(object_id))
        .flags(record_flags::PERSISTENT)
        .form(b"NAME", base)
        .field(b"DATA", &placement(pos, heading))
}

pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    let v = Vanilla { lo };
    let site = places::current_site(lo, TAMRIEL, 150.0)?;
    eprintln!(
        "autotest26: current marker {} in {:?}, surface {}, heading {:.1}°, shore {:?}, points {:?}",
        site.marker,
        site.grid,
        site.surface,
        site.heading.to_degrees(),
        site.shore,
        site.points
    );
    let points: Vec<(&str, f32)> = site.points.iter().map(|p| (p.0, p.2)).collect();

    let object = PropertyValue::Object;
    let mut properties = vec![
        ("Player".to_owned(), object(PLAYER_REF)),
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
        properties.push((name.to_string(), object(id)));
    }
    for i in 0..points.len() {
        properties.push((
            format!("Current{i}"),
            object(own(CURRENT_MARKERS + i as u32)),
        ));
    }

    let template = v.id("WEMerchantMiscMaleCommoner")?;
    let (race, class) = (v.id("ImperialRace")?, v.id("Citizen")?);
    let npc = Npc {
        object_id: COMMONER,
        edid: "VrmAt26Commoner".into(),
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

    let quest = Quest {
        object_id: QUEST,
        edid: "VrmAt26Quest",
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

    let heading = v.id("XMarkerHeading")?;
    let mut markers: Vec<RecordBuilder> = site
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
    markers.push(marker(SHORE, heading, site.shore, site.heading));
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
        "A lake's current on a still player and NPC. Run by script; reports saved.",
        NEXT_OBJECT_ID,
        &[
            Node::top(
                b"GLOB",
                vec![records::global(SKIP_TEST, "VrmAt26SkipTest", 0.0)],
            ),
            Node::top(b"NPC_", vec![npc]),
            worlds,
            Node::top(b"QUST", vec![quest]),
        ],
    );
    Ok(TestMod {
        plugin,
        scripts: scripts(&points)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_mods::script::dry_run::{form, run};

    #[test]
    fn the_tests_run_and_report() {
        let points = [("marker", 372.0), ("512 along", 300.0)];
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
        for i in 0..points.len() {
            properties.push((
                format!("Current{i}"),
                form(own(CURRENT_MARKERS + i as u32).0, "ObjectReference"),
            ));
        }
        // Only the done box.
        let shown = run(
            scripts(&points).unwrap(),
            &[(own(QUEST).0, SCRIPT, properties)],
            1,
        );
        assert_eq!(shown.boxes.len(), 1);
        assert_eq!(shown.saved_at_box, Some(0), "saved before the done box");
    }
}
