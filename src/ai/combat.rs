//! Fighting and dying.

use esp::FormId;
use glam::Vec3;

use crate::engine::Engine;
use crate::world::ragdoll::RagdollPose;

impl Engine {
    /// Kill an actor: it stops whatever it was doing and falls as a ragdoll (or
    /// just stops, without one). False if it isn't loaded or is already dead.
    pub fn kill_actor(&mut self, actor: FormId) -> bool {
        let Some(key) = self.actor_cells.get(&actor).copied() else { return false };
        let Some(index) = self.cells.get(&key).and_then(|rt| rt.actors.iter().position(|a| a.ref_id == actor)) else { return false };
        let (pose, transform) = match self.scene.cells.get(&key).and_then(|rc| rc.actors.get(index)) {
            Some(inst) => (inst.pose.clone(), inst.transform),
            None => return false,
        };
        let Some(a) = self.cells.get_mut(&key).and_then(|rt| rt.actors.get_mut(index)) else { return false };
        if a.dead {
            return false;
        }
        a.dead = true;
        a.objects_changed |= !a.objects.is_empty();
        a.objects.clear();
        let velocity = Vec3::new(a.heading.sin(), a.heading.cos(), 0.0) * a.speed;
        let skeleton = a.skeleton.clone();
        let capsule = a.capsule;
        if let Some(c) = capsule {
            self.physics.set_enabled(&[c], false);
        }
        if let Some(desc) = skeleton.ragdoll.clone() {
            let rd = self.physics.spawn_ragdoll(&desc, |i| transform * pose.get(desc.bodies[i].bone).copied().unwrap_or_default() * desc.bodies[i].offset, velocity, actor);
            let mapping = RagdollPose::new(&desc, &skeleton, &pose, transform);
            if let Some(a) = self.cells.get_mut(&key).and_then(|rt| rt.actors.get_mut(index)) {
                a.ragdoll = Some((rd, mapping));
            }
            log::info!("{actor} dies ({} ragdoll bodies, {} joints)", desc.bodies.len(), desc.joints.len());
        } else {
            log::info!("{actor} dies (no ragdoll)");
        }
        self.furniture.release(actor);
        // Scripts hear of it (killer unknown).
        self.send_script_event(actor, "OnDying", vec![papyrus::Value::None]);
        self.send_script_event(actor, "OnDeath", vec![papyrus::Value::None]);
        if self.barks.current.as_ref().is_some_and(|b| b.speaker == actor) {
            self.barks.current = None;
        }
        true
    }

    pub fn is_dead(&self, actor: FormId) -> bool {
        self.actor_cells.get(&actor).and_then(|k| self.cells.get(k)).and_then(|rt| rt.actors.iter().find(|a| a.ref_id == actor)).is_some_and(|a| a.dead)
    }
}
