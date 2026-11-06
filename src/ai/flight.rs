//! Dragon flight and the flight leaves of authored package trees.
use super::*;
use package::{Input, Node};
#[cfg(test)]
#[path = "flight_tests.rs"]
mod tests;

pub(crate) const GROUND: u8 = 0;
pub(crate) const TAKEOFF: u8 = 1;
pub(crate) const CRUISE: u8 = 2;
pub(crate) const HOVER: u8 = 3;
pub(crate) const LAND: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Flight {
    pub allowed: bool,
    pub allow_crash: bool,
    pub allow_search: bool,
    pub marker: Option<FormId>,
    pub phase: u8,
    pub elapsed: f32,
    pub combat_elapsed: f32,
    pub grabbed: Option<FormId>,
    pub perched: Option<FormId>,
    pub combat_mode: u8,
    pub mode_time: f32,
}
impl Default for Flight {
    fn default() -> Self {
        Self {
            allowed: true,
            allow_crash: true,
            allow_search: true,
            marker: None,
            phase: GROUND,
            elapsed: 0.,
            combat_elapsed: 0.,
            grabbed: None,
            perched: None,
            combat_mode: 0,
            mode_time: 0.,
        }
    }
}
fn number(n: &Node, i: usize, default: f32) -> f32 {
    match n.values.get(i) {
        Some(Input::Float(f)) if f.is_finite() => *f,
        Some(Input::Int(i)) => *i as f32,
        _ => default,
    }
}

/// Keep a stable ring inside authored bounds; advance on a tangent, with a
/// radial correction to join the ring when starting outside it.
fn orbit_target(
    pos: Vec3,
    center: Vec3,
    inner: f32,
    outer: f32,
    height: f32,
    distance: f32,
) -> Vec3 {
    let inner = inner.max(1.);
    let outer = outer.max(inner);
    let radius = (inner + outer) * 0.5;
    let d = (pos - center).truncate();
    let angle = d.y.atan2(d.x) + distance / radius;
    center + Vec3::new(angle.cos() * radius, angle.sin() * radius, height)
}

impl Engine {
    pub(crate) fn flying_state(&self, actor: FormId) -> u8 {
        self.flights.get(&actor).map_or(GROUND, |f| f.phase)
    }
    pub(crate) fn set_allow_flying(
        &mut self,
        actor: FormId,
        allowed: bool,
        crash: bool,
        search: bool,
    ) {
        if actor == PLAYER_REF || !self.is_actor_ref(actor) {
            return;
        }
        let f = self.flights.entry(actor).or_default();
        f.allowed = allowed;
        f.allow_crash = crash;
        f.allow_search = search;
    }
    pub(crate) fn set_landing_marker(&mut self, actor: FormId, marker: Option<FormId>) {
        if actor != PLAYER_REF && self.is_actor_ref(actor) {
            self.flights.entry(actor).or_default().marker = marker.filter(|m| !m.is_null());
        }
    }
    fn dragon(&self, actor: FormId) -> bool {
        self.has_keyword(actor, FormId(0x35D59))
    }

    fn flight_speed(&self, actor: FormId) -> (f32, f32) {
        let movement = self
            .race_of(actor)
            .and_then(|r| self.lo.get(r))
            .and_then(|r| {
                let d = r.get(b"FLMV")?;
                Some(r.fid(FormId(u32::from_le_bytes(d.get(..4)?.try_into().ok()?))))
            })
            .and_then(|r| self.lo.get(r))
            .and_then(|r| {
                let d = r.get(b"SPED")?;
                let f = |at| Some(f32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?));
                Some((f(16)?, f(36)?))
            });
        movement
            .filter(|(s, t)| s.is_finite() && *s > 0. && t.is_finite() && *t > 0.)
            .unwrap_or((1000., 1.57))
    }

    fn flight_style(&self, actor: FormId) -> [f32; 8] {
        let mut values = [0.5, 1., 0.5, 0.5, 0.5, 0.5, 0.5, 0.75];
        if let Some(r) = self
            .templates_of(actor)
            .and_then(|t| t.form(&self.lo, crate::world::template::AI_DATA, b"ZNAM"))
            .and_then(|r| self.lo.get(r))
            && let Some(data) = r.get(b"CSFL")
        {
            for (v, d) in values.iter_mut().zip(data.chunks_exact(4)) {
                let f = f32::from_le_bytes(d.try_into().unwrap());
                if f.is_finite() {
                    *v = f.clamp(0., 1.);
                }
            }
        }
        values
    }
    fn nearest_dragon_perch(&self, actor: FormId, pos: Vec3) -> Option<FormId> {
        let place = self.current_place_of(actor)?.0;
        self.furniture
            .items
            .iter()
            .filter(|f| {
                !self.is_disabled(f.ref_id)
                    && self
                        .current_place_of(f.ref_id)
                        .is_some_and(|(p, _)| schedule::same_space(p, place))
                    && (f.ways.iter().any(|w| {
                        w.graph
                            .as_ref()
                            .is_some_and(|g| g.to_ascii_lowercase().contains("dragon"))
                    }) || self
                        .base_of(f.ref_id)
                        .and_then(|r| self.lo.get(r))
                        .and_then(|r| r.editor_id())
                        .is_some_and(|n| n.to_ascii_lowercase().contains("dragonperch")))
            })
            .filter_map(|f| Some((f.ref_id, self.ref_position(f.ref_id)?.distance_squared(pos))))
            .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
            .map(|v| v.0)
    }

    fn flight_leaf(&self, actor: FormId) -> Option<(Package, procedure::Path, Node)> {
        let x = self.execution(actor)?;
        if x.pinned || x.combat.is_some() {
            return None;
        }
        let p = x.current.and_then(|i| x.packages.get(i))?;
        let c = x.run.as_ref()?.cursor.as_ref()?;
        let tree = p.tree.as_ref()?;
        // Shout runs alongside Orbit in several templates; it observes the
        // caster while Orbit supplies flight movement.
        let (path, n) = c
            .active
            .iter()
            .find_map(|path| {
                let n = tree.at(path)?;
                matches!(n.procedure.as_str(), "Orbit" | "Hover" | "FlightGrab")
                    .then_some((path, n))
            })
            .or_else(|| c.moving(tree))?;
        Some((p.clone(), path.clone(), n.clone()))
    }

    fn set_flight_phase(&mut self, actor: FormId, phase: u8, event: &str) {
        let f = self.flights.entry(actor).or_default();
        if f.phase != phase {
            f.phase = phase;
            if phase != GROUND && phase != LAND {
                f.perched = None;
            }
            f.elapsed = 0.;
            self.send_actor_graph_event(actor, event);
        }
    }

    pub(crate) fn update_flights(&mut self, dt: f32) {
        for a in self.cells.values_mut().flat_map(|c| &mut c.actors) {
            a.flight_controlled = false;
        }
        let mut actors = self
            .actor_cells
            .keys()
            .copied()
            .filter(|&a| self.dragon(a))
            .collect::<Vec<_>>();
        actors.sort();
        for actor in actors {
            self.step_flight(actor, dt.max(0.), false);
        }
    }

    /// Returns whether this dragon's movement was handled, also when away.
    pub(super) fn step_flight(&mut self, actor: FormId, dt: f32, away: bool) -> bool {
        if !self.dragon(actor)
            || !self.ai_enabled
            || self.ai_off.contains(&actor)
            || self.is_disabled(actor)
            || self.is_dead(actor)
        {
            return false;
        }
        let Some((place, pos, heading, pinned)) = self.execution(actor).map(|x| {
            (
                x.place
                    .or_else(|| self.current_place_of(actor).map(|a| a.0)),
                x.pos,
                x.heading,
                x.pinned,
            )
        }) else {
            return false;
        };
        if pinned {
            return false;
        }
        let Some(place) = place else {
            return false;
        };
        let (speed, turn) = self.flight_speed(actor);
        let leaf = self.flight_leaf(actor);
        let combat = self
            .execution(actor)
            .and_then(|x| x.combat.as_ref())
            .map(|c| c.target);
        let f = self.flights.entry(actor).or_default();
        f.elapsed = (f.elapsed + dt).min(3600.);
        f.combat_elapsed = if combat.is_some() {
            (f.combat_elapsed + dt).min(3600.)
        } else {
            0.
        };
        let mut f = *f;
        if !f.allowed && f.phase == GROUND {
            let perched = f.perched.is_some();
            if let Some(a) = self.actor_mut(actor) {
                a.flight_controlled = perched;
                a.velocity = Vec3::ZERO;
                a.speed = 0.;
            }
            return perched;
        }
        let mut finish = None;
        let mut perch = None;
        let mut hover = false;
        let mut target = None;
        let land_after = combat::gmst_f32(&self.lo, "fCombatHoverTimeMin", 5.).max(1.);
        if let Some(enemy) = combat {
            let style = self.flight_style(actor);
            if f.combat_elapsed >= f.mode_time {
                let weights = [style[7], style[0], style[2], style[5]];
                let total = weights.iter().sum::<f32>().max(0.001);
                let mut roll = (self.rand() % 10000) as f32 / 10000. * total;
                f.combat_mode = 0;
                for (i, weight) in weights.into_iter().enumerate() {
                    if roll < weight {
                        f.combat_mode = i as u8;
                        break;
                    }
                    roll -= weight;
                }
                f.combat_elapsed = 0.;
                f.mode_time = land_after
                    * match f.combat_mode {
                        1 => style[3].max(0.1) * 2.,
                        2 => style[4].max(0.1) * 2.,
                        3 => style[6].max(0.1) * 2.,
                        _ => 2.,
                    };
                let state = self.flights.get_mut(&actor).unwrap();
                state.combat_mode = f.combat_mode;
                state.combat_elapsed = 0.;
                state.mode_time = f.mode_time;
            }
            if let Some(enemy_pos) = self.ref_position(enemy) {
                if target.is_none() {
                    match f.combat_mode {
                        1 if f.allowed => {
                            target = Some(enemy_pos + Vec3::Z * 1000.);
                            hover = true;
                        }
                        2 => target = self.nav.height_at(enemy_pos).map(|z| enemy_pos.with_z(z)),
                        3 => {
                            perch = self.nearest_dragon_perch(actor, enemy_pos);
                            target = perch.and_then(|r| self.ref_position(r));
                        }
                        _ => {}
                    }
                    if target.is_none() && f.allowed {
                        // An unavailable ground/perch destination cannot turn
                        // the fallback orbit point into an airborne landing.
                        f.combat_mode = 0;
                        self.flights.get_mut(&actor).unwrap().combat_mode = 0;
                        target = Some(orbit_target(
                            pos,
                            enemy_pos,
                            2500.,
                            4000.,
                            1000.,
                            speed * dt,
                        ));
                    }
                }
            }
        } else if let Some((p, path, n)) = &leaf {
            let effective = p.for_procedure(Some(n));
            let destination = self
                .package_place(actor, &effective)
                .filter(|(p, _)| schedule::same_space(*p, place))
                .map(|(_, p)| p);
            match n.procedure.as_str() {
                "Orbit" if target.is_none() && f.allowed => {
                    let radius = combat::gmst_f32(&self.lo, "fCombatOrbitDistance", 4000.);
                    if let Some(center) = destination {
                        target = Some(orbit_target(
                            pos,
                            center,
                            number(n, 1, radius),
                            number(n, 2, radius),
                            number(n, 3, 1000.),
                            speed * dt,
                        ));
                    }
                }
                "Hover" if target.is_none() && f.allowed => {
                    target = destination.map(|p| p + Vec3::Z * number(n, 1, 1000.));
                    hover = true;
                    if target.is_some_and(|t| t.distance(pos) < 96.) {
                        finish = Some(path.clone());
                    }
                }
                "FlightGrab" if target.is_none() && f.allowed => {
                    let victim = match n.values.first() {
                        Some(Input::Target(t)) => self.package_target_of(actor, p, Some(*t)),
                        _ => None,
                    };
                    target = victim.and_then(|r| self.ref_position(r));
                    if !matches!(n.values.get(1), Some(Input::Bool(true))) && !away {
                        if let Some(to) = target {
                            let d = to - pos;
                            if self
                                .physics
                                .raycast_excluding(pos, d.normalize_or_zero(), d.length(), actor)
                                .is_some_and(|(distance, hit)| {
                                    distance + 16. < d.length() && hit != victim
                                })
                            {
                                if let Some(a) = self.actor_mut(actor) {
                                    a.flight_controlled = true;
                                    a.velocity = Vec3::ZERO;
                                    a.speed = 0.;
                                }
                                return true;
                            }
                        }
                    }
                    if let Some(victim) =
                        victim.filter(|_| target.is_some_and(|t| t.distance(pos) < 256.))
                    {
                        if f.grabbed.is_none() {
                            self.flights.get_mut(&actor).unwrap().grabbed = Some(victim);
                            self.flights.get_mut(&actor).unwrap().elapsed = 0.;
                            self.set_flight_phase(actor, CRUISE, "FlyStartCruise");
                            self.send_actor_graph_event(actor, "to_Flight_Kill_Grab_Action");
                            self.set_vehicle(victim, Some(actor));
                            if let Some(p) = self.passengers.get_mut(&victim) {
                                p.offset = Vec3::new(0., 180., -120.);
                            }
                        } else if f.elapsed >= 1.067
                            && (away || self.actor_ref(actor).is_none_or(|a| a.graph.is_none()))
                        {
                            self.raise_anim_events(actor, &["FlightActionEnd".into()]);
                            self.set_vehicle(victim, None);
                            self.send_actor_graph_event(actor, "to_Flight_Cruising");
                            self.flights.get_mut(&actor).unwrap().grabbed = None;
                            finish = Some(path.clone());
                        }
                        target = Some(pos);
                    }
                }
                "Sit" if target.is_none() => {
                    perch = effective
                        .target
                        .and_then(|t| self.package_target_of(actor, p, Some(t)));
                    target = destination;
                }
                "Travel" | "Patrol" | "Follow" | "Escort" if target.is_none() && f.allowed => {
                    // Patrol uses the same linked points and progress as walkers.
                    if n.procedure == "Patrol" {
                        let start = effective
                            .target
                            .and_then(|t| self.package_target_of(actor, p, Some(t)));
                        let points = start.map(|r| self.patrol_route(r));
                        let index = self
                            .execution(actor)
                            .and_then(|x| x.patrol)
                            .map_or(0, |p| p.1);
                        target = points
                            .as_ref()
                            .and_then(|points| points.get(index))
                            .map(|p| p.pos);
                        if let Some(start) = start {
                            if target.is_none() && !effective.repeat {
                                finish = Some(path.clone());
                                target = Some(pos);
                            } else if target.is_none() {
                                if let Some(x) = self.execution_mut(actor) {
                                    x.patrol = Some((start, 0));
                                }
                                target = points.as_ref().and_then(|p| p.first()).map(|p| p.pos);
                            } else if target
                                .is_some_and(|t| t.distance(pos) < effective.point_radius.max(96.))
                            {
                                if let Some(x) = self.execution_mut(actor) {
                                    x.patrol = Some((start, index + 1));
                                }
                            }
                        }
                    } else {
                        target = destination;
                        if target.is_some_and(|t| t.distance(pos) < 96.) && n.procedure != "Follow"
                        {
                            finish = Some(path.clone());
                        }
                    }
                }
                _ => {}
            }
        }
        let requested_landing = (!f.allowed && f.phase != GROUND)
            || perch.is_some()
            || (combat.is_some() && matches!(f.combat_mode, 2 | 3));
        let mut landing = requested_landing;
        if requested_landing {
            if let Some((_, point)) = f
                .marker
                .and_then(|r| self.current_place_of(r))
                .filter(|(p, _)| schedule::same_space(*p, place))
            {
                target = Some(point);
                perch = f.marker.filter(|&r| self.furniture.get(r).is_some());
            } else if !f.allowed {
                target = f
                    .allow_search
                    .then(|| self.nav.height_at(pos))
                    .flatten()
                    .map(|z| pos.with_z(z));
            }
            if target.is_none() {
                target = Some(pos);
                hover = true;
                landing = false;
            }
        }
        if let Some(victim) = f.grabbed.filter(|_| {
            leaf.as_ref()
                .is_none_or(|(_, _, n)| n.procedure != "FlightGrab")
                || requested_landing
        }) {
            self.set_vehicle(victim, None);
            self.flights.get_mut(&actor).unwrap().grabbed = None;
        }
        if target.is_none() && f.phase == HOVER {
            target = Some(pos);
            hover = true;
        }
        if target.is_none()
            && combat.is_none()
            && let Some(r) = f.perched
        {
            target = self.ref_position(r);
            perch = Some(r);
            landing = true;
        }
        let Some(target) = target.filter(|t| t.is_finite()) else {
            return false;
        };
        let phase = if landing {
            LAND
        } else if hover {
            HOVER
        } else {
            CRUISE
        };
        let already_landed = landing && f.phase == GROUND && target.distance(pos) < 96.;
        let grabbing = self
            .flights
            .get(&actor)
            .is_some_and(|f| f.grabbed.is_some());
        if grabbing {
            // Re-enter the action after attaching/loading a body. Its release
            // notification carries a bone payload and stays internal.
            if self
                .actor_ref(actor)
                .and_then(|a| a.graph.as_ref())
                .is_some_and(|g| {
                    !g.active_states()
                        .iter()
                        .any(|s| s.starts_with("ST_Flight_Kill"))
                })
            {
                self.send_actor_graph_event(actor, "to_Flight_Kill_Grab_Action");
            }
        } else if f.phase == GROUND && (!landing || target.distance(pos) > 256.) {
            self.set_flight_phase(actor, TAKEOFF, "TakeOff");
        } else if f.phase == TAKEOFF && f.elapsed < 1. {
            // Let the takeoff clip leave the ground before cruising.
        } else if !already_landed {
            let perch_event = perch.and_then(|r| self.furniture.get(r)).and_then(|f| {
                f.ways
                    .first()
                    .map(|w| w.enter.clone())
                    .or_else(|| f.events.first().cloned())
            });
            let event = match phase {
                LAND if perch.is_some() => perch_event.as_deref().unwrap_or("FlyStopPerchTower"),
                LAND if f.allow_crash
                    && self.actor_health(actor).is_some_and(|(h, m)| h < m * 0.5) =>
                {
                    "FlyStopInjured"
                }
                LAND => "FlyStopDefault",
                HOVER => "HoverStart",
                _ => "FlyStartCruise",
            };
            self.set_flight_phase(actor, phase, event);
        }
        let delta = target - pos;
        let desired = delta.x.atan2(delta.y);
        let angle = (desired - heading + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        let heading = heading + angle.clamp(-turn * dt, turn * dt);
        let mut next = pos + delta.clamp_length_max(speed * dt);
        if !landing && f.phase == CRUISE && f.elapsed >= 2. && f.elapsed - dt < 2. {
            self.send_actor_graph_event(actor, "Glide");
        }
        if landing
            && next.distance(target) < 96.
            && (already_landed || away || (f.phase == LAND && f.elapsed >= 1.5))
        {
            next = target;
            self.set_flight_phase(
                actor,
                GROUND,
                if perch.is_some() {
                    "PerchLandEnd"
                } else {
                    "LandEnd"
                },
            );
            self.flights.get_mut(&actor).unwrap().perched = perch;
            if let Some((p, path, n)) = &leaf
                && n.procedure == "Sit"
                && p.tree
                    .as_ref()
                    .is_some_and(|t| t.single_procedure().is_none())
            {
                finish = Some(path.clone());
            }
            if let Some(perch) = perch.filter(|_| f.perched != perch) {
                let event = self.furniture.get(perch).and_then(|f| {
                    f.ways
                        .first()
                        .map(|w| w.enter.clone())
                        .or_else(|| f.events.first().cloned())
                });
                if let Some(event) = event {
                    self.send_actor_graph_event(actor, &event);
                }
            }
        } else if hover && next.distance(target) < 96. {
            self.set_flight_phase(actor, HOVER, "FlyStartHover");
        }
        if let Some(x) = self.execution_mut(actor) {
            x.pos = next;
            x.heading = heading;
        }
        if away {
            self.put_away(actor, place, next);
        } else if let Some(a) = self.actor_mut(actor) {
            a.flight_controlled = true;
            a.velocity = if dt > 0. {
                (next - pos) / dt
            } else {
                Vec3::ZERO
            };
            a.speed = a.velocity.length();
            a.lift = 0.;
        }
        if let Some(path) = finish {
            if let Some(x) = self.execution_mut(actor) {
                if let Some(c) = x.run.as_mut().and_then(|r| r.cursor.as_mut()) {
                    c.finish(&path);
                }
                x.next_eval = 0.;
                x.settled = false;
            }
        }
        true
    }
    pub(super) fn flight_graph_events(&mut self, actor: FormId, events: &[String]) {
        if events
            .iter()
            .any(|e| e.eq_ignore_ascii_case("FlightActionRelease"))
            && let Some(victim) = self.flights.get(&actor).and_then(|f| f.grabbed)
        {
            self.set_vehicle(victim, None);
            self.flights.get_mut(&actor).unwrap().grabbed = None;
            if let Some((_, path, _, _)) = self.active_procedure(actor, "FlightGrab")
                && let Some(x) = self.execution_mut(actor)
            {
                if let Some(c) = x.run.as_mut().and_then(|r| r.cursor.as_mut()) {
                    c.finish(&path);
                }
                x.next_eval = 0.;
                x.settled = false;
            }
            self.send_actor_graph_event(actor, "to_Flight_Cruising");
        }
    }
}
