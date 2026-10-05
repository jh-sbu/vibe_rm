//! Condition (CTDA) evaluation.

pub mod functions;

use esp::{FormId, LoadedRecord};

use crate::engine::{Engine, PLAYER_REF};

#[derive(Debug, Clone)]
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
    /// Raw reference field (alias id when `run_on` is a quest alias).
    pub reference_raw: u32,
    /// String parameters (CIS1 / CIS2) that follow the CTDA.
    pub string_p1: Option<std::sync::Arc<str>>,
    pub string_p2: Option<std::sync::Arc<str>>,
}

/// Parse all CTDA subrecords of a record, attaching CIS1/CIS2 string parameters.
pub fn parse_all(rec: &LoadedRecord<'_>) -> Vec<Condition> {
    let mut out: Vec<Condition> = Vec::new();
    for s in rec.subrecords() {
        match &s.tag.0 {
            b"CTDA" => {
                if let Some(c) = parse(rec, s.data) {
                    out.push(c);
                }
            }
            b"CIS1" => {
                if let Some(c) = out.last_mut() {
                    c.string_p1 = Some(s.zstring().into());
                }
            }
            b"CIS2" => {
                if let Some(c) = out.last_mut() {
                    c.string_p2 = Some(s.zstring().into());
                }
            }
            _ => {}
        }
    }
    out
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
        reference: if d.len() >= 28 && u32_at(20) == 2 { FormId(fid(u32_at(24))) } else { FormId::NULL },
        reference_raw: if d.len() >= 28 { u32_at(24) } else { 0 },
        string_p1: None,
        string_p2: None,
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
    /// For furniture idles, the furniture reference.
    pub target: Option<FormId>,
    pub quest: Option<FormId>,
    /// Set when picking from the IDLE tree.
    pub idle: Option<IdleQuery>,
}

/// The subject's furniture state while picking an idle.
#[derive(Debug, Clone, Copy, Default)]
pub struct IdleQuery {
    /// Furniture marker animation type: 1 sit, 2 lay, 4 lean (0 none).
    pub anim_type: u32,
    /// `IsFurnitureEntryType` value of the side used (front 0x10000, back 0x20000,
    /// right 0x40000, left 0x80000).
    pub entry: u32,
    /// `GetSitting` / `GetSleeping`: 2 getting in, 3 in, 4 getting out.
    pub state: f32,
    pub quick: bool,
    /// Using food or drink (`GetIsUsedItemType`).
    pub eating: bool,
    /// Overrides the subject's race for `IsChild` (no subject when picking ahead of time).
    pub child: Option<bool>,
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

/// Debug description of how each condition evaluates.
pub fn explain(e: &Engine, conds: &[Condition], ctx: Context) -> String {
    conds
        .iter()
        .map(|c| {
            format!(
                "{}({:08X},{:08X}) run_on={} op={} val={} {} => {}",
                functions::name(c.func),
                c.p1,
                c.p2,
                c.run_on,
                c.op,
                c.value,
                if c.or { "OR" } else { "AND" },
                eval_one(e, c, ctx)
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn eval_one(e: &Engine, c: &Condition, ctx: Context) -> bool {
    let mut subject = match c.run_on {
        0 => ctx.subject,
        1 => ctx.target,
        2 => Some(c.reference),
        // Quest alias: the reference field holds the alias id.
        5 => ctx.quest.and_then(|q| e.alias_ref(q, c.reference_raw)),
        _ => ctx.subject,
    };
    if c.swap {
        subject = ctx.target;
    }
    // Unsupported functions fail: hiding content beats showing everything.
    let Some(lhs) = function_value(e, c, subject, ctx) else { return false };
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
        365 => b(ctx.idle.and_then(|q| q.child).unwrap_or_else(|| subj_base.and_then(|n| e.npc_race(n)).is_some_and(|r| e.race_is_child(r)))), // IsChild
        125 => b(false),                                                  // IsGuard
        141 => b(e.conversation.as_ref().is_some_and(|cv| Some(cv.npc_ref) == subject && cv.current.is_some())), // IsTalking
        149 => b(e.current_weather() == Some(p1)),                        // GetIsCurrentWeather
        1 => {
            // GetDistance
            let a = subject.and_then(|s| e.ref_position(s))?;
            let t = e.ref_position(p1)?;
            Some(a.distance(t))
        }
        32 => b(subject.and_then(|s| e.lo.cell_of_ref(s)) == e.lo.cell_of_ref(p1)), // GetInSameCell
        566 => b(ctx.quest.and_then(|q| e.alias_ref(q, c.p1)).is_some_and(|r| Some(r) == subject)), // GetIsAliasRef
        629 => {
            // GetVMQuestVariable(quest, "::name_var")
            let name = c.string_p2.as_deref()?;
            Some(e.vm.get_var(papyrus::ObjectId::Form(c.p1), name).map(|v| v.as_float()).unwrap_or(0.0))
        }
        630 => {
            // GetVMScriptVariable(script, "::name_var") on the subject
            let name = c.string_p2.as_deref()?;
            Some(subject.and_then(|s| e.vm.get_var(papyrus::ObjectId::Form(s.0), name)).map(|v| v.as_float()).unwrap_or(0.0))
        }
        249 => b(e.conversation.as_ref().is_some_and(|cv| Some(cv.npc_ref) == subject)), // IsInDialogueWithPlayer
        47 => Some(0.0),                                                  // GetItemCount
        35 => b(subject.is_some_and(|s| e.is_disabled(s))),               // GetDisabled
        359 => b(e.current_location().is_some_and(|l| e.location_within(l, p1))), // GetInCurrentLoc
        562 => b(e.current_location().is_some_and(|l| e.has_keyword(l, p1))), // LocationHasKeyword
        606 => Some(0.0),                                                 // GetKeywordDataForLocation
        579 | 286 | 403 | 161 | 182 => Some(0.0),                         // equipped shout, sneaking, relationship, package, equipped
        255 => b(subj_base.is_some_and(|n| e.offers_services_now(n))),   // GetOffersServicesNow
        // Nobody fights, swims, bleeds out, feeds or takes commands yet.
        289 | 101 | 185 | 580 | 700 | 226 => b(false),
        // Behaviour graph variables of the subject's graph (the idle-picking
        // fallback below answers for actors without one).
        675 | 447 if subject.is_some_and(|s| e.graph_variable(s, "").is_some()) => {
            subject.and_then(|s| e.graph_variable(s, c.string_p1.as_deref().unwrap_or("")))
        }
        // The line being said, while saying it.
        434 | 435 => {
            let line = e.conversation.as_ref().filter(|cv| Some(cv.npc_ref) == subject)?.current.as_ref()?;
            Some(if c.func == 434 { line.emotion.0 } else { line.emotion.1 } as f32)
        }
        623 => Some(subject.and_then(|s| e.actor_speed(s)).unwrap_or(0.0)), // GetMovementSpeed
        // GetEquippedItemType: hands empty until there is an inventory.
        597 => Some(0.0),
        263 => b(false), // IsWeaponOut
        // Nor flees, attacks, staggers, recoils or is ridden.
        329 | 672 | 701 | 702 | 714 => b(false),
        _ => idle_function_value(e, c, ctx),
    }
}

/// Furniture and idle functions, answered from the query when picking idles.
fn idle_function_value(e: &Engine, c: &Condition, ctx: Context) -> Option<f32> {
    let q = ctx.idle?;
    let furniture_base = ctx.target.and_then(|t| e.base_of(t));
    match c.func {
        159 => Some(if q.anim_type == 2 { 0.0 } else { q.state }), // GetSitting
        49 => Some(if q.anim_type == 2 { q.state } else { 0.0 }),  // GetSleeping
        631 | 703 => b(q.quick),                                    // IsEntering / IsExitingInteractionQuick
        614 => b(q.entry == c.p1),                                 // IsFurnitureEntryType
        613 => b(q.anim_type == c.p1),                             // IsFurnitureAnimType
        163 => b(furniture_base == Some(FormId(c.p1))),            // IsCurrentFurnitureObj
        247 => b(q.eating),                                        // GetIsUsedItemType
        25 | 704 => b(false),                                      // IsMoving, IsPathing
        // GetGraphVariableInt: settled in a pose (not mid-transition).
        675 => Some(if c.string_p1.as_deref().is_some_and(|v| v.eq_ignore_ascii_case("bNeutralState")) { 1.0 } else { 0.0 }),
        _ => None,
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

    /// Location (LCTN) of the current cell.
    pub fn current_location(&self) -> Option<FormId> {
        let cell = match self.location {
            crate::engine::Location::Interior(c) => Some(c),
            crate::engine::Location::Exterior { world, center } => self.lo.world(world).and_then(|w| w.cells.get(&center).copied()),
            _ => None,
        }?;
        let rec = self.lo.get(cell)?;
        let d = rec.get(b"XLCN")?;
        Some(rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().ok()?))))
    }

    /// Whether location `l` is `target` or nested inside it.
    pub fn location_within(&self, mut l: FormId, target: FormId) -> bool {
        for _ in 0..16 {
            if l == target {
                return true;
            }
            let Some(rec) = self.lo.get(l) else { return false };
            let Some(d) = rec.get(b"PNAM") else { return false };
            l = rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap())));
        }
        false
    }

    /// Whether an NPC is in a vendor faction whose hours include the current time.
    pub fn offers_services_now(&self, npc: FormId) -> bool {
        self.npc_factions(npc).iter().any(|(f, _)| {
            let Some(r) = self.lo.get(*f) else { return false };
            let flags = r.get(b"DATA").map(|d| u32::from_le_bytes(d[0..4].try_into().unwrap())).unwrap_or(0);
            if flags & 0x4000 == 0 {
                return false;
            }
            match r.get(b"VENV") {
                Some(v) if v.len() >= 4 => {
                    let (start, end) = (u16::from_le_bytes([v[0], v[1]]) as f32, u16::from_le_bytes([v[2], v[3]]) as f32);
                    start == end || (self.hour >= start && self.hour < end)
                }
                _ => true,
            }
        })
    }

    /// Dialogue conditions declared on a quest (CTDAs before the NEXT marker).
    pub fn quest_dialogue_conditions(&self, quest: FormId) -> Vec<Condition> {
        let Some(rec) = self.lo.get(quest) else { return Vec::new() };
        let mut out: Vec<Condition> = Vec::new();
        for s in rec.subrecords() {
            match &s.tag.0 {
                b"NEXT" | b"INDX" | b"QOBJ" | b"ALST" | b"ALLS" | b"ANAM" => break,
                b"CTDA" => {
                    if let Some(c) = parse(&rec, s.data) {
                        out.push(c);
                    }
                }
                b"CIS1" => {
                    if let Some(c) = out.last_mut() {
                        c.string_p1 = Some(s.zstring().into());
                    }
                }
                b"CIS2" => {
                    if let Some(c) = out.last_mut() {
                        c.string_p2 = Some(s.zstring().into());
                    }
                }
                _ => {}
            }
        }
        out
    }

    pub fn alias_ref(&self, quest: FormId, alias: u32) -> Option<FormId> {
        self.scripts.quests.get(&quest).and_then(|q| q.aliases.get(&alias).copied())
    }

    pub fn current_weather(&self) -> Option<FormId> {
        self.sky.as_ref().map(|(w, _)| w.id)
    }

    /// A random number that doesn't advance the generator (for read-only condition checks).
    /// A fresh roll for each condition that asks (xorshift, seeded from the engine RNG).
    pub fn peek_rand(&self) -> u64 {
        let mut x = self.cond_rng.get() ^ self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.cond_rng.set(x);
        x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32
    }
}

pub fn subject_is_player(s: Option<FormId>) -> bool {
    s == Some(PLAYER_REF)
}
