//! Condition (CTDA) evaluation.

pub mod functions;

use esp::{FormId, LoadedRecord};

use crate::engine::{Engine, PLAYER_REF};

#[derive(Debug, Clone, Copy)]
pub struct Condition {
    pub op: u8,
    pub or: bool,
    pub use_global: bool,
    pub swap: bool,
    /// Comparison value, or a GLOB FormID when `use_global` is set.
    pub value: f32,
    pub global: FormId,
    pub func: u16,
    pub p1: u32,
    pub p2: u32,
    pub run_on: u32,
    pub reference: FormId,
}

/// Parse all CTDA subrecords of a record (optionally only those following a given marker).
pub fn parse_all(rec: &LoadedRecord<'_>) -> Vec<Condition> {
    rec.subrecords().filter(|s| s.tag.0 == *b"CTDA").filter_map(|s| parse(rec, s.data)).collect()
}

pub fn parse(rec: &LoadedRecord<'_>, d: &[u8]) -> Option<Condition> {
    if d.len() < 24 {
        return None;
    }
    let u32_at = |o: usize| u32::from_le_bytes(d[o..o + 4].try_into().unwrap());
    let flags = d[0];
    let use_global = flags & 0x04 != 0;
    let func = u16::from_le_bytes([d[8], d[9]]);
    // Parameters that are FormIDs need globalising; integers pass through unchanged
    // (FormIDs are local to the plugin; globalize only values that look like ids).
    let fid = |v: u32| if v == 0 { 0 } else { rec.fid(FormId(v)).0 };
    let p1 = u32_at(12);
    let p2 = u32_at(16);
    Some(Condition {
        op: flags >> 5,
        or: flags & 0x01 != 0,
        use_global,
        swap: flags & 0x10 != 0,
        value: if use_global { 0.0 } else { f32::from_bits(u32_at(4)) },
        global: if use_global { FormId(fid(u32_at(4))) } else { FormId::NULL },
        func,
        p1: if param_is_form(func, 1) { fid(p1) } else { p1 },
        p2: if param_is_form(func, 2) { fid(p2) } else { p2 },
        run_on: u32_at(20),
        reference: if d.len() >= 28 { FormId(fid(u32_at(24))) } else { FormId::NULL },
    })
}

/// Whether parameter `n` of function `func` is a form reference (vs. an int/enum).
fn param_is_form(func: u16, n: u8) -> bool {
    let Some(f) = functions::find(func) else { return false };
    let ty = if n == 1 { f.2 } else { f.3 };
    !matches!(
        ty,
        "" | "ptInteger"
            | "ptFloat"
            | "ptAxis"
            | "ptSex"
            | "ptActorValue"
            | "ptCastingSource"
            | "ptString"
            | "ptAlias"
            | "ptFormType"
            | "ptQuestStage"
            | "ptMiscStat"
            | "ptAlignment"
            | "ptCrimeType"
            | "ptCriticalStage"
            | "ptEvent"
            | "ptEventData"
            | "ptFurnitureAnim"
            | "ptFurnitureEntry"
            | "ptPackdata"
            | "ptPlayerAction"
            | "ptVATSValueFunction"
            | "ptVATSValueParam"
            | "ptWardState"
    )
}

/// Who a condition is evaluated on.
#[derive(Debug, Clone, Copy, Default)]
pub struct Context {
    pub subject: Option<FormId>,
    pub target: Option<FormId>,
    pub quest: Option<FormId>,
}

pub fn evaluate(e: &Engine, conds: &[Condition], ctx: Context) -> bool {
    // Skyrim combines ORs before ANDs: (a OR b) AND (c OR d) ...
    let mut result = true;
    let mut group = false;
    let mut in_group = false;
    for c in conds {
        let v = eval_one(e, c, ctx);
        group = if in_group { group || v } else { v };
        in_group = true;
        if !c.or {
            result &= group;
            in_group = false;
            if !result {
                return false;
            }
        }
    }
    if in_group {
        result &= group;
    }
    result
}

fn eval_one(e: &Engine, c: &Condition, ctx: Context) -> bool {
    let mut subject = match c.run_on {
        0 => ctx.subject,
        1 => ctx.target,
        2 => Some(c.reference),
        _ => ctx.subject,
    };
    if c.swap {
        subject = ctx.target;
    }
    let Some(lhs) = function_value(e, c, subject, ctx) else { return true };
    let rhs = if c.use_global { e.global_value(c.global) } else { c.value };
    match c.op {
        0 => (lhs - rhs).abs() < 1e-4,
        1 => (lhs - rhs).abs() >= 1e-4,
        2 => lhs > rhs,
        3 => lhs >= rhs,
        4 => lhs < rhs,
        5 => lhs <= rhs,
        _ => true,
    }
}

fn b(x: bool) -> Option<f32> {
    Some(if x { 1.0 } else { 0.0 })
}

/// Value of a condition function; `None` means "unsupported" (treated as passing).
fn function_value(e: &Engine, c: &Condition, subject: Option<FormId>, ctx: Context) -> Option<f32> {
    let p1 = FormId(c.p1);
    let subj_base = subject.and_then(|s| e.base_of(s));
    match c.func {
        18 => Some(e.hour),                       // GetCurrentTime
        170 => Some((e.day % 7) as f32),          // GetDayOfWeek
        77 => Some((e.peek_rand() % 100) as f32), // GetRandomPercent
        74 => Some(e.global_value(p1)),           // GetGlobalValue
        56 => b(e.scripts.quests.get(&p1).is_some_and(|q| q.running)), // GetQuestRunning
        58 => Some(e.scripts.quests.get(&p1).map(|q| q.stage as f32).unwrap_or(0.0)), // GetStage
        59 => b(e.scripts.quests.get(&p1).is_some_and(|q| q.done.contains(&(c.p2 as u16)))), // GetStageDone
        543 => b(e.scripts.quests.get(&p1).is_some_and(|q| q.completed)), // GetQuestCompleted
        72 => b(subj_base == Some(p1) || subject == Some(p1)),            // GetIsID
        136 => b(subject == Some(p1)),                                    // GetIsReference
        70 => b(subj_base.map(|n| e.npc_is_female(n) as u32) == Some(c.p1)), // GetIsSex
        131 => b(e.npc_is_female(FormId(0x7)) as u32 == c.p1),            // GetPCIsSex
        69 => b(subj_base.and_then(|n| e.npc_race(n)) == Some(p1)),       // GetIsRace
        130 => b(e.npc_race(FormId(0x7)) == Some(p1)),                    // GetPCIsRace
        71 => b(subj_base.is_some_and(|n| e.npc_factions(n).iter().any(|(f, _)| *f == p1))), // GetInFaction
        73 => Some(subj_base.and_then(|n| e.npc_factions(n).into_iter().find(|(f, _)| *f == p1).map(|x| x.1 as f32)).unwrap_or(-1.0)), // GetFactionRank
        426 => b(subj_base.and_then(|n| e.npc_voice_type(n)).is_some_and(|v| v == p1 || e.formlist(p1).contains(&v))), // GetIsVoiceType
        560 => b(subject.is_some_and(|s| e.has_keyword(s, p1))),          // HasKeyword
        300 => b(matches!(e.location, crate::engine::Location::Interior(_))), // IsInInterior
        67 => b(subject.and_then(|s| e.lo.cell_of_ref(s)) == Some(p1)),   // GetInCell
        310 => b(matches!(e.location, crate::engine::Location::Exterior { world, .. } if world == p1)), // GetInWorldspace
        46 => b(false),                                                   // GetDead
        84 => Some(0.0),                                                  // GetDeadCount
        80 => Some(1.0),                                                  // GetLevel
        14 => Some(100.0),                                                // GetActorValue
        365 => b(subj_base.and_then(|n| e.npc_race(n)).is_some_and(|r| e.race_is_child(r))), // IsChild
        125 => b(false),                                                  // IsGuard
        141 | 249 => b(false),                                            // IsTalking / IsInDialogueWithPlayer
        149 => b(e.current_weather() == Some(p1)),                        // GetIsCurrentWeather
        1 => {
            // GetDistance
            let a = subject.and_then(|s| e.ref_position(s))?;
            let t = e.ref_position(p1)?;
            Some(a.distance(t))
        }
        32 => b(subject.and_then(|s| e.lo.cell_of_ref(s)) == e.lo.cell_of_ref(p1)), // GetInSameCell
        _ => {
            let _ = ctx;
            None
        }
    }
}

impl Engine {
    pub fn npc_race(&self, npc: FormId) -> Option<FormId> {
        let rec = self.lo.get(npc)?;
        let d = rec.get(b"RNAM")?;
        Some(rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().ok()?))))
    }

    pub fn npc_factions(&self, npc: FormId) -> Vec<(FormId, i8)> {
        let Some(rec) = self.lo.get(npc) else { return Vec::new() };
        rec.subrecords().filter(|s| s.tag.0 == *b"SNAM" && s.data.len() >= 5).map(|s| (rec.fid(s.form_id(0)), s.data[4] as i8)).collect()
    }

    pub fn npc_voice_type(&self, npc: FormId) -> Option<FormId> {
        let rec = self.lo.get(npc)?;
        let d = rec.get(b"VTCK")?;
        Some(rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().ok()?))))
    }

    pub fn race_is_child(&self, race: FormId) -> bool {
        // RACE DATA flags bit 0x4 = child.
        self.lo.get(race).and_then(|r| r.get(b"DATA").map(|d| d.len() >= 36 && u32::from_le_bytes(d[32..36].try_into().unwrap()) & 0x4 != 0)).unwrap_or(false)
    }

    pub fn current_weather(&self) -> Option<FormId> {
        self.sky.as_ref().map(|(w, _)| w.id)
    }

    /// A random number that doesn't advance the generator (for read-only condition checks).
    pub fn peek_rand(&self) -> u64 {
        let x = (self.scripts.real_time * 1000.0) as u64 ^ 0x9E37_79B9_7F4A_7C15;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32
    }
}

pub fn subject_is_player(s: Option<FormId>) -> bool {
    s == Some(PLAYER_REF)
}
