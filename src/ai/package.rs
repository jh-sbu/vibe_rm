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
}

fn behaviour_of(template: &str) -> Behaviour {
    let t = template.to_ascii_lowercase();
    if t.starts_with("travel") || t.starts_with("sleep") || t.starts_with("hold") || t == "patrol" {
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

pub fn parse(lo: &LoadOrder, id: FormId) -> Option<Package> {
    let rec = lo.get(id)?;
    if rec.tag().0 != *b"PACK" {
        return None;
    }
    let editor_id = rec.editor_id().unwrap_or_default();
    let mut schedule = Schedule { month: -1, day_of_week: -1, hour: -1, minute: -1, duration: 0 };
    let mut template = FormId::NULL;
    let mut location = None;
    let mut last_input = String::new();
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
            b"ANAM" => last_input = sr.zstring(),
            // The first "Location" input is the package's main location.
            b"PLDT" if location.is_none() && last_input == "Location" && sr.data.len() >= 12 => {
                let v = sr.u32(4);
                let f = rec.fid(FormId(v));
                let kind = match sr.u32(0) {
                    0 => LocationKind::NearReference(f),
                    1 => LocationKind::InCell(f),
                    2 => LocationKind::NearCurrent,
                    3 => LocationKind::NearEditor,
                    6 => LocationKind::NearLinkedRef(if v == 0 { FormId::NULL } else { f }),
                    12 => LocationKind::NearSelf,
                    k => LocationKind::Other(k),
                };
                location = Some(Location { kind, radius: sr.i32(8).max(0) as f32 });
            }
            _ => {}
        }
    }
    let template_name = lo.get(template).and_then(|t| t.editor_id()).unwrap_or_default();
    Some(Package {
        id,
        editor_id,
        behaviour: behaviour_of(&template_name),
        template: template_name,
        schedule,
        conditions: condition::parse_all(&rec),
        location,
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
