//! Actor AI: package selection, sandboxing / travelling over the navmesh.

pub mod nav;
pub mod package;

use std::sync::Arc;

use esp::FormId;
use glam::{Mat4, Quat, Vec3};

use crate::engine::{Engine, PLAYER_REF};
use crate::world::animation::{ActorAnim, BoundClip};
use crate::world::skeleton::Skeleton;
use package::{Behaviour, LocationKind, Package};

/// Forward walk speed (units/s) from the default NPC movement type.
pub const WALK_SPEED: f32 = 80.0;
/// Turn rate while walking, radians/s.
const TURN_RATE: f32 = 4.0;
/// Real seconds between package re-evaluations.
const EVAL_INTERVAL: f32 = 3.0;
/// Sandbox wander radius clamp.
const SANDBOX_MIN: f32 = 200.0;
const SANDBOX_MAX: f32 = 1200.0;
/// Longest path an actor will set out on (avoids crossing half the map).
const MAX_PATH: f32 = 6000.0;

/// Where and how the current package wants the actor to be.
#[derive(Debug, Clone, Copy)]
pub struct Goal {
    pub behaviour: Behaviour,
    pub centre: Vec3,
    pub radius: f32,
}

#[derive(Debug)]
enum State {
    Idle(f32),
    Walk { path: Vec<Vec3>, next: usize, budget: f32 },
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
            state: State::Idle(1.0 + stagger),
            next_eval: stagger,
            speed: 0.0,
        }
    }

    pub fn transform(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(Vec3::splat(self.scale), Quat::from_rotation_z(-self.heading), self.pos)
    }

    pub fn is_walking(&self) -> bool {
        matches!(self.state, State::Walk { .. })
    }

    /// Stop and stand (e.g. when spoken to).
    pub fn halt(&mut self, idle: f32) {
        self.state = State::Idle(idle);
        self.speed = 0.0;
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

    /// Advance movement. Returns true when the actor moved.
    pub fn step(&mut self, dt: f32, nav: &nav::NavWorld, rand: &mut impl FnMut() -> u64) -> bool {
        match &mut self.state {
            State::Idle(t) => {
                self.speed = 0.0;
                *t -= dt;
                if *t > 0.0 {
                    return false;
                }
                self.plan(nav, rand);
                false
            }
            State::Walk { path, next, budget } => {
                *budget -= dt;
                if *budget < 0.0 || *next >= path.len() {
                    log::debug!("{} walk ended ({})", self.ref_id, if *budget < 0.0 { "timeout" } else { "arrived" });
                    self.state = State::Idle(4.0 + (rand() % 1100) as f32 / 100.0);
                    self.speed = 0.0;
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
                let speed = WALK_SPEED * remaining.cos().max(0.0).powi(2);
                self.speed = speed;
                let fwd = Vec3::new(self.heading.sin(), self.heading.cos(), 0.0);
                let mut p = self.pos + fwd * (speed * dt).min(dist);
                let z_hint = Vec3::new(p.x, p.y, self.pos.z);
                p.z = nav.height_at(z_hint).unwrap_or_else(|| {
                    // Interpolate towards the waypoint height off the mesh.
                    self.pos.z + (target.z - self.pos.z) * ((speed * dt) / dist).min(1.0)
                });
                self.pos = p;
                true
            }
        }
    }

    /// Choose what to do next from the current goal.
    fn plan(&mut self, nav: &nav::NavWorld, rand: &mut impl FnMut() -> u64) {
        let idle = |r: &mut dyn FnMut() -> u64, lo: f32, hi: f32| lo + (r() % 1000) as f32 / 1000.0 * (hi - lo);
        let Some(goal) = self.goal else {
            self.state = State::Idle(idle(rand, 5.0, 10.0));
            return;
        };
        let target = match goal.behaviour {
            Behaviour::Hold => None,
            Behaviour::Travel => {
                let near = goal.radius.max(96.0);
                (self.pos.truncate().distance(goal.centre.truncate()) > near).then_some(goal.centre)
            }
            Behaviour::Sandbox => {
                // Return to the sandbox area first if we've strayed from it.
                let r = goal.radius.clamp(SANDBOX_MIN, SANDBOX_MAX);
                if self.pos.truncate().distance(goal.centre.truncate()) > r * 1.5 {
                    Some(goal.centre)
                } else {
                    nav.random_point(goal.centre, r, &mut *rand)
                }
            }
        };
        let path = target.and_then(|t| nav.find_path(self.pos, t));
        match path {
            Some(path) if !path.is_empty() => {
                let len: f32 = std::iter::once(self.pos).chain(path.iter().copied()).collect::<Vec<_>>().windows(2).map(|w| w[0].distance(w[1])).sum();
                if len > MAX_PATH {
                    log::debug!("{} path too long ({len:.0})", self.ref_id);
                    self.state = State::Idle(idle(rand, 10.0, 20.0));
                    return;
                }
                log::debug!("{} walking {:.0} units via {} points ({:?})", self.ref_id, len, path.len(), goal.behaviour);
                self.state = State::Walk { path, next: 0, budget: len / WALK_SPEED * 2.5 + 5.0 };
            }
            _ => {
                if target.is_some() {
                    log::debug!("{} no path to {:?}", self.ref_id, target);
                }
                self.state = State::Idle(idle(rand, 6.0, 16.0));
            }
        }
    }

    /// Pick the idle or walk clip to match the current speed.
    pub fn animate(&mut self, dt: f32) -> Option<Vec<Mat4>> {
        let walking = self.speed > 5.0;
        let anim = self.anim.as_mut()?;
        match (walking, &self.walk, &self.idle) {
            (true, Some(w), _) => {
                anim.play(w.clone(), 0.25);
                anim.speed = (self.speed / WALK_SPEED).clamp(0.3, 1.5);
            }
            (false, _, Some(i)) => {
                anim.play(i.clone(), 0.35);
                anim.speed = 1.0;
            }
            _ => {}
        }
        Some(anim.update(&self.skeleton, dt))
    }
}

impl Engine {
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
                if a.next_eval - dt > 0.0 {
                    continue;
                }
                let ctx = crate::condition::Context { subject: Some(a.ref_id), target: None, quest: None };
                let pick = a.packages.iter().position(|p| {
                    p.schedule.matches(self.hour, self.day) && crate::condition::evaluate(self, &p.conditions, ctx)
                });
                let goal = pick.map(|pi| {
                    let p = &a.packages[pi];
                    let (centre, radius) = self.package_target(a, p);
                    Goal { behaviour: p.behaviour, centre, radius }
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
                a.current = pick;
                a.goal = goal;
                a.halt(0.5);
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
        let nav = std::mem::take(&mut self.nav);
        let mut moved = Vec::new();
        for (key, rt) in self.cells.iter_mut() {
            let Some(rc) = self.scene.cells.get_mut(key) else { continue };
            for (inst, a) in rc.actors.iter_mut().zip(rt.actors.iter_mut()) {
                if talking == Some(a.ref_id) {
                    a.halt(3.0);
                    let d = player - a.pos;
                    a.turn_towards(d.normalize_or_zero(), dt);
                } else if !self.ai_enabled {
                    a.halt(1.0);
                } else {
                    a.step(dt, &nav, &mut rand);
                }
                inst.transform = a.transform();
                if let Some(pose) = a.animate(dt) {
                    inst.pose = pose;
                }
                if a.is_walking() || talking == Some(a.ref_id) {
                    moved.push((a.ref_id, a.pos, a.capsule));
                }
            }
        }
        self.nav = nav;
        for (r, pos, capsule) in moved {
            if let Some(c) = capsule {
                self.physics.move_actor_capsule(c, pos);
            }
            self.moved_refs.insert(r, pos);
        }
    }
}
