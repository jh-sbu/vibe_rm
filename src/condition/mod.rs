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
    /// In a package template's procedure tree: the input run on (`run_on` 6,
    /// package data), and whether the first parameter is an input (flag 0x08).
    pub pack_input: Option<u8>,
    pub p1_input: bool,
    /// GetNumericPackageData / GetWithinPackageLocation: the package input the
    /// first parameter names, worked out when the package is parsed (`None`:
    /// the input is missing or of the wrong kind, and the condition fails).
    pub package_data: Option<PackageData>,
    /// Parameters naming quest aliases ("use aliases", flag 0x02): the first
    /// parameter is an alias id of the condition's quest.
    pub p1_alias: bool,
    /// The first parameter as a package input resolved for the actor.
    pub p1_ref: Option<RefOf>,
    /// Run on a Story Manager event's data (`run_on` 7): the member (`R1`, `L2`...).
    pub event_member: [u8; 2],
}

/// A package input a condition reads (see `Condition::package_data`).
#[derive(Debug, Clone, Copy)]
pub enum PackageData {
    Number(f32),
    Location(crate::ai::package::Location),
}

/// A reference worked out for whoever the condition runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefOf {
    /// The subject itself.
    Subject,
    /// The subject's linked reference (with the keyword, if not null).
    LinkedRef(FormId),
}

/// `run_on` for a condition with no one to run on (a package input that names
/// nothing): it fails.
pub const RUN_ON_NOTHING: u32 = u32::MAX;

/// Parse all CTDA subrecords of a record, attaching CIS1/CIS2 string parameters.
pub fn parse_all(rec: &LoadedRecord<'_>) -> Vec<Condition> {
    parse_all_before(rec, None)
}

/// As [`parse_all`], stopping at the first `stop` subrecord (a package's
/// procedure tree, `XNAM`, whose branches have conditions of their own).
pub fn parse_all_before(rec: &LoadedRecord<'_>, stop: Option<&[u8; 4]>) -> Vec<Condition> {
    let mut out: Vec<Condition> = Vec::new();
    for s in rec.subrecords() {
        match &s.tag.0 {
            t if Some(t) == stop => break,
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
    // "Use aliases" (flag 0x02): a reference or location parameter is an alias id.
    let p1_alias = flags & 0x02 != 0
        && functions::find(func)
            .is_some_and(|f| matches!(f.2, "ptReference" | "ptActor" | "ptLocation"));
    Some(Condition {
        op: flags >> 5,
        or: flags & 0x01 != 0,
        use_global,
        swap: flags & 0x10 != 0,
        value: if use_global {
            0.0
        } else {
            f32::from_bits(u32_at(4))
        },
        global: if use_global {
            FormId(fid(u32_at(4)))
        } else {
            FormId::NULL
        },
        func,
        // Package inputs by index (flag 0x08) and aliases (0x02) aren't forms.
        p1: if flags & 0x08 == 0 && !p1_alias && param_is_form(func, 1) {
            fid(p1)
        } else {
            p1
        },
        // GetEventData: (function, member) then a form, but for GetValue.
        p2: if param_is_form(func, 2) || (func == GET_EVENT_DATA && p1 & 0xffff != 2) {
            fid(p2)
        } else {
            p2
        },
        run_on: u32_at(20),
        reference: if d.len() >= 28 && u32_at(20) == 2 {
            FormId(fid(u32_at(24)))
        } else {
            FormId::NULL
        },
        reference_raw: if d.len() >= 28 { u32_at(24) } else { 0 },
        string_p1: None,
        string_p2: None,
        pack_input: (u32_at(20) == 6 && d.len() >= 32).then(|| u32_at(28) as u8),
        p1_input: flags & 0x08 != 0,
        package_data: None,
        p1_alias,
        p1_ref: None,
        event_member: if u32_at(20) == RUN_ON_EVENT_DATA && d.len() >= 32 {
            [d[28], d[29]]
        } else {
            [0; 2]
        },
    })
}

/// `run_on`: the data of the Story Manager event behind the quest.
const RUN_ON_EVENT_DATA: u32 = 7;
const GET_EVENT_DATA: u16 = 576;
/// Their first parameter is a package input's index (see `Condition::package_data`).
pub const GET_NUMERIC_PACKAGE_DATA: u16 = 612;
pub const GET_WITHIN_PACKAGE_LOCATION: u16 = 325;

/// Whether parameter `n` of function `func` is a form reference (vs. an int/enum).
pub(crate) fn param_is_form(func: u16, n: u8) -> bool {
    let Some(f) = functions::find(func) else {
        return false;
    };
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
    /// A magic effect's conditions: its spell was dual cast
    /// (`EffectWasDualCast`).
    pub dual_cast: bool,
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
    /// Picking ahead of time: the user holds the items `GetItemCount` asks about
    /// (firewood for a wood pile's put-down).
    pub holding: bool,
    /// `IsExitingInstant`: leaving the furniture without an exit animation.
    pub instant: bool,
}

pub fn evaluate(e: &Engine, conds: &[Condition], ctx: Context) -> bool {
    combine(conds, |c| eval_one(e, c, ctx))
}

/// Conditions the package alone decides (GetNumericPackageData against a
/// number, its input worked out with `package_data`): whether they pass.
/// `None` when any needs the world.
pub fn evaluate_package_data(conds: &[Condition]) -> Option<bool> {
    let numbers = conds
        .iter()
        .map(|c| match (c.func, c.package_data, c.use_global) {
            (GET_NUMERIC_PACKAGE_DATA, Some(PackageData::Number(v)), false) => Some(v),
            // An input missing or of another kind: the condition fails.
            (GET_NUMERIC_PACKAGE_DATA, None, false) => Some(f32::NAN),
            _ => None,
        })
        .collect::<Option<Vec<f32>>>()?;
    let mut lhs = numbers.into_iter();
    Some(combine(conds, |c| {
        let v = lhs.next().unwrap_or(f32::NAN);
        !v.is_nan() && compare(c.op, v, c.value)
    }))
}

/// Skyrim combines ORs before ANDs: (a OR b) AND (c OR d) ...
fn combine(conds: &[Condition], mut eval: impl FnMut(&Condition) -> bool) -> bool {
    let mut result = true;
    let mut group = false;
    let mut in_group = false;
    for c in conds {
        let v = eval(c);
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
        // The subject's linked reference (package inputs: with a keyword).
        4 => ctx
            .subject
            .and_then(|s| e.linked_ref(s, (!c.reference.is_null()).then_some(c.reference))),
        // Quest alias: the reference field holds the alias id.
        5 => ctx.quest.and_then(|q| e.alias_ref(q, c.reference_raw)),
        RUN_ON_EVENT_DATA => e
            .story_event_for(ctx.quest)
            .and_then(|ev| ev.member_form(c.event_member)),
        RUN_ON_NOTHING => return false,
        _ => ctx.subject,
    };
    if c.swap {
        subject = ctx.target;
    }
    // Unsupported functions fail: hiding content beats showing everything.
    let Some(lhs) = function_value(e, c, subject, ctx) else {
        return false;
    };
    let rhs = if c.use_global {
        e.global_value(c.global)
    } else {
        c.value
    };
    compare(c.op, lhs, rhs)
}

fn compare(op: u8, lhs: f32, rhs: f32) -> bool {
    match op {
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

/// Value of a condition function; `None` means "unsupported" (the condition fails).
fn function_value(e: &Engine, c: &Condition, subject: Option<FormId>, ctx: Context) -> Option<f32> {
    let p1 = match c.p1_ref {
        Some(RefOf::Subject) => ctx.subject?,
        Some(RefOf::LinkedRef(kw)) => e.linked_ref(ctx.subject?, (!kw.is_null()).then_some(kw))?,
        // An empty alias: nothing to compare with.
        None if c.p1_alias => alias(e, ctx, c.p1).unwrap_or_default(),
        None => FormId(c.p1),
    };
    let subj_base = subject.and_then(|s| e.base_of(s));
    match c.func {
        GET_NUMERIC_PACKAGE_DATA => match c.package_data {
            Some(PackageData::Number(v)) => Some(v),
            _ => None,
        },
        // The location is placed for the actor whose package it is.
        GET_WITHIN_PACKAGE_LOCATION => match c.package_data {
            Some(PackageData::Location(l)) => e
                .within_package_location(subject?, ctx.subject?, l, ctx.quest)
                .and_then(b),
            _ => None,
        },
        18 => Some(e.hour),                       // GetCurrentTime
        170 => Some((e.day % 7) as f32),          // GetDayOfWeek
        77 => Some((e.peek_rand() % 100) as f32), // GetRandomPercent
        74 => Some(e.global_value(p1)),           // GetGlobalValue
        56 => b(e.scripts.quests.get(&p1).is_some_and(|q| q.running)), // GetQuestRunning
        50 => b(subject.is_some_and(|s| e.talked_to_pc.contains(&s))), // GetTalkedToPC
        58 => Some(
            e.scripts
                .quests
                .get(&p1)
                .map(|q| q.stage as f32)
                .unwrap_or(0.0),
        ), // GetStage
        59 => b(e
            .scripts
            .quests
            .get(&p1)
            .is_some_and(|q| q.done.contains(&(c.p2 as u16)))), // GetStageDone
        543 => b(e.scripts.quests.get(&p1).is_some_and(|q| q.completed)), // GetQuestCompleted
        72 => b(subj_base == Some(p1) || subject == Some(p1)), // GetIsID
        136 => b(subject == Some(p1)),            // GetIsReference
        // IsHostileToActor: either fights the other. With no actor (a
        // magic effect's: cloaks' damage), the effect's caster.
        719 => {
            let other = if p1.is_null() { ctx.target } else { Some(p1) };
            b(subject.zip(other).is_some_and(|(s, o)| {
                e.combat_target(s) == Some(o) || e.combat_target(o) == Some(s)
            }))
        }
        70 => b(subj_base.map(|n| e.npc_is_female(n) as u32) == Some(c.p1)), // GetIsSex
        131 => b(e.npc_is_female(FormId(0x7)) as u32 == c.p1),               // GetPCIsSex
        69 => b(subj_base.and_then(|n| e.npc_race(n)) == Some(p1)),          // GetIsRace
        130 => b(e.npc_race(FormId(0x7)) == Some(p1)),                       // GetPCIsRace
        // Rank -1 (potential followers' `CurrentFollowerFaction`) isn't membership.
        71 => b(subject.is_some_and(|s| e.npc_factions(s).iter().any(|&(f, r)| f == p1 && r >= 0))), // GetInFaction
        73 => Some(
            subject
                .and_then(|s| {
                    e.npc_factions(s)
                        .into_iter()
                        .find(|(f, _)| *f == p1)
                        .map(|x| x.1 as f32)
                })
                .unwrap_or(-1.0),
        ), // GetFactionRank
        426 => b(subject
            .and_then(|s| e.actor_voice_type(s))
            .is_some_and(|v| v == p1 || e.formlist(p1).contains(&v))), // GetIsVoiceType
        560 => b(subject.is_some_and(|s| e.has_keyword(s, p1))), // HasKeyword
        // Undead: of an `ActorTypeUndead` race (the game caches its own
        // reading; see `known_ambiguities/magic.md`).
        715 => b(subject.is_some_and(|s| e.has_keyword(s, FormId(0x13796)))), // IsUndead
        // Commanded: summoned (or raised) by someone.
        700 => b(subject.is_some_and(|s| e.commander_of(s).is_some())), // IsCommandedActor
        354 => b(subject.is_some_and(|s| e.is_essential(s))),           // IsEssential
        721 => b(subject.is_some_and(|s| e.is_poison(s))),              // IsPoison
        300 => b(matches!(e.location, crate::engine::Location::Interior(_))), // IsInInterior
        67 => b(subject.and_then(|s| e.lo.cell_of_ref(s)) == Some(p1)), // GetInCell
        310 => {
            b(matches!(e.location, crate::engine::Location::Exterior { world, .. } if world == p1))
        } // GetInWorldspace
        46 => b(subject.is_some_and(|s| e.is_dead(s))),                 // GetDead
        84 => Some(0.0),                                                // GetDeadCount
        80 => Some(e.actor_level(subject?) as f32),                     // GetLevel
        // Actor values (`actor_values`).
        14 => Some(e.actor_value(subject?, c.p1)), // GetActorValue
        277 => Some(e.av_base(subject?, c.p1)),    // GetBaseActorValue
        494 => Some(e.av_permanent(subject?, c.p1)), // GetPermanentActorValue
        640 => Some(e.actor_value_fraction(subject?, c.p1)), // GetActorValuePercent
        365 => b(ctx.idle.and_then(|q| q.child).unwrap_or_else(|| {
            subj_base
                .and_then(|n| e.npc_race(n))
                .is_some_and(|r| e.race_is_child(r))
        })), // IsChild
        125 => b(subject.is_some_and(|s| e.is_guard(s))), // IsGuard
        61 => b(subject.is_some_and(|s| e.is_alarmed(s))), // GetAlarmed
        499 => Some(e.crime.arrests.days_in_jail as f32), // GetDaysInJail
        657 => b(subject.is_some_and(|s| e.crime.arrests.arresting.is_some_and(|a| a.0 == s))), // GetArrestingActor
        459 | 375 | 376 => {
            // GetCrimeGold (Violent / Nonviolent): the player's with the faction
            // given, else the subject's crime faction; the player's own, all.
            let pick = |b: crate::crime::Bounty| match c.func {
                375 => b.violent,
                376 => b.nonviolent,
                _ => b.total(),
            };
            let faction = Some(p1)
                .filter(|f| !f.is_null())
                .or_else(|| subject.and_then(|s| e.crime_faction(s)));
            Some(match faction {
                Some(f) => pick(e.bounty(f)),
                None if subject == Some(PLAYER_REF) => {
                    e.crime.bounties.values().map(|&b| pick(b)).sum()
                }
                None => 0,
            } as f32)
        }
        // GetStolenItemValueNoCrime / GetStolenItemValue: what the player stole
        // from the faction's people unseen / seen.
        366 | 373 if subject == Some(PLAYER_REF) => Some(
            e.crime
                .stolen_value
                .get(&p1)
                .map_or(0, |v| if c.func == 366 { v.0 } else { v.1 }) as f32,
        ),
        366 | 373 => Some(0.0),
        144 => Some(e.trespass_warning_level() as f32), // GetTrespassWarningLevel
        314 => b(subject.is_some_and(|s| e.crime.reacting_victim == Some(s))), // IsActorAVictim
        145 => b(subject == Some(PLAYER_REF) && e.trespassing()), // IsTrespassing
        152 => b(subject.and_then(|s| e.crime_faction(s)) == Some(p1)), // GetIsCrimeFaction
        497 => b(subject
            .and_then(|s| e.crime_faction(s))
            .is_some_and(|f| e.can_pay_crime_gold(f))), // CanPayCrimeGold
        652 => b(subject.is_some_and(|s| e.in_shared_crime_faction(s, p1))), // GetInSharedCrimeFaction
        141 => b(e
            .conversation
            .as_ref()
            .is_some_and(|cv| Some(cv.npc_ref) == subject && cv.current.is_some())
            || subject.is_some_and(|s| e.is_barking(s) || e.is_scene_speaking(s))), // IsTalking
        GET_EVENT_DATA => {
            // (function, member): 0 GetIsID, 1 IsInList, 2 GetValue, 3 HasKeyword.
            let ev = e.story_event_for(ctx.quest)?;
            let m = [(c.p1 >> 16) as u8, (c.p1 >> 24) as u8];
            let x = ev.member_form(m);
            let form = FormId(c.p2);
            let is = |x: FormId, f: FormId| x == f || e.base_of(x) == Some(f);
            match c.p1 & 0xffff {
                0 => b(x.is_some_and(|x| is(x, form))),
                1 => b(x.is_some_and(|x| e.formlist(form).iter().any(|&f| is(x, f)))),
                2 => Some(ev.member_value(m)),
                3 => b(x.is_some_and(|x| e.has_keyword(x, form))),
                _ => None,
            }
        }
        248 => b(e.is_scene_playing(p1)), // IsScenePlaying
        550 => b(e.is_scene_action_complete(p1, c.p2)), // IsSceneActionComplete
        590 => b(subject.is_some_and(|s| e.scene_of_actor(s).is_some())), // IsInScene
        429 => b(subject.is_some_and(|s| e.scenes.packages.contains_key(&s))), // IsScenePackageRunning
        149 => b(e.current_weather() == Some(p1)), // GetIsCurrentWeather
        62 => b(e.precipitating(crate::world::weather::flags::RAINY)), // IsRaining
        75 => b(e.precipitating(crate::world::weather::flags::SNOW)), // IsSnowing
        147 => Some(e.wind_speed()),               // GetWindSpeed
        148 => Some(e.weather.pct * 100.0),        // GetCurrentWeatherPercent
        266 | 267 => {
            // IsPleasant, IsCloudy
            let class = if c.func == 266 { 0 } else { 1 };
            b(e.weather
                .current
                .as_ref()
                .is_some_and(|w| w.classification() == class))
        }
        1 => {
            // GetDistance: far beyond anything between different interiors or
            // worldspaces.
            let (pa, a) = e.current_place_of(subject?)?;
            let (pb, t) = e.current_place_of(p1)?;
            Some(if crate::ai::schedule::same_space(pa, pb) {
                a.distance(t)
            } else {
                f32::MAX
            })
        }
        32 => b(subject.and_then(|s| e.lo.cell_of_ref(s)) == e.lo.cell_of_ref(p1)), // GetInSameCell
        566 => b(ctx
            .quest
            .and_then(|q| e.alias_ref(q, c.p1))
            .is_some_and(|r| Some(r) == subject)), // GetIsAliasRef
        629 => {
            // GetVMQuestVariable(quest, "::name_var")
            let name = c.string_p2.as_deref()?;
            Some(
                e.vm.get_var(papyrus::ObjectId::Form(c.p1), name)
                    .map(|v| v.as_float())
                    .unwrap_or(0.0),
            )
        }
        630 => {
            // GetVMScriptVariable(script, "::name_var") on the subject
            let name = c.string_p2.as_deref()?;
            Some(
                subject
                    .and_then(|s| e.vm.get_var(papyrus::ObjectId::Form(s.0), name))
                    .map(|v| v.as_float())
                    .unwrap_or(0.0),
            )
        }
        249 => b(e
            .conversation
            .as_ref()
            .is_some_and(|cv| Some(cv.npc_ref) == subject)), // IsInDialogueWithPlayer
        35 => b(subject.is_some_and(|s| e.is_disabled(s))), // GetDisabled
        491 => b(subject.is_some_and(|s| e.map_marker_visible(s))), // GetMapMarkerVisible
        250 => b(e.is_cleared(p1)),                         // GetLocationCleared
        598 => b(alias(e, ctx, c.p1).is_some_and(|l| e.is_cleared(l))), // GetLocationAliasCleared
        // Locations: of the subject (a location itself, when filling location aliases).
        359 => b(subject_location(e, subject).is_some_and(|l| e.location_within(l, p1))), // GetInCurrentLoc
        360 => b(subject_location(e, subject)
            .zip(alias(e, ctx, c.p1))
            .is_some_and(|(l, a)| e.location_within(l, a))), // GetInCurrentLocAlias
        562 => b(subject_location(e, subject).is_some_and(|l| e.has_keyword(l, p1))), // LocationHasKeyword
        565 => b(editor_location(e, subject).is_some_and(|l| e.location_within(l, p1))), // GetIsEditorLocation
        567 => b(editor_location(e, subject)
            .zip(alias(e, ctx, c.p1))
            .is_some_and(|(l, a)| e.location_within(l, a))), // GetIsEditorLocAlias
        181 | 604 => {
            // HasSameEditorLocAsRefAlias / IsInSameCurrentLocAsRefAlias (alias, keyword):
            // both locations, or their parents with the keyword, are the same.
            let other = alias(e, ctx, c.p1);
            let (mine, theirs) = if c.func == 181 {
                (
                    editor_location(e, subject),
                    other.and_then(|r| e.editor_location(r)),
                )
            } else {
                (
                    subject_location(e, subject),
                    other.and_then(|r| e.ref_current_location(r)),
                )
            };
            let kw = FormId(c.p2);
            let up = |l: Option<FormId>| l.and_then(|l| e.location_with_keyword(l, kw));
            b(up(mine).is_some_and(|m| up(theirs) == Some(m)))
        }
        605 => b(alias(e, ctx, c.p1) == Some(FormId(c.p2))), // LocAliasIsLocation
        610 => b(alias(e, ctx, c.p1).is_some_and(|l| e.has_keyword(l, FormId(c.p2)))), // LocAliasHasKeyword
        561 => b(subject.is_some_and(|s| e.ref_types(s).contains(&p1))),               // HasRefType
        563 => b(subject_location(e, subject)
            .is_some_and(|l| !e.location_refs_of_type(l, p1).is_empty())), // LocationHasRefType
        503 => b(true), // GetAllowWorldInteractions
        641 => b(subj_base.is_some_and(|n| {
            e.lo.get(n)
                .and_then(|r| r.get(b"ACBS").map(|d| d[0] & 0x20 != 0))
                .unwrap_or(false)
        })), // IsUnique
        555 => b(subject.is_some_and(|s| s == PLAYER_REF || e.actor_cells.contains_key(&s))), // HasLoaded3D
        606 => Some(
            e.location_keyword_data
                .get(&(p1, FormId(c.p2)))
                .copied()
                .unwrap_or(0.0),
        ), // GetKeywordDataForLocation
        651 => Some(
            e.current_location()
                .and_then(|l| e.location_keyword_data.get(&(l, p1)).copied())
                .unwrap_or(0.0),
        ), // GetKeywordDataForCurrentLocation
        403 => Some(e.relationship_rank(subject?, p1) as f32), // GetRelationshipRank
        258 => b(e.has_association(subject?, Some(p1), FormId(c.p2))), // HasAssociationType
        259 => b(e.has_family_relationship(subject?, Some(p1))), // HasFamilyRelationship
        261 => b(e.has_parent_relationship(subject?, (!p1.is_null()).then_some(p1))), // HasParentRelationship
        617 => b(e.has_association(subject?, None, p1)), // HasAssociationTypeAny
        618 => b(e.has_family_relationship(subject?, None)), // HasFamilyRelationshipAny
        615 | 616 => Some(e.relationship_extreme(subject?, c.func == 615) as f32), // GetHighest / GetLowestRelationshipRank
        726 => b(subject.is_none()),                                               // DoesNotExist
        372 => b(subject.is_some_and(|s| {
            e.formlist(p1)
                .iter()
                .any(|&f| f == s || subj_base == Some(f))
        })), // IsInList
        453 => b(false), // GetPlayerTeammate: no followers yet
        448 => b(subject.is_some_and(|s| e.has_perk(s, p1))), // HasPerk
        // IsWeaponSkillType: of a weapon (a perk's weapon tab), else the
        // subject's weapon.
        109 => b(subject
            .and_then(|s| {
                if e.lo.tag_of(s).is_some_and(|t| t.0 == *b"WEAP") {
                    Some(s)
                } else {
                    e.weapon_of(s)
                }
            })
            .and_then(|w| crate::perks::weapon_skill(&e.lo, w))
            == Some(c.p1)),
        682 => b(subject.is_some_and(|s| e.worn_with_keyword(s, p1) > 0)), // WornHasKeyword
        722 => Some(subject.map_or(0, |s| e.worn_with_keyword(s, p1)) as f32), // WornApparelHasKeywordCount
        569 => b(subject.is_some_and(|s| e.is_blocking(s))),                   // IsBlocking
        // IsSprinting: only the player sprints so far (NPCs run).
        568 => b(subject == Some(crate::engine::PLAYER_REF) && e.player.sprinting),
        // GetLastHitCritical: the latest blow the subject took was a critical strike.
        508 => b(subject.is_some_and(|s| e.critically_hit.contains(&s))),
        // IsAttackType: the attack it is making has that type keyword.
        697 => b(subject.and_then(|s| e.attack_type_of(s)) == Some(p1)),
        // Magic (`crate::magic`).
        214 => b(subject.is_some_and(|s| e.has_magic_effect(s, p1))), // HasMagicEffect
        699 => b(subject.is_some_and(|s| e.has_magic_effect_keyword(s, p1))), // HasMagicEffectKeyword
        264 => b(subject.is_some_and(|s| e.has_spell(s, p1))),                // HasSpell
        223 => b(subject.is_some_and(|s| e.is_spell_target(s, p1))),          // IsSpellTarget
        // EPMagic_SpellHasKeyword / SpellHasSkill: of the spell a perk entry
        // point asks about (its effects').
        693 | 696 => b(subject.and_then(|s| e.magic_item(s)).is_some_and(|m| {
            m.effects.iter().any(|x| {
                e.magic_effect(x.effect).is_some_and(|f| {
                    if c.func == 693 {
                        f.keywords.contains(&p1)
                    } else {
                        f.skill == Some(c.p1)
                    }
                })
            })
        })),
        // SpellHasCastingPerk: the spell tab's authored perk, not HasPerk
        // on its caster. Vanilla cost perks use this to select their tier.
        713 => b(subject.and_then(|s| e.spell_casting_perk(s)) == Some(p1)),
        632 => b(subject.is_some_and(|s| e.is_casting(s))), // IsCasting
        627 => b(subject.is_some_and(|s| e.is_dual_casting(s))), // IsDualCasting
        602 => b(subject.is_some_and(|s| e.ward_state(s) == c.p1 as i32)), // IsWardState
        724 => b(ctx.dual_cast),                            // EffectWasDualCast
        161 => b(subject.is_some_and(|s| e.runs_package(s, p1))), // GetIsCurrentPackage
        579 => b(subject.is_some_and(|s| e.equipped_shout(s) == Some(p1))), // GetEquippedShout
        378 => b(subject.is_some_and(|s| e.has_shout(s, p1))), // HasShout
        463 => b(e.grabbed_ref().is_some_and(|g| g == p1)), // IsPlayerGrabbedRef
        286 => b(subject.is_some_and(|s| e.is_sneaking(s))), // IsSneaking
        45 => b(subject.is_some_and(|s| e.detects(s, p1))), // GetDetected
        91 => b(subject.is_some_and(|s| e.is_alerted(s))),  // GetIsAlerted
        711 => Some(subject.map_or(0.0, |s| e.light_level(s))), // GetLightLevel
        255 => b(subject.is_some_and(|s| e.offers_services_now(s))), // GetOffersServicesNow
        289 => {
            b(subject.is_some_and(|s| e.combat_state(s) != crate::ai::combat::CombatState::None))
        } // IsInCombat
        580 => b(subject.is_some_and(|s| e.is_bleeding_out(s))), // IsBleedingOut
        332 => b(subject.is_some_and(|s| e.is_in_dangerous_water(s))),
        // Nobody flies, swims or feeds yet.
        101 | 185 | 226 => b(false),
        // Behaviour graph variables of the subject's graph (the idle-picking
        // fallback below answers for actors without one).
        675 | 447 if subject.is_some_and(|s| e.graph_variable(s, "").is_some()) => {
            subject.and_then(|s| e.graph_variable(s, c.string_p1.as_deref().unwrap_or("")))
        }
        // The line being said, while saying it.
        434 | 435 => {
            let line = e
                .conversation
                .as_ref()
                .filter(|cv| Some(cv.npc_ref) == subject)?
                .current
                .as_ref()?;
            Some(if c.func == 434 {
                line.emotion.0
            } else {
                line.emotion.1
            } as f32)
        }
        623 => Some(subject.and_then(|s| e.actor_speed(s)).unwrap_or(0.0)), // GetMovementSpeed
        // GetEquippedItemType (left 0, right 1).
        597 => Some(
            subject
                .and_then(|s| e.inventories.get(&s))
                .map_or(0, |i| i.hand(&e.lo, c.p1 == 0) as i32) as f32,
        ),
        5 => b(subject.is_some_and(|s| e.is_locked(s))), // GetLocked
        65 => Some(
            subject
                .and_then(|s| e.lock_of(s))
                .map_or(0.0, |l| l.level as f32),
        ), // GetLockLevel
        // GetItemCount; picking a furniture idle ahead of time (no subject), the
        // query says whether its user holds what the idle asks about.
        47 => match (subject, ctx.idle) {
            (None, Some(q)) => b(q.holding),
            _ => Some(subject.map_or(0, |s| e.item_count(s, p1)) as f32),
        },
        182 => b(subject
            .and_then(|s| e.inventories.get(&s))
            .is_some_and(|i| i.is_equipped(p1))), // GetEquipped
        263 => b(subject.is_some_and(|s| e.weapon_drawn(s))), // IsWeaponOut
        // Outside idle picking, sitting / sleeping come from what the actor is doing.
        159 | 49 if ctx.idle.is_none() => {
            Some(subject.map_or(0.0, |s| e.sit_sleep_state(s, c.func == 49)))
        }
        237 => b(false), // GetIsGhost
        353 => b(subject.is_some_and(|s| {
            s == PLAYER_REF
                || e.created(s).is_some_and(|c| c.actor)
                || e.lo.get(s).is_some_and(|r| r.tag().0 == *b"ACHR")
        })), // IsActor
        362 => b(subject
            .and_then(|s| e.linked_ref(s, (!p1.is_null()).then_some(p1)))
            .is_some()), // HasLinkedRef
        650 => b(
            subject.and_then(|s| e.linked_ref(s, (c.p2 != 0).then_some(FormId(c.p2)))) == Some(p1),
        ), // IsLinkedTo
        // IsMoving / IsPathing outside idle picking: walking about.
        25 | 704 if ctx.idle.is_none() => b(subject
            .and_then(|s| e.actor_speed(s))
            .is_some_and(|v| v > 1.0)),
        // Nor flees, attacks, staggers, recoils or is ridden.
        329 => b(subject.is_some_and(|s| e.is_fleeing(s))), // IsFleeing
        477 => Some(subject.map_or(0.0, |s| e.threat_ratio(s, p1))), // GetThreatRatio
        672 | 701 | 702 | 714 => b(false),
        _ => idle_function_value(e, c, ctx),
    }
}

/// What alias `id` of the condition's quest holds.
fn alias(e: &Engine, ctx: Context, id: u32) -> Option<FormId> {
    ctx.quest.and_then(|q| e.alias_ref(q, id))
}

/// The location a condition's subject (else the player) is in: itself for a location.
fn subject_location(e: &Engine, subject: Option<FormId>) -> Option<FormId> {
    let s = subject.unwrap_or(PLAYER_REF);
    if e.is_location(s) {
        Some(s)
    } else {
        e.ref_current_location(s)
    }
}

/// The subject's editor location: itself for a location.
fn editor_location(e: &Engine, subject: Option<FormId>) -> Option<FormId> {
    let s = subject?;
    if e.is_location(s) {
        Some(s)
    } else {
        e.editor_location(s)
    }
}

/// Furniture and idle functions, answered from the query when picking idles.
fn idle_function_value(e: &Engine, c: &Condition, ctx: Context) -> Option<f32> {
    let q = ctx.idle?;
    let furniture_base = ctx.target.and_then(|t| e.base_of(t));
    match c.func {
        159 => Some(if q.anim_type == 2 { 0.0 } else { q.state }), // GetSitting
        49 => Some(if q.anim_type == 2 { q.state } else { 0.0 }),  // GetSleeping
        631 | 703 => b(q.quick),   // IsEntering / IsExitingInteractionQuick
        637 => b(q.instant),       // IsExitingInstant
        614 => b(q.entry == c.p1), // IsFurnitureEntryType
        613 => b(q.anim_type == c.p1), // IsFurnitureAnimType
        163 => b(furniture_base == Some(FormId(c.p1))), // IsCurrentFurnitureObj
        247 => b(q.eating),        // GetIsUsedItemType
        25 | 704 => b(false),      // IsMoving, IsPathing
        // GetGraphVariableInt: settled in a pose (not mid-transition).
        675 => Some(
            if c.string_p1
                .as_deref()
                .is_some_and(|v| v.eq_ignore_ascii_case("bNeutralState"))
            {
                1.0
            } else {
                0.0
            },
        ),
        _ => None,
    }
}

impl Engine {
    pub fn npc_race(&self, npc: FormId) -> Option<FormId> {
        let rec = self.lo.get(npc)?;
        let d = rec.get(b"RNAM")?;
        Some(rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().ok()?))))
    }

    /// Factions and ranks of an actor (a reference, or an NPC record), from its
    /// templates when they give them.
    pub fn npc_factions(&self, actor: FormId) -> Vec<(FormId, i8)> {
        let mut out = self
            .templates_of(actor)
            .map(|t| t.factions(&self.lo))
            .unwrap_or_default();
        if let Some(changes) = self.faction_changes_of(actor) {
            for (&faction, &rank) in changes {
                match out.iter_mut().find(|x| x.0 == faction) {
                    Some(x) => x.1 = rank,
                    None => out.push((faction, rank)),
                }
            }
        }
        out
    }

    /// What scripts changed of an actor's factions. An NPC record stands for
    /// its first reference (the player's record for the player).
    fn faction_changes_of(&self, actor: FormId) -> Option<&std::collections::HashMap<FormId, i8>> {
        if self.scripts.faction_changes.is_empty() {
            return None;
        }
        let actor = match actor {
            FormId(0x7) => PLAYER_REF,
            a if self.lo.tag_of(a).is_some_and(|t| t.0 == *b"NPC_") => {
                self.npc_refs_index().get(&a).copied()?
            }
            a => a,
        };
        self.scripts.faction_changes.get(&actor)
    }

    /// An actor's rank in a faction: -1 when it isn't in it.
    pub fn faction_rank(&self, actor: FormId, faction: FormId) -> i32 {
        self.npc_factions(actor)
            .iter()
            .find(|x| x.0 == faction)
            .map_or(-1, |x| i32::from(x.1))
    }

    /// Puts an actor in a faction at a rank (-1: out of it).
    pub fn set_faction_rank(&mut self, actor: FormId, faction: FormId, rank: i32) {
        let actor = if actor == FormId(0x7) {
            PLAYER_REF
        } else {
            actor
        };
        let rank = rank.clamp(-1, 127) as i8;
        if self.faction_rank(actor, faction) != i32::from(rank) {
            log::info!("{actor} faction {faction}: rank {rank}");
        }
        self.scripts
            .faction_changes
            .entry(actor)
            .or_default()
            .insert(faction, rank);
    }

    pub fn npc_voice_type(&self, npc: FormId) -> Option<FormId> {
        let rec = self.lo.get(npc)?;
        let d = rec.get(b"VTCK")?;
        Some(rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().ok()?))))
    }

    /// An actor's voice type: its base's, else the one its templates give it
    /// for traits (leveled actors such as bandits).
    pub fn actor_voice_type(&self, r: FormId) -> Option<FormId> {
        self.base_of(r)
            .and_then(|n| self.npc_voice_type(n))
            .or_else(|| {
                self.templates_of(r)?
                    .form(&self.lo, crate::world::template::TRAITS, b"VTCK")
            })
    }

    pub fn race_is_child(&self, race: FormId) -> bool {
        // RACE DATA flags bit 0x4 = child.
        self.lo
            .get(race)
            .and_then(|r| {
                r.get(b"DATA").map(|d| {
                    d.len() >= 36 && u32::from_le_bytes(d[32..36].try_into().unwrap()) & 0x4 != 0
                })
            })
            .unwrap_or(false)
    }

    /// Location (LCTN) of the current cell.
    pub fn current_location(&self) -> Option<FormId> {
        let cell = match self.location {
            crate::engine::Location::Interior(c) => Some(c),
            crate::engine::Location::Exterior { world, center } => self
                .lo
                .world(world)
                .and_then(|w| w.cells.get(&center).copied()),
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
            let Some(rec) = self.lo.get(l) else {
                return false;
            };
            let Some(d) = rec.get(b"PNAM") else {
                return false;
            };
            l = rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap())));
        }
        false
    }

    /// Whether an NPC is in a vendor faction whose hours include the current time.
    pub fn offers_services_now(&self, actor: FormId) -> bool {
        self.npc_factions(actor).iter().any(|(f, _)| {
            let Some(r) = self.lo.get(*f) else {
                return false;
            };
            let flags = r
                .get(b"DATA")
                .map(|d| u32::from_le_bytes(d[0..4].try_into().unwrap()))
                .unwrap_or(0);
            if flags & 0x4000 == 0 {
                return false;
            }
            match r.get(b"VENV") {
                Some(v) if v.len() >= 4 => {
                    let (start, end) = (
                        u16::from_le_bytes([v[0], v[1]]) as f32,
                        u16::from_le_bytes([v[2], v[3]]) as f32,
                    );
                    start == end || (self.hour >= start && self.hour < end)
                }
                _ => true,
            }
        })
    }

    /// Dialogue conditions declared on a quest (CTDAs before the NEXT marker).
    pub fn quest_dialogue_conditions(&self, quest: FormId) -> Vec<Condition> {
        self.quest_conditions(quest, false)
    }

    /// A quest's Story Manager event conditions (CTDAs after the NEXT marker,
    /// before the stages): checked against the event when a node starts it.
    pub fn quest_event_conditions(&self, quest: FormId) -> Vec<Condition> {
        self.quest_conditions(quest, true)
    }

    fn quest_conditions(&self, quest: FormId, event: bool) -> Vec<Condition> {
        let Some(rec) = self.lo.get(quest) else {
            return Vec::new();
        };
        let mut out: Vec<Condition> = Vec::new();
        let mut after_next = false;
        for s in rec.subrecords() {
            match &s.tag.0 {
                b"NEXT" if event && !after_next => after_next = true,
                b"NEXT" | b"INDX" | b"QOBJ" | b"ALST" | b"ALLS" | b"ANAM" => break,
                _ if event != after_next => {}
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
        self.scripts
            .quests
            .get(&quest)
            .and_then(|q| q.aliases.get(&alias).copied())
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
