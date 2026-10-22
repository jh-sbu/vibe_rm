//! `test-mod autotest8`: which actors the game moves by their packages
//! while their cell is unloaded. The engine schedules only references
//! flagged persistent (`refresh_whereabouts_step` in `src/ai/schedule.rs`),
//! but Skyrim.esm has 394 unique actors' references not flagged so, among
//! them Suvaris Atheron (`SuvarisAtheronREF`), whom `WindhelmBullyScene`
//! needs at the gate while her reference is placed in her house. What else
//! keeps a reference in the game while its cell is unloaded isn't
//! documented: a unique base, a quest alias holding it, or neither.
//!
//! autotest6's setup: actors of our own in the Riverwood Trader (overridden
//! as Skyrim.esm has it), standing upstairs in the editor, each with one
//! Travel package (a copy of `MS13LucanHoldPositionPackage`, radius 0,
//! always on) to a marker of its own on the ground floor. autotest6 found
//! persistent ones moved within seconds of the save loading, 40 short of the
//! marker on the straight line, before the trader loaded. The actors:
//!
//! | actor           | flagged persistent | unique base | held by an alias of the quest   |
//! | --------------- | ------------------ | ----------- | ------------------------------- |
//! | **Persistent**  | yes                | no          | no (autotest6's case: the control) |
//! | **Temporary**   | no                 | no          | no                              |
//! | **Unique**      | no                 | yes         | no                              |
//! | **UniqueAlias** | no                 | yes         | alias 0, filled by unique actor |
//! | **ForcedAlias** | no                 | no          | alias 1, a forced reference     |
//!
//! Both aliases are optional, so the quest starts whether they fill or not.
//! No script property names the actors (a property naming a reference may
//! itself keep it): the script finds each with `Game.GetFormFromFile` as it
//! reads it, and lets go after. **Unique** isn't in any location's unique
//! actors (`ACUN`), as vanilla ones are, so a unique actor that doesn't move
//! leaves the location lists untested.
//!
//! The readings, all from `OnUpdate`: whether each alias filled; every actor
//! 5 s after the save loads (the trader unloaded) and 10 s later; then the
//! player is moved in by the front door, every actor read as all their 3D
//! has loaded, 1, 3 and 10 s later; then the player is put back. Each:
//! position, heading, distance and height from its marker, whether its 3D
//! is loaded and its package, or "not in memory" when the game has no such
//! reference. One page per test, then `vrm-tool test-results`.
//!
//! | reading                                          | means                                               |
//! | ------------------------------------------------ | --------------------------------------------------- |
//! | unloaded: on the line, 40 short of its marker     | kept and moved while unloaded, as a persistent one |
//! | unloaded: at its editor spot (upstairs, 240 up)   | kept, but not moved while unloaded                 |
//! | unloaded: not in memory; at load, upstairs        | not kept: made as the cell loads, at its editor spot |
//! | an alias empty                                    | the fill needs the reference loaded (or persistent) |
//!
//! The editor spots and markers (ground floor, z 0), in the order above:
//! (-500, -13) to (-276, -333); (-436, -13) to (-148, -333); (-308, -13) to
//! (-340, -237); (-180, -13) to (-388, -317); (-116, -13) to (-404, -93).
//!
//! The plugin must be named `VrmAutoTest8.esp` (the script looks its actors
//! up by that name). Load the usual save (outdoors near Riverwood, without
//! the other test plugins: autotest6 and autotest7 override the trader too;
//! never inside the trader: the report says so if the actors had loaded)
//! and leave the game alone for about a minute. Don't keep playing with the
//! plugin: the actors stay in the trader.
//!
//! Run in Skyrim SE (2026-10-22, once). None of the actors was "not in
//! memory": at the first reading (46 s after loading) the game had moved
//! all five while the trader was unloaded, each on the straight line from
//! its editor spot to its marker, the height too, as autotest6's persistent
//! ones. At load they dropped onto the floor beneath, keeping XY, and
//! walked a little nearer.
//!
//! - **Persistent**, **Temporary** and **UniqueAlias**: 40 short of the
//!   marker in XY.
//! - **Unique** and **ForcedAlias**: 30 short in XY (as autotest7's
//!   StackedA, which was taken to be making room for StackedB). Nothing
//!   here shares a marker, and the lines are 226 and 299 long against
//!   368 to 431 for the others: what sets 30 or 40 isn't known.
//! - **UniqueAlias**'s alias stayed empty throughout (the unique actor
//!   fill didn't take the non-persistent reference, though the reference
//!   was in memory and moving); **ForcedAlias**'s held its reference.
//!
//! So persistence doesn't decide which actors move while unloaded: the
//! engine schedules every actor reference since
//! (`refresh_whereabouts_step`).

use anyhow::Result;
use esp::writer::{AttachedScript, Node, PropertyValue, RecordBuilder, write_plugin};
use esp::{FormId, LoadOrder, record_flags};
use papyrus::build::{Arg, FunctionBuilder, ScriptBuilder, auto_var, id, string};
use papyrus::pex::op;

use super::autotest2::{Npc, Vanilla, line, put};
use super::autotest4::{now, poll, put_num, report};
use super::autotest6::{ALWAYS, pldt, trader_cell_with, where_fn};
use super::records::{self, Alias, Fill, Quest, alias_flags};
use super::script::{Code, num, on_update};
use super::venv::placement;
use super::{TestMod, own};

// Object ids: the quest, the NPCs, their packages, the markers, then the
// actors (one of each per case).
const QUEST: u32 = 0x800;
const NPCS: u32 = 0x810;
const PACKAGES: u32 = 0x820;
const MARKERS: u32 = 0x830;
const ENTRY: u32 = 0x83F;
const ACTORS: u32 = 0x840;
const NEXT_OBJECT_ID: u32 = 0x850;

const PLUGIN: &str = "VrmAutoTest8.esp";
const SCRIPT: &str = "VrmAutoTest8Script";
const PLAYER_REF: FormId = FormId(0x14);
const TRADER: &str = "RiverwoodRiverwoodTrader";
/// Height of the trader's upper floor, where the actors stand in the editor.
const UPSTAIRS: f32 = 240.0;
/// The player's spot: inside the front door, facing in.
const ENTRY_AT: [f32; 3] = [-180.0, -461.0, 0.0];

/// How an alias of the quest holds a case's actor.
#[derive(Clone, Copy, PartialEq)]
enum Held {
    No,
    /// Filled by unique actor (`ALUA`, the NPC).
    UniqueActor,
    /// A forced reference (`ALFR`).
    Forced,
}

/// An actor: its label, whether flagged persistent, whether its base is
/// unique, how an alias holds it, its editor spot (upstairs) and its
/// marker (on the ground floor).
const CASES: &[(&str, bool, bool, Held, [f32; 2], [f32; 2])] = &[
    (
        "Persistent",
        true,
        false,
        Held::No,
        [-500.0, -13.0],
        [-276.0, -333.0],
    ),
    (
        "Temporary",
        false,
        false,
        Held::No,
        [-436.0, -13.0],
        [-148.0, -333.0],
    ),
    (
        "Unique",
        false,
        true,
        Held::No,
        [-308.0, -13.0],
        [-340.0, -237.0],
    ),
    (
        "UniqueAlias",
        false,
        true,
        Held::UniqueActor,
        [-180.0, -13.0],
        [-388.0, -317.0],
    ),
    (
        "ForcedAlias",
        false,
        false,
        Held::Forced,
        [-116.0, -13.0],
        [-404.0, -93.0],
    ),
];

const TESTS: &[(&str, &str)] = &[
    ("Unloaded", "Actors before the cell loads"),
    ("Loaded", "Actors as the cell loads"),
    ("Later", "Actors after the cell loaded"),
];

/// The aliases holding actors: id, name and the case.
fn aliases() -> Vec<(u32, &'static str, usize)> {
    CASES
        .iter()
        .enumerate()
        .filter(|(_, c)| c.3 != Held::No)
        .enumerate()
        .map(|(alias, (i, c))| (alias as u32, c.0, i))
        .collect()
}

/// `Get(Int n)`: our actor with object id `n` in `plugin`, None when the
/// game has no such reference.
pub(super) fn get_fn(plugin: &str) -> FunctionBuilder {
    let mut c = Code::new("Actor");
    let n = c.param("n", "Int");
    let f = c.global("Game", "GetFormFromFile", [n, string(plugin)]);
    let a = c.temp("Actor");
    c.cast(a.clone(), f);
    c.finish(a)
}

/// Whether `a` is None.
pub(super) fn is_none(c: &mut Code, a: Arg) -> Arg {
    let none = c.temp("Actor");
    c.cast(none.clone(), Arg::None);
    c.calc(op::CMP_EQ, "Bool", a, none)
}

/// `Line(Int n, ObjectReference m)`: `Where` our actor `n` is (from marker
/// `m`), or that the game has none.
pub(super) fn line_fn() -> FunctionBuilder {
    let mut c = Code::new("String");
    let n = c.param("n", "Int");
    let m = c.param("m", "ObjectReference");
    let a = c.own("Get", "Actor", [n]);
    let missing = is_none(&mut c, a.clone());
    let found = c.label();
    c.jmpf(missing, found)
        .ret(string("not in memory"))
        .bind(found);
    let s = c.own("Where", "String", [a, m]);
    c.finish(s)
}

/// `Loaded(Int n)`: whether our actor `n` is there with its 3D loaded.
pub(super) fn loaded_fn() -> FunctionBuilder {
    let mut c = Code::new("Bool");
    let n = c.param("n", "Int");
    let a = c.own("Get", "Actor", [n]);
    let missing = is_none(&mut c, a.clone());
    let found = c.label();
    c.jmpf(missing, found).ret(false.into()).bind(found);
    let l = c.method("ObjectReference", a, "Is3DLoaded", []);
    c.finish(l)
}

/// Every actor read into `r`, under `title`.
fn read_all(c: &mut Code, r: &Arg, title: &str) {
    let t = now(c);
    put_num(c, r, &format!("{title}, real time "), t);
    line(c, r, ":\n");
    for (i, case) in CASES.iter().enumerate() {
        let s = c.own(
            "Line",
            "String",
            [
                ((ACTORS + i as u32) as i32).into(),
                auto_var(&format!("Marker{i}")),
            ],
        );
        put(c, r, &format!("  {}: ", case.0), s);
        line(c, r, "\n");
    }
}

/// What each alias holds.
fn read_aliases(c: &mut Code, r: &Arg) {
    line(c, r, "aliases:");
    for (alias, label, _) in aliases() {
        let a = c.method("Quest", id("self"), "GetAlias", [(alias as i32).into()]);
        let ra = c.temp("ReferenceAlias");
        c.cast(ra.clone(), a);
        let held = c.method("ReferenceAlias", ra, "GetReference", []);
        put(c, r, &format!(" {label} holds "), held);
        line(c, r, ";");
    }
    line(c, r, "\n");
}

/// The first page: the aliases, every actor while the trader is unloaded.
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
    read_aliases(&mut c, &r);
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
        for i in 0..CASES.len() {
            let l = c.own("Loaded", "Bool", [((ACTORS + i as u32) as i32).into()]);
            c.jmpf(l, not_yet);
        }
        c.jmp(hit);
        c.bind(not_yet);
    });
    put_num(&mut c, &r, "moved in at real time ", start);
    put_num(&mut c, &r, "; every actor's 3D after ", waited);
    line(&mut c, &r, " s (-1: not in 30 s)\n");
    read_all(&mut c, &r, "loaded, at once");
    c.wait(1.0);
    read_all(&mut c, &r, "1 s later");
    c.finish(r)
}

/// The third page: 3 and 10 s after loading, and the aliases again; then
/// the player is put back.
fn test_later() -> FunctionBuilder {
    let (mut c, r) = report();
    for (wait, title) in [(2.0, "3 s later"), (7.0, "10 s later")] {
        c.wait(wait);
        read_all(&mut c, &r, title);
    }
    read_aliases(&mut c, &r);
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
    for i in 0..CASES.len() {
        s.auto_property(&format!("Marker{i}"), "ObjectReference");
    }
    s.variable("Ran", "Bool", false.into());
    s.variable("Back", "ObjectReference", Arg::None);
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
        .function("OnUpdate", on_update("autotest8", "a minute", TESTS))
        .function("Num", num())
        .function("Where", where_fn())
        .function("Get", get_fn(PLUGIN))
        .function("Line", line_fn())
        .function("Loaded", loaded_fn())
        .function("TestUnloaded", test_unloaded())
        .function("TestLoaded", test_loaded())
        .function("TestLater", test_later());
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
    for i in 0..CASES.len() {
        properties.push((format!("Marker{i}"), object(own(MARKERS + i as u32))));
    }

    let packages = CASES
        .iter()
        .enumerate()
        .map(|(i, case)| {
            let location = pldt(0, own(MARKERS + i as u32), 0);
            records::copy(
                lo,
                "MS13LucanHoldPositionPackage",
                PACKAGES + i as u32,
                &format!("VrmAutoTest8{}", case.0),
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

    let npcs: Vec<RecordBuilder> = CASES
        .iter()
        .enumerate()
        .map(|(i, &(label, _, unique, _, _, _))| {
            Npc {
                object_id: NPCS + i as u32,
                edid: format!("VrmAutoTest8{label}Npc"),
                name: Some("vibe_rm Test NPC"),
                level: (10, false, 1, 0),
                template: (template, 0x1),
                temper: (0, 0),
                factions: Vec::new(),
                script: None,
                assistance: 0,
                combat_style: None,
                unique,
            }
            .record_with_packages(race, class, &[own(PACKAGES + i as u32)])
        })
        .collect();

    let quest_aliases = aliases()
        .into_iter()
        .map(|(alias, name, i)| Alias {
            id: alias,
            name,
            flags: alias_flags::OPTIONAL,
            fill: match CASES[i].3 {
                Held::UniqueActor => Fill::Unique(own(NPCS + i as u32)),
                _ => Fill::Forced(own(ACTORS + i as u32)),
            },
            packages: Vec::new(),
        })
        .collect();
    let quests = vec![
        Quest {
            object_id: QUEST,
            edid: "VrmAutoTest8Quest",
            flags: 0x0011,
            priority: 0,
            kind: 0,
            script: Some(AttachedScript {
                name: SCRIPT.into(),
                properties,
            }),
            stages: &[],
            aliases: quest_aliases,
        }
        .record(),
    ];

    let mut persistent_refs = vec![
        RecordBuilder::new(b"REFR", own(ENTRY))
            .flags(persistent)
            .form(b"NAME", heading_marker)
            .field(b"DATA", &placement(ENTRY_AT, 0.0)),
    ];
    let mut temporary_refs = Vec::new();
    for (i, &(_, is_persistent, _, _, [x, y], [mx, my])) in CASES.iter().enumerate() {
        persistent_refs.push(
            RecordBuilder::new(b"REFR", own(MARKERS + i as u32))
                .flags(persistent)
                .form(b"NAME", heading_marker)
                .field(b"DATA", &placement([mx, my, 0.0], 0.0)),
        );
        let actor = RecordBuilder::new(b"ACHR", own(ACTORS + i as u32))
            .form(b"NAME", own(NPCS + i as u32))
            .field(b"DATA", &placement([x, y, UPSTAIRS], std::f32::consts::PI));
        if is_persistent {
            persistent_refs.push(actor.flags(persistent));
        } else {
            temporary_refs.push(actor);
        }
    }
    let cells = trader_cell_with(lo, trader, persistent_refs, temporary_refs)?;

    // Top groups in the Creation Kit's order.
    let plugin = write_plugin(
        0,
        &["Skyrim.esm"],
        "vibe_rm",
        "Which actors the game moves by their packages while their cell is unloaded: test \
         actors in the Riverwood Trader, persistent or not, unique or not, held by an alias or \
         not, read by script on loading a save, reports saved.",
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
        for i in 0..CASES.len() {
            properties.push((
                format!("Marker{i}"),
                form(own(MARKERS + i as u32).0, "ObjectReference"),
            ));
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
            "aliases: UniqueAlias holds ",
            "; ForcedAlias holds ",
            "unloaded, at once, real time ",
            "  Persistent: 1 1 1, heading 1, from its marker 1 (height 0), 3D False",
            "  ForcedAlias: 1 1 1, heading 1, ",
            "every actor's 3D after -1 s",
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
        assert!(MARKERS + CASES.len() as u32 <= ENTRY);
        assert!(NPCS + CASES.len() as u32 <= PACKAGES);
        assert!(PACKAGES + CASES.len() as u32 <= MARKERS);
        assert!(ACTORS + CASES.len() as u32 <= NEXT_OBJECT_ID);
    }

    #[test]
    fn only_the_control_is_flagged_persistent() {
        let persistent: Vec<_> = CASES.iter().filter(|c| c.1).map(|c| c.0).collect();
        assert_eq!(persistent, ["Persistent"]);
        assert_eq!(
            aliases().iter().map(|a| a.1).collect::<Vec<_>>(),
            ["UniqueAlias", "ForcedAlias"]
        );
    }
}
