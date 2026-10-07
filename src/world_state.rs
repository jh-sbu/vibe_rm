//! What happens to references stays when their cells unload and load again (in
//! memory; what saves will write out): dead actors stay dead, lying where they
//! fell, in the pose they lay in; the hurt come back as hurt as they left, less
//! what they would have healed meanwhile; doors the player opened stay open.
//! Taken items, locks, enable state and inventories are kept elsewhere
//! (`ScriptState`, `Engine::inventories`).

use std::collections::HashMap;

use esp::FormId;
use glam::{Mat4, Vec3};

use crate::ai::schedule::Place;
use crate::engine::Engine;

#[derive(Default)]
pub struct WorldState {
    /// Dead actors and where their bodies lie.
    pub dead: HashMap<FormId, (Place, Vec3)>,
    /// Animated doors the player left open.
    pub open_doors: std::collections::HashSet<FormId>,
    /// Dead actors' ragdoll bodies as they lay (world transforms).
    pub body_poses: HashMap<FormId, Vec<Mat4>>,
    /// Hurt actors that unloaded.
    pub wounds: HashMap<FormId, Wounds>,
}

/// An actor's health as it unloaded, and how fast it comes back.
#[derive(Debug, Clone, Copy)]
pub struct Wounds {
    pub health: f32,
    pub max: f32,
    /// Share of the most a second (out of combat).
    pub rate: f32,
    /// `ScriptState::real_time` it unloaded at.
    pub at: f64,
}

impl Wounds {
    /// Health now, having healed since.
    pub fn health_at(&self, now: f64) -> f32 {
        (self.health + self.max * self.rate * (now - self.at).max(0.0) as f32).min(self.max)
    }
}

impl Engine {
    /// An actor died: remember it, and where (until its body settles elsewhere).
    pub(crate) fn remember_dead(&mut self, actor: FormId) {
        if let Some(at) = self.current_place_of(actor) {
            self.world_state.dead.insert(actor, at);
        }
    }

    /// Before a cell unloads: where its dead actors' bodies came to rest (and
    /// how), and how hurt the living are.
    pub(crate) fn remember_actors(&mut self, key: crate::render::CellKey) {
        let Some(place) = self.place_of_key(key) else { return };
        let Some(rt) = self.cells.get(&key) else { return };
        let now = self.scripts.real_time;
        let mut rest = Vec::new();
        let mut hurt = Vec::new();
        for a in &rt.actors {
            if !a.dead {
                let w = (a.health < a.stats.max_health - 0.5)
                    .then(|| Wounds { health: a.health, max: a.stats.max_health, rate: a.stats.health_regen / 100.0, at: now });
                hurt.push((a.ref_id, w));
                continue;
            }
            let bodies = a.ragdoll.as_ref().map(|(rd, _)| self.physics.ragdoll_bodies(rd));
            let mut pos = bodies.as_ref().and_then(|b| b.first()).map_or(a.pos, |m| m.w_axis.truncate());
            if let Some(z) = self.nav.height_at(pos) {
                pos.z = z;
            }
            rest.push((a.ref_id, pos, bodies));
        }
        for (r, pos, bodies) in rest {
            self.world_state.dead.insert(r, (place, pos));
            if let Some(b) = bodies {
                self.world_state.body_poses.insert(r, b);
            }
        }
        for (r, w) in hurt {
            match w {
                Some(w) => self.world_state.wounds.insert(r, w),
                None => self.world_state.wounds.remove(&r),
            };
        }
    }

    /// The health a loading actor comes back with (its most, unless it left hurt).
    pub(crate) fn returning_health(&mut self, actor: FormId, max: f32) -> f32 {
        let now = self.scripts.real_time;
        self.world_state.wounds.remove(&actor).map_or(max, |w| w.health_at(now).min(max))
    }

    /// Where a dead actor's body lies.
    pub fn body_of(&self, actor: FormId) -> Option<(Place, Vec3)> {
        self.world_state.dead.get(&actor).copied()
    }
}
