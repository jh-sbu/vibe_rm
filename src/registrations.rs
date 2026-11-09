//! Line-of-sight and tracked-statistic registrations (`RegisterForLOS`,
//! `RegisterForSingleLOSGain` / `Lost`, `UnregisterForLOS`,
//! `RegisterForTrackedStatsEvent`), whose events go only to the form, alias
//! or effect that registered (the Papyrus index's descriptions:
//! `research/pex-natives/registration-batch13-los-stats.md`).

use esp::FormId;
use glam::Vec3;
use papyrus::{ObjectId, Value};

use crate::engine::{Engine, PLAYER_REF};

/// What a line-of-sight registration waits for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LosMode {
    /// `RegisterForLOS`: every gain and loss, the first at once.
    Both,
    /// `RegisterForSingleLOSGain`: the first time the viewer sees the target.
    Gain,
    /// `RegisterForSingleLOSLost`: the first time it doesn't.
    Lost,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LosRegistration {
    pub obj: ObjectId,
    pub viewer: FormId,
    pub target: FormId,
    pub mode: LosMode,
    /// Whether the viewer saw the target when last looked at (none yet).
    pub seen: Option<bool>,
}

/// Registrations looked at each frame, in turn ("roughly one pick runs per
/// frame"; a few more here, so a handful of them don't lag).
const LOS_PER_FRAME: usize = 4;

/// The stats that send `OnTrackedStatsEvent` (the Papyrus index's
/// `OnTrackedStatsEvent` page). Bounties are kept by the crime factions
/// here, not as counted stats, so they don't send it yet.
const TRACKED: [&str; 27] = [
    "Locations Discovered",
    "Dungeons Cleared",
    "Most Gold Carried",
    "Skill Increases",
    "Skill Books Read",
    "Quests Completed",
    "Misc Objectives Completed",
    "Dragon Souls Collected",
    "Words Of Power Learned",
    "Shouts Learned",
    "Shouts Mastered",
    "Magic Items Made",
    "Weapons Made",
    "Armor Made",
    "Locks Picked",
    "Pockets Picked",
    "Eastmarch Bounty",
    "Falkreath Bounty",
    "Haafingar Bounty",
    "Hjaalmarch Bounty",
    "The Pale Bounty",
    "The Reach Bounty",
    "The Rift Bounty",
    "Tribal Orcs Bounty",
    "Whiterun Bounty",
    "Winterhold Bounty",
    "Level Increases",
];

impl Engine {
    /// `RegisterForLOS` and the single forms: replaces `obj`'s registration
    /// for the same viewer and target.
    pub fn register_for_los(
        &mut self,
        obj: ObjectId,
        viewer: FormId,
        target: FormId,
        mode: LosMode,
    ) {
        self.unregister_for_los(obj, viewer, target);
        self.scripts.los.push(LosRegistration {
            obj,
            viewer,
            target,
            mode,
            seen: None,
        });
    }

    /// `UnregisterForLOS`.
    pub fn unregister_for_los(&mut self, obj: ObjectId, viewer: FormId, target: FormId) {
        self.scripts
            .los
            .retain(|r| !(r.obj == obj && r.viewer == viewer && r.target == target));
    }

    /// Whether a registration's object can still hear events: aliases and
    /// quests lose theirs when the quest stops, effects when they end.
    fn listener_alive(&self, obj: ObjectId) -> bool {
        let running = |q: u32| {
            self.scripts
                .quests
                .get(&FormId(q))
                .is_some_and(|s| s.running)
        };
        match obj {
            ObjectId::Alias { quest, .. } => running(quest),
            ObjectId::Form(f) if self.scripts.quests.contains_key(&FormId(f)) => running(f),
            ObjectId::Form(_) => true,
            ObjectId::Effect(id) => self.magic.effects.iter().any(|x| x.id == id && !x.done),
        }
    }

    /// Whether `viewer` sees `target` for a registration: the player when it
    /// is in the camera's view with a clear line to it; an NPC by its line
    /// of sight (`GetLineOfSight`'s view cone). None when either has no place
    /// in the world.
    fn los_sees(&self, viewer: FormId, target: FormId) -> Option<bool> {
        if viewer == PLAYER_REF {
            let to = self.ref_position(target)? + Vec3::Z * 60.0;
            let (eye, forward) = self.player_aim();
            let dir = (to - eye).normalize_or_zero();
            let half_y = self.camera.fov_y * 0.5;
            // A 16:9 view's horizontal half angle.
            let half = (half_y.tan() * 16.0 / 9.0).atan();
            if dir == Vec3::ZERO || forward.dot(dir) < half.cos() {
                return Some(false);
            }
        }
        self.line_of_sight(viewer, target)
    }

    /// Look at a few line-of-sight registrations this frame and queue their
    /// gain and loss events.
    pub(crate) fn update_los(&mut self) {
        let mut los = std::mem::take(&mut self.scripts.los);
        los.retain(|r| self.listener_alive(r.obj));
        let n = los.len();
        if n > 0 {
            let start = self.scripts.los_cursor % n;
            let mut done = Vec::new();
            for k in 0..LOS_PER_FRAME.min(n) {
                let i = (start + k) % n;
                let r = &mut los[i];
                let Some(seen) = self.los_sees(r.viewer, r.target) else {
                    continue;
                };
                if r.seen == Some(seen) {
                    continue;
                }
                r.seen = Some(seen);
                let event = if seen { "OnGainLOS" } else { "OnLostLOS" };
                let fires = match r.mode {
                    LosMode::Both => true,
                    LosMode::Gain => seen,
                    LosMode::Lost => !seen,
                };
                if fires {
                    let args = vec![self.object_value(r.viewer), self.object_value(r.target)];
                    self.scripts
                        .pending_events
                        .push((r.obj, event.into(), args));
                    if r.mode != LosMode::Both {
                        done.push(i);
                    }
                }
            }
            self.scripts.los_cursor = (start + LOS_PER_FRAME) % n;
            for i in done.into_iter().rev() {
                los.remove(i);
            }
        }
        // Registrations made by the events above were pushed meanwhile.
        los.append(&mut self.scripts.los);
        self.scripts.los = los;
    }

    /// `RegisterForTrackedStatsEvent` / `UnregisterForTrackedStatsEvent`.
    pub fn register_for_tracked_stats(&mut self, obj: ObjectId, on: bool) {
        let listeners = &mut self.scripts.tracked_stats_listeners;
        listeners.retain(|&o| o != obj);
        if on {
            listeners.push(obj);
        }
    }

    /// A counted stat changed to `value`: `OnTrackedStatsEvent` to those
    /// registered, for the stats that send it.
    pub(crate) fn tracked_stat_changed(&mut self, name: &str, value: i32) {
        let Some(name) = TRACKED.iter().find(|t| t.eq_ignore_ascii_case(name)) else {
            return;
        };
        let listeners = std::mem::take(&mut self.scripts.tracked_stats_listeners);
        let listeners: Vec<ObjectId> = listeners
            .into_iter()
            .filter(|&o| self.listener_alive(o))
            .collect();
        for &obj in &listeners {
            self.scripts.pending_events.push((
                obj,
                "OnTrackedStatsEvent".into(),
                vec![Value::str(name), Value::Int(value)],
            ));
        }
        self.scripts.tracked_stats_listeners = listeners;
    }
}

#[cfg(test)]
#[path = "registrations_tests.rs"]
mod tests;
