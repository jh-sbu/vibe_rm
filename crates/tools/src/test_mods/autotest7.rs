//! `test-mod autotest7`: `autotest6`'s open ends
//! (`known_ambiguities/unloaded-placement.md`). The same setup: persistent
//! actors of our own in the Riverwood Trader (overridden as Skyrim.esm has
//! it), each with one package, always on, read while the trader is unloaded
//! and as it loads. Here they start upstairs or on the ground floor at set
//! distances from their markers:
//!
//! | actor           | package          | starts                          | settles                            |
//! | --------------- | ---------------- | ------------------------------- | ---------------------------------- |
//! | **Inside128Up** | Travel, r 128    | upstairs, 86 in XY (256 in 3D)  | autotest6's one reading, again     |
//! | **Xy300Up**     | Travel, r 300    | upstairs, 200 in XY (313 in 3D) | the radius in XY or in 3D          |
//! | **Out128**      | Travel, r 128    | ground floor, 200 off           | 40 short, or stopped at the radius |
//! | **Near0**       | Travel, r 0      | ground floor, 25 off            | within 40: stays, or on the target |
//! | **Inside32**    | Travel, r 32     | ground floor, 30 off            | inside the radius but within 40    |
//! | **StackedA/B**  | Travel, r 0      | upstairs, 250 and 330 off, in line | both 40 short at one point: how far apart at load |
//! | **InCellUp/Down** | Sandbox, the cell | upstairs / ground floor        | where a whole-cell sandbox appears |
//! | **Sit**         | Sit (a copy of `MS13CamillaWorkTrader`, the chair by the counter) | upstairs | another procedure: in the chair while unloaded? |
//!
//! The readings: as the quest starts (`OnInit`, the save just loaded), 5 s
//! later, as every actor's 3D loads after the player is moved in by the
//! front door, 3 and 10 s after that; then the player is put back. Each:
//! position, heading, distance and height from the actor's marker (the
//! **Sit** chair, `0005AC1B`, is at -348 -91 0; the cell sandboxes have
//! none), whether its 3D is loaded and its package. One page a reading
//! (twelve actors don't fit two to a box), then `vrm-tool test-results`.
//!
//! Load the usual save (outdoors near Riverwood, without the test plugins;
//! not inside the trader) and leave the game alone for about a minute.
//! Don't keep playing with the plugin: the actors stay in the trader.
//!
//! Run in Skyrim SE (2026-10-21, once). `OnInit` ran twice, as it does for
//! a start-game-enabled quest added to a save: the first reading lost its
//! title (the variables were set again under it) and came before the
//! second (real time 275.6). Readings at the trader's load (288.4) and
//! after, all on the ground floor:
//!
//! - **Out128**: on its line, stopped 118 short in XY: 10 inside the
//!   radius, not 40 short.
//! - **Near0** (25 off) and **Inside32** (30 off): not moved while
//!   unloaded; after loading they walked a little nearer (16 and 6 off).
//! - **Inside128Up**: at its editor spot, then exactly on its marker (as in
//!   autotest6).
//! - **Xy300Up** (upstairs, 200 off in XY, radius 300): not moved while
//!   unloaded; as the cell loaded, on the ground floor 164 from its marker.
//! - **StackedA/B**: the first reading caught both on their lines 112.13
//!   along (moving at one speed); then B 40 short, A 30 (10 further on).
//!   As they loaded A was set 38.25 from B (autotest6's separation too).
//! - **InCellUp/Down**: where they stood while unloaded; as the cell loaded,
//!   at a sweep idle marker and at the cooking pot.
//! - **Sit**: on its line, stopped 80 short of the chair in XY (mid-air);
//!   in the chair, with its heading, as the cell loaded.
//!
//! What it leaves open: `known_ambiguities/unloaded-placement.md`.

use anyhow::Result;
use esp::writer::Node;
use esp::writer::{AttachedScript, PropertyValue, RecordBuilder, write_plugin};
use esp::{FormId, LoadOrder, record_flags};
use papyrus::build::{Arg, FunctionBuilder, ScriptBuilder, auto_var, id, string};
use papyrus::pex::op;

use super::autotest2::{Npc, Vanilla, line, put};
use super::autotest4::{now, poll, put_num, report};
use super::autotest6::{ALWAYS, pldt, trader_cell, where_fn};
use super::records::{self, Quest};
use super::script::{Code, num, on_update};
use super::venv::placement;
use super::{TestMod, own};

// Object ids: the quest, the NPCs, their packages, the markers, then the
// actors.
const QUEST: u32 = 0x800;
const NPCS: u32 = 0x810;
const PACKAGES: u32 = 0x820;
const MARKERS: u32 = 0x830;
const ENTRY: u32 = 0x83F;
const ACTORS: u32 = 0x840;
const NEXT_OBJECT_ID: u32 = 0x850;

const SCRIPT: &str = "VrmAutoTest7Script";
const PLAYER_REF: FormId = FormId(0x14);
const TRADER: &str = "RiverwoodRiverwoodTrader";
/// The player's spot: inside the front door, facing in.
const ENTRY_AT: [f32; 3] = [-180.0, -461.0, 0.0];

/// What a package does and where.
#[derive(Clone, Copy)]
enum Goal {
    /// Travel near our marker (index into [`MARKER_AT`]), radius.
    Travel(usize, u32),
    /// Sandbox anywhere in the trader.
    InCell,
    /// Sit in the chair the copied package names.
    Sit,
}

/// A package: name, the vanilla package copied, what it does.
const PACKS: &[(&str, &str, Goal)] = &[
    (
        "Inside128Up",
        "MS13LucanHoldPositionPackage",
        Goal::Travel(0, 128),
    ),
    (
        "Xy300Up",
        "MS13LucanHoldPositionPackage",
        Goal::Travel(1, 300),
    ),
    (
        "Out128",
        "MS13LucanHoldPositionPackage",
        Goal::Travel(2, 128),
    ),
    ("Near0", "MS13LucanHoldPositionPackage", Goal::Travel(3, 0)),
    (
        "Inside32",
        "MS13LucanHoldPositionPackage",
        Goal::Travel(4, 32),
    ),
    (
        "Stacked",
        "MS13LucanHoldPositionPackage",
        Goal::Travel(5, 0),
    ),
    ("InCell", "CamillaWorkTrader8x10", Goal::InCell),
    ("Sit", "MS13CamillaWorkTrader", Goal::Sit),
];

/// The markers on the navmesh (the third on a step 11 up).
const MARKER_AT: [[f32; 3]; 6] = [
    [-404.0, -93.0, 0.0],
    [-276.0, -333.0, 0.0],
    [-564.0, -333.0, 11.0],
    [-500.0, -397.0, 0.0],
    [-436.0, -429.0, 0.0],
    [-388.0, -317.0, 0.0],
];

/// An actor: its label, its package (index into [`PACKS`]), where it stands
/// in the editor (upstairs at 240).
const CASES: &[(&str, usize, [f32; 3])] = &[
    ("Inside128Up", 0, [-372.0, -13.0, 240.0]),
    ("Xy300Up", 1, [-340.0, -143.0, 240.0]),
    ("Out128", 2, [-364.0, -333.0, 0.0]),
    ("Near0", 3, [-475.0, -397.0, 0.0]),
    ("Inside32", 4, [-406.0, -429.0, 0.0]),
    ("StackedA", 5, [-388.0, -67.0, 240.0]),
    ("StackedB", 5, [-388.0, 13.0, 240.0]),
    ("InCellUp", 6, [-244.0, -13.0, 241.0]),
    ("InCellDown", 6, [100.0, -40.0, 0.0]),
    ("Sit", 7, [-500.0, -13.0, 240.0]),
];

const TESTS: &[(&str, &str)] = &[
    ("Start", "Actors as the quest starts"),
    ("Unloaded", "Actors 5 s later, unloaded"),
    ("Loaded", "Actors as the cell loads"),
    ("Later3", "Actors 3 s after loading"),
    ("Later10", "Actors 10 s after loading"),
];

/// The marker property of case `i`'s package, if it has one.
fn marker_of(i: usize) -> Option<String> {
    match PACKS[CASES[i].1].2 {
        Goal::Travel(m, _) => Some(format!("Marker{m}")),
        Goal::InCell | Goal::Sit => None,
    }
}

/// Every actor read into `r`, under `title` and the real time.
fn read_all(c: &mut Code, r: &Arg, title: &str) {
    let t = now(c);
    put_num(c, r, &format!("{title}, real time "), t);
    line(c, r, ":\n");
    for (i, (label, _, _)) in CASES.iter().enumerate() {
        let m = marker_of(i).map_or(Arg::None, |m| auto_var(&m));
        let s = c.own("Where", "String", [auto_var(label), m]);
        put(c, r, &format!("  {label}: "), s);
        line(c, r, "\n");
    }
}

/// The two stopping at one point: how far apart.
fn stacked_apart(c: &mut Code, r: &Arg) {
    let d = c.method(
        "ObjectReference",
        auto_var("StackedA"),
        "GetDistance",
        [auto_var("StackedB")],
    );
    put_num(c, r, "  StackedA-StackedB apart ", d);
    line(c, r, "\n");
}

fn test_start() -> FunctionBuilder {
    let (mut c, r) = report();
    put(&mut c, &r, "", id("StartLog"));
    c.finish(r)
}

fn test_unloaded() -> FunctionBuilder {
    let (mut c, r) = report();
    c.controls(false, true, true);
    let cell = c.method("ObjectReference", auto_var("Player"), "GetParentCell", []);
    let inside = c.calc(op::CMP_EQ, "Bool", cell, auto_var("TraderCell"));
    let outside = c.label();
    c.jmpf(inside, outside);
    line(
        &mut c,
        &r,
        "INVALID: the player was in the trader when the test began (the actors had loaded)\n",
    );
    c.bind(outside);
    read_all(&mut c, &r, "unloaded");
    c.finish(r)
}

/// The player moved in by the front door (where they were kept in
/// `Back`), every actor as its 3D loads.
fn test_loaded() -> FunctionBuilder {
    let (mut c, r) = report();
    let player = auto_var("Player");
    let back = c.method(
        "ObjectReference",
        player.clone(),
        "PlaceAtMe",
        [auto_var("XMarker"), 1.into(), false.into(), false.into()],
    );
    c.assign(id("Back"), back);
    let start = now(&mut c);
    c.method(
        "ObjectReference",
        player,
        "MoveTo",
        [
            auto_var("Entry"),
            0.0f32.into(),
            0.0f32.into(),
            0.0f32.into(),
            true.into(),
        ],
    );
    let waited = poll(&mut c, "loaded", 0.1, 30.0, |c, hit| {
        let not_yet = c.label();
        for (label, _, _) in CASES {
            let l = c.method("ObjectReference", auto_var(label), "Is3DLoaded", []);
            c.jmpf(l, not_yet);
        }
        c.jmp(hit);
        c.bind(not_yet);
    });
    put_num(&mut c, &r, "moved in at real time ", start);
    put_num(&mut c, &r, "; every actor's 3D after ", waited);
    line(&mut c, &r, " s (-1: not in 30 s)\n");
    read_all(&mut c, &r, "loaded");
    stacked_apart(&mut c, &r);
    c.finish(r)
}

fn test_later3() -> FunctionBuilder {
    let (mut c, r) = report();
    c.wait(3.0);
    read_all(&mut c, &r, "3 s later");
    stacked_apart(&mut c, &r);
    c.finish(r)
}

/// 10 s after loading; then the player is put back.
fn test_later10() -> FunctionBuilder {
    let (mut c, r) = report();
    c.wait(7.0);
    read_all(&mut c, &r, "10 s later");
    stacked_apart(&mut c, &r);
    let back = id("Back");
    c.method(
        "ObjectReference",
        auto_var("Player"),
        "MoveTo",
        [
            back.clone(),
            0.0f32.into(),
            0.0f32.into(),
            0.0f32.into(),
            true.into(),
        ],
    );
    c.wait(3.0);
    c.method("ObjectReference", back.clone(), "Disable", [false.into()]);
    c.method("ObjectReference", back, "Delete", []);
    c.finish(r)
}

fn scripts() -> Result<Vec<(String, Vec<u8>)>> {
    let mut s = ScriptBuilder::new(SCRIPT, "Quest");
    s.auto_property("Player", "Actor");
    s.auto_property("TraderCell", "Cell");
    s.auto_property("XMarker", "Form");
    s.auto_property("Entry", "ObjectReference");
    for m in 0..MARKER_AT.len() {
        s.auto_property(&format!("Marker{m}"), "ObjectReference");
    }
    for (label, _, _) in CASES {
        s.auto_property(label, "Actor");
    }
    s.variable("Ran", "Bool", false.into());
    s.variable("Back", "ObjectReference", Arg::None);
    s.variable("StartLog", "String", string(""));
    for (name, _) in TESTS {
        s.variable(&format!("Result{name}"), "String", string(""));
    }
    // The first reading as the quest starts, then the rest.
    let mut init = Code::new("None");
    read_all(&mut init, &id("StartLog"), "as the quest starts");
    init.method(
        "Form",
        id("self"),
        "RegisterForSingleUpdate",
        [5.0f32.into()],
    );
    s.function("OnInit", init.finish(Arg::None))
        .function("OnUpdate", on_update("autotest7", "a minute", TESTS))
        .function("Num", num())
        .function("Where", where_fn())
        .function("TestStart", test_start())
        .function("TestUnloaded", test_unloaded())
        .function("TestLoaded", test_loaded())
        .function("TestLater3", test_later3())
        .function("TestLater10", test_later10());
    Ok(vec![(SCRIPT.to_owned(), papyrus::pex::write(&s.build()?)?)])
}

pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    let v = Vanilla { lo };
    let template = v.id("WEMerchantMiscMaleCommoner")?;
    let race = v.id("ImperialRace")?;
    let class = v.id("Citizen")?;
    let heading_marker = v.id("XMarkerHeading")?;
    let trader = v.id(TRADER)?;
    let persistent = record_flags::PERSISTENT;

    let object = PropertyValue::Object;
    let mut properties = vec![
        ("Player".to_owned(), object(PLAYER_REF)),
        ("TraderCell".to_owned(), object(trader)),
        ("XMarker".to_owned(), object(v.id("XMarker")?)),
        ("Entry".to_owned(), object(own(ENTRY))),
    ];
    for m in 0..MARKER_AT.len() {
        properties.push((format!("Marker{m}"), object(own(MARKERS + m as u32))));
    }
    for (i, (label, _, _)) in CASES.iter().enumerate() {
        properties.push((label.to_string(), object(own(ACTORS + i as u32))));
    }

    let packages = PACKS
        .iter()
        .enumerate()
        .map(|(i, &(name, from, goal))| {
            let location = match goal {
                Goal::Travel(m, radius) => Some(pldt(0, own(MARKERS + m as u32), radius)),
                Goal::InCell => Some(pldt(1, trader, 0)),
                Goal::Sit => None,
            };
            records::copy(
                lo,
                from,
                PACKAGES + i as u32,
                &format!("VrmAutoTest7{name}"),
                None,
                |tag, d| match tag {
                    b"PLDT" => {
                        if let Some(l) = &location {
                            *d = l.clone();
                        }
                    }
                    b"PSDT" => *d = ALWAYS.to_vec(),
                    // Flag 0x1: offers services.
                    b"PKDT" if d.len() >= 4 => d[0] &= !0x1,
                    _ => {}
                },
            )
        })
        .collect::<Result<Vec<_>>>()?;

    let npcs: Vec<RecordBuilder> = PACKS
        .iter()
        .enumerate()
        .map(|(i, (name, _, _))| {
            Npc {
                object_id: NPCS + i as u32,
                edid: format!("VrmAutoTest7{name}Npc"),
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
            .record_with_packages(race, class, &[own(PACKAGES + i as u32)])
        })
        .collect();

    let quests = vec![
        Quest {
            object_id: QUEST,
            edid: "VrmAutoTest7Quest",
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

    let mut refs = vec![
        RecordBuilder::new(b"REFR", own(ENTRY))
            .flags(persistent)
            .form(b"NAME", heading_marker)
            .field(b"DATA", &placement(ENTRY_AT, 0.0)),
    ];
    for (m, at) in MARKER_AT.iter().enumerate() {
        refs.push(
            RecordBuilder::new(b"REFR", own(MARKERS + m as u32))
                .flags(persistent)
                .form(b"NAME", heading_marker)
                .field(b"DATA", &placement(*at, 0.0)),
        );
    }
    for (i, &(_, pack, at)) in CASES.iter().enumerate() {
        refs.push(
            RecordBuilder::new(b"ACHR", own(ACTORS + i as u32))
                .flags(persistent)
                .form(b"NAME", own(NPCS + pack as u32))
                .field(b"DATA", &placement(at, std::f32::consts::PI)),
        );
    }
    let cells = trader_cell(lo, trader, refs)?;

    // Top groups in the Creation Kit's order.
    let plugin = write_plugin(
        0,
        &["Skyrim.esm"],
        "vibe_rm",
        "autotest6's open ends: where test actors in the Riverwood Trader are placed by their \
         packages before and as the cell loads, read by script on loading a save, reports saved.",
        NEXT_OBJECT_ID,
        &[
            Node::top(b"NPC_", npcs),
            cells,
            Node::top(b"QUST", quests),
            Node::top(b"PACK", packages),
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
            ("TraderCell".to_owned(), form(0x133C9, "Cell")),
            ("XMarker".to_owned(), form(0x3B, "Form")),
            ("Entry".to_owned(), form(own(ENTRY).0, "ObjectReference")),
        ];
        for m in 0..MARKER_AT.len() {
            properties.push((
                format!("Marker{m}"),
                form(own(MARKERS + m as u32).0, "ObjectReference"),
            ));
        }
        for (i, (label, _, _)) in CASES.iter().enumerate() {
            properties.push((label.to_string(), form(own(ACTORS + i as u32).0, "Actor")));
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
            "as the quest starts, real time 0:\n  Inside128Up: 1 1 1, heading 1, from its marker 1",
            "unloaded, real time ",
            "  InCellUp: 1 1 1, heading 1, 3D False, package ",
            "every actor's 3D after -1 s",
            "  StackedA-StackedB apart 1\n",
            "10 s later, real time ",
        ] {
            assert!(all.contains(expected), "{expected:?} not in the reports");
        }
        // Each box fits on the screen.
        for b in &shown.boxes {
            assert!(b.lines().count() <= 20, "{} lines:\n{b}", b.lines().count());
        }
    }

    #[test]
    fn markers_and_actors_fit_their_object_ids() {
        assert!(MARKERS + MARKER_AT.len() as u32 <= ENTRY);
        assert!(NPCS + PACKS.len() as u32 <= PACKAGES);
        assert!(PACKAGES + PACKS.len() as u32 <= MARKERS);
        assert!(ACTORS + CASES.len() as u32 <= NEXT_OBJECT_ID);
    }
}
