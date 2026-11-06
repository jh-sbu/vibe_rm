//! Escort destination and follower-list resolution; the tree owns its initial leg.
use super::{
    package::{Behaviour, Input, Location, LocationKind, Node, Package, Target},
    procedure::{Cursor, Found},
    schedule::{Place, same_space},
};
use crate::engine::Engine;
use esp::FormId;
use glam::Vec3;

#[cfg(test)]
#[path = "escort_tests.rs"]
mod tests;

impl super::exec::ActorExec {
    /// Hysteresis belongs to the durable leaf, not to the walking body.
    pub(super) fn escort_waiting(&mut self, lagging: bool, caught_up: bool) -> bool {
        let path = self
            .current
            .and_then(|i| self.packages.get(i))
            .and_then(|p| self.run.as_ref()?.cursor.as_ref()?.moving(p.tree.as_ref()?))
            .filter(|(_, n)| n.procedure == "Escort")
            .map(|(path, _)| path.clone());
        let Some(state) =
            path.and_then(|path| self.run.as_mut()?.cursor.as_mut()?.states.get_mut(&path))
        else {
            return lagging;
        };
        if lagging {
            state.spoken = true;
        } else if caught_up {
            state.spoken = false;
        }
        state.spoken
    }
}

fn initial_pending(n: &Node, c: &Cursor, path: &[u32]) -> bool {
    matches!(n.values.get(9), Some(Input::Location(_)))
        && !c.states.get(path).is_some_and(|s| s.arrived)
}

impl Engine {
    pub(super) fn escort_followers(
        &self,
        actor: FormId,
        p: &Package,
        n: &Node,
        c: &Cursor,
    ) -> Vec<FormId> {
        let mut refs = Vec::new();
        for value in [n.values.first(), n.values.get(10), n.values.get(11)]
            .into_iter()
            .flatten()
        {
            match value {
                Input::ObjectList(slot) => {
                    refs.extend(c.lists.get(slot).into_iter().flatten().map(|f| match *f {
                        Found::Reference(r) => r,
                        Found::Inventory { owner, .. } => owner,
                    }))
                }
                Input::Target(t @ (Target::Object(_) | Target::ObjectType(_))) => {
                    let mut search = p.clone();
                    search.location = Some(Location {
                        kind: LocationKind::NearSelf,
                        radius: p.escort_wait.max(768.),
                    });
                    refs.extend(self.package_objects(actor, &search, *t));
                }
                Input::Target(t) => refs.extend(self.package_target_of(actor, p, Some(*t))),
                _ => {}
            }
        }
        let mut seen = std::collections::HashSet::new();
        refs.retain(|&r| {
            r != actor
                && self.is_actor_ref(r)
                && !self.is_dead(r)
                && !self.is_disabled(r)
                && seen.insert(r)
        });
        let count = match n.values.get(1) {
            Some(Input::Int(v)) => (*v).max(0) as usize,
            _ => 1,
        };
        refs.truncate(count.min(4096));
        refs
    }

    pub(super) fn escort_destination(
        &self,
        actor: FormId,
        p: &Package,
        n: &Node,
        c: &Cursor,
        path: &[u32],
    ) -> Option<(Place, Vec3)> {
        let dest = if initial_pending(n, c, path) {
            n.values.get(9)
        } else {
            n.values.get(2)
        }?;
        let mut goal = p.clone();
        goal.target = None;
        goal.location = None;
        match dest {
            Input::Location(l) => goal.location = Some(*l),
            Input::Target(t) => {
                let r = self.package_target_of(actor, p, Some(*t))?;
                if p.template_is("PatrolAndHerd") && !initial_pending(n, c, path) {
                    let route = self.patrol_route(r);
                    if !route.is_empty() {
                        let index =
                            c.states.get(path).map_or(0, |s| s.count as usize) % route.len();
                        return self.current_place_of(route[index].ref_id);
                    }
                }
                return self.current_place_of(r);
            }
            _ => return None,
        }
        self.package_location_place(actor, &goal)
    }

    pub(super) fn project_escort(
        &self,
        actor: FormId,
        p: &Package,
        n: &Node,
        c: &Cursor,
        path: &[u32],
        effective: &mut Package,
    ) {
        if initial_pending(n, c, path) {
            effective.target = None;
            effective.behaviour = Behaviour::Travel;
            return;
        }
        let Some((space, pos)) = self.current_place_of(actor) else {
            return;
        };
        let destination = self
            .escort_destination(actor, p, n, c, path)
            .map_or(pos, |(_, at)| at);
        let left = pos.distance(destination);
        let mut followers = self.escort_followers(actor, p, n, c);
        // A missing or cross-space follower prevents leaving it behind.
        if followers.iter().any(|&r| {
            self.current_place_of(r)
                .is_none_or(|(s, _)| !same_space(s, space))
        }) {
            effective.behaviour = Behaviour::Hold;
        }
        followers.sort_by(|&a, &b| {
            let score = |r| {
                self.ref_position(r).map_or(f32::INFINITY, |at| {
                    if at.distance(destination) >= left {
                        pos.distance(at)
                    } else {
                        -pos.distance(at)
                    }
                })
            };
            score(b).total_cmp(&score(a)).then(a.cmp(&b))
        });
        effective.target = followers.first().copied().map(Target::Ref);
    }

    pub(crate) fn advance_escort(&mut self, actor: FormId) -> bool {
        let Some((p, path, n, c)) = self.active_procedure(actor, "Escort") else {
            return false;
        };
        let initial = initial_pending(&n, &c, &path);
        let patrol = p.template_is("PatrolAndHerd");
        if !initial && !patrol {
            return false;
        }
        if patrol && !initial {
            let Some(Input::Target(t)) = n.values.get(2) else {
                return false;
            };
            if self
                .package_target_of(actor, &p, Some(*t))
                .is_none_or(|r| self.patrol_route(r).len() < 2)
            {
                return false;
            }
        }
        let Some((here, pos)) = self.current_place_of(actor) else {
            return false;
        };
        let Some((there, at)) = self.escort_destination(actor, &p, &n, &c, &path) else {
            return false;
        };
        let radius = match n.values.get(9).filter(|_| initial) {
            Some(Input::Location(l)) => l.radius,
            _ => 0.,
        };
        if !same_space(here, there) || pos.distance(at) > radius.max(96.) {
            return false;
        }
        if let Some(x) = self.execution_mut(actor) {
            if let Some(s) = x
                .run
                .as_mut()
                .and_then(|r| r.cursor.as_mut())
                .and_then(|c| c.states.get_mut(&path))
            {
                if initial {
                    s.arrived = true;
                } else {
                    s.count = s.count.wrapping_add(1);
                }
            }
            x.next_eval = 0.;
            x.settled = false;
        }
        true
    }
}
