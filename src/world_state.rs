//! What happens to references stays when their cells unload and load again (in
//! memory; what saves will write out): dead actors stay dead, lying where they
//! fell; doors the player opened stay open. Taken items, locks, enable state
//! and inventories are kept elsewhere (`ScriptState`, `Engine::inventories`).

use std::collections::HashMap;

use esp::FormId;
use glam::Vec3;

use crate::ai::schedule::Place;
use crate::engine::Engine;

#[derive(Default)]
pub struct WorldState {
    /// Dead actors and where their bodies lie.
    pub dead: HashMap<FormId, (Place, Vec3)>,
    /// Animated doors the player left open.
    pub open_doors: std::collections::HashSet<FormId>,
}

impl Engine {
    /// An actor died: remember it, and where (until its body settles elsewhere).
    pub(crate) fn remember_dead(&mut self, actor: FormId) {
        if let Some(at) = self.current_place_of(actor) {
            self.world_state.dead.insert(actor, at);
        }
    }

    /// Before a cell unloads: where its dead actors' bodies came to rest.
    pub(crate) fn remember_bodies(&mut self, key: crate::render::CellKey) {
        let Some(place) = self.place_of_key(key) else { return };
        let Some(rt) = self.cells.get(&key) else { return };
        let mut rest = Vec::new();
        for a in rt.actors.iter().filter(|a| a.dead) {
            let body = a.ragdoll.as_ref().and_then(|(rd, _)| self.physics.ragdoll_bodies(rd).first().map(|m| m.w_axis.truncate()));
            let mut pos = body.unwrap_or(a.pos);
            if let Some(z) = self.nav.height_at(pos) {
                pos.z = z;
            }
            rest.push((a.ref_id, pos));
        }
        for (r, pos) in rest {
            self.world_state.dead.insert(r, (place, pos));
        }
    }

    /// Where a dead actor's body lies.
    pub fn body_of(&self, actor: FormId) -> Option<(Place, Vec3)> {
        self.world_state.dead.get(&actor).copied()
    }
}
