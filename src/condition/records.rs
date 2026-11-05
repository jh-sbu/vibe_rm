//! Condition records and evaluation that needs only package data. Shared with tools.

use super::functions;
use esp::{FormId, LoadedRecord};

#[derive(Debug, Clone, Default)]
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
pub(super) const RUN_ON_EVENT_DATA: u32 = 7;
pub(super) const GET_EVENT_DATA: u16 = 576;
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
pub(super) fn combine(conds: &[Condition], mut eval: impl FnMut(&Condition) -> bool) -> bool {
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

pub(super) fn compare(op: u8, lhs: f32, rhs: f32) -> bool {
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
