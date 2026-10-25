//! `test-mod autotest15`: whether a scene goes on while its actor is
//! unloaded (`plans/ai-actor-execution.md` step 7,
//! `known_ambiguities/actor-execution.md` "Scenes"). We have a scene's
//! package action finish with its actor away, by what the actor's package
//! did there (put 40 short of a Travel target in range, after which its run
//! ends 2.5 s on), where we used to have it wait for the actor to load
//! (kept behind `VRM_SCENES_WAIT_FOR_LOADED` until this ran). Here scenes
//! are started with their actors loaded, unloaded, and in the cell the
//! player just left, and watched from afar.
//!
//! Each actor is made as in autotest13 (the same template NPC, its own
//! Travel package home, its events logged into its `ResultEvents`) and is
//! held by an alias of the test quest. Each has its scene: phase 0 runs a
//! Travel package to the actor's far marker (action 1), phase 1 a 2 s
//! timer (action 2). Every reading gives the actor's distance from its far
//! marker and from home, its cell, 3D and package (autotest14's `Where`).
//!
//! | actor        | stands in               | its scene sends it to           |
//! | ------------ | ----------------------- | ------------------------------- |
//! | **Loaded**   | the Palace of the Kings | 150-400 away                    |
//! | **Near**     | Tamriel -20, 0          | 2500-3200 away (in jump range)  |
//! | **Beyond**   | the same                | Tamriel -18, 0 (out of range)   |
//!
//! As built against Skyrim.esm (2026-10-25) the markers are 362 (Loaded),
//! 2,711 (Near), 8,843 (Beyond, only 139 below) and 2,838 (Buffered) from
//! home. Beyond's target is as level as the jumpers' were but twice as far
//! as any, so it also says whether distance alone stops the jump
//! (`known_ambiguities/actor-execution.md`, "What decides the jump").
//! | **Buffered** | Tamriel -30, 8          | 2500-3200 away                  |
//!
//! The run, a page each (`OnUpdate`, 5 s after the save loads). The scenes
//! are watched every quarter second: when action 1 completes
//! (`IsActionComplete`), with the actor read then, and when the scene stops
//! playing.
//!
//! 1. **Loaded**: the player moved into the palace; SceneLoaded started
//!    once Loaded has its 3D, and watched for 60 s.
//! 2. **Away**: SceneNear and SceneBeyond started with the player still in
//!    the palace, their actors read before and watched for 60 s.
//! 3. **Buffered**: the player moved to Buffered's cell until it has its
//!    3D, then back into the palace; SceneBuffered started 1 s later and
//!    watched for 60 s.
//! 4. **Visit**: the player moved to Near and Beyond's cell; both read as
//!    they load, whether their scenes play, then watched for 150 s (Beyond
//!    may have a long walk).
//!
//! | reading                                                     | means                                          |
//! | ----------------------------------------------------------- | ---------------------------------------------- |
//! | SceneNear's action 1 done within seconds, the scene over 2 s later, Near unloaded | package actions finish away (our default) |
//! | SceneNear still playing until Visit, done once Near walks there loaded | scenes wait for their actors to load (the old policy) |
//! | SceneNear never playing                                     | scenes don't start with their actors unloaded  |
//! | SceneBeyond playing until Visit, done as Beyond arrives      | an out-of-range traveller holds its phase      |
//! | SceneBeyond done away, Beyond put by its marker              | distance alone doesn't stop a jump (height may) |
//! | SceneBuffered done as Buffered walks there, unloaded        | buffered actors finish their actions as they arrive |
//! | SceneLoaded done as Loaded arrives                          | (the control: our arrival rule)                |
//!
//! The plugin must be named `VrmAutoTest15.esp`. Load a save outdoors near
//! Riverwood, without the other test plugins, with a character not in
//! combat, and leave the game alone: the run takes about seven minutes.
//! As with autotest13, don't keep playing with the plugin: it carries
//! Skyrim.esm's records of Tamriel, its persistent cell and the palace.
//!
//! Run in Skyrim SE (2026-10-25, once, the test character's usual save near
//! Riverwood):
//!
//! - **Loaded**: SceneLoaded played at once; Loaded's travel was done 8.1 s
//!   on, 9 from its marker, the scene over 2 s later. Loaded logged
//!   `OnPackageEnd` for the scene's package, then 0.1 s later
//!   `OnPackageStart` for its own (no `OnPackageChange`), walked home and
//!   logged its `OnPackageEnd`.
//! - **Away**: both scenes played at once with their actors unloaded.
//!   Near was 40 short of its marker, unloaded, and its travel done 2.98 s
//!   on (`OnPackageEnd` 2.53 s after `OnPackageStart`), SceneNear over 1.8 s
//!   later. Beyond (8,843 off, 139 below) stayed, then was 40 short and its
//!   travel done 39 s on, still unloaded (`OnPackageEnd` in the frame it was
//!   put there), SceneBeyond over 2.3 s later. Each took up its own Travel
//!   home 2.1 s after its travel was done with `OnPackageStart` alone; Near
//!   (2,670 from home, unasked) was put 40 short of home 34 s after that, in
//!   the same frame as Beyond's jump.
//! - **Buffered**: Buffered kept its 3D after the player went back into the
//!   palace (`OnCellDetach`, no `OnUnload` until the player went on to Near
//!   and Beyond's cell a minute later) and walked a path: 60 s on it was
//!   1,442 from its marker (from 2,681) and 2,586 from home, SceneBuffered
//!   still playing.
//! - **Visit**: both scenes over. Near was 40 from home; Beyond, which had
//!   set off home (8,800, level) 63 s before, was 3,588 from home and 5,298
//!   from its marker.
//!
//! So scenes play on with their actors unloaded: a package action finishes
//! as the package's run does away (our default; the waiting policy is
//! gone). Unloaded travellers are put at once only when asked to evaluate
//! (`EvaluatePackage`, a scene) and in range; otherwise later, at any
//! distance seen so far when about level with their targets.

use anyhow::{Context, Result};
use esp::writer::{AttachedScript, Node, PropertyValue, RecordBuilder, write_plugin};
use esp::{FormId, LoadOrder, record_flags};
use papyrus::build::{Arg, FunctionBuilder, ScriptBuilder, auto_var, id, string};
use papyrus::pex::op;

use super::autotest2::{Npc, Vanilla, line, put};
use super::autotest4::{now, poll, put_num, report};
use super::autotest6::ALWAYS;
use super::autotest8::{get_fn, is_none};
use super::autotest13::{WALK, actor_script, package};
use super::autotest14::{move_player, since, where_fn};
use super::places::{self, Interior, Spots, World};
use super::records::{self, Alias, Fill, Quest};
use super::script::{Code, num, on_update_with};
use super::venv::placement;
use super::{TestMod, own};

// Object ids.
const QUEST: u32 = 0x800;
const NPCS: u32 = 0x810;
const PACKAGES: u32 = 0x820;
const SCENES: u32 = 0x830;
const HOMES: u32 = 0x840;
const FARS: u32 = 0x850;
const ENTRIES: u32 = 0x860;
const ACTORS: u32 = 0x870;
const NEXT_OBJECT_ID: u32 = 0x880;

const PLUGIN: &str = "VrmAutoTest15.esp";
const SCRIPT: &str = "VrmAutoTest15Script";
const ACTOR_SCRIPT: &str = "VrmAutoTest15ActorScript";
const PLAYER_REF: FormId = FormId(0x14);

/// Where actors stand and markers go: the palace and Tamriel cells.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Place {
    Palace,
    Away,
    /// Two cells east of `Away`: out of jumping range.
    Beyond,
    Buffer,
}

const PLACES: [Place; 4] = [Place::Palace, Place::Away, Place::Beyond, Place::Buffer];

const PALACE: (&str, &str) = ("WindhelmPalaceoftheKings", "Palace of the Kings");

impl Place {
    fn index(self) -> usize {
        PLACES.iter().position(|p| *p == self).unwrap()
    }
    /// A Tamriel cell's grid.
    fn grid(self) -> Option<(i32, i32)> {
        Some(match self {
            Place::Palace => return None,
            Place::Away => (-20, 0),
            Place::Beyond => (-18, 0),
            Place::Buffer => (-30, 8),
        })
    }
}

struct Actor {
    label: &'static str,
    home: Place,
    /// Its far marker in another place (none: its own spot's pair).
    far: Option<Place>,
}

const ACTOR_LIST: &[Actor] = &[
    Actor {
        label: "Loaded",
        home: Place::Palace,
        far: None,
    },
    Actor {
        label: "Near",
        home: Place::Away,
        far: None,
    },
    Actor {
        label: "Beyond",
        home: Place::Away,
        far: Some(Place::Beyond),
    },
    Actor {
        label: "Buffered",
        home: Place::Buffer,
        far: None,
    },
];

/// Seconds of each scene's timer phase.
const TIMER_SECONDS: f32 = 2.0;

const TESTS: &[(&str, &str)] = &[
    ("Loaded", "A scene with its actor loaded"),
    ("Away", "Scenes started with their actors unloaded"),
    ("Buffered", "A scene started in the cell just left"),
    ("Visit", "The unloaded scenes' actors visited"),
];

fn k_of(label: &str) -> usize {
    ACTOR_LIST
        .iter()
        .position(|a| a.label == label)
        .unwrap_or_else(|| panic!("no actor {label}"))
}

fn scene(k: usize) -> Arg {
    auto_var(&format!("Scene{k}"))
}

fn entry(p: Place) -> Arg {
    auto_var(&format!("Entry{}", p.index()))
}

/// Actor `label` read into `r`, after `text`, a line.
fn put_where(c: &mut Code, r: &Arg, text: &str, label: &str) {
    let k = k_of(label);
    let s = c.own(
        "Where",
        "String",
        [
            ((ACTORS + k as u32) as i32).into(),
            auto_var(&format!("Home{k}")),
            auto_var(&format!("Far{k}")),
        ],
    );
    put(c, r, text, s);
    line(c, r, "\n");
}

/// The player moved to `p`, then up to 60 s for the 3D of every one of
/// `labels`; `r` gets how long that took.
fn visit(c: &mut Code, r: &Arg, p: Place, labels: &[&str]) {
    move_player(c, entry(p));
    let ks: Vec<usize> = labels.iter().map(|l| k_of(l)).collect();
    let waited = poll(c, "loaded", 0.2, 60.0, |c, hit| {
        let not_yet = c.label();
        for k in &ks {
            let a = c.own("Get", "Actor", [((ACTORS + *k as u32) as i32).into()]);
            let missing = is_none(c, a.clone());
            c.jmpt(missing, not_yet);
            let l = c.method("ObjectReference", a, "Is3DLoaded", []);
            c.jmpf(l, not_yet);
        }
        c.jmp(hit);
        c.bind(not_yet);
    });
    put_num(c, r, "3D after ", waited);
    line(c, r, " s (-1: not in 60 s)\n");
}

/// Whether `label`'s scene plays and its travel is done, a line.
fn put_scene(c: &mut Code, r: &Arg, label: &str) {
    let k = k_of(label);
    let playing = c.method("Scene", scene(k), "IsPlaying", []);
    put(c, r, &format!("  Scene{label} playing "), playing);
    let done = c.method("Scene", scene(k), "IsActionComplete", [1.into()]);
    put(c, r, ", travel done ", done);
    line(c, r, "\n");
}

/// The scenes of `labels` watched every quarter second for up to `seconds`
/// (from `T0`), until none plays: when each one's travel (action 1) is
/// done, with its actor read then, and when it stops playing.
fn watch(c: &mut Code, r: &Arg, labels: &[&str], seconds: f32) {
    let ks: Vec<usize> = labels.iter().map(|l| k_of(l)).collect();
    let done: Vec<Arg> = ks
        .iter()
        .map(|k| c.local(&format!("done{k}"), "Bool"))
        .collect();
    let over: Vec<Arg> = ks
        .iter()
        .map(|k| c.local(&format!("over{k}"), "Bool"))
        .collect();
    for v in done.iter().chain(&over) {
        c.assign(v.clone(), false.into());
    }
    let all = c.local("allOver", "Bool");
    let (top, end) = (c.label(), c.label());
    c.bind(top);
    c.wait(0.25);
    let t = since(c, "T0");
    let late = c.calc(op::CMP_GT, "Bool", t.clone(), seconds.into());
    c.jmpt(late, end);
    c.assign(all.clone(), true.into());
    for (i, (label, k)) in labels.iter().zip(&ks).enumerate() {
        let (seen, skip_over) = (c.label(), c.label());
        c.jmpt(done[i].clone(), seen);
        let complete = c.method("Scene", scene(*k), "IsActionComplete", [1.into()]);
        c.jmpf(complete, seen);
        c.assign(done[i].clone(), true.into());
        put_num(c, r, &format!("  {label} travel done at "), t.clone());
        put_where(c, r, " s: ", label);
        c.bind(seen);
        c.jmpt(over[i].clone(), skip_over);
        let playing = c.method("Scene", scene(*k), "IsPlaying", []);
        let still = c.label();
        c.jmpt(playing, still);
        c.assign(over[i].clone(), true.into());
        put_num(c, r, &format!("  Scene{label} over at "), t.clone());
        line(c, r, " s\n");
        c.jmp(skip_over);
        c.bind(still);
        c.assign(all.clone(), false.into());
        c.bind(skip_over);
    }
    c.jmpf(all, top);
    c.bind(end);
    for (i, label) in labels.iter().enumerate() {
        let finished = c.label();
        c.jmpt(over[i].clone(), finished);
        put_where(
            c,
            r,
            &format!("  Scene{label} still playing after {seconds} s: "),
            label,
        );
        c.bind(finished);
    }
}

/// `labels`' scenes started, `T0` set, and whether each plays at once.
fn start(c: &mut Code, r: &Arg, labels: &[&str]) {
    let t = now(c);
    c.assign(id("T0"), t);
    for label in labels {
        c.method("Scene", scene(k_of(label)), "Start", []);
    }
    c.wait(0.1);
    line(c, r, "started; 0.1 s on:\n");
    for label in labels {
        put_scene(c, r, label);
    }
}

/// Step 1: a scene with its actor loaded (the control).
fn test_loaded() -> FunctionBuilder {
    let (mut c, r) = report();
    line(&mut c, &r, "in the palace; ");
    visit(&mut c, &r, Place::Palace, &["Loaded"]);
    put_where(&mut c, &r, "  Loaded: ", "Loaded");
    start(&mut c, &r, &["Loaded"]);
    watch(&mut c, &r, &["Loaded"], 60.0);
    c.finish(r)
}

const AWAY: [&str; 2] = ["Near", "Beyond"];

/// Step 2: scenes started with their actors unloaded.
fn test_away() -> FunctionBuilder {
    let (mut c, r) = report();
    line(&mut c, &r, "from the palace:\n");
    for label in AWAY {
        put_where(&mut c, &r, &format!("  {label}: "), label);
    }
    start(&mut c, &r, &AWAY);
    watch(&mut c, &r, &AWAY, 60.0);
    line(&mut c, &r, "then:\n");
    for label in AWAY {
        put_where(&mut c, &r, &format!("  {label}: "), label);
    }
    c.finish(r)
}

/// Step 3: a scene started with its actor in the cell just left.
fn test_buffered() -> FunctionBuilder {
    let (mut c, r) = report();
    line(&mut c, &r, "at Buffered's cell; ");
    visit(&mut c, &r, Place::Buffer, &["Buffered"]);
    line(&mut c, &r, "back in the palace; ");
    visit(&mut c, &r, Place::Palace, &["Loaded"]);
    c.wait(1.0);
    put_where(&mut c, &r, "  Buffered: ", "Buffered");
    start(&mut c, &r, &["Buffered"]);
    watch(&mut c, &r, &["Buffered"], 60.0);
    put_where(&mut c, &r, "then: ", "Buffered");
    c.finish(r)
}

/// Step 4: the unloaded scenes' actors visited.
fn test_visit() -> FunctionBuilder {
    let (mut c, r) = report();
    line(&mut c, &r, "at Near and Beyond's cell; ");
    visit(&mut c, &r, Place::Away, &AWAY);
    for label in AWAY {
        put_where(&mut c, &r, &format!("  {label}: "), label);
        put_scene(&mut c, &r, label);
    }
    let t = now(&mut c);
    c.assign(id("T0"), t);
    watch(&mut c, &r, &AWAY, 150.0);
    c.finish(r)
}

fn scripts() -> Result<Vec<(String, Vec<u8>)>> {
    let mut s = ScriptBuilder::new(SCRIPT, "Quest");
    s.auto_property("Player", "Actor");
    for p in PLACES {
        s.auto_property(&format!("Entry{}", p.index()), "ObjectReference");
    }
    for k in 0..ACTOR_LIST.len() {
        s.auto_property(&format!("Home{k}"), "ObjectReference");
        s.auto_property(&format!("Far{k}"), "ObjectReference");
        s.auto_property(&format!("Scene{k}"), "Scene");
    }
    s.variable("Ran", "Bool", false.into());
    s.variable("T0", "Float", 0.0f32.into());
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
        .function(
            "OnUpdate",
            on_update_with(
                "autotest15",
                "vibe_rm autotest15 running: about seven minutes. Leave the game be.",
                TESTS,
            ),
        )
        .function("Num", num())
        .function("Get", get_fn(PLUGIN))
        .function("Where", where_fn())
        .function("TestLoaded", test_loaded())
        .function("TestAway", test_away())
        .function("TestBuffered", test_buffered())
        .function("TestVisit", test_visit());
    let actors = actor_script(ACTOR_SCRIPT)?;
    Ok(vec![
        (SCRIPT.to_owned(), papyrus::pex::write(&s.build()?)?),
        (
            ACTOR_SCRIPT.to_owned(),
            papyrus::pex::write(&actors.build()?)?,
        ),
    ])
}

/// The positions: each actor's spot and far marker, each place's entry,
/// and each place's cell.
struct Layout {
    cells: Vec<FormId>,
    entries: Vec<[f32; 3]>,
    homes: Vec<[f32; 3]>,
    fars: Vec<[f32; 3]>,
}

fn layout(lo: &LoadOrder, v: &Vanilla) -> Result<Layout> {
    let tamriel = v.id("Tamriel")?;
    let cells = PLACES
        .iter()
        .map(|p| match p.grid() {
            Some(grid) => lo
                .world(tamriel)
                .and_then(|w| w.cells.get(&grid))
                .copied()
                .with_context(|| format!("no Tamriel cell at {grid:?}")),
            None => v.id(PALACE.0),
        })
        .collect::<Result<Vec<_>>>()?;
    // Spots each place needs: one per actor standing there (with its pair
    // in reach), one per far marker of an actor elsewhere.
    let mut spots: Vec<Spots> = Vec::new();
    for (i, p) in PLACES.iter().enumerate() {
        let homes = ACTOR_LIST.iter().filter(|a| a.home == *p).count();
        let visitors = ACTOR_LIST.iter().filter(|a| a.far == Some(*p)).count();
        let (reach, apart) = match p {
            Place::Palace => ((150.0, 400.0), 100.0),
            Place::Beyond => ((250.0, 450.0), 120.0),
            _ => ((2500.0, 3200.0), 150.0),
        };
        let s = places::spots(lo, cells[i], homes + visitors, reach, apart)
            .with_context(|| format!("spots in {p:?}"))?;
        spots.push(s);
    }
    let mut used = vec![0usize; PLACES.len()];
    let mut take = |p: Place| {
        let i = p.index();
        let pair = spots[i].pairs[used[i]];
        used[i] += 1;
        pair
    };
    let mut homes = Vec::new();
    let mut pairs = Vec::new();
    for a in ACTOR_LIST {
        let (spot, pair) = take(a.home);
        homes.push(spot);
        pairs.push(pair);
    }
    let fars = ACTOR_LIST
        .iter()
        .zip(pairs)
        .map(|(a, pair)| match a.far {
            None => pair,
            Some(p) => take(p).0,
        })
        .collect();
    Ok(Layout {
        cells,
        entries: spots.iter().map(|s| s.entry).collect(),
        homes,
        fars,
    })
}

pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    let v = Vanilla { lo };
    let template = v.id("WEMerchantMiscMaleCommoner")?;
    let race = v.id("ImperialRace")?;
    let class = v.id("Citizen")?;
    let heading_marker = v.id("XMarkerHeading")?;
    let tamriel = v.id("Tamriel")?;
    let persistent = record_flags::PERSISTENT;
    let at = layout(lo, &v)?;

    let object = PropertyValue::Object;
    let mut properties = vec![("Player".to_owned(), object(PLAYER_REF))];
    for p in PLACES {
        let i = p.index();
        properties.push((format!("Entry{i}"), object(own(ENTRIES + i as u32))));
    }
    for k in 0..ACTOR_LIST.len() as u32 {
        properties.push((format!("Home{k}"), object(own(HOMES + k))));
        properties.push((format!("Far{k}"), object(own(FARS + k))));
        properties.push((format!("Scene{k}"), object(own(SCENES + k))));
    }

    // Two packages each: its scene's Travel to its far marker, and its own
    // Travel home.
    let mut packages = Vec::new();
    for (k, a) in ACTOR_LIST.iter().enumerate() {
        let id = PACKAGES + 2 * k as u32;
        packages.push(package(
            lo,
            id,
            &format!("VrmAutoTest15{}Travel", a.label),
            own(FARS + k as u32),
            ALWAYS,
            None,
            WALK,
        )?);
        packages.push(package(
            lo,
            id + 1,
            &format!("VrmAutoTest15{}Home", a.label),
            own(HOMES + k as u32),
            ALWAYS,
            None,
            WALK,
        )?);
    }

    let actor_script = AttachedScript {
        name: ACTOR_SCRIPT.into(),
        properties: Vec::new(),
    };
    let npcs: Vec<RecordBuilder> = ACTOR_LIST
        .iter()
        .enumerate()
        .map(|(k, a)| {
            Npc {
                object_id: NPCS + k as u32,
                edid: format!("VrmAutoTest15{}Npc", a.label),
                name: Some("vibe_rm Test NPC"),
                level: (10, false, 1, 0),
                template: (template, 0x1),
                temper: (0, 0),
                factions: Vec::new(),
                script: Some(actor_script.clone()),
                assistance: 0,
                combat_style: None,
                unique: false,
            }
            .record_with_packages(race, class, &[own(PACKAGES + 2 * k as u32 + 1)])
        })
        .collect();

    // Alias k holds actor k.
    let aliases = ACTOR_LIST
        .iter()
        .enumerate()
        .map(|(k, a)| Alias {
            id: k as u32,
            name: a.label,
            flags: 0,
            fill: Fill::Forced(own(ACTORS + k as u32)),
            packages: Vec::new(),
        })
        .collect();
    let quests = vec![
        Quest {
            object_id: QUEST,
            edid: "VrmAutoTest15Quest",
            flags: 0x0011,
            priority: 0,
            kind: 0,
            script: Some(AttachedScript {
                name: SCRIPT.into(),
                properties,
            }),
            stages: &[],
            aliases,
        }
        .record(),
    ];
    let scenes: Vec<RecordBuilder> = ACTOR_LIST
        .iter()
        .enumerate()
        .map(|(k, a)| {
            records::package_scene(
                SCENES + k as u32,
                &format!("VrmAutoTest15{}Scene", a.label),
                own(QUEST),
                k as u32,
                own(PACKAGES + 2 * k as u32),
                TIMER_SECONDS,
            )
        })
        .collect();

    // Every reference persistent, Tamriel's in its persistent cell.
    let marker = |id: FormId, at: [f32; 3]| {
        RecordBuilder::new(b"REFR", id)
            .flags(persistent)
            .form(b"NAME", heading_marker)
            .field(b"DATA", &placement(at, 0.0))
    };
    let mut by_place: Vec<Vec<RecordBuilder>> = PLACES.iter().map(|_| Vec::new()).collect();
    for p in PLACES {
        let i = p.index();
        by_place[i].push(marker(own(ENTRIES + i as u32), at.entries[i]));
    }
    for (k, a) in ACTOR_LIST.iter().enumerate() {
        let home = a.home.index();
        by_place[home].push(marker(own(HOMES + k as u32), at.homes[k]));
        let far = a.far.map_or(home, Place::index);
        by_place[far].push(marker(own(FARS + k as u32), at.fars[k]));
        by_place[home].push(
            RecordBuilder::new(b"ACHR", own(ACTORS + k as u32))
                .flags(persistent)
                .form(b"NAME", own(NPCS + k as u32))
                .field(b"DATA", &placement(at.homes[k], 0.0)),
        );
    }

    let mut interiors = Vec::new();
    let mut tamriel_persistent = Vec::new();
    for (p, refs) in PLACES.into_iter().zip(by_place) {
        match p.grid() {
            None => interiors.push(Interior {
                cell: at.cells[p.index()],
                name: PALACE.1,
                persistent: refs,
                temporary: Vec::new(),
            }),
            Some(_) => tamriel_persistent.extend(refs),
        }
    }
    let worlds = vec![World {
        world: tamriel,
        name: "Skyrim",
        grid: Place::Away.grid().unwrap(),
        persistent: tamriel_persistent,
        temporary: Vec::new(),
    }];

    // Top groups in the Creation Kit's order.
    let plugin = write_plugin(
        0,
        &["Skyrim.esm"],
        "vibe_rm",
        "Whether a scene's package action finishes while its actor is unloaded: scenes started \
         with their actors loaded, unloaded and in the cell just left, then visited. Read by \
         script; reports saved.",
        NEXT_OBJECT_ID,
        &[
            Node::top(b"NPC_", npcs),
            places::interiors(lo, interiors)?,
            places::worlds(lo, worlds)?,
            Node::top(b"QUST", quests),
            Node::top(b"PACK", packages),
            Node::top(b"SCEN", scenes),
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

    fn properties() -> Vec<(String, papyrus::Value)> {
        let mut properties = vec![("Player".to_owned(), form(0x14, "Actor"))];
        for p in PLACES {
            properties.push((
                format!("Entry{}", p.index()),
                form(own(ENTRIES + p.index() as u32).0, "ObjectReference"),
            ));
        }
        for k in 0..ACTOR_LIST.len() as u32 {
            properties.push((
                format!("Home{k}"),
                form(own(HOMES + k).0, "ObjectReference"),
            ));
            properties.push((format!("Far{k}"), form(own(FARS + k).0, "ObjectReference")));
            properties.push((format!("Scene{k}"), form(own(SCENES + k).0, "Scene")));
        }
        properties
    }

    #[test]
    fn the_tests_run_and_report() {
        let boxes = TESTS.len();
        let shown = run(
            scripts().unwrap(),
            &[(own(QUEST).0, SCRIPT, properties())],
            boxes,
        );
        assert_eq!(shown.boxes.len(), boxes, "every test reports");
        assert_eq!(shown.saved_at_box, Some(0), "saves before the reports");
        let all = shown.boxes.join("\n");
        for expected in [
            "in the palace; 3D after -1 s (-1: not in 60 s)\n",
            "started; 0.1 s on:\n  SceneLoaded playing False, travel done False\n",
            "  SceneLoaded over at 0.450 s\n",
            "  SceneNear over at 0.450 s\n  SceneBeyond over at 0.450 s\nthen:\n",
            "back in the palace; 3D after ",
            "at Near and Beyond's cell; 3D after ",
            "  SceneBeyond playing False, travel done False\n",
        ] {
            assert!(all.contains(expected), "{expected:?} in\n{all}");
        }
        for b in &shown.boxes {
            let lines = b.lines().count();
            assert!(lines <= 20, "{lines} lines:\n{b}");
        }
    }

    #[test]
    fn object_ids_fit() {
        let n = ACTOR_LIST.len() as u32;
        const { assert!(QUEST < NPCS) };
        assert!(NPCS + n <= PACKAGES);
        assert!(PACKAGES + 2 * n <= SCENES);
        assert!(SCENES + n <= HOMES);
        assert!(HOMES + n <= FARS);
        assert!(FARS + n <= ENTRIES);
        assert!(ENTRIES + PLACES.len() as u32 <= ACTORS);
        assert!(ACTORS + n <= NEXT_OBJECT_ID);
    }
}
