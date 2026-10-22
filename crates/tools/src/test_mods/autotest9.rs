//! `test-mod autotest9`: autotest8's open ends
//! (`known_ambiguities/unloaded-placement.md`, `alias-fills.md`). autotest8
//! found the game moving actors by their packages while their cell (the
//! Riverwood Trader, beside the player) was unloaded, flagged persistent or
//! not, and a unique-actor alias left empty for a unique actor whose
//! reference isn't flagged persistent. Here:
//!
//! **How far.** The same Travel packages (a copy of
//! `MS13LucanHoldPositionPackage`, radius 0, always on, to a marker of
//! the actor's own) for actors in five places, each actor and its marker in
//! one cell:
//!
//! | place          | cell                                              | what it adds                      |
//! | -------------- | ------------------------------------------------- | --------------------------------- |
//! | **Near**       | the Riverwood Trader                              | autotest8 again                   |
//! | **Mid**        | Ysolda's House, Whiterun                          | another hold's town               |
//! | **Far**        | Kraldar's House, Winterhold (`WinterholdKorirshouse`) | across the map                |
//! | **Outdoors**   | Tamriel -37, 21 (west of Solitude)                | an exterior cell far off          |
//! | **Otherworld** | Darkwater Pass 25, -13 (`DarkwaterWorld`)         | another worldspace                |
//!
//! In each: **Persistent** (flagged persistent, the control), **Read** (not
//! flagged, read while unloaded, as autotest8) and **Unread** (not flagged,
//! first looked up as the player arrives: if looking an actor up with
//! `Game.GetFormFromFile` is what kept autotest8's in the game, Unread
//! stays at its editor spot where Read moved).
//!
//! **The unique-actor alias.** Three unique actors in the Riverwood
//! Trader, each filling an optional unique-actor alias of the quest as it
//! starts (the save loading): **AliasPersistent** (flagged persistent: the
//! control), **AliasPlain** (not flagged: autotest8's empty one) and
//! **AliasListed** (not flagged, but as vanilla unique actors are: its
//! reference's persist location (`XLCN`) the trader's location
//! `RiverwoodLucansDryGoodsLocation`, and that location listing it among its
//! unique actors (`ACUN`), as `WindhelmLocation` lists Suvaris Atheron).
//! Once the player is in the trader (AliasPlain loaded), a second quest,
//! started then by script, fills an alias with AliasPlain too: whether a
//! loaded actor fills where an unloaded one didn't.
//!
//! The spots are picked on each cell's navmesh as the plugin is built
//! (`places::spots`: flat triangles of its largest part, on an interior's
//! main floor), the markers 250 to 450 from their actors.
//!
//! The readings (`OnUpdate`), a page each: the aliases 5 s after loading;
//! every Persistent and Read 10 s and 60 s after loading (and the aliases
//! again); then the player is moved to each place in turn (to a marker of
//! ours), and that place's actors are read as all their 3D loads and 3 s
//! later (at Near, the alias actors too, then the second quest's alias);
//! then the player is put back. Each reading: position, heading, distance
//! and height from its marker, 3D and package, or "not in memory".
//!
//! | reading                                                  | means                                                |
//! | -------------------------------------------------------- | ---------------------------------------------------- |
//! | Read / Unread 40 short of their marker everywhere         | every actor moves wherever it is                    |
//! | moved Near, at its editor spot (or not in memory) farther | the game moves only actors near the player          |
//! | Unread at its editor spot where Read moved                | looking an actor up keeps it (autotest8 misled)     |
//! | AliasListed filled, AliasPlain not                        | unique-actor fills go by the location lists         |
//! | the second quest's alias filled                           | unique-actor fills need the actor loaded            |
//!
//! The plugin must be named `VrmAutoTest9.esp` (the script looks its
//! actors up by that name). Use a character that has never been to any of
//! the five places (the game keeps recently visited interiors' references
//! around) and load a save outdoors near Riverwood, without the other test
//! plugins (autotest6 to autotest8 override the trader too). Leave the game
//! alone for about four minutes: the player is moved through all five
//! places and back, with loading screens. Don't keep playing with the
//! plugin: besides the actors it carries Skyrim.esm's records of Tamriel
//! and its persistent cell (without the Creation Club's changes to them)
//! and the trader's location.
//!
//! Run in Skyrim SE (2026-10-22, once, the test character's usual save):
//!
//! - Every actor in all five places had moved while unloaded: on its line,
//!   stopped short of its marker in XY (40, or 50 for six of the
//!   eighteen) by the 10 s reading, and unchanged at 60 s. **Unread**
//!   actors, looked up only as the player arrived, were where **Read** ones
//!   were: the lookup played no part. At load they dropped onto the floor
//!   beneath; some then walked nearer.
//! - The aliases: only **AliasListed** filled, as the quest started and
//!   after. **AliasPersistent** stayed empty, though flagged persistent,
//!   and the second quest, started with **AliasPlain** loaded beside the
//!   player, left its alias empty too.
//!
//! Run again in a new game (`coc Riverwood` from the main menu, the
//! Survival Mode question answered no, which delayed the start):
//!
//! - Every actor read while unloaded was exactly on its marker (0 off,
//!   height 0) by the 10 s reading, not short of it. At load most were
//!   within 3 of their markers; **MidUnread** 50 short, **NearPersistent**
//!   45 off (on its marker while unloaded); some then walked on.
//! - The aliases exactly as before: only **AliasListed** filled.
//!
//! So distance doesn't decide which actors move while unloaded (the engine
//! moves every one), and neither persistence, being loaded nor a new game
//! lets an unlisted unique actor fill an alias. Vanilla has unique-actor
//! aliases for actors no location lists (Brynjolf's); whether those fill
//! isn't observed (`known_ambiguities/alias-fills.md`). Where a new game
//! puts actors: `known_ambiguities/unloaded-placement.md`.

use anyhow::{Context, Result};
use esp::writer::{AttachedScript, Node, PropertyValue, RecordBuilder, write_plugin};
use esp::{FormId, LoadOrder, record_flags};
use papyrus::build::{Arg, FunctionBuilder, ScriptBuilder, auto_var, id, string};
use papyrus::pex::op;

use super::autotest2::{Npc, Vanilla, line, put};
use super::autotest4::{now, poll, put_num, report};
use super::autotest6::{ALWAYS, pldt, where_fn};
use super::autotest8::{get_fn, line_fn, loaded_fn};
use super::places::{self, Interior, Spots, World};
use super::records::{self, Alias, Fill, Quest, alias_flags};
use super::script::{Code, num, on_update};
use super::venv::placement;
use super::{TestMod, own};

// Object ids: the quests, the NPCs, their packages, the markers, the
// entries, then the actors (one NPC, package and marker per actor).
const QUEST: u32 = 0x800;
const QUEST_B: u32 = 0x801;
const NPCS: u32 = 0x810;
const PACKAGES: u32 = 0x830;
const MARKERS: u32 = 0x850;
const ENTRIES: u32 = 0x870;
const ACTORS: u32 = 0x880;
const NEXT_OBJECT_ID: u32 = 0x8A0;

const PLUGIN: &str = "VrmAutoTest9.esp";
const SCRIPT: &str = "VrmAutoTest9Script";
const PLAYER_REF: FormId = FormId(0x14);

/// Where actors and their markers go: an interior cell (editor id, name)
/// or an exterior one (worldspace editor id, its name, grid).
#[derive(Clone, Copy)]
enum Cell {
    Interior(&'static str, &'static str),
    Exterior(&'static str, &'static str, (i32, i32)),
}

/// The places, in the order the player visits them.
const PLACES: &[(&str, Cell)] = &[
    (
        "Near",
        Cell::Interior("RiverwoodRiverwoodTrader", "Riverwood Trader"),
    ),
    (
        "Mid",
        Cell::Interior("WhiterunYsoldasHouse", "Ysolda's House"),
    ),
    (
        "Far",
        Cell::Interior("WinterholdKorirshouse", "Kraldar's House"),
    ),
    ("Outdoors", Cell::Exterior("Tamriel", "Skyrim", (-37, 21))),
    (
        "Otherworld",
        Cell::Exterior("DarkwaterWorld", "Darkwater Pass", (25, -13)),
    ),
];

/// The trader's location, named as Skyrim.esm has it.
const TRADER_LOCATION: (&str, &str) = ("RiverwoodLucansDryGoodsLocation", "Lucan's Dry Goods");

/// What an actor is for.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Role {
    Persistent,
    Read,
    Unread,
    AliasPersistent,
    AliasPlain,
    AliasListed,
}

impl Role {
    fn persistent(self) -> bool {
        matches!(self, Role::Persistent | Role::AliasPersistent)
    }
    fn unique(self) -> bool {
        matches!(
            self,
            Role::AliasPersistent | Role::AliasPlain | Role::AliasListed
        )
    }
    /// Read while its place is unloaded.
    fn read_early(self) -> bool {
        matches!(self, Role::Persistent | Role::Read)
    }
}

const DISTANCE_ROLES: [Role; 3] = [Role::Persistent, Role::Read, Role::Unread];
/// The alias actors (all at Near), in the order of the quest's aliases.
const ALIAS_ROLES: [Role; 3] = [Role::AliasPersistent, Role::AliasPlain, Role::AliasListed];

/// An actor: its label, place (index into [`PLACES`]) and role. Its
/// index is its object id's offset from [`ACTORS`] (and [`NPCS`],
/// [`PACKAGES`], [`MARKERS`]).
struct Actor {
    label: String,
    place: usize,
    role: Role,
}

fn actors() -> Vec<Actor> {
    let mut out = Vec::new();
    for (p, (name, _)) in PLACES.iter().enumerate() {
        for role in DISTANCE_ROLES {
            out.push(Actor {
                label: format!("{name}{role:?}"),
                place: p,
                role,
            });
        }
    }
    for role in ALIAS_ROLES {
        out.push(Actor {
            label: format!("{role:?}"),
            place: 0,
            role,
        });
    }
    out
}

const TESTS: &[(&str, &str)] = &[
    ("Aliases", "The unique-actor aliases"),
    ("Unloaded10", "Actors 10 s after loading"),
    ("Unloaded60", "Actors 60 s after loading"),
    ("Near", "The Riverwood Trader"),
    ("Mid", "Ysolda's House"),
    ("Far", "Kraldar's House"),
    ("Outdoors", "Tamriel -37, 21"),
    ("Otherworld", "Darkwater Pass"),
];

/// Actor `k` read into `r` (its `Where`, or "not in memory").
fn read_actor(c: &mut Code, r: &Arg, k: usize, label: &str) {
    let s = c.own(
        "Line",
        "String",
        [
            ((ACTORS + k as u32) as i32).into(),
            auto_var(&format!("Marker{k}")),
        ],
    );
    put(c, r, &format!("  {label}: "), s);
    line(c, r, "\n");
}

/// The actors `which` takes, under `title` and the real time.
fn read_some(c: &mut Code, r: &Arg, title: &str, which: impl Fn(&Actor) -> bool) {
    let t = now(c);
    put_num(c, r, &format!("{title}, real time "), t);
    line(c, r, ":\n");
    for (k, a) in actors().iter().enumerate() {
        if which(a) {
            read_actor(c, r, k, &a.label);
        }
    }
}

/// What `quest`'s aliases hold, on one line.
fn read_aliases(c: &mut Code, r: &Arg, title: &str, quest: Arg, labels: &[&str]) {
    line(c, r, &format!("{title}:"));
    for (alias, label) in labels.iter().enumerate() {
        let a = c.method("Quest", quest.clone(), "GetAlias", [(alias as i32).into()]);
        let ra = c.temp("ReferenceAlias");
        c.cast(ra.clone(), a);
        let held = c.method("ReferenceAlias", ra, "GetReference", []);
        put(c, r, &format!(" {label} holds "), held);
        line(c, r, ";");
    }
    line(c, r, "\n");
}

fn alias_labels() -> Vec<String> {
    ALIAS_ROLES.iter().map(|r| format!("{r:?}")).collect()
}

/// The first quest's aliases.
fn quest_aliases(c: &mut Code, r: &Arg, title: &str) {
    let labels = alias_labels();
    let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
    read_aliases(c, r, title, id("self"), &labels);
}

/// The first page: the player held still (and warned of if already in a
/// test place), the aliases.
fn test_aliases() -> FunctionBuilder {
    let (mut c, r) = report();
    c.controls(false, true, true);
    let cell = c.method("ObjectReference", auto_var("Player"), "GetParentCell", []);
    for (p, (name, place)) in PLACES.iter().enumerate() {
        if let Cell::Interior(..) = place {
            let inside = c.calc(
                op::CMP_EQ,
                "Bool",
                cell.clone(),
                auto_var(&format!("Cell{p}")),
            );
            let outside = c.label();
            c.jmpf(inside, outside);
            line(
                &mut c,
                &r,
                &format!("INVALID: the player was in {name}'s cell when the test began\n"),
            );
            c.bind(outside);
        }
    }
    quest_aliases(&mut c, &r, "as the quest starts");
    c.finish(r)
}

/// Every actor read while unloaded, `wait` seconds on.
fn test_unloaded(wait: f32, title: &str) -> FunctionBuilder {
    let (mut c, r) = report();
    c.wait(wait);
    read_some(&mut c, &r, title, |a| a.role.read_early());
    quest_aliases(&mut c, &r, "aliases");
    c.finish(r)
}

/// The player moved to place `p` (kept where they were in `Back` before
/// the first), its actors read as their 3D loads and 3 s later; at the
/// last, the player put back.
fn test_visit(p: usize) -> FunctionBuilder {
    let (mut c, r) = report();
    let player = auto_var("Player");
    if p == 0 {
        let back = c.method(
            "ObjectReference",
            player.clone(),
            "PlaceAtMe",
            [auto_var("XMarker"), 1.into(), false.into(), false.into()],
        );
        c.assign(id("Back"), back);
    }
    let here: Vec<(usize, Actor)> = actors()
        .into_iter()
        .enumerate()
        .filter(|(_, a)| a.place == p)
        .collect();
    let start = now(&mut c);
    c.method(
        "ObjectReference",
        player.clone(),
        "MoveTo",
        [
            auto_var(&format!("Entry{p}")),
            0.0f32.into(),
            0.0f32.into(),
            0.0f32.into(),
            true.into(),
        ],
    );
    let waited = poll(&mut c, "loaded", 0.1, 60.0, |c, hit| {
        let not_yet = c.label();
        for (k, _) in &here {
            let l = c.own("Loaded", "Bool", [((ACTORS + *k as u32) as i32).into()]);
            c.jmpf(l, not_yet);
        }
        c.jmp(hit);
        c.bind(not_yet);
    });
    put_num(&mut c, &r, "moved in at real time ", start);
    put_num(&mut c, &r, "; every actor's 3D after ", waited);
    line(&mut c, &r, " s (-1: not in 60 s)\n");
    line(&mut c, &r, "at once:\n");
    for (k, a) in &here {
        read_actor(&mut c, &r, *k, &a.label);
    }
    if p == 0 {
        let quest_b = auto_var("QuestB");
        c.method("Quest", quest_b.clone(), "Start", []);
        c.wait(1.0);
        read_aliases(
            &mut c,
            &r,
            "second quest, started now",
            quest_b,
            &["AliasPlain"],
        );
        quest_aliases(&mut c, &r, "first quest");
        c.wait(2.0);
    } else {
        c.wait(3.0);
    }
    line(&mut c, &r, "3 s later:\n");
    for (k, a) in here.iter().filter(|(_, a)| !a.role.unique()) {
        read_actor(&mut c, &r, *k, &a.label);
    }
    if p + 1 == PLACES.len() {
        let back = id("Back");
        c.method(
            "ObjectReference",
            player,
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
    }
    c.finish(r)
}

fn scripts() -> Result<Vec<(String, Vec<u8>)>> {
    let mut s = ScriptBuilder::new(SCRIPT, "Quest");
    s.auto_property("Player", "Actor");
    s.auto_property("XMarker", "Form");
    s.auto_property("QuestB", "Quest");
    for (p, (_, place)) in PLACES.iter().enumerate() {
        s.auto_property(&format!("Entry{p}"), "ObjectReference");
        if let Cell::Interior(..) = place {
            s.auto_property(&format!("Cell{p}"), "Cell");
        }
    }
    for k in 0..actors().len() {
        s.auto_property(&format!("Marker{k}"), "ObjectReference");
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
        .function("OnUpdate", on_update("autotest9", "four minutes", TESTS))
        .function("Num", num())
        .function("Where", where_fn())
        .function("Get", get_fn(PLUGIN))
        .function("Line", line_fn())
        .function("Loaded", loaded_fn())
        .function("TestAliases", test_aliases())
        .function("TestUnloaded10", test_unloaded(5.0, "10 s after loading"))
        .function("TestUnloaded60", test_unloaded(50.0, "60 s after loading"));
    for (p, (name, _)) in PLACES.iter().enumerate() {
        s.function(&format!("Test{name}"), test_visit(p));
    }
    Ok(vec![(SCRIPT.to_owned(), papyrus::pex::write(&s.build()?)?)])
}

/// Each place's cell and spots: an entry and a pair per actor there.
fn place_spots(lo: &LoadOrder, v: &Vanilla) -> Result<Vec<(FormId, Spots)>> {
    let all = actors();
    PLACES
        .iter()
        .enumerate()
        .map(|(p, (name, place))| {
            let cell = match *place {
                Cell::Interior(edid, _) => v.id(edid)?,
                Cell::Exterior(world, _, grid) => *lo
                    .world(v.id(world)?)
                    .and_then(|w| w.cells.get(&grid))
                    .with_context(|| format!("{world} has no cell at {grid:?}"))?,
            };
            let n = all.iter().filter(|a| a.place == p).count();
            let spots = places::spots(lo, cell, n, (250.0, 450.0), 80.0)
                .with_context(|| format!("spots in {name}"))?;
            Ok((cell, spots))
        })
        .collect()
}

pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    let v = Vanilla { lo };
    let template = v.id("WEMerchantMiscMaleCommoner")?;
    let race = v.id("ImperialRace")?;
    let class = v.id("Citizen")?;
    let heading_marker = v.id("XMarkerHeading")?;
    let location = v.id(TRADER_LOCATION.0)?;
    let persistent = record_flags::PERSISTENT;
    let all = actors();
    let spots = place_spots(lo, &v)?;

    let object = PropertyValue::Object;
    let mut properties = vec![
        ("Player".to_owned(), object(PLAYER_REF)),
        ("XMarker".to_owned(), object(v.id("XMarker")?)),
        ("QuestB".to_owned(), object(own(QUEST_B))),
    ];
    for (p, (_, place)) in PLACES.iter().enumerate() {
        properties.push((format!("Entry{p}"), object(own(ENTRIES + p as u32))));
        if let Cell::Interior(..) = place {
            properties.push((format!("Cell{p}"), object(spots[p].0)));
        }
    }
    for k in 0..all.len() {
        properties.push((format!("Marker{k}"), object(own(MARKERS + k as u32))));
    }

    let packages = all
        .iter()
        .enumerate()
        .map(|(k, a)| {
            let at = pldt(0, own(MARKERS + k as u32), 0);
            records::copy(
                lo,
                "MS13LucanHoldPositionPackage",
                PACKAGES + k as u32,
                &format!("VrmAutoTest9{}", a.label),
                None,
                |tag, d| match tag {
                    b"PLDT" => *d = at.clone(),
                    b"PSDT" => *d = ALWAYS.to_vec(),
                    // Flag 0x1: offers services.
                    b"PKDT" if d.len() >= 4 => d[0] &= !0x1,
                    _ => {}
                },
            )
        })
        .collect::<Result<Vec<_>>>()?;

    let npcs: Vec<RecordBuilder> = all
        .iter()
        .enumerate()
        .map(|(k, a)| {
            Npc {
                object_id: NPCS + k as u32,
                edid: format!("VrmAutoTest9{}Npc", a.label),
                name: Some("vibe_rm Test NPC"),
                level: (10, false, 1, 0),
                template: (template, 0x1),
                temper: (0, 0),
                factions: Vec::new(),
                script: None,
                assistance: 0,
                combat_style: None,
                unique: a.role.unique(),
            }
            .record_with_packages(race, class, &[own(PACKAGES + k as u32)])
        })
        .collect();

    let unique_alias = |id: u32, k: usize| Alias {
        id,
        name: match all[k].role {
            Role::AliasPersistent => "AliasPersistent",
            Role::AliasPlain => "AliasPlain",
            _ => "AliasListed",
        },
        flags: alias_flags::OPTIONAL,
        fill: Fill::Unique(own(NPCS + k as u32)),
        packages: Vec::new(),
    };
    let alias_actor = |role: Role| all.iter().position(|a| a.role == role).unwrap();
    let quests = vec![
        Quest {
            object_id: QUEST,
            edid: "VrmAutoTest9Quest",
            flags: 0x0011,
            priority: 0,
            kind: 0,
            script: Some(AttachedScript {
                name: SCRIPT.into(),
                properties,
            }),
            stages: &[],
            aliases: ALIAS_ROLES
                .iter()
                .enumerate()
                .map(|(i, &role)| unique_alias(i as u32, alias_actor(role)))
                .collect(),
        }
        .record(),
        Quest {
            object_id: QUEST_B,
            edid: "VrmAutoTest9LateQuest",
            flags: 0,
            priority: 0,
            kind: 0,
            script: None,
            stages: &[],
            aliases: vec![unique_alias(0, alias_actor(Role::AliasPlain))],
        }
        .record(),
    ];

    // The references by place: persistent (the entry, markers and
    // persistent actors) and temporary.
    let marker = |id: FormId, at: [f32; 3]| {
        RecordBuilder::new(b"REFR", id)
            .flags(persistent)
            .form(b"NAME", heading_marker)
            .field(b"DATA", &placement(at, 0.0))
    };
    let mut by_place: Vec<(Vec<RecordBuilder>, Vec<RecordBuilder>)> = spots
        .iter()
        .enumerate()
        .map(|(p, (_, s))| (vec![marker(own(ENTRIES + p as u32), s.entry)], Vec::new()))
        .collect();
    let mut used = vec![0; PLACES.len()];
    let listed = alias_actor(Role::AliasListed);
    for (k, a) in all.iter().enumerate() {
        let (spot, at) = spots[a.place].1.pairs[used[a.place]];
        used[a.place] += 1;
        let (persistent_refs, temporary_refs) = &mut by_place[a.place];
        persistent_refs.push(marker(own(MARKERS + k as u32), at));
        let mut actor =
            RecordBuilder::new(b"ACHR", own(ACTORS + k as u32)).form(b"NAME", own(NPCS + k as u32));
        if k == listed {
            actor = actor.form(b"XLCN", location);
        }
        let actor = actor.field(b"DATA", &placement(spot, 0.0));
        if a.role.persistent() {
            persistent_refs.push(actor.flags(persistent));
        } else {
            temporary_refs.push(actor);
        }
    }

    let mut interiors = Vec::new();
    let mut worlds = Vec::new();
    for ((p, (_, place)), (persistent, temporary)) in PLACES.iter().enumerate().zip(by_place) {
        match *place {
            Cell::Interior(_, name) => interiors.push(Interior {
                cell: spots[p].0,
                name,
                persistent,
                temporary,
            }),
            Cell::Exterior(world, name, grid) => worlds.push(World {
                world: v.id(world)?,
                name,
                grid,
                persistent,
                temporary,
            }),
        }
    }

    // The trader's location listing AliasListed among its unique actors
    // (`ACUN`: the NPC, the reference, the location).
    let acun: Vec<u8> = [
        own(NPCS + listed as u32),
        own(ACTORS + listed as u32),
        location,
    ]
    .iter()
    .flat_map(|f| f.0.to_le_bytes())
    .collect();
    let locations = vec![places::override_of(
        lo,
        location,
        Some(TRADER_LOCATION.1),
        &[],
        &[(b"ACUN", acun)],
    )?];

    // Top groups in the Creation Kit's order.
    let plugin = write_plugin(
        0,
        &["Skyrim.esm"],
        "vibe_rm",
        "Which actors the game moves by their packages while unloaded, by distance from the \
         player, and which fill unique-actor aliases: test actors in five places, read by script \
         on loading a save as the player is taken to each, reports saved.",
        NEXT_OBJECT_ID,
        &[
            Node::top(b"NPC_", npcs),
            places::interiors(lo, interiors)?,
            places::worlds(lo, worlds)?,
            Node::top(b"QUST", quests),
            Node::top(b"PACK", packages),
            Node::top(b"LCTN", locations),
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
            ("XMarker".to_owned(), form(0x3B, "Form")),
            ("QuestB".to_owned(), form(own(QUEST_B).0, "Quest")),
        ];
        for (p, (_, place)) in PLACES.iter().enumerate() {
            properties.push((
                format!("Entry{p}"),
                form(own(ENTRIES + p as u32).0, "ObjectReference"),
            ));
            if let Cell::Interior(..) = place {
                properties.push((format!("Cell{p}"), form(0x133C9 + p as u32, "Cell")));
            }
        }
        for k in 0..actors().len() {
            properties.push((
                format!("Marker{k}"),
                form(own(MARKERS + k as u32).0, "ObjectReference"),
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
            "as the quest starts: AliasPersistent holds ",
            "; AliasListed holds ",
            "10 s after loading, real time ",
            "  OtherworldRead: 1 1 1, heading 1, from its marker 1 (height 0), 3D False",
            "every actor's 3D after -1 s",
            "second quest, started now: AliasPlain holds ",
            "  AliasListed: 1 1 1",
            "  FarUnread: 1 1 1",
            "3 s later:\n  OtherworldPersistent: ",
        ] {
            assert!(all.contains(expected), "{expected:?} not in the reports");
        }
        assert!(!all.contains("10 s after loading, real time 5:\n  NearUnread"));
        // Each box fits on the screen.
        for b in &shown.boxes {
            assert!(b.lines().count() <= 20, "{} lines:\n{b}", b.lines().count());
        }
    }

    #[test]
    fn object_ids_fit() {
        let n = actors().len() as u32;
        assert!(NPCS + n <= PACKAGES);
        assert!(PACKAGES + n <= MARKERS);
        assert!(MARKERS + n <= ENTRIES);
        assert!(ENTRIES + PLACES.len() as u32 <= ACTORS);
        assert!(ACTORS + n <= NEXT_OBJECT_ID);
    }

    #[test]
    fn unread_actors_are_not_read_while_unloaded() {
        for a in actors() {
            assert_eq!(
                a.role.read_early(),
                matches!(a.role, Role::Persistent | Role::Read)
            );
        }
        assert_eq!(actors().iter().filter(|a| a.role.persistent()).count(), 6);
    }
}
