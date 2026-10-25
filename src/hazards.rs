//! Placed hazards (`PHZD` references of `HAZD` records: campfires' and
//! braziers' `FireLgPlacedHazard`, fire plates, the Throat of the World's
//! blizzard). Every target interval (`DATA` 0x10) a hazard gives its spell
//! (0x18) to the player and the living actors within its radius (0x04, in
//! feet as spells' areas are), only the player with "Affects Player Only"
//! (flag 0x1). The spell's effects land as a hazard's, quietly: no fight
//! with anyone, and each touch replaces the last, so a burn lasts while
//! they stay and its duration after. Impacts leave them too (`NAM2`), for
//! the record's lifetime. Record layout: UESP's HAZD page and
//! CommonLibSSE's `BGSHazardData`. `known_ambiguities/triggers.md`.

use esp::FormId;
use glam::Vec3;

use crate::casting::UNITS_PER_FOOT;
use crate::engine::{Engine, PLAYER_REF};
use crate::render::CellKey;

/// `HAZD` `DATA` flag: affects the player only.
const PLAYER_ONLY: u32 = 0x1;
/// Heights above the feet tested against a hazard's reach (as triggers').
const BODY_POINTS: [f32; 3] = [10.0, 60.0, 110.0];
/// How long a hazard an impact left lasts when its record gives no lifetime.
const DEFAULT_LIFETIME: f32 = 10.0;

/// A hazard in the world: placed in a loaded cell, or left by an impact.
pub struct Hazard {
    /// The reference (the base for one an impact left).
    pub ref_id: FormId,
    /// The hazard record.
    base: FormId,
    pos: Vec3,
    /// Reach in game units.
    radius: f32,
    interval: f32,
    spell: FormId,
    player_only: bool,
    /// Seconds to the next touch.
    next: f32,
    /// Seconds left (those an impact left; placed ones last).
    life: Option<f32>,
    /// How many of its record may exist at once (`DATA` limit).
    limit: u32,
}

/// The hazard a `PHZD` reference places, if its base gives a spell.
pub fn hazard_of(lo: &esp::LoadOrder, r: FormId) -> Option<Hazard> {
    let rec = lo.get(r).filter(|rec| rec.tag().0 == *b"PHZD")?;
    let rf = crate::world::records::reference(&rec);
    if rf.deleted() {
        return None;
    }
    hazard_at(lo, rf.base, r, rf.position, rf.scale)
}

/// A hazard of a `HAZD` record at a place, if it gives a spell. Its
/// `lifetime` (`DATA` 0x08) is how long one an impact left lasts.
fn hazard_at(
    lo: &esp::LoadOrder,
    base_id: FormId,
    ref_id: FormId,
    pos: Vec3,
    scale: f32,
) -> Option<Hazard> {
    let base = lo.get(base_id).filter(|b| b.tag().0 == *b"HAZD")?;
    let d = base.get(b"DATA").filter(|d| d.len() >= 0x1C)?;
    let f32_at = |o: usize| f32::from_le_bytes(d[o..o + 4].try_into().unwrap());
    let u32_at = |o: usize| u32::from_le_bytes(d[o..o + 4].try_into().unwrap());
    let spell = base.fid(FormId(u32_at(0x18)));
    if spell.is_null() {
        return None;
    }
    let interval = f32_at(0x10).max(0.05);
    Some(Hazard {
        ref_id,
        base: base_id,
        pos,
        radius: f32_at(0x04).max(0.0) * UNITS_PER_FOOT * scale,
        interval,
        spell,
        player_only: u32_at(0x14) & PLAYER_ONLY != 0,
        next: 0.0,
        life: None,
        limit: u32_at(0x00),
    })
}

/// The lifetime of a hazard record (`DATA` 0x08), seconds.
fn lifetime_of(lo: &esp::LoadOrder, base: FormId) -> Option<f32> {
    let rec = lo.get(base)?;
    let d = rec.get(b"DATA").filter(|d| d.len() >= 0x0C)?;
    Some(f32::from_le_bytes(d[8..12].try_into().unwrap()))
}

impl Hazard {
    /// Counts down to its next touch, and whether it is due now.
    fn due(&mut self, dt: f32) -> bool {
        self.next -= dt;
        if self.next > 0.0 {
            return false;
        }
        self.next += self.interval;
        if self.next <= 0.0 {
            self.next = self.interval;
        }
        true
    }

    /// Who among the bodies it reaches (the player only, if it says so).
    fn reaches(
        &self,
        bodies: &[(FormId, Vec3)],
        r: FormId,
        touches: &mut Vec<(FormId, FormId, FormId)>,
    ) {
        for &(who, feet) in bodies {
            if self.player_only && who != PLAYER_REF {
                continue;
            }
            let near = BODY_POINTS
                .iter()
                .any(|z| (feet + Vec3::Z * z).distance(self.pos) <= self.radius);
            if near {
                touches.push((r, self.spell, who));
            }
        }
    }
}

impl Engine {
    /// An impact leaves a hazard where it lands (`NAM2`): it lasts its
    /// record's lifetime, and at its limit the oldest of its kind goes.
    pub(crate) fn spawn_hazard(&mut self, base: FormId, at: Vec3) {
        let Some(mut h) = hazard_at(&self.lo, base, base, at, 1.0) else {
            return;
        };
        let life = lifetime_of(&self.lo, base).filter(|&l| l > 0.0);
        h.life = Some(life.unwrap_or(DEFAULT_LIFETIME));
        let hazards = &mut self.impacts.hazards;
        if h.limit > 0 {
            while hazards.iter().filter(|x| x.base == base).count() >= h.limit as usize {
                let Some(oldest) = hazards.iter().position(|x| x.base == base) else {
                    break;
                };
                hazards.remove(oldest);
            }
        }
        log::debug!("hazard {base} left at {at:.0}");
        hazards.push(h);
    }

    /// Set up the placed hazards among a loaded cell's references.
    pub(crate) fn load_hazards(&mut self, key: CellKey, refs: &[FormId]) {
        let hazards: Vec<Hazard> = refs
            .iter()
            .filter_map(|&r| hazard_of(&self.lo, r))
            .collect();
        if !hazards.is_empty() {
            log::debug!("{key:?}: {} placed hazards", hazards.len());
        }
        if let Some(rt) = self.cells.get_mut(&key) {
            rt.hazards = hazards;
        }
    }

    /// Hazards touch who is within reach on their intervals (every frame).
    pub(crate) fn update_hazards(&mut self, dt: f32) {
        let mut bodies: Vec<(FormId, Vec3)> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| !a.dead)
            .map(|a| (a.ref_id, a.exec.pos))
            .collect();
        if !self.player_dead()
            && let Some(p) = self.ref_position(PLAYER_REF)
        {
            bodies.push((PLAYER_REF, p));
        }
        let mut touches: Vec<(FormId, FormId, FormId)> = Vec::new();
        let keys: Vec<CellKey> = self.cells.keys().copied().collect();
        for key in keys {
            let n = self.cells[&key].hazards.len();
            for i in 0..n {
                let r = self.cells[&key].hazards[i].ref_id;
                if self.is_disabled(r) {
                    continue;
                }
                let h = &mut self.cells.get_mut(&key).unwrap().hazards[i];
                if h.due(dt) {
                    h.reaches(&bodies, r, &mut touches);
                }
            }
        }
        // Those impacts left run out their lifetimes.
        self.impacts.hazards.retain_mut(|h| {
            h.life = h.life.map(|l| l - dt);
            h.life.is_none_or(|l| l > 0.0)
        });
        for h in &mut self.impacts.hazards {
            if h.due(dt) {
                h.reaches(&bodies, h.ref_id, &mut touches);
            }
        }
        for (hazard, spell, who) in touches {
            log::debug!("hazard {hazard} touches {who} with {spell}");
            self.apply_item_quietly(spell, hazard, who);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esp::writer::{Node, RecordBuilder, write_plugin};

    #[test]
    fn a_hazard_an_impact_leaves_has_its_records_reach_lifetime_and_limit() {
        let dir = std::env::temp_dir().join(format!("vrm-impact-hazard-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // Limit 2, radius 3 feet, lifetime 5 s, image space radius 0, interval
        // 0.5 s, flags 0, the spell, light, impact data set and sound.
        let mut data = Vec::new();
        data.extend_from_slice(&2u32.to_le_bytes());
        for f in [3.0f32, 5.0, 0.0, 0.5] {
            data.extend_from_slice(&f.to_le_bytes());
        }
        for w in [0u32, 0xA00, 0, 0, 0] {
            data.extend_from_slice(&w.to_le_bytes());
        }
        let mut impact_data = [0u8; 24];
        impact_data[20] = 1;
        let records = [
            Node::top(
                b"HAZD",
                vec![RecordBuilder::new(b"HAZD", FormId(0x800)).field(b"DATA", &data)],
            ),
            Node::top(
                b"IPCT",
                vec![
                    RecordBuilder::new(b"IPCT", FormId(0x810))
                        .field(b"DATA", &impact_data)
                        .form(b"NAM2", FormId(0x800)),
                    RecordBuilder::new(b"IPCT", FormId(0x811)).field(b"DATA", &impact_data),
                ],
            ),
        ];
        std::fs::write(
            dir.join("Hazard.esp"),
            write_plugin(0, &[], "test", "", 0x900, &records),
        )
        .unwrap();
        let lo = esp::LoadOrder::load(&dir, &["Hazard.esp".into()]).unwrap();
        let _ = std::fs::remove_dir_all(&dir);

        let impact = crate::impacts::Impact::load(&lo, FormId(0x810)).unwrap();
        assert_eq!(impact.hazard, Some(FormId(0x800)));
        let plain = crate::impacts::Impact::load(&lo, FormId(0x811)).unwrap();
        assert_eq!(plain.hazard, None);

        let mut h = hazard_at(&lo, FormId(0x800), FormId(0x800), Vec3::ZERO, 1.0).unwrap();
        assert_eq!(h.limit, 2);
        assert_eq!(h.radius, 3.0 * UNITS_PER_FOOT);
        assert_eq!(lifetime_of(&lo, FormId(0x800)), Some(5.0));
        // Touching at once, then every half second.
        assert!(h.due(0.016));
        assert!(!h.due(0.4));
        assert!(h.due(0.2));
        // Only the bodies within its reach, at any of their body points.
        let bodies = [
            (FormId(0x14), Vec3::new(10.0, 0.0, -100.0)),
            (FormId(0x900), Vec3::new(500.0, 0.0, 0.0)),
        ];
        let mut touches = Vec::new();
        h.reaches(&bodies, FormId(0x800), &mut touches);
        assert_eq!(touches, vec![(FormId(0x800), FormId(0xA00), FormId(0x14))]);
    }
}
