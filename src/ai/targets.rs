//! Package Object ID / Object Type matching and spatial reference selection.
use std::collections::BTreeSet;

use esp::{FormId, LoadOrder};
use glam::Vec3;

use super::{
    furniture::Use,
    package::{Behaviour, Package, Target},
    schedule::{Place, same_space},
};
use crate::engine::Engine;

/// Object-type numbers are the public xEdit enum, not condition FormType IDs.
/// This predicate is also used when searching inventory rather than the world.
pub(crate) fn matches_base(lo: &LoadOrder, base: FormId, target: Target) -> bool {
    let Some(r) = lo.get(base) else { return false };
    let tag = r.tag().0;
    match target {
        Target::Object(id) => base == id,
        Target::ObjectType(kind) => match kind {
            1 => tag == *b"ACTI",
            2 | 17 => tag == *b"ARMO",
            3 => tag == *b"BOOK",
            4 => tag == *b"CONT",
            5 => tag == *b"DOOR",
            6 => tag == *b"INGR",
            7 => tag == *b"LIGH",
            8 => tag == *b"MISC",
            9 => tag == *b"FLOR",
            10 => tag == *b"FURN",
            11 => tag == *b"WEAP",
            12 => tag == *b"AMMO",
            13 => tag == *b"KEYM",
            14 => tag == *b"ALCH",
            15 => {
                tag == *b"ALCH"
                    && r.get(b"ENIT")
                        .and_then(|d| d.get(4..8))
                        .is_some_and(|d| u32::from_le_bytes(d.try_into().unwrap()) & 2 != 0)
            }
            16 => {
                tag == *b"WEAP"
                    || tag == *b"AMMO"
                    || (tag == *b"ARMO"
                        && r.get(b"BOD2")
                            .and_then(|d| d.get(4..8))
                            .is_some_and(|d| u32::from_le_bytes(d.try_into().unwrap()) < 2))
            }
            19 | 20 => {
                tag == *b"WEAP"
                    && r.get(b"DNAM").and_then(|d| d.first()).is_some_and(|a| {
                        if kind == 19 {
                            (1..=6).contains(a)
                        } else {
                            matches!(a, 7 | 9)
                        }
                    })
            }
            21 => tag == *b"SPEL",
            22..=24 => {
                tag == *b"SPEL"
                    && r.get(b"SPIT").and_then(|d| d.get(20..24)).is_some_and(|d| {
                        let delivery = u32::from_le_bytes(d.try_into().unwrap());
                        match kind {
                            22 => (2..=4).contains(&delivery),
                            23 => delivery == 1,
                            _ => delivery == 0,
                        }
                    })
            }
            25 => matches!(&tag, b"NPC_" | b"LVLN"),
            26 | 27 => {
                tag == *b"FURN"
                    && r.subrecords().filter(|s| s.tag.0 == *b"FNPR").any(|s| {
                        s.data
                            .first()
                            .is_some_and(|v| *v & if kind == 26 { 2 } else { 1 } != 0)
                    })
            }
            28 => tag == *b"SHOU",
            // None / Weapons: None have no reference to select.
            _ => false,
        },
        _ => false,
    }
}

impl Engine {
    /// All matching world references, nearest first with a stable FormID tie.
    /// A whole-cell location admits every height and distance in that cell.
    pub(crate) fn package_objects(
        &self,
        actor: FormId,
        p: &Package,
        target: Target,
    ) -> Vec<FormId> {
        let Some((place, centre)) = self
            .package_location_place(actor, p)
            .or_else(|| self.current_place_of(actor))
        else {
            return vec![];
        };
        let radius = p
            .location
            .map(|l| l.radius)
            .filter(|r| r.is_finite() && *r > 0.)
            .unwrap_or(super::FURNITURE_SEARCH);
        let mut cells = vec![];
        match place {
            Place::Interior(c) => cells.push(c),
            Place::Exterior(w, _) => {
                if let Some(world) = self.lo.world(w) {
                    cells.extend(world.persistent_cell);
                    let low = crate::engine::grid_of((centre - Vec3::splat(radius)).truncate());
                    let high = crate::engine::grid_of((centre + Vec3::splat(radius)).truncate());
                    cells.extend(
                        world
                            .cells
                            .iter()
                            .filter(|(g, _)| {
                                g.0 >= low.0 && g.0 <= high.0 && g.1 >= low.1 && g.1 <= high.1
                            })
                            .map(|(_, &c)| c),
                    );
                }
            }
        }
        let mut refs = BTreeSet::new();
        for cell in cells {
            if let Some(c) = self.lo.cell(cell) {
                refs.extend(c.persistent.iter().chain(&c.temporary).copied());
            }
        }
        refs.extend(
            self.created_refs
                .refs
                .iter()
                .filter(|(_, c)| c.container.is_none())
                .map(|(&r, _)| r),
        );
        refs.extend(self.world_state.moved.keys().copied());
        refs.extend(self.actor_cells.keys().chain(self.away.keys()).copied());
        let npc = self
            .actor_ref(actor)
            .map_or_else(|| self.base_of(actor).unwrap_or(actor), |a| a.npc);
        let mut found = Vec::new();
        for r in refs {
            if r == actor || self.is_disabled(r) {
                continue;
            }
            let Some(rf) = self.reference_of(r).filter(|rf| !rf.deleted()) else {
                continue;
            };
            let matches = if let Some(f) = self
                .furniture
                .get(r)
                .filter(|_| matches!(target, Target::ObjectType(26 | 27)))
            {
                let kind = if matches!(target, Target::ObjectType(26)) {
                    Use::Sleep
                } else {
                    Use::Sit
                };
                f.markers.iter().any(|m| m.kind == kind)
            } else {
                matches_base(&self.lo, rf.base, target)
            };
            if !matches {
                continue;
            }
            let current = self
                .away
                .get(&r)
                .and_then(|x| Some((x.place?, x.pos)))
                .or_else(|| self.current_place_of(r));
            let Some((there, at)) = current else { continue };
            if !same_space(place, there)
                || !at.is_finite()
                || (centre.is_finite() && at.distance(centre) > radius)
            {
                continue;
            }
            if self.cells.values().flat_map(|rt| &rt.actors).any(|a| {
                a.ref_id != actor && !a.dead && a.exec.goal.is_some_and(|g| g.furniture == Some(r))
            }) {
                continue;
            }
            let mut rank = 0;
            if let Some(f) = self.furniture.get(r) {
                let kinds: &[Use] = match p.behaviour {
                    Behaviour::Sleep => &[Use::Sleep],
                    Behaviour::Sit if !matches!(target, Target::ObjectType(10)) => {
                        &[Use::Sit, Use::Special]
                    }
                    _ => &[Use::Sit, Use::Sleep, Use::Special, Use::Lean, Use::Idle],
                };
                if !f.markers.iter().enumerate().any(|(i, m)| {
                    kinds.contains(&m.kind)
                        && self
                            .furniture
                            .user_of(r, i as u8)
                            .is_none_or(|user| user == actor)
                        && super::furniture::may_use(&self.lo, npc, m.kind, f.owner)
                }) {
                    continue;
                }
                if p.behaviour == Behaviour::Sleep {
                    rank = match f.owner {
                        Some(owner) if owner == npc => 0,
                        Some(_) => 1,
                        None => 2,
                    };
                }
            }
            let distance = self
                .ref_position(actor)
                .filter(|v| v.is_finite())
                .map_or(0., |pos| at.distance_squared(pos));
            found.push((rank, distance, r));
        }
        found.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)).then(a.2.cmp(&b.2)));
        found.into_iter().map(|(_, _, r)| r).collect()
    }
}

#[cfg(test)]
#[path = "targets_tests.rs"]
mod tests;
