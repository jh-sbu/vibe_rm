//! Condition (CTDA) evaluation.

pub mod functions;

use esp::{FormId, LoadedRecord};
use glam::Vec3;

use crate::engine::{Engine, PLAYER_REF};
use crate::spells::{LEFT, RIGHT, VOICE};

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
    /// The package input a condition reads, worked out when the package is parsed:
    /// GetNumericPackageData and GetWithinPackageLocation take it from their first
    /// parameter, GetWithinDistance its distance from its second. `None`: the input
    /// is missing or of the wrong kind, and the condition fails.
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
pub const IS_NULL_PACKAGE_DATA: u16 = 611;
/// Its second parameter is the package's distance input when the condition takes
/// package inputs (see `Condition::package_data`).
pub const GET_WITHIN_DISTANCE: u16 = 639;

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

/// The engine's hand (or voice) slot of a casting source (`ptCastingSource`):
/// 0 the left hand, 1 the right, 2 the other (the voice), as CommonLibSSE's
/// `CastingSource`. None for anything else.
fn casting_hand(source: u32) -> Option<usize> {
    match source {
        0 => Some(LEFT),
        1 => Some(RIGHT),
        2 => Some(VOICE),
        _ => None,
    }
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

/// One condition alone, its OR / AND link ignored.
pub fn evaluate_one(e: &Engine, c: &Condition, ctx: Context) -> bool {
    eval_one(e, c, ctx)
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

/// The record type a `ptFormType` parameter names (xEdit's `wbFormTypeEnum`, not
/// CommonLibSSE's FormType: vanilla passes 1 for armor, 6 light, 12 weapon and 13
/// actor). None for values the enum doesn't list.
fn form_type_tag(value: u32) -> Option<&'static [u8; 4]> {
    Some(match value {
        0 => b"ACTI",
        1 => b"ARMO",
        2 => b"BOOK",
        3 => b"CONT",
        4 => b"DOOR",
        5 => b"INGR",
        6 => b"LIGH",
        7 => b"MISC",
        8 => b"STAT",
        9 => b"GRAS",
        10 => b"TREE",
        12 => b"WEAP",
        13 => b"NPC_",
        14 => b"LVLN",
        15 => b"SPEL",
        16 => b"ENCH",
        17 => b"ALCH",
        18 => b"LVLI",
        19 => b"KEYM",
        20 => b"AMMO",
        21 => b"FLOR",
        22 => b"FURN",
        23 => b"SOUM",
        24 => b"LTEX",
        25 => b"CSTY",
        26 => b"LSCR",
        27 => b"LVSP",
        28 => b"ANIO",
        29 => b"WATR",
        30 => b"IDLM",
        31 => b"EFSH",
        32 => b"PROJ",
        33 => b"TACT",
        34 => b"EXPL",
        35 => b"TXST",
        36 => b"DEBR",
        37 => b"MICN",
        38 => b"FLST",
        39 => b"PERK",
        40 => b"BPTD",
        41 => b"ADDN",
        42 => b"MSTT",
        43 => b"CAMS",
        44 => b"IPCT",
        45 => b"IPDS",
        46 => b"QUST",
        47 => b"PACK",
        48 => b"VTYP",
        49 => b"CLAS",
        50 => b"RACE",
        51 => b"EYES",
        52 => b"HDPT",
        53 => b"FACT",
        54 => b"NOTE",
        55 => b"WTHR",
        56 => b"CLMT",
        57 => b"ARMA",
        58 => b"GLOB",
        59 => b"IMGS",
        60 => b"IMAD",
        61 => b"ECZN",
        62 => b"MESG",
        63 => b"COBJ",
        64 => b"ASPC",
        65 => b"RGDL",
        66 => b"SCPT",
        67 => b"MGEF",
        68 => b"MUSC",
        69 => b"SCOL",
        70 => b"KYWD",
        71 => b"LCTN",
        72 => b"LCRT",
        73 => b"FSTP",
        74 => b"FSTS",
        75 => b"MATT",
        76 => b"AACT",
        77 => b"MUST",
        78 => b"WOOP",
        79 => b"SHOU",
        80 => b"RELA",
        81 => b"EQUP",
        82 => b"ASTP",
        83 => b"OTFT",
        84 => b"ARTO",
        85 => b"MATO",
        87 => b"LGTM",
        88 => b"SPGD",
        89 => b"RFCT",
        90 => b"APPA",
        91 => b"MOVT",
        92 => b"HAZD",
        93 => b"SMEN",
        94 => b"SNDR",
        95 => b"DUAL",
        96 => b"SNCT",
        97 => b"SLGM",
        98 => b"SOPM",
        99 => b"COLL",
        100 => b"SCRL",
        101 => b"CLFM",
        102 => b"RVRB",
        _ => return None,
    })
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
        // The base object's record type, as the condition's `ptFormType` numbers it.
        432 => b(subj_base
            .and_then(|n| e.lo.tag_of(n))
            .is_some_and(|t| form_type_tag(c.p1) == Some(&t.0))), // GetIsObjectType
        // The subject's weapon in hand is one the form list names.
        398 => b(subject
            .and_then(|s| e.weapon_of(s))
            .is_some_and(|w| e.formlist(p1).contains(&w))), // IsWeaponInList
        // Rank -1 (potential followers' `CurrentFollowerFaction`) isn't membership.
        71 => b(subject.is_some_and(|s| e.is_member(s, p1))), // GetInFaction
        132 => b(e.is_member(PLAYER_REF, p1)),                // GetPCInFaction
        42 => b(subject.is_some_and(|s| e.same_faction(s, p1))), // SameFaction
        133 => b(subject.is_some_and(|s| e.same_faction(s, PLAYER_REF))), // SameFactionAsPC
        43 => b(subject.is_some_and(|s| e.same_race(s, p1))), // SameRace
        134 => b(subject.is_some_and(|s| e.same_race(s, PLAYER_REF))), // SameRaceAsPC
        44 => b(subject.is_some_and(|s| e.same_sex(s, p1))),  // SameSex
        135 => b(subject.is_some_and(|s| e.same_sex(s, PLAYER_REF))), // SameSexAsPC
        68 => b(subject.and_then(|s| e.class_of(s)) == Some(p1)), // GetIsClass
        129 => b(e.class_of(PLAYER_REF) == Some(p1)),         // GetPCIsClass
        // Gold is the subject's inventory's, as `GetItemCount` reads it.
        48 => Some(subject.map_or(0, |s| e.item_count(s, crate::crime::GOLD)) as f32), // GetGold
        // Fractions of the most (`GetActorValuePercent`), stamina and health.
        128 => Some(e.actor_value_fraction(subject?, esp::actor_value::STAMINA)), // GetStaminaPercentage
        430 => Some(e.actor_value_fraction(subject?, esp::actor_value::HEALTH)), // GetHealthPercentage
        // Only the player's carry weight is tracked (`player_overencumbered`).
        734 => b(subject == Some(PLAYER_REF) && e.player_overencumbered()), // IsOverEncumbered
        // Journal statistics by name (`CIS1`); a name it doesn't know counts 0.
        312 => Some(e.query_stat(c.string_p1.as_deref()?) as f32), // GetPCMiscStat
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
        560 => b(subject.is_some_and(|s| e.has_keyword(s, p1))),   // HasKeyword
        // How the first faction regards the second in combat (0 neutral, 1 enemy,
        // 2 ally, 3 friend).
        410 => Some(e.faction_combat_reaction(p1, FormId(c.p2)) as f32), // GetFactionCombatReaction
        // The player's state: hours passing in bed, and the first-person camera.
        175 => b(e.waiting.is_some_and(|w| w.sleeping)), // IsPCSleeping
        391 => b(!e.third_person),                       // IsPC1stPerson
        530 => Some(e.xp_for_next_level()),              // GetXPForNextLevel
        // Ownership (`ref_owner`: a script's, else the record's, else the cell's):
        // the subject's owner is the one named (none named: the player).
        278 => b(subject.is_some_and(|s| e.owned_by(e.ref_owner(s), p1))), // IsOwner
        // The cell's owner is the second parameter's (none named: the player).
        280 => b(e.owned_by(e.ref_owner(p1), FormId(c.p2))), // IsCellOwner
        // The subject stands in a cell it owns.
        146 => b(subject.is_some_and(|s| {
            let cell = match (s, e.location) {
                (PLAYER_REF, crate::engine::Location::Interior(c)) => Some(c),
                (PLAYER_REF, _) => None,
                _ => e.lo.cell_of_ref(s),
            };
            cell.is_some_and(|c| e.is_owner_of(s, e.ref_owner(c)))
        })), // IsInMyOwnedCell
        // A greatsword, battleaxe or warhammer in the right hand (the animation
        // types that take both hands); bows and staves are not counted.
        636 => b(subject
            .and_then(|s| e.weapon_in_hand(s, RIGHT))
            .and_then(|w| e.lo.get(w))
            .is_some_and(|w| matches!(crate::world::inventory::weapon_anim_type(&w), 5 | 6))), // HasTwoHandedWeaponEquipped
        // A bound weapon in the hand the casting source names (the voice has none).
        706 => b(subject.is_some_and(|s| {
            casting_hand(c.p1)
                .filter(|&h| h != VOICE)
                .and_then(|h| e.weapon_in_hand(s, h))
                .is_some_and(|w| crate::world::inventory::weapon_bound(&e.lo, w))
        })), // HasBoundWeaponEquipped
        465 => Some(subject.map_or(0, |s| e.keyword_item_count(s, p1)) as f32), // GetKeywordItemCount
        // The subject's race (templates' `RNAM`, as `SameRace` reads it) is a
        // playable one.
        254 => b(subject
            .and_then(|s| e.race_of(s))
            .is_some_and(|r| e.race_is_playable(r))), // GetIsPlayableRace
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
        // Game hours since the subject died in play (no value for the living, or
        // for those dead before the game: unsupported). Unit unconfirmed
        // (`known_ambiguities/death.md`).
        361 => subject
            .and_then(|s| e.death(s))
            .map(|d| (e.game_hours_total() - d.at) as f32), // GetTimeDead
        // The subject killed the actor named (the victim).
        408 => b(subject.is_some_and(|s| e.death(p1).is_some_and(|d| d.killer == Some(s)))), // IsKiller
        // The subject killed one of the actors (or their bases) the list names.
        409 => b(subject.is_some_and(|s| {
            let list = e.formlist(p1);
            e.world_state.deaths.iter().any(|(&v, d)| {
                d.killer == Some(s) && list.iter().any(|&f| f == v || e.base_of(v) == Some(f))
            })
        })), // IsKillerObject
        80 => Some(e.actor_level(subject?) as f32), // GetLevel
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
        // The faction's player flags (`crate::factions`), whoever the subject.
        193 => b(e.player_expelled_from(p1)), // GetPCExpelled
        197 => b(e.player_enemy_of(p1)),      // GetPCEnemyofFaction
        458 => b(subject.is_some_and(|s| e.crime_player_enemy(s))), // GetActorCrimePlayerEnemy
        499 => Some(e.crime.arrests.days_in_jail as f32), // GetDaysInJail
        657 => b(subject.is_some_and(|s| e.crime.arrests.arresting.is_some_and(|a| a.0 == s))), // GetArrestingActor
        459 | 375 | 376 | 533 | 534 | 535 => {
            // GetCrimeGold (Violent / Nonviolent) and GetInfamy (all / Violent /
            // Nonviolent): the player's with the faction given, else the
            // subject's crime faction; the player's own, all.
            let pick = |b: crate::crime::Bounty| match c.func {
                375 => b.violent,
                376 => b.nonviolent,
                533 => b.infamy_violent + b.infamy_nonviolent,
                534 => b.infamy_violent,
                535 => b.infamy_nonviolent,
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
        366 | 373 if subject == Some(PLAYER_REF) => {
            Some(e.stolen_item_value(p1, c.func == 373) as f32)
        }
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
        1 => distance_between(e, subject?, p1), // GetDistance
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
            // GetVMScriptVariable(reference, "::name_var"): the variable of a
            // script on the reference named, not the subject (vanilla dialogue
            // asks the speaker about triggers such as `MQ106DelphineBedroomTrigger`).
            // A null reference holds no scripts.
            let name = c.string_p2.as_deref()?;
            Some(
                named(p1)
                    .and_then(|r| e.vm.get_var(papyrus::ObjectId::Form(r.0), name))
                    .map(|v| v.as_float())
                    .unwrap_or(0.0),
            )
        }
        // GetScriptVariable(reference, "name"): the reference's script variable, read
        // like GetVMScriptVariable's; the name comes in CIS2 (see `parse_all_before`).
        // Vanilla data never calls it (`known_ambiguities/condition-variables.md`).
        53 => {
            let name = c.string_p2.as_deref()?;
            Some(
                e.vm.get_var(papyrus::ObjectId::Form(p1.0), name)
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
        180 | 181 | 603 | 604 => {
            // HasSameEditorLocAsRef / IsInSameCurrentLocAsRef (a reference, keyword)
            // and their Alias forms: both locations, or their parents with the
            // keyword, are the same.
            let other = if matches!(c.func, 180 | 603) {
                named(p1)
            } else {
                alias(e, ctx, c.p1)
            };
            let editor = c.func < 200;
            let (mine, theirs) = if editor {
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
        // The subject is in a location (or inside one) a form list names.
        444 => b(subject_location(e, subject)
            .is_some_and(|l| e.formlist(p1).iter().any(|&t| e.location_within(l, t)))), // GetInCurrentLocFormList
        // Some loaded cell lies in the location (or inside it).
        625 => b(e
            .loaded_locations()
            .into_iter()
            .any(|l| e.location_within(l, p1))), // IsLocationLoaded
        626 => b(alias(e, ctx, c.p1).is_some_and(|t| {
            e.loaded_locations()
                .into_iter()
                .any(|l| e.location_within(l, t))
        })), // IsLocAliasLoaded
        605 => b(alias(e, ctx, c.p1) == Some(FormId(c.p2))), // LocAliasIsLocation
        610 => b(alias(e, ctx, c.p1).is_some_and(|l| e.has_keyword(l, FormId(c.p2)))), // LocAliasHasKeyword
        561 => b(subject.is_some_and(|s| e.ref_types(s).contains(&p1))),               // HasRefType
        563 => b(subject_location(e, subject)
            .is_some_and(|l| !e.location_refs_of_type(l, p1).is_empty())), // LocationHasRefType
        // The references of a ref type in a location (or inside it), the living
        // and the dead ones; the alias forms ask about the location alias's.
        591 | 592 => Some(e.location_population(p1, FormId(c.p2), c.func == 592) as f32), // GetRefTypeDeadCount / GetRefTypeAliveCount
        600 | 601 => Some(match alias(e, ctx, c.p1) {
            Some(l) => e.location_population(l, FormId(c.p2), c.func == 601),
            None => 0,
        } as f32), // GetLocAliasRefTypeDeadCount / GetLocAliasRefTypeAliveCount
        608 => Some(
            alias(e, ctx, c.p1)
                .and_then(|l| e.location_keyword_data.get(&(l, FormId(c.p2))).copied())
                .unwrap_or(0.0),
        ), // GetKeywordDataForAlias
        // The player's cell lies in the region (its `XCLR`).
        117 => b(e
            .player_cell()
            .is_some_and(|c| e.cell_regions(c).contains(&p1))), // IsPlayerInRegion
        528 => b(subject.is_some_and(|s| e.critical_stage(s) == c.p1 as i32)), // IsInCriticalStage
        445 => b(subject.and_then(|s| e.encounter_zone_of(s)) == Some(p1)),    // GetInZone
        // Menu mode: a menu that pauses the game is open. Other menu numbers
        // (Oblivion's) name no Skyrim menu here, so they are unsupported.
        36 => (c.p1 == 0).then(|| e.in_menu_mode() as u32 as f32), // MenuMode
        // The controls the two parameters select (movement, fighting) are all
        // disabled; none selected asks whether any control is.
        98 => {
            let d = e.disabled_controls;
            b(match (c.p1 != 0, c.p2 != 0) {
                (false, false) => d.any(),
                (m, f) => (!m || d.movement) && (!f || d.fighting),
            })
        } // GetPlayerControlsDisabled
        // Game time moves when the world runs, and while waiting or sleeping.
        265 => b(!e.in_menu_mode() || e.waiting.is_some()), // IsTimePassing
        716 => Some(e.scripts.wall_time as f32 / 3600.0),   // GetRealHoursPassed
        12 => Some(e.frame_seconds),                        // GetSecondsPassed
        // A package input (`package_data`: 1 when unset).
        611 => match c.package_data {
            Some(PackageData::Number(v)) => Some(v),
            _ => None,
        }, // IsNullPackageData
        // The subject's scale: a loaded actor's, else a placed reference's.
        24 => Some(e.ref_scale(subject?)), // GetScale
        // Creatures are the actors whose race carries `ActorTypeCreature`.
        64 => b(subject.is_some_and(|s| e.has_keyword(s, FormId(0x13795)))), // GetIsCreature
        // How the subject's factions regard the actor's in combat (0 neutral, 1
        // enemy, 2 ally, 3 friend); no actor means the player.
        449 => {
            Some(e.faction_relation(subject?, if p1.is_null() { PLAYER_REF } else { p1 }) as f32)
        } // GetFactionRelation
        // The subject's rank in the faction less the actor's (-1: not a member).
        60 => Some((e.faction_rank(subject?, p1) - e.faction_rank(FormId(c.p2), p1)) as f32), // GetFactionRankDifference
        // The animation type of the weapon in the right hand, else the left; 0
        // (hand to hand) with none.
        108 => Some(
            subject
                .and_then(|s| {
                    e.weapon_in_hand(s, RIGHT)
                        .or_else(|| e.weapon_in_hand(s, LEFT))
                })
                .and_then(|w| e.lo.get(w))
                .map_or(0, |w| crate::world::inventory::weapon_anim_type(&w)) as f32,
        ), // GetWeaponAnimType
        // The subject's base object is one that can be picked up.
        487 => b(subj_base.is_some_and(|n| e.is_carryable(n))), // IsCarryable
        // Heights between references: the reference's less the subject's
        // (null: the subject's combat target; the activated one for 680).
        720 => {
            let s = subject?;
            let t = reference_or_target(e, s, p1)?;
            Some(e.ref_position(t)?.z - e.ref_position(s)?.z)
        } // GetTargetHeight
        680 => Some(e.ref_position(ctx.target?)?.z - e.ref_position(subject?)?.z), // GetActivatorHeight
        // The base value of the clothes worn; a disease from any source on the subject.
        41 => Some(subject.map_or(0, |s| e.clothing_value(s)) as f32), // GetClothingValue
        39 => b(subject.is_some_and(|s| e.has_disease(s))),            // GetDisease
        306 => b(subject.is_some_and(|s| e.torch_out(s))),             // IsActorUsingATorch
        230 => b(e.ref_cell(FormId(c.p2)) == Some(p1)),                // GetInCellParam
        // Combat: the combat group the subject fights in (its side of the fight,
        // the player included while it is fought) and its targets.
        709 => Some(
            subject
                .and_then(|s| e.combat_group(s))
                .map_or(0, |g| g.0.len()) as f32,
        ), // GetCombatGroupMemberCount
        416 => Some(
            subject
                .and_then(|s| e.combat_group(s))
                .map_or(0, |g| g.1.len()) as f32,
        ), // GetGroupTargetCount
        // The only one of its side still fighting.
        674 => b(subject
            .and_then(|s| e.combat_group(s))
            .is_some_and(|g| g.0.len() == 1)), // IsLastHostileActor
        707 => b(subject
            .and_then(|s| e.combat_target(s))
            .is_some_and(|t| e.has_keyword(t, p1))), // GetCombatTargetHasKeyword
        // Lethal intent: the actor (null: the subject's combat target, else the
        // context's) is one it can kill.
        678 => {
            let s = subject?;
            let victim = named(p1).or_else(|| e.combat_target(s)).or(ctx.target)?;
            b(!e.spared_by(victim, s))
        } // ShouldAttackKill
        705 => b(subject.is_some_and(|s| e.should_help(s, p1))), // GetShouldHelp
        476 => b(subject.is_some_and(|s| e.is_protected(s))),    // IsProtected
        118 => b(subject.is_some_and(|s| e.aggro_radius_violated(s))), // GetActorAggroRadiusViolated
        503 => b(true),                                                // GetAllowWorldInteractions
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
        453 => b(subject.is_some_and(|s| e.is_player_teammate(s))), // GetPlayerTeammate
        454 => Some(e.player_teammate_count() as f32),              // GetPlayerTeammateCount
        448 => b(subject.is_some_and(|s| e.has_perk(s, p1))),       // HasPerk
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
        // EPMagic_IsAdvanceSkill: the skill a use is advancing.
        681 => b(e.advancing_skill.get() == Some(c.p1)),
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
        // Casting sources (`ptCastingSource`): the hands' and the voice's
        // spells. The voice has no casting state (powers are cast at once),
        // so its casting type, delivery and spell are unsupported.
        570 => {
            // HasEquippedSpell: a spell (or power, shout) in the slot.
            let slot = casting_hand(c.p1)?;
            b(subject.is_some_and(|s| e.equipped_spell(s, slot).is_some()))
        }
        571 | 572 => {
            let hand = casting_hand(c.p1).filter(|&h| h != VOICE)?;
            let item = subject
                .and_then(|s| e.hand_casting(s, hand))
                .and_then(|spell| e.magic_item(spell));
            // -1 when the hand casts nothing (as `GetFactionRank` does).
            Some(item.map_or(-1.0, |m| {
                (if c.func == 571 {
                    m.cast_type
                } else {
                    m.delivery
                }) as f32
            })) // GetCurrentCastingType / GetCurrentDeliveryType
        }
        595 => {
            let hand = casting_hand(c.p2).filter(|&h| h != VOICE)?;
            b(subject.and_then(|s| e.hand_casting(s, hand)) == Some(p1)) // IsCurrentSpell
        }
        596 => {
            let hand = casting_hand(c.p1).filter(|&h| h != VOICE)?;
            b(subject
                .and_then(|s| e.hand_casting(s, hand))
                .is_some_and(|spell| e.spell_has_keyword(spell, FormId(c.p2)))) // SpellHasKeyword
        }
        627 => b(subject.is_some_and(|s| e.is_dual_casting(s))), // IsDualCasting
        602 => b(subject.is_some_and(|s| e.ward_state(s) == c.p1 as i32)), // IsWardState
        724 => b(ctx.dual_cast),                                 // EffectWasDualCast
        161 => b(subject.is_some_and(|s| e.runs_package(s, p1))), // GetIsCurrentPackage
        579 => b(subject.is_some_and(|s| e.equipped_shout(s) == Some(p1))), // GetEquippedShout
        378 => b(subject.is_some_and(|s| e.has_shout(s, p1))),   // HasShout
        463 => b(e.grabbed_ref().is_some_and(|g| g == p1)),      // IsPlayerGrabbedRef
        286 => b(subject.is_some_and(|s| e.is_sneaking(s))),     // IsSneaking
        45 => b(subject.is_some_and(|s| e.detects(s, p1))),      // GetDetected
        91 => b(subject.is_some_and(|s| e.is_alerted(s))),       // GetIsAlerted
        711 => Some(subject.map_or(0.0, |s| e.light_level(s))),  // GetLightLevel
        255 => b(subject.is_some_and(|s| e.offers_services_now(s))), // GetOffersServicesNow
        289 => {
            b(subject.is_some_and(|s| e.combat_state(s) != crate::ai::combat::CombatState::None))
        } // IsInCombat
        323 => Some(subject.map_or(0, |s| e.combat_state(s) as i32) as f32), // GetCombatState
        513 => b(subject.is_some_and(|s| e.combat_target(s) == Some(p1))), // IsCombatTarget
        673 => b(subject.is_some_and(|s| e.power_attacking(s))), // IsPowerAttacking
        103 => b(subject.is_some_and(|s| e.shield_out(s))),      // IsShieldOut
        102 => b(subject.is_some_and(|s| e.torch_out(s))),       // IsTorchOut
        580 => b(subject.is_some_and(|s| e.is_bleeding_out(s))), // IsBleedingOut
        332 => b(subject.is_some_and(|s| e.is_in_dangerous_water(s))),
        // Nobody flies, swims, feeds or eats corpses yet.
        101 | 185 | 226 | 227 => b(false),
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
        157 => Some(subject.map_or(0, |s| e.open_state(s) as i32) as f32), // GetOpenState
        65 => Some(
            subject
                .and_then(|s| e.lock_of(s))
                .map_or(0.0, |l| l.level as f32),
        ), // GetLockLevel
        // A door not locked now (`is_locked`); anything else isn't a door.
        718 => b(subj_base
            .and_then(|n| e.lo.tag_of(n))
            .is_some_and(|t| t.0 == *b"DOOR")
            && subject.is_some_and(|s| !e.is_locked(s))), // IsUnlockedDoor
        // Placed open by default: the reference carries `ONAM` (UESP REFR).
        215 => {
            b(subject.is_some_and(|s| e.lo.get(s).is_some_and(|rec| rec.get(b"ONAM").is_some())))
        } // GetDefaultOpen
        470 => subject.map(|s| e.current_destruction_stage(s) as f32), // GetDestructionStage
        203 => b(subject.is_some_and(|s| e.is_destroyed(s))),          // GetDestroyed
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
        237 => b(subject.is_some_and(|s| e.is_ghost(s))), // GetIsGhost
        // The platform: a PC build (UESP Skyrim:Console, oldid 3628513, "Returns
        // true if the game is running on a Windows environment"). Gore is never
        // disabled (`known_ambiguities/status-conditions.md`).
        521 => b(true),        // IsWin32
        309 | 520 => b(false), // IsXBox, IsPS3
        547 => b(false),       // IsGoreDisabled
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
        // IsRunning: moving at the running gait (the player's run toggle, held
        // down, or an NPC nearer its run speed than its walk).
        287 => b(subject.is_some_and(|s| {
            if s == PLAYER_REF {
                e.player.moving && e.player.running
            } else {
                e.actor_running(s)
            }
        })),
        // GetUnconscious: bleeding out (`known_ambiguities/death.md`).
        242 => b(subject.is_some_and(|s| e.is_bleeding_out(s))),
        // Nor flees, attacks, staggers, recoils or is ridden.
        329 => b(subject.is_some_and(|s| e.is_fleeing(s))), // IsFleeing
        477 => Some(subject.map_or(0.0, |s| e.threat_ratio(s, p1))), // GetThreatRatio
        672 | 701 | 702 | 714 => b(false),
        // Positions and angles of the subject, along an axis ('X', 'Y' or 'Z').
        // Angles are in degrees, as scripts give them (`GetAngleZ` in Papyrus).
        6 => axis_component(c.p1, e.ref_position(subject?)?), // GetPos
        8 => axis_component(c.p1, e.ref_rotation(subject?)).map(f32::to_degrees), // GetAngle
        // Where a placed reference was placed in the editor, or a created
        // reference began in the world.
        10 => axis_component(c.p1, starting_placement(e, subject?)?.position), // GetStartingPos
        11 => axis_component(c.p1, starting_placement(e, subject?)?.rotation).map(f32::to_degrees), // GetStartingAngle
        // The reference's bearing from the subject's facing, in [-180, 180).
        // A null reference names the subject's combat target, or nothing to face
        // without one (`known_ambiguities/condition-geometry.md`).
        99 => {
            let (from, s) = (e.ref_position(subject?)?, subject?);
            let to = e.ref_position(reference_or_target(e, s, p1)?)?;
            let bearing = (to.x - from.x).atan2(to.y - from.y);
            let heading = (bearing - e.ref_rotation(s).z).to_degrees();
            Some((heading + 180.0).rem_euclid(360.0) - 180.0)
        } // GetHeadingAngle
        // The reference's angle less the subject's along an axis, in [0, 360).
        584 => {
            let s = subject?;
            let other = axis_component(c.p2, e.ref_rotation(reference_or_target(e, s, p1)?))?;
            let mine = axis_component(c.p2, e.ref_rotation(s))?;
            Some((other - mine).to_degrees().rem_euclid(360.0))
        } // GetRelativeAngle
        // Within the distance the package's float input gives (its second
        // parameter); outside packages, the parameter is the float itself.
        639 => {
            let distance = match (c.package_data, c.p1_input) {
                (Some(PackageData::Number(d)), true) => d,
                (None, false) => f32::from_bits(c.p2),
                _ => return None,
            };
            b(distance_between(e, subject?, named(p1)?)? <= distance)
        } // GetWithinDistance
        577 => {
            // Nearer to the first reference than to the second (ties: no).
            let s = subject?;
            let near = distance_between(e, s, named(p1)?)?;
            let far = distance_between(e, s, named(FormId(c.p2))?)?;
            b(near < far)
        } // IsCloserToAThanB
        27 => b(e.line_of_sight(subject?, reference_or_target(e, subject?, p1)?)?), // GetLineOfSight
        _ => idle_function_value(e, c, ctx),
    }
}

/// What alias `id` of the condition's quest holds.
fn alias(e: &Engine, ctx: Context, id: u32) -> Option<FormId> {
    ctx.quest.and_then(|q| e.alias_ref(q, id))
}

/// A reference parameter that names one (a null form names nothing).
fn named(r: FormId) -> Option<FormId> {
    (!r.is_null()).then_some(r)
}

/// A reference parameter, or where it is null the subject's combat target (the
/// vanilla combat idles pass null and ask about the one being fought).
fn reference_or_target(e: &Engine, subject: FormId, r: FormId) -> Option<FormId> {
    named(r).or_else(|| e.combat_target(subject))
}

/// The component of a vector an axis parameter names: 'X', 'Y' or 'Z' (ASCII).
fn axis_component(axis: u32, v: Vec3) -> Option<f32> {
    match axis {
        88 => Some(v.x),
        89 => Some(v.y),
        90 => Some(v.z),
        _ => None,
    }
}

/// A reference's original transform: plugin placement for placed references,
/// or its saved origin for ones made during play.
fn starting_placement(e: &Engine, r: FormId) -> Option<crate::world::records::Reference> {
    if let Some(mut reference) = e.created_reference(r) {
        let origin = e.created(r)?.origin?;
        reference.position = origin.position;
        reference.rotation = origin.rotation;
        return Some(reference);
    }
    let rec = e.lo.get(r)?;
    Some(crate::world::records::reference(&rec))
}

/// The distance between two references (`GetDistance`): far beyond anything
/// between different interiors or worldspaces.
fn distance_between(e: &Engine, from: FormId, to: FormId) -> Option<f32> {
    let (pa, a) = e.current_place_of(from)?;
    let (pb, t) = e.current_place_of(to)?;
    Some(if crate::ai::schedule::same_space(pa, pb) {
        a.distance(t)
    } else {
        f32::MAX
    })
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

    /// The actor's class (`CNAM` of its NPC, through its templates).
    pub fn class_of(&self, actor: FormId) -> Option<FormId> {
        self.templates_of(actor)?
            .form(&self.lo, crate::world::template::STATS, b"CNAM")
    }

    /// Whether an actor is in a faction: rank -1 (a potential follower's) isn't membership.
    pub fn is_member(&self, actor: FormId, faction: FormId) -> bool {
        self.npc_factions(actor)
            .iter()
            .any(|&(f, r)| f == faction && r >= 0)
    }

    /// Whether two actors are members of a common faction (`SameFaction`), rank -1
    /// not counting on either side, as `GetInFaction` doesn't count it.
    pub fn same_faction(&self, a: FormId, b: FormId) -> bool {
        let mine: Vec<FormId> = self
            .npc_factions(a)
            .into_iter()
            .filter(|&(_, r)| r >= 0)
            .map(|(f, _)| f)
            .collect();
        self.npc_factions(b)
            .iter()
            .any(|&(f, r)| r >= 0 && mine.contains(&f))
    }

    /// Whether an `owner` (an NPC or faction, `None` unowned) is the one named:
    /// null names the player, whose faction counts as theirs too.
    fn owned_by(&self, owner: Option<FormId>, named: FormId) -> bool {
        let Some(owner) = owner else { return false };
        if named.is_null() {
            owner == FormId(0x7) || owner == FormId(0xDB1)
        } else {
            owner == named
        }
    }

    /// Whether `who` is the owner, or in the faction that is.
    fn is_owner_of(&self, who: FormId, owner: Option<FormId>) -> bool {
        let Some(owner) = owner else { return false };
        owner == who
            || self.base_of(who) == Some(owner)
            || (self.lo.tag_of(owner).is_some_and(|t| t.0 == *b"FACT")
                && self.is_member(who, owner))
    }

    /// How many items with the keyword a reference carries (its starting
    /// inventory until it has changed, as `item_count` reads it).
    pub fn keyword_item_count(&self, r: FormId, keyword: FormId) -> i32 {
        let held = self
            .inventories
            .get(&r)
            .cloned()
            .or_else(|| self.starting_inventory(r))
            .unwrap_or_default();
        held.items
            .iter()
            .filter(|&&(item, n)| n > 0 && self.has_keyword(item, keyword))
            .map(|&(_, n)| n)
            .sum()
    }

    /// Whether two actors are of the same race (their bases' `RNAM`).
    pub fn same_race(&self, a: FormId, b: FormId) -> bool {
        self.race_of(a)
            .zip(self.race_of(b))
            .is_some_and(|(x, y)| x == y)
    }

    /// Whether two actors are of the same sex (their bases' `ACBS`).
    pub fn same_sex(&self, a: FormId, b: FormId) -> bool {
        self.base_of(a)
            .zip(self.base_of(b))
            .is_some_and(|(x, y)| self.npc_is_female(x) == self.npc_is_female(y))
    }

    /// Factions and ranks of an actor (a reference, or an NPC record), from its
    /// templates when they give them.
    pub fn npc_factions(&self, actor: FormId) -> Vec<(FormId, i8)> {
        let mut out = self
            .templates_of(actor)
            .map(|t| t.factions(&self.lo))
            .unwrap_or_default();
        // Alias memberships are temporary overlays, never edits to the NPC
        // record or script ranks. Read current fills so clear/refill/stop and
        // save restoration cannot leave stale memberships behind.
        if let Some(reference) = self
            .faction_actor_ref(actor)
            .filter(|&r| self.is_actor_ref(r))
        {
            let mut factions = Vec::new();
            for (&quest, state) in self.scripts.quests.iter().filter(|(_, s)| s.running) {
                if !state.aliases.values().any(|&r| r == reference) {
                    continue;
                }
                for alias in self
                    .alias_specs(quest)
                    .iter()
                    .filter(|a| !a.location && state.aliases.get(&a.id) == Some(&reference))
                {
                    factions.extend_from_slice(&alias.factions);
                }
            }
            factions.sort_unstable();
            factions.dedup();
            for faction in factions {
                match out.iter_mut().find(|x| x.0 == faction) {
                    Some(x) => x.1 = x.1.max(0),
                    None => out.push((faction, 0)),
                }
            }
        }
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
        self.scripts
            .faction_changes
            .get(&self.faction_actor_ref(actor)?)
    }

    /// Use the same reference identity for script and alias faction overlays.
    fn faction_actor_ref(&self, actor: FormId) -> Option<FormId> {
        Some(match actor {
            FormId(0x7) => PLAYER_REF,
            a if self.lo.tag_of(a).is_some_and(|t| t.0 == *b"NPC_") => {
                self.npc_refs_index().get(&a).copied()?
            }
            a => a,
        })
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
        self.refresh_combat_factions(actor);
    }

    /// The factions an actor belongs to now (rank 0 or more): its record's and
    /// its templates', as scripts changed them.
    pub fn member_factions(&self, actor: FormId) -> Vec<FormId> {
        self.npc_factions(actor)
            .into_iter()
            .filter(|&(_, rank)| rank >= 0)
            .map(|(f, _)| f)
            .collect()
    }

    /// A loaded actor's fights go by the factions it belongs to now: a hired
    /// follower (`CurrentFollowerFaction`, an ally of the player's faction) helps
    /// the player's side as soon as a script puts it there.
    fn refresh_combat_factions(&mut self, actor: FormId) {
        let now = self.member_factions(actor);
        if let Some(a) = self.actor_mut(actor)
            && a.stats.factions != now
        {
            std::sync::Arc::make_mut(&mut a.stats).factions = now;
        }
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
        self.race_flag(race, 0x4)
    }

    /// Whether a race is playable (`DATA` flag 0x1).
    pub fn race_is_playable(&self, race: FormId) -> bool {
        self.race_flag(race, 0x1)
    }

    /// A race's `DATA` flag bits (xEdit's `wbRACE`: 0x1 playable, 0x4 child).
    fn race_flag(&self, race: FormId, mask: u32) -> bool {
        self.lo
            .get(race)
            .and_then(|r| {
                r.get(b"DATA").map(|d| {
                    d.len() >= 36 && u32::from_le_bytes(d[32..36].try_into().unwrap()) & mask != 0
                })
            })
            .unwrap_or(false)
    }

    /// An actor's base `ACBS` flag (essential or protected): as a script
    /// left it, else a loaded actor's (its templates' when they give the
    /// base data), else its base's.
    fn base_flag(&self, actor: FormId, essential: bool) -> bool {
        use crate::protection::acbs;
        let flag = if essential {
            acbs::ESSENTIAL
        } else {
            acbs::PROTECTED
        };
        if let Some(on) = self.changed_base_flag(actor, flag) {
            return on;
        }
        match self.actor_ref(actor) {
            Some(a) if essential => a.stats.essential,
            Some(a) => a.stats.protected,
            None => self
                .base_of(actor)
                .is_some_and(|b| self.base_has_flag(b, flag)),
        }
    }

    /// Essential (`IsEssential`; `ACBS` 0x2), or filling an essential alias
    /// of a running quest (`known_ambiguities/protection.md`).
    pub fn is_essential(&self, actor: FormId) -> bool {
        self.base_flag(actor, true) || self.in_alias_with(actor, crate::aliases::flags::ESSENTIAL)
    }

    /// Protected (`IsProtected`; `ACBS` 0x800), or filling a protected alias
    /// of a running quest.
    pub fn is_protected(&self, actor: FormId) -> bool {
        self.base_flag(actor, false) || self.in_alias_with(actor, crate::aliases::flags::PROTECTED)
    }

    /// Whether `attacker` can't kill `victim`: essential actors, and protected
    /// ones but to the player, bleed out instead.
    pub fn spared_by(&self, victim: FormId, attacker: FormId) -> bool {
        self.is_essential(victim) || (self.is_protected(victim) && attacker != PLAYER_REF)
    }

    /// Whether `helper` would come to `other`'s aid: it helps allies (assistance
    /// 1) and friends (2), and a teammate the player.
    pub fn should_help(&self, helper: FormId, other: FormId) -> bool {
        if other == PLAYER_REF && self.is_player_teammate(helper) {
            return true;
        }
        let Some(a) = self.actor_ref(helper) else {
            return false;
        };
        match self.faction_relation(helper, other) {
            2 => a.stats.assistance >= 1,
            3 => a.stats.assistance >= 2,
            _ => false,
        }
    }

    /// The player is inside the actor's attack aggro radius.
    pub fn aggro_radius_violated(&self, actor: FormId) -> bool {
        self.actor_ref(actor).is_some_and(|a| {
            !a.dead
                && a.stats
                    .aggro_attack
                    .is_some_and(|r| a.pos.distance(self.player.position) < r)
        })
    }

    /// A reference's scale: a loaded actor's, else a placed reference's.
    pub fn ref_scale(&self, r: FormId) -> f32 {
        if let Some(a) = self.actor_ref(r) {
            return a.scale;
        }
        self.lo
            .get(r)
            .map_or(1.0, |rec| crate::world::records::reference(&rec).scale)
    }

    /// How an actor's factions regard another's in combat: the strongest of the
    /// members' reactions (3 friend, 2 ally, 1 enemy, 0 neutral).
    pub fn faction_relation(&self, a: FormId, b: FormId) -> u32 {
        let members = |x| -> Vec<FormId> {
            self.npc_factions(x)
                .into_iter()
                .filter(|&(_, r)| r >= 0)
                .map(|(f, _)| f)
                .collect()
        };
        let theirs = members(b);
        members(a)
            .into_iter()
            .flat_map(|f| theirs.iter().map(move |&g| (f, g)))
            .map(|(f, g)| self.faction_combat_reaction(f, g))
            .max()
            .unwrap_or(0)
    }

    /// Whether an object can be picked up: a carried kind of item (lights
    /// only with the "can be carried" flag).
    pub fn is_carryable(&self, base: FormId) -> bool {
        let Some(tag) = self.lo.tag_of(base) else {
            return false;
        };
        match &tag.0 {
            b"ALCH" | b"AMMO" | b"APPA" | b"ARMO" | b"BOOK" | b"INGR" | b"KEYM" | b"MISC"
            | b"NOTE" | b"SCRL" | b"SLGM" | b"WEAP" => true,
            b"LIGH" => self
                .lo
                .get(base)
                .and_then(|r| {
                    r.get(b"DATA")
                        .filter(|d| d.len() >= 16)
                        .map(|d| d[12] & 0x2 != 0)
                })
                .unwrap_or(false),
            _ => false,
        }
    }

    /// The summed base value of the clothing (not armor) an actor wears.
    pub fn clothing_value(&self, actor: FormId) -> i32 {
        let Some(inv) = self.inventories.get(&actor) else {
            return 0;
        };
        inv.equipped()
            .iter()
            .filter_map(|&f| self.lo.get(f))
            .filter(|r| r.tag().0 == *b"ARMO")
            // `BOD2`: the slots, then the armor type (0 light, 1 heavy, 2 clothing).
            .filter(|r| {
                r.get(b"BOD2").is_some_and(|d| {
                    d.len() >= 8 && u32::from_le_bytes(d[4..8].try_into().unwrap()) == 2
                })
            })
            .filter_map(|r| {
                r.get(b"DATA")
                    .filter(|d| d.len() >= 4)
                    .map(|d| i32::from_le_bytes(d[0..4].try_into().unwrap()))
            })
            .sum()
    }

    /// The cell the player is in (an exterior's: the loaded grid's centre).
    pub fn player_cell(&self) -> Option<FormId> {
        match self.location {
            crate::engine::Location::Interior(c) => Some(c),
            crate::engine::Location::Exterior { world, center } => self
                .lo
                .world(world)
                .and_then(|w| w.cells.get(&center).copied()),
            _ => None,
        }
    }

    /// The cell a reference is in: the player's, a loaded actor's, else a
    /// placed reference's.
    pub fn ref_cell(&self, r: FormId) -> Option<FormId> {
        if r == PLAYER_REF {
            return self.player_cell();
        }
        match (self.actor_cells.get(&r), self.location) {
            (Some(&crate::render::CellKey::Interior(c)), _) => Some(c),
            (
                Some(&crate::render::CellKey::Exterior(x, y)),
                crate::engine::Location::Exterior { world, .. },
            ) => self
                .lo
                .world(world)
                .and_then(|w| w.cells.get(&(x, y)).copied()),
            _ => self.lo.cell_of_ref(r),
        }
    }

    /// A reference's encounter zone: its own (`XEZN`), else its cell's.
    pub fn encounter_zone_of(&self, r: FormId) -> Option<FormId> {
        let zone = |r: FormId| {
            let rec = self.lo.get(r)?;
            let d = rec.get(b"XEZN").filter(|d| d.len() >= 4)?;
            Some(rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap()))))
        };
        zone(r).or_else(|| zone(self.ref_cell(r)?))
    }

    /// Location (LCTN) of the current cell.
    pub fn current_location(&self) -> Option<FormId> {
        let cell = self.player_cell()?;
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

#[cfg(test)]
#[path = "status_tests.rs"]
mod status_tests;

#[cfg(test)]
#[path = "geometry_tests.rs"]
mod geometry_tests;

#[cfg(test)]
#[path = "reference_tests.rs"]
mod reference_tests;

#[cfg(test)]
#[path = "variable_tests.rs"]
mod variable_tests;

#[cfg(test)]
#[path = "compare_tests.rs"]
mod compare_tests;

#[cfg(test)]
#[path = "crime_tests.rs"]
mod crime_tests;

#[cfg(test)]
#[path = "death_tests.rs"]
mod death_tests;
