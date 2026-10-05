//! Actor AI: package selection, sandboxing / travelling over the navmesh.

pub mod furniture;
pub mod idles;
pub mod nav;
pub mod package;
pub mod schedule;

use std::sync::Arc;

use esp::FormId;
use glam::{Mat4, Quat, Vec3};

use crate::engine::{Engine, PLAYER_REF};
use crate::world::animation::{ActorAnim, BoundClip};
use crate::world::skeleton::Skeleton;
use furniture::{FurnitureWorld, Seat, Use};
use package::{Allow, Behaviour, LocationKind, Package, Target};

/// Forward walk speed (units/s) when the walk clip has no root motion.
pub const WALK_SPEED: f32 = 80.0;
/// Turn rate while walking, radians/s.
const TURN_RATE: f32 = 4.0;
/// Real seconds between package re-evaluations.
const EVAL_INTERVAL: f32 = 3.0;
/// Sandbox wander radius clamp.
const SANDBOX_MIN: f32 = 200.0;
const SANDBOX_MAX: f32 = 1200.0;
/// Longest path an actor will set out on while sandboxing / travelling.
const MAX_SANDBOX_PATH: f32 = 6000.0;
const MAX_TRAVEL_PATH: f32 = 30_000.0;
/// How far from a sleep / sit location to look for a bed or chair.
const FURNITURE_SEARCH: f32 = 768.0;

/// Where and how the current package wants the actor to be.
#[derive(Debug, Clone, Copy)]
pub struct Goal {
    pub behaviour: Behaviour,
    pub centre: Vec3,
    pub radius: f32,
    pub allow: Allow,
    pub energy: f32,
    /// Specific furniture to use (a Sit package's chair).
    pub furniture: Option<FormId>,
    /// Patrol start or follow target.
    pub target: Option<FormId>,
    pub repeat: bool,
    pub start_nearest: bool,
    /// Follow: (min, max) distance to keep.
    pub follow_radius: (f32, f32),
}

impl Goal {
    pub fn travel(to: Vec3) -> Goal {
        Goal {
            behaviour: Behaviour::Travel,
            centre: to,
            radius: 0.0,
            allow: Allow { wandering: false, ..package::Allow::default() },
            energy: 50.0,
            furniture: None,
            target: None,
            repeat: false,
            start_nearest: false,
            follow_radius: (0.0, 0.0),
        }
    }
}

/// A point on a patrol route.
#[derive(Debug, Clone, Copy)]
pub struct PatrolPoint {
    pub ref_id: FormId,
    pub pos: Vec3,
}

#[derive(Debug)]
enum State {
    Idle(f32),
    /// Following a path; `to_seat` when heading for the start of `ActorRuntime::seat`.
    Walk { path: Vec<Vec3>, next: usize, budget: f32, to_seat: bool },
    /// At the start of the seat's enter animation, turning to face the right way.
    Approach(f32),
    /// Playing enter clip `step` of the seat, which started at `from` / `heading`.
    Enter { step: usize, from: Vec3, heading: f32 },
    /// In the furniture for this many more seconds.
    Use(f32),
    /// Playing exit clip `step`, which started at `from` / `heading`.
    Exit { step: usize, from: Vec3, heading: f32 },
}

/// Clip and behaviour lookups for the actor being stepped.
pub struct Clips<'a> {
    pub vfs: &'a vfs::Vfs,
    pub anims: &'a mut crate::world::animation::AnimationLibrary,
    pub behaviors: &'a mut crate::world::animation::BehaviorLibrary,
}

impl Clips<'_> {
    pub fn clip(&mut self, path: &str, skeleton_path: &str, skeleton: &Skeleton) -> Option<Arc<BoundClip>> {
        self.anims.clip(self.vfs, path, skeleton_path, skeleton)
    }

    /// Clips to enter, loop in and leave the state that behaviour `event` leads to,
    /// for an actor of the character project at `project`.
    pub fn event(&mut self, event: &str, project: &str, skeleton_path: &str, female: bool, skeleton: &Skeleton) -> Option<furniture::UseClips> {
        use havok::behavior::ClipMode;
        let played = self.behaviors.event_clips(self.vfs, project, event)?;
        let mut load = |c: &havok::behavior::PlayedClip| {
            crate::world::animation::project_clip_paths(project, &c.animation, female)
                .iter()
                .find_map(|p| self.anims.clip(self.vfs, p, skeleton_path, skeleton))
        };
        let (last, enter) = played.clips.split_last()?;
        let idle = load(last)?;
        let enter = enter.iter().map(&mut load).collect::<Option<Vec<_>>>()?;
        // The exit ends back in the default idle; only its one-shot clips matter.
        let exit = played.exit.iter().filter(|c| c.mode == ClipMode::SinglePlay).filter_map(&mut load).collect();
        let objects = played
            .draws
            .iter()
            .filter_map(|d| {
                let clip = enter.get(d.clip).unwrap_or(&idle);
                let time = if d.from_end { clip.duration() + d.time } else { d.time };
                Some(furniture::ObjectCue { clip: d.clip.min(enter.len()), time: time.max(0.0), anio: d.payload.clone()? })
            })
            .collect();
        Some(furniture::UseClips { enter, idle, exit, idle_loops: last.mode != ClipMode::SinglePlay, objects })
    }
}

enum PatrolStep {
    Walk(Vec3),
    /// Pausing or using a marker; the state is already set.
    Busy,
    /// End of a one-way route (or none): stay.
    Done,
}

/// What the AI needs from the world while stepping an actor.
pub struct World<'a> {
    pub nav: &'a nav::NavWorld,
    pub furniture: &'a mut FurnitureWorld,
    pub clips: Clips<'a>,
    /// Whether an NPC may use a kind of furniture with this owner.
    pub may_use: &'a dyn Fn(FormId, Use, Option<FormId>) -> bool,
    /// Furniture that is a specific actor's package target (kept free for them).
    pub claimed: &'a std::collections::HashMap<FormId, FormId>,
    /// Patrol routes by start reference.
    pub patrols: &'a std::collections::HashMap<FormId, Arc<Vec<PatrolPoint>>>,
    /// Current positions of follow targets.
    pub targets: &'a std::collections::HashMap<FormId, Vec3>,
    pub rand: &'a mut dyn FnMut() -> u64,
}

fn path_length(from: Vec3, path: &[Vec3]) -> f32 {
    std::iter::once(&from).chain(path).zip(path).map(|(a, b)| a.distance(*b)).sum()
}

fn uniform(rand: &mut dyn FnMut() -> u64, lo: f32, hi: f32) -> f32 {
    lo + (rand() % 1000) as f32 / 1000.0 * (hi - lo)
}

/// Runtime state of one spawned actor.
pub struct ActorRuntime {
    pub ref_id: FormId,
    pub npc: FormId,
    pub skeleton: Arc<Skeleton>,
    pub anim: Option<ActorAnim>,
    pub idle: Option<Arc<BoundClip>>,
    pub walk: Option<Arc<BoundClip>>,
    pub capsule: Option<rapier3d::prelude::ColliderHandle>,
    pub packages: Vec<Package>,
    pub current: Option<usize>,
    pub goal: Option<Goal>,
    /// Feet position and heading (radians clockwise from +Y).
    pub pos: Vec3,
    pub heading: f32,
    pub scale: f32,
    pub editor_pos: Vec3,
    /// Load door the actor is walking to in order to leave the cell.
    pub exiting: Option<FormId>,
    pub skeleton_path: String,
    pub female: bool,
    /// Furniture marker reserved (walking to it) or in use.
    pub seat: Option<Seat>,
    /// Just spawned with the cell: may start out already in furniture.
    pub fresh: bool,
    /// Anim objects (ANIO editor ids) in hand, and whether they changed this frame.
    pub objects: Vec<String>,
    pub objects_changed: bool,
    /// Get out of the furniture as soon as possible (package changed).
    leave: bool,
    /// Patrol progress: (route start, index of the point heading for).
    patrol: Option<(FormId, usize)>,
    state: State,
    next_eval: f32,
    speed: f32,
}

impl ActorRuntime {
    pub fn new(ref_id: FormId, npc: FormId, skeleton: Arc<Skeleton>, transform: Mat4, packages: Vec<Package>, stagger: f32) -> Self {
        let (scale, rot, pos) = transform.to_scale_rotation_translation();
        let f = rot * Vec3::Y;
        ActorRuntime {
            ref_id,
            npc,
            skeleton,
            anim: None,
            idle: None,
            walk: None,
            capsule: None,
            packages,
            current: None,
            goal: None,
            pos,
            heading: f.x.atan2(f.y),
            scale: scale.x,
            editor_pos: pos,
            exiting: None,
            skeleton_path: String::new(),
            female: false,
            seat: None,
            fresh: true,
            objects: Vec::new(),
            objects_changed: false,
            leave: false,
            patrol: None,
            state: State::Idle(1.0 + stagger),
            next_eval: stagger,
            speed: 0.0,
        }
    }

    pub fn transform(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(Vec3::splat(self.scale), Quat::from_rotation_z(-self.heading), self.pos)
    }

    fn state_name(&self) -> &'static str {
        match self.state {
            State::Idle(_) => "idle",
            State::Walk { .. } => "walk",
            State::Approach(_) => "approach",
            State::Enter { .. } => "enter",
            State::Use(_) => "use",
            State::Exit { .. } => "exit",
        }
    }

    pub fn is_walking(&self) -> bool {
        matches!(self.state, State::Walk { .. })
    }

    /// Getting into, using or getting out of furniture.
    pub fn in_furniture(&self) -> bool {
        matches!(self.state, State::Enter { .. } | State::Use(_) | State::Exit { .. })
    }

    /// Stop and stand (e.g. when spoken to). Actors in furniture stay put.
    pub fn halt(&mut self, idle: f32) {
        if self.in_furniture() {
            return;
        }
        self.state = State::Idle(idle);
        self.speed = 0.0;
    }

    /// The package changed: get up (if in furniture) or stop, then plan anew.
    pub fn interrupt(&mut self, furniture: &mut FurnitureWorld) {
        if self.in_furniture() {
            self.leave = true;
        } else {
            self.give_up_seat(furniture);
            self.halt(0.5);
        }
    }

    fn give_up_seat(&mut self, furniture: &mut FurnitureWorld) {
        if self.seat.take().is_some() {
            furniture.release(self.ref_id);
        }
    }

    fn turn_towards(&mut self, dir: Vec3, dt: f32) -> f32 {
        let want = dir.x.atan2(dir.y);
        let mut d = (want - self.heading).rem_euclid(std::f32::consts::TAU);
        if d > std::f32::consts::PI {
            d -= std::f32::consts::TAU;
        }
        let step = d.clamp(-TURN_RATE * dt, TURN_RATE * dt);
        self.heading += step;
        d - step
    }

    /// Step aside from other actors (and the player) within reach, preferring to
    /// pass on the side we're already heading for. Stays on the navmesh.
    fn keep_clear(&mut self, bodies: &[(FormId, Vec3)], nav: &nav::NavWorld, dt: f32, before: Vec3) {
        const REACH: f32 = 48.0;
        // Someone already standing at our destination: stop short of it.
        if let State::Walk { path, next, to_seat: false, .. } = &mut self.state
            && let Some(&dest) = path.last()
            && self.pos.truncate().distance(dest.truncate()) < REACH * 2.0
            && bodies.iter().any(|&(id, p)| id != self.ref_id && p.truncate().distance(dest.truncate()) < REACH && (p.z - dest.z).abs() < 100.0)
        {
            *next = path.len();
            return;
        }
        let fwd = glam::Vec2::new(self.heading.sin(), self.heading.cos());
        let mut push = glam::Vec2::ZERO;
        let mut blocked = false;
        for &(id, p) in bodies {
            if id == self.ref_id || (p.z - self.pos.z).abs() > 100.0 {
                continue;
            }
            let d = (self.pos - p).truncate();
            let dist = d.length();
            if dist >= REACH {
                continue;
            }
            let away = if dist > 1e-3 { d / dist } else { fwd.perp() };
            let w = (REACH - dist) / REACH;
            push += away * w;
            // Someone ahead: sidestep, to the right when they're dead ahead.
            let to_them = -away;
            blocked |= fwd.dot(to_them) > 0.7 && dist < REACH * 0.8;
            if fwd.dot(to_them) > 0.5 {
                let left = fwd.perp();
                push += if left.dot(to_them) > 0.0 { -left } else { left } * w * 0.7;
            }
        }
        if push == glam::Vec2::ZERO {
            return;
        }
        let step = push.clamp_length_max(1.0) * WALK_SPEED * 0.8 * dt;
        // Blocked ahead: give up this frame's forward progress and only sidestep.
        let from = if blocked { before } else { self.pos };
        let mut p = from + step.extend(0.0);
        match nav.height_at(p) {
            Some(z) if (z - from.z).abs() < 30.0 => {
                p.z = z;
                self.pos = p;
            }
            // No room to step aside: wait behind them.
            _ if blocked => self.pos = before,
            _ => {}
        }
    }

    /// Place the actor along a clip's root motion from a start pose.
    fn follow_motion(&mut self, from: Vec3, heading: f32) {
        let Some(anim) = &self.anim else { return };
        if let Some(m) = &anim.clip.motion {
            let (off, yaw) = m.sample(anim.time);
            self.pos = from + furniture::to_world(off, heading);
            self.heading = heading - yaw;
        }
    }

    /// Start sitting / lying / leaning in the reserved seat for `secs`.
    fn settle(&mut self, secs: f32, fade: f32) {
        let Some(seat) = &self.seat else { return };
        self.pos = seat.pos;
        self.heading = seat.heading;
        if let Some(anim) = &mut self.anim {
            anim.play(seat.clips.idle.clone(), fade);
        }
        self.state = State::Use(secs);
    }

    /// Advance movement. Returns true when the actor moved.
    pub fn step(&mut self, dt: f32, w: &mut World) -> bool {
        match &mut self.state {
            State::Idle(t) => {
                self.speed = 0.0;
                *t -= dt;
                if *t > 0.0 {
                    return false;
                }
                self.plan(w);
                false
            }
            State::Walk { path, next, budget, to_seat } => {
                *budget -= dt;
                if *budget < 0.0 || *next >= path.len() {
                    let arrived = *budget >= 0.0;
                    log::debug!("{} walk ended ({})", self.ref_id, if arrived { "arrived" } else { "timeout" });
                    self.speed = 0.0;
                    let follow = self.goal.is_some_and(|g| g.behaviour == Behaviour::Follow);
                    if *to_seat && arrived {
                        self.state = State::Approach(0.0);
                    } else if follow || (arrived && self.goal.is_some_and(|g| g.behaviour == Behaviour::Patrol)) {
                        // Keep up with the target / carry on along the route.
                        self.state = State::Idle(0.1);
                    } else {
                        self.give_up_seat(w.furniture);
                        self.state = State::Idle(uniform(w.rand, 4.0, 15.0));
                    }
                    return false;
                }
                let target = path[*next];
                let to = (target - self.pos).truncate();
                let dist = to.length();
                if dist < 12.0 {
                    *next += 1;
                    return false;
                }
                let dir = (to / dist).extend(0.0);
                let remaining = self.turn_towards(dir, dt);
                // Slow down while facing away from the next waypoint.
                let speed = self.walk_speed() * remaining.cos().max(0.0).powi(2);
                self.speed = speed;
                let fwd = Vec3::new(self.heading.sin(), self.heading.cos(), 0.0);
                let mut p = self.pos + fwd * (speed * dt).min(dist);
                let z_hint = Vec3::new(p.x, p.y, self.pos.z);
                p.z = w.nav.height_at(z_hint).unwrap_or_else(|| {
                    // Interpolate towards the waypoint height off the mesh.
                    self.pos.z + (target.z - self.pos.z) * ((speed * dt) / dist).min(1.0)
                });
                self.pos = p;
                true
            }
            State::Approach(t) => {
                *t += dt;
                let t = *t;
                let Some(seat) = self.seat.clone() else {
                    self.state = State::Idle(1.0);
                    return false;
                };
                let (start, h0) = furniture::Seat::enter_start(&seat);
                // Close the last few units and turn to the start heading.
                let to = (start - self.pos).truncate();
                let step = (WALK_SPEED * 0.5 * dt).min(to.length());
                self.pos += (to.normalize_or_zero() * step).extend(0.0);
                let remaining = self.turn_towards(Vec3::new(h0.sin(), h0.cos(), 0.0), dt);
                if remaining.abs() < 0.03 && to.length() < 2.0 || t > 3.0 {
                    self.pos = start;
                    self.heading = h0;
                    if !seat.clips.enter.is_empty() {
                        log::debug!("{} entering {} ({} clips)", self.ref_id, seat.furniture, seat.clips.enter.len());
                    }
                    self.state = State::Enter { step: 0, from: start, heading: h0 };
                    self.next_clip(true, 0.2);
                }
                true
            }
            State::Enter { from, heading, .. } | State::Exit { from, heading, .. } => {
                let (from, heading) = (*from, *heading);
                self.follow_motion(from, heading);
                if self.anim.as_ref().is_none_or(|a| a.finished()) {
                    let entering = matches!(self.state, State::Enter { .. });
                    if let State::Enter { step, from, heading } | State::Exit { step, from, heading } = &mut self.state {
                        // The next clip starts where this one left the actor.
                        *step += 1;
                        *from = self.pos;
                        *heading = self.heading;
                    }
                    if !self.next_clip(entering, 0.15) && !entering {
                        self.stand_up(w);
                    }
                }
                true
            }
            State::Use(t) => {
                *t -= dt;
                if *t > 0.0 && !self.leave {
                    return false;
                }
                self.leave = false;
                let Some(seat) = &self.seat else {
                    self.stand_up(w);
                    return false;
                };
                if !seat.clips.exit.is_empty() {
                    log::debug!("{} getting up ({} clips)", self.ref_id, seat.clips.exit.len());
                }
                self.state = State::Exit { step: 0, from: seat.pos, heading: seat.heading };
                if !self.next_clip(false, 0.2) {
                    self.stand_up(w);
                }
                false
            }
        }
    }

    /// Start the current enter / exit clip. Returns false when the sequence is done
    /// (entering then settles into the seat).
    fn next_clip(&mut self, entering: bool, fade: f32) -> bool {
        let (State::Enter { step, .. } | State::Exit { step, .. }) = self.state else { return false };
        let Some(seat) = &self.seat else { return false };
        let list = if entering { &seat.clips.enter } else { &seat.clips.exit };
        match (list.get(step).cloned(), &mut self.anim) {
            (Some(clip), Some(anim)) => {
                anim.play_once(clip, fade);
                true
            }
            _ => {
                if entering {
                    let secs = seat.duration;
                    self.settle(secs, if step == 0 { 0.4 } else { fade });
                }
                false
            }
        }
    }

    /// Back on the floor after using furniture.
    fn stand_up(&mut self, w: &mut World) {
        log::debug!("{} stood up at {:?}", self.ref_id, self.pos);
        self.give_up_seat(w.furniture);
        if let Some(z) = w.nav.height_at(self.pos) {
            self.pos.z = z;
        }
        self.state = State::Idle(uniform(w.rand, 2.0, 6.0));
    }

    /// Choose what to do next from the current goal.
    fn plan(&mut self, w: &mut World) {
        // VRM_AI_NO_SNAP: walk into furniture even on cell load (to watch enter animations).
        let fresh = std::mem::take(&mut self.fresh) && std::env::var_os("VRM_AI_NO_SNAP").is_none();
        self.give_up_seat(w.furniture);
        let Some(goal) = self.goal else {
            self.state = State::Idle(uniform(w.rand, 5.0, 10.0));
            return;
        };
        if self.seek_furniture(&goal, fresh, w) {
            return;
        }
        // Heading back into the sandbox area from elsewhere counts as travel.
        let mut travelling = goal.behaviour != Behaviour::Sandbox;
        let target = match goal.behaviour {
            Behaviour::Hold => None,
            Behaviour::Patrol => match self.patrol_target(&goal, w) {
                PatrolStep::Walk(p) => Some(p),
                PatrolStep::Busy => return,
                PatrolStep::Done => None,
            },
            Behaviour::Follow => {
                let (min, max) = goal.follow_radius;
                let t = goal.target.and_then(|t| w.targets.get(&t).copied());
                match t {
                    Some(t) if self.pos.distance(t) > max => {
                        // Head for a spot `min` short of the target.
                        let back = (self.pos - t).truncate().normalize_or_zero().extend(0.0) * min;
                        Some(t + back)
                    }
                    Some(_) => {
                        self.state = State::Idle(0.5);
                        return;
                    }
                    None => None,
                }
            }
            Behaviour::Travel | Behaviour::Sleep | Behaviour::Sit => {
                let near = goal.radius.max(96.0);
                (self.pos.truncate().distance(goal.centre.truncate()) > near).then_some(goal.centre)
            }
            Behaviour::Sandbox => {
                // Return to the sandbox area first if we've strayed from it.
                let r = goal.radius.clamp(SANDBOX_MIN, SANDBOX_MAX);
                if self.pos.truncate().distance(goal.centre.truncate()) > r * 1.5 {
                    travelling = true;
                    Some(goal.centre)
                } else if goal.allow.wandering || (w.rand)().is_multiple_of(4) {
                    w.nav.random_point(goal.centre, r, &mut *w.rand)
                } else {
                    None
                }
            }
        };
        let path = target.and_then(|t| w.nav.find_path(self.pos, t));
        match path {
            Some(path) if !path.is_empty() => {
                let len = path_length(self.pos, &path);
                let max = if travelling { MAX_TRAVEL_PATH } else { MAX_SANDBOX_PATH };
                if len > max {
                    log::debug!("{} path too long ({len:.0})", self.ref_id);
                    self.state = State::Idle(uniform(w.rand, 10.0, 20.0));
                    return;
                }
                log::debug!("{} walking {:.0} units via {} points ({:?})", self.ref_id, len, path.len(), goal.behaviour);
                let mut budget = len / WALK_SPEED * 2.5 + 5.0;
                if goal.behaviour == Behaviour::Follow {
                    budget = budget.min(2.0);
                }
                self.state = State::Walk { path, next: 0, budget, to_seat: false };
            }
            _ => {
                if target.is_some() {
                    log::debug!("{} no path to {:?}", self.ref_id, target);
                }
                self.state = State::Idle(uniform(w.rand, 6.0, 16.0));
            }
        }
    }

    /// Where to go next on a patrol. At a point that is an idle marker (or other
    /// usable furniture) the actor uses it before moving on.
    fn patrol_target(&mut self, goal: &Goal, w: &mut World) -> PatrolStep {
        let Some(start) = goal.target else { return PatrolStep::Done };
        let Some(points) = w.patrols.get(&start).cloned() else { return PatrolStep::Done };
        if points.is_empty() {
            return PatrolStep::Done;
        }
        let idx = match self.patrol {
            Some((s, i)) if s == start => i,
            _ if goal.start_nearest => {
                let near = points.iter().enumerate().min_by(|a, b| a.1.pos.distance(self.pos).total_cmp(&b.1.pos.distance(self.pos)));
                near.map_or(0, |(i, _)| i)
            }
            _ => 0,
        };
        let Some(p) = points.get(idx).copied() else { return PatrolStep::Done };
        if self.pos.truncate().distance(p.pos.truncate()) > goal.radius.max(48.0) {
            self.patrol = Some((start, idx));
            return PatrolStep::Walk(p.pos);
        }
        // Reached: move the cursor on, wrapping when repeatable.
        let next = if idx + 1 < points.len() {
            Some(idx + 1)
        } else if goal.repeat {
            Some(0)
        } else {
            None
        };
        self.patrol = Some((start, next.unwrap_or(idx)));
        if w.furniture.get(p.ref_id).is_some() {
            let at = Goal { behaviour: Behaviour::Patrol, centre: p.pos, furniture: Some(p.ref_id), ..*goal };
            if self.seek_furniture(&at, false, w) {
                return PatrolStep::Busy;
            }
        }
        match next {
            Some(n) => {
                self.state = State::Idle(uniform(w.rand, 0.2, 1.5));
                log::debug!("{} patrol point {idx} reached, next {n}", self.ref_id);
                PatrolStep::Busy
            }
            None => PatrolStep::Done,
        }
    }

    /// Look for furniture the goal wants used; reserve it and head there (or, when
    /// `fresh`, start out in it). Returns true if the actor is now on its way.
    fn seek_furniture(&mut self, goal: &Goal, fresh: bool, w: &mut World) -> bool {
        // Only humanoids have furniture animations.
        if !self.skeleton_path.contains("actors/character/") {
            return false;
        }
        let (kinds, centre, radius, secs): (Vec<Use>, Vec3, f32, f32) = match goal.behaviour {
            Behaviour::Sleep => (vec![Use::Sleep], goal.centre, goal.radius.max(FURNITURE_SEARCH), f32::INFINITY),
            // A Sit package's chair may be special furniture (a throne, a writing desk).
            Behaviour::Sit => {
                // A specific target can be anything usable; otherwise a chair or special seat.
                let kinds = if goal.furniture.is_some() {
                    vec![Use::Sit, Use::Special, Use::Sleep, Use::Lean, Use::Idle]
                } else {
                    vec![Use::Sit, Use::Special]
                };
                (kinds, goal.centre, goal.radius.max(FURNITURE_SEARCH), f32::INFINITY)
            }
            Behaviour::Sandbox => {
                let mut kinds = Vec::new();
                if goal.allow.sitting {
                    kinds.push(Use::Sit);
                }
                if goal.allow.idle_markers {
                    kinds.extend([Use::Lean, Use::Idle]);
                }
                if goal.allow.special_furniture {
                    kinds.push(Use::Special);
                }
                if kinds.is_empty() {
                    return false;
                }
                // Restless actors wander more and sit for less long.
                let chance = if goal.allow.wandering { 0.75 - goal.energy / 200.0 } else { 0.9 };
                if uniform(w.rand, 0.0, 1.0) > chance {
                    return false;
                }
                let secs = (150.0 - goal.energy * 1.2) * uniform(w.rand, 0.6, 1.4);
                (kinds, goal.centre, goal.radius.clamp(SANDBOX_MIN, SANDBOX_MAX), secs)
            }
            // Patrol idle markers: the point's marker, for its idle time.
            Behaviour::Patrol if goal.furniture.is_some() => {
                (vec![Use::Idle, Use::Lean, Use::Special, Use::Sit], goal.centre, 96.0, uniform(w.rand, 6.0, 12.0))
            }
            Behaviour::Travel | Behaviour::Hold | Behaviour::Patrol | Behaviour::Follow => return false,
        };
        let npc = self.npc;
        let mut options = Vec::new();
        for &kind in &kinds {
            let me = self.ref_id;
            options.extend(w.furniture.free_near(kind, centre, radius, |f, kind| {
                goal.furniture.is_none_or(|r| r == f.ref_id)
                    && w.claimed.get(&f.ref_id).is_none_or(|&a| a == me)
                    && (w.may_use)(npc, kind, f.owner)
            }));
        }
        if options.is_empty() {
            log::debug!("{} no free {kinds:?} within {radius:.0} of {centre:?}", self.ref_id);
            return false;
        }
        // Beds: own bed first, then nearest. Otherwise a random nearby choice.
        if goal.behaviour == Behaviour::Sleep {
            let rank = |f: FormId| match w.furniture.get(f).and_then(|f| f.owner) {
                Some(o) if o == npc => 0,
                Some(_) => 1,
                None => 2,
            };
            options.sort_by(|a, b| {
                (rank(a.0), a.2.pos.distance(self.pos)).partial_cmp(&(rank(b.0), b.2.pos.distance(self.pos))).unwrap()
            });
        } else {
            for i in (1..options.len()).rev() {
                options.swap(i, ((w.rand)() % (i as u64 + 1)) as usize);
            }
        }
        for (fid, mi, marker) in options.into_iter().take(6) {
            let Some(f) = w.furniture.get(fid) else { continue };
            let ways = furniture::ways_to_use(f, &marker, &self.skeleton_path, self.female, &self.skeleton, &mut w.clips, &mut *w.rand);
            // Idle markers say how long they are used for.
            let secs = if f.idle_time > 0.0 && matches!(goal.behaviour, Behaviour::Sandbox | Behaviour::Patrol) { f.idle_time } else { secs };
            // The entry whose starting spot is on the navmesh and nearest.
            let best = ways
                .into_iter()
                .map(|(entry, clips)| {
                    let (start, _) = furniture::enter_start(&marker, &clips.enter);
                    (entry, clips, start)
                })
                .filter(|(_, _, start)| w.nav.height_at(*start).is_some_and(|z| (z - start.z).abs() < 48.0))
                .min_by(|a, b| a.2.distance(self.pos).total_cmp(&b.2.distance(self.pos)));
            let Some((entry, clips, start)) = best else {
                log::debug!("{} can't use {fid} marker {mi}: no reachable entry", self.ref_id);
                continue;
            };
            let seat = Seat {
                furniture: fid,
                pos: marker.pos,
                heading: marker.heading,
                clips: Arc::new(clips),
                duration: secs,
            };
            if fresh {
                w.furniture.reserve(fid, mi, self.ref_id);
                self.seat = Some(seat);
                self.settle(secs, 0.0);
                log::debug!("{} starts in {fid} marker {mi} ({:?})", self.ref_id, marker.kind);
                return true;
            }
            let Some(route) = w.nav.find_path(self.pos, start) else { continue };
            let len = path_length(self.pos, &route);
            if len > MAX_TRAVEL_PATH {
                continue;
            }
            w.furniture.reserve(fid, mi, self.ref_id);
            log::debug!("{} heading for {fid} marker {mi} ({:?}, {entry:?} entry, {len:.0} units)", self.ref_id, marker.kind);
            self.seat = Some(seat);
            self.state = State::Walk { path: route, next: 0, budget: len / WALK_SPEED * 2.5 + 5.0, to_seat: true };
            return true;
        }
        false
    }

    /// Pick the idle or walk clip to match the current speed.
    /// Ground speed of the walk clip (its root motion), so feet don't slide.
    pub fn walk_speed(&self) -> f32 {
        self.walk
            .as_ref()
            .and_then(|w| w.motion.as_ref().map(|m| m.end().0.truncate().length() / w.duration()))
            .filter(|v| (20.0..400.0).contains(v))
            .unwrap_or(WALK_SPEED)
    }

    pub fn animate(&mut self, dt: f32) -> Option<Vec<Mat4>> {
        let walk_speed = self.walk_speed();
        let walking = self.speed > 5.0;
        let locomotion = matches!(self.state, State::Idle(_) | State::Walk { .. } | State::Approach(_));
        let anim = self.anim.as_mut()?;
        match (walking, &self.walk, &self.idle) {
            _ if !locomotion => anim.speed = 1.0,
            (true, Some(w), _) => {
                anim.play(w.clone(), 0.25);
                anim.speed = (self.speed / walk_speed).clamp(0.3, 1.5);
            }
            (false, _, Some(i)) => {
                anim.play(i.clone(), 0.35);
                anim.speed = 1.0;
            }
            _ => {}
        }
        let pose = anim.update(&self.skeleton, dt);
        self.update_objects();
        Some(pose)
    }

    /// Draw the seat's anim objects as their cues come up; they are put away
    /// (`AnimObjectUnequip`) once the actor is out of the furniture or idle.
    fn update_objects(&mut self) {
        let (clip, time) = match (&self.state, &self.anim) {
            (State::Enter { step, .. }, Some(a)) => (*step, a.time),
            (State::Use(_), Some(a)) => (usize::MAX, a.time),
            (State::Exit { .. }, _) => return,
            _ => {
                self.objects_changed |= !self.objects.is_empty();
                self.objects.clear();
                return;
            }
        };
        let Some(seat) = &self.seat else { return };
        let idle = seat.clips.enter.len();
        let clip = clip.min(idle);
        for cue in &seat.clips.objects {
            let due = cue.clip < clip || (cue.clip == clip && time >= cue.time);
            if due && !self.objects.iter().any(|o| o.eq_ignore_ascii_case(&cue.anio)) {
                self.objects.push(cue.anio.clone());
                self.objects_changed = true;
            }
        }
    }
}

impl Engine {
    /// Play behaviour `event` on a loaded humanoid actor (Papyrus `PlayIdle` /
    /// `Debug.SendAnimationEvent`). Idle-stopping events return it to its AI.
    pub fn play_animation_event(&mut self, actor: FormId, event: &str) -> bool {
        let Some(key) = self.actor_cells.get(&actor).copied() else { return false };
        let Some(rt) = self.cells.get_mut(&key) else { return false };
        let Some(a) = rt.actors.iter_mut().find(|a| a.ref_id == actor) else { return false };
        let lower = event.to_ascii_lowercase();
        if matches!(lower.as_str(), "idlestop" | "idlestopinstant" | "idleforcedefaultstate" | "idlechairexitstart" | "idlefurnitureexit") {
            if a.in_furniture() {
                a.leave = true;
            }
            return true;
        }
        let Some(project) = a.skeleton_path.split("/character assets").next().filter(|b| b.ends_with("actors/character")).map(str::to_owned)
        else {
            return false;
        };
        let mut clips = Clips { vfs: &self.vfs, anims: &mut self.anims, behaviors: &mut self.behaviors };
        let Some(c) = clips.event(event, &project, &a.skeleton_path, a.female, &a.skeleton) else { return false };
        self.furniture.release(a.ref_id);
        // Where the enter clips leave the actor; it stays there.
        let mut pos = a.pos;
        let mut heading = a.heading;
        for m in c.enter.iter().filter_map(|c| c.motion.as_ref()) {
            let (t, yaw) = m.end();
            pos += furniture::to_world(t, heading);
            heading -= yaw;
        }
        // A one-shot gesture (no loop at the end) is played through once.
        let duration = if c.idle_loops { f32::INFINITY } else { c.idle.duration() };
        log::debug!("{actor} plays {event} ({} enter clips)", c.enter.len());
        a.seat = Some(Seat { furniture: FormId::NULL, pos, heading, clips: Arc::new(c), duration });
        a.state = State::Enter { step: 0, from: a.pos, heading: a.heading };
        a.leave = false;
        a.next_clip(true, 0.25);
        true
    }

    /// Send a loaded actor to use specific furniture (Papyrus `Activate` by an NPC,
    /// the `use` console command). It stays until its package changes.
    pub fn use_furniture(&mut self, actor: FormId, furniture: FormId) -> bool {
        let Some(f) = self.furniture.get(furniture) else { return false };
        let centre = f.markers.first().map_or(Vec3::ZERO, |m| m.pos);
        let Some(key) = self.actor_cells.get(&actor).copied() else { return false };
        let mut seed = self.rand() | 1;
        let mut rand = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let may_use = |_: FormId, _: Use, _: Option<FormId>| true;
        let (claimed, targets) = Default::default();
        let mut w = World {
            nav: &self.nav,
            furniture: &mut self.furniture,
            clips: Clips { vfs: &self.vfs, anims: &mut self.anims, behaviors: &mut self.behaviors },
            may_use: &may_use,
            claimed: &claimed,
            patrols: &self.patrol_paths,
            targets: &targets,
            rand: &mut rand,
        };
        let Some(a) = self.cells.get_mut(&key).and_then(|rt| rt.actors.iter_mut().find(|a| a.ref_id == actor)) else { return false };
        a.give_up_seat(w.furniture);
        let goal = Goal { behaviour: Behaviour::Sit, centre, radius: 64.0, furniture: Some(furniture), ..Goal::travel(centre) };
        a.seek_furniture(&goal, false, &mut w)
    }

    /// The patrol route starting at `start`: it and the references reached by
    /// following default linked refs, until the chain ends or loops back.
    fn patrol_route(&self, start: FormId) -> Vec<PatrolPoint> {
        let mut out: Vec<PatrolPoint> = Vec::new();
        let mut cur = Some(start);
        while let Some(r) = cur {
            if out.len() >= 64 || out.iter().any(|p| p.ref_id == r) {
                break;
            }
            let Some(pos) = self.ref_position(r) else { break };
            out.push(PatrolPoint { ref_id: r, pos });
            cur = self.linked_ref(r, Some(FormId::NULL));
        }
        out
    }

    /// Feet position and heading of a loaded actor.
    pub fn actor_pose(&self, r: FormId) -> Option<(Vec3, f32)> {
        let key = self.actor_cells.get(&r)?;
        let a = self.cells.get(key)?.actors.iter().find(|a| a.ref_id == r)?;
        Some((a.pos, a.heading))
    }

    /// Where a package location points to, as (centre, radius).
    fn package_target(&self, a: &ActorRuntime, p: &Package) -> (Vec3, f32) {
        let Some(loc) = p.location else { return (a.editor_pos, 0.0) };
        let centre = match loc.kind {
            LocationKind::NearReference(r) => self.ref_position(r),
            LocationKind::NearLinkedRef(kw) => {
                self.linked_ref(a.ref_id, (!kw.is_null()).then_some(kw)).and_then(|r| self.ref_position(r))
            }
            LocationKind::NearCurrent | LocationKind::NearSelf => Some(a.pos),
            LocationKind::InCell(_) => return (a.editor_pos, loc.radius.max(SANDBOX_MAX)),
            LocationKind::NearEditor | LocationKind::Other(_) => Some(a.editor_pos),
        };
        (centre.unwrap_or(a.editor_pos), loc.radius)
    }

    /// Choose each actor's package and goal.
    fn evaluate_packages(&mut self, dt: f32) {
        let mut decisions = Vec::new();
        for (key, rt) in &self.cells {
            for (i, a) in rt.actors.iter().enumerate() {
                if a.next_eval - dt > 0.0 || a.exiting.is_some() {
                    continue;
                }
                let ctx = crate::condition::Context { subject: Some(a.ref_id), target: None, quest: None };
                let pick = a.packages.iter().position(|p| {
                    p.schedule.matches(self.hour, self.day) && crate::condition::evaluate(self, &p.conditions, ctx)
                });
                let goal = pick.map(|pi| {
                    let p = &a.packages[pi];
                    let (centre, radius) = self.package_target(a, p);
                    let target = match p.target {
                        Some(Target::Ref(r)) => Some(r),
                        Some(Target::LinkedRef(kw)) => self.linked_ref(a.ref_id, kw),
                        _ => None,
                    };
                    // Only Sit / Sleep packages target furniture; a patrol start or follow
                    // target can be an idle marker without being "the package's seat".
                    let furniture = target
                        .filter(|_| matches!(p.behaviour, Behaviour::Sit | Behaviour::Sleep))
                        .filter(|r| self.furniture.get(*r).is_some());
                    let (centre, radius) = match p.behaviour {
                        Behaviour::Patrol => (centre, p.point_radius),
                        Behaviour::Follow => (target.and_then(|t| self.ref_position(t)).unwrap_or(centre), radius),
                        _ => (centre, radius),
                    };
                    Goal {
                        behaviour: p.behaviour,
                        centre,
                        radius,
                        allow: p.allow,
                        energy: p.energy,
                        furniture,
                        target,
                        repeat: p.repeat,
                        start_nearest: p.start_nearest,
                        follow_radius: p.follow_radius,
                    }
                });
                decisions.push((*key, i, pick, goal));
            }
        }
        for rt in self.cells.values_mut() {
            for a in &mut rt.actors {
                a.next_eval -= dt;
            }
        }
        for (key, i, pick, goal) in decisions {
            let Some(a) = self.cells.get_mut(&key).and_then(|rt| rt.actors.get_mut(i)) else { continue };
            a.next_eval = EVAL_INTERVAL;
            if pick != a.current {
                if let Some(p) = pick.map(|pi| &a.packages[pi]) {
                    log::debug!("{} -> package {} ({}, {:?})", a.ref_id, p.editor_id, p.template, goal);
                }
                // The first package an actor gets doesn't interrupt what it is doing
                // (e.g. an idle a script started on load).
                let first = a.current.is_none();
                a.current = pick;
                a.goal = goal;
                if !first {
                    a.interrupt(&mut self.furniture);
                }
            } else {
                a.goal = goal;
            }
        }
    }

    /// Run AI and animation for every loaded actor.
    pub(crate) fn update_actors(&mut self, dt: f32) {
        self.evaluate_packages(dt);
        let talking = self.conversation.as_ref().map(|c| c.npc_ref);
        let player = self.ref_position(PLAYER_REF).unwrap_or_default();
        let mut seed = self.rand() | 1;
        let mut rand = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let mut patrol_starts = Vec::new();
        let mut targets = std::collections::HashMap::new();
        for a in self.cells.values().flat_map(|rt| &rt.actors) {
            match a.goal {
                Some(g) if g.behaviour == Behaviour::Patrol => patrol_starts.extend(g.target),
                Some(g) if g.behaviour == Behaviour::Follow => {
                    if let Some(t) = g.target
                        && let Some(p) = self.ref_position(t)
                    {
                        targets.insert(t, p);
                    }
                }
                _ => {}
            }
        }
        for s in patrol_starts {
            if !self.patrol_paths.contains_key(&s) {
                let route = Arc::new(self.patrol_route(s));
                log::debug!("patrol route from {s}: {} points", route.len());
                self.patrol_paths.insert(s, route);
            }
        }
        let nav = std::mem::take(&mut self.nav);
        let mut furniture = std::mem::take(&mut self.furniture);
        let (lo, vfs) = (&self.lo, &self.vfs);
        let clips = Clips { vfs, anims: &mut self.anims, behaviors: &mut self.behaviors };
        let may_use = |npc: FormId, kind: Use, owner: Option<FormId>| furniture::may_use(lo, npc, kind, owner);
        let claimed = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter_map(|a| Some((a.goal?.furniture?, a.ref_id)))
            .collect();
        let mut world = World {
            nav: &nav,
            furniture: &mut furniture,
            clips,
            may_use: &may_use,
            claimed: &claimed,
            patrols: &self.patrol_paths,
            targets: &targets,
            rand: &mut rand,
        };
        let mut moved = Vec::new();
        let mut gone = Vec::new();
        let mut held = Vec::new();
        // Everyone's position last frame, for walkers to keep clear of.
        let mut bodies: Vec<(FormId, Vec3)> = self.cells.values().flat_map(|rt| &rt.actors).map(|a| (a.ref_id, a.pos)).collect();
        bodies.push((PLAYER_REF, player));
        if log::log_enabled!(log::Level::Trace) {
            let free: Vec<&ActorRuntime> = self.cells.values().flat_map(|rt| &rt.actors).filter(|a| !a.in_furniture()).collect();
            let mut overlaps = 0;
            for (i, a) in free.iter().enumerate() {
                for b in &free[i + 1..] {
                    if a.pos.distance(b.pos) < 35.0 {
                        overlaps += 1;
                        log::trace!("overlap {} {:?} / {} {:?}", a.ref_id, a.state_name(), b.ref_id, b.state_name());
                    }
                }
            }
            log::trace!("actor overlaps: {overlaps}");
        }
        for (key, rt) in self.cells.iter_mut() {
            let Some(rc) = self.scene.cells.get_mut(key) else { continue };
            for (index, (inst, a)) in rc.actors.iter_mut().zip(rt.actors.iter_mut()).enumerate() {
                if talking == Some(a.ref_id) {
                    // Seated actors talk from where they are.
                    if !a.in_furniture() {
                        a.halt(3.0);
                        let d = player - a.pos;
                        a.turn_towards(d.normalize_or_zero(), dt);
                    }
                } else if !self.ai_enabled {
                    a.halt(1.0);
                } else {
                    let before = a.pos;
                    if a.step(dt, &mut world) && a.is_walking() {
                        a.keep_clear(&bodies, &nav, dt, before);
                    }
                }
                inst.transform = a.transform();
                if let Some(pose) = a.animate(dt) {
                    inst.pose = pose;
                }
                if std::mem::take(&mut a.objects_changed) {
                    log::debug!("{} holds {:?}", a.ref_id, a.objects);
                    held.push((*key, index, a.objects.clone()));
                }
                if a.is_walking() || matches!(a.state, State::Approach(_) | State::Enter { .. } | State::Exit { .. }) || talking == Some(a.ref_id) {
                    moved.push((a.ref_id, a.pos, a.capsule));
                }
                // Reached the door (or gave up out of sight): leave the cell.
                if a.exiting.is_some()
                    && !a.is_walking()
                    && a.goal.is_none_or(|g| g.centre.truncate().distance(a.pos.truncate()) < 64.0 || a.pos.distance(player) > 2500.0)
                {
                    gone.push(a.ref_id);
                }
            }
        }
        self.nav = nav;
        self.furniture = furniture;
        for (key, index, objects) in held {
            self.attach_anim_objects(key, index, &objects);
        }
        for r in gone {
            self.despawn_actor(r);
        }
        for (r, pos, capsule) in moved {
            if let Some(c) = capsule {
                self.physics.move_actor_capsule(c, pos);
            }
            self.moved_refs.insert(r, pos);
        }
    }
}
