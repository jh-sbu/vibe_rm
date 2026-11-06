//! Object-list consumers. Selection is revalidated before movement or transfer.
use super::{
    exec::Outcome,
    package::{Behaviour, Gait, Input, Location, LocationKind, Node, Package},
    procedure::{Cursor, Found, Path},
    schedule::{Place, same_space},
};
use crate::engine::Engine;
use esp::FormId;
use glam::Vec3;

fn number(n: &Node, i: usize, default: f32) -> f32 {
    match n.values.get(i) {
        Some(Input::Float(v)) => *v,
        Some(Input::Int(v)) => *v as f32,
        _ => default,
    }
}
fn flag(n: &Node, i: usize) -> bool {
    matches!(n.values.get(i), Some(Input::Bool(true)))
}
fn source(f: Found) -> FormId {
    match f {
        Found::Reference(r) => r,
        Found::Inventory { owner, .. } => owner,
    }
}

impl Engine {
    pub(super) fn active_procedure(
        &self,
        actor: FormId,
        name: &str,
    ) -> Option<(Package, Path, Node, Cursor)> {
        let x = self.execution(actor)?;
        if !self.ai_enabled
            || self.ai_off.contains(&actor)
            || x.pinned
            || x.combat.is_some()
            || self.is_dead(actor)
        {
            return None;
        }
        let p = x.current.and_then(|i| x.packages.get(i))?;
        let c = x
            .run
            .as_ref()
            .filter(|r| r.outcome == Outcome::Running)?
            .cursor
            .as_ref()?;
        let tree = p.tree.as_ref()?;
        let path = c
            .active
            .iter()
            .find(|path| tree.at(path).is_some_and(|n| n.procedure == name))?;
        Some((p.clone(), path.clone(), tree.at(path)?.clone(), c.clone()))
    }

    fn acquisition_owner(&self, actor: FormId, found: Found) -> Option<FormId> {
        let r = source(found);
        if matches!(found, Found::Inventory { owner, .. } if owner == actor || (self.is_actor_ref(owner) && self.is_dead(owner)))
        {
            return None;
        }
        let owner = self
            .ref_owner(r)
            .or_else(|| self.is_actor_ref(r).then(|| self.base_of(r)).flatten())?;
        if Some(owner) == self.base_of(actor) {
            return None;
        }
        if self.lo.tag_of(owner).is_some_and(|t| t.0 == *b"FACT") {
            let flags = self
                .lo
                .get(owner)
                .and_then(|r| {
                    r.get(b"DATA")
                        .and_then(|d| d.get(..4))
                        .map(|d| u32::from_le_bytes(d.try_into().unwrap()))
                })
                .unwrap_or(0);
            if flags & 0x8000 == 0
                || self.npc_factions(actor).iter().any(|&(f, rank)| {
                    f == owner && i32::from(rank) >= self.ref_owner_rank(r).max(0)
                })
            {
                return None;
            }
        }
        Some(owner)
    }

    fn acquire_candidate(
        &self,
        actor: FormId,
        n: &Node,
        c: &Cursor,
        path: &[u32],
    ) -> Option<(usize, Found)> {
        let Input::ObjectList(slot) = n.values.first()? else {
            return None;
        };
        let next = c.states.get(path).map_or(0, |s| s.next as usize);
        c.lists
            .get(slot)?
            .iter()
            .copied()
            .enumerate()
            .skip(next)
            .find(|(_, f)| {
                let r = source(*f);
                if self.is_disabled(r) || self.reference_deleted(r) {
                    return false;
                }
                if !flag(n, 2) && self.acquisition_owner(actor, *f).is_some() {
                    return false;
                }
                match *f {
                    Found::Reference(r) => self.is_item_ref(r),
                    Found::Inventory { owner, item } => {
                        let own_spell = owner == actor && self.has_spell(actor, item);
                        if !own_spell
                            && self
                                .inventories
                                .get(&owner)
                                .is_none_or(|inv| inv.count(item) <= 0)
                        {
                            return false;
                        }
                        let living =
                            owner != actor && self.is_actor_ref(owner) && !self.is_dead(owner);
                        !living
                            || (flag(n, 3)
                                && flag(n, 2)
                                && self.can_pickpocket_item_by(actor, owner, item))
                    }
                }
            })
    }

    // A missing record is stale, but a created reference is valid without one.
    fn reference_deleted(&self, r: FormId) -> bool {
        self.created(r).is_none() && self.lo.get(r).is_none_or(|r| r.header.is_deleted())
    }

    /// Current procedure's explicit destination, shared by loaded and away goals.
    pub(super) fn procedure_destination(
        &self,
        actor: FormId,
        p: &Package,
        c: &Cursor,
    ) -> Option<(Place, Vec3)> {
        let (path, n) = c.moving(p.tree.as_ref()?)?;
        match n.procedure.as_str() {
            "Escort" => self.escort_destination(actor, p, n, c, path),
            "Travel" => match n.values.first() {
                Some(Input::ObjectList(slot)) => c
                    .lists
                    .get(slot)?
                    .iter()
                    .find_map(|f| self.current_place_of(source(*f))),
                _ => None,
            },
            "Acquire" => {
                let (_, f) = self.acquire_candidate(actor, n, c, path)?;
                self.current_place_of(source(f))
            }
            "Flee" => self.flee_destination(actor, p, n, c),
            _ => None,
        }
    }

    fn flee_targets(&self, actor: FormId, p: &Package, n: &Node, c: &Cursor) -> Vec<FormId> {
        let mut refs: Vec<_> = match n.values.first() {
            Some(Input::ObjectList(slot)) => c
                .lists
                .get(slot)
                .into_iter()
                .flatten()
                .copied()
                .map(source)
                .collect(),
            Some(Input::Target(t)) => self
                .package_target_of(actor, p, Some(*t))
                .into_iter()
                .collect(),
            _ => vec![],
        };
        // Only an unspecified list may use current hostile actors as threats.
        if refs.is_empty() && !matches!(n.values.first(), Some(Input::Target(_))) {
            refs.extend(
                self.cells
                    .values()
                    .flat_map(|rt| &rt.actors)
                    .filter(|a| {
                        a.exec
                            .combat
                            .as_ref()
                            .is_some_and(|fight| fight.target == actor)
                    })
                    .map(|a| a.ref_id),
            );
        }
        refs.retain(|&r| r != actor && !self.is_disabled(r) && (!flag(n, 7) || !self.is_dead(r)));
        refs
    }

    fn flee_destination(
        &self,
        actor: FormId,
        p: &Package,
        n: &Node,
        c: &Cursor,
    ) -> Option<(Place, Vec3)> {
        let (place, pos) = self.current_place_of(actor)?;
        if !flag(n, 2) {
            let mut destination = p.for_procedure(Some(n));
            destination.location = n.values.get(1).and_then(|v| {
                if let Input::Location(l) = v {
                    Some(*l)
                } else {
                    None
                }
            });
            return self.package_location_place(actor, &destination);
        }
        let threats = self.flee_targets(actor, p, n, c);
        let nearest = threats
            .into_iter()
            .filter_map(|r| self.current_place_of(r))
            .filter(|(space, _)| same_space(place, *space))
            .map(|(_, at)| at)
            .min_by(|a, b| a.distance_squared(pos).total_cmp(&b.distance_squared(pos)))?;
        let distance = self.flee_distance(place, n);
        if pos.distance(nearest) >= distance {
            return Some((place, pos));
        }
        let mut seed =
            actor.0 as u64 ^ c.revision ^ (pos.x.to_bits() as u64) << 32 ^ pos.y.to_bits() as u64;
        let point = super::combat::place_away_on(&self.nav, pos, nearest, 1200., || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            seed
        });
        // Away spaces have no loaded NavWorld. Coarse routing is added in step 12.
        let point = point.or_else(|| {
            (!self.actor_cells.contains_key(&actor)).then(|| {
                pos + (pos - nearest).normalize_or(Vec3::X)
                    * (distance - pos.distance(nearest)).min(1200.)
            })
        });
        point.map(|point| (place, point))
    }

    fn flee_distance(&self, place: Place, n: &Node) -> f32 {
        let distance = number(n, 5, 0.);
        if distance > 0. {
            distance
        } else {
            let (setting, fallback) = if matches!(place, Place::Interior(_)) {
                ("fFleeDistanceInterior", 3000.)
            } else {
                ("fFleeDistanceExterior", 5000.)
            };
            super::combat::gmst_f32(&self.lo, setting, fallback).max(0.)
        }
    }

    pub(super) fn project_procedure(&self, actor: FormId, p: &Package, c: &Cursor) -> Package {
        let mut effective = p.for_procedure(c.body(p.tree.as_ref().unwrap()));
        if let Some((path, n)) = c.moving(p.tree.as_ref().unwrap()) {
            if n.procedure == "Escort" {
                self.project_escort(actor, p, n, c, path, &mut effective);
            }
            if n.procedure == "UseIdleMarker"
                && !self
                    .package_target_of(actor, &effective, effective.target)
                    .is_some_and(|r| self.furniture.get(r).is_some())
            {
                effective.behaviour = Behaviour::Hold;
            }
            if matches!(n.procedure.as_str(), "Acquire" | "Flee") {
                effective.behaviour = Behaviour::Travel;
                effective.location = Some(Location {
                    kind: LocationKind::NearSelf,
                    radius: if n.procedure == "Flee" {
                        number(n, 3, 96.).max(1.)
                    } else {
                        64.
                    },
                });
                effective.target = None;
                if n.procedure == "Acquire"
                    && self
                        .acquire_candidate(
                            actor,
                            n,
                            c,
                            &c.moving(p.tree.as_ref().unwrap()).unwrap().0,
                        )
                        .is_none()
                {
                    effective.behaviour = Behaviour::Hold;
                }
                if n.procedure == "Flee" {
                    effective.gait = Gait::Run;
                }
            }
        }
        effective
    }

    pub(crate) fn run_acquire(&mut self, actor: FormId) -> bool {
        let Some((p, path, n, c)) = self.active_procedure(actor, "Acquire") else {
            return false;
        };
        let quantity = number(&n, 1, 1.).max(0.) as u32;
        let count = c.states.get(&path).map_or(0, |s| s.count);
        if count >= quantity {
            self.finish_consumer(actor, &path);
            return true;
        }
        let Some((index, found)) = self.acquire_candidate(actor, &n, &c, &path) else {
            if c.states
                .get(&path)
                .is_some_and(|s| s.elapsed >= super::exec::AWAY_CADENCE as f64)
                && let Some(Input::ObjectList(slot)) = n.values.first()
            {
                let found = self.refill_acquisition(actor, &p, *slot);
                let filled = !found.is_empty();
                if let Some(c) = self
                    .execution_mut(actor)
                    .and_then(|x| x.run.as_mut())
                    .and_then(|r| r.cursor.as_mut())
                {
                    c.lists.insert(*slot, found);
                    if let Some(s) = c.states.get_mut(&path) {
                        s.next = 0;
                        s.elapsed = 0.;
                    }
                }
                if let Some(x) = self.execution_mut(actor) {
                    x.next_eval = 0.;
                    x.settled = false;
                }
                return filled;
            }
            return false;
        };
        let r = source(found);
        let Some((here, pos)) = self.current_place_of(actor) else {
            return false;
        };
        let Some((there, at)) = self.current_place_of(r) else {
            return false;
        };
        if !same_space(here, there) || pos.distance(at) > 96. {
            return false;
        }
        let mark = self.acquisition_owner(actor, found);
        let remaining = (quantity - count).min(i32::MAX as u32) as i32;
        let (moved, item, pocket) = match found {
            Found::Reference(r) => {
                let item = self.base_of(r).unwrap();
                let before = self.item_count(actor, item);
                self.take_item_by(actor, r, mark);
                (self.item_count(actor, item) - before, item, false)
            }
            Found::Inventory { owner, item } if owner == actor => (
                if self.has_spell(actor, item) {
                    1
                } else {
                    self.item_count(actor, item).min(remaining)
                },
                item,
                false,
            ),
            Found::Inventory { owner, item } => {
                let pocket = self.is_actor_ref(owner) && !self.is_dead(owner);
                if pocket
                    && (self.rand() % 100) as f32
                        >= self.pickpocket_chance_by(actor, owner, item, remaining)
                {
                    self.report_package_theft(actor, Some(owner), mark, 0, true);
                    (0, item, false)
                } else {
                    let moved = self
                        .remove_stack_with_copy(
                            owner,
                            item,
                            remaining,
                            None,
                            Some(actor),
                            mark,
                            None,
                        )
                        .parts
                        .iter()
                        .map(|(_, n)| n)
                        .sum();
                    self.unequip_if_gone(owner, item);
                    (moved, item, pocket)
                }
            }
        };
        if moved > 0 && mark.is_some() {
            let value = crate::world::inventory::item_info(&self.lo, item)
                .map_or(0, |i| i.value)
                .saturating_mul(moved);
            self.report_package_theft(
                actor,
                self.is_actor_ref(r).then_some(r),
                mark,
                value,
                pocket,
            );
        }
        if let Some(state) = self
            .execution_mut(actor)
            .and_then(|x| x.run.as_mut())
            .and_then(|r| r.cursor.as_mut())
            .and_then(|c| c.states.get_mut(&path))
        {
            state.count = state.count.saturating_add(moved.max(0) as u32);
            state.next = index as u32 + 1;
            if state.count >= quantity {
                state.finished = true;
            }
        }
        if let Some(x) = self.execution_mut(actor) {
            x.next_eval = 0.;
            x.settled = false;
        }
        true
    }

    fn finish_consumer(&mut self, actor: FormId, path: &[u32]) {
        if let Some(x) = self.execution_mut(actor) {
            if let Some(c) = x.run.as_mut().and_then(|r| r.cursor.as_mut()) {
                c.finish(path);
            }
            x.next_eval = 0.;
            x.settled = false;
        }
    }

    pub(crate) fn run_package_flee(&mut self, actor: FormId) -> bool {
        let Some((p, path, n, c)) = self.active_procedure(actor, "Flee") else {
            return false;
        };
        let Some((place, pos)) = self.current_place_of(actor) else {
            return false;
        };
        let complete = if flag(&n, 2) {
            self.flee_targets(actor, &p, &n, &c)
                .into_iter()
                .filter_map(|r| self.current_place_of(r))
                .filter(|(s, _)| same_space(place, *s))
                .all(|(_, at)| pos.distance(at) >= self.flee_distance(place, &n))
        } else {
            self.flee_destination(actor, &p, &n, &c)
                .is_some_and(|(space, at)| {
                    same_space(place, space) && pos.distance(at) <= number(&n, 3, 96.).max(1.)
                })
        };
        if complete {
            self.finish_consumer(actor, &path);
            return true;
        }
        if !flag(&n, 8)
            && self.actor_cells.contains_key(&actor)
            && c.states
                .get(&path)
                .is_some_and(|s| !s.spoken || s.elapsed >= 4.)
            && self.bark_channel_available()
        {
            self.bark(actor, b"FLEE");
            if let Some(s) = self
                .execution_mut(actor)
                .and_then(|x| x.run.as_mut())
                .and_then(|r| r.cursor.as_mut())
                .and_then(|c| c.states.get_mut(&path))
            {
                s.spoken = true;
                s.elapsed = 0.;
            }
        }
        false
    }

    pub(super) fn package_is_fleeing(&self, actor: FormId) -> bool {
        self.active_procedure(actor, "Flee")
            .is_some_and(|(_, _, n, _)| match n.values.get(9) {
                Some(Input::Bool(v)) => *v,
                _ => true,
            })
    }
}
