//! Waiting/sleeping catch-up across procedure boundaries without running a body
//! graph or advancing Papyrus real time. Each actor clock is rebased afterwards.
use super::{State, exec::ActorExec, package::Behaviour, schedule::same_space};
use crate::engine::{Engine, PLAYER_REF};
use esp::FormId;

type Signature = Vec<(FormId, Option<FormId>, bool, Vec<Vec<u32>>, u64, usize)>;

fn signature(
    actor: FormId,
    x: &ActorExec,
) -> (FormId, Option<FormId>, bool, Vec<Vec<u32>>, u64, usize) {
    let cursor = x.run.as_ref().and_then(|r| r.cursor.as_ref());
    (
        actor,
        x.package(),
        x.run_completed(),
        cursor.map_or_else(Vec::new, |c| c.active.clone()),
        cursor
            .filter(|c| !c.active.is_empty())
            .map_or(0, |c| c.revision),
        x.run_events.len(),
    )
}
/// Whether `x`'s package, as far as it has got, has nothing timed to do.
fn quiet_package(x: &ActorExec) -> bool {
    let Some(p) = x.current.and_then(|i| x.packages.get(i)) else {
        return true;
    };
    if !matches!(
        p.behaviour,
        Behaviour::Sandbox
            | Behaviour::Travel
            | Behaviour::Hold
            | Behaviour::Sleep
            | Behaviour::Sit
            | Behaviour::Patrol
    ) {
        return false;
    }
    let (Some(tree), Some(cursor)) = (
        p.tree.as_ref(),
        x.run.as_ref().and_then(|r| r.cursor.as_ref()),
    ) else {
        return true;
    };
    cursor.active.iter().all(|path| {
        tree.at(path).is_some_and(|n| {
            matches!(
                n.procedure.as_str(),
                "Travel" | "Sandbox" | "Sit" | "Sleep" | "Patrol"
            )
        })
    })
}
fn player_follower(goal: Option<super::Goal>) -> bool {
    goal.is_some_and(|g| {
        matches!(g.behaviour, Behaviour::Follow | Behaviour::Escort) && g.target == Some(PLAYER_REF)
    })
}

/// Wait procedures active in `x`'s package: the seconds each has left.
fn waits_left(x: &ActorExec) -> impl Iterator<Item = f64> + '_ {
    let tree = x
        .current
        .and_then(|i| x.packages.get(i))
        .and_then(|p| p.tree.as_ref());
    let cursor = x.run.as_ref().and_then(|r| r.cursor.as_ref());
    tree.zip(cursor).into_iter().flat_map(|(tree, cursor)| {
        cursor.active.iter().filter_map(move |path| {
            let n = tree.at(path).filter(|n| n.procedure == "Wait")?;
            Some(n.seconds() - cursor.states.get(path).map_or(0.0, |p| p.elapsed))
        })
    })
}

impl Engine {
    /// `only`: the away actors to look at (none: all of them).
    fn time_signature(&self, only: Option<&[FormId]>) -> Signature {
        let away: Vec<_> = match only {
            Some(ids) => ids
                .iter()
                .filter(|id| !self.actor_cells.contains_key(id))
                .filter_map(|&id| Some(signature(id, self.away.get(&id)?)))
                .collect(),
            None => self
                .away
                .iter()
                .filter(|(id, _)| !self.actor_cells.contains_key(id))
                .map(|(&id, x)| signature(id, x))
                .collect(),
        };
        let mut s: Signature = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| !a.dead)
            .map(|a| signature(a.ref_id, &a.exec))
            .chain(away)
            .collect();
        s.sort_by_key(|s| s.0);
        s
    }

    /// Away actors that time passing in a menu could change before it ends,
    /// sorted. The others are settled in a package that only travels,
    /// sandboxes, sits, sleeps, holds or patrols (an away patroller stands
    /// where it was put), with no timed step (Wait, retries, practice, arrival
    /// or recovery) pending: they are stepped once, at the end, their packages
    /// read then. The clock's hour doesn't change during the catch-up, so
    /// readings between would see the same schedule.
    fn menu_busy(&self) -> Vec<FormId> {
        let quiet = |id: FormId, x: &ActorExec| {
            let cursor = x.run.as_ref().and_then(|r| r.cursor.as_ref());
            x.settled
                && x.ends_at.is_none()
                && x.jump_at.is_none()
                // Only a procedure's destination walks a settled actor on.
                && (x.strode_at.is_none() || cursor.is_none())
                && x.bleeding.is_none()
                && x.run.as_ref().is_none_or(|r| r.practice.is_none())
                && !self.offscreen_routes.contains_key(&id)
                && !self.rides.contains_key(&id)
                && !self.flights.contains_key(&id)
                && !self.passengers.contains_key(&id)
                && quiet_package(x)
        };
        let mut busy: Vec<_> = self
            .away
            .iter()
            .filter(|&(&id, x)| {
                x.place.is_some() && !self.actor_cells.contains_key(&id) && !quiet(id, x)
            })
            .map(|(&id, _)| id)
            .collect();
        busy.sort();
        busy
    }

    fn step_menu_away(&mut self, clock: f64, only: Option<&[FormId]>) {
        let ids = match only {
            Some(ids) => ids.to_vec(),
            None => {
                let mut all: Vec<_> = self
                    .away
                    .iter()
                    .filter(|(_, x)| x.place.is_some())
                    .map(|(&id, _)| id)
                    .collect();
                all.sort();
                all
            }
        };
        for id in ids {
            if self.away.get(&id).is_some_and(|x| x.place.is_some()) {
                self.step_away_at(id, clock, clock);
            }
        }
    }

    /// Complete zero-time transitions at the current boundary, for loaded
    /// actors and the away ones in `only` (none: all). A zero-duration
    /// repeating tree is bounded rather than spinning indefinitely in a menu.
    fn settle_menu_time(&mut self, clock: f64, only: Option<&[FormId]>) {
        for _ in 0..32 {
            let before = self.time_signature(only);
            self.advance_loaded_trees(0.0);
            self.evaluate_packages(0.0);
            self.loaded_procedure_locks();
            self.run_loaded_finds();
            self.menu_paths(0.0, clock);
            self.complete_arrivals();
            self.step_menu_away(clock, only);
            if self.time_signature(only) == before {
                break;
            }
        }
    }

    /// Seconds until the next loaded actor's Wait ends or walk arrives.
    fn next_loaded_menu_event(&self) -> f64 {
        let mut dt = f64::INFINITY;
        let mut consider = |t: f64| {
            if t.is_finite() && t > 1e-6 {
                dt = dt.min(t);
            }
        };
        for a in self.cells.values().flat_map(|rt| &rt.actors).filter(|a| {
            !a.dead && !a.exec.pinned && a.exec.combat.is_none() && a.exec.bleeding.is_none()
        }) {
            waits_left(&a.exec).for_each(&mut consider);
            if let State::Walk { path, next, .. } = &a.state {
                let mut from = a.exec.pos;
                let mut distance = 0.0;
                for &point in path.iter().skip(*next) {
                    distance += from.distance(point) as f64;
                    from = point;
                }
                if a.walk_speed() > 0.0 {
                    consider(distance / a.walk_speed() as f64);
                }
            }
        }
        dt
    }

    /// Seconds from `clock` until away actor `id`'s next timed step: an
    /// arrival, a practice action or a Wait's end (none: nothing pending).
    /// Its Waits and practice count from when it was last stepped.
    fn next_away_menu_event(&self, id: FormId, clock: f64) -> Option<f64> {
        let x = self.away.get(&id)?;
        if self.actor_cells.contains_key(&id) || self.processing(id).is_none() {
            return None;
        }
        let since = clock - x.stepped_at;
        let practice = self
            .away_practice_settings(id)
            .and_then(|_| x.run.as_ref()?.practice.as_ref())
            .map(|p| p.wait as f64 - since);
        [
            x.ends_at.map(|t| t - clock),
            x.jump_at.map(|t| t - clock),
            practice,
        ]
        .into_iter()
        .flatten()
        .chain(waits_left(x).map(|t| t - since))
        .filter(|t| t.is_finite() && *t > 1e-6)
        .min_by(f64::total_cmp)
    }

    /// Plan the next loaded Travel after a Wait, and walk already planned
    /// routes. Furniture interaction and attack graphs do not run in a menu.
    fn menu_paths(&mut self, seconds: f32, clock: f64) {
        self.prepare_mounts(seconds);
        self.update_flights(seconds);
        self.refresh_actor_door_access();
        for rt in self.cells.values_mut() {
            for a in &mut rt.actors {
                if !a.dead
                    && !a.exec.pinned
                    && a.exec.combat.is_none()
                    && a.exec.bleeding.is_none()
                    && a.exec.run.as_ref().is_some_and(|r| r.cursor.is_some())
                    && a.exec.goal.is_some_and(|g| {
                        g.behaviour == Behaviour::Travel
                            && a.exec.pos.truncate().distance(g.centre.truncate())
                                > g.radius.max(96.0)
                    })
                    && a.in_furniture()
                {
                    a.leave_furniture_in_menu(&mut self.furniture, &self.nav);
                }
            }
        }
        let mut paths = Vec::new();
        let mut departures = Vec::new();
        for (&key, rt) in &self.cells {
            let Some(here) = self.place_of_key(key) else {
                continue;
            };
            for a in &rt.actors {
                if a.dead
                    || a.exec.pinned
                    || a.exec.combat.is_some()
                    || a.exec.bleeding.is_some()
                    || a.flight_controlled
                    || a.mount_driver.is_some()
                    || self.passengers.contains_key(&a.ref_id)
                    || player_follower(a.exec.goal)
                {
                    continue;
                }
                let Some(goal) = a.exec.goal else {
                    continue;
                };
                let Some(p) = a.exec.current.and_then(|i| a.exec.packages.get(i)) else {
                    continue;
                };
                if a.exec.run.as_ref().is_none_or(|r| r.cursor.is_none()) {
                    continue;
                }
                let mut target = goal.centre;
                if let Some((there, at)) = self
                    .package_place(a.ref_id, p)
                    .filter(|(there, _)| !same_space(here, *there))
                {
                    if a.exiting.is_none() {
                        if let Some((door, spot)) = self.door_towards(a.ref_id, a.exec.pos, there) {
                            departures.push((a.ref_id, door, spot, there, at));
                            target = spot;
                        } else {
                            continue;
                        }
                    }
                }
                if !a.is_walking()
                    && goal.behaviour == Behaviour::Travel
                    && target.is_finite()
                    && a.exec.pos.truncate().distance(target.truncate()) > goal.radius.max(96.0)
                {
                    if let Some(path) = self
                        .nav
                        .find_path_for(
                            a.exec.pos,
                            target,
                            a.exec.path_policy(),
                            &a.exec.door_access,
                        )
                        .filter(|p| !p.is_empty())
                    {
                        paths.push((a.ref_id, path));
                    }
                }
            }
        }
        for (id, door, spot, there, at) in departures {
            self.whereabouts.put(id, there, at);
            if let Some(a) = self.actor_mut(id) {
                a.exiting = Some(door);
                a.exec.goal = Some(super::Goal::travel(spot));
            }
        }
        for (id, path) in paths {
            if let Some(a) = self.actor_mut(id) {
                let budget =
                    super::path_length(a.exec.pos, &path) / a.walk_speed().max(1.0) * 2.5 + 5.0;
                a.state = State::Walk {
                    path,
                    next: 0,
                    budget,
                    to_seat: false,
                };
            }
        }
        let mut gone = Vec::new();
        for (&key, rt) in &mut self.cells {
            for a in &mut rt.actors {
                if a.dead
                    || a.exec.pinned
                    || a.exec.combat.is_some()
                    || a.exec.bleeding.is_some()
                    || a.flight_controlled
                    || a.mount_driver.is_some()
                    || self.passengers.contains_key(&a.ref_id)
                    || player_follower(a.exec.goal)
                {
                    continue;
                }
                let speed = a.walk_speed();
                a.walk_on(speed * seconds);
                let cursor = a.exec.run.as_ref().is_some_and(|r| r.cursor.is_some());
                if cursor
                    && matches!(&a.state, State::Walk { path, next, to_seat: false, .. } if *next >= path.len())
                {
                    a.state = State::Idle(0.0);
                }
                if a.exiting.is_some()
                    && !a.is_walking()
                    && a.exec
                        .goal
                        .is_some_and(|g| g.centre.truncate().distance(a.exec.pos.truncate()) < 64.0)
                {
                    gone.push((a.ref_id, key));
                }
            }
        }
        for (id, key) in gone {
            let from = self.place_of_key(key);
            let destination = self.whereabouts.of.get(&id).copied();
            self.despawn_actor(id);
            if let (Some(from), Some((there, pos))) = (from, destination) {
                if let Some(x) = self.away.get_mut(&id) {
                    x.place = Some(from);
                    x.stepped_at = clock;
                }
                self.put_away(id, there, pos);
            }
        }
        self.sync_riding_bodies();
        self.sync_passengers();
    }

    pub(crate) fn advance_menu_time(&mut self, seconds: f32) {
        if !seconds.is_finite() || seconds < 0.0 {
            return;
        }
        let base = self.scripts.real_time;
        let seconds = seconds as f64;
        self.settle_menu_time(base, None);
        let cadence = super::exec::AWAY_CADENCE as f64;
        let mut busy = self.menu_busy();
        let mut next_reading = cadence;
        let mut elapsed = 0.0;
        while elapsed < seconds {
            let clock = base + elapsed;
            let remaining = seconds - elapsed;
            // Loaded actors and busy away ones are all stepped every few
            // seconds; between, only those whose Wait, practice or arrival
            // falls due. Quiet ones are stepped once, across the whole span.
            let reading = self
                .cells
                .values()
                .flat_map(|rt| &rt.actors)
                .any(|a| !a.dead)
                || !busy.is_empty();
            let events: Vec<_> = busy
                .iter()
                .filter_map(|&id| Some((id, self.next_away_menu_event(id, clock)?)))
                .collect();
            let mut dt = remaining.min(self.next_loaded_menu_event());
            if reading {
                dt = dt.min(next_reading - elapsed);
            }
            for &(_, t) in &events {
                dt = dt.min(t);
            }
            let dt = dt.max(1e-6).min(remaining);
            // Move the old procedure first. A Wait entered at this boundary
            // starts at zero, rather than receiving the old Travel's seconds.
            self.menu_paths(dt as f32, clock + dt);
            self.complete_arrivals();
            self.advance_loaded_trees(dt as f32);
            self.evaluate_packages(dt as f32);
            elapsed += dt;
            let now = base + elapsed;
            if elapsed >= seconds {
                self.step_menu_away(now, None);
                self.settle_menu_time(now, None);
            } else if elapsed >= next_reading - 1e-6 {
                next_reading += cadence;
                self.step_menu_away(now, Some(&busy));
                self.settle_menu_time(now, Some(&busy));
                busy = self.menu_busy();
            } else {
                let due: Vec<_> = events
                    .iter()
                    .filter(|&&(_, t)| t <= dt + 1e-6)
                    .map(|&(id, _)| id)
                    .collect();
                self.step_menu_away(now, Some(&due));
                self.settle_menu_time(now, Some(&due));
            }
        }
        // Absolute away clocks used the virtual timeline. Papyrus real time
        // stood still, so retain the same remaining delays on its real clock.
        for (&id, x) in &mut self.away {
            if self.actor_cells.contains_key(&id) || x.stepped_at <= base {
                continue;
            }
            let shift = x.stepped_at - base;
            x.stepped_at = base;
            for t in [&mut x.ends_at, &mut x.jump_at, &mut x.strode_at]
                .into_iter()
                .flatten()
            {
                *t -= shift;
            }
        }
    }
}

impl super::ActorRuntime {
    /// A menu jump skips the exit clip as scheduled respawning did, retaining
    /// a carried load while releasing the seat and its ordinary idle objects.
    fn leave_furniture_in_menu(
        &mut self,
        furniture: &mut super::furniture::FurnitureWorld,
        nav: &super::nav::NavWorld,
    ) {
        if let Some(graph) = self.graph.as_mut() {
            graph.send_event("IdleForceDefaultState");
            let instant = self
                .seat
                .as_ref()
                .filter(|s| !s.clips.idle_loops)
                .and_then(|s| s.clips.instant_exit.clone())
                .filter(|e| e != "IdleForceDefaultState");
            if self
                .untaken_offset
                .take()
                .into_iter()
                .chain(instant)
                .any(|event| graph.send_event(&event))
            {
                self.carrying = !self.objects.is_empty();
            }
        }
        if let Some(seat) = &self.seat {
            let (start, _) = seat.enter_start();
            self.exec.pos = start.with_z(self.exec.pos.z);
            if let Some(z) = nav.height_at(self.exec.pos) {
                self.exec.pos.z = z;
            }
        }
        self.give_up_seat(furniture);
        self.leave = false;
        if !self.carrying {
            self.objects_changed |= !self.objects.is_empty();
            self.objects.clear();
        }
        self.state = State::Idle(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        exec::tests::{ACTOR, CELL, MARKER, PACKAGE, fixture, loaded_actor},
        package::{Input, Location, LocationKind, Node, Package},
        procedure::Cursor,
        schedule::Place,
    };
    use super::*;
    use glam::Vec3;
    use std::sync::Arc;

    fn leaf(name: &str, values: Vec<Input>) -> Node {
        Node {
            kind: "Procedure".into(),
            flags: 0,
            conditions: vec![],
            children: vec![],
            procedure: name.into(),
            args: vec![],
            values,
        }
    }
    fn wait(t: f32) -> Node {
        leaf("Wait", vec![Input::Float(t)])
    }
    fn package(children: Vec<Node>, repeat: bool) -> Package {
        let tree = Node {
            kind: "Sequence".into(),
            children,
            flags: u32::from(repeat),
            ..leaf("", vec![])
        };
        let mut p = Package::bare(PACKAGE, Behaviour::Travel);
        p.schedule.hour = -1;
        p.schedule.day_of_week = -1;
        p.tree = Some(Arc::new(tree));
        p
    }
    fn loaded(e: &mut Engine, p: Package) {
        e.location = crate::engine::Location::Interior(CELL);
        loaded_actor(e);
        let generation = e.scripts.alias_gen;
        let a = e.actor_mut(ACTOR).unwrap();
        a.exec.alias_gen = generation;
        a.exec.packages = vec![p];
        a.exec.current = None;
        a.exec.run = None;
        a.exec.goal = None;
        a.exec.next_eval = 0.0;
        a.exec.pos = Vec3::ZERO;
        a.fresh = false;
    }
    fn away(e: &mut Engine, p: Package) {
        let mut cursor = Cursor {
            tree_key: p.editor_id.clone(),
            ..Default::default()
        };
        cursor.step(p.tree.as_ref().unwrap(), 0.0, true, |_| true, || 1);
        let mut x = ActorExec::new(vec![p], Vec3::ZERO, 0.0, 0.0);
        x.place = Some(Place::Interior(CELL));
        x.current = Some(0);
        x.alias_gen = e.scripts.alias_gen;
        x.begin_run(Some(0), 0.0, false);
        x.run.as_mut().unwrap().cursor = Some(cursor);
        e.away.insert(ACTOR, x);
    }
    fn plane(e: &mut Engine) {
        e.nav.insert_fixture(
            FormId(0xFF000800),
            esp::navmesh::NavMesh {
                version: 12,
                world: FormId::NULL,
                cell: CELL,
                grid: (0, 0),
                vertices: vec![
                    [-10000., -10000., 0.],
                    [10000., -10000., 0.],
                    [0., 10000., 0.],
                ],
                triangles: vec![esp::navmesh::NavTriangle {
                    vertices: [0, 1, 2],
                    edges: [-1; 3],
                    flags: 0,
                    cover: 0,
                }],
                edge_links: vec![],
                doors: vec![],
            },
        );
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn a_large_loaded_wait_consumes_successive_waits_and_repeats() {
        let (_f, mut e) = fixture();
        loaded(&mut e, package(vec![wait(2.), wait(3.)], true));
        e.pass_time(12.5);
        let c = e
            .actor_ref(ACTOR)
            .unwrap()
            .exec
            .run
            .as_ref()
            .unwrap()
            .cursor
            .as_ref()
            .unwrap();
        assert_eq!(c.active, vec![vec![1]]);
        assert!((c.states[&vec![1]].elapsed - 0.5).abs() < 1e-5);
        assert_eq!(e.scripts.real_time, 0.0);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn wait_stop_movement_controls_an_existing_walk() {
        for stop in [false, true] {
            let (_f, mut e) = fixture();
            let n = leaf("Wait", vec![Input::Float(5.), Input::Bool(stop)]);
            loaded(&mut e, package(vec![n], false));
            let a = e.actor_mut(ACTOR).unwrap();
            a.state = State::Walk {
                path: vec![Vec3::X * 1000.],
                next: 0,
                budget: 20.,
                to_seat: false,
            };
            e.pass_time(1.);
            let a = e.actor_ref(ACTOR).unwrap();
            assert_eq!(a.exec.pos.x > 0., !stop);
            assert_eq!(a.is_walking(), !stop);
        }
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn a_large_away_wait_consumes_successive_waits_without_advancing_papyrus_time() {
        let (_f, mut e) = fixture();
        away(&mut e, package(vec![wait(2.), wait(3.)], true));
        e.pass_time(12.5);
        let c = e.away[&ACTOR]
            .run
            .as_ref()
            .unwrap()
            .cursor
            .as_ref()
            .unwrap();
        assert_eq!(c.active, vec![vec![1]]);
        assert!((c.states[&vec![1]].elapsed - 0.5).abs() < 1e-5);
        assert_eq!(e.away[&ACTOR].stepped_at, e.scripts.real_time);
        e.scripts.real_time += 0.5;
        e.step_away(ACTOR);
        assert!(
            (e.away[&ACTOR]
                .run
                .as_ref()
                .unwrap()
                .cursor
                .as_ref()
                .unwrap()
                .states[&vec![1]]
                .elapsed
                - 1.0)
                .abs()
                < 1e-5
        );
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn a_settled_away_sandboxer_is_quiet_and_a_waiting_one_busy_until_its_wait_ends() {
        let (_f, mut e) = fixture();
        away(&mut e, package(vec![leaf("Sandbox", vec![])], false));
        e.advance_menu_time(0.0);
        assert!(e.away[&ACTOR].settled);
        assert!(!e.menu_busy().contains(&ACTOR));
        e.pass_time(600.0);
        // Stepped at the end: its packages were read, on the rebased clock.
        assert_eq!(e.away[&ACTOR].stepped_at, e.scripts.real_time);

        let (_f, mut e) = fixture();
        away(
            &mut e,
            package(vec![wait(100.), leaf("Sandbox", vec![])], false),
        );
        e.advance_menu_time(0.0);
        assert!(e.menu_busy().contains(&ACTOR));
        e.pass_time(150.0);
        let c = e.away[&ACTOR]
            .run
            .as_ref()
            .unwrap()
            .cursor
            .as_ref()
            .unwrap();
        assert_eq!(c.active, vec![vec![1]]);
        assert!(!e.menu_busy().contains(&ACTOR));
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn unused_seconds_after_wait_are_spent_walking_the_next_procedure() {
        let (_f, mut e) = fixture();
        plane(&mut e);
        let travel = leaf(
            "Travel",
            vec![Input::Location(Location {
                kind: LocationKind::NearReference(MARKER),
                radius: 0.0,
            })],
        );
        loaded(&mut e, package(vec![wait(2.), travel, wait(5.)], false));
        let speed = e.actor_ref(ACTOR).unwrap().walk_speed();
        e.pass_time(2.0 + 1500.0 / speed);
        assert!((e.actor_ref(ACTOR).unwrap().exec.pos.x - 1500.).abs() < 0.01);
        e.pass_time(1500.0 / speed + 2.0);
        let c = e
            .actor_ref(ACTOR)
            .unwrap()
            .exec
            .run
            .as_ref()
            .unwrap()
            .cursor
            .as_ref()
            .unwrap();
        assert_eq!(c.active, vec![vec![2]]);
        assert!((c.states[&vec![2]].elapsed - 2.0).abs() < 0.01);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn an_away_whole_cell_travel_finishes_then_uses_remaining_menu_seconds() {
        let (_f, mut e) = fixture();
        let travel = leaf(
            "Travel",
            vec![Input::Location(Location {
                kind: LocationKind::InCell(CELL),
                radius: 0.0,
            })],
        );
        away(&mut e, package(vec![travel, wait(5.)], false));
        e.pass_time(4.0);
        let x = &e.away[&ACTOR];
        let c = x.run.as_ref().unwrap().cursor.as_ref().unwrap();
        assert_eq!(c.active, vec![vec![1]]);
        assert!((c.states[&vec![1]].elapsed - 1.5).abs() < 1e-5);
        assert!(x.ends_at.is_none());
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn loaded_whole_cell_travel_crosses_a_door_without_restarting_its_run() {
        use esp::writer::{Node as RecordNode, RecordBuilder, write_plugin};
        let (_f, mut e) = fixture();
        let other = FormId(0x830);
        let door = FormId(0x831);
        let partner = FormId(0x832);
        let mut teleport = [0; 28];
        teleport[..4].copy_from_slice(&door.0.to_le_bytes());
        let cells = [(CELL, door, partner), (other, partner, door)].map(|(cell, r, to)| {
            let mut xtel = teleport;
            xtel[..4].copy_from_slice(&to.0.to_le_bytes());
            RecordNode::Group {
                label: cell.0.to_le_bytes(),
                group_type: esp::group_type::CELL_CHILDREN,
                children: vec![RecordNode::Group {
                    label: cell.0.to_le_bytes(),
                    group_type: esp::group_type::CELL_TEMPORARY,
                    children: vec![RecordNode::Record(
                        RecordBuilder::new(b"REFR", r).field(b"XTEL", &xtel),
                    )],
                }],
            }
        });
        let dir = std::env::temp_dir().join(format!("vrm-menu-door-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let records = vec![
            RecordNode::top(
                b"NPC_",
                vec![RecordBuilder::new(b"NPC_", super::super::exec::tests::BASE)],
            ),
            RecordNode::top(
                b"ACHR",
                vec![
                    RecordBuilder::new(b"ACHR", ACTOR)
                        .form(b"NAME", super::super::exec::tests::BASE),
                ],
            ),
            RecordNode::Group {
                label: *b"CELL",
                group_type: 0,
                children: vec![
                    RecordNode::Record(RecordBuilder::new(b"CELL", CELL).field(b"DATA", &[1, 0])),
                    RecordNode::Record(RecordBuilder::new(b"CELL", other).field(b"DATA", &[1, 0])),
                    cells[0].clone(),
                    cells[1].clone(),
                ],
            },
        ];
        std::fs::write(
            dir.join("Doors.esp"),
            write_plugin(0, &[], "test", "menu door transition", 0x900, &records),
        )
        .unwrap();
        e.lo = esp::LoadOrder::load(&dir, &["Doors.esp".into()]).unwrap();
        let travel = leaf(
            "Travel",
            vec![Input::Location(Location {
                kind: LocationKind::InCell(other),
                radius: 0.0,
            })],
        );
        loaded(&mut e, package(vec![travel, wait(5.)], false));
        let key = crate::render::CellKey::Interior(CELL);
        e.cells
            .get_mut(&key)
            .unwrap()
            .doors
            .push(crate::world::cell::Door {
                ref_id: door,
                position: Vec3::ZERO,
                destination: Some((partner, Vec3::ZERO, Vec3::ZERO)),
            });
        e.scene
            .cells
            .entry(key)
            .or_default()
            .actors
            .push(crate::render::ActorInstance {
                meshes: vec![],
                attachments: vec![],
                equipment: vec![],
                held_light: None,
                transform: glam::Mat4::IDENTITY,
                pose: vec![],
                lights: [0; 8],
                radius: 64.,
                shadow_only: false,
                skin_tone: None,
                weight: 0.,
                alpha: 1.0,
            });
        e.pass_time(4.0);
        assert!(!e.actor_cells.contains_key(&ACTOR));
        let x = &e.away[&ACTOR];
        assert_eq!(x.place, Some(Place::Interior(other)));
        let c = x.run.as_ref().unwrap().cursor.as_ref().unwrap();
        assert_eq!(c.active, vec![vec![1]]);
        assert!((c.states[&vec![1]].elapsed - 1.5).abs() < 1e-5);
        assert_eq!(
            x.run_events
                .iter()
                .filter(|e| matches!(e, super::super::exec::RunEvent::Start(_)))
                .count(),
            1
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
