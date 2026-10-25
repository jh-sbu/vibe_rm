//! Actors away: advancing the executions of actors that unloaded
//! (`plans/ai-actor-execution.md` step 5). Their packages are read again on
//! a cadence and as each half hour of game time begins, their runs start,
//! change and end with the usual events, and their packages move them by the
//! rules `test-mod autotest13` and `autotest14` observed; the choices are in
//! `known_ambiguities/actor-execution.md` ("Away").

use esp::FormId;
use glam::Vec3;

use super::package::{Behaviour, Package};
use super::schedule::{Place, same_space, unloaded_arrival};
use crate::engine::{Engine, PLAYER_REF};

/// Away actors stepped per frame, in turn.
const BUDGET: usize = 100;
/// Real seconds between readings of an away actor's packages (autotest13:
/// within about 3 s of a condition changing).
const EVAL_INTERVAL: f32 = 3.0;
/// How far off a Travel package's target in the same space can be for an
/// actor away to be put at it: every one moved had it within 4,100 (and 830
/// of height), every one left 4,950 or more (autotest13, autotest14).
pub(crate) const JUMP_RANGE: f32 = 4500.0;
/// Real seconds from a Travel run's start to its end, for an actor away put
/// at its target (autotest14: `OnPackageEnd` 2.5 s after `OnPackageStart`).
const END_DELAY: f64 = 2.5;
/// Units a second an actor walks in a cell the player has just left
/// (autotest13: 78 to 87).
const BUFFERED_WALK_SPEED: f32 = 80.0;
/// How far beyond a package's radius (at least 96) an actor away from a
/// non-Travel package's place must be, by this factor, to be put there: as
/// waiting has them start over (`jump_to_schedules`).
const PLACE_SLACK: f32 = 1.5;
/// How near its target a follower away keeps (beyond the package's follow
/// distance).
const FOLLOW_SLACK: f32 = 256.0;

/// Which away actors to step next, and the cells the player last left.
#[derive(Default)]
pub struct Stepper {
    queue: Vec<FormId>,
    cursor: usize,
    /// The places unloaded as the player last changed location: their actors
    /// walk on at loaded speed (autotest13's buffered cells) until the player
    /// moves on again.
    pub(crate) buffered: Vec<Place>,
}

/// What an actor's package does with it away, this step.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Move {
    /// Nothing more until its package changes.
    Settle,
    /// Put at `pos` in `place` (NaN: anywhere in it); `ends` its run
    /// `END_DELAY` later.
    Put {
        place: Place,
        pos: Vec3,
        target: Vec3,
        ends: bool,
    },
    /// Walking toward `end` in the cell it is in.
    Walk { end: Vec3 },
    /// Keeping up with whom it follows.
    Follow { place: Place, pos: Vec3 },
}

/// Where a package has an actor away at `pos` in `place` go, its package's
/// target `target` in `there`: Travel is put on its target's line 40 short
/// when that's in range in the same space (in a buffered cell it walks
/// there), onto it in another space, and left where it is beyond; other
/// packages are put at their place when away from it.
fn plan_move(
    p: &Package,
    place: Place,
    pos: Vec3,
    there: Place,
    target: Vec3,
    buffered: bool,
) -> Move {
    let radius = p.location.map_or(0.0, |l| l.radius);
    let end = unloaded_arrival(p.behaviour, radius, (place, pos), there, target);
    if p.behaviour == Behaviour::Travel {
        return if !same_space(place, there) {
            Move::Put {
                place: there,
                pos: end,
                target,
                ends: true,
            }
        } else if target.is_nan() || pos.is_nan() {
            Move::Settle
        } else if buffered {
            Move::Walk { end }
        } else if pos.distance(target) <= JUMP_RANGE {
            Move::Put {
                place: there,
                pos: end,
                target,
                ends: true,
            }
        } else {
            Move::Settle
        };
    }
    let off = !same_space(place, there)
        || (target.is_finite()
            && !(pos.truncate().distance(target.truncate()) <= radius.max(96.0) * PLACE_SLACK));
    if off {
        Move::Put {
            place: there,
            pos: end,
            target,
            ends: false,
        }
    } else {
        Move::Settle
    }
}

/// The place `pos` is in, in `like`'s space: an exterior's cell by position.
fn place_at(like: Place, pos: Vec3) -> Place {
    match like {
        Place::Exterior(w, _) if pos.is_finite() => {
            Place::Exterior(w, crate::engine::grid_of(pos.truncate()))
        }
        _ => like,
    }
}

impl Engine {
    /// Step the next few away actors.
    pub(crate) fn update_away(&mut self) {
        if !self.ai_enabled {
            return;
        }
        let s = &mut self.away_steps;
        if s.cursor >= s.queue.len() {
            s.queue = self
                .away
                .iter()
                .filter(|(_, x)| x.place.is_some())
                .map(|(&r, _)| r)
                .collect();
            s.queue.sort();
            s.cursor = 0;
        }
        let end = (s.cursor + BUDGET).min(s.queue.len());
        let batch = s.queue[s.cursor..end].to_vec();
        s.cursor = end;
        for r in batch {
            self.step_away(r);
        }
    }

    /// Step every away actor at once (time passed in a menu).
    pub(crate) fn step_all_away(&mut self) {
        let mut all: Vec<FormId> = self
            .away
            .iter()
            .filter(|(_, x)| x.place.is_some())
            .map(|(&r, _)| r)
            .collect();
        all.sort();
        for r in all {
            self.step_away(r);
        }
    }

    /// Advance actor `r`'s execution away: read its packages when due, move it
    /// as its package has it, and end a run that has had its time.
    pub(crate) fn step_away(&mut self, r: FormId) {
        let now = self.scripts.real_time;
        if self.actor_cells.contains_key(&r) || self.is_disabled(r) {
            return;
        }
        let Some(x) = self.away.get(&r).filter(|x| x.place.is_some()) else {
            return;
        };
        let dt = (now - x.stepped_at).max(0.0) as f32;
        self.refresh_away_packages(r);
        let slot = (self.game_hours_total() * 2.0).floor() as i64;
        let Some(x) = self.away.get_mut(&r) else {
            return;
        };
        x.stepped_at = now;
        x.next_eval -= dt;
        if x.next_eval <= 0.0 || x.eval_slot != Some(slot) {
            self.evaluate_away(r, slot);
        }
        self.advance_away(r, dt);
        if let Some(x) = self.away.get_mut(&r)
            && x.ends_at.is_some_and(|t| t <= now)
        {
            x.ends_at = None;
            x.complete_run(false);
        }
    }

    /// An away actor whose aliases changed takes up its new package stack.
    fn refresh_away_packages(&mut self, r: FormId) {
        let generation = self.scripts.alias_gen;
        let Some(npc) = self
            .away
            .get(&r)
            .filter(|x| x.alias_gen != generation && !x.npc.is_null())
            .map(|x| x.npc)
        else {
            return;
        };
        let packages = self.actor_packages(r, npc);
        let Some(x) = self.away.get_mut(&r) else {
            return;
        };
        x.alias_gen = generation;
        if x.packages
            .iter()
            .map(|p| p.id)
            .eq(packages.iter().map(|p| p.id))
        {
            return;
        }
        x.current = x
            .package()
            .and_then(|id| packages.iter().position(|p| p.id == id));
        x.packages = packages;
        x.next_eval = 0.0;
    }

    /// Read away actor `r`'s packages (in half hour `slot`), as loaded actors'
    /// are (`evaluate_packages`), and start the one that applies.
    fn evaluate_away(&mut self, r: FormId, slot: i64) {
        let Some(x) = self.away.get(&r) else {
            return;
        };
        let applies = |p: &Package| {
            let ctx = crate::condition::Context {
                subject: Some(r),
                quest: p.quest,
                ..Default::default()
            };
            p.schedule.matches(self.hour, self.day)
                && crate::condition::evaluate(self, &p.conditions, ctx)
        };
        let finished = x
            .done_once
            .filter(|&id| x.packages.iter().any(|p| p.id == id && applies(p)));
        let forget = finished.is_none() && x.done_once.is_some();
        let pick = (!x.pinned)
            .then(|| {
                x.packages
                    .iter()
                    .position(|p| applies(p) && Some(p.id) != finished)
            })
            .flatten();
        let id = pick.map(|i| x.packages[i].id);
        let for_scene = self
            .scenes
            .packages
            .get(&r)
            .is_some_and(|(_, _, ps)| ps.iter().any(|p| Some(p.id) == id));
        let lock_old = match self.whereabouts.lock_package.get(&r) {
            Some(&old) => old,
            None => x.package(),
        };
        let pinned = x.pinned;
        let now = self.game_hours_total();
        let Some(x) = self.away.get_mut(&r) else {
            return;
        };
        x.next_eval = EVAL_INTERVAL;
        x.eval_slot = Some(slot);
        if forget {
            x.done_once = None;
        }
        if pinned {
            return;
        }
        if id != x.package() {
            log::debug!("{r} away -> package {id:?}");
            x.begin_run(pick, now, for_scene);
            x.current = pick;
            x.goal = None;
            x.stop = Default::default();
            x.patrol = None;
            // Whatever it was doing is over; its new package takes it on.
            x.stage = None;
            x.settled = false;
            x.ends_at = None;
            x.on_arrival = None;
        } else {
            x.current = pick;
        }
        if lock_old != id {
            self.package_door_locks(vec![(r, lock_old, id)]);
        }
    }

    /// Move away actor `r` as its package has it, `dt` real seconds on.
    fn advance_away(&mut self, r: FormId, dt: f32) {
        let Some(x) = self.away.get(&r) else {
            return;
        };
        let (Some(place), pos) = (x.place, x.pos) else {
            return;
        };
        let Some(p) = x
            .current
            .and_then(|i| x.packages.get(i))
            .filter(|p| Some(p.id) == x.package())
        else {
            if let Some(x) = self.away.get_mut(&r) {
                x.settled = true;
            }
            return;
        };
        let step = if matches!(p.behaviour, Behaviour::Follow | Behaviour::Escort) {
            // Keeping up with whom it follows, when they are away too.
            let near = p.follow_radius.1.max(0.0) + FOLLOW_SLACK;
            self.package_target_of(r, p, p.target)
                .filter(|&t| t != PLAYER_REF && !self.actor_cells.contains_key(&t))
                .and_then(|t| self.away_position(t))
                .filter(|&(tp, tpos)| !same_space(tp, place) || !(tpos.distance(pos) <= near))
                .map_or(Move::Settle, |(place, pos)| Move::Follow { place, pos })
        } else if x.settled {
            return;
        } else {
            match self.package_place(r, p) {
                Some((there, target)) => plan_move(
                    p,
                    place,
                    pos,
                    there,
                    target,
                    self.away_steps.buffered.contains(&place),
                ),
                None => Move::Settle,
            }
        };
        let heading = match step {
            Move::Put { target, .. } if pos.is_finite() => {
                self.package_placement_heading(r, p, pos, target)
            }
            _ => None,
        };
        let now = self.scripts.real_time;
        match step {
            Move::Settle => {
                if let Some(x) = self.away.get_mut(&r) {
                    x.settled = true;
                }
            }
            Move::Put {
                place: there,
                pos: to,
                ends,
                ..
            } => {
                if let Some(x) = self.away.get_mut(&r) {
                    x.settled = true;
                    x.ends_at = ends.then_some(now + END_DELAY);
                    x.on_arrival = heading;
                }
                if !(to == pos && there == place) {
                    self.put_away(r, there, to);
                }
            }
            Move::Follow {
                place: there,
                pos: to,
            } => self.put_away(r, there, to),
            Move::Walk { end } => {
                let left = end - pos;
                let stride = BUFFERED_WALK_SPEED * dt;
                let arrived = left.length() <= stride;
                let to = if arrived {
                    end
                } else {
                    pos + left.normalize_or_zero() * stride
                };
                if to != pos {
                    self.put_away(r, place, to);
                }
                if arrived && let Some(x) = self.away.get_mut(&r) {
                    // As a loaded traveller's run ends as it arrives.
                    x.settled = true;
                    x.complete_run(false);
                }
            }
        }
    }

    /// Where an actor not loaded is: its execution's place, else where its
    /// schedule has it.
    fn away_position(&self, r: FormId) -> Option<(Place, Vec3)> {
        match self.away.get(&r).and_then(|x| Some((x.place?, x.pos))) {
            Some(at) => Some(at),
            None => self.whereabouts.of.get(&r).copied(),
        }
    }

    /// Put away actor `r` at `pos` in `there` (NaN: anywhere in it), facing
    /// the way it went. Its scripts hear of a change of location, and one
    /// put in a loaded cell comes in.
    fn put_away(&mut self, r: FormId, there: Place, pos: Vec3) {
        let Some(x) = self.away.get_mut(&r) else {
            return;
        };
        let Some(from) = x.place else {
            return;
        };
        let to = place_at(there, pos);
        let way = (pos - x.pos).truncate();
        if way.is_finite() && way.length() > 1.0 {
            x.heading = way.x.atan2(way.y);
        }
        (x.place, x.pos) = (Some(to), pos);
        let heading = x.on_arrival;
        self.whereabouts.put(r, to, pos);
        match heading {
            Some(h) => self.whereabouts.placement_heading.insert(r, h),
            None => self.whereabouts.placement_heading.remove(&r),
        };
        let (old, new) = (self.place_location(from), self.place_location(to));
        if old != new {
            log::debug!("{r} away: location {old:?} -> {new:?}");
            let args = vec![
                self.object_value(old.unwrap_or(FormId::NULL)),
                self.object_value(new.unwrap_or(FormId::NULL)),
            ];
            self.send_script_event(r, "OnLocationChange", args);
        }
        if let Some(key) = self.key_of_place(to) {
            self.bring_in(key, to, vec![(r, (from != to).then_some(from))]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CELL: FormId = FormId(0x830);
    const WORLD: FormId = FormId(0x831);

    fn travel(radius: f32) -> Package {
        let mut p = Package::bare(FormId(0x810), Behaviour::Travel);
        p.location = Some(super::super::package::Location {
            kind: super::super::package::LocationKind::NearReference(FormId(0x811)),
            radius,
        });
        p
    }

    fn outside(x: f32, y: f32) -> (Place, Vec3) {
        let pos = Vec3::new(x, y, 0.0);
        (
            Place::Exterior(WORLD, crate::engine::grid_of(pos.truncate())),
            pos,
        )
    }

    #[test]
    fn travel_in_range_is_put_40_short_and_ends() {
        let (place, pos) = outside(0.0, 0.0);
        let (there, target) = outside(3000.0, 0.0);
        let m = plan_move(&travel(0.0), place, pos, there, target, false);
        assert_eq!(
            m,
            Move::Put {
                place: there,
                pos: Vec3::new(2960.0, 0.0, 0.0),
                target,
                ends: true
            }
        );
    }

    #[test]
    fn travel_out_of_range_stays_where_it_is() {
        let (place, pos) = outside(0.0, 0.0);
        let (there, target) = outside(5000.0, 0.0);
        assert_eq!(
            plan_move(&travel(0.0), place, pos, there, target, false),
            Move::Settle
        );
        // Far below counts too (autotest14's FerryEast: 7,600 off, 2,650 below).
        let below = target - Vec3::new(1000.0, 0.0, 4000.0);
        assert_eq!(
            plan_move(&travel(0.0), place, pos, there, below, false),
            Move::Settle
        );
    }

    #[test]
    fn travel_into_another_space_is_put_onto_its_target() {
        let (place, pos) = outside(0.0, 0.0);
        let inside = Place::Interior(CELL);
        let target = Vec3::new(10.0, 20.0, 30.0);
        // However far: the doors' actors were moved whatever the distance.
        let m = plan_move(&travel(0.0), place, pos, inside, target, false);
        assert_eq!(
            m,
            Move::Put {
                place: inside,
                pos: target,
                target,
                ends: true
            }
        );
    }

    #[test]
    fn travel_in_a_buffered_cell_walks() {
        let (place, pos) = outside(0.0, 0.0);
        let (there, target) = outside(6000.0, 0.0);
        let m = plan_move(&travel(0.0), place, pos, there, target, true);
        assert_eq!(
            m,
            Move::Walk {
                end: Vec3::new(5960.0, 0.0, 0.0)
            }
        );
    }

    #[test]
    fn other_packages_are_put_at_their_place_only_when_away_from_it() {
        let mut sandbox = travel(200.0);
        sandbox.behaviour = Behaviour::Sandbox;
        let (place, pos) = outside(0.0, 0.0);
        let (there, target) = outside(250.0, 0.0);
        assert_eq!(
            plan_move(&sandbox, place, pos, there, target, false),
            Move::Settle
        );
        // Out of range for Travel, but other packages go (as schedules had
        // them).
        let (there, target) = outside(9000.0, 0.0);
        assert_eq!(
            plan_move(&sandbox, place, pos, there, target, false),
            Move::Put {
                place: there,
                pos: target,
                target,
                ends: false
            }
        );
        // A whole cell elsewhere: anywhere in it.
        let inside = Place::Interior(CELL);
        let Move::Put { place, pos, .. } =
            plan_move(&sandbox, place, pos, inside, Vec3::NAN, false)
        else {
            panic!("not put");
        };
        assert_eq!(place, inside);
        assert!(pos.is_nan());
    }

    #[test]
    fn an_exterior_place_follows_the_position() {
        let (place, _) = outside(0.0, 0.0);
        let to = place_at(place, Vec3::new(5000.0, -100.0, 0.0));
        assert_eq!(to, Place::Exterior(WORLD, (1, -1)));
        let inside = Place::Interior(CELL);
        assert_eq!(place_at(inside, Vec3::new(5000.0, 0.0, 0.0)), inside);
        assert_eq!(place_at(place, Vec3::NAN), place);
    }

    use crate::ai::exec::tests::{
        ACTOR, CELL as ROOM, MARKER, PACKAGE as HOME, PACKAGE2 as FAR, fixture, loaded_actor,
    };

    /// Applying all day, or from noon to midnight.
    fn package(id: FormId, behaviour: Behaviour, noon_on: bool) -> Package {
        let mut p = Package::bare(id, behaviour);
        p.schedule.day_of_week = -1;
        p.schedule.hour = if noon_on { 12 } else { -1 };
        p.schedule.duration = 12 * 60;
        p
    }

    /// The fixture's actor, standing at `x` in its room on its home package
    /// (the morning), with a Travel package to the marker (3,000 east) from
    /// noon, then unloaded. `buffered`, its room stays in the buffer.
    fn away_at(e: &mut Engine, x: f32, buffered: bool) {
        e.hour = 8.0;
        loaded_actor(e);
        let now = e.game_hours_total();
        let a = e.actor_mut(ACTOR).unwrap();
        a.fresh = false;
        a.exec.pos = Vec3::new(x, 0.0, 0.0);
        let mut far = travel(0.0);
        far.id = FAR;
        far.schedule = package(FAR, Behaviour::Travel, true).schedule;
        far.location = Some(super::super::package::Location {
            kind: super::super::package::LocationKind::NearReference(MARKER),
            radius: 0.0,
        });
        a.exec.packages = vec![far, package(HOME, Behaviour::Sandbox, false)];
        a.exec.current = Some(1);
        a.exec.begin_run(Some(1), now, false);
        a.exec.run_events.clear();
        e.unload_all();
        if !buffered {
            e.away_steps.buffered.clear();
        }
        e.step_away(ACTOR);
    }

    /// The package events its scripts have heard, in order.
    fn heard(e: &mut Engine) -> Vec<(String, u32)> {
        e.report_package_runs();
        let out = e
            .scripts
            .pending_events
            .iter()
            .filter(|(o, _, _)| *o == papyrus::ObjectId::Form(ACTOR.0))
            .filter_map(|(_, ev, args)| match &args[..] {
                [papyrus::Value::Object(papyrus::ObjectId::Form(p), _)] => Some((ev.clone(), *p)),
                _ => None,
            })
            .collect();
        e.scripts.pending_events.clear();
        out
    }

    fn ev(name: &str, p: FormId) -> (String, u32) {
        (name.to_owned(), p.0)
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn an_actor_away_set_off_in_range_is_put_40_short_and_its_run_ends_after() {
        let (_f, mut e) = fixture();
        away_at(&mut e, 0.0, false);
        assert!(heard(&mut e).is_empty());
        assert_eq!(e.running_package(ACTOR), Some(HOME));
        // Noon: its packages are read as the half hour begins.
        e.hour = 12.1;
        e.step_away(ACTOR);
        assert_eq!(
            heard(&mut e),
            [ev("OnPackageChange", HOME), ev("OnPackageStart", FAR)]
        );
        let at = (Place::Interior(ROOM), Vec3::new(2960.0, 0.0, 0.0));
        assert_eq!(
            (e.away[&ACTOR].place, e.away[&ACTOR].pos),
            (Some(at.0), at.1)
        );
        assert_eq!(e.whereabouts.of.get(&ACTOR), Some(&at));
        assert_eq!(e.away[&ACTOR].stage, None);
        // Its run ends 2.5 s on.
        e.scripts.real_time += 2.0;
        e.step_away(ACTOR);
        assert!(heard(&mut e).is_empty());
        e.scripts.real_time += 1.0;
        e.step_away(ACTOR);
        assert_eq!(heard(&mut e), [ev("OnPackageEnd", FAR)]);
        assert_eq!(e.running_package(ACTOR), Some(FAR));
        // Refreshes keep it where its package put it.
        e.refresh_whereabouts();
        assert_eq!(e.whereabouts.of.get(&ACTOR), Some(&at));
        assert_eq!(e.whereabouts.package.get(&ACTOR), Some(&Some(FAR)));
        // The evening: home again, where it is.
        e.hour = 0.2;
        e.day += 1;
        e.step_away(ACTOR);
        assert_eq!(
            heard(&mut e),
            [ev("OnPackageChange", FAR), ev("OnPackageStart", HOME)]
        );
        assert_eq!(e.away[&ACTOR].pos, at.1);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn an_actor_away_with_its_target_out_of_range_stays_put() {
        let (_f, mut e) = fixture();
        away_at(&mut e, -3000.0, false);
        e.hour = 12.1;
        e.step_away(ACTOR);
        e.scripts.real_time += 10.0;
        e.step_away(ACTOR);
        assert_eq!(
            heard(&mut e),
            [ev("OnPackageChange", HOME), ev("OnPackageStart", FAR)]
        );
        assert_eq!(e.away[&ACTOR].pos, Vec3::new(-3000.0, 0.0, 0.0));
        assert!(!e.away[&ACTOR].run_completed());
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn an_actor_in_a_cell_the_player_just_left_walks_on() {
        let (_f, mut e) = fixture();
        away_at(&mut e, -3000.0, true);
        e.hour = 12.1;
        e.step_away(ACTOR);
        e.scripts.real_time += 10.0;
        e.step_away(ACTOR);
        assert_eq!(e.away[&ACTOR].pos, Vec3::new(-2200.0, 0.0, 0.0));
        assert!(!e.away[&ACTOR].run_completed());
        heard(&mut e);
        // It arrives 40 short, and its run ends then.
        e.scripts.real_time += 100.0;
        e.step_away(ACTOR);
        assert_eq!(e.away[&ACTOR].pos, Vec3::new(2960.0, 0.0, 0.0));
        assert_eq!(heard(&mut e), [ev("OnPackageEnd", FAR)]);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn a_walker_unloading_outside_the_buffer_takes_up_its_journey_at_once() {
        let (_f, mut e) = fixture();
        // It set off before unloading, and is walking.
        loaded_actor(&mut e);
        let a = e.actor_mut(ACTOR).unwrap();
        a.exec.pos = Vec3::ZERO;
        a.exec.packages = vec![package(FAR, Behaviour::Travel, false)];
        a.exec.packages[0].location = Some(super::super::package::Location {
            kind: super::super::package::LocationKind::NearReference(MARKER),
            radius: 0.0,
        });
        a.exec.current = Some(0);
        a.exec.begin_run(Some(0), 8.0, false);
        a.state = crate::ai::State::Walk {
            path: Vec::new(),
            next: 0,
            budget: 0.0,
            to_seat: false,
        };
        a.fresh = false;
        e.unload_all();
        e.away_steps.buffered.clear();
        e.step_away(ACTOR);
        // Away, it is put 40 short.
        let x = &e.away[&ACTOR];
        assert_eq!(x.package(), Some(FAR));
        assert_eq!(x.pos, Vec3::new(2960.0, 0.0, 0.0));
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn a_script_moving_an_actor_away_sets_it_off_again_from_there() {
        let (_f, mut e) = fixture();
        away_at(&mut e, 0.0, false);
        e.hour = 12.1;
        e.step_away(ACTOR);
        assert!(e.move_to_editor_location(ACTOR));
        assert_eq!(e.away[&ACTOR].pos, Vec3::ZERO);
        // Its package takes it on from there.
        e.step_away(ACTOR);
        assert_eq!(e.away[&ACTOR].pos, Vec3::new(2960.0, 0.0, 0.0));
    }
}
