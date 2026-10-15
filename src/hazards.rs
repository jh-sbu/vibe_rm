//! Placed hazards (`PHZD` references of `HAZD` records: campfires' and
//! braziers' `FireLgPlacedHazard`, fire plates, the Throat of the World's
//! blizzard). Every target interval (`DATA` 0x10) a hazard gives its spell
//! (0x18) to the player and the living actors within its radius (0x04, in
//! feet as spells' areas are), only the player with "Affects Player Only"
//! (flag 0x1). The spell's effects land as a hazard's, quietly: no fight
//! with anyone, and each touch replaces the last, so a burn lasts while
//! they stay and its duration after. Record layout: UESP's HAZD page and
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

/// A placed hazard in a loaded cell.
pub struct Hazard {
    pub ref_id: FormId,
    pos: Vec3,
    /// Reach in game units.
    radius: f32,
    interval: f32,
    spell: FormId,
    player_only: bool,
    /// Seconds to the next touch.
    next: f32,
}

/// The hazard a `PHZD` reference places, if its base gives a spell.
pub fn hazard_of(lo: &esp::LoadOrder, r: FormId) -> Option<Hazard> {
    let rec = lo.get(r).filter(|rec| rec.tag().0 == *b"PHZD")?;
    let rf = crate::world::records::reference(&rec);
    if rf.deleted() {
        return None;
    }
    let base = lo.get(rf.base).filter(|b| b.tag().0 == *b"HAZD")?;
    let d = base.get(b"DATA").filter(|d| d.len() >= 0x1C)?;
    let f32_at = |o: usize| f32::from_le_bytes(d[o..o + 4].try_into().unwrap());
    let u32_at = |o: usize| u32::from_le_bytes(d[o..o + 4].try_into().unwrap());
    let spell = base.fid(FormId(u32_at(0x18)));
    if spell.is_null() {
        return None;
    }
    let interval = f32_at(0x10).max(0.05);
    Some(Hazard {
        ref_id: r,
        pos: rf.position,
        radius: f32_at(0x04).max(0.0) * UNITS_PER_FOOT * rf.scale,
        interval,
        spell,
        player_only: u32_at(0x14) & PLAYER_ONLY != 0,
        next: 0.0,
    })
}

impl Engine {
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
            .map(|a| (a.ref_id, a.pos))
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
                h.next -= dt;
                if h.next > 0.0 {
                    continue;
                }
                h.next += h.interval;
                if h.next <= 0.0 {
                    h.next = h.interval;
                }
                for &(who, feet) in &bodies {
                    if h.player_only && who != PLAYER_REF {
                        continue;
                    }
                    let near = BODY_POINTS
                        .iter()
                        .any(|z| (feet + Vec3::Z * z).distance(h.pos) <= h.radius);
                    if near {
                        touches.push((r, h.spell, who));
                    }
                }
            }
        }
        for (hazard, spell, who) in touches {
            log::debug!("hazard {hazard} touches {who} with {spell}");
            self.apply_item_quietly(spell, hazard, who);
        }
    }
}
