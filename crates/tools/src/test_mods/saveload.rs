//! `test-mod saveload`: what Skyrim SE keeps, across a save and its load,
//! of NPCs that are loaded near the player and busy when the save is made,
//! and of a script's looping sound (`plans/saves-remaining.md`, step 9;
//! `known_ambiguities/saves.md`, `known_ambiguities/remaining-natives.md`
//! "Sound instances"). On load our engine restarts every actor's
//! execution as a cell reload does (`Engine::away`, `restore_executions`):
//! seated Sit-package actors stand by their seats, sandboxers pick anew,
//! casts and swings in progress are dropped and behaviour graphs start
//! from the actor's equipment and stance. autotest14 reloaded a save only
//! for actors away from the player, so nothing says what SE does with
//! actors loaded at save time. Script sounds don't play on after our load.
//!
//! Where: the developer cell `WarehouseFurniture` (navmeshed, furniture, no
//! actors of its own), as autotest21's fights. Eight NPCs of ours stand
//! there (persistent, the commoner template's traits, unaggressive and
//! foolhardy so the fight doesn't scatter the others), each logging its
//! package, furniture, combat and 3D events into its own `ResultEvents`:
//!
//! | actor         | state at the save                                   | how                                   |
//! | ------------- | --------------------------------------------------- | ------------------------------------- |
//! | **Sitter**    | seated in the warehouse's chair (`CommonChair01F`)  | a Sit package (`MS13CamillaWorkTrader` copied) |
//! | **Sleeper**   | asleep in a bedroll of ours it owns                 | `DefaultSleepAnytimeNoLockNoWarn` copied, near the bedroll |
//! | **Sandboxer** | sandboxing (an idle marker, furniture or wandering) | `DefaultSandboxEditorLocation512`    |
//! | **Sneaker**   | sneaking, holding its place                         | a hold package flagged Always Sneak (`StartSneaking` if not sneaking in 3 s) |
//! | **Fighter**   | fighting Target with an iron sword                  | `StartCombat`, a hold package         |
//! | **Target**    | held: AI off, 100,000 health                        | `EnableAI(false)` (autotest21: stays in combat, can't act) |
//! | **Caster**    | UseMagic practice: Flames at a marker               | `WCollegeOnmundPracticeFlames12x4` copied |
//! | **Walker**    | halfway along a Travel walk                         | a Travel package under `VrmSaveLoadGo == 1` |
//!
//! The run (`saveload/saveload.psc`, 5 s after a save loads):
//!
//! 1. **Setup**: the player moved into the warehouse; the states started;
//!    up to 90 s for each (sit state 3, sleep state 3, sneaking, combat
//!    state 1, `IsCastingRight`), then 5 s more; everyone read.
//! 2. **Before**: the walker sent off and caught at half its walk; a looping
//!    2D sound (`AMBChargenExecutionWalla2DLP`, a crowd) started with
//!    `Sound.Play`, its instance id kept; everyone read (**BeforeGraph**:
//!    graph variables). A box asks whether the loop is audible; once it's
//!    closed the game saves, and the user loads that save. The script goes
//!    on in the loaded game: a jump of over 5 s in real time (or real time
//!    going back, after a restart) means a load (**Save**).
//! 3. **Load0**, **Load2**, **Load10**: everyone read at once, 2 s and 10 s
//!    after the load (**Load0Graph**, **Load10Graph**: graph variables).
//!    Each actor's line: sit and sleep states, `IsSneaking`,
//!    `IsWeaponDrawn`, `GetCombatState`, package, 3D, its move (XY / Z) and
//!    turn since the save request, and Fighter's combat target, Target's
//!    health, Caster's magicka or the walker's distances. Graph lines:
//!    `iState`, `IsSneaking`, `bIsSynced`, `bAnimationDriven`,
//!    `iSyncIdleLocomotion`, `i1HMState`, `iRightHandType`, `IsAttacking`,
//!    `IsCastingRight`, `IsEquipping`, `IsStaggering`, `iGetUpType`,
//!    `Speed` (each is a variable of the vanilla humanoid project,
//!    `vrm-tool hkb-vars`).
//! 4. **Sound**: a box asks whether the loop played on after the load;
//!    `Sound.StopInstance` on the kept id; a box asks whether it stopped.
//!    Then the game saves and the done box comes up.
//!
//! | reading                                                 | means                                           |
//! | ------------------------------------------------------- | ----------------------------------------------- |
//! | Sitter sit3 at Load0, moved 0                           | furniture is kept seated through a load         |
//! | Sitter sit0 near its chair, then sitting again          | furniture restarts as on a cell reload (ours)   |
//! | Sleeper slp3 at Load0                                   | sleep is kept                                   |
//! | Sandboxer in the same place and package at Load0        | sandbox idles are kept                          |
//! | Sneaker snk1 at Load0                                   | sneaking is kept                                |
//! | Fighter cmb1, tgt1, drw1 at Load0                       | a fight is kept, weapon drawn                   |
//! | Fighter cmb0 at Load0, fighting again later             | combat is re-acquired after the load            |
//! | Caster castR 1 at Load0                                 | a cast in progress is kept                      |
//! | Walker moved 0 at Load0 and walking on (toFar falling)  | a walk is kept where it was                     |
//! | Walker at its far marker or home at Load0               | a walk is finished or restarted on load         |
//! | `OnPackageStart` / `OnSit` logged after the load         | the load restarts packages / furniture          |
//! | the loop heard after the load, stopped by its old id    | script sounds are saved with their ids          |
//! | the loop silent after the load                          | script sounds end on load (ours)                |
//!
//! The plugin must be named `VrmSaveLoad.esp`. Load a throwaway save
//! anywhere (not in combat), without other test plugins, and **stay at the
//! keyboard**: the run takes about four minutes and asks four things (three
//! by ear). Don't keep playing with the plugin: it carries Skyrim.esm's
//! record of the warehouse cell.
//!
//! **Manual checklist** (optional, after the done box; no script reads it):
//! try to save (the journal's Save, and a quicksave with F5) in each of
//! these, and note which bring up "You cannot save right now"
//! (`sCantSaveNow`) or another message: in combat with a hostile (a wolf
//! or bandit outdoors), in the air mid-jump or falling, in dialogue with
//! an NPC, during a scene (an NPC conversation going on nearby) and while
//! sitting in a chair. Note the exact text of each.
//!
//! **Checkpoints.** The game saves after Setup (a crash there keeps its
//! report) and at the end. Between them nothing saves, so the walker is
//! caught halfway and the readings after the load are timed from it. If the
//! game crashes during Setup, load the newest save, open the console at
//! once and enter `set VrmSaveLoadSkipTest to 1`; `3` skips the sound
//! questions. The Before, load and graph readings can't be skipped.
//!
//! A dry run in our engine (`vibe_rm --plugin tests/assets/VrmSaveLoad.esp
//! --loose tests/assets --cell Riverwood --screenshot <png> --wait 30000`,
//! traces at `RUST_LOG=vibe_rm::script::natives=debug`; offscreen answers
//! the boxes and loads nothing, so the readings after "NOT LOADED" are of
//! the live game) reaches the done box. In ours Sitter sat at 0.25 s,
//! Sleeper slept at 0.75 s, Sneaker sneaked at 0.25 s, Fighter fought, and
//! the walker was caught at 1,174 of 2,342. One state never came: Caster,
//! weapon drawn on its practice package, never read `IsCastingRight`. Its
//! SE readings still count; the setup line gives -1 for a state not
//! reached. (Sneaker first didn't sneak: the graph took `SneakStart`, but
//! ours read the stance back from the graph's `IsSneaking`, which no
//! vanilla graph writes. Fixed 2026-11-17: the engine writes it.)
//!
//! Not run in Skyrim SE yet.

use anyhow::{Context, Result};
use esp::writer::{AttachedScript, Node, PropertyValue, RecordBuilder, write_plugin};
use esp::{FormId, LoadOrder, record_flags};

use super::autotest2::{Npc, Vanilla};
use super::autotest6::{ALWAYS, pldt};
use super::autotest13::{WALK, package};
use super::places::{self, Interior};
use super::records::{self, Quest};
use super::venv::placement;
use super::{TestMod, own, script};

const QUEST: u32 = 0x800;
const GO: u32 = 0x801;
const SKIP_TEST: u32 = 0x802;
const ASK_HEAR: u32 = 0x803;
const ASK_HEARD_AFTER: u32 = 0x804;
const ASK_STOPPED: u32 = 0x805;
const BED: u32 = 0x807;
const ENTRY: u32 = 0x808;
const CAST_TARGET: u32 = 0x809;
const WALK_FAR: u32 = 0x80A;
const HOMES: u32 = 0x810;
const NPCS: u32 = 0x820;
const ACTORS: u32 = 0x830;
const PKG_SIT: u32 = 0x840;
const PKG_SLEEP: u32 = 0x841;
const PKG_CAST: u32 = 0x842;
const PKG_WALK_FAR: u32 = 0x843;
/// Hold packages at home: Walker, Sneaker, Fighter, Target.
const PKG_HOMES: u32 = 0x848;
const NEXT_OBJECT_ID: u32 = 0x850;

const SCRIPT: &str = "VrmSaveLoadScript";
const ACTOR_SCRIPT: &str = "VrmSaveLoadActorScript";
const SOURCE: &str = include_str!("saveload/saveload.psc");
const ACTOR_SOURCE: &str = include_str!("saveload/actor.psc");
const PLAYER_REF: FormId = FormId(0x14);

/// The actors, in the script's order.
const ACTORS_LIST: [&str; 8] = [
    "Sitter",
    "Sleeper",
    "Sandboxer",
    "Sneaker",
    "Fighter",
    "Target",
    "Caster",
    "Walker",
];

/// The hold packages' owners and the script properties naming them.
const HOLDERS: [(&str, &str); 4] = [
    ("Walker", "PkgWalkHome"),
    ("Sneaker", "PkgSneakHome"),
    ("Fighter", "PkgFightHome"),
    ("Target", "PkgTargetHome"),
];

/// `PKDT` general flag: Always Sneak.
const ALWAYS_SNEAK: u32 = 1 << 17;

fn k_of(name: &str) -> usize {
    ACTORS_LIST.iter().position(|n| *n == name).unwrap()
}

/// The questions asked by ear: name, text, buttons.
const QUESTIONS: [(u32, &str, &str, &[&str]); 3] = [
    (
        ASK_HEAR,
        "AskHear",
        "vibe_rm saveload: a looping crowd murmur should be playing now. Do you hear it? \
         When you close this box the game saves. Then load that save (the newest) from the \
         menu straight away; the test goes on by itself after the load.",
        &["Yes", "No", "Not sure"],
    ),
    (
        ASK_HEARD_AFTER,
        "AskHeardAfter",
        "vibe_rm saveload: since the load, have you heard the crowd murmur that was playing \
         before the save?",
        &["Yes", "No", "Not sure"],
    ),
    (
        ASK_STOPPED,
        "AskStopped",
        "vibe_rm saveload: the script has just stopped the crowd murmur by the id it had \
         before the save. Is the murmur still playing?",
        &["Still playing", "It stopped", "It wasn't playing"],
    ),
];

fn scripts() -> Result<Vec<(String, Vec<u8>)>> {
    script::compile(&[SOURCE, ACTOR_SOURCE])
}

fn xy(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// The nearest of `of` to `p`, in XY.
fn clearance(p: [f32; 3], of: &[[f32; 3]]) -> f32 {
    of.iter().map(|c| xy(p, *c)).fold(f32::MAX, f32::min)
}

/// Where everything goes in the warehouse.
struct Layout {
    entry: [f32; 3],
    homes: [[f32; 3]; 8],
    bed: [f32; 3],
    cast_target: [f32; 3],
    walk_far: [f32; 3],
}

/// The warehouse's references, in XY: kept clear of.
fn references(lo: &LoadOrder, cell: FormId) -> Result<Vec<[f32; 3]>> {
    let index = lo.cell(cell).context("not a cell")?;
    let mut out = Vec::new();
    for &r in index.persistent.iter().chain(&index.temporary) {
        let Some(d) = lo
            .get(r)
            .and_then(|rec| rec.get(b"DATA").map(|d| d.to_vec()))
        else {
            continue;
        };
        if d.len() >= 12 {
            let f = |o: usize| f32::from_le_bytes(d[o..o + 4].try_into().unwrap());
            out.push([f(0), f(4), f(8)]);
        }
    }
    Ok(out)
}

/// The warehouse's own chair (`CommonChair01F`): the Sit package's.
const CHAIR: FormId = FormId(0x0008_0B19);

/// `reference`'s position (its `DATA`).
fn position(lo: &LoadOrder, reference: FormId) -> Result<[f32; 3]> {
    let d = lo
        .get(reference)
        .and_then(|r| r.get(b"DATA").map(|d| d.to_vec()))
        .with_context(|| format!("no reference {reference}"))?;
    anyhow::ensure!(d.len() >= 12, "{reference}: short DATA");
    let f = |o: usize| f32::from_le_bytes(d[o..o + 4].try_into().unwrap());
    Ok([f(0), f(4), f(8)])
}

fn layout(lo: &LoadOrder, cell: FormId, chair: [f32; 3]) -> Result<Layout> {
    let floors = places::floors(lo, cell)?;
    let refs = references(lo, cell)?;
    // The best floor by `score` of those `fits` allows.
    let best = |fits: &dyn Fn([f32; 3]) -> bool, score: &dyn Fn([f32; 3]) -> f32| {
        floors
            .iter()
            .copied()
            .filter(|p| fits(*p))
            .max_by(|a, b| score(*a).total_cmp(&score(*b)))
    };
    let n = floors.len() as f32;
    let mid = [0, 1].map(|i| floors.iter().map(|f| f[i]).sum::<f32>() / n);
    let entry = best(&|_| true, &|p| -xy(p, [mid[0], mid[1], 0.0])).context("no floor")?;
    // The walk: the two floors farthest apart, clear of the references and
    // away from the chair.
    let open: Vec<[f32; 3]> = floors
        .iter()
        .copied()
        .filter(|p| clearance(*p, &refs) >= 80.0 && xy(*p, chair) >= 400.0)
        .collect();
    let mut walk = None;
    let mut longest = 0.0;
    for a in &open {
        for b in &open {
            let d = xy(*a, *b);
            if d > longest {
                longest = d;
                walk = Some((*a, *b));
            }
        }
    }
    let (walk_home, walk_far) = walk.context("no walk")?;
    let mut taken = vec![entry, walk_home, walk_far, chair];
    // Open space: far from the references and what's placed.
    let room = |p: [f32; 3], taken: &[[f32; 3]]| clearance(p, &refs).min(clearance(p, taken));
    let pick = |taken: &mut Vec<[f32; 3]>,
                what: &str,
                fits: &dyn Fn([f32; 3]) -> bool|
     -> Result<[f32; 3]> {
        let t = taken.clone();
        let p = best(&|p| fits(p) && clearance(p, &t) >= 120.0, &|p| room(p, &t))
            .with_context(|| format!("no room in the warehouse for {what}"))?;
        taken.push(p);
        Ok(p)
    };
    let sitter = pick(&mut taken, "the sitter", &|p| {
        (120.0..=300.0).contains(&xy(p, chair))
    })?;
    let bed = pick(&mut taken, "the bedroll", &|p| {
        xy(p, chair) >= 400.0 && xy(p, walk_home).min(xy(p, walk_far)) >= 300.0
    })?;
    let sleeper = pick(&mut taken, "the sleeper", &|p| {
        (120.0..=300.0).contains(&xy(p, bed))
    })?;
    // The fight as far from the chair and bedroll as can be, and away from
    // both ends of the walk.
    let t = taken.clone();
    let off_walk = |p: [f32; 3]| xy(p, walk_home).min(xy(p, walk_far)) >= 350.0;
    let fighter = best(&|p| clearance(p, &t) >= 120.0 && off_walk(p), &|p| {
        xy(p, chair).min(xy(p, bed)) + room(p, &t).min(150.0)
    })
    .context("no room for the fight")?;
    taken.push(fighter);
    let target = pick(&mut taken, "the target", &|p| {
        (150.0..=250.0).contains(&xy(p, fighter)) && off_walk(p)
    })?;
    let sandboxer = pick(&mut taken, "the sandboxer", &|_| true)?;
    let sneaker = pick(&mut taken, "the sneaker", &|_| true)?;
    let caster = pick(&mut taken, "the caster", &|p| xy(p, fighter) >= 300.0)?;
    let cast_target = pick(&mut taken, "the cast target", &|p| {
        (400.0..=700.0).contains(&xy(p, caster))
    })?;
    Ok(Layout {
        entry,
        homes: [
            sitter, sleeper, sandboxer, sneaker, fighter, target, caster, walk_home,
        ],
        bed,
        cast_target,
        walk_far,
    })
}

/// Caster's UseMagic practice: Flames at our marker, as `spell-visuals`
/// copies Onmund's.
fn cast_package(lo: &LoadOrder, flames: FormId, home: FormId) -> Result<RecordBuilder> {
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
    let value = std::cell::Cell::new(0);
    records::copy(
        lo,
        "WCollegeOnmundPracticeFlames12x4",
        PKG_CAST,
        "VrmSaveLoadCast",
        None,
        |tag, d| match tag {
            b"PLDT" => *d = pldt(0, home, 32),
            b"PSDT" => *d = ALWAYS.to_vec(),
            b"CTDA" | b"CIS2" => d.clear(),
            // Target selectors: type 1 a specific form (the spell), type 0
            // a reference (the target).
            b"PTDA" if d.len() >= 8 => {
                let form = if d[0] == 1 { flames } else { own(CAST_TARGET) };
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
    )
}

/// Clears `PKDT`'s "offers services" flag (0x1).
fn no_services(d: &mut [u8]) {
    if d.len() >= 4 {
        d[0] &= !0x1;
    }
}

pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    let v = Vanilla { lo };
    let warehouse = v.id("WarehouseFurniture")?;
    let template = v.id("WEMerchantMiscMaleCommoner")?;
    let race = v.id("ImperialRace")?;
    let class = v.id("Citizen")?;
    let heading_marker = v.id("XMarkerHeading")?;
    let x_marker = v.id("XMarker")?;
    let flames = v.id("Flames")?;
    let sandbox = v.id("DefaultSandboxEditorLocation512")?;
    let persistent = record_flags::PERSISTENT;
    anyhow::ensure!(
        lo.get(CHAIR).and_then(|r| r
            .get(b"NAME")
            .map(|d| r.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap())))))
            == Some(v.id("CommonChair01F")?),
        "{CHAIR} isn't the warehouse's CommonChair01F"
    );
    let at = layout(lo, warehouse, position(lo, CHAIR)?)?;

    // Packages, each actor's list first to last.
    let mut packages = vec![
        records::copy(
            lo,
            "MS13CamillaWorkTrader",
            PKG_SIT,
            "VrmSaveLoadSit",
            None,
            |tag, d| match tag {
                b"PLDT" => *d = pldt(0, CHAIR, 0),
                b"PSDT" => *d = ALWAYS.to_vec(),
                b"PKDT" => no_services(d),
                _ => {}
            },
        )?,
        // Near the bedroll (its owner's), not the editor location's 1000.
        records::copy(
            lo,
            "DefaultSleepAnytimeNoLockNoWarn",
            PKG_SLEEP,
            "VrmSaveLoadSleep",
            None,
            |tag, d| match tag {
                b"PLDT" => *d = pldt(0, own(BED), 150),
                b"PSDT" => *d = ALWAYS.to_vec(),
                _ => {}
            },
        )?,
        cast_package(lo, flames, own(HOMES + k_of("Caster") as u32))?,
        package(
            lo,
            PKG_WALK_FAR,
            "VrmSaveLoadWalkFar",
            own(WALK_FAR),
            ALWAYS,
            Some(own(GO)),
            WALK,
        )?,
    ];
    for (i, (name, _)) in HOLDERS.iter().enumerate() {
        let home = own(HOMES + k_of(name) as u32);
        let edid = format!("VrmSaveLoad{name}Home");
        packages.push(if *name == "Sneaker" {
            // Holding its place, sneaking (`PKDT` flag 0x20000: Always Sneak).
            records::copy(
                lo,
                "MS13LucanHoldPositionPackage",
                PKG_HOMES + i as u32,
                &edid,
                None,
                |tag, d| match tag {
                    b"PLDT" => *d = pldt(0, home, 0),
                    b"PSDT" => *d = ALWAYS.to_vec(),
                    b"CTDA" | b"CIS2" => d.clear(),
                    b"PKDT" if d.len() >= 4 => {
                        let flags = u32::from_le_bytes(d[0..4].try_into().unwrap());
                        let flags = (flags & !0x1) | ALWAYS_SNEAK;
                        d[0..4].copy_from_slice(&flags.to_le_bytes());
                    }
                    _ => {}
                },
            )?
        } else {
            package(lo, PKG_HOMES + i as u32, &edid, home, ALWAYS, None, WALK)?
        });
    }
    let hold = |name: &str| {
        let i = HOLDERS.iter().position(|(n, _)| *n == name).unwrap();
        own(PKG_HOMES + i as u32)
    };
    let lists: [Vec<FormId>; 8] = [
        vec![own(PKG_SIT)],
        vec![own(PKG_SLEEP)],
        vec![sandbox],
        vec![hold("Sneaker")],
        vec![hold("Fighter")],
        vec![hold("Target")],
        vec![own(PKG_CAST)],
        vec![own(PKG_WALK_FAR), hold("Walker")],
    ];

    let actor_script = AttachedScript {
        name: ACTOR_SCRIPT.into(),
        properties: Vec::new(),
    };
    let npcs: Vec<RecordBuilder> = ACTORS_LIST
        .iter()
        .enumerate()
        .map(|(k, name)| {
            Npc {
                object_id: NPCS + k as u32,
                edid: format!("VrmSaveLoad{name}Npc"),
                name: Some("vibe_rm Test NPC"),
                level: (10, false, 1, 0),
                template: (template, 0x1),
                // Unaggressive, foolhardy: bystanders don't flee the fight.
                temper: (0, 4),
                factions: Vec::new(),
                script: Some(actor_script.clone()),
                assistance: 0,
                combat_style: None,
                unique: true,
            }
            .record_with_packages(race, class, &lists[k])
        })
        .collect();

    let object = PropertyValue::Object;
    let mut properties = vec![
        ("Player".to_owned(), object(PLAYER_REF)),
        ("Entry".to_owned(), object(own(ENTRY))),
        ("Chair".to_owned(), object(CHAIR)),
        ("Bed".to_owned(), object(own(BED))),
        (
            "WalkerHome".to_owned(),
            object(own(HOMES + k_of("Walker") as u32)),
        ),
        ("WalkerFar".to_owned(), object(own(WALK_FAR))),
        ("Go".to_owned(), object(own(GO))),
        ("SkipTest".to_owned(), object(own(SKIP_TEST))),
        (
            "Loop".to_owned(),
            object(v.id("AMBChargenExecutionWalla2DLP")?),
        ),
        ("Flames".to_owned(), object(flames)),
        ("Sword".to_owned(), object(v.id("IronSword")?)),
        ("PkgSit".to_owned(), object(own(PKG_SIT))),
        ("PkgSleep".to_owned(), object(own(PKG_SLEEP))),
        ("PkgSandbox".to_owned(), object(sandbox)),
        ("PkgCast".to_owned(), object(own(PKG_CAST))),
        ("PkgWalkFar".to_owned(), object(own(PKG_WALK_FAR))),
    ];
    for (name, property) in HOLDERS {
        properties.push((property.to_owned(), object(hold(name))));
    }
    for (id, name, _, _) in QUESTIONS {
        properties.push((name.to_owned(), object(own(id))));
    }
    for (k, name) in ACTORS_LIST.iter().enumerate() {
        properties.push((name.to_string(), object(own(ACTORS + k as u32))));
    }
    let quest = Quest {
        object_id: QUEST,
        edid: "VrmSaveLoadQuest",
        flags: 0x11,
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

    let messages: Vec<RecordBuilder> = QUESTIONS
        .iter()
        .map(|(id, name, text, buttons)| {
            let mut r = RecordBuilder::new(b"MESG", own(*id))
                .zstring(b"EDID", &format!("VrmSaveLoad{name}"))
                .zstring(b"DESC", text)
                .zstring(b"FULL", "vibe_rm saveload")
                .u32(b"DNAM", 1)
                .u32(b"TNAM", 0);
            for b in *buttons {
                r = r.zstring(b"ITXT", b);
            }
            r
        })
        .collect();

    let marker = |id: u32, base: FormId, p: [f32; 3]| {
        RecordBuilder::new(b"REFR", own(id))
            .flags(persistent)
            .form(b"NAME", base)
            .field(b"DATA", &placement(p, 0.0))
    };
    let mut refs = vec![
        marker(ENTRY, heading_marker, at.entry),
        marker(CAST_TARGET, x_marker, lift(at.cast_target, 60.0)),
        marker(WALK_FAR, heading_marker, at.walk_far),
        RecordBuilder::new(b"REFR", own(BED))
            .flags(persistent)
            .form(b"NAME", v.id("Bedroll01")?)
            .form(b"XOWN", own(NPCS + k_of("Sleeper") as u32))
            .field(b"DATA", &placement(at.bed, 0.0)),
    ];
    for k in 0..ACTORS_LIST.len() {
        refs.push(marker(HOMES + k as u32, heading_marker, at.homes[k]));
        refs.push(
            RecordBuilder::new(b"ACHR", own(ACTORS + k as u32))
                .flags(persistent)
                .form(b"NAME", own(NPCS + k as u32))
                .field(b"DATA", &placement(at.homes[k], 0.0)),
        );
    }
    let cell = places::interiors(
        lo,
        vec![Interior {
            cell: warehouse,
            name: "Warehouse Furniture",
            persistent: refs,
            temporary: Vec::new(),
        }],
    )?;

    let plugin = write_plugin(
        0,
        &["Skyrim.esm"],
        "vibe_rm",
        "What a save and its load keep of busy NPCs near the player (sitting, asleep, \
         sandboxing, sneaking, fighting, practising magic, walking) and of a script's looping \
         sound. The user loads the save the script makes and answers three questions by ear; \
         reports saved.",
        NEXT_OBJECT_ID,
        &[
            Node::top(
                b"GLOB",
                vec![
                    records::global(GO, "VrmSaveLoadGo", 0.0),
                    records::global(SKIP_TEST, "VrmSaveLoadSkipTest", 0.0),
                ],
            ),
            Node::top(b"MESG", messages),
            Node::top(b"NPC_", npcs),
            cell,
            Node::top(b"QUST", vec![quest]),
            Node::top(b"PACK", packages),
        ],
    );
    Ok(TestMod {
        plugin,
        scripts: scripts()?,
    })
}

/// `p` raised by `dz`.
fn lift(p: [f32; 3], dz: f32) -> [f32; 3] {
    [p[0], p[1], p[2] + dz]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_mods::script::dry_run::{form, run};

    #[test]
    fn the_run_reaches_the_done_box_and_saves_before_it() {
        let mut properties = vec![("Player".to_owned(), form(0x14, "Actor"))];
        for (k, name) in ACTORS_LIST.iter().enumerate() {
            properties.push((name.to_string(), form(own(ACTORS + k as u32).0, "Actor")));
        }
        let objects = [
            ("Entry", "ObjectReference"),
            ("Chair", "ObjectReference"),
            ("Bed", "ObjectReference"),
            ("WalkerHome", "ObjectReference"),
            ("WalkerFar", "ObjectReference"),
            ("Go", "GlobalVariable"),
            ("SkipTest", "GlobalVariable"),
            ("Loop", "Sound"),
            ("Flames", "Spell"),
            ("Sword", "Weapon"),
            ("AskHear", "Message"),
            ("AskHeardAfter", "Message"),
            ("AskStopped", "Message"),
            ("PkgSit", "Package"),
            ("PkgSleep", "Package"),
            ("PkgSandbox", "Package"),
            ("PkgCast", "Package"),
            ("PkgWalkFar", "Package"),
            ("PkgWalkHome", "Package"),
            ("PkgSneakHome", "Package"),
            ("PkgFightHome", "Package"),
            ("PkgTargetHome", "Package"),
        ];
        for (i, (name, class)) in objects.iter().enumerate() {
            properties.push((name.to_string(), form(0x2000 + i as u32, class)));
        }
        // The opening box and the done box; no load comes in the dry run,
        // so it reads the live game after 180 s.
        let shown = run(scripts().unwrap(), &[(own(QUEST).0, SCRIPT, properties)], 2);
        assert_eq!(shown.boxes.len(), 2);
        assert!(shown.boxes[1].contains("done"));
        assert_eq!(shown.saved_at_box, Some(1), "saved before the done box");
    }

    #[test]
    fn object_ids_fit() {
        assert!(NPCS - HOMES >= ACTORS_LIST.len() as u32);
        assert!(ACTORS + ACTORS_LIST.len() as u32 <= PKG_SIT);
        assert!(PKG_HOMES + HOLDERS.len() as u32 <= NEXT_OBJECT_ID);
    }

    #[test]
    fn the_scripts_compile() {
        assert_eq!(scripts().unwrap().len(), 2);
    }
}
