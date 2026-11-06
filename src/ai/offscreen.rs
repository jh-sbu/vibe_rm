//! Coarse NAVI routes, refined into local NAVM paths, for actors without bodies.
use super::{
    nav::{NavWorld, Policy},
    schedule::{Place, same_space},
};
use crate::engine::Engine;
use esp::FormId;
use glam::Vec3;
use std::{
    cell::RefCell,
    collections::{BTreeMap, BinaryHeap, HashMap},
    sync::Arc,
};

#[derive(Clone, Copy)]
struct Edge {
    to: FormId,
    door: Option<(FormId, FormId)>,
}
#[derive(Default)]
pub(crate) struct Network {
    infos: BTreeMap<FormId, esp::navi::Info>,
    edges: HashMap<FormId, Vec<Edge>>,
    regions: RefCell<HashMap<Place, Arc<NavWorld>>>,
}
#[derive(Clone, Copy)]
struct Portal {
    door: FormId,
    partner: FormId,
    place: Place,
    pos: Vec3,
    yaw: f32,
}
#[derive(Clone, Copy)]
struct Waypoint {
    place: Place,
    pos: Vec3,
    portal: Option<Portal>,
}
#[derive(Clone)]
pub(crate) struct Journey {
    there: Place,
    end: Vec3,
    policy: Policy,
    way: Vec<Waypoint>,
    next: usize,
    expected: (Place, Vec3),
}

#[derive(Clone, Copy, PartialEq)]
struct Open {
    cost: f32,
    id: FormId,
}
impl Eq for Open {}
impl Ord for Open {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other
            .cost
            .total_cmp(&self.cost)
            .then(other.id.cmp(&self.id))
    }
}
impl PartialOrd for Open {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

fn place(info: &esp::navi::Info) -> Place {
    if info.world.is_null() {
        Place::Interior(info.cell)
    } else {
        Place::Exterior(info.world, (i32::from(info.grid.0), i32::from(info.grid.1)))
    }
}
fn at(space: Place, pos: Vec3) -> Place {
    match space {
        Place::Exterior(w, _) => Place::Exterior(w, crate::engine::grid_of(pos.truncate())),
        p => p,
    }
}

impl Network {
    fn build(e: &Engine) -> Self {
        let infos = esp::navi::Index::load(&e.lo).infos;
        let mut doors: HashMap<FormId, Vec<FormId>> = HashMap::new();
        for (&mesh, info) in &infos {
            for &door in &info.doors {
                doors.entry(door).or_default().push(mesh);
            }
        }
        let mut edges = HashMap::new();
        for (&mesh, info) in &infos {
            let mut out = Vec::new();
            for &to in &info.links {
                if infos
                    .get(&to)
                    .is_some_and(|other| same_space(place(info), place(other)))
                {
                    out.push(Edge { to, door: None });
                }
            }
            for &door in &info.doors {
                let Some((partner, _, _)) = e.reference_of(door).and_then(|r| r.teleport) else {
                    continue;
                };
                for &to in doors.get(&partner).into_iter().flatten() {
                    out.push(Edge {
                        to,
                        door: Some((door, partner)),
                    });
                }
            }
            out.sort_by_key(|edge| (edge.to, edge.door));
            out.dedup_by_key(|edge| (edge.to, edge.door));
            edges.insert(mesh, out);
        }
        Self {
            infos,
            edges,
            regions: Default::default(),
        }
    }

    fn region(&self, e: &Engine, space: Place) -> Arc<NavWorld> {
        if let Some(nav) = self.regions.borrow().get(&space) {
            return nav.clone();
        }
        let mut nav = NavWorld::default();
        match space {
            Place::Interior(cell) => {
                nav.load_cell(&e.lo, cell);
            }
            Place::Exterior(world, (x, y)) => {
                if let Some(world) = e.lo.world(world) {
                    if let Some(cell) = world.persistent_cell {
                        nav.load_cell(&e.lo, cell);
                    }
                    for dx in -1..=1 {
                        for dy in -1..=1 {
                            if let Some(&cell) = world.cells.get(&(x + dx, y + dy)) {
                                nav.load_cell(&e.lo, cell);
                            }
                        }
                    }
                }
            }
        }
        let nav = Arc::new(nav);
        let mut cache = self.regions.borrow_mut();
        if cache.len() >= 64 {
            cache.clear();
        }
        cache.insert(space, nav.clone());
        nav
    }

    fn route(
        &self,
        e: &Engine,
        actor: FormId,
        from: (Place, Vec3),
        there: Place,
        end: Vec3,
        policy: Policy,
    ) -> Option<Journey> {
        let start = self.region(e, from.0).mesh_at_point(from.1)?;
        let goal = self.region(e, there).mesh_at_point(end)?;
        if !self.infos.contains_key(&start) || !self.infos.contains_key(&goal) {
            return None;
        }
        let mut cost = HashMap::from([(start, 0.)]);
        let mut came = HashMap::new();
        let mut open = BinaryHeap::from([Open {
            cost: 0.,
            id: start,
        }]);
        let mut found = false;
        while let Some(Open { cost: g, id }) = open.pop() {
            if cost.get(&id).is_some_and(|&c| g > c) {
                continue;
            }
            if id == goal {
                found = true;
                break;
            }
            let here = &self.infos[&id];
            for edge in self.edges.get(&id).into_iter().flatten() {
                if edge
                    .door
                    .is_some_and(|(a, b)| !e.may_cross_route_door(actor, a, b))
                {
                    continue;
                }
                let next = &self.infos[&edge.to];
                let preferred = here.preferred_links.contains(&edge.to) || next.preferred > 0.;
                if policy.only && !preferred {
                    continue;
                }
                let distance = if let Some((door, partner)) = edge.door {
                    Vec3::from(here.centre).distance(e.ref_position(door)?)
                        + Vec3::from(next.centre).distance(e.ref_position(partner)?)
                } else {
                    Vec3::from(here.centre).distance(Vec3::from(next.centre))
                };
                let ng = g + distance.max(1.) * if policy.prefer && !preferred { 4. } else { 1. };
                if cost.get(&edge.to).is_none_or(|&c| ng < c) {
                    cost.insert(edge.to, ng);
                    came.insert(edge.to, (id, *edge));
                    open.push(Open {
                        cost: ng,
                        id: edge.to,
                    });
                }
            }
        }
        if !found {
            return None;
        }
        let mut route = Vec::new();
        let mut node = goal;
        while node != start {
            let &(previous, edge) = came.get(&node)?;
            route.push(edge);
            node = previous;
        }
        route.reverse();
        let mut way = Vec::new();
        let (mut space, mut pos) = from;
        let walk = |space: Place, pos: Vec3, to: Vec3, way: &mut Vec<Waypoint>| -> Option<()> {
            for point in self.region(e, space).find_path_with(pos, to, policy)? {
                way.push(Waypoint {
                    place: at(space, point),
                    pos: point,
                    portal: None,
                });
            }
            Some(())
        };
        for edge in route {
            if let Some((door, partner)) = edge.door {
                let (_, exit, rotation) = e.reference_of(door)?.teleport?;
                let partner_place = e.current_place_of(partner)?.0;
                let entry = e
                    .reference_of(partner)
                    .and_then(|r| r.teleport)
                    .filter(|(back, _, _)| *back == door)
                    .map(|(_, p, _)| p)
                    .or_else(|| e.ref_position(door))?;
                walk(space, pos, entry, &mut way)?;
                let exit_place = at(partner_place, exit);
                way.push(Waypoint {
                    place: at(space, entry),
                    pos: entry,
                    portal: Some(Portal {
                        door,
                        partner,
                        place: exit_place,
                        pos: exit,
                        yaw: rotation.z,
                    }),
                });
                (space, pos) = (exit_place, exit);
            }
            let info = &self.infos[&edge.to];
            let to = if edge.to == goal {
                end
            } else {
                self.region(e, place(info))
                    .point_on_mesh(edge.to, Vec3::from(info.centre))?
            };
            walk(space, pos, to, &mut way)?;
            (space, pos) = (at(place(info), to), to);
        }
        if way.is_empty() {
            walk(space, pos, end, &mut way)?;
        }
        Some(Journey {
            there,
            end,
            policy,
            way,
            next: 0,
            expected: from,
        })
    }
}

impl Engine {
    pub(super) fn route_navigation_available(&self) -> bool {
        !self
            .offscreen
            .get_or_init(|| Network::build(self))
            .infos
            .is_empty()
    }

    fn may_cross_route_door(&self, actor: FormId, door: FormId, partner: FormId) -> bool {
        !self.is_disabled(door)
            && !self.is_disabled(partner)
            && self.npc_may_open(actor, door)
            && self.npc_may_open(actor, partner)
    }

    /// Consume a stride along a cached route. No route leaves the actor in place.
    pub(super) fn walk_offscreen(
        &mut self,
        actor: FormId,
        there: Place,
        end: Vec3,
        mut stride: f32,
        ends: bool,
    ) {
        let Some(from) = self.away.get(&actor).and_then(|x| Some((x.place?, x.pos))) else {
            return;
        };
        let policy = self.away[&actor].path_policy();
        self.inventory_mut(actor);
        let existing = self.offscreen_routes.remove(&actor).filter(|j| {
            same_space(j.there, there)
                && j.end.distance_squared(end) < 1.
                && j.policy == policy
                && same_space(j.expected.0, from.0)
                && j.expected.1.distance_squared(from.1) < 1.
                && j.way.iter().skip(j.next).all(|p| {
                    p.portal
                        .is_none_or(|p| self.may_cross_route_door(actor, p.door, p.partner))
                })
        });
        let journey = existing.or_else(|| {
            self.offscreen
                .get_or_init(|| Network::build(self))
                .route(self, actor, from, there, end, policy)
        });
        let Some(mut journey) = journey else {
            return;
        };
        let (mut space, mut pos) = from;
        for _ in 0..512 {
            let Some(point) = journey.way.get(journey.next).copied() else {
                break;
            };
            if !same_space(space, point.place) {
                return;
            }
            if point
                .portal
                .is_some_and(|p| !self.may_cross_route_door(actor, p.door, p.partner))
            {
                return;
            }
            let distance = pos.distance(point.pos);
            if distance > stride {
                pos += (point.pos - pos).normalize_or_zero() * stride;
                self.put_away(actor, at(space, pos), pos);
                break;
            }
            stride -= distance;
            pos = point.pos;
            space = point.place;
            journey.next += 1;
            self.put_away(actor, space, pos);
            if self.actor_cells.contains_key(&actor) {
                return;
            }
            if let Some(portal) = point.portal {
                for door in [portal.door, portal.partner] {
                    if self.is_locked(door) {
                        self.set_locked(door, false);
                    }
                }
                (space, pos) = (portal.place, portal.pos);
                self.put_away(actor, space, pos);
                if let Some(x) = self.execution_mut(actor) {
                    x.heading = portal.yaw;
                }
            }
            if self.actor_cells.contains_key(&actor) {
                return;
            }
            if stride <= 0. {
                break;
            }
        }
        if journey.next == journey.way.len() {
            if let Some(x) = self.away.get_mut(&actor) {
                x.settled = true;
                if ends {
                    x.complete_procedure(false);
                }
            }
        } else {
            journey.expected = (space, pos);
            self.offscreen_routes.insert(actor, journey);
        }
    }
}

#[cfg(test)]
#[path = "offscreen_tests.rs"]
mod tests;
