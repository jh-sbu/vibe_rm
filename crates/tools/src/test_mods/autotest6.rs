//! `test-mod autotest6`: where the game puts actors when the cell their
//! package sends them to loads for the first time (`spawn_actors` spreads
//! them over any navmesh within 96 units and 300 high, so an actor sent to a
//! marker under an upper floor often appears upstairs: Lucan and Camilla in
//! the Riverwood Trader).
//!
//! The plugin adds persistent actors of our own to the Riverwood Trader
//! (`RiverwoodRiverwoodTrader`, overridden as Skyrim.esm has it: ccBGSSSE001's
//! override of the record is the same), each standing upstairs in the editor
//! and running one package, always on, that sends it to the ground floor,
//! under the upper floor:
//!
//! - **Travel0**, **Travel32**, **Travel128**: Travel (a copy of
//!   `MS13LucanHoldPositionPackage`) near a marker of its own, radius 0, 32
//!   and 128.
//! - **Sandbox**: Sandbox (a copy of `CamillaWorkTrader8x10`, its schedule
//!   made always, "offers services" cleared) near a marker, radius 200.
//! - **InCell**: the same Sandbox, its location the whole cell.
//! - **SharedA**, **SharedB**, **SharedC**: Travel radius 0, all to one
//!   marker.
//!
//! As the other autotests: loading a save with the plugin for the first time
//! starts `VrmAutoTest6Quest`. Its script reads every actor while the trader
//! is unloaded (at once and 10 s later), moves the player into the trader by
//! the front door, waits for every actor's 3D, reads them at once, 1, 3 and
//! 10 s later, and puts the player back where they were. Each reading: the
//! position, heading, distance and height from the actor's marker, whether
//! its 3D is loaded and its package. The actors' own script logs `OnLoad`
//! with their position then. `vrm-tool test-results` reads the reports:
//! two readings to a page (**Unloaded**, **Loaded**, **Later**), so each
//! message box fits on the screen, then **Loads**.
//!
//! Load the usual save (outdoors near Riverwood, without the test plugins,
//! never inside the trader: the report says so if the actors were loaded
//! before the test) and leave the game alone for about a minute. Lucan and
//! Camilla may be about; the markers are on the west side, away from
//! theirs. Don't keep playing with the plugin: the actors stay in the
//! trader. Reloading the same save and running again tells whether the
//! placement is the same each time.
//!
//! What each reading at load means:
//!
//! | the actor at load               | reading                                      |
//! | ------------------------------- | -------------------------------------------- |
//! | upstairs, where the editor has it | placed at its editor spot; walks from there |
//! | on its marker (0 off, height 0) | placed at the package's place exactly        |
//! | within the radius, downstairs   | placed somewhere in the package's radius     |
//! | near the marker, upstairs       | the game spreads across floors too           |
//! | elsewhere                       | another rule (furniture, idle markers...)    |
//!
//! For the three sharing a marker, their distances from each other: about
//! 0 when stacked, more when the game spreads them (by how much), one on
//! it and the others queued when it doesn't place them at all. **InCell**:
//! its editor spot, or somewhere else in the cell. The unloaded readings
//! show whether the game moved anyone before the cell loaded (unloaded
//! actors' movement), and `OnLoad` whether the place at load differs from
//! the first reading after it.
//!
//! Run in Skyrim SE (2026-10-21, once; the reports split into pages after
//! the first build's single box ran off the screen). None of the readings
//! above: the game had moved every actor but **InCell** before the trader
//! loaded, within seconds of the save loading, and at load only dropped
//! them onto the floor beneath, keeping XY. Nobody was upstairs, nobody
//! spread out.
//!
//! - Travel (all five far from their marker, radius 0, 32 or shared): on
//!   the straight line from the editor spot to the marker, the height too
//!   (so 22 to 34 units in the air), stopped exactly 40 units short in XY.
//!   They didn't walk the last 40, and turned to the marker's heading.
//! - **Travel128**, starting 86 away in XY (inside its radius): exactly on
//!   the marker, keeping its heading.
//! - **Sandbox**: exactly on its marker with the marker's heading; it
//!   started sandboxing at once (the cooking pot within 3 s).
//! - **InCell**: at its editor spot upstairs while unloaded; 0.2 s after
//!   the player came in, on the ground floor beside a sweep idle marker.
//! - The shared marker: each 40 short on its own line, two of them 5 units
//!   apart; at load one was pushed to 62 from the other.
//! - `OnLoad` came 0.2 to 6 s after the 3D (0.64 s), and two calls'
//!   lines interleaved in the log (each call's parts appended across
//!   frames): no use for the place at load.
//!
//! The engine follows this (`unloaded_arrival` in `src/ai/schedule.rs`);
//! what it leaves open: `known_ambiguities/unloaded-placement.md`.

use anyhow::{Context, Result};
use esp::writer::{AttachedScript, Node, PropertyValue, RecordBuilder, write_plugin};
use esp::{FormId, LoadOrder, group_type, record_flags};
use papyrus::build::{Arg, FunctionBuilder, ScriptBuilder, auto_var, id, string};
use papyrus::pex::op;

use super::autotest2::{Npc, Vanilla, line, put};
use super::autotest4::{now, num_of, poll, put_num, relay, report, self_ref};
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

const SCRIPT: &str = "VrmAutoTest6Script";
const ACTOR_SCRIPT: &str = "VrmAutoTest6ActorScript";
const PLAYER_REF: FormId = FormId(0x14);
const TRADER: &str = "RiverwoodRiverwoodTrader";

/// Height of the trader's upper floor (its navmesh), where the actors stand
/// in the editor.
const UPSTAIRS: f32 = 240.0;
/// The player's spot: inside the front door, facing in.
const ENTRY_AT: [f32; 3] = [-180.0, -461.0, 0.0];

/// Where a package sends its actor (`PLDT`).
#[derive(Clone, Copy)]
enum Place {
    /// Near our marker (index into [`MARKER_AT`]), radius.
    Near(usize, u32),
    /// Anywhere in the trader.
    InCell,
}

/// A package: name, the vanilla package copied, where it sends its actor.
const PACKS: &[(&str, &str, Place)] = &[
    ("Travel0", "MS13LucanHoldPositionPackage", Place::Near(0, 0)),
    (
        "Travel32",
        "MS13LucanHoldPositionPackage",
        Place::Near(1, 32),
    ),
    (
        "Travel128",
        "MS13LucanHoldPositionPackage",
        Place::Near(2, 128),
    ),
    ("Sandbox", "CamillaWorkTrader8x10", Place::Near(3, 200)),
    ("InCell", "CamillaWorkTrader8x10", Place::InCell),
    ("Shared", "MS13LucanHoldPositionPackage", Place::Near(4, 0)),
];

/// The markers, on the ground floor with the upper floor overhead (both
/// within 96 units of each, as our navmesh has them).
const MARKER_AT: [[f32; 2]; 5] = [
    [-276.0, -333.0],
    [-148.0, -333.0],
    [-404.0, -93.0],
    [-340.0, -237.0],
    [-388.0, -317.0],
];

/// An actor: its label, its package (index into [`PACKS`]), its editor spot
/// upstairs.
const CASES: &[(&str, usize, [f32; 2])] = &[
    ("Travel0", 0, [-500.0, -13.0]),
    ("Travel32", 1, [-436.0, -13.0]),
    ("Travel128", 2, [-372.0, -13.0]),
    ("Sandbox", 3, [-308.0, -13.0]),
    ("InCell", 4, [-244.0, -13.0]),
    ("SharedA", 5, [-180.0, -13.0]),
    ("SharedB", 5, [-116.0, -13.0]),
    ("SharedC", 5, [-500.0, 19.0]),
];

const TESTS: &[(&str, &str)] = &[
    ("Unloaded", "Actors before the cell loads"),
    ("Loaded", "Actors as the cell loads"),
    ("Later", "Actors after the cell loaded"),
    ("Loads", "The actors' OnLoad"),
];

/// The marker property of case `i`'s package, if it has one.
fn marker_of(i: usize) -> Option<String> {
    match PACKS[CASES[i].1].2 {
        Place::Near(m, _) => Some(format!("Marker{m}")),
        Place::InCell => None,
    }
}

/// `Where(Actor a, ObjectReference m)`: `a`'s position, heading, distance
/// and height from `m` (when there is one), whether its 3D is loaded and its
/// package.
pub(super) fn where_fn() -> FunctionBuilder {
    let mut c = Code::new("String");
    let a = c.param("a", "Actor");
    let m = c.param("m", "ObjectReference");
    let r = c.local("r", "String");
    c.assign(r.clone(), string(""));
    let [x, y, z, angle] = ["GetPositionX", "GetPositionY", "GetPositionZ", "GetAngleZ"]
        .map(|f| c.method("ObjectReference", a.clone(), f, []));
    c.numbers(
        &r,
        [("", x), (" ", y), (" ", z.clone()), (", heading ", angle)],
    );
    let none = c.temp("ObjectReference");
    c.cast(none.clone(), Arg::None);
    let is_none = c.calc(op::CMP_EQ, "Bool", m.clone(), none);
    let skip = c.label();
    c.jmpt(is_none, skip);
    let d = c.method("ObjectReference", a.clone(), "GetDistance", [m.clone()]);
    let mz = c.method("ObjectReference", m, "GetPositionZ", []);
    let dz = c.calc(op::FSUB, "Float", z, mz);
    c.numbers(&r, [(", from its marker ", d), (" (height ", dz)]);
    line(&mut c, &r, ")");
    c.bind(skip);
    let loaded = c.method("ObjectReference", a.clone(), "Is3DLoaded", []);
    put(&mut c, &r, ", 3D ", loaded);
    let package = c.method("Actor", a, "GetCurrentPackage", []);
    put(&mut c, &r, ", package ", package);
    c.finish(r)
}

/// Every actor read, under `title`.
fn read_all(c: &mut Code, r: &Arg, title: &str) {
    line(c, r, &format!("{title}:\n"));
    for (i, (label, _, _)) in CASES.iter().enumerate() {
        let m = marker_of(i).map_or(Arg::None, |m| auto_var(&m));
        let s = c.own("Where", "String", [auto_var(label), m]);
        put(c, r, &format!("  {label}: "), s);
        line(c, r, "\n");
    }
}

/// The three sharing a marker: their distances from each other.
fn shared_distances(c: &mut Code, r: &Arg) {
    line(c, r, "  shared marker, apart:");
    for (a, b) in [
        ("SharedA", "SharedB"),
        ("SharedA", "SharedC"),
        ("SharedB", "SharedC"),
    ] {
        let d = c.method("ObjectReference", auto_var(a), "GetDistance", [auto_var(b)]);
        put_num(c, r, &format!(" {}-{} ", &a[6..], &b[6..]), d);
    }
    line(c, r, "\n");
}

/// The first page: every actor while the trader is unloaded.
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
    read_all(&mut c, &r, "unloaded, at once");
    c.wait(10.0);
    read_all(&mut c, &r, "unloaded, 10 s later");
    c.finish(r)
}

/// The second page: the player moved in by the front door (where they
/// were kept in `Back`), every actor as its 3D loads and a second later.
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
    c.assign(id("Start"), start.clone());
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
    read_all(&mut c, &r, "loaded, at once");
    shared_distances(&mut c, &r);
    c.wait(1.0);
    read_all(&mut c, &r, "1 s later");
    shared_distances(&mut c, &r);
    c.finish(r)
}

/// The third page: 3 and 10 s after loading; then the player is put back.
fn test_later() -> FunctionBuilder {
    let (mut c, r) = report();
    for (wait, title) in [(2.0, "3 s later"), (7.0, "10 s later")] {
        c.wait(wait);
        read_all(&mut c, &r, title);
        shared_distances(&mut c, &r);
    }
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

fn test_loads() -> FunctionBuilder {
    let (mut c, r) = report();
    put_num(&mut c, &r, "the player moved in at real time ", id("Start"));
    line(&mut c, &r, "\n");
    put(&mut c, &r, "", id("LoadLog"));
    c.finish(r)
}

/// `LogLoad(ObjectReference a, String label)`, from the actors' script:
/// the real time and where `a` is.
fn log_load() -> FunctionBuilder {
    let mut c = Code::new("None");
    let a = c.param("a", "ObjectReference");
    let label = c.param("label", "String");
    let log = id("LoadLog");
    let t = now(&mut c);
    let t = num_of(&mut c, t);
    c.append(
        log.clone(),
        [string("  "), label, string(" at real time "), t],
    );
    let [x, y, z, angle] = ["GetPositionX", "GetPositionY", "GetPositionZ", "GetAngleZ"]
        .map(|f| c.method("ObjectReference", a.clone(), f, []));
    c.numbers(&log, [(": ", x), (" ", y), (" ", z), (", heading ", angle)]);
    c.append(log, [string("\n")]);
    c.finish(Arg::None)
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
    s.variable("Start", "Float", 0.0f32.into());
    s.variable("Back", "ObjectReference", Arg::None);
    s.variable("LoadLog", "String", string(""));
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
        .function("OnUpdate", on_update("autotest6", "a minute", TESTS))
        .function("Num", num())
        .function("Where", where_fn())
        .function("LogLoad", log_load())
        .function("TestUnloaded", test_unloaded())
        .function("TestLoaded", test_loaded())
        .function("TestLater", test_later())
        .function("TestLoads", test_loads());

    let mut actor = ScriptBuilder::new(ACTOR_SCRIPT, "Actor");
    actor.auto_property("Main", SCRIPT);
    actor.auto_property("Label", "String");
    relay(&mut actor, "OnLoad", &[], "LogLoad", |c, _| {
        vec![self_ref(c), auto_var("Label")]
    });

    let mut out = Vec::new();
    for (name, b) in [(SCRIPT, &s), (ACTOR_SCRIPT, &actor)] {
        out.push((name.to_owned(), papyrus::pex::write(&b.build()?)?));
    }
    Ok(out)
}

/// A package's location (`PLDT`): near a reference (type 0) or in a cell
/// (type 1), and the radius.
pub(super) fn pldt(kind: u32, target: FormId, radius: u32) -> Vec<u8> {
    [kind, target.0, radius]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect()
}

/// `PSDT` of a package that always applies: any month, day, date, hour and
/// minute, no duration.
pub(super) const ALWAYS: [u8; 12] = [0xFF, 0xFF, 0, 0xFF, 0xFF, 0, 0, 0, 0, 0, 0, 0];

/// The top `CELL` group overriding interior `cell` as Skyrim.esm has it
/// (our plugin isn't localized: its name written out as the trader's), with
/// `refs` as its persistent references (the packages and scripts name them).
pub(super) fn trader_cell(lo: &LoadOrder, cell: FormId, refs: Vec<RecordBuilder>) -> Result<Node> {
    trader_cell_with(lo, cell, refs, Vec::new())
}

/// [`trader_cell`], with `temporary` references too (in the cell's
/// temporary group, as the Creation Kit writes those not flagged
/// persistent).
pub(super) fn trader_cell_with(
    lo: &LoadOrder,
    cell: FormId,
    persistent: Vec<RecordBuilder>,
    temporary: Vec<RecordBuilder>,
) -> Result<Node> {
    let rec = lo.get(cell).context("the trader's cell")?;
    let mut record =
        RecordBuilder::new(b"CELL", cell).flags(rec.flags() & !record_flags::COMPRESSED);
    for s in rec.subrecords() {
        match &s.tag.0 {
            b"FULL" => {}
            b"EDID" => {
                record = record
                    .field(b"EDID", s.data)
                    .zstring(b"FULL", "Riverwood Trader")
            }
            tag => record = record.field(tag, s.data),
        }
    }
    let label = cell.0.to_le_bytes();
    let mut groups = vec![Node::Group {
        label,
        group_type: group_type::CELL_PERSISTENT,
        children: persistent.into_iter().map(Node::Record).collect(),
    }];
    if !temporary.is_empty() {
        groups.push(Node::Group {
            label,
            group_type: group_type::CELL_TEMPORARY,
            children: temporary.into_iter().map(Node::Record).collect(),
        });
    }
    let children = Node::Group {
        label,
        group_type: group_type::CELL_CHILDREN,
        children: groups,
    };
    // Interior cells are grouped by the last two decimal digits of their
    // object id: block, then sub-block.
    let object_id = cell.0 & 0xFF_FFFF;
    Ok(Node::Group {
        label: *b"CELL",
        group_type: group_type::TOP,
        children: vec![Node::Group {
            label: ((object_id % 10) as i32).to_le_bytes(),
            group_type: group_type::INTERIOR_BLOCK,
            children: vec![Node::Group {
                label: ((object_id / 10 % 10) as i32).to_le_bytes(),
                group_type: group_type::INTERIOR_SUB_BLOCK,
                children: vec![Node::Record(record), children],
            }],
        }],
    })
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
        .map(|(i, &(name, from, place))| {
            let location = match place {
                Place::Near(m, radius) => pldt(0, own(MARKERS + m as u32), radius),
                Place::InCell => pldt(1, trader, 0),
            };
            records::copy(
                lo,
                from,
                PACKAGES + i as u32,
                &format!("VrmAutoTest6{name}"),
                None,
                |tag, d| match tag {
                    b"PLDT" => *d = location.clone(),
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
                edid: format!("VrmAutoTest6{name}Npc"),
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
            edid: "VrmAutoTest6Quest",
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
    for (m, [x, y]) in MARKER_AT.iter().enumerate() {
        refs.push(
            RecordBuilder::new(b"REFR", own(MARKERS + m as u32))
                .flags(persistent)
                .form(b"NAME", heading_marker)
                .field(b"DATA", &placement([*x, *y, 0.0], 0.0)),
        );
    }
    for (i, &(label, pack, [x, y])) in CASES.iter().enumerate() {
        let script = AttachedScript {
            name: ACTOR_SCRIPT.into(),
            properties: vec![
                ("Main".into(), object(own(QUEST))),
                ("Label".into(), PropertyValue::String(label.into())),
            ],
        };
        refs.push(
            RecordBuilder::new(b"ACHR", own(ACTORS + i as u32))
                .flags(persistent)
                .field(b"VMAD", &esp::writer::vmad(&[script]))
                .form(b"NAME", own(NPCS + pack as u32))
                .field(b"DATA", &placement([x, y, UPSTAIRS], std::f32::consts::PI)),
        );
    }
    let cells = trader_cell(lo, trader, refs)?;

    // Top groups in the Creation Kit's order.
    let plugin = write_plugin(
        0,
        &["Skyrim.esm"],
        "vibe_rm",
        "Where actors are placed as their package's cell loads: test actors in the Riverwood \
         Trader, read by script on loading a save, reports saved.",
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
            "unloaded, at once:\n  Travel0: 1 1 1, heading 1, from its marker 1 (height 0), 3D False",
            "  InCell: 1 1 1, heading 1, 3D False, package ",
            "every actor's 3D after -1 s",
            "10 s later:\n  Travel0: ",
            "  shared marker, apart: A-B 1 A-C 1 B-C 1\n",
            "the player moved in at real time ",
        ] {
            assert!(all.contains(expected), "{expected:?} not in the reports");
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
