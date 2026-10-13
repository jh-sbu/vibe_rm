//! Records the tier 2 test mod writes beyond NPCs, factions and plain quests:
//! conditions, spells, perks, keywords, quests with aliases and stages,
//! Story Manager nodes, scenes, shouts and copies of vanilla records. Each
//! is laid out as the engine's reader takes it (`src/magic.rs`,
//! `src/perks.rs`, `src/aliases.rs`, `crates/esp/src/story.rs` and
//! `scene.rs`), after the game's own records.

use anyhow::{Context, Result};
use esp::writer::{AttachedScript, RecordBuilder, vmad};
use esp::{FormId, LoadOrder};

use super::own;
use super::script::bytes;

/// Condition comparisons.
pub mod op {
    pub const EQ: u8 = 0;
    pub const GE: u8 = 3;
}

/// Condition functions (`src/condition/functions.rs`).
pub mod func {
    pub const GET_ACTOR_VALUE: u16 = 14;
    pub const GET_STAGE: u16 = 58;
    pub const GET_IS_ID: u16 = 72;
    pub const GET_LEVEL: u16 = 80;
    pub const HAS_SHOUT: u16 = 378;
    pub const GET_EVENT_DATA: u16 = 576;
    pub const GET_ACTOR_VALUE_PERCENT: u16 = 640;
}

/// What a condition runs on: the subject, the target, a reference.
pub mod run_on {
    pub const SUBJECT: u32 = 0;
    pub const TARGET: u32 = 1;
}

/// A condition (`CTDA`): `func(p1, p2) <op> value`, run on `run_on`.
pub fn ctda(func: u16, p1: u32, p2: u32, op: u8, value: f32, run_on: u32) -> [u8; 32] {
    bytes(&[
        &[op << 5, 0, 0, 0],
        &value.to_le_bytes(),
        &func.to_le_bytes(),
        &[0, 0],
        &p1.to_le_bytes(),
        &p2.to_le_bytes(),
        &run_on.to_le_bytes(),
        &0u32.to_le_bytes(),
        &u32::MAX.to_le_bytes(),
    ])
}

/// `GetEventData` member `m` (`K1`, `R1`...) is `form`: a Story Manager
/// node's filter, as the game's script event nodes filter their keyword.
pub fn event_data_is(member: &[u8; 2], form: FormId) -> [u8; 32] {
    let p1 = (member[0] as u32) << 16 | (member[1] as u32) << 24;
    ctda(
        func::GET_EVENT_DATA,
        p1,
        form.0,
        op::EQ,
        1.0,
        run_on::SUBJECT,
    )
}

fn with_conditions(mut rec: RecordBuilder, conditions: &[[u8; 32]]) -> RecordBuilder {
    for c in conditions {
        rec = rec.field(b"CTDA", c);
    }
    rec
}

/// One effect of a spell.
pub struct Effect {
    pub mgef: FormId,
    pub magnitude: f32,
    pub area: u32,
    pub duration: u32,
    pub conditions: Vec<[u8; 32]>,
}

impl Effect {
    pub fn new(mgef: FormId, magnitude: f32, duration: u32) -> Self {
        Effect {
            mgef,
            magnitude,
            area: 0,
            duration,
            conditions: Vec::new(),
        }
    }

    pub fn when(mut self, c: [u8; 32]) -> Self {
        self.conditions.push(c);
        self
    }
}

/// Spell types, casting types and deliveries (`SPIT`).
pub mod spell {
    pub const SPELL: u32 = 0;
    pub const ABILITY: u32 = 4;
    pub const CONSTANT: u32 = 0;
    pub const FIRE_AND_FORGET: u32 = 1;
    pub const SELF: u32 = 0;
    pub const AIMED: u32 = 2;
    /// `SPIT` flags: the cost not worked out from the effects; resistance
    /// ignored.
    pub const MANUAL_COST: u32 = 0x1;
    pub const IGNORE_RESISTANCE: u32 = 0x10_0000;
}

/// A spell (`SPEL`): type, casting type, delivery, flags (`MANUAL_COST` is
/// always set: no magicka cost), its effects. `slot` is its equip slot.
#[allow(clippy::too_many_arguments)]
pub fn spell(
    object_id: u32,
    edid: &str,
    kind: u32,
    casting: u32,
    delivery: u32,
    flags: u32,
    slot: Option<FormId>,
    effects: &[Effect],
) -> RecordBuilder {
    // SPIT: cost, flags, type, charge time, casting type, delivery, cast
    // duration, range, half cost perk.
    let spit: [u8; 36] = bytes(&[
        &0u32.to_le_bytes(),
        &(flags | spell::MANUAL_COST).to_le_bytes(),
        &kind.to_le_bytes(),
        &0f32.to_le_bytes(),
        &casting.to_le_bytes(),
        &delivery.to_le_bytes(),
        &0f32.to_le_bytes(),
        &0f32.to_le_bytes(),
        &0u32.to_le_bytes(),
    ]);
    let mut rec = RecordBuilder::new(b"SPEL", own(object_id))
        .zstring(b"EDID", edid)
        .field(b"OBND", &[0; 12])
        .zstring(b"FULL", edid);
    if let Some(slot) = slot {
        rec = rec.form(b"ETYP", slot);
    }
    rec = rec.zstring(b"DESC", "").field(b"SPIT", &spit);
    for e in effects {
        let efit: [u8; 12] = bytes(&[
            &e.magnitude.to_le_bytes(),
            &e.area.to_le_bytes(),
            &e.duration.to_le_bytes(),
        ]);
        rec = rec.form(b"EFID", e.mgef).field(b"EFIT", &efit);
        rec = with_conditions(rec, &e.conditions);
    }
    rec
}

pub fn keyword(object_id: u32, edid: &str) -> RecordBuilder {
    RecordBuilder::new(b"KYWD", own(object_id)).zstring(b"EDID", edid)
}

/// A perk's section.
pub enum Section {
    /// Set `stage` of `quest` when the perk is added.
    Quest(FormId, u8),
    Ability(FormId),
    /// An entry point: the point, its function (1 set, 2 add, 3 multiply),
    /// its condition tabs (none written), the value, and the priority.
    Entry {
        point: u8,
        function: u8,
        tabs: u8,
        value: f32,
        priority: u8,
    },
}

/// A perk (`PERK`): playable, one rank, its sections.
pub fn perk(object_id: u32, edid: &str, sections: &[Section]) -> RecordBuilder {
    // DATA: trait, level, ranks, playable, hidden.
    let mut rec = RecordBuilder::new(b"PERK", own(object_id))
        .zstring(b"EDID", edid)
        .zstring(b"FULL", edid)
        .zstring(b"DESC", "")
        .field(b"DATA", &[0, 0, 1, 1, 0]);
    for s in sections {
        // PRKE: section type, rank, priority.
        rec = match *s {
            Section::Quest(quest, stage) => {
                let data: [u8; 8] = bytes(&[&quest.0.to_le_bytes(), &[stage, 0, 0, 0]]);
                rec.field(b"PRKE", &[0, 0, 0]).field(b"DATA", &data)
            }
            Section::Ability(spell) => rec.field(b"PRKE", &[1, 0, 0]).form(b"DATA", spell),
            Section::Entry {
                point,
                function,
                tabs,
                value,
                priority,
            } => rec
                .field(b"PRKE", &[2, 0, priority])
                .field(b"DATA", &[point, function, tabs])
                .field(b"EPFT", &[1])
                .f32(b"EPFD", value),
        };
        rec = rec.field(b"PRKF", &[]);
    }
    rec
}

/// How a reference alias is filled.
pub enum Fill {
    /// By scripts (`ForceRefTo`).
    Empty,
    Forced(FormId),
    Unique(FormId),
    /// A new reference of `object` at what alias `at` holds, at a level
    /// (`ALCL`: 0 easy .. 3 very hard, 4 none).
    Create {
        object: FormId,
        at: u32,
        level: u32,
    },
    /// The first reference the conditions accept.
    Matching(Vec<[u8; 32]>),
}

/// Alias flags (`FNAM`).
pub mod alias_flags {
    pub const OPTIONAL: u32 = 0x2;
    pub const ALLOW_REUSE: u32 = 0x8;
    pub const IN_LOADED_AREA: u32 = 0x20;
    pub const CLOSEST: u32 = 0x2000;
}

pub struct Alias {
    pub id: u32,
    pub name: &'static str,
    pub flags: u32,
    pub fill: Fill,
    /// Its packages (`ALPC`).
    pub packages: Vec<FormId>,
}

/// A quest with stages, objectives and aliases: `DNAM` flags (0x1 start
/// game enabled, 0x10 starts enabled), priority, type, its script.
pub struct Quest<'a> {
    pub object_id: u32,
    pub edid: &'a str,
    pub flags: u16,
    pub priority: u8,
    pub kind: u32,
    pub script: Option<AttachedScript>,
    pub stages: &'a [u16],
    pub aliases: Vec<Alias>,
}

impl Quest<'_> {
    pub fn record(self) -> RecordBuilder {
        let dnam: [u8; 12] = bytes(&[
            &self.flags.to_le_bytes(),
            &[self.priority, 0],
            &0u32.to_le_bytes(),
            &self.kind.to_le_bytes(),
        ]);
        let mut rec = RecordBuilder::new(b"QUST", own(self.object_id)).zstring(b"EDID", self.edid);
        if let Some(script) = self.script {
            rec = rec.field(b"VMAD", &vmad(&[script]));
        }
        rec = rec
            .zstring(b"FULL", self.edid)
            .field(b"DNAM", &dnam)
            .field(b"NEXT", &[]);
        // INDX: stage, flags, unknown; one log entry (QSDT flags).
        for stage in self.stages {
            let indx: [u8; 4] = bytes(&[&stage.to_le_bytes(), &[0, 0]]);
            rec = rec.field(b"INDX", &indx).field(b"QSDT", &[0]);
        }
        let next = self.aliases.iter().map(|a| a.id + 1).max().unwrap_or(0);
        rec = rec.u32(b"ANAM", next);
        for a in self.aliases {
            rec = rec
                .u32(b"ALST", a.id)
                .zstring(b"ALID", a.name)
                .u32(b"FNAM", a.flags);
            rec = match a.fill {
                Fill::Empty => rec,
                Fill::Forced(r) => rec.form(b"ALFR", r),
                Fill::Unique(npc) => rec.form(b"ALUA", npc),
                Fill::Create { object, at, level } => rec
                    .form(b"ALCO", object)
                    .u32(b"ALCA", at)
                    .u32(b"ALCL", level),
                Fill::Matching(conditions) => with_conditions(rec, &conditions),
            };
            for p in &a.packages {
                rec = rec.form(b"ALPC", *p);
            }
            rec = rec.field(b"ALED", &[]);
        }
        rec
    }
}

/// A Story Manager branch node (`SMBN`) or quest node (`SMQN`, with its
/// quests: quest, flags, hours before it may run again): its parent, the
/// sibling before it, its conditions and most quests running under it.
pub fn story_node(
    object_id: u32,
    edid: &str,
    parent: FormId,
    previous: FormId,
    conditions: &[[u8; 32]],
    max_running: u32,
    quests: Option<&[(FormId, u32, f32)]>,
) -> RecordBuilder {
    let tag = if quests.is_some() { b"SMQN" } else { b"SMBN" };
    let mut rec = RecordBuilder::new(tag, own(object_id))
        .zstring(b"EDID", edid)
        .form(b"PNAM", parent)
        .form(b"SNAM", previous)
        .u32(b"CITC", conditions.len() as u32);
    rec = with_conditions(rec, conditions)
        .u32(b"DNAM", 0)
        .u32(b"XNAM", max_running);
    if let Some(quests) = quests {
        rec = rec.u32(b"QNAM", quests.len() as u32);
        for &(quest, flags, reset) in quests {
            rec = rec
                .form(b"NNAM", quest)
                .u32(b"FNAM", flags)
                .f32(b"RNAM", reset);
        }
    }
    rec
}

/// Scene flags (`FNAM`) and actor behaviour (`DNAM`).
pub mod scene_flags {
    pub const STOP_QUEST_ON_END: u32 = 0x2;
    pub const DEATH_END: u32 = 0x2;
}

/// A scene (`SCEN`) of `quest`: one phase (start and completion conditions)
/// with one actor (a reference alias of the quest, its behaviour flags) and a
/// timer action of `seconds` on it.
#[allow(clippy::too_many_arguments)]
pub fn scene(
    object_id: u32,
    edid: &str,
    quest: FormId,
    flags: u32,
    alias: u32,
    behaviour: u32,
    start: &[[u8; 32]],
    completion: &[[u8; 32]],
    seconds: f32,
) -> RecordBuilder {
    let mut rec = RecordBuilder::new(b"SCEN", own(object_id))
        .zstring(b"EDID", edid)
        .u32(b"FNAM", flags)
        .field(b"HNAM", &[])
        .zstring(b"NAM0", "");
    rec = with_conditions(rec, start).field(b"NEXT", &[]);
    rec = with_conditions(rec, completion)
        .field(b"NEXT", &[])
        .u32(b"WNAM", 200)
        .field(b"HNAM", &[])
        .u32(b"ALID", alias)
        .u32(b"LNAM", 0)
        .u32(b"DNAM", behaviour)
        // The timer: type 2, its actor, index 1, phases 0 to 0, seconds.
        .field(b"ANAM", &2u16.to_le_bytes())
        .zstring(b"NAM0", "")
        .u32(b"ALID", alias)
        .u32(b"INAM", 1)
        .u32(b"SNAM", 0)
        .u32(b"ENAM", 0)
        .f32(b"SNAM", seconds)
        .field(b"ANAM", &[])
        .form(b"PNAM", quest)
        .u32(b"INAM", 2);
    // VNAM as the game's scenes have it.
    rec.field(b"VNAM", &[0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0])
}

pub fn word_of_power(object_id: u32, edid: &str) -> RecordBuilder {
    RecordBuilder::new(b"WOOP", own(object_id))
        .zstring(b"EDID", edid)
        .zstring(b"FULL", edid)
        .zstring(b"TNAM", edid)
}

/// A shout (`SHOU`) of three words: (word, spell, recovery seconds).
pub fn shout(object_id: u32, edid: &str, words: &[(FormId, FormId, f32)]) -> RecordBuilder {
    let mut rec = RecordBuilder::new(b"SHOU", own(object_id))
        .zstring(b"EDID", edid)
        .zstring(b"FULL", edid)
        .zstring(b"DESC", "");
    for &(word, spell, recovery) in words {
        let snam: [u8; 12] = bytes(&[
            &word.0.to_le_bytes(),
            &spell.0.to_le_bytes(),
            &recovery.to_le_bytes(),
        ]);
        rec = rec.field(b"SNAM", &snam);
    }
    rec
}

/// A copy of a Skyrim.esm record under our object id and editor id (its
/// localized names left out: our plugin isn't localized), `script` attached,
/// each subrecord passed through `patch`.
pub fn copy(
    lo: &LoadOrder,
    from: &str,
    object_id: u32,
    edid: &str,
    script: Option<AttachedScript>,
    patch: impl Fn(&[u8; 4], &mut Vec<u8>),
) -> Result<RecordBuilder> {
    let id = lo
        .find_editor_id(from)
        .with_context(|| format!("{from} not found"))?;
    anyhow::ensure!(id.0 >> 24 == 0, "{from} isn't Skyrim.esm's");
    let rec = lo.get(id).context("record")?;
    let mut out = RecordBuilder::new(&rec.tag().0, own(object_id)).zstring(b"EDID", edid);
    if let Some(script) = script {
        out = out.field(b"VMAD", &vmad(&[script]));
    }
    for s in rec.subrecords() {
        let tag = s.tag.0;
        if matches!(&tag, b"EDID" | b"VMAD" | b"FULL" | b"DESC") {
            continue;
        }
        let mut data = s.data.to_vec();
        patch(&tag, &mut data);
        out = out.field(&tag, &data);
    }
    Ok(out)
}

/// Group combat reactions (`XNAM`).
pub mod reaction {
    pub const ENEMY: u32 = 1;
    pub const ALLY: u32 = 2;
    pub const FRIEND: u32 = 3;
}

/// A faction (no crime) with its reactions to others: (our object id,
/// reaction).
pub fn faction(object_id: u32, edid: &str, relations: &[(u32, u32)]) -> RecordBuilder {
    let mut rec = RecordBuilder::new(b"FACT", own(object_id))
        .zstring(b"EDID", edid)
        .zstring(b"FULL", edid);
    for &(other, reaction) in relations {
        let xnam: [u8; 12] = bytes(&[
            &own(other).0.to_le_bytes(),
            &0i32.to_le_bytes(),
            &reaction.to_le_bytes(),
        ]);
        rec = rec.field(b"XNAM", &xnam);
    }
    rec.u32(b"DATA", 0).field(b"CRVA", &[0; 20])
}
