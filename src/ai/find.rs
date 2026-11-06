//! Find's per-run ObjectLists, shared with Acquire and Flee procedures.
use super::{
    exec::Outcome,
    package::{Input, Package, Target},
    procedure::{Found, Path},
};
use crate::engine::Engine;
use esp::FormId;
use std::collections::HashSet;

fn inventory_criterion(e: &Engine, target: Target) -> bool {
    match target {
        Target::Object(item) => {
            crate::world::inventory::item_info(&e.lo, item).is_some()
                || e.lo.tag_of(item).is_some_and(|t| t.0 == *b"FLST")
        }
        Target::ObjectType(2 | 3 | 6 | 7 | 8 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 19 | 20) => true,
        _ => false,
    }
}

struct Request {
    path: Path,
    package: Package,
    list: u8,
    target: Target,
    own: bool,
    random: bool,
}

impl Engine {
    /// Revisit an exhausted acquisition search without forgetting its quota.
    pub(super) fn refill_acquisition(
        &mut self,
        actor: FormId,
        package: &Package,
        slot: u8,
    ) -> Vec<Found> {
        fn find(node: &super::package::Node, slot: u8) -> Option<&super::package::Node> {
            if node.procedure == "Find"
                && matches!(node.values.get(2), Some(Input::ObjectList(i)) if *i == slot)
            {
                return Some(node);
            }
            node.children.iter().find_map(|n| find(n, slot))
        }
        let Some(node) = package.tree.as_ref().and_then(|n| find(n, slot)) else {
            return vec![];
        };
        let Some(Input::Target(target)) = node.values.get(1) else {
            return vec![];
        };
        let mut found = self.search_package(
            actor,
            &Request {
                path: vec![],
                package: package.for_procedure(Some(node)),
                list: slot,
                target: *target,
                own: false,
                random: matches!(node.values.get(4), Some(Input::Bool(true))),
            },
        );
        found.retain(|f| !matches!(f, Found::Inventory { owner, .. } if *owner == actor));
        found
    }
    fn find_requests(&self, actor: FormId) -> Vec<Request> {
        let Some(x) = self.execution(actor) else {
            return vec![];
        };
        if !self.ai_enabled
            || self.ai_off.contains(&actor)
            || x.pinned
            || x.combat.is_some()
            || self.processing(actor).is_none()
        {
            return vec![];
        }
        let Some(run) = x.run.as_ref().filter(|r| r.outcome == Outcome::Running) else {
            return vec![];
        };
        let Some(p) = x.current.and_then(|i| x.packages.get(i)) else {
            return vec![];
        };
        let (Some(tree), Some(c)) = (&p.tree, &run.cursor) else {
            return vec![];
        };
        c.active
            .iter()
            .filter_map(|path| {
                let n = tree.at(path).filter(|n| n.procedure == "Find")?;
                let progress = c.states.get(path)?;
                // Count attempts and retain a real-time retry timer for this leaf.
                if progress.count > 0 && progress.elapsed < super::exec::AWAY_CADENCE as f64 {
                    return None;
                }
                let (Some(Input::Target(target)), Some(Input::ObjectList(list))) =
                    (n.values.get(1), n.values.get(2))
                else {
                    return None;
                };
                Some(Request {
                    path: path.clone(),
                    package: p.for_procedure(Some(n)),
                    list: *list,
                    target: *target,
                    own: matches!(n.values.get(3), Some(Input::Bool(true))),
                    random: matches!(n.values.get(4), Some(Input::Bool(true))),
                })
            })
            .collect()
    }

    fn search_package(&mut self, actor: FormId, q: &Request) -> Vec<Found> {
        let mut found = vec![];
        let item_search = inventory_criterion(self, q.target);
        if q.own {
            let items = self.inventory_mut(actor).items.clone();
            found.extend(
                items
                    .into_iter()
                    .filter(|(base, n)| *n > 0 && self.matches_package_item(*base, q.target))
                    .map(|(item, _)| Found::Inventory { owner: actor, item }),
            );
            if matches!(q.target, Target::Object(_) | Target::ObjectType(21..=24)) {
                found.extend(
                    self.actor_spells(actor)
                        .into_iter()
                        .filter(|&f| self.matches_package_item(f, q.target))
                        .map(|item| Found::Inventory { owner: actor, item }),
                );
            }
        }
        let direct = match q.target {
            Target::Object(_) | Target::ObjectType(_) => None,
            _ => self.package_target_of(actor, &q.package, Some(q.target)),
        };
        for r in self.package_references(actor, &q.package) {
            let Some(base) = self.base_of(r) else {
                continue;
            };
            if direct == Some(r) || self.matches_package_reference(r, q.target) {
                found.push(Found::Reference(r));
            }
            if item_search
                && (self.is_actor_ref(r) || self.lo.tag_of(base).is_some_and(|t| t.0 == *b"CONT"))
            {
                let items = self.inventory_mut(r).items.clone();
                found.extend(
                    items
                        .into_iter()
                        .filter(|(base, n)| *n > 0 && self.matches_package_item(*base, q.target))
                        .map(|(item, _)| Found::Inventory { owner: r, item }),
                );
            }
        }
        let mut seen = HashSet::new();
        found.retain(|f| seen.insert(*f));
        found.truncate(4096);
        if q.random {
            for i in (1..found.len()).rev() {
                found.swap(i, self.rand() as usize % (i + 1));
            }
        }
        found
    }

    /// Empty searches remain active beside Wander and retry on the AI cadence.
    pub(crate) fn run_find(&mut self, actor: FormId) -> bool {
        let mut complete = false;
        for q in self.find_requests(actor) {
            let found = self.search_package(actor, &q);
            if let Some(c) = self
                .execution_mut(actor)
                .and_then(|x| x.run.as_mut())
                .and_then(|r| r.cursor.as_mut())
            {
                if let Some(p) = c.states.get_mut(&q.path) {
                    p.count = p.count.saturating_add(1);
                    p.elapsed = 0.;
                }
                let filled = !found.is_empty();
                c.lists.insert(q.list, found);
                if filled {
                    c.finish(&q.path);
                    complete = true;
                }
            }
        }
        complete
    }

    pub(crate) fn run_loaded_finds(&mut self) {
        for _ in 0..8 {
            let mut actors: Vec<_> = self.actor_cells.keys().copied().collect();
            actors.sort();
            let mut changed = false;
            for actor in actors {
                changed |= self.run_find(actor);
                changed |= self.run_acquire(actor);
                changed |= self.run_package_flee(actor);
                changed |= self.run_follow_to(actor);
                changed |= self.advance_escort(actor);
            }
            if !changed {
                break;
            }
            self.advance_loaded_trees(0.);
            self.evaluate_packages(0.);
        }
    }
}

#[cfg(test)]
#[path = "find_tests.rs"]
mod tests;
