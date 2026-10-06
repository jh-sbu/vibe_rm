//! AI packages (`PACK`): schedule, conditions, procedure template and target location.

use esp::{FormId, LoadOrder};

use crate::condition::{self, Condition};

/// What an actor does while a package runs. Derived from the package's template,
/// which in the Creation Kit names a procedure tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Behaviour {
    /// Wander between random spots near the location, idling at each.
    Sandbox,
    /// Walk to the location and stay there.
    Travel,
    /// Stay put.
    Hold,
    /// Go to the location and sleep in a bed there.
    Sleep,
    /// Sit in the target chair (or one near the location).
    Sit,
    /// Walk a chain of linked patrol markers from the target reference.
    Patrol,
    /// Stay near the target reference.
    Follow,
}

/// What a sandboxing actor may do besides wander (the template's "Allow ..." inputs).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Allow {
    pub sitting: bool,
    pub sleeping: bool,
    pub eating: bool,
    pub idle_markers: bool,
    pub special_furniture: bool,
    pub wandering: bool,
    /// Eating is the point (the Eat template): every seated idle is a meal, not
    /// just the occasional one.
    pub meal: bool,
}

impl Allow {
    const NONE: Allow = Allow {
        sitting: false,
        sleeping: false,
        eating: false,
        idle_markers: false,
        special_furniture: false,
        wandering: true,
        meal: false,
    };
}

/// A package's preferred speed (`PKDT`, when its "Preferred Speed" flag is set).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Gait {
    #[default]
    Walk,
    Jog,
    Run,
    FastWalk,
}

/// `PKDT` general flags.
const PKDT_PREFERRED_SPEED: u32 = 1 << 13;
const PKDT_ALWAYS_SNEAK: u32 = 1 << 17;
const PKDT_UNLOCK_AT_START: u32 = 0x40;
const PKDT_UNLOCK_ON_CHANGE: u32 = 0x80;

/// A package "TargetSelector" / "SingleRef" input (`PTDA`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Ref(FormId),
    LinkedRef(Option<FormId>),
    /// Object ids / types, aliases, etc. (not resolved yet).
    Other,
}

#[derive(Debug, Clone, Copy)]
pub enum LocationKind {
    NearReference(FormId),
    InCell(FormId),
    NearCurrent,
    NearEditor,
    NearLinkedRef(FormId),
    NearSelf,
    /// Object ids / types, aliases, etc. Approximated by the editor location.
    Other(u32),
}

#[derive(Debug, Clone, Copy)]
pub struct Location {
    pub kind: LocationKind,
    pub radius: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Schedule {
    pub month: i8,
    pub day_of_week: i8,
    pub hour: i8,
    pub minute: i8,
    /// Minutes.
    pub duration: i32,
}

impl Schedule {
    pub fn matches(&self, hour: f32, day: u32) -> bool {
        if self.day_of_week >= 0 && (day % 7) as i8 != self.day_of_week {
            return false;
        }
        if self.hour < 0 {
            return true;
        }
        let start = self.hour as f32 + self.minute.max(0) as f32 / 60.0;
        let len = self.duration as f32 / 60.0;
        if len <= 0.0 {
            return true;
        }
        let since = (hour - start).rem_euclid(24.0);
        since < len
    }
}

#[derive(Debug, Clone)]
pub struct Package {
    pub id: FormId,
    pub editor_id: String,
    pub template: String,
    pub behaviour: Behaviour,
    pub schedule: Schedule,
    pub conditions: Vec<Condition>,
    pub location: Option<Location>,
    pub target: Option<Target>,
    pub allow: Allow,
    /// 0..100: how restless a sandboxing actor is.
    pub energy: f32,
    /// Patrol: how close to each point counts as reached; loop at the end; start
    /// at the nearest point rather than the first.
    pub point_radius: f32,
    pub repeat: bool,
    pub start_nearest: bool,
    /// Follow: keep between these distances from the target.
    pub follow_radius: (f32, f32),
    pub gait: Gait,
    pub sneak: bool,
    /// The sleeper locks its home's doors ("Lock Doors?" input).
    pub lock_doors: bool,
    /// `PKDT`: unlock the home's doors as the package starts / once it gives way
    /// to another.
    pub unlock_at_start: bool,
    pub unlock_on_change: bool,
}

fn behaviour_of(template: &str) -> Behaviour {
    let t = template.to_ascii_lowercase();
    if t.starts_with("sleep") {
        Behaviour::Sleep
    } else if t == "sit" || t == "sittarget" {
        Behaviour::Sit
    } else if t.contains("patrol") {
        Behaviour::Patrol
    } else if t.starts_with("follow") || t.starts_with("escort") {
        Behaviour::Follow
    } else if t.starts_with("travel") || t.starts_with("hold") || t == "patrol" {
        Behaviour::Travel
    } else if t.starts_with("sandbox")
        || t.starts_with("eat")
        || t.starts_with("sit")
        || t.starts_with("useidlemarker")
        || t.starts_with("wander")
        || t.starts_with("guard")
        || t.starts_with("find")
    {
        Behaviour::Sandbox
    } else {
        Behaviour::Hold
    }
}

/// A package data input value, in record order.
#[derive(Debug, Clone, Copy)]
enum Input {
    Bool(bool),
    Float(f32),
    Location(Location),
    Target(Target),
    Other,
}

fn location(rec: &esp::LoadedRecord<'_>, d: &[u8]) -> Option<Location> {
    if d.len() < 12 {
        return None;
    }
    let v = u32::from_le_bytes(d[4..8].try_into().unwrap());
    let f = rec.fid(FormId(v));
    let kind = match u32::from_le_bytes(d[0..4].try_into().unwrap()) {
        0 => LocationKind::NearReference(f),
        1 => LocationKind::InCell(f),
        2 => LocationKind::NearCurrent,
        3 => LocationKind::NearEditor,
        6 => LocationKind::NearLinkedRef(if v == 0 { FormId::NULL } else { f }),
        12 => LocationKind::NearSelf,
        k => LocationKind::Other(k),
    };
    Some(Location { kind, radius: i32::from_le_bytes(d[8..12].try_into().unwrap()).max(0) as f32 })
}

fn target(rec: &esp::LoadedRecord<'_>, d: &[u8]) -> Option<Target> {
    if d.len() < 8 {
        return None;
    }
    let v = u32::from_le_bytes(d[4..8].try_into().unwrap());
    Some(match u32::from_le_bytes(d[0..4].try_into().unwrap()) {
        0 => Target::Ref(rec.fid(FormId(v))),
        3 => Target::LinkedRef((v != 0).then(|| rec.fid(FormId(v)))),
        _ => Target::Other,
    })
}

/// Data inputs of a package with their indices (`UNAM`), in record order.
fn inputs(rec: &esp::LoadedRecord<'_>) -> Vec<(u8, Input)> {
    let mut values = Vec::new();
    let mut indices = Vec::new();
    let mut kind = String::new();
    for sr in rec.subrecords() {
        match &sr.tag.0 {
            // The procedure tree follows; its UNAMs are names, not input indices.
            b"XNAM" => break,
            b"ANAM" => {
                kind = sr.zstring();
                values.push(Input::Other);
            }
            b"CNAM" if kind == "Bool" && !sr.data.is_empty() => *values.last_mut().unwrap() = Input::Bool(sr.data[0] != 0),
            b"CNAM" if kind == "Float" && sr.data.len() >= 4 => *values.last_mut().unwrap() = Input::Float(sr.f32(0)),
            b"PLDT" if !values.is_empty() => {
                if let Some(l) = location(rec, sr.data) {
                    *values.last_mut().unwrap() = Input::Location(l);
                }
            }
            b"PTDA" if !values.is_empty() => {
                if let Some(t) = target(rec, sr.data) {
                    *values.last_mut().unwrap() = Input::Target(t);
                }
            }
            b"UNAM" if !sr.data.is_empty() => indices.push(sr.data[0]),
            _ => {}
        }
    }
    indices.into_iter().zip(values).collect()
}

/// Input names of a template package, by index (`UNAM` + `BNAM` after the procedure tree).
fn input_names(lo: &LoadOrder, template: FormId) -> std::collections::HashMap<u8, String> {
    let mut out = std::collections::HashMap::new();
    let Some(rec) = lo.get(template) else { return out };
    let mut tree = false;
    let mut index = None;
    for sr in rec.subrecords() {
        match &sr.tag.0 {
            b"XNAM" => tree = true,
            b"UNAM" if tree && !sr.data.is_empty() => index = Some(sr.data[0]),
            b"BNAM" if tree => {
                if let Some(i) = index.take() {
                    // "Allow Sitting*", "AllowSitting", "Energy*" -> "allowsitting", "energy".
                    let name: String = sr.zstring().chars().filter(char::is_ascii_alphanumeric).collect();
                    out.insert(i, name.to_ascii_lowercase());
                }
            }
            _ => {}
        }
    }
    out
}

pub fn parse(lo: &LoadOrder, id: FormId) -> Option<Package> {
    let rec = lo.get(id)?;
    if rec.tag().0 != *b"PACK" {
        return None;
    }
    let editor_id = rec.editor_id().unwrap_or_default();
    let mut schedule = Schedule { month: -1, day_of_week: -1, hour: -1, minute: -1, duration: 0 };
    let mut template = FormId::NULL;
    let (mut gait, mut sneak) = (Gait::Walk, false);
    let mut pkdt_flags = 0;
    for sr in rec.subrecords() {
        match &sr.tag.0 {
            b"PSDT" if sr.data.len() >= 12 => {
                schedule = Schedule {
                    month: sr.data[0] as i8,
                    day_of_week: sr.data[1] as i8,
                    hour: sr.data[3] as i8,
                    minute: sr.data[4] as i8,
                    duration: sr.i32(8),
                };
            }
            b"PKCU" if sr.data.len() >= 8 => template = rec.fid(sr.form_id(4)),
            // General flags, type, interrupt override, preferred speed.
            b"PKDT" if sr.data.len() >= 7 => {
                let flags = u32::from_le_bytes(sr.data[0..4].try_into().unwrap());
                if flags & PKDT_PREFERRED_SPEED != 0 {
                    gait = match sr.data[6] {
                        1 => Gait::Jog,
                        2 => Gait::Run,
                        3 => Gait::FastWalk,
                        _ => Gait::Walk,
                    };
                }
                sneak = flags & PKDT_ALWAYS_SNEAK != 0;
                pkdt_flags = flags;
            }
            _ => {}
        }
    }
    let template_name = lo.get(template).and_then(|t| t.editor_id()).unwrap_or_default();
    let behaviour = behaviour_of(&template_name);
    let names = input_names(lo, template);
    let inputs = inputs(&rec);
    let named = |n: &str| inputs.iter().find(|(i, _)| names.get(i).is_some_and(|x| x == n)).map(|(_, v)| *v);
    let flag = |n: &str| match named(n) {
        Some(Input::Bool(b)) => Some(b),
        _ => None,
    };
    let mut allow = Allow::NONE;
    let fields: [(&mut bool, &[&str]); 6] = [
        (&mut allow.sitting, &["allowsitting"]),
        (&mut allow.sleeping, &["allowsleeping"]),
        (&mut allow.eating, &["alloweating"]),
        (&mut allow.idle_markers, &["allowidlemarkers"]),
        (&mut allow.special_furniture, &["allowspecialfurniture", "allowfurniture"]),
        (&mut allow.wandering, &["allowwandering"]),
    ];
    for (field, keys) in fields {
        if let Some(b) = keys.iter().find_map(|k| flag(k)) {
            *field = b;
        }
    }
    // Eating means sitting down at a table.
    if template_name.eq_ignore_ascii_case("eat") {
        allow.sitting = true;
        allow.eating = true;
        allow.meal = true;
    }
    let energy = match named("energy") {
        Some(Input::Float(e)) => e.clamp(0.0, 100.0),
        _ => 50.0,
    };
    let float = |keys: &[&str], default: f32| {
        keys.iter()
            .find_map(|k| match named(k) {
                Some(Input::Float(f)) => Some(f),
                _ => None,
            })
            .unwrap_or(default)
    };
    let point_radius = float(&["patrolradius", "pointradius"], 50.0);
    let repeat = ["repeatable"].iter().find_map(|k| flag(k)).unwrap_or(true);
    let start_nearest = ["startatnearest", "startatnearestpoint"].iter().find_map(|k| flag(k)).unwrap_or(false);
    let follow_radius = (float(&["minradius"], 128.0), float(&["maxradius"], 384.0));
    // The first location input is the package's main location; likewise for targets.
    let location = inputs.iter().find_map(|(_, v)| match v {
        Input::Location(l) => Some(*l),
        _ => None,
    });
    let target = inputs.iter().find_map(|(_, v)| match v {
        Input::Target(t) => Some(*t),
        _ => None,
    });
    Some(Package {
        id,
        editor_id,
        behaviour,
        template: template_name,
        schedule,
        conditions: condition::parse_all(&rec),
        location,
        target,
        allow,
        energy,
        point_radius,
        repeat,
        start_nearest,
        follow_radius,
        gait,
        sneak,
        lock_doors: flag("lockdoors").unwrap_or(false),
        unlock_at_start: pkdt_flags & PKDT_UNLOCK_AT_START != 0,
        unlock_on_change: pkdt_flags & PKDT_UNLOCK_ON_CHANGE != 0,
    })
}

/// Packages of an NPC in priority order (following the AI-packages template).
pub fn npc_packages(lo: &LoadOrder, mut npc: FormId) -> Vec<Package> {
    const TPL_AI_PACKAGES: u16 = 0x20;
    for _ in 0..8 {
        let Some(rec) = lo.get(npc) else { break };
        let acbs = rec.get(b"ACBS").unwrap_or(&[]);
        let tpl_flags = if acbs.len() >= 20 { u16::from_le_bytes([acbs[18], acbs[19]]) } else { 0 };
        let template = rec.get(b"TPLT").filter(|d| d.len() >= 4).map(|d| rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap()))));
        if tpl_flags & TPL_AI_PACKAGES != 0
            && let Some(t) = template.filter(|t| !t.is_null())
            && lo.get(t).is_some_and(|r| r.tag().0 == *b"NPC_")
        {
            npc = t;
            continue;
        }
        return rec
            .subrecords()
            .filter(|s| s.tag.0 == *b"PKID")
            .filter_map(|s| parse(lo, rec.fid(s.form_id(0))))
            .collect();
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_wraps_midnight() {
        let s = Schedule { month: -1, day_of_week: -1, hour: 22, minute: -1, duration: 8 * 60 };
        assert!(s.matches(23.0, 0));
        assert!(s.matches(3.0, 0));
        assert!(!s.matches(7.0, 0));
        assert!(!s.matches(12.0, 0));
    }
}
