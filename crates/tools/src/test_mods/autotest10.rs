//! `test-mod autotest10`: which vanilla unique actors fill a unique-actor
//! alias (`known_ambiguities/alias-fills.md`). In autotest8 and autotest9
//! only a test actor its location listed among its unique actors filled;
//! one flagged persistent but unlisted didn't, in a save or a new game.
//! Vanilla has about 600 unique-actor aliases for actors no winning
//! location record lists, which the engine fills.
//!
//! Locations list unique actors in Skyrim.esm as `LCUN`; an override from
//! Dawnguard or later as `ACUN`, and those overrides list fewer:
//! `RiverwoodLocation` lists 15 in Skyrim.esm (Delphine, Orgnar and
//! Faendal among them) and 7 in its Dawnguard override, the winner. Whether
//! the game reads only the winning record (so Delphine is unlisted in the
//! game) or merges each plugin's lists isn't documented; the engine
//! (`src/locations.rs`) reads only the winner.
//!
//! One quest of ours, starting as the save loads, with an optional
//! unique-actor alias for each of these (allowing reserved, dead and
//! disabled references, so only the fill rule is tested):
//!
//! | group                                    | actors                                     | the reference      |
//! | ---------------------------------------- | ------------------------------------------ | ------------------ |
//! | listed in the winning location record    | Aela, Balgruuf / Lod, Brill                 | persistent / not   |
//! | listed in Skyrim.esm only (`LCUN`, dropped by the winning override) | Tolfdir, Lurbuk, Delphine, Faendal / Orgnar, Leif Wayfinder | persistent / not |
//! | never listed, no `XLCN`                  | Brynjolf, Nils / two Helgen soldiers        | persistent / not (initially disabled) |
//! | no placed reference                      | Muril                                       | (the control: empty) |
//!
//! The readings: each alias as the quest starts (`OnUpdate`, 5 s after
//! loading), then 10 s later with its actor's reference looked up in
//! Skyrim.esm (`Game.GetFormFromFile`): found or not, disabled, dead.
//!
//! | reading                                    | means                                              |
//! | ------------------------------------------ | -------------------------------------------------- |
//! | Skyrim.esm-only actors filled              | the game merges the plugins' location lists       |
//! | Skyrim.esm-only actors empty               | only the winning record counts (vanilla Delphine unlisted) |
//! | Brynjolf, Nils filled                      | listing isn't needed for vanilla actors (what then?) |
//! | the never-listed empty                     | unique-actor aliases need a listing; vanilla's are left empty |
//!
//! Load any save (the aliases have no packages or scripts, so nothing
//! happens to the actors) and leave the game alone for about 20 s. The
//! plugin name doesn't matter.
//!
//! Run in Skyrim SE (2026-10-22) twice, in a new game (`coc Riverwood` from
//! the main menu) and from the test character's save outside Riverwood,
//! with the same readings line for line:
//!
//! - Every actor with its reference in memory filled, as the quest started:
//!   the listed, those listed only in Skyrim.esm (persistent or not) and
//!   the never listed (Brynjolf, Nils), none disabled or dead.
//! - The two Helgen soldiers (not persistent, initially disabled, never
//!   listed) stayed empty, and the game had no reference for them in
//!   memory (`GetFormFromFile` None), though the aliases allow disabled
//!   ones. **Muril** (no reference) stayed empty.
//!
//! So a vanilla unique actor fills whether a location lists it or not, and
//! this says nothing of whether the plugins' location lists merge. That
//! leaves autotest8's and autotest9's unlisted test actors (in memory,
//! one persistent) unexplained: what sets them apart from Brynjolf is
//! that our plugin defines them (an `.esp`, not a master), as far as
//! these tests go (`known_ambiguities/alias-fills.md`).

use anyhow::{Context, Result};
use esp::writer::{AttachedScript, Node, write_plugin};
use esp::{FormId, LoadOrder};
use papyrus::build::{Arg, FunctionBuilder, ScriptBuilder, id, string};
use papyrus::pex::op;

use super::TestMod;
use super::autotest2::{Vanilla, line, put};
use super::autotest4::report;
use super::records::{Alias, Fill, Quest, alias_flags};
use super::script::{Code, on_update};

const QUEST: u32 = 0x800;
const NEXT_OBJECT_ID: u32 = 0x801;
const SCRIPT: &str = "VrmAutoTest10Script";

/// Alias flags beyond the shared ones: allow dead, disabled and reserved.
const ALLOW_DEAD: u32 = 0x10;
const ALLOW_DISABLED: u32 = 0x80;
const ALLOW_RESERVED: u32 = 0x200;

/// An actor: its label, the NPC's editor id, its group.
const ACTORS: &[(&str, &str, &str)] = &[
    ("Aela", "AelaTheHuntress", "listed, P"),
    ("Balgruuf", "BalgruuftheGreater", "listed, P"),
    ("Lod", "Lod", "listed"),
    ("Brill", "Brill", "listed"),
    ("Tolfdir", "Tolfdir", "Skyrim.esm-listed, P"),
    ("Lurbuk", "Lurbuk", "Skyrim.esm-listed, P"),
    ("Delphine", "Delphine", "Skyrim.esm-listed, P"),
    ("Faendal", "Faendal", "Skyrim.esm-listed, P"),
    ("Orgnar", "Orgnar", "Skyrim.esm-listed"),
    ("Leif", "LeifWayfinder", "Skyrim.esm-listed"),
    ("Brynjolf", "Brynjolf", "never listed, P"),
    ("Nils", "Nils", "never listed, P"),
    (
        "HelgenArcher",
        "dunCGImperialSoldierArcherA01",
        "never listed, disabled",
    ),
    (
        "HelgenSoldier",
        "dunCGImperialSoldierFodderC01",
        "never listed, disabled",
    ),
    ("Muril", "Muril", "no reference"),
];

const TESTS: &[(&str, &str)] = &[
    ("Start", "Unique-actor aliases as the quest starts"),
    ("Later", "The aliases 10 s later, and their references"),
];

/// Alias `i`'s line: what it holds.
fn alias_held(c: &mut Code, r: &Arg, i: usize) {
    let a = c.method("Quest", id("self"), "GetAlias", [(i as i32).into()]);
    let ra = c.temp("ReferenceAlias");
    c.cast(ra.clone(), a);
    let held = c.method("ReferenceAlias", ra, "GetReference", []);
    let (label, _, group) = ACTORS[i];
    put(c, r, &format!("  {label} ({group}): "), held);
}

fn test_start() -> FunctionBuilder {
    let (mut c, r) = report();
    for i in 0..ACTORS.len() {
        alias_held(&mut c, &r, i);
        line(&mut c, &r, "\n");
    }
    c.finish(r)
}

/// 10 s on: each alias, and its actor's reference (`refs[i]`, an object
/// id in Skyrim.esm) found or not, disabled, dead.
fn test_later(refs: &[Option<u32>]) -> FunctionBuilder {
    let (mut c, r) = report();
    c.wait(10.0);
    for (i, reference) in refs.iter().enumerate() {
        alias_held(&mut c, &r, i);
        let Some(object_id) = reference else {
            line(&mut c, &r, "\n");
            continue;
        };
        let f = c.global(
            "Game",
            "GetFormFromFile",
            [(*object_id as i32).into(), string("Skyrim.esm")],
        );
        let a = c.temp("Actor");
        c.cast(a.clone(), f);
        let none = c.temp("Actor");
        c.cast(none.clone(), Arg::None);
        let missing = c.calc(op::CMP_EQ, "Bool", a.clone(), none);
        let (found, next) = (c.label(), c.label());
        c.jmpf(missing, found);
        line(&mut c, &r, "; reference not in memory\n");
        c.jmp(next);
        c.bind(found);
        let disabled = c.method("ObjectReference", a.clone(), "IsDisabled", []);
        put(&mut c, &r, "; disabled ", disabled);
        let dead = c.method("Actor", a, "IsDead", []);
        put(&mut c, &r, ", dead ", dead);
        line(&mut c, &r, "\n");
        c.bind(next);
    }
    c.finish(r)
}

fn scripts(refs: &[Option<u32>]) -> Result<Vec<(String, Vec<u8>)>> {
    let mut s = ScriptBuilder::new(SCRIPT, "Quest");
    s.variable("Ran", "Bool", false.into());
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
        .function("OnUpdate", on_update("autotest10", "20 seconds", TESTS))
        .function("TestStart", test_start())
        .function("TestLater", test_later(refs));
    Ok(vec![(SCRIPT.to_owned(), papyrus::pex::write(&s.build()?)?)])
}

/// Each actor's NPC and its first placed reference in Skyrim.esm.
fn actors(lo: &LoadOrder) -> Result<Vec<(FormId, Option<FormId>)>> {
    let v = Vanilla { lo };
    let mut placed = std::collections::HashMap::new();
    for &a in lo.ids_of_type(b"ACHR") {
        let rec = lo.get(a).context("reference")?;
        if let Some(d) = rec.get(b"NAME").filter(|d| d.len() >= 4) {
            let base = rec.fid(FormId(u32::from_le_bytes(d[..4].try_into().unwrap())));
            placed.entry(base).or_insert(a);
        }
    }
    ACTORS
        .iter()
        .map(|&(_, npc, _)| {
            let npc = v.id(npc)?;
            Ok((npc, placed.get(&npc).copied()))
        })
        .collect()
}

pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    let actors = actors(lo)?;
    let flags = alias_flags::OPTIONAL | ALLOW_DEAD | ALLOW_DISABLED | ALLOW_RESERVED;
    let aliases = ACTORS
        .iter()
        .zip(&actors)
        .enumerate()
        .map(|(i, (&(label, _, _), &(npc, _)))| Alias {
            id: i as u32,
            name: label,
            flags,
            fill: Fill::Unique(npc),
            packages: Vec::new(),
        })
        .collect();
    let quests = vec![
        Quest {
            object_id: QUEST,
            edid: "VrmAutoTest10Quest",
            flags: 0x0011,
            priority: 0,
            kind: 0,
            script: Some(AttachedScript {
                name: SCRIPT.into(),
                properties: Vec::new(),
            }),
            stages: &[],
            aliases,
        }
        .record(),
    ];
    let refs: Vec<Option<u32>> = actors.iter().map(|(_, r)| r.map(|r| r.0)).collect();
    let plugin = write_plugin(
        0,
        &["Skyrim.esm"],
        "vibe_rm",
        "Which vanilla unique actors fill a unique-actor alias: a quest's aliases for actors \
         listed by their location or not, read by script on loading a save, reports saved.",
        NEXT_OBJECT_ID,
        &[Node::top(b"QUST", quests)],
    );
    Ok(TestMod {
        plugin,
        scripts: scripts(&refs)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_mods::own;
    use crate::test_mods::script::dry_run::run;

    #[test]
    fn the_tests_run_and_report() {
        // Made-up references; the last actor has none.
        let refs: Vec<Option<u32>> = (0..ACTORS.len())
            .map(|i| (i + 1 < ACTORS.len()).then_some(0x1000 + i as u32))
            .collect();
        let shown = run(
            scripts(&refs).unwrap(),
            &[(own(QUEST).0, SCRIPT, Vec::new())],
            TESTS.len(),
        );
        assert_eq!(shown.boxes.len(), TESTS.len(), "every test reports");
        assert_eq!(shown.saved_at_box, Some(0), "saves before the boxes");
        let all = shown.boxes.join("\n");
        for expected in [
            "  Aela (listed, P): None\n",
            "  Delphine (Skyrim.esm-listed, P): None; disabled False, dead False\n",
            "  Muril (no reference): None\n",
        ] {
            assert!(all.contains(expected), "{expected:?} not in the reports");
        }
        for b in &shown.boxes {
            assert!(b.lines().count() <= 20, "{} lines:\n{b}", b.lines().count());
        }
    }
}
