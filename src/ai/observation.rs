//! Observation and group-follow refinements on the authored procedure trees.
use super::{
    package::{Behaviour, Input, Package},
    schedule::same_space,
};
use crate::engine::Engine;

#[cfg(test)]
#[path = "observation_tests.rs"]
mod tests;
use esp::FormId;
use std::collections::HashMap;

impl Engine {
    pub(super) fn observation_targets(&self) -> HashMap<FormId, (FormId, bool)> {
        let mut targets = HashMap::new();
        for actor in self.actor_cells.keys().copied() {
            if let Some((p, _, n, _)) = self.active_procedure(actor, "KeepAnEyeOn") {
                let Some(Input::Target(t)) = n.values.first() else {
                    continue;
                };
                let Some(target) = self.package_target_of(actor, &p, Some(*t)) else {
                    continue;
                };
                if matches!(n.values.get(1), Some(Input::Location(area)) if self.within_package_location(target, actor, *area, p.quest) == Some(true))
                {
                    targets.insert(actor, (target, true));
                }
            }
        }
        // Explicit script requests take priority over package observation.
        targets.extend(
            self.scripts
                .look_at
                .iter()
                .map(|(&actor, &target)| (actor, target)),
        );
        targets.retain(|actor, (target, _)| {
            self.current_place_of(*actor)
                .zip(self.current_place_of(*target))
                .is_some_and(|((a, _), (b, _))| same_space(a, b))
        });
        targets
    }

    pub(crate) fn run_follow_to(&mut self, actor: FormId) -> bool {
        let Some((p, path, n, _)) = self.active_procedure(actor, "FollowTo") else {
            return false;
        };
        let (Some(Input::Target(target)), Some(Input::Location(destination))) =
            (n.values.first(), n.values.get(1))
        else {
            return false;
        };
        let Some(target) = self.package_target_of(actor, &p, Some(*target)) else {
            return false;
        };
        if self.within_package_location(target, actor, *destination, p.quest) != Some(true) {
            return false;
        }
        if let Some(x) = self.execution_mut(actor) {
            if let Some(c) = x.run.as_mut().and_then(|r| r.cursor.as_mut()) {
                c.finish(&path);
            }
            x.next_eval = 0.;
            x.settled = false;
        }
        true
    }

    pub(super) fn group_follow_radius(
        &self,
        actor: FormId,
        p: &Package,
        target: Option<FormId>,
        radius: (f32, f32),
    ) -> (f32, f32) {
        if p.template_is("SingleFileFollow") {
            let space =
                super::combat::gmst_f32(&self.lo, "fFollowSpaceBetweenFollowers", 192.).max(0.);
            return (radius.0.max(space), radius.1.max(space));
        }
        if !p.template_is("GroupMovement") {
            return radius;
        }
        let rank = self
            .actor_cells
            .keys()
            .chain(self.away.keys())
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .filter(|&r| r < actor)
            .filter(|&r| {
                let Some(x) = self.execution(r) else {
                    return false;
                };
                let Some(other) = x.current.and_then(|i| x.packages.get(i)) else {
                    return false;
                };
                if !other.template_is("GroupMovement") || self.is_dead(r) {
                    return false;
                }
                let effective = x.effective(other);
                effective.behaviour == Behaviour::Follow
                    && self.package_target_of(r, &effective, effective.target) == target
            })
            .count();
        let spacing = rank as f32
            * super::combat::gmst_f32(&self.lo, "fFollowSpaceBetweenFollowers", 192.).max(0.);
        (radius.0 + spacing, radius.1 + spacing)
    }
}
