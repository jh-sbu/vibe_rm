//! Actor AI: package selection, sandboxing / travelling over the navmesh.

pub mod combat;
mod equipment;
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
use crate::world::behavior::{GraphAnim, ProjectRuntime};
use crate::world::movement::MoveSpeeds;
use crate::world::skeleton::Skeleton;
use furniture::{FurnitureWorld, Seat, Use};
use package::{Allow, Behaviour, LocationKind, Package, Target};

/// Forward walk speed (units/s) when the walk clip has no root motion.
pub const WALK_SPEED: f32 = 80.0;
/// Turn rate while walking, radians/s.
const TURN_RATE: f32 = 4.0;
/// How close the player must be for NPCs to look at them.
const HEAD_TRACK_DISTANCE: f32 = 450.0;
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
    pub gait: package::Gait,
    pub sneak: bool,
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
            gait: package::Gait::Walk,
            sneak: false,
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
    /// Getting in: the graph plays the seat's enter animation (moving the actor by
    /// its root motion) for about this many more seconds.
    Enter(f32),
    /// In the furniture for this many more seconds.
    Use(f32),
    /// Getting out, for at most this many more seconds (the graph says when the
    /// actor is out with `IdleFurnitureExit`).
    Exit(f32),
}

/// An idle played while in furniture (eating, drinking, another way of sitting):
/// the graph plays it; the AI ends it after a while.
struct SubIdle {
    /// Seconds until it is stopped (or over, for a one-shot).
    left: f32,
    /// Events that may end a held idle, in order (none for a one-shot that ends by itself).
    stop: Vec<String>,
}

/// Events that end an idle played in furniture: anim-object idles (eating,
/// drinking) put their object away on `AnimObjectIdleStop`.
const SUB_IDLE_EXITS: [&str; 2] = ["AnimObjectIdleStop", "IdleStop"];

/// Clip and behaviour lookups for the actor being stepped.
pub struct Clips<'a> {
    pub vfs: &'a vfs::Vfs,
    pub anims: &'a mut crate::world::animation::AnimationLibrary,
    pub behaviors: &'a mut crate::world::animation::BehaviorLibrary,
}

impl Clips<'_> {
    /// Clips to enter, loop in and leave the state that behaviour `event` leads to,
    /// for an actor of `project`.
    /// Furniture and idle markers are left with IdleChairExitStart / IdleStop,
    /// anim-object idles (standing eating, drinking) with AnimObjectIdleStop.
    pub fn event(&mut self, event: &str, project: &ProjectRuntime, skeleton_path: &str, female: bool, skeleton: &Skeleton) -> Option<furniture::UseClips> {
        self.event_with_exits(event, &["IdleChairExitStart", "AnimObjectIdleStop", "IdleStop"], project, skeleton_path, female, skeleton)
    }

    /// Like [`Clips::event`], leaving through the first of behaviour events `exits`
    /// that the graph handles from the loop.
    pub fn event_with_exits(
        &mut self,
        event: &str,
        exits: &[&str],
        project: &ProjectRuntime,
        skeleton_path: &str,
        female: bool,
        skeleton: &Skeleton,
    ) -> Option<furniture::UseClips> {
        use havok::behavior::ClipMode;
        let played = self.behaviors.event_clips(project, event, exits)?;
        let mut load = |c: &havok::behavior::PlayedClip| {
            crate::world::animation::project_clip_paths(&project.dir, &c.animation, female)
                .iter()
                .find_map(|p| self.anims.project_clip(self.vfs, p, skeleton_path, skeleton, c.speed, project))
        };
        let (last, enter) = played.clips.split_last()?;
        let idle = load(last)?;
        let enter = enter.iter().map(&mut load).collect::<Option<Vec<_>>>()?;
        // The exit ends back in the default idle; only its one-shot clips matter.
        let exit = played.exit.iter().filter(|c| c.mode == ClipMode::SinglePlay).filter_map(&mut load).collect();
        Some(furniture::UseClips {
            event: event.to_owned(),
            exits: exits.iter().map(|e| e.to_string()).collect(),
            enter,
            idle,
            exit,
            idle_loops: last.mode != ClipMode::SinglePlay && !played.loop_once,
        })
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
    /// The actor's behaviour graph; actors without one play `anim`.
    pub graph: Option<GraphAnim>,
    /// Ground speed of the graph's walk (its clips' root motion).
    pub graph_walk_speed: Option<f32>,
    /// Movement types of the graph's default and sneaking states, with the
    /// `iState` values that select them.
    pub moves: Option<(MoveSpeeds, f32)>,
    pub sneak_moves: Option<(MoveSpeeds, f32)>,
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
    pub child: bool,
    /// Furniture marker reserved (walking to it) or in use.
    pub seat: Option<Seat>,
    /// Just spawned with the cell: may start out already in furniture.
    pub fresh: bool,
    /// Idle being played in the furniture, if any.
    sub: Option<SubIdle>,
    /// Seconds until the next seated idle is picked; set `wants_idle` when due.
    next_idle: f32,
    pub(crate) wants_idle: bool,
    /// Wants to eat or drink where it stands (a sandbox pause).
    pub(crate) wants_meal: bool,
    /// Wants an idle from the `ActionIdle` tree (actors standing about: hands on
    /// hips, a shrug; creatures grazing, lying down, shaking off).
    pub(crate) wants_action_idle: bool,
    /// Anim objects (ANIO editor ids) in hand, and whether they changed this frame.
    pub objects: Vec<String>,
    pub objects_changed: bool,
    /// Get out of the furniture as soon as possible (package changed).
    leave: bool,
    /// The graph is in its locomotion state (`moveStart` sent).
    moving: bool,
    /// Heading at the last graph update, for its turn rate.
    graph_heading: f32,
    /// Turning in place through the graph: -1 right, 1 left, 0 not.
    turning: i8,
    /// The graph is sneaking (`SneakStart` sent).
    sneaking: bool,
    /// What to look at (world space), for humanoids' head tracking.
    pub look_at: Option<Vec3>,
    /// The goal was set by hand (console `travel`): packages leave it alone.
    pinned: bool,
    /// The weapon is in hand (between the graph's `weaponDraw` and `weaponSheathe`),
    /// and whether that changed this frame.
    pub(crate) weapon_out: bool,
    pub(crate) equipment_changed: bool,
    /// Weapons drawn (or being drawn): the graph was sent `WeapEquip`.
    pub(crate) drawn: bool,
    /// Sounds the graph asked for this frame (`SoundPlay` payloads: SNDR editor ids).
    pub(crate) sounds: Vec<String>,
    /// What it fights with, how much health it has left, whom it is fighting,
    /// its weapon's reach (0 unarmed), seconds until it looks for enemies again, and
    /// whether its graph reached a hit frame this update.
    pub(crate) stats: Arc<combat::CombatStats>,
    pub(crate) health: f32,
    pub(crate) combat: Option<combat::Combat>,
    pub(crate) weapon_reach: f32,
    pub(crate) detect_in: f32,
    hit_frame: bool,
    /// Seconds left bleeding out (essential actors brought down).
    pub(crate) bleeding: Option<f32>,
    /// Dead, and the ragdoll it lies as (with the mapping back to its bones).
    pub(crate) dead: bool,
    pub(crate) ragdoll: Option<(crate::physics::Ragdoll, crate::world::ragdoll::RagdollPose)>,
    /// The lit torch it holds, and the shield it put away for it.
    pub(crate) torch: Option<FormId>,
    pub(crate) stowed_shield: Option<FormId>,
    /// The graph raised `IdleFurnitureExit`: out of the furniture.
    out_of_furniture: bool,
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
            graph: None,
            graph_walk_speed: None,
            moves: None,
            sneak_moves: None,
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
            child: false,
            seat: None,
            fresh: true,
            sub: None,
            next_idle: 0.0,
            wants_idle: false,
            wants_meal: false,
            wants_action_idle: false,
            dead: false,
            ragdoll: None,
            stats: Default::default(),
            health: 50.0,
            combat: None,
            weapon_reach: 0.0,
            detect_in: stagger,
            hit_frame: false,
            bleeding: None,
            torch: None,
            stowed_shield: None,
            weapon_out: false,
            equipment_changed: false,
            drawn: false,
            sounds: Vec::new(),
            objects: Vec::new(),
            objects_changed: false,
            leave: false,
            moving: false,
            graph_heading: f32::NAN,
            turning: 0,
            sneaking: false,
            look_at: None,
            pinned: false,
            out_of_furniture: false,
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
            State::Enter(_) => "enter",
            State::Use(_) => "use",
            State::Exit(_) => "exit",
        }
    }

    pub fn is_walking(&self) -> bool {
        matches!(self.state, State::Walk { .. })
    }

    /// Getting into, using or getting out of furniture.
    pub fn in_furniture(&self) -> bool {
        matches!(self.state, State::Enter(_) | State::Use(_) | State::Exit(_))
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
        self.sub = None;
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
        let rate = self.turn_rate();
        let step = d.clamp(-rate * dt, rate * dt);
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

    /// Settled in the reserved seat (the graph is in its loop) for `secs`.
    fn settle(&mut self, secs: f32) {
        let Some(seat) = &self.seat else { return };
        self.pos = seat.pos;
        self.heading = seat.heading;
        self.state = State::Use(secs);
        self.next_idle = 4.0;
    }

    /// Send the graph an event now; true when it took it.
    fn graph_event(&mut self, event: &str, clips: &mut Clips) -> bool {
        match &mut self.graph {
            Some(g) => g.handle_event(event, clips.vfs, clips.anims, &self.skeleton),
            None => false,
        }
    }

    /// At the start of the seat's enter animation: have the graph play it.
    fn begin_enter(&mut self, w: &mut World) {
        let Some(seat) = self.seat.clone() else { return };
        if !self.graph_event(&seat.clips.event, &mut w.clips) {
            log::debug!("{}: the graph won't take {} for {}", self.ref_id, seat.clips.event, seat.furniture);
            self.give_up_seat(w.furniture);
            self.state = State::Idle(uniform(w.rand, 2.0, 5.0));
            return;
        }
        log::debug!("{} entering {} ({}, {:.1} s)", self.ref_id, seat.furniture, seat.clips.event, seat.clips.enter_time());
        self.out_of_furniture = false;
        self.state = State::Enter(seat.clips.enter_time());
    }

    /// Have the graph leave the seat by the first exit it takes.
    fn begin_exit(&mut self, w: &mut World) {
        let Some(seat) = self.seat.clone() else {
            self.stand_up(w);
            return;
        };
        let exit = seat.clips.exits.iter().find(|e| self.graph_event(e, &mut w.clips)).cloned();
        let Some(exit) = exit else {
            // Nothing to play (or no graph): just stand.
            self.stand_up(w);
            return;
        };
        log::debug!("{} getting up ({exit}, {:.1} s)", self.ref_id, seat.clips.exit_time());
        self.out_of_furniture = false;
        self.state = State::Exit(seat.clips.exit_time() + 0.5);
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
                        if arrived {
                            self.maybe_eat(w);
                        }
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
                let speed = self.move_speed() * remaining.cos().max(0.0).powi(2);
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
                    self.begin_enter(w);
                }
                true
            }
            // The graph moves the actor (root motion, applied in `animate`).
            State::Enter(t) => {
                *t -= dt;
                if *t <= 0.0 {
                    let secs = self.seat.as_ref().map_or(0.0, |s| s.duration);
                    self.settle(secs);
                }
                true
            }
            State::Exit(t) => {
                // The graph may finish a swing or a gesture before its exit begins.
                if !self.graph.as_ref().is_some_and(|g| g.waiting()) {
                    *t -= dt;
                }
                if *t <= 0.0 || self.out_of_furniture {
                    self.stand_up(w);
                }
                true
            }
            State::Use(t) => {
                *t -= dt;
                let remaining = *t;
                if self.step_sub_idle(dt, &mut w.clips) {
                    return false;
                }
                if remaining > 0.0 && !self.leave {
                    // Now and then pick an idle to play in the chair (the engine
                    // walks the idle tree, then calls `start_sub_idle`).
                    if self.seat.as_ref().is_some_and(|s| s.kind == Use::Sit && !s.furniture.is_null()) {
                        self.next_idle -= dt;
                        if self.next_idle <= 0.0 {
                            self.next_idle = uniform(w.rand, 5.0, 12.0);
                            self.wants_idle = true;
                        }
                    }
                    return false;
                }
                self.leave = false;
                // A one-shot gesture (no loop to hold) has already ended by itself;
                // some (picking up firewood) hold their last pose in the graph's
                // furniture state until scripts and inventory move them on, so the
                // graph goes back to its default state.
                if self.seat.as_ref().is_some_and(|s| !s.clips.idle_loops) {
                    if let Some(g) = &mut self.graph {
                        g.send_event("IdleForceDefaultState");
                    }
                    self.stand_up(w);
                } else {
                    self.begin_exit(w);
                }
                false
            }
        }
    }

    /// Back on the floor after using furniture.
    fn stand_up(&mut self, w: &mut World) {
        log::debug!("{} stood up at {:?}", self.ref_id, self.pos);
        self.give_up_seat(w.furniture);
        self.objects_changed |= !self.objects.is_empty();
        self.objects.clear();
        if let Some(z) = w.nav.height_at(self.pos) {
            self.pos.z = z;
        }
        self.state = State::Idle(uniform(w.rand, 2.0, 6.0));
    }

    /// Brought down in furniture (bleeding out): out of it at once, onto the floor
    /// where getting in began (clear of the bench or chair), without the exit
    /// animation.
    pub(crate) fn fall_out_of_furniture(&mut self, furniture: &mut FurnitureWorld, nav: &nav::NavWorld) {
        if !self.in_furniture() {
            return;
        }
        if let Some(seat) = &self.seat {
            let (start, _) = seat.enter_start();
            log::debug!("{} drops out of {}: seat {:?} heading {:.0}, was at {:?}, enter start {:?}", self.ref_id, seat.furniture, seat.pos, seat.heading.to_degrees(), self.pos, start);
            self.pos = start.with_z(self.pos.z);
        }
        self.give_up_seat(furniture);
        self.leave = false;
        self.objects_changed |= !self.objects.is_empty();
        self.objects.clear();
        if let Some(z) = nav.height_at(self.pos) {
            self.pos.z = z;
        }
        self.state = State::Idle(0.0);
    }

    /// Choose what to do next from the current goal.
    fn plan(&mut self, w: &mut World) {
        // VRM_AI_NO_SNAP: walk into furniture even on cell load (to watch enter animations).
        let fresh = std::mem::take(&mut self.fresh) && std::env::var_os("VRM_AI_NO_SNAP").is_none();
        self.give_up_seat(w.furniture);
        // Actors standing about now and then play one of their idles first
        // (creatures more often: they have no furniture to use).
        let humanoid = self.graph.as_ref().is_some_and(|g| g.project().humanoid());
        let wandering = self.goal.is_none_or(|g| matches!(g.behaviour, Behaviour::Sandbox | Behaviour::Hold));
        let odds = if humanoid { 4 } else { 3 };
        if self.graph.is_some() && wandering && !fresh && (w.rand)() % odds == 0 {
            self.wants_action_idle = true;
            self.state = State::Idle(uniform(w.rand, 1.0, 3.0));
            return;
        }
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
                self.maybe_eat(w);
            }
        }
    }

    /// Now and then a sandboxing actor pausing on its feet has a bite or a drink.
    fn maybe_eat(&mut self, w: &mut World) {
        let sandbox = self.goal.is_some_and(|g| g.behaviour == Behaviour::Sandbox && g.allow.eating);
        // Meals are the humanoid graphs' (creatures graze through their own idles).
        let humanoid = self.graph.as_ref().is_some_and(|g| g.project().humanoid());
        self.wants_meal = sandbox && humanoid && (w.rand)() % 4 == 0;
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
            let Some(project) = self.graph.as_ref().map(|g| g.project().clone()) else { return false };
            let ways = furniture::ways_to_use(f, mi, &marker, self.child, &project, &self.skeleton_path, self.female, &self.skeleton, &mut w.clips, &mut *w.rand);
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
                kind: marker.kind,
                anim_type: marker.anim_type,
                entry,
                pos: marker.pos,
                heading: marker.heading,
                clips: Arc::new(clips),
                duration: secs,
            };
            if fresh {
                // Start out in it: play the way in at once, out of sight.
                if !self.graph_event(&seat.clips.event, &mut w.clips) {
                    continue;
                }
                if let Some(g) = &mut self.graph {
                    g.set_variable("isInFurniture", 1.0);
                    let settle = seat.clips.enter_time() + 1.0;
                    let mut t = 0.0;
                    while t < settle {
                        g.update(0.25, w.clips.vfs, w.clips.anims, &self.skeleton);
                        t += 0.25;
                    }
                }
                w.furniture.reserve(fid, mi, self.ref_id);
                self.seat = Some(seat);
                self.settle(secs);
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

    /// Walking speed: the movement type's, else the graph's or walk clip's root
    /// motion, so feet don't slide.
    pub fn walk_speed(&self) -> f32 {
        if let Some(v) = self.moves.map(|(m, _)| m.walk).filter(|v| *v > 1.0).or(self.graph_walk_speed) {
            return v;
        }
        self.walk
            .as_ref()
            .and_then(|w| w.motion.as_ref().map(|m| m.end().0.truncate().length() / w.duration()))
            .filter(|v| (20.0..400.0).contains(v))
            .unwrap_or(WALK_SPEED)
    }

    /// Foot placement: the ground under each leg's ankle (as the graph left them
    /// last update), from rays through the static world. Off in furniture, where
    /// the animation puts the feet.
    fn find_ground(&mut self, physics: &crate::physics::Physics) {
        let model = self.transform();
        let (feet, heading, scale) = (self.pos, self.heading, self.scale);
        let on = !self.in_furniture();
        let Some(g) = self.graph.as_mut() else { return };
        let Some(ik) = g.project().shared.project.character.as_ref().and_then(|c| c.foot_ik.clone()) else { return };
        g.foot_ik = on;
        if !on {
            return;
        }
        let ground = g
            .ankles
            .iter()
            .map(|ankle| {
                let w = model.transform_point3(*ankle);
                let from = Vec3::new(w.x, w.y, feet.z + ik.raycast_up * scale);
                let (toi, normal) = physics.ground_ray(from, -Vec3::Z, (ik.raycast_up + ik.raycast_down) * scale)?;
                // Into model space: relative to the feet, unscaled, unturned.
                let z = (from.z - toi - feet.z) / scale;
                Some((z, Quat::from_rotation_z(heading) * normal))
            })
            .collect();
        g.ground = ground;
    }

    /// Speed for the package's gait: walking at the graph's walk, running at its
    /// movement type's run, sneaking at its sneaking movement type's walk.
    pub fn move_speed(&self) -> f32 {
        let walk = self.walk_speed();
        let (gait, sneak) = self.goal.map_or((package::Gait::Walk, false), |g| (g.gait, g.sneak));
        if sneak {
            return self.sneak_moves.map_or(walk * 0.75, |(m, _)| if gait == package::Gait::Run { m.run } else { m.walk });
        }
        let run = self.moves.map(|(m, _)| m.run).filter(|r| *r > walk).unwrap_or(walk * 3.0);
        match gait {
            package::Gait::Walk => walk,
            package::Gait::FastWalk => walk + (run - walk) * 0.25,
            package::Gait::Jog => (walk + run) * 0.5,
            package::Gait::Run => run,
        }
    }

    /// Turn rate (radians per second) from the movement type, walking or standing.
    fn turn_rate(&self) -> f32 {
        let m = if self.goal.is_some_and(|g| g.sneak) { self.sneak_moves.or(self.moves) } else { self.moves };
        let r = m.map(|(m, _)| if self.speed > 5.0 { m.turn_moving } else { m.turn_walk });
        r.filter(|r| *r > 0.1).unwrap_or(TURN_RATE)
    }

    pub fn animate(&mut self, dt: f32, clips: &mut Clips) -> Option<Vec<Mat4>> {
        if self.graph.is_some() {
            return Some(self.animate_graph(dt, clips));
        }
        let walk_speed = self.walk_speed();
        let walking = self.speed > 5.0;
        let anim = self.anim.as_mut()?;
        match (walking, &self.walk, &self.idle) {
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
        Some(anim.update(&self.skeleton, dt))
    }

    /// Drive the behaviour graph from the AI state, run it, and follow its root
    /// motion while in furniture. Anim objects come and go with its events.
    fn animate_graph(&mut self, dt: f32, clips: &mut Clips) -> Vec<Mat4> {
        let walking = self.speed > 5.0 && matches!(self.state, State::Walk { .. } | State::Approach(_));
        let furniture = self.in_furniture() && self.seat.as_ref().is_some_and(|s| !s.furniture.is_null());
        // Turn rate in degrees per second, counter-clockwise (left) positive as Havok
        // has it; headings turn clockwise.
        let turned = if self.graph_heading.is_finite() { wrap_angle(self.heading - self.graph_heading) } else { 0.0 };
        self.graph_heading = self.heading;
        let turn_delta = if dt > 1e-4 { -turned.to_degrees() / dt } else { 0.0 };
        let still = !walking && !self.in_furniture();
        let model = self.transform();
        let sneak = self.goal.is_some_and(|g| g.sneak) && !self.in_furniture();
        let state = if sneak { self.sneak_moves.or(self.moves) } else { self.moves }.map(|(_, s)| s);
        let g = self.graph.as_mut().expect("checked");
        if sneak != self.sneaking {
            g.send_event(if sneak { "SneakStart" } else { "SneakStop" });
            self.sneaking = sneak;
        }
        if let Some(s) = state {
            g.set_variable("iState", s);
        }
        // Head tracking (humanoids: their graph's look-at follows bHeadTracking).
        let look = self.look_at.filter(|_| g.project().humanoid());
        g.set_variable("bHeadTracking", if look.is_some() { 1.0 } else { 0.0 });
        g.look_target = look.map(|w| model.inverse().transform_point3(w));
        g.set_variable("Speed", if walking { self.speed } else { 0.0 });
        g.set_variable("TurnDelta", turn_delta);
        // Standing still and turning: the graph's turn-in-place loops (creatures).
        let turning = match (still, turn_delta) {
            (true, d) if d > 20.0 => 1,
            (true, d) if d < -20.0 => -1,
            _ => 0,
        };
        if turning != self.turning {
            g.send_event(match turning {
                1 => "turnLeft",
                -1 => "turnRight",
                _ => "turnStop",
            });
            self.turning = turning;
        }
        g.set_variable("isInFurniture", if furniture { 1.0 } else { 0.0 });
        if walking != self.moving {
            g.send_event(if walking { "moveStart" } else { "moveStop" });
            if walking {
                // Walking, not sprinting (horses' locomotion starts out in its sprint).
                g.send_event("SprintStop");
            }
            self.moving = walking;
        }
        let frame = g.update(dt, clips.vfs, clips.anims, &self.skeleton);
        if self.in_furniture() {
            let (t, yaw) = frame.motion;
            self.pos += furniture::to_world(t * self.scale, self.heading);
            self.heading -= yaw;
        }
        for r in frame.raised {
            let e = r.event.to_ascii_lowercase();
            if self.combat.is_some() && log::log_enabled!(target: "combat_events", log::Level::Trace) {
                log::trace!(target: "combat_events", "{} raised {}{}", self.ref_id, r.event, r.payload.as_deref().map(|p| format!(" ({p})")).unwrap_or_default());
            }
            match e.as_str() {
                "animobjdraw" => {
                    if let Some(anio) = r.payload
                        && !self.objects.iter().any(|o| o.eq_ignore_ascii_case(&anio))
                    {
                        self.objects.push(anio);
                        self.objects_changed = true;
                    }
                }
                "animobjectunequip" => {
                    self.objects_changed |= !self.objects.is_empty();
                    self.objects.clear();
                }
                "idlefurnitureexit" => self.out_of_furniture = true,
                "weapondraw" | "weaponsheathe" => {
                    let out = e == "weapondraw";
                    self.equipment_changed |= out != self.weapon_out;
                    self.weapon_out = out;
                }
                "hitframe" => self.hit_frame = true,
                "soundplay" | "npcsoundplay" => {
                    if let Some(p) = r.payload {
                        self.sounds.push(p);
                    }
                }
                _ => {}
            }
        }
        frame.pose
    }

    /// Play the idle `clips` describe in the furniture, then go back to the seat's
    /// loop. False when the actor isn't settled in, is already playing another
    /// idle, or the graph won't take it.
    pub fn start_sub_idle(&mut self, clips: furniture::UseClips, lib: &mut Clips, rand: &mut dyn FnMut() -> u64) -> bool {
        if !matches!(self.state, State::Use(_)) || self.leave || self.sub.is_some() {
            return false;
        }
        if !self.graph_event(&clips.event, lib) {
            return false;
        }
        // A held idle is stopped after a while; a gesture runs its course.
        self.sub = Some(if clips.idle_loops {
            SubIdle { left: uniform(rand, 8.0, 20.0), stop: clips.exits.clone() }
        } else {
            SubIdle { left: clips.enter_time() + clips.idle.duration() + clips.exit_time() + 0.5, stop: Vec::new() }
        });
        true
    }

    /// Advance the idle being played in the furniture. False when there is none
    /// (any more); leaving the furniture first stops it and lets it wind down.
    fn step_sub_idle(&mut self, dt: f32, clips: &mut Clips) -> bool {
        let waiting = self.graph.as_ref().is_some_and(|g| g.waiting());
        let Some(sub) = &mut self.sub else { return false };
        // A stopped idle's way out may wait for its moment in the graph.
        if !(waiting && sub.stop.is_empty()) {
            sub.left -= dt;
        }
        if self.leave && !sub.stop.is_empty() {
            sub.left = sub.left.min(0.0);
        }
        if sub.left > 0.0 {
            return true;
        }
        // Stop a held idle by the first exit the graph takes, then give its way
        // out a moment before moving on.
        let stops = std::mem::take(&mut sub.stop);
        if !stops.is_empty() {
            let _ = stops.iter().any(|e| self.graph_event(e, clips));
            if let Some(sub) = &mut self.sub {
                sub.left = 1.0;
            }
            return true;
        }
        self.sub = None;
        false
    }
}

/// A small random number generator seeded from the engine's.
/// `a` in -pi..pi.
fn wrap_angle(a: f32) -> f32 {
    let a = a.rem_euclid(std::f32::consts::TAU);
    if a > std::f32::consts::PI { a - std::f32::consts::TAU } else { a }
}

fn xorshift(seed: u64) -> impl FnMut() -> u64 {
    let mut seed = seed | 1;
    move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    }
}

impl Engine {
    /// Play behaviour `event` on a loaded humanoid actor (Papyrus `PlayIdle` /
    /// `Debug.SendAnimationEvent`). Idle-stopping events return it to its AI.
    pub fn play_animation_event(&mut self, actor: FormId, event: &str) -> bool {
        self.play_idle(actor, event, None)
    }

    /// [`Engine::play_animation_event`], leaving a looping idle after `secs` (it is
    /// otherwise held until the AI moves on). True when the actor's graph took it.
    fn play_idle(&mut self, actor: FormId, event: &str, secs: Option<f32>) -> bool {
        let Some(key) = self.actor_cells.get(&actor).copied() else { return false };
        let Some(rt) = self.cells.get_mut(&key) else { return false };
        let Some(a) = rt.actors.iter_mut().find(|a| a.ref_id == actor) else { return false };
        let lower = event.to_ascii_lowercase();
        if matches!(lower.as_str(), "idlestop" | "idlestopinstant" | "idleforcedefaultstate" | "idlechairexitstart" | "idlefurnitureexit") {
            if a.in_furniture() {
                a.leave = true;
            } else if let Some(g) = &mut a.graph {
                g.send_event(event);
            }
            return true;
        }
        let Some(project) = a.graph.as_ref().map(|g| g.project().clone()) else { return false };
        let mut clips = Clips { vfs: &self.vfs, anims: &mut self.anims, behaviors: &mut self.behaviors };
        // Seated, the event is one of the seat's own idles (`idleChairArmsCrossedVar1`).
        if matches!(a.state, State::Use(_)) && a.seat.as_ref().is_some_and(|s| s.kind != Use::Idle) {
            let Some(c) = clips.event_with_exits(event, &SUB_IDLE_EXITS, &project, &a.skeleton_path, a.female, &a.skeleton) else { return false };
            let started = a.start_sub_idle(c, &mut clips, &mut xorshift(self.rng));
            if started {
                log::debug!("{actor} plays {event} in its seat");
            }
            return started;
        }
        // Planning: where the way in leaves the actor, how long it lasts.
        let Some(c) = clips.event(event, &project, &a.skeleton_path, a.female, &a.skeleton) else { return false };
        if !a.graph_event(event, &mut clips) {
            log::debug!("{actor}: the graph won't take {event}");
            return false;
        }
        self.furniture.release(a.ref_id);
        let mut pos = a.pos;
        let mut heading = a.heading;
        for m in c.enter.iter().filter_map(|c| c.motion.as_ref()) {
            let (t, yaw) = m.end();
            pos += furniture::to_world(t, heading);
            heading -= yaw;
        }
        // A one-shot gesture (no loop at the end) is played through once.
        let duration = if c.idle_loops { secs.unwrap_or(f32::INFINITY) } else { c.idle.duration() };
        log::debug!("{actor} plays {event} ({:.1} s in, {:.1} s out)", c.enter_time(), c.exit_time());
        let enter = c.enter_time();
        a.sub = None;
        a.seat = Some(Seat { furniture: FormId::NULL, kind: Use::Idle, anim_type: 0, entry: furniture::Entry::Front, pos, heading, clips: Arc::new(c), duration });
        a.state = State::Enter(enter);
        a.leave = false;
        true
    }

    /// Idles for actors that asked for one, from the `ActionIdle` tree's branches
    /// for their graphs (hands on hips, a sigh; a horse grazes, a chicken settles
    /// down to sit).
    fn play_action_idles(&mut self) {
        use crate::condition::{Context, IdleQuery};
        let wanting: Vec<(FormId, Arc<ProjectRuntime>)> = self
            .cells
            .values_mut()
            .flat_map(|rt| {
                rt.actors.iter_mut().filter_map(|a| std::mem::take(&mut a.wants_action_idle).then(|| Some((a.ref_id, a.graph.as_ref()?.project().clone()))).flatten())
            })
            .collect();
        if wanting.is_empty() {
            return;
        }
        let Some(root) = self.idles.get_or_insert_with(|| idles::IdleIndex::build(&self.lo)).find(&self.lo, "ActionIdle") else {
            log::warn!("no ActionIdle in the idle tree");
            return;
        };
        for (r, project) in wanting {
            // Standing still, out of furniture.
            let ctx = Context { subject: Some(r), idle: Some(IdleQuery::default()), ..Default::default() };
            let Some((idle, event)) = self.idles.as_ref().and_then(|ix| ix.select_for(self, root, ctx, &project)) else {
                log::debug!("{r}: no idle in ActionIdle for {}", project.name);
                continue;
            };
            let secs = 6.0 + (self.rand() % 1400) as f32 / 100.0;
            if self.play_idle(r, &event, Some(secs)) {
                log::debug!("{r} idles: {idle} ({event}) for up to {secs:.0} s");
            }
        }
    }

    /// Standing meals for sandboxing actors that asked for one: the idle tree's
    /// `EatingRoot` / `DrinkingRoot` pick bread, an apple, a tankard...
    fn play_standing_meals(&mut self) {
        use crate::condition::{Context, IdleQuery};
        let wanting: Vec<FormId> = self
            .cells
            .values_mut()
            .flat_map(|rt| rt.actors.iter_mut().filter_map(|a| std::mem::take(&mut a.wants_meal).then_some(a.ref_id)))
            .collect();
        if wanting.is_empty() {
            return;
        }
        let Some(ix) = self.idles.as_ref() else { return };
        let roots: Vec<FormId> = ["EatingRoot", "DrinkingRoot"].iter().filter_map(|n| ix.find(&self.lo, n)).collect();
        for r in wanting {
            let query = IdleQuery { eating: true, ..Default::default() };
            let ctx = Context { subject: Some(r), idle: Some(query), ..Default::default() };
            let Some((idle, event)) = self.idles.as_ref().and_then(|ix| ix.select_among(self, &roots, ctx)) else { continue };
            let secs = 10.0 + (self.rand() % 1500) as f32 / 100.0;
            if self.play_idle(r, &event, Some(secs)) {
                log::debug!("{r} has a standing meal: {idle} ({event}) for {secs:.0} s");
            }
        }
    }

    /// Pick idles from the idle tree (`NonCombatIdles`) for actors in furniture that
    /// asked for one: eating and drinking when their package allows it, the odd
    /// change of sitting pose.
    fn play_furniture_idles(&mut self) {
        use crate::condition::{Context, IdleQuery};
        let wanting: Vec<(crate::render::CellKey, FormId)> = self
            .cells
            .iter_mut()
            .flat_map(|(k, rt)| rt.actors.iter_mut().filter_map(move |a| std::mem::take(&mut a.wants_idle).then_some((*k, a.ref_id))))
            .collect();
        if wanting.is_empty() {
            return;
        }
        let Some(root) = self.idles.as_ref().and_then(|ix| ix.find(&self.lo, "NonCombatIdles")) else { return };
        let special = self.lo.find_editor_id("FurnitureSpecial");
        for (key, r) in wanting {
            let roll = self.rand();
            let Some(a) = self.cells.get(&key).and_then(|rt| rt.actors.iter().find(|a| a.ref_id == r)) else { continue };
            let Some(seat) = a.seat.clone() else { continue };
            // Wood piles, pour spots and the like are "sat in" too, but aren't seats.
            if special.is_some_and(|kw| self.has_keyword(seat.furniture, kw)) {
                continue;
            }
            let Some(project) = a.graph.as_ref().map(|g| g.project().clone()).filter(|p| p.humanoid()) else { continue };
            let query = IdleQuery {
                anim_type: seat.anim_type,
                entry: seat.entry.entry_type(),
                state: 3.0,
                // Sandboxing actors eat now and then (food or drink in hand); the
                // rest of the time they sit, with the odd change of pose.
                eating: a.goal.is_some_and(|g| g.allow.meal || (g.allow.eating && roll % 3 == 0)),
                ..Default::default()
            };
            let ctx = Context { subject: Some(r), target: Some(seat.furniture), idle: Some(query), ..Default::default() };
            let Some((idle, event)) = self.idles.as_ref().and_then(|ix| ix.select(self, root, ctx)) else { continue };
            let (skeleton_path, female, skeleton) = (a.skeleton_path.clone(), a.female, a.skeleton.clone());
            let mut clips = Clips { vfs: &self.vfs, anims: &mut self.anims, behaviors: &mut self.behaviors };
            let Some(c) = clips.event_with_exits(&event, &SUB_IDLE_EXITS, &project, &skeleton_path, female, &skeleton) else {
                log::debug!("{r}: no clips for furniture idle {idle} ({event})");
                continue;
            };
            drop(clips);
            log::debug!("{r} plays {event} in {} ({} enter, {} exit clips)", seat.furniture, c.enter.len(), c.exit.len());
            let mut rand = xorshift(self.rand());
            let mut clips = Clips { vfs: &self.vfs, anims: &mut self.anims, behaviors: &mut self.behaviors };
            if let Some(a) = self.cells.get_mut(&key).and_then(|rt| rt.actors.iter_mut().find(|a| a.ref_id == r)) {
                a.start_sub_idle(c, &mut clips, &mut rand);
            }
        }
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

    /// Send an actor to `to` at a gait (testing): the goal is pinned, so its
    /// packages no longer replace it.
    pub fn travel_to(&mut self, actor: FormId, to: Vec3, gait: package::Gait, sneak: bool) -> bool {
        let Some(key) = self.actor_cells.get(&actor).copied() else { return false };
        let Some(a) = self.cells.get_mut(&key).and_then(|rt| rt.actors.iter_mut().find(|a| a.ref_id == actor)) else { return false };
        a.interrupt(&mut self.furniture);
        a.goal = Some(Goal { gait, sneak, ..Goal::travel(to) });
        a.pinned = true;
        true
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

    /// Active states of a loaded actor's behaviour graph, outermost first.
    /// A behaviour graph variable of a loaded actor (0 for one its graph lacks);
    /// `None` when it has no graph.
    pub fn graph_variable(&self, r: FormId, name: &str) -> Option<f32> {
        let key = self.actor_cells.get(&r)?;
        let a = self.cells.get(key)?.actors.iter().find(|a| a.ref_id == r)?;
        Some(a.graph.as_ref()?.variable(name).unwrap_or(0.0))
    }

    /// How fast a loaded actor is moving.
    pub fn actor_speed(&self, r: FormId) -> Option<f32> {
        let key = self.actor_cells.get(&r)?;
        Some(self.cells.get(key)?.actors.iter().find(|a| a.ref_id == r)?.speed)
    }

    /// Gesture along with a line of dialogue: an idle from the `ActionTalking`
    /// tree for the speaker's graph (hands on hips, expressive or angry gestures,
    /// by the line's emotion and the pose the speaker is in).
    pub(crate) fn talking_gesture(&mut self, actor: FormId) {
        use crate::condition::{Context, IdleQuery};
        let Some(key) = self.actor_cells.get(&actor).copied() else { return };
        let Some(a) = self.cells.get(&key).and_then(|rt| rt.actors.iter().find(|a| a.ref_id == actor)) else { return };
        let Some(project) = a.graph.as_ref().map(|g| g.project().clone()) else { return };
        let query = match &a.seat {
            Some(s) if a.in_furniture() => IdleQuery { anim_type: s.anim_type, entry: s.entry.entry_type(), state: 3.0, ..Default::default() },
            _ => IdleQuery::default(),
        };
        let target = a.seat.as_ref().map(|s| s.furniture).filter(|f| !f.is_null());
        let Some(root) = self.idles.get_or_insert_with(|| idles::IdleIndex::build(&self.lo)).find(&self.lo, "ActionTalking") else { return };
        let ctx = Context { subject: Some(actor), target, idle: Some(query), ..Default::default() };
        let Some((idle, event)) = self.idles.as_ref().and_then(|ix| ix.select_for(self, root, ctx, &project)) else {
            log::debug!("{actor}: no talking idle");
            return;
        };
        let mut clips = Clips { vfs: &self.vfs, anims: &mut self.anims, behaviors: &mut self.behaviors };
        let Some(a) = self.cells.get_mut(&key).and_then(|rt| rt.actors.iter_mut().find(|a| a.ref_id == actor)) else { return };
        let took = a.graph_event(&event, &mut clips);
        log::debug!("{actor} talks with {idle} ({event}){}", if took { "" } else { ": the graph won't take it" });
    }

    /// The conversation is over: standing speakers stop their dialogue idle.
    pub(crate) fn end_talking_gestures(&mut self, actor: FormId) {
        let Some(key) = self.actor_cells.get(&actor).copied() else { return };
        let Some(a) = self.cells.get_mut(&key).and_then(|rt| rt.actors.iter_mut().find(|a| a.ref_id == actor)) else { return };
        if !a.in_furniture()
            && let Some(g) = &mut a.graph
        {
            g.send_event("IdleStop");
        }
    }

    /// `GetSitting` / `GetSleeping` of a loaded actor: 0 not, 2 getting in, 3 in,
    /// 4 getting out (beds count as sleeping, other furniture as sitting).
    pub fn sit_sleep_state(&self, r: FormId, sleeping: bool) -> f32 {
        let Some(a) = self.actor_cells.get(&r).and_then(|k| self.cells.get(k)).and_then(|rt| rt.actors.iter().find(|a| a.ref_id == r)) else { return 0.0 };
        let Some(seat) = &a.seat else { return 0.0 };
        if (seat.kind == Use::Sleep) != sleeping || matches!(seat.kind, Use::Idle) {
            return 0.0;
        }
        match a.state {
            State::Enter(_) => 2.0,
            State::Use(_) => 3.0,
            State::Exit(_) => 4.0,
            _ => 0.0,
        }
    }

    /// Set a behaviour graph variable of a loaded actor; false if it has no graph.
    pub fn set_graph_variable(&mut self, r: FormId, name: &str, value: f32) -> bool {
        let Some(key) = self.actor_cells.get(&r) else { return false };
        let Some(g) = self.cells.get_mut(key).and_then(|rt| rt.actors.iter_mut().find(|a| a.ref_id == r)).and_then(|a| a.graph.as_mut()) else {
            return false;
        };
        g.set_variable(name, value);
        true
    }

    pub fn graph_states(&self, r: FormId) -> Option<Vec<String>> {
        let key = self.actor_cells.get(&r)?;
        let a = self.cells.get(key)?.actors.iter().find(|a| a.ref_id == r)?;
        Some(a.graph.as_ref()?.active_states())
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
                let ctx = crate::condition::Context { subject: Some(a.ref_id), ..Default::default() };
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
                        gait: p.gait,
                        sneak: p.sneak,
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
            if a.pinned || a.dead {
                continue;
            }
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
        let talking = self.conversation.as_ref().map(|c| c.npc_ref).or(self.barks.current.as_ref().map(|b| b.speaker));
        let player = self.ref_position(PLAYER_REF).unwrap_or_default();
        let eye = self.player.eye();
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
        let mut equip = Vec::new();
        let mut sounds: Vec<(String, Vec3)> = Vec::new();
        let mut swings: Vec<combat::Swing> = Vec::new();
        // Attacks started this frame (attacker, target): their targets may block.
        let mut started: Vec<(FormId, FormId)> = Vec::new();
        let mut lost: Vec<FormId> = Vec::new();
        // Where everyone is, for fights.
        let mut positions: std::collections::HashMap<FormId, Vec3> =
            self.cells.values().flat_map(|rt| &rt.actors).filter(|a| !a.dead).map(|a| (a.ref_id, a.pos)).collect();
        if self.player_died_at.is_none() {
            // The player's feet.
            let feet = self.player.position - Vec3::Z * (self.physics.player_half_height + self.physics.player_radius);
            positions.insert(PLAYER_REF, feet);
        }
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
                if a.dead {
                    if let Some((rd, mapping)) = &a.ragdoll {
                        inst.pose = mapping.pose(&a.skeleton, &self.physics.ragdoll_bodies(rd));
                    }
                    if std::mem::take(&mut a.objects_changed) {
                        held.push((*key, index, Vec::new()));
                    }
                    continue;
                }
                if talking == Some(a.ref_id) {
                    // Seated actors talk from where they are.
                    if !a.in_furniture() {
                        a.halt(3.0);
                        let d = player - a.pos;
                        a.turn_towards(d.normalize_or_zero(), dt);
                    }
                } else if !self.ai_enabled || a.bleeding.is_some() {
                    a.halt(1.0);
                } else if let Some(target) = a.combat.as_ref().map(|c| c.target) {
                    match positions.get(&target).filter(|p| p.distance(a.pos) < combat::LOSE_DISTANCE) {
                        Some(&tp) => {
                            if let Some(ev) = a.combat_step(dt, &mut world, tp) {
                                // Attacks the graph has no state for fall back to the basic one.
                                let took = a.graph_event(&ev, &mut world.clips) || a.graph_event("attackStart", &mut world.clips);
                                log::debug!("{} swings: {ev}{}", a.ref_id, if took { "" } else { " (the graph won't take it)" });
                                if !took {
                                    // Its graph isn't readied (a stagger cut the draw
                                    // short, say): draw again.
                                    a.drawn = false;
                                    if let Some(c) = a.combat.as_mut() {
                                        c.refused();
                                    }
                                }
                                started.push((a.ref_id, target));
                            }
                        }
                        None => lost.push(a.ref_id),
                    }
                } else {
                    let before = a.pos;
                    if a.step(dt, &mut world) && a.is_walking() {
                        a.keep_clear(&bodies, &nav, dt, before);
                    }
                }
                // NPCs look at the player close by, and while talking to them.
                let near = a.pos.distance(player) < HEAD_TRACK_DISTANCE;
                a.look_at = (near || talking == Some(a.ref_id)).then_some(eye);
                a.find_ground(&self.physics);
                inst.transform = a.transform();
                if let Some(pose) = a.animate(dt, &mut world.clips) {
                    inst.pose = pose;
                }
                if std::mem::take(&mut a.objects_changed) {
                    log::debug!("{} holds {:?}", a.ref_id, a.objects);
                    held.push((*key, index, a.objects.clone()));
                }
                if std::mem::take(&mut a.equipment_changed) {
                    equip.push((*key, index, a.ref_id));
                }
                if std::mem::take(&mut a.hit_frame)
                    && let Some(c) = a.combat.as_mut()
                    && !std::mem::replace(&mut c.struck, true)
                {
                    swings.push(combat::Swing { attacker: a.ref_id, target: c.target, attack: c.attack, pos: a.pos, heading: a.heading });
                }
                for s in a.sounds.drain(..) {
                    sounds.push((s, a.pos + Vec3::Z * 64.0 * a.scale));
                }
                if a.is_walking() || matches!(a.state, State::Approach(_) | State::Enter(_) | State::Exit(_) | State::Use(_)) || talking == Some(a.ref_id) {
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
        for (key, index, actor) in equip {
            self.refresh_equipment(key, index, actor);
        }
        for r in lost {
            self.end_combat(r);
        }
        self.update_guards(dt, &started);
        self.resolve_swings(swings);
        self.update_bleedouts(dt);
        if self.ai_enabled {
            self.detect_enemies(dt);
        }
        for (sound, at) in sounds {
            self.play_sound_at(&sound, at);
        }
        self.play_furniture_idles();
        self.play_standing_meals();
        self.play_action_idles();
        self.update_torches();
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
