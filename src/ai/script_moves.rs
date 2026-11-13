//! Scripted movement: `PathToReference` (latent: the script waits for the
//! actor to get there) and `KeepOffsetFromActor` / `ClearKeepOffsetFromActor`
//! (`research/pex-natives/ai-batch20-script-moves.md`). Both pin the actor's
//! goal, as the console's `travel` does, so its packages don't replace it.

use super::*;

/// How near the target a path ends (units).
const ARRIVED: f32 = 96.0;
/// The longest a `PathToReference` waits (seconds) before giving up.
pub(crate) const PATH_WAIT_LIMIT: f32 = 300.0;

/// A `PathToReference` the calling script waits on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PathWait {
    pub actor: FormId,
    pub target: FormId,
    pub key: u64,
    /// The walk / run share asked for (`afWalkRunPercent`).
    pub share: f32,
}

/// The `Vm::signal` key prefix of `PathToReference` waits.
pub(crate) const PATH_KEY: u64 = 0x5041_5448_0000_0000;

/// Real seconds a path restored from a save waits for its actor's cell to
/// load before giving the script False (no source: loading's cells come in
/// within a few frames).
pub(crate) const RESTORED_PATH_WAIT: f64 = 10.0;

/// `KeepOffsetFromActor`'s settings for one actor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct KeepOffset {
    pub target: FormId,
    /// In the target's frame: x to its right, y ahead, z up.
    pub offset: Vec3,
    /// Turned from the target's heading (radians).
    pub angle: f32,
    pub catch_up: f32,
    pub follow: f32,
}

/// A gait for `PathToReference`'s share of the way from walking to running.
fn gait_for(share: f32) -> package::Gait {
    match share {
        s if s < 0.25 => package::Gait::Walk,
        s if s < 0.5 => package::Gait::FastWalk,
        s if s < 0.75 => package::Gait::Jog,
        _ => package::Gait::Run,
    }
}

impl Engine {
    /// `PathToReference(aTarget, afWalkRunPercent)`: the actor sets off; the
    /// key the script waits on, or None when it can't go (not loaded, or
    /// the target has no place).
    pub(crate) fn path_to_reference(
        &mut self,
        actor: FormId,
        target: FormId,
        share: f32,
    ) -> Option<u64> {
        let to = self.ref_position(target)?;
        self.script_moves.keep_offsets.remove(&actor);
        if !self.travel_to(actor, to, gait_for(share), false) {
            return None;
        }
        self.script_moves.next_key += 1;
        let key = PATH_KEY | self.script_moves.next_key;
        // A later request for the same actor interrupts the earlier one: it
        // fails at the next update (the VM is busy with this call now).
        for p in self
            .script_moves
            .paths
            .iter_mut()
            .filter(|p| p.actor == actor)
        {
            p.actor = FormId::NULL;
        }
        self.script_moves.paths.push(PathWait {
            actor,
            target,
            key,
            share,
        });
        Some(key)
    }

    /// Paths a loaded save restored (their scripts still waiting): each
    /// sets off again once its actor has loaded, or gives False if it can't,
    /// or if its actor hasn't loaded within [`RESTORED_PATH_WAIT`].
    fn start_restored_paths(&mut self) {
        let now = self.scripts.real_time;
        for (p, deadline) in std::mem::take(&mut self.script_moves.pending) {
            if !self.vm.is_waiting_for(p.key) {
                continue;
            }
            let loaded = self.actor_ref(p.actor).is_some();
            if !loaded && now < deadline {
                self.script_moves.pending.push((p, deadline));
                continue;
            }
            let started = loaded
                && !self.is_dead(p.actor)
                && self
                    .ref_position(p.target)
                    .is_some_and(|to| self.travel_to(p.actor, to, gait_for(p.share), false));
            if started {
                self.script_moves.paths.push(p);
            } else {
                self.vm.signal_with(p.key, papyrus::Value::Bool(false));
            }
        }
    }

    /// Release an actor's pinned goal to its packages.
    fn unpin(&mut self, actor: FormId) {
        if let Some(a) = self.actor_mut(actor) {
            a.exec.pinned = false;
            a.exec.next_eval = 0.0;
        }
    }

    /// `KeepOffsetFromActor`.
    pub(crate) fn keep_offset_from_actor(&mut self, actor: FormId, keep: KeepOffset) {
        if actor == PLAYER_REF || keep.target == actor {
            return;
        }
        self.script_moves.keep_offsets.insert(actor, keep);
    }

    /// `ClearKeepOffsetFromActor`.
    pub(crate) fn clear_keep_offset(&mut self, actor: FormId) {
        if self.script_moves.keep_offsets.remove(&actor).is_some() {
            self.unpin(actor);
        }
    }

    /// Paths arriving or failing, and actors keeping their offsets (every
    /// frame).
    pub(crate) fn update_script_moves(&mut self, dt: f32) {
        self.start_restored_paths();
        for p in std::mem::take(&mut self.script_moves.paths) {
            let pos = self.actor_ref(p.actor).map(|a| (a.exec.pos, a.exec.pinned));
            let to = self.ref_position(p.target);
            let result = match (pos, to) {
                (Some((at, true)), Some(to)) if !self.is_dead(p.actor) => {
                    if at.truncate().distance(to.truncate()) <= ARRIVED {
                        Some(true)
                    } else {
                        None
                    }
                }
                _ => Some(false),
            };
            match result {
                None => self.script_moves.paths.push(p),
                Some(ok) => {
                    if !p.actor.is_null() {
                        self.unpin(p.actor);
                    }
                    self.vm.signal_with(p.key, papyrus::Value::Bool(ok));
                }
            }
        }
        let keeps: Vec<(FormId, KeepOffset)> = self
            .script_moves
            .keep_offsets
            .iter()
            .map(|(&a, &k)| (a, k))
            .collect();
        for (actor, k) in keeps {
            let target = if k.target == PLAYER_REF {
                Some((self.player.position, self.camera.yaw))
            } else {
                self.actor_ref(k.target)
                    .map(|t| (t.exec.pos, t.exec.heading))
            };
            let (Some((at, heading)), Some(pos)) =
                (target, self.actor_ref(actor).map(|a| a.exec.pos))
            else {
                continue;
            };
            let (right, ahead) = (
                Vec3::new(heading.cos(), -heading.sin(), 0.0),
                Vec3::new(heading.sin(), heading.cos(), 0.0),
            );
            let spot = at + right * k.offset.x + ahead * k.offset.y + Vec3::Z * k.offset.z;
            let far = pos.truncate().distance(spot.truncate());
            if far > k.follow.max(1.0) {
                let gait = if far > k.catch_up {
                    package::Gait::Run
                } else {
                    package::Gait::Walk
                };
                self.travel_to(actor, spot, gait, false);
            } else if let Some(a) = self.actor_mut(actor) {
                a.exec.goal = Some(Goal::travel(pos));
                a.exec.pinned = true;
                let face = heading + k.angle;
                a.turn_towards(Vec3::new(face.sin(), face.cos(), 0.0), dt);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_walk_run_share_picks_a_gait() {
        use super::package::Gait;
        assert_eq!(super::gait_for(0.0), Gait::Walk);
        assert_eq!(super::gait_for(0.3), Gait::FastWalk);
        assert_eq!(super::gait_for(0.6), Gait::Jog);
        assert_eq!(super::gait_for(1.0), Gait::Run);
    }
}
