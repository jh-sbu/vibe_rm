//! Actor AI: package selection, sandboxing / travelling over the navmesh.

pub mod alias;
pub mod archery;
pub mod away;
pub mod casters;
pub mod combat;
mod equipment;
pub mod exec;
pub mod furniture;
mod greet;
pub mod idles;
mod influence;
pub mod magic_practice;
pub mod nav;
pub mod package;
pub mod practice;
pub mod schedule;
mod summons;
pub mod threat;

#[cfg(test)]
mod movement_tests;

use std::sync::Arc;

use esp::FormId;
use glam::{Mat4, Quat, Vec3};

use crate::engine::{Engine, PLAYER_REF};
use crate::world::animation::{ActorAnim, BoundClip};
use crate::world::behavior::{GraphAnim, ProjectRuntime};
use crate::world::inventory::{LEFT, RIGHT};
use crate::world::movement::MoveSpeeds;
use crate::world::skeleton::Skeleton;
use furniture::{FurnitureWorld, Seat, Use};
use package::{Allow, Behaviour, LocationKind, Package, Target};
use schedule::Place;

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
/// Seconds an Activate package's actor stays in furniture with an idle that
/// loops (no source).
const ACTIVATE_USE: (f32, f32) = (3.0, 6.0);
/// Standing actors facing someone turn once they are this far off (radians), and
/// stop within the second.
const FACE_TURN_START: f32 = 0.6;
const FACE_TURN_STOP: f32 = 0.05;

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
    /// A Sit package's target is any furniture (object type `FURN`): leans and
    /// idle markers as well as seats.
    pub any_furniture: bool,
    /// Patrol start or follow target.
    pub target: Option<FormId>,
    pub repeat: bool,
    pub start_nearest: bool,
    /// Follow: (min, max) distance to keep.
    pub follow_radius: (f32, f32),
    /// Escort: how far behind the escorted actor may fall before the escort waits.
    pub escort_wait: f32,
    /// Escort: run while the escorted actor is this far ahead.
    pub escort_run: f32,
    pub gait: package::Gait,
    pub sneak: bool,
    /// UseWeapon: the targets and barrage timing.
    pub practice: Option<practice::PracticeGoal>,
    /// UseMagic: the spell, its target and timing.
    pub magic: Option<magic_practice::MagicGoal>,
    /// Activate: how many times to activate the target (0 for no limit).
    pub activate: u32,
}

/// Keep a scheduled arrival turn only after package evaluation confirms a
/// Travel goal. Before the first evaluation, `None` means "not chosen yet";
/// at the evaluation boundary, `None` means there is no applicable package.
fn arrival_turn_after_package_eval(turn: Option<f32>, goal: Option<Goal>) -> Option<f32> {
    goal.is_some_and(|goal| goal.behaviour == Behaviour::Travel)
        .then_some(turn)
        .flatten()
}

impl Goal {
    pub fn travel(to: Vec3) -> Goal {
        Goal {
            behaviour: Behaviour::Travel,
            centre: to,
            radius: 0.0,
            allow: Allow {
                wandering: false,
                ..package::Allow::default()
            },
            energy: 50.0,
            furniture: None,
            any_furniture: false,
            target: None,
            repeat: false,
            start_nearest: false,
            follow_radius: (0.0, 0.0),
            escort_wait: 0.0,
            escort_run: 0.0,
            gait: package::Gait::Walk,
            sneak: false,
            practice: None,
            magic: None,
            activate: 0,
        }
    }
}

/// Where an actor is in its package's `Package::stops`: the stop, and when it
/// got there (real time; `None`: still on its way).
#[derive(Debug, Clone, Copy, Default)]
struct StopProgress {
    index: usize,
    arrived: Option<f64>,
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
    Walk {
        path: Vec<Vec3>,
        next: usize,
        budget: f32,
        to_seat: bool,
    },
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
    pub fn event(
        &mut self,
        event: &str,
        project: &ProjectRuntime,
        skeleton_path: &str,
        female: bool,
        skeleton: &Skeleton,
    ) -> Option<furniture::UseClips> {
        self.event_with_exits(
            event,
            &["IdleChairExitStart", "AnimObjectIdleStop", "IdleStop"],
            project,
            skeleton_path,
            female,
            skeleton,
        )
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
                .find_map(|p| {
                    self.anims
                        .project_clip(self.vfs, p, skeleton_path, skeleton, c.speed, project)
                })
        };
        let (last, enter) = played.clips.split_last()?;
        let idle = load(last)?;
        let enter = enter.iter().map(&mut load).collect::<Option<Vec<_>>>()?;
        // The exit ends back in the default idle; only its one-shot clips matter.
        let exit = played
            .exit
            .iter()
            .filter(|c| c.mode == ClipMode::SinglePlay)
            .filter_map(&mut load)
            .collect();
        Some(furniture::UseClips {
            event: event.to_owned(),
            exits: exits.iter().map(|e| e.to_string()).collect(),
            enter,
            idle,
            exit,
            idle_loops: last.mode != ClipMode::SinglePlay && !played.loop_once,
            instant_exit: None,
            puts_down: false,
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
    /// Whether an actor holds any of an item (furniture ways for users with or
    /// without a load).
    pub holds: &'a dyn Fn(FormId, FormId) -> bool,
    pub rand: &'a mut dyn FnMut() -> u64,
}

/// Signed angle (radians, -pi..pi) to turn from `heading` to face `dir`.
fn angle_to(heading: f32, dir: Vec3) -> f32 {
    let d = (dir.x.atan2(dir.y) - heading).rem_euclid(std::f32::consts::TAU);
    if d > std::f32::consts::PI {
        d - std::f32::consts::TAU
    } else {
        d
    }
}

fn path_length(from: Vec3, path: &[Vec3]) -> f32 {
    std::iter::once(&from)
        .chain(path)
        .zip(path)
        .map(|(a, b)| a.distance(*b))
        .sum()
}

/// The stage an actor in `state` has reached, with `seat` (furniture,
/// marker) reserved or in use: seated while getting into or using furniture
/// (not an idle played where it stands, which has none).
fn stage_of(state: &State, seat: Option<(FormId, u8)>) -> exec::Stage {
    match (state, seat) {
        (State::Enter(_) | State::Use(_), Some((furniture, marker))) if !furniture.is_null() => {
            exec::Stage::Seated { furniture, marker }
        }
        (State::Walk { .. } | State::Approach(_), _) => exec::Stage::Walking,
        _ => exec::Stage::Standing,
    }
}

fn uniform(rand: &mut dyn FnMut() -> u64, lo: f32, hi: f32) -> f32 {
    lo + (rand() % 1000) as f32 / 1000.0 * (hi - lo)
}

/// An actor's footprint ([`ActorRuntime::footprint`]): the points within
/// `radius` of the segment `a`-`b`, at height `z`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Footprint {
    pub id: FormId,
    pub z: f32,
    pub a: glam::Vec2,
    pub b: glam::Vec2,
    pub radius: f32,
}

/// Runtime state of one spawned actor.
pub struct ActorRuntime {
    pub ref_id: FormId,
    pub npc: FormId,
    pub skeleton: Arc<Skeleton>,
    /// The actor's behaviour graph; actors without one play `anim`.
    pub graph: Option<GraphAnim>,
    actions: idles::LocomotionActions,
    velocity: Vec3,
    /// Ground speed of the graph's walk (its clips' root motion).
    pub graph_walk_speed: Option<f32>,
    /// Movement types of the graph's default and sneaking states, with the
    /// `iState` values that select them.
    pub moves: Option<(MoveSpeeds, f32)>,
    pub sneak_moves: Option<(MoveSpeeds, f32)>,
    /// Worn armor's speed multiplier, refreshed before each AI update. Keep
    /// movement types and clip speeds unmodified for animation playback.
    pub(crate) armor_speed_mult: f32,
    pub anim: Option<ActorAnim>,
    pub idle: Option<Arc<BoundClip>>,
    pub walk: Option<Arc<BoundClip>>,
    pub capsule: Option<rapier3d::prelude::ColliderHandle>,
    /// What it is doing, where it stands and how it is: what outlasts its
    /// unloading.
    pub exec: exec::ActorExec,
    /// How far the collision surface under the feet lies above `pos` (eased): the
    /// body stands on it, as the game's character controller would.
    pub lift: f32,
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
    /// Running whatever the package's gait (an escort fallen behind).
    hurry: bool,
    /// Whom to face while standing (an escort waiting, a follower in range), and
    /// whether it is turning to them.
    face: Option<(FormId, bool)>,
    /// Travel marker heading to face after scheduled placement.
    pub(crate) turn_after_arrival: Option<f32>,
    /// The weapon is in hand (between the graph's `weaponDraw` and `weaponSheathe`),
    /// and whether that changed this frame.
    pub(crate) weapon_out: bool,
    pub(crate) equipment_changed: bool,
    /// Weapons drawn (or being drawn): the graph was sent `WeapEquip`.
    pub(crate) drawn: bool,
    /// Sounds the graph asked for this frame (`SoundPlay` payloads: SNDR editor ids).
    pub(crate) sounds: Vec<String>,
    /// Every event the graph raised this frame, for scripts (`OnAnimationEvent`).
    pub(crate) graph_events: Vec<String>,
    pub(crate) spell_fx_events: Vec<String>,
    /// Furniture it began getting into this frame: it activates it.
    pub(crate) activated: Option<FormId>,
    /// The footstep set of what it wears, and its footstep events (lowercase) the
    /// graph raised this frame.
    pub(crate) footstep_set: Option<Arc<crate::world::footsteps::FootstepSet>>,
    pub(crate) footsteps: Vec<String>,
    /// What it fights with, its weapon's reach (0 unarmed), seconds until it
    /// looks for enemies again, and whether its graph reached a hit frame this
    /// update.
    pub(crate) stats: Arc<combat::CombatStats>,
    /// Combat strength (`threat`), and seconds until it is worked out again.
    pub(crate) strength: f32,
    pub(crate) strength_in: f32,
    /// The combat state scripts last heard of (`OnCombatStateChanged`).
    pub(crate) combat_reported: combat::CombatState,
    pub(crate) weapon_reach: f32,
    /// What its hands fight with (`combat::fighting_hands`).
    pub(crate) hands: [combat::Fights; 2],
    /// Seconds before its stamina starts coming back after being spent;
    /// what a power attack costs before the attack's own multiplier (by weapon
    /// weight).
    pub(crate) stamina_wait: f32,
    pub(crate) stamina_spent: bool,
    pub(crate) power_cost: f32,
    /// Wields a bow (and its draw speed); its graph released the arrow this
    /// frame, and it looses it.
    pub(crate) bow: bool,
    pub(crate) bow_speed: f32,
    /// The spell it fights with (set by the engine each frame while it has
    /// one and the magicka), and its casts to make: one for a
    /// fire-and-forget spell, one each landing of a concentration spell.
    pub(crate) combat_spell: Option<casters::CombatSpell>,
    pub(crate) casts: Option<casters::CombatCast>,
    /// The hand it casts with in its graph while it fights with a spell (set
    /// by the engine: `ActorRuntime::set_hand_vars`); its graph raised the
    /// hands' `SpellFire` and `Equipped` (a release clip over).
    pub(crate) cast_hand: Option<usize>,
    pub(crate) spell_fire: [bool; 2],
    pub(crate) hand_equipped: [bool; 2],
    pub(crate) arrow_release: bool,
    pub(crate) loose: bool,
    /// Practising with weapons (UseWeapon), and where its loosed arrow aims.
    pub(crate) practice: Option<practice::Practice>,
    pub(crate) practice_aim: Option<Vec3>,
    /// Took another weapon to practise: the one it had (to take back after).
    pub(crate) practice_weapon: Option<Option<FormId>>,
    /// Practising a spell (UseMagic).
    pub(crate) magic_practice: Option<magic_practice::MagicPractice>,
    pub(crate) detect_in: f32,
    /// Hit frames raised this update (`combat::HIT_RIGHT` / `HIT_LEFT`).
    hit_frames: u8,
    /// Dead, and the ragdoll it lies as (with the mapping back to its bones).
    pub(crate) dead: bool,
    pub(crate) ragdoll: Option<(crate::physics::Ragdoll, crate::world::ragdoll::RagdollPose)>,
    /// The lit torch it holds, and what it put away for it (a shield, a
    /// second weapon, a weapon in both hands).
    pub(crate) torch: Option<FormId>,
    pub(crate) stowed_left: Option<FormId>,
    /// What `stowed_left` set aside had both hands.
    pub(crate) stowed_both: bool,
    /// The armor and weapons its models show (see `Engine::update_worn`).
    pub(crate) worn: Vec<FormId>,
    /// The graph raised `IdleFurnitureExit`: out of the furniture.
    out_of_furniture: bool,
    /// Carrying a load away from furniture (firewood from a wood pile): its anim
    /// objects stay in hand until the graph puts them away.
    pub(crate) carrying: bool,
    /// The last arm offset (`OffsetWoodStart`) the graph raised in the furniture
    /// that no machine took: the arms' offset machines sit outside its furniture
    /// state.
    untaken_offset: Option<String>,
    state: State,
    speed: f32,
}

impl ActorRuntime {
    pub fn new(
        ref_id: FormId,
        npc: FormId,
        skeleton: Arc<Skeleton>,
        transform: Mat4,
        packages: Vec<Package>,
        stagger: f32,
    ) -> Self {
        let (scale, rot, pos) = transform.to_scale_rotation_translation();
        let f = rot * Vec3::Y;
        ActorRuntime {
            ref_id,
            npc,
            skeleton,
            graph: None,
            actions: Default::default(),
            velocity: Vec3::ZERO,
            graph_walk_speed: None,
            moves: None,
            sneak_moves: None,
            armor_speed_mult: 1.0,
            anim: None,
            idle: None,
            walk: None,
            capsule: None,
            exec: exec::ActorExec::new(packages, pos, f.x.atan2(f.y), stagger),
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
            strength: 0.0,
            strength_in: 0.0,
            combat_reported: Default::default(),
            weapon_reach: 0.0,
            hands: Default::default(),
            stamina_wait: 0.0,
            stamina_spent: false,
            bow: false,
            bow_speed: 1.0,
            combat_spell: None,
            casts: None,
            cast_hand: None,
            spell_fire: [false; 2],
            hand_equipped: [false; 2],
            arrow_release: false,
            practice: None,
            practice_aim: None,
            practice_weapon: None,
            magic_practice: None,
            loose: false,
            power_cost: 20.0,
            detect_in: stagger,
            hit_frames: 0,
            torch: None,
            stowed_left: None,
            stowed_both: false,
            worn: Vec::new(),
            weapon_out: false,
            equipment_changed: false,
            drawn: false,
            sounds: Vec::new(),
            graph_events: Vec::new(),
            spell_fx_events: Vec::new(),
            activated: None,
            footstep_set: None,
            footsteps: Vec::new(),
            objects: Vec::new(),
            objects_changed: false,
            leave: false,
            moving: false,
            graph_heading: f32::NAN,
            turning: 0,
            sneaking: false,
            look_at: None,
            hurry: false,
            face: None,
            turn_after_arrival: None,
            out_of_furniture: false,
            carrying: false,
            untaken_offset: None,
            state: State::Idle(1.0 + stagger),
            speed: 0.0,
            lift: 0.0,
        }
    }

    /// Where the feet stand: on the collision surface over the navmesh.
    pub fn feet(&self) -> Vec3 {
        self.exec.pos + Vec3::Z * self.lift
    }

    /// Standing height, scaled.
    pub fn height(&self) -> f32 {
        self.skeleton.height * self.scale
    }

    pub fn transform(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            Vec3::splat(self.scale),
            Quat::from_rotation_z(-self.exec.heading),
            self.feet(),
        )
    }

    /// Stand on the collision surface (statics and terrain) where the navmesh
    /// lies above or below it: the highest one within a step of the navmesh
    /// height, as far as the character's foot IK rays reach (40 up, 32 down for
    /// humanoids; 100 / 128 for giants). Furniture places the actor itself.
    fn stand_on_ground(&mut self, physics: &crate::physics::Physics, dt: f32) {
        if self.in_furniture() {
            self.lift = 0.0;
            return;
        }
        let (up, down) = self
            .graph
            .as_ref()
            .and_then(|g| g.project().shared.project.character.as_ref())
            .and_then(|c| c.foot_ik.as_ref())
            .map_or((40.0, 64.0), |ik| (ik.raycast_up, ik.raycast_down));
        let (up, down) = (up * self.scale, down * self.scale);
        let from = self.exec.pos + Vec3::Z * up;
        let want = physics
            .ground_ray(from, -Vec3::Z, up + down)
            .map_or(0.0, |(toi, _)| up - toi);
        log::trace!("{} stands {want:.1} above the navmesh", self.ref_id);
        // Quick to step up, a little slower to settle down.
        let gain: f32 = if want > self.lift { 0.5 } else { 0.3 };
        self.lift += (want - self.lift) * (1.0 - (1.0 - gain).powf(dt * 30.0));
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

    /// How it is moving, for its footsteps: sneaking, or running when nearer its
    /// movement type's run speed than its walk.
    pub(crate) fn gait(&self) -> crate::world::footsteps::Gait {
        use crate::world::footsteps::Gait;
        if self.sneaking {
            return Gait::Sneak;
        }
        let (walk, run) = self.moves.map_or_else(
            || (self.walk_speed(), self.walk_speed() * 2.5),
            |(m, _)| (m.walk, m.run),
        );
        if self.speed > (walk + run) / 2.0 * self.armor_speed_mult {
            Gait::Run
        } else {
            Gait::Walk
        }
    }

    /// Walk to `at` to search there (its packages wait meanwhile).
    pub(crate) fn search_at(&mut self, at: Vec3, furniture: &mut FurnitureWorld) {
        if !self.in_furniture()
            && self
                .exec
                .goal
                .is_some_and(|g| g.behaviour == Behaviour::Travel && self.exec.pinned)
        {
            self.exec.goal = Some(Goal::travel(at));
            return;
        }
        self.interrupt(furniture);
        self.exec.goal = Some(Goal::travel(at));
        self.exec.pinned = true;
    }

    /// Run after `target` and stay close to it (a guard coming to arrest
    /// the player); its packages wait meanwhile.
    pub(crate) fn pursue(&mut self, target: FormId, furniture: &mut FurnitureWorld) {
        self.attend(target, (64.0, 128.0), package::Gait::Run, furniture);
    }

    /// Keep within `radius` (closest, farthest) of `target`; its packages
    /// wait meanwhile.
    pub(crate) fn attend(
        &mut self,
        target: FormId,
        radius: (f32, f32),
        gait: package::Gait,
        furniture: &mut FurnitureWorld,
    ) {
        let already = self.exec.pinned
            && self
                .exec
                .goal
                .is_some_and(|g| g.behaviour == Behaviour::Follow && g.target == Some(target));
        if already {
            return;
        }
        self.exec.search = None;
        self.interrupt(furniture);
        self.exec.goal = Some(Goal {
            behaviour: Behaviour::Follow,
            target: Some(target),
            follow_radius: radius,
            gait,
            ..Goal::travel(self.exec.pos)
        });
        self.exec.pinned = true;
    }

    /// Stop pursuing: back to its packages.
    pub(crate) fn end_pursuit(&mut self) {
        if self.exec.pinned
            && self.exec.search.is_none()
            && self
                .exec
                .goal
                .is_some_and(|g| g.behaviour == Behaviour::Follow)
        {
            self.exec.pinned = false;
            self.exec.next_eval = 0.0;
        }
    }

    /// Stop searching: back to its packages.
    pub(crate) fn end_search(&mut self) {
        if self.exec.search.take().is_some() && self.exec.pinned {
            self.exec.pinned = false;
            self.exec.next_eval = 0.0;
        }
    }

    /// Ground speed it is moving at.
    pub(crate) fn speed(&self) -> f32 {
        self.speed
    }

    /// Its graph is sneaking.
    pub(crate) fn is_sneaking(&self) -> bool {
        self.sneaking
    }

    /// The kind of furniture it is heading for, in or leaving and how far
    /// along: 2 on its way or getting in, 3 in it, 4 getting out.
    pub fn furniture_phase(&self) -> Option<(Use, i32)> {
        let kind = self.seat.as_ref()?.kind;
        match self.state {
            State::Walk { to_seat: true, .. } | State::Approach(_) | State::Enter(_) => {
                Some((kind, 2))
            }
            State::Use(_) => Some((kind, 3)),
            State::Exit(_) => Some((kind, 4)),
            _ => None,
        }
    }

    /// Getting into, using or getting out of furniture.
    pub fn in_furniture(&self) -> bool {
        matches!(self.state, State::Enter(_) | State::Use(_) | State::Exit(_))
    }

    /// How far it has got with its package, to take up again once it loads
    /// again; none before it has planned anything (still `fresh`).
    pub(crate) fn stage(&self) -> Option<exec::Stage> {
        let seat = self.seat.as_ref().map(|s| (s.furniture, s.marker));
        (!self.fresh).then(|| stage_of(&self.state, seat))
    }

    /// Time passed in a menu: walking, it goes on along its path by `reach`
    /// units at once, to finish its walk as it would have (arriving) when it
    /// reaches the end. Whether it walked.
    pub(crate) fn walk_on(&mut self, reach: f32) -> bool {
        let State::Walk {
            path, next, budget, ..
        } = &mut self.state
        else {
            return false;
        };
        let mut left = reach.max(0.0);
        while let Some(&point) = path.get(*next) {
            let to = point - self.exec.pos;
            let flat = to.truncate();
            if flat.length() > 1.0 {
                self.exec.heading = flat.x.atan2(flat.y);
            }
            let d = to.length();
            if d > left {
                self.exec.pos += to * (left / d);
                return true;
            }
            left -= d;
            self.exec.pos = point;
            *next += 1;
        }
        // Not out of time: it arrived.
        *budget = budget.max(1.0);
        true
    }

    /// Stop and stand (e.g. when spoken to). Actors in furniture stay put.
    pub fn halt(&mut self, idle: f32) {
        if self.in_furniture() {
            return;
        }
        self.turn_after_arrival = None;
        self.state = State::Idle(idle);
        self.speed = 0.0;
    }

    /// The package changed: get up (if in furniture) or stop, then plan anew.
    pub fn interrupt(&mut self, furniture: &mut FurnitureWorld) {
        self.face = None;
        self.turn_after_arrival = None;
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

    /// Face `target` while standing, carrying on a turn already under way (`was`).
    fn face_towards(&mut self, target: Option<FormId>, was: Option<(FormId, bool)>) {
        self.face = target.map(|t| (t, was.is_some_and(|(f, turning)| f == t && turning)));
    }

    /// Standing: turn to whom it waits for once they are well off to one side,
    /// until facing them.
    fn face_target(&mut self, w: &World, dt: f32) {
        let Some((id, turning)) = self.face else {
            return;
        };
        let Some(&p) = w.targets.get(&id) else { return };
        let dir = (p - self.exec.pos).truncate();
        if dir.length() < 1.0 {
            return;
        }
        let off = angle_to(self.exec.heading, dir.extend(0.0)).abs();
        if turning || off > FACE_TURN_START {
            let remaining = self.turn_towards(dir.normalize().extend(0.0), dt);
            self.face = Some((id, remaining.abs() > FACE_TURN_STOP));
        }
    }

    fn turn_towards(&mut self, dir: Vec3, dt: f32) -> f32 {
        let d = angle_to(self.exec.heading, dir);
        let rate = self.turn_rate();
        let step = d.clamp(-rate * dt, rate * dt);
        self.exec.heading += step;
        d - step
    }

    /// Step aside from other actors (and the player) within reach, preferring to
    /// pass on the side we're already heading for. Stays on the navmesh.
    fn keep_clear(
        &mut self,
        bodies: &[(FormId, Vec3)],
        nav: &nav::NavWorld,
        dt: f32,
        before: Vec3,
    ) {
        const REACH: f32 = 48.0;
        // Someone already standing at our destination: stop short of it.
        if let State::Walk {
            path,
            next,
            to_seat: false,
            ..
        } = &mut self.state
            && let Some(&dest) = path.last()
            && self.exec.pos.truncate().distance(dest.truncate()) < REACH * 2.0
            && bodies.iter().any(|&(id, p)| {
                id != self.ref_id
                    && p.truncate().distance(dest.truncate()) < REACH
                    && (p.z - dest.z).abs() < 100.0
            })
        {
            *next = path.len();
            return;
        }
        let fwd = glam::Vec2::new(self.exec.heading.sin(), self.exec.heading.cos());
        let mut push = glam::Vec2::ZERO;
        let mut blocked = false;
        for &(id, p) in bodies {
            if id == self.ref_id || (p.z - self.exec.pos.z).abs() > 100.0 {
                continue;
            }
            let d = (self.exec.pos - p).truncate();
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
        let from = if blocked { before } else { self.exec.pos };
        let mut p = from + step.extend(0.0);
        match nav.height_at(p) {
            Some(z) if (z - from.z).abs() < 30.0 => {
                p.z = z;
                self.exec.pos = p;
            }
            // No room to step aside: wait behind them.
            _ if blocked => self.exec.pos = before,
            _ => {}
        }
    }

    /// Walking from `was` to where it is now, it slides along (or stops at)
    /// solid collision the navmesh runs on through (a closed gate, as a
    /// backstop to obstacles blocking the navmesh). Not getting into or out
    /// of furniture, nor jumps a step can't make (moved by a script).
    fn collide(&mut self, physics: &crate::physics::Physics, nav: &nav::NavWorld, was: Vec3) {
        const MAX_STEP: f32 = 100.0;
        let Some(capsule) = self.capsule else {
            return;
        };
        let moved = (self.exec.pos - was).truncate().length();
        if moved < 0.01
            || moved > MAX_STEP
            || self.in_furniture()
            || matches!(self.state, State::Approach(_))
        {
            return;
        }
        let lift = Vec3::Z * self.lift;
        let to = self.exec.pos + lift;
        let p = physics.slide_actor(capsule, was + lift, to);
        if p.truncate().distance(to.truncate()) > 0.01 {
            log::trace!("{} slides to {p:?} rather than {to:?}", self.ref_id);
            let z = nav
                .height_at(p.with_z(self.exec.pos.z))
                .unwrap_or(self.exec.pos.z);
            self.exec.pos = p.with_z(z);
        }
    }

    /// What it stands on, seen from above: its bound's footprint, the long
    /// way a segment (along its heading for a wolf or a horse) and the short
    /// way the radius about it.
    pub(crate) fn footprint(&self) -> Footprint {
        let half = self.skeleton.footprint * self.scale;
        let fwd = glam::Vec2::new(self.exec.heading.sin(), self.exec.heading.cos());
        let (axis, along) = if half.y >= half.x {
            (fwd, half.y - half.x)
        } else {
            (fwd.perp(), half.x - half.y)
        };
        let c = self.exec.pos.truncate();
        Footprint {
            id: self.ref_id,
            z: self.exec.pos.z,
            a: c - axis * along,
            b: c + axis * along,
            radius: half.min_element(),
        }
    }

    /// A fighter steps out of the others' footprints (all but its target's:
    /// it closes to reach that one), half of each overlap (the other steps
    /// too, or will). Stays on the navmesh.
    fn keep_apart(&mut self, others: &[Footprint], target: FormId, nav: &nav::NavWorld) {
        let me = self.footprint();
        let mut push = glam::Vec2::ZERO;
        for o in others {
            if o.id == self.ref_id || o.id == target || (o.z - me.z).abs() > 100.0 {
                continue;
            }
            let (s, dist) = archery::segment_distance(
                me.a.extend(0.0),
                me.b.extend(0.0),
                o.a.extend(0.0),
                o.b.extend(0.0),
            );
            let overlap = me.radius + o.radius - dist;
            if overlap <= 0.0 {
                continue;
            }
            // From the nearest point of theirs to the nearest of ours.
            let p = me.a.lerp(me.b, s);
            let along = o.b - o.a;
            let t = (p - o.a).dot(along) / along.length_squared().max(1e-6);
            let q = o.a + along * t.clamp(0.0, 1.0);
            let away = (p - q)
                .try_normalize()
                .or_else(|| (me.a + me.b - o.a - o.b).try_normalize())
                .unwrap_or(glam::Vec2::new(
                    self.exec.heading.cos(),
                    -self.exec.heading.sin(),
                ));
            push += away * overlap * 0.5;
        }
        if push == glam::Vec2::ZERO {
            return;
        }
        let mut p = self.exec.pos + push.extend(0.0);
        if let Some(z) = nav
            .height_at(p)
            .filter(|z| (z - self.exec.pos.z).abs() < 30.0)
        {
            p.z = z;
            self.exec.pos = p;
        }
    }

    /// Settled in the reserved seat (the graph is in its loop) for `secs`.
    fn settle(&mut self, secs: f32) {
        let Some(seat) = &self.seat else { return };
        self.exec.pos = seat.pos;
        self.exec.heading = seat.heading;
        // A one-shot (picking up firewood) lasts as long as its clip.
        let secs = if seat.clips.idle_loops {
            secs
        } else {
            seat.clips.idle.duration()
        };
        self.state = State::Use(secs);
        self.next_idle = 4.0;
    }

    /// Send the graph an event now; true when it took it.
    fn graph_event(&mut self, event: &str, clips: &mut Clips) -> bool {
        if event.contains("Spell") || event.contains("Equipped_Event") {
            self.spell_fx_events.push(event.to_owned());
        }
        match &mut self.graph {
            Some(g) => g.handle_event(event, clips.vfs, clips.anims, &self.skeleton),
            None => false,
        }
    }

    /// At the start of the seat's enter animation: have the graph play it.
    fn begin_enter(&mut self, w: &mut World) {
        self.turn_after_arrival = None;
        let Some(seat) = self.seat.clone() else {
            return;
        };
        self.put_load_away(&seat.clips, &mut w.clips);
        if !self.graph_event(&seat.clips.event, &mut w.clips) {
            log::debug!(
                "{}: the graph won't take {} for {}",
                self.ref_id,
                seat.clips.event,
                seat.furniture
            );
            self.give_up_seat(w.furniture);
            self.state = State::Idle(uniform(w.rand, 2.0, 5.0));
            return;
        }
        log::debug!(
            "{} entering {} ({}, {:.1} s)",
            self.ref_id,
            seat.furniture,
            seat.clips.event,
            seat.clips.enter_time()
        );
        self.out_of_furniture = false;
        self.untaken_offset = None;
        self.state = State::Enter(seat.clips.enter_time());
        if !seat.furniture.is_null() {
            self.activated = Some(seat.furniture);
        }
    }

    /// Before using furniture: a load brought to it is put away first, while the
    /// arms' offset machines (outside the furniture state) still hear
    /// `OffsetStop`. A put-down's clip puts it away itself.
    fn put_load_away(&mut self, clips: &furniture::UseClips, lib: &mut Clips) {
        if std::mem::take(&mut self.carrying) && !clips.puts_down {
            self.graph_event("OffsetStop", lib);
            self.objects_changed |= !self.objects.is_empty();
            self.objects.clear();
        }
    }

    /// Have the graph leave the seat by the first exit it takes.
    fn begin_exit(&mut self, w: &mut World) {
        let Some(seat) = self.seat.clone() else {
            self.stand_up(w);
            return;
        };
        let exit = seat
            .clips
            .exits
            .iter()
            .find(|e| self.graph_event(e, &mut w.clips))
            .cloned();
        let Some(exit) = exit else {
            // Nothing to play (or no graph): just stand.
            self.stand_up(w);
            return;
        };
        log::debug!(
            "{} getting up ({exit}, {:.1} s)",
            self.ref_id,
            seat.clips.exit_time()
        );
        self.out_of_furniture = false;
        self.state = State::Exit(seat.clips.exit_time() + 0.5);
    }

    /// Advance movement. Returns true when the actor moved.
    pub fn step(&mut self, dt: f32, w: &mut World) -> bool {
        if matches!(self.state, State::Idle(_)) && self.practising() {
            let goal = self.exec.goal.and_then(|g| g.practice).unwrap();
            self.practice_step(dt, &goal, w);
            return false;
        }
        if matches!(self.state, State::Idle(_)) && self.magic_practising() {
            let goal = self.exec.goal.and_then(|g| g.magic).unwrap();
            self.magic_practice_step(dt, &goal, w);
            return false;
        }
        let idle_turn_rate = self.turn_rate();
        match &mut self.state {
            State::Idle(t) => {
                self.speed = 0.0;
                if self.face.is_some() {
                    // A live conversation / escort-facing request takes priority.
                    self.turn_after_arrival = None;
                }
                if self.turn_after_arrival.is_some()
                    && self
                        .exec
                        .goal
                        .is_some_and(|goal| goal.behaviour != Behaviour::Travel)
                {
                    self.turn_after_arrival = None;
                }
                // Initial package evaluation may be delayed by the spawn
                // stagger. Hold this actor idle until evaluation selects
                // Travel or clears the pending target.
                if self.turn_after_arrival.is_some() && self.exec.goal.is_none() {
                    return false;
                }
                if let Some(heading) = self.turn_after_arrival.filter(|_| {
                    self.exec
                        .goal
                        .is_some_and(|goal| goal.behaviour == Behaviour::Travel)
                }) {
                    let dir = Vec3::new(heading.sin(), heading.cos(), 0.0);
                    let delta = angle_to(self.exec.heading, dir);
                    let step = delta.clamp(-idle_turn_rate * dt, idle_turn_rate * dt);
                    let remaining = delta - step;
                    self.exec.heading += step;
                    if remaining.abs() <= FACE_TURN_STOP {
                        self.exec.heading = heading;
                        self.turn_after_arrival = None;
                    }
                    *t -= dt;
                    return false;
                }
                *t -= dt;
                if *t > 0.0 {
                    self.face_target(w, dt);
                    return false;
                }
                self.plan(w);
                false
            }
            State::Walk {
                path,
                next,
                budget,
                to_seat,
            } => {
                *budget -= dt;
                if *budget < 0.0 || *next >= path.len() {
                    let arrived = *budget >= 0.0;
                    log::debug!(
                        "{} walk ended ({})",
                        self.ref_id,
                        if arrived { "arrived" } else { "timeout" }
                    );
                    self.speed = 0.0;
                    let follow = self.exec.goal.is_some_and(|g| {
                        matches!(g.behaviour, Behaviour::Follow | Behaviour::Escort)
                    });
                    if *to_seat && arrived {
                        self.state = State::Approach(0.0);
                    } else if follow
                        || (arrived
                            && self
                                .exec
                                .goal
                                .is_some_and(|g| g.behaviour == Behaviour::Patrol))
                    {
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
                let to = (target - self.exec.pos).truncate();
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
                let fwd = Vec3::new(self.exec.heading.sin(), self.exec.heading.cos(), 0.0);
                let mut p = self.exec.pos + fwd * (speed * dt).min(dist);
                let z_hint = Vec3::new(p.x, p.y, self.exec.pos.z);
                p.z = w.nav.height_at(z_hint).unwrap_or_else(|| {
                    // Interpolate towards the waypoint height off the mesh.
                    self.exec.pos.z + (target.z - self.exec.pos.z) * ((speed * dt) / dist).min(1.0)
                });
                self.exec.pos = p;
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
                let to = (start - self.exec.pos).truncate();
                let step = (WALK_SPEED * 0.5 * dt).min(to.length());
                self.exec.pos += (to.normalize_or_zero() * step).extend(0.0);
                let remaining = self.turn_towards(Vec3::new(h0.sin(), h0.cos(), 0.0), dt);
                if remaining.abs() < 0.03 && to.length() < 2.0 || t > 3.0 {
                    self.exec.pos = start;
                    self.exec.heading = h0;
                    self.begin_enter(w);
                }
                true
            }
            // The graph moves the actor (root motion, applied in `animate`).
            // The graph left the furniture by itself (a wood pile's put-down).
            State::Enter(_) | State::Use(_) if self.out_of_furniture => {
                self.stand_up(w);
                false
            }
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
                    if self
                        .seat
                        .as_ref()
                        .is_some_and(|s| s.kind == Use::Sit && !s.furniture.is_null())
                    {
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
                // furniture state, so the graph goes back to its default state.
                // The arms then take the offset the one-shot raised for carrying
                // its load away (`OffsetWoodStart`), which their machines couldn't
                // hear in the furniture state; else the instant exit the idle tree
                // gives.
                if let Some(seat) = self.seat.clone().filter(|s| !s.clips.idle_loops) {
                    self.graph_event("IdleForceDefaultState", &mut w.clips);
                    let offset = self.untaken_offset.take();
                    let instant = seat
                        .clips
                        .instant_exit
                        .clone()
                        .filter(|e| e != "IdleForceDefaultState");
                    if let Some(exit) = offset
                        .into_iter()
                        .chain(instant)
                        .find(|e| self.graph_event(e, &mut w.clips))
                    {
                        log::debug!("{} leaves {} with {exit}", self.ref_id, seat.furniture);
                        self.carrying = !self.objects.is_empty();
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
        log::debug!("{} stood up at {:?}", self.ref_id, self.exec.pos);
        // Using what an Activate package names is activating it.
        let used = self.seat.as_ref().map(|s| s.furniture);
        if let Some(goal) = self
            .exec
            .goal
            .filter(|g| g.behaviour == Behaviour::Activate)
            && used.is_some_and(|f| Some(f) == goal.furniture)
        {
            self.activation_done(&goal);
        }
        self.give_up_seat(w.furniture);
        if !self.carrying {
            self.objects_changed |= !self.objects.is_empty();
            self.objects.clear();
        }
        if let Some(z) = w.nav.height_at(self.exec.pos) {
            self.exec.pos.z = z;
        }
        self.state = State::Idle(uniform(w.rand, 2.0, 6.0));
    }

    /// Brought down in furniture (bleeding out): out of it at once, onto the floor
    /// where getting in began (clear of the bench or chair), without the exit
    /// animation.
    pub(crate) fn fall_out_of_furniture(
        &mut self,
        furniture: &mut FurnitureWorld,
        nav: &nav::NavWorld,
    ) {
        if !self.in_furniture() {
            return;
        }
        if let Some(seat) = &self.seat {
            let (start, _) = seat.enter_start();
            log::debug!(
                "{} drops out of {}: seat {:?} heading {:.0}, was at {:?}, enter start {:?}",
                self.ref_id,
                seat.furniture,
                seat.pos,
                seat.heading.to_degrees(),
                self.exec.pos,
                start
            );
            self.exec.pos = start.with_z(self.exec.pos.z);
        }
        self.give_up_seat(furniture);
        self.leave = false;
        self.objects_changed |= !self.objects.is_empty();
        self.objects.clear();
        self.carrying = false;
        if let Some(z) = nav.height_at(self.exec.pos) {
            self.exec.pos.z = z;
        }
        self.state = State::Idle(0.0);
    }

    /// The Activate package's target was activated once more: after "Number to
    /// Activate" of them it is done, and passed over while it still applies.
    fn activation_done(&mut self, goal: &Goal) {
        let Some(run) = self.exec.run.as_mut() else {
            return;
        };
        run.activations += 1;
        log::debug!(
            "{} activated {:?} ({} of {})",
            self.ref_id,
            goal.target,
            run.activations,
            goal.activate
        );
        if goal.activate > 0 && run.activations >= goal.activate {
            self.exec.complete_run(true);
        }
    }

    /// At a UseWeapon goal's location, out of combat.
    fn practising(&self) -> bool {
        self.exec.combat.is_none()
            && self.exec.goal.is_some_and(|g| {
                g.practice.is_some()
                    && self.exec.pos.truncate().distance(g.centre.truncate()) <= g.radius.max(96.0)
            })
    }

    /// Choose what to do next from the current goal.
    fn plan(&mut self, w: &mut World) {
        if !self.practising() {
            self.end_practice(w);
        }
        if !self.magic_practising() {
            self.end_magic_practice(w);
        }
        // VRM_AI_NO_SNAP: walk into furniture even on cell load (to watch enter animations).
        let fresh = std::mem::take(&mut self.fresh) && std::env::var_os("VRM_AI_NO_SNAP").is_none();
        self.give_up_seat(w.furniture);
        self.hurry = false;
        let face = self.face.take();
        // Actors standing about now and then play one of their idles first
        // (creatures more often: they have no furniture to use); not with a load
        // in their arms.
        let humanoid = self.graph.as_ref().is_some_and(|g| g.project().humanoid());
        let wandering = self
            .exec
            .goal
            .is_none_or(|g| matches!(g.behaviour, Behaviour::Sandbox | Behaviour::Hold));
        let odds = if humanoid { 4 } else { 3 };
        if self.graph.is_some() && wandering && !fresh && !self.carrying && (w.rand)() % odds == 0 {
            self.wants_action_idle = true;
            self.state = State::Idle(uniform(w.rand, 1.0, 3.0));
            return;
        }
        let Some(goal) = self.exec.goal else {
            self.state = State::Idle(uniform(w.rand, 5.0, 10.0));
            return;
        };
        // Activating starts beside the target, not in it.
        let snap = fresh && goal.behaviour != Behaviour::Activate;
        if self.seek_furniture(&goal, snap, w) {
            return;
        }
        // At what an Activate package names, that isn't furniture: activate it.
        if goal.behaviour == Behaviour::Activate
            && goal.furniture.is_none()
            && let Some(target) = goal.target
            && self.exec.pos.truncate().distance(goal.centre.truncate()) <= goal.radius.max(96.0)
        {
            self.activated = Some(target);
            self.activation_done(&goal);
            self.state = State::Idle(uniform(w.rand, 2.0, 6.0));
            return;
        }
        // Heading back into the sandbox area from elsewhere counts as travel.
        let mut travelling = goal.behaviour != Behaviour::Sandbox;
        let target = match goal.behaviour {
            // Goals are what a force greeter does meanwhile, never this.
            Behaviour::Hold | Behaviour::ForceGreet => None,
            Behaviour::Patrol => match self.patrol_target(&goal, w) {
                PatrolStep::Walk(p) => Some(p),
                PatrolStep::Busy => return,
                PatrolStep::Done => None,
            },
            Behaviour::Follow => {
                let (min, max) = goal.follow_radius;
                let t = goal.target.and_then(|t| w.targets.get(&t).copied());
                match t {
                    Some(t) if self.exec.pos.distance(t) > max => {
                        // Head for a spot `min` short of the target.
                        let back = (self.exec.pos - t)
                            .truncate()
                            .normalize_or_zero()
                            .extend(0.0)
                            * min;
                        Some(t + back)
                    }
                    Some(_) => {
                        self.face_towards(goal.target, face);
                        self.state = State::Idle(0.5);
                        return;
                    }
                    None => None,
                }
            }
            Behaviour::Escort => {
                let near = goal.radius.max(96.0);
                let left = self.exec.pos.truncate().distance(goal.centre.truncate());
                let t = goal.target.and_then(|t| w.targets.get(&t).copied());
                // Behind: too far off and no nearer the destination (one gone on ahead
                // isn't waited for).
                let ahead = t.is_some_and(|t| t.truncate().distance(goal.centre.truncate()) < left);
                let apart = t.map_or(0.0, |t| self.exec.pos.distance(t));
                if left <= near {
                    self.face_towards(goal.target, face);
                    None
                } else if !ahead && t.is_some() && apart > goal.escort_wait {
                    // Wait for the escorted actor to catch up, facing them.
                    log::debug!(
                        "{} waiting for {:?} ({apart:.0} behind)",
                        self.ref_id,
                        goal.target
                    );
                    self.face_towards(goal.target, face);
                    self.state = State::Idle(0.5);
                    return;
                } else {
                    // Run to catch up with one gone on ahead ("Run If Behind Distance").
                    self.hurry = ahead && apart > goal.escort_run;
                    Some(goal.centre)
                }
            }
            Behaviour::Travel
            | Behaviour::Sleep
            | Behaviour::Sit
            | Behaviour::Activate
            | Behaviour::UseWeapon
            | Behaviour::UseMagic => {
                let near = goal.radius.max(96.0);
                (self.exec.pos.truncate().distance(goal.centre.truncate()) > near)
                    .then_some(goal.centre)
            }
            Behaviour::Sandbox => {
                // Return to the sandbox area first if we've strayed from it.
                let r = goal.radius.clamp(SANDBOX_MIN, SANDBOX_MAX);
                if self.exec.pos.truncate().distance(goal.centre.truncate()) > r * 1.5 {
                    travelling = true;
                    Some(goal.centre)
                } else if goal.allow.wandering || (w.rand)().is_multiple_of(4) {
                    w.nav.random_point(goal.centre, r, &mut *w.rand)
                } else {
                    None
                }
            }
        };
        let path = target.and_then(|t| w.nav.find_path(self.exec.pos, t));
        match path {
            Some(path) if !path.is_empty() => {
                let len = path_length(self.exec.pos, &path);
                let max = if travelling {
                    MAX_TRAVEL_PATH
                } else {
                    MAX_SANDBOX_PATH
                };
                if len > max {
                    log::debug!("{} path too long ({len:.0})", self.ref_id);
                    self.state = State::Idle(uniform(w.rand, 10.0, 20.0));
                    return;
                }
                log::debug!(
                    "{} walking {:.0} units via {} points ({:?}{})",
                    self.ref_id,
                    len,
                    path.len(),
                    goal.behaviour,
                    if self.hurry { ", running" } else { "" }
                );
                let mut budget = len / WALK_SPEED * 2.5 + 5.0;
                // Look back at the target now and then.
                if matches!(goal.behaviour, Behaviour::Follow | Behaviour::Escort) {
                    budget = budget.min(2.0);
                }
                self.state = State::Walk {
                    path,
                    next: 0,
                    budget,
                    to_seat: false,
                };
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
        let sandbox = self
            .exec
            .goal
            .is_some_and(|g| g.behaviour == Behaviour::Sandbox && g.allow.eating);
        // Meals are the humanoid graphs' (creatures graze through their own idles).
        let humanoid = self.graph.as_ref().is_some_and(|g| g.project().humanoid());
        self.wants_meal = sandbox && humanoid && (w.rand)() % 4 == 0;
    }

    /// Where to go next on a patrol. At a point that is an idle marker (or other
    /// usable furniture) the actor uses it before moving on.
    fn patrol_target(&mut self, goal: &Goal, w: &mut World) -> PatrolStep {
        let Some(start) = goal.target else {
            return PatrolStep::Done;
        };
        let Some(points) = w.patrols.get(&start).cloned() else {
            return PatrolStep::Done;
        };
        if points.is_empty() {
            return PatrolStep::Done;
        }
        let idx = match self.exec.patrol {
            Some((s, i)) if s == start => i,
            _ if goal.start_nearest => {
                let near = points.iter().enumerate().min_by(|a, b| {
                    a.1.pos
                        .distance(self.exec.pos)
                        .total_cmp(&b.1.pos.distance(self.exec.pos))
                });
                near.map_or(0, |(i, _)| i)
            }
            _ => 0,
        };
        let Some(p) = points.get(idx).copied() else {
            return PatrolStep::Done;
        };
        if self.exec.pos.truncate().distance(p.pos.truncate()) > goal.radius.max(48.0) {
            self.exec.patrol = Some((start, idx));
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
        self.exec.patrol = Some((start, next.unwrap_or(idx)));
        if w.furniture.get(p.ref_id).is_some() {
            let at = Goal {
                behaviour: Behaviour::Patrol,
                centre: p.pos,
                furniture: Some(p.ref_id),
                ..*goal
            };
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
        // Creatures only use the furniture their package names (ambushers' coffins,
        // thrones and alcoves), through their own graphs' furniture idles.
        if !self.skeleton_path.contains("actors/character/") && goal.furniture.is_none() {
            return false;
        }
        let (kinds, centre, radius, secs): (Vec<Use>, Vec3, f32, f32) = match goal.behaviour {
            // A specific target can be any marker (a creature's coffin, throne or alcove).
            Behaviour::Sleep if goal.furniture.is_some() => (
                vec![Use::Sleep, Use::Sit, Use::Special],
                goal.centre,
                goal.radius.max(FURNITURE_SEARCH),
                f32::INFINITY,
            ),
            Behaviour::Sleep => (
                vec![Use::Sleep],
                goal.centre,
                goal.radius.max(FURNITURE_SEARCH),
                f32::INFINITY,
            ),
            // Something to activate that isn't furniture is activated where it stands.
            Behaviour::Activate if goal.furniture.is_none() => return false,
            // A Sit package's chair may be special furniture (a throne, a writing desk).
            Behaviour::Sit | Behaviour::Activate => {
                // A specific target can be anything usable; otherwise a chair or special seat.
                let kinds = if goal.furniture.is_some() || goal.any_furniture {
                    vec![Use::Sit, Use::Special, Use::Sleep, Use::Lean, Use::Idle]
                } else {
                    vec![Use::Sit, Use::Special]
                };
                // Activating is for a moment (a one-shot idle lasts its clip).
                let secs = if goal.behaviour == Behaviour::Activate {
                    uniform(w.rand, ACTIVATE_USE.0, ACTIVATE_USE.1)
                } else {
                    f32::INFINITY
                };
                (kinds, goal.centre, goal.radius.max(FURNITURE_SEARCH), secs)
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
                let chance = if goal.allow.wandering {
                    0.75 - goal.energy / 200.0
                } else {
                    0.9
                };
                if uniform(w.rand, 0.0, 1.0) > chance {
                    return false;
                }
                let secs = (150.0 - goal.energy * 1.2) * uniform(w.rand, 0.6, 1.4);
                (
                    kinds,
                    goal.centre,
                    goal.radius.clamp(SANDBOX_MIN, SANDBOX_MAX),
                    secs,
                )
            }
            // Patrol idle markers: the point's marker, for its idle time.
            Behaviour::Patrol if goal.furniture.is_some() => (
                vec![Use::Idle, Use::Lean, Use::Special, Use::Sit],
                goal.centre,
                96.0,
                uniform(w.rand, 6.0, 12.0),
            ),
            Behaviour::Travel
            | Behaviour::Hold
            | Behaviour::Patrol
            | Behaviour::Follow
            | Behaviour::Escort
            | Behaviour::ForceGreet
            | Behaviour::UseWeapon
            | Behaviour::UseMagic => return false,
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
            log::debug!(
                "{} no free {kinds:?} within {radius:.0} of {centre:?}",
                self.ref_id
            );
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
                (rank(a.0), a.2.pos.distance(self.exec.pos))
                    .partial_cmp(&(rank(b.0), b.2.pos.distance(self.exec.pos)))
                    .unwrap()
            });
        } else {
            for i in (1..options.len()).rev() {
                options.swap(i, ((w.rand)() % (i as u64 + 1)) as usize);
            }
        }
        for (fid, mi, marker) in options.into_iter().take(6) {
            let Some(f) = w.furniture.get(fid) else {
                continue;
            };
            let Some(project) = self.graph.as_ref().map(|g| g.project().clone()) else {
                return false;
            };
            let ways = furniture::ways_to_use(
                f,
                mi,
                &marker,
                self.child,
                &project,
                &self.skeleton_path,
                self.female,
                &self.skeleton,
                &mut w.clips,
                &mut *w.rand,
                &|item| (w.holds)(self.ref_id, item),
            );
            // Idle markers say how long they are used for.
            let secs = if f.idle_time > 0.0
                && matches!(goal.behaviour, Behaviour::Sandbox | Behaviour::Patrol)
            {
                f.idle_time
            } else {
                secs
            };
            // The entry whose starting spot is on the navmesh and nearest.
            let best = ways
                .into_iter()
                .map(|(entry, clips)| {
                    let (start, _) = furniture::enter_start(&marker, &clips.enter);
                    (entry, clips, start)
                })
                .filter(|(_, _, start)| {
                    w.nav
                        .height_at(*start)
                        .is_some_and(|z| (z - start.z).abs() < 48.0)
                })
                .min_by(|a, b| {
                    a.2.distance(self.exec.pos)
                        .total_cmp(&b.2.distance(self.exec.pos))
                });
            let Some((entry, clips, start)) = best else {
                log::debug!(
                    "{} can't use {fid} marker {mi}: no reachable entry",
                    self.ref_id
                );
                continue;
            };
            let seat = Seat {
                furniture: fid,
                marker: mi,
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
                self.put_load_away(&seat.clips, &mut w.clips);
                if !self.graph_event(&seat.clips.event, &mut w.clips) {
                    continue;
                }
                let mut raised = Vec::new();
                if let Some(g) = &mut self.graph {
                    g.set_variable("isInFurniture", 1.0);
                    let settle = seat.clips.enter_time() + 1.0;
                    let mut t = 0.0;
                    while t < settle {
                        let frame = g.update(0.25, w.clips.vfs, w.clips.anims, &self.skeleton);
                        raised.extend(frame.raised);
                        t += 0.25;
                    }
                }
                // Keep the objects drawn or put away while skipping the entry.
                // Sounds, footsteps and other transient events stay skipped.
                self.untaken_offset = None;
                for r in raised {
                    self.anim_object_event(&r.event.to_ascii_lowercase(), r.payload.as_deref());
                    self.note_untaken(&r);
                }
                w.furniture.reserve(fid, mi, self.ref_id);
                self.seat = Some(seat);
                self.settle(secs);
                log::debug!(
                    "{} starts in {fid} marker {mi} ({:?})",
                    self.ref_id,
                    marker.kind
                );
                return true;
            }
            let Some(route) = w.nav.find_path(self.exec.pos, start) else {
                continue;
            };
            let len = path_length(self.exec.pos, &route);
            if len > MAX_TRAVEL_PATH {
                continue;
            }
            w.furniture.reserve(fid, mi, self.ref_id);
            log::debug!(
                "{} heading for {fid} marker {mi} ({:?}, {entry:?} entry, {len:.0} units)",
                self.ref_id,
                marker.kind
            );
            self.seat = Some(seat);
            self.state = State::Walk {
                path: route,
                next: 0,
                budget: len / WALK_SPEED * 2.5 + 5.0,
                to_seat: true,
            };
            return true;
        }
        false
    }

    /// Walking speed: the movement type's, else the graph's or walk clip's root
    /// motion, so feet don't slide.
    pub fn walk_speed(&self) -> f32 {
        if let Some(v) = self
            .moves
            .map(|(m, _)| m.walk)
            .filter(|v| *v > 1.0)
            .or(self.graph_walk_speed)
        {
            return v;
        }
        self.walk
            .as_ref()
            .and_then(|w| {
                w.motion
                    .as_ref()
                    .map(|m| m.end().0.truncate().length() / w.duration())
            })
            .filter(|v| (20.0..400.0).contains(v))
            .unwrap_or(WALK_SPEED)
    }

    /// Foot placement: the ground under each leg's ankle (as the graph left them
    /// last update), from rays through the static world. Humanoids' foot IK is
    /// switched on by the engine: off in furniture, where the animation puts the
    /// feet, and while the graph asks (`bHumanoidFootIKDisable`: killmoves, paired
    /// animations, swimming).
    fn find_ground(&mut self, physics: &crate::physics::Physics) {
        let model = self.transform();
        let (feet, heading, scale) = (self.feet(), self.exec.heading, self.scale);
        let in_furniture = self.in_furniture();
        let Some(g) = self.graph.as_mut() else { return };
        let on = !in_furniture
            && g.variable("bHumanoidFootIKDisable")
                .is_none_or(|v| v == 0.0);
        g.set_variable("bHumanoidFootIKEnable", if on { 1.0 } else { 0.0 });
        let Some(ik) = g
            .project()
            .shared
            .project
            .character
            .as_ref()
            .and_then(|c| c.foot_ik.clone())
        else {
            return;
        };
        let ground = g
            .ankles
            .iter()
            .map(|ankle| {
                let w = model.transform_point3(*ankle);
                let from = Vec3::new(w.x, w.y, feet.z + ik.raycast_up * scale);
                let (toi, normal) = physics.ground_ray(
                    from,
                    -Vec3::Z,
                    (ik.raycast_up + ik.raycast_down) * scale,
                )?;
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
        self.base_move_speed() * self.armor_speed_mult
    }

    fn base_move_speed(&self) -> f32 {
        let walk = self.walk_speed();
        let (gait, sneak) = self
            .exec
            .goal
            .map_or((package::Gait::Walk, false), |g| (g.gait, g.sneak));
        if sneak {
            return self.sneak_moves.map_or(walk * 0.75, |(m, _)| {
                if gait == package::Gait::Run {
                    m.run
                } else {
                    m.walk
                }
            });
        }
        let run = self
            .moves
            .map(|(m, _)| m.run)
            .filter(|r| *r > walk)
            .unwrap_or(walk * 3.0);
        if self.hurry {
            return run;
        }
        match gait {
            package::Gait::Walk => walk,
            package::Gait::FastWalk => walk + (run - walk) * 0.25,
            package::Gait::Jog => (walk + run) * 0.5,
            package::Gait::Run => run,
        }
    }

    /// Turn rate (radians per second) from the movement type, walking or standing.
    fn turn_rate(&self) -> f32 {
        let m = if self.exec.goal.is_some_and(|g| g.sneak) {
            self.sneak_moves.or(self.moves)
        } else {
            self.moves
        };
        let r = m.map(|(m, _)| {
            if self.speed > 5.0 {
                m.turn_moving
            } else {
                m.turn_walk
            }
        });
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
        let walking =
            self.speed > 5.0 && matches!(self.state, State::Walk { .. } | State::Approach(_));
        let furniture =
            self.in_furniture() && self.seat.as_ref().is_some_and(|s| !s.furniture.is_null());
        // Turn rate in degrees per second, counter-clockwise (left) positive as Havok
        // has it; headings turn clockwise.
        let turned = if self.graph_heading.is_finite() {
            wrap_angle(self.exec.heading - self.graph_heading)
        } else {
            0.0
        };
        self.graph_heading = self.exec.heading;
        let turn_delta = if dt > 1e-4 {
            -turned.to_degrees() / dt
        } else {
            0.0
        };
        let still = !walking && !self.in_furniture();
        let model = self.transform();
        let sneak = self.exec.goal.is_some_and(|g| g.sneak) && !self.in_furniture();
        let state = if sneak {
            self.sneak_moves.or(self.moves)
        } else {
            self.moves
        }
        .map(|(_, s)| s);
        let g = self.graph.as_mut().expect("checked");
        if sneak != self.sneaking {
            self.actions.send(g, "ActionSneak");
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
        if let Some(direction) = crate::world::movement::direction(self.exec.heading, self.velocity)
        {
            g.set_variable("Direction", direction);
        }
        g.set_variable("TurnDelta", turn_delta);
        // Standing still and turning: the graph's turn-in-place loops (creatures).
        let turning = match (still, turn_delta) {
            (true, d) if d > 20.0 => 1,
            (true, d) if d < -20.0 => -1,
            _ => 0,
        };
        if turning != self.turning {
            self.actions.send(
                g,
                match turning {
                    1 => "ActionTurnLeft",
                    -1 => "ActionTurnRight",
                    _ => "ActionTurnStop",
                },
            );
            self.turning = turning;
        }
        g.set_variable("isInFurniture", if furniture { 1.0 } else { 0.0 });
        if walking != self.moving {
            self.actions.send(
                g,
                if walking {
                    "ActionMoveStart"
                } else {
                    "ActionMoveStop"
                },
            );
            if walking {
                // Walking, not sprinting (horses' locomotion starts out in its sprint).
                self.actions.send(g, "ActionSprintStop");
            }
            self.moving = walking;
        }
        let frame = g.update(dt, clips.vfs, clips.anims, &self.skeleton);
        if self.in_furniture() {
            let (t, yaw) = frame.motion;
            self.exec.pos += furniture::to_world(t * self.scale, self.exec.heading);
            self.exec.heading -= yaw;
        }
        let in_furniture = self.in_furniture();
        for r in frame.raised {
            if in_furniture {
                self.note_untaken(&r);
            }
            self.graph_events.push(r.event.clone());
            let e = r.event.to_ascii_lowercase();
            if self.exec.combat.is_some()
                && log::log_enabled!(target: "combat_events", log::Level::Trace)
            {
                log::trace!(target: "combat_events", "{} raised {}{}", self.ref_id, r.event, r.payload.as_deref().map(|p| format!(" ({p})")).unwrap_or_default());
            }
            self.anim_object_event(&e, r.payload.as_deref());
            match e.as_str() {
                "idlefurnitureexit" => self.out_of_furniture = true,
                "weapondraw" | "weaponsheathe" => {
                    let out = e == "weapondraw";
                    self.equipment_changed |= out != self.weapon_out;
                    self.weapon_out = out;
                }
                "hitframe" => self.hit_frames |= combat::hit_frame_bit(r.payload.as_deref()),
                "arrowrelease" => self.arrow_release = true,
                "mrh_spellfire_event" => self.spell_fire[RIGHT] = true,
                "mlh_spellfire_event" => self.spell_fire[LEFT] = true,
                "mrh_equipped_event" => self.hand_equipped[RIGHT] = true,
                "mlh_equipped_event" => self.hand_equipped[LEFT] = true,
                // Body-part layers running the same clips (humanoids' arms) raise
                // their triggers again: one sound each.
                "soundplay" | "npcsoundplay" => {
                    if let Some(p) = r.payload
                        && !self.sounds.contains(&p)
                    {
                        self.sounds.push(p);
                    }
                }
                _ if !self.footsteps.contains(&e)
                    && self.footstep_set.as_ref().is_some_and(|s| s.has_tag(&e)) =>
                {
                    self.footsteps.push(e)
                }
                _ => {}
            }
        }
        frame.pose
    }

    fn note_untaken(&mut self, r: &havok::behavior::runtime::Raised) {
        if !r.taken && r.event.starts_with("Offset") {
            self.untaken_offset = Some(r.event.clone());
        }
    }

    /// Apply persistent object state, including during furniture entry fast-forward.
    fn anim_object_event(&mut self, event: &str, payload: Option<&str>) {
        match event {
            "animobjdraw" => {
                if let Some(anio) = payload
                    && !self.objects.iter().any(|o| o.eq_ignore_ascii_case(anio))
                {
                    self.objects.push(anio.to_owned());
                    self.objects_changed = true;
                }
            }
            "animobjectunequip" => {
                self.objects_changed |= !self.objects.is_empty();
                self.objects.clear();
                self.carrying = false;
            }
            _ => {}
        }
    }

    /// Play the idle `clips` describe in the furniture, then go back to the seat's
    /// loop. False when the actor isn't settled in, is already playing another
    /// idle, or the graph won't take it.
    pub fn start_sub_idle(
        &mut self,
        clips: furniture::UseClips,
        lib: &mut Clips,
        rand: &mut dyn FnMut() -> u64,
    ) -> bool {
        if !matches!(self.state, State::Use(_)) || self.leave || self.sub.is_some() {
            return false;
        }
        if !self.graph_event(&clips.event, lib) {
            return false;
        }
        // A held idle is stopped after a while; a gesture runs its course.
        self.sub = Some(if clips.idle_loops {
            SubIdle {
                left: uniform(rand, 8.0, 20.0),
                stop: clips.exits.clone(),
            }
        } else {
            SubIdle {
                left: clips.enter_time() + clips.idle.duration() + clips.exit_time() + 0.5,
                stop: Vec::new(),
            }
        });
        true
    }

    /// Advance the idle being played in the furniture. False when there is none
    /// (any more); leaving the furniture first stops it and lets it wind down.
    fn step_sub_idle(&mut self, dt: f32, clips: &mut Clips) -> bool {
        let waiting = self.graph.as_ref().is_some_and(|g| g.waiting());
        let Some(sub) = &mut self.sub else {
            return false;
        };
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
    if a > std::f32::consts::PI {
        a - std::f32::consts::TAU
    } else {
        a
    }
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
        let Some(key) = self.actor_cells.get(&actor).copied() else {
            return false;
        };
        let Some(rt) = self.cells.get_mut(&key) else {
            return false;
        };
        let Some(a) = rt.actors.iter_mut().find(|a| a.ref_id == actor) else {
            return false;
        };
        let lower = event.to_ascii_lowercase();
        if matches!(
            lower.as_str(),
            "idlestop"
                | "idlestopinstant"
                | "idleforcedefaultstate"
                | "idlechairexitstart"
                | "idlefurnitureexit"
        ) {
            if a.in_furniture() {
                a.leave = true;
            } else if let Some(g) = &mut a.graph {
                g.send_event(event);
            }
            return true;
        }
        let Some(project) = a.graph.as_ref().map(|g| g.project().clone()) else {
            return false;
        };
        let mut clips = Clips {
            vfs: &self.vfs,
            anims: &mut self.anims,
            behaviors: &mut self.behaviors,
        };
        // Seated, the event is one of the seat's own idles (`idleChairArmsCrossedVar1`).
        if matches!(a.state, State::Use(_)) && a.seat.as_ref().is_some_and(|s| s.kind != Use::Idle)
        {
            let Some(c) = clips.event_with_exits(
                event,
                &SUB_IDLE_EXITS,
                &project,
                &a.skeleton_path,
                a.female,
                &a.skeleton,
            ) else {
                return false;
            };
            let started = a.start_sub_idle(c, &mut clips, &mut xorshift(self.rng));
            if started {
                log::debug!("{actor} plays {event} in its seat");
            }
            return started;
        }
        // Planning: where the way in leaves the actor, how long it lasts.
        let Some(c) = clips.event(event, &project, &a.skeleton_path, a.female, &a.skeleton) else {
            return false;
        };
        if !a.graph_event(event, &mut clips) {
            log::debug!("{actor}: the graph won't take {event}");
            return false;
        }
        self.furniture.release(a.ref_id);
        let mut pos = a.exec.pos;
        let mut heading = a.exec.heading;
        for m in c.enter.iter().filter_map(|c| c.motion.as_ref()) {
            let (t, yaw) = m.end();
            pos += furniture::to_world(t, heading);
            heading -= yaw;
        }
        // A one-shot gesture (no loop at the end) is played through once.
        let duration = if c.idle_loops {
            secs.unwrap_or(f32::INFINITY)
        } else {
            c.idle.duration()
        };
        log::debug!(
            "{actor} plays {event} ({:.1} s in, {:.1} s out)",
            c.enter_time(),
            c.exit_time()
        );
        let enter = c.enter_time();
        a.sub = None;
        a.seat = Some(Seat {
            furniture: FormId::NULL,
            marker: 0,
            kind: Use::Idle,
            anim_type: 0,
            entry: furniture::Entry::Front,
            pos,
            heading,
            clips: Arc::new(c),
            duration,
        });
        a.state = State::Enter(enter);
        a.leave = false;
        true
    }

    /// Pick an action's event for this actor's graph. `None` means refused.
    pub(crate) fn action(&mut self, actor: FormId, name: &str) -> Option<String> {
        let project = if actor == PLAYER_REF {
            self.player_body.as_ref()?.project()?.clone()
        } else {
            self.actor_ref(actor)?.graph.as_ref()?.project().clone()
        };
        let root = self
            .idles
            .get_or_insert_with(|| idles::IdleIndex::build(&self.lo))
            .find(&self.lo, name)?;
        let ctx = crate::condition::Context {
            subject: Some(actor),
            idle: Some(crate::condition::IdleQuery {
                sneaking: (name == "ActionSneak").then(|| {
                    if actor == PLAYER_REF {
                        self.player_body
                            .as_ref()
                            .is_some_and(|b| b.graph_sneaking())
                    } else {
                        self.actor_ref(actor).is_some_and(|a| a.sneaking)
                    }
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        self.idles.as_ref()?.action(self, root, ctx, &project)
    }

    pub(crate) fn locomotion_actions(&mut self, actor: FormId) -> idles::LocomotionActions {
        idles::LocomotionActions(
            idles::LocomotionActions::NAMES
                .into_iter()
                .map(|name| (name, self.action(actor, name)))
                .collect(),
        )
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
                rt.actors.iter_mut().filter_map(|a| {
                    std::mem::take(&mut a.wants_action_idle)
                        .then(|| Some((a.ref_id, a.graph.as_ref()?.project().clone())))
                        .flatten()
                })
            })
            .collect();
        if wanting.is_empty() {
            return;
        }
        let Some(root) = self
            .idles
            .get_or_insert_with(|| idles::IdleIndex::build(&self.lo))
            .find(&self.lo, "ActionIdle")
        else {
            log::warn!("no ActionIdle in the idle tree");
            return;
        };
        for (r, project) in wanting {
            // Standing still, out of furniture.
            let ctx = Context {
                subject: Some(r),
                idle: Some(IdleQuery::default()),
                ..Default::default()
            };
            let Some((idle, event)) = self
                .idles
                .as_ref()
                .and_then(|ix| ix.select_for(self, root, ctx, &project))
            else {
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
            .flat_map(|rt| {
                rt.actors
                    .iter_mut()
                    .filter_map(|a| std::mem::take(&mut a.wants_meal).then_some(a.ref_id))
            })
            .collect();
        if wanting.is_empty() {
            return;
        }
        let Some(ix) = self.idles.as_ref() else {
            return;
        };
        let roots: Vec<FormId> = ["EatingRoot", "DrinkingRoot"]
            .iter()
            .filter_map(|n| ix.find(&self.lo, n))
            .collect();
        for r in wanting {
            let query = IdleQuery {
                eating: true,
                ..Default::default()
            };
            let ctx = Context {
                subject: Some(r),
                idle: Some(query),
                ..Default::default()
            };
            let Some((idle, event)) = self
                .idles
                .as_ref()
                .and_then(|ix| ix.select_among(self, &roots, ctx))
            else {
                continue;
            };
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
            .flat_map(|(k, rt)| {
                rt.actors.iter_mut().filter_map(move |a| {
                    std::mem::take(&mut a.wants_idle).then_some((*k, a.ref_id))
                })
            })
            .collect();
        if wanting.is_empty() {
            return;
        }
        let Some(root) = self
            .idles
            .as_ref()
            .and_then(|ix| ix.find(&self.lo, "NonCombatIdles"))
        else {
            return;
        };
        let special = self.lo.find_editor_id("FurnitureSpecial");
        for (key, r) in wanting {
            let roll = self.rand();
            let Some(a) = self
                .cells
                .get(&key)
                .and_then(|rt| rt.actors.iter().find(|a| a.ref_id == r))
            else {
                continue;
            };
            let Some(seat) = a.seat.clone() else { continue };
            // Wood piles, pour spots and the like are "sat in" too, but aren't seats.
            if special.is_some_and(|kw| self.has_keyword(seat.furniture, kw)) {
                continue;
            }
            let Some(project) = a
                .graph
                .as_ref()
                .map(|g| g.project().clone())
                .filter(|p| p.humanoid())
            else {
                continue;
            };
            let query = IdleQuery {
                anim_type: seat.anim_type,
                entry: seat.entry.entry_type(),
                state: 3.0,
                // Sandboxing actors eat now and then (food or drink in hand); the
                // rest of the time they sit, with the odd change of pose.
                eating: a
                    .exec
                    .goal
                    .is_some_and(|g| g.allow.meal || (g.allow.eating && roll % 3 == 0)),
                ..Default::default()
            };
            let ctx = Context {
                subject: Some(r),
                target: Some(seat.furniture),
                idle: Some(query),
                ..Default::default()
            };
            let Some((idle, event)) = self
                .idles
                .as_ref()
                .and_then(|ix| ix.select(self, root, ctx))
            else {
                continue;
            };
            let (skeleton_path, female, skeleton) =
                (a.skeleton_path.clone(), a.female, a.skeleton.clone());
            let mut clips = Clips {
                vfs: &self.vfs,
                anims: &mut self.anims,
                behaviors: &mut self.behaviors,
            };
            let Some(c) = clips.event_with_exits(
                &event,
                &SUB_IDLE_EXITS,
                &project,
                &skeleton_path,
                female,
                &skeleton,
            ) else {
                log::debug!("{r}: no clips for furniture idle {idle} ({event})");
                continue;
            };
            drop(clips);
            log::debug!(
                "{r} plays {event} in {} ({} enter, {} exit clips)",
                seat.furniture,
                c.enter.len(),
                c.exit.len()
            );
            let mut rand = xorshift(self.rand());
            let mut clips = Clips {
                vfs: &self.vfs,
                anims: &mut self.anims,
                behaviors: &mut self.behaviors,
            };
            if let Some(a) = self
                .cells
                .get_mut(&key)
                .and_then(|rt| rt.actors.iter_mut().find(|a| a.ref_id == r))
            {
                a.start_sub_idle(c, &mut clips, &mut rand);
            }
        }
    }

    /// Send a loaded actor to use specific furniture (Papyrus `Activate` by an NPC,
    /// the `use` console command). It stays until its package changes.
    pub fn use_furniture(&mut self, actor: FormId, furniture: FormId) -> bool {
        let Some(f) = self.furniture.get(furniture) else {
            return false;
        };
        let centre = f.markers.first().map_or(Vec3::ZERO, |m| m.pos);
        let Some(key) = self.actor_cells.get(&actor).copied() else {
            return false;
        };
        let mut seed = self.rand() | 1;
        let mut rand = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let may_use = |_: FormId, _: Use, _: Option<FormId>| true;
        let (claimed, targets) = Default::default();
        let inventories = &self.inventories;
        let holds =
            |r: FormId, item: FormId| inventories.get(&r).is_some_and(|i| i.count(item) > 0);
        let mut w = World {
            nav: &self.nav,
            furniture: &mut self.furniture,
            clips: Clips {
                vfs: &self.vfs,
                anims: &mut self.anims,
                behaviors: &mut self.behaviors,
            },
            may_use: &may_use,
            claimed: &claimed,
            patrols: &self.patrol_paths,
            targets: &targets,
            holds: &holds,
            rand: &mut rand,
        };
        let Some(a) = self
            .cells
            .get_mut(&key)
            .and_then(|rt| rt.actors.iter_mut().find(|a| a.ref_id == actor))
        else {
            return false;
        };
        a.give_up_seat(w.furniture);
        let goal = Goal {
            behaviour: Behaviour::Sit,
            centre,
            radius: 64.0,
            furniture: Some(furniture),
            ..Goal::travel(centre)
        };
        a.seek_furniture(&goal, false, &mut w)
    }

    /// Send an actor to `to` at a gait (testing): the goal is pinned, so its
    /// packages no longer replace it.
    pub fn travel_to(&mut self, actor: FormId, to: Vec3, gait: package::Gait, sneak: bool) -> bool {
        let Some(key) = self.actor_cells.get(&actor).copied() else {
            return false;
        };
        let Some(a) = self
            .cells
            .get_mut(&key)
            .and_then(|rt| rt.actors.iter_mut().find(|a| a.ref_id == actor))
        else {
            return false;
        };
        a.interrupt(&mut self.furniture);
        a.exec.goal = Some(Goal {
            gait,
            sneak,
            ..Goal::travel(to)
        });
        a.exec.pinned = true;
        true
    }

    /// Have an actor lead `target` to `to` (an Escort package's procedure), waiting
    /// whenever the target is more than `wait` behind and running to catch up when it
    /// is more than `run` ahead.
    pub fn escort(&mut self, actor: FormId, target: FormId, to: Vec3, wait: f32, run: f32) -> bool {
        let Some(key) = self.actor_cells.get(&actor).copied() else {
            return false;
        };
        let Some(a) = self
            .cells
            .get_mut(&key)
            .and_then(|rt| rt.actors.iter_mut().find(|a| a.ref_id == actor))
        else {
            return false;
        };
        a.interrupt(&mut self.furniture);
        a.exec.goal = Some(Goal {
            behaviour: Behaviour::Escort,
            target: Some(target),
            escort_wait: wait,
            escort_run: run,
            ..Goal::travel(to)
        });
        a.exec.pinned = true;
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
            let Some(pos) = self.ref_position(r) else {
                break;
            };
            out.push(PatrolPoint { ref_id: r, pos });
            cur = self.linked_ref(r, Some(FormId::NULL));
        }
        out
    }

    /// A behaviour graph variable of a loaded reference, an object's graph first as
    /// Papyrus `GetAnimationVariable*` reads it (0 for one its graph lacks); `None`
    /// when it runs no graph.
    pub fn graph_variable(&self, r: FormId, name: &str) -> Option<f32> {
        if self.object_has_graph(r) {
            return Some(self.object_graph(r)?.variable(name).unwrap_or(0.0));
        }
        let key = self.actor_cells.get(&r)?;
        let a = self.cells.get(key)?.actors.iter().find(|a| a.ref_id == r)?;
        Some(a.graph.as_ref()?.variable(name).unwrap_or(0.0))
    }

    /// How fast a loaded actor is moving.
    pub fn actor_speed(&self, r: FormId) -> Option<f32> {
        let key = self.actor_cells.get(&r)?;
        Some(
            self.cells
                .get(key)?
                .actors
                .iter()
                .find(|a| a.ref_id == r)?
                .speed,
        )
    }

    /// Whether a loaded NPC is moving at its running gait (`IsRunning`).
    pub fn actor_running(&self, r: FormId) -> bool {
        use crate::world::footsteps::Gait;
        let Some(key) = self.actor_cells.get(&r) else {
            return false;
        };
        self.cells
            .get(key)
            .and_then(|c| c.actors.iter().find(|a| a.ref_id == r))
            .is_some_and(|a| a.speed() > 1.0 && a.gait() == Gait::Run)
    }

    /// Gesture along with a line of dialogue: an idle from the `ActionTalking`
    /// tree for the speaker's graph (hands on hips, expressive or angry gestures,
    /// by the line's emotion and the pose the speaker is in).
    pub(crate) fn talking_gesture(&mut self, actor: FormId) {
        use crate::condition::{Context, IdleQuery};
        let Some(key) = self.actor_cells.get(&actor).copied() else {
            return;
        };
        let Some(a) = self
            .cells
            .get(&key)
            .and_then(|rt| rt.actors.iter().find(|a| a.ref_id == actor))
        else {
            return;
        };
        let Some(project) = a.graph.as_ref().map(|g| g.project().clone()) else {
            return;
        };
        let query = match &a.seat {
            Some(s) if a.in_furniture() => IdleQuery {
                anim_type: s.anim_type,
                entry: s.entry.entry_type(),
                state: 3.0,
                ..Default::default()
            },
            _ => IdleQuery::default(),
        };
        let target = a
            .seat
            .as_ref()
            .map(|s| s.furniture)
            .filter(|f| !f.is_null());
        let Some(root) = self
            .idles
            .get_or_insert_with(|| idles::IdleIndex::build(&self.lo))
            .find(&self.lo, "ActionTalking")
        else {
            return;
        };
        let ctx = Context {
            subject: Some(actor),
            target,
            idle: Some(query),
            ..Default::default()
        };
        let Some((idle, event)) = self
            .idles
            .as_ref()
            .and_then(|ix| ix.select_for(self, root, ctx, &project))
        else {
            log::debug!("{actor}: no talking idle");
            return;
        };
        self.send_gesture(key, actor, &idle.to_string(), &event);
    }

    /// Send `event` (the idle `label`) to the speaker's graph as a gesture; true
    /// if the graph took it.
    fn send_gesture(
        &mut self,
        key: crate::render::CellKey,
        actor: FormId,
        label: &str,
        event: &str,
    ) -> bool {
        let mut clips = Clips {
            vfs: &self.vfs,
            anims: &mut self.anims,
            behaviors: &mut self.behaviors,
        };
        let Some(a) = self
            .cells
            .get_mut(&key)
            .and_then(|rt| rt.actors.iter_mut().find(|a| a.ref_id == actor))
        else {
            return false;
        };
        let took = a.graph_event(event, &mut clips);
        log::debug!(
            "{actor} talks with {label} ({event}){}",
            if took {
                ""
            } else {
                ": the graph won't take it"
            }
        );
        took
    }

    /// Gesture along with a response: the speaker idle the INFO names for it
    /// (`SNAM`: pointing, giving, saluting...) when the speaker's graph takes
    /// its event, else the random talking gesture.
    pub(crate) fn line_gesture(&mut self, actor: FormId, idle: Option<FormId>) {
        if let Some(idle) = idle
            && let Some(key) = self.actor_cells.get(&actor).copied()
            && let Some(event) = self.idle_event(idle)
        {
            let label = self
                .lo
                .get(idle)
                .and_then(|r| r.editor_id())
                .unwrap_or_else(|| idle.to_string());
            if self.send_gesture(key, actor, &label, &event) {
                return;
            }
        }
        self.talking_gesture(actor);
    }

    /// The conversation is over: standing speakers stop their dialogue idle.
    pub(crate) fn end_talking_gestures(&mut self, actor: FormId) {
        let Some(key) = self.actor_cells.get(&actor).copied() else {
            return;
        };
        let Some(a) = self
            .cells
            .get_mut(&key)
            .and_then(|rt| rt.actors.iter_mut().find(|a| a.ref_id == actor))
        else {
            return;
        };
        if !a.in_furniture()
            && let Some(g) = &mut a.graph
        {
            g.send_event("IdleStop");
        }
    }

    /// `GetSitting` / `GetSleeping` of a loaded actor: 0 not, 2 getting in, 3 in,
    /// 4 getting out (beds count as sleeping, other furniture as sitting).
    pub fn sit_sleep_state(&self, r: FormId, sleeping: bool) -> f32 {
        let Some(a) = self
            .actor_cells
            .get(&r)
            .and_then(|k| self.cells.get(k))
            .and_then(|rt| rt.actors.iter().find(|a| a.ref_id == r))
        else {
            return 0.0;
        };
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
        let Some(key) = self.actor_cells.get(&r) else {
            return false;
        };
        let Some(g) = self
            .cells
            .get_mut(key)
            .and_then(|rt| rt.actors.iter_mut().find(|a| a.ref_id == r))
            .and_then(|a| a.graph.as_mut())
        else {
            return false;
        };
        g.set_variable(name, value);
        true
    }

    /// A loaded actor's bones (names containing `filter`): posed and bind
    /// positions in model space, and the posed bone's up axis.
    pub fn describe_bones(&self, r: FormId, filter: &str) -> Vec<String> {
        let Some(key) = self.actor_cells.get(&r) else {
            return vec![format!("{r} isn't loaded")];
        };
        let Some(i) = self
            .cells
            .get(key)
            .and_then(|c| c.actors.iter().position(|a| a.ref_id == r))
        else {
            return vec![format!("{r} isn't loaded")];
        };
        let skeleton = &self.cells[key].actors[i].skeleton;
        let Some(inst) = self.scene.cells.get(key).and_then(|c| c.actors.get(i)) else {
            return vec![format!("{r} isn't drawn")];
        };
        let bind = skeleton.model_space(&skeleton.bind_locals());
        let filter = filter.to_ascii_lowercase();
        skeleton
            .bones
            .iter()
            .enumerate()
            .filter(|(_, b)| b.name.to_ascii_lowercase().contains(&filter))
            .filter_map(|(b, bone)| {
                let m = inst.pose.get(b)?;
                let (p, q) = (m.w_axis.truncate(), bind[b].w_axis.truncate());
                let up = m.z_axis.truncate();
                Some(format!(
                    "{b:3} {:28} pose ({:7.1} {:7.1} {:7.1}) bind ({:7.1} {:7.1} {:7.1}) z ({:5.2} {:5.2} {:5.2})",
                    bone.name, p.x, p.y, p.z, q.x, q.y, q.z, up.x, up.y, up.z
                ))
            })
            .collect()
    }

    pub fn graph_states(&self, r: FormId) -> Option<Vec<String>> {
        if r == crate::engine::PLAYER_REF {
            return self.player_graph_states();
        }
        let key = self.actor_cells.get(&r)?;
        let a = self.cells.get(key)?.actors.iter().find(|a| a.ref_id == r)?;
        Some(a.graph.as_ref()?.active_states())
    }

    /// Feet position and heading of a loaded actor.
    /// Whether a loaded actor is running a package: its scene's, else the one
    /// its schedule picked.
    pub fn runs_package(&self, r: FormId, package: FormId) -> bool {
        if let Some((_, _, pkgs)) = self.scenes.packages.get(&r) {
            return pkgs.iter().any(|p| p.id == package);
        }
        let Some(a) = self
            .actor_cells
            .get(&r)
            .and_then(|k| self.cells.get(k))
            .and_then(|rt| rt.actors.iter().find(|a| a.ref_id == r))
        else {
            return false;
        };
        a.exec
            .current
            .and_then(|i| a.exec.packages.get(i))
            .is_some_and(|p| p.id == package)
    }

    /// Where a loaded actor's (or the player's) feet stand and how tall it is.
    pub fn actor_body(&self, r: FormId) -> Option<(Vec3, f32)> {
        if r == PLAYER_REF {
            let h = self.physics.player_half_height + self.physics.player_radius;
            return Some((self.player.position - Vec3::Z * h, 2.0 * h));
        }
        let key = self.actor_cells.get(&r)?;
        let a = self.cells.get(key)?.actors.iter().find(|a| a.ref_id == r)?;
        Some((a.feet(), a.height()))
    }

    pub fn actor_pose(&self, r: FormId) -> Option<(Vec3, f32)> {
        let key = self.actor_cells.get(&r)?;
        let a = self.cells.get(key)?.actors.iter().find(|a| a.ref_id == r)?;
        Some((a.exec.pos, a.exec.heading))
    }

    /// Where a package location points to, as (centre, radius).
    fn package_target(&self, a: &ActorRuntime, p: &Package) -> (Vec3, f32) {
        match p.location {
            Some(loc) => self.location_target(a, loc, p.quest),
            None => (a.editor_pos, 0.0),
        }
    }

    fn location_target(
        &self,
        a: &ActorRuntime,
        loc: package::Location,
        quest: Option<FormId>,
    ) -> (Vec3, f32) {
        let centre = match loc.kind {
            LocationKind::NearReference(r) => self.ref_position(r),
            LocationKind::NearLinkedRef(kw) => self
                .linked_ref(a.ref_id, (!kw.is_null()).then_some(kw))
                .and_then(|r| self.ref_position(r)),
            LocationKind::NearCurrent | LocationKind::NearSelf => Some(a.exec.pos),
            LocationKind::NearAlias(alias) => quest
                .and_then(|q| self.alias_ref(q, alias))
                .and_then(|r| self.ref_position(r)),
            LocationKind::InCell(_) => return (a.editor_pos, loc.radius.max(SANDBOX_MAX)),
            // About its editor place when that is in the location, else at the location's marker.
            LocationKind::InLocAlias(alias) => {
                let Some(l) = quest.and_then(|q| self.alias_ref(q, alias)) else {
                    return (a.editor_pos, loc.radius);
                };
                if self
                    .editor_location(a.ref_id)
                    .is_some_and(|e| self.location_within(e, l))
                {
                    return (a.editor_pos, loc.radius.max(SANDBOX_MAX));
                }
                self.location_marker(l).and_then(|m| self.ref_position(m))
            }
            LocationKind::NearEditor | LocationKind::Other(_) => Some(a.editor_pos),
        };
        (centre.unwrap_or(a.editor_pos), loc.radius)
    }

    /// GetWithinPackageLocation: whether `subject` is within package location
    /// `loc`, placed for `actor`, the actor whose package it is, as its force
    /// greet trigger is (`None`: that actor isn't loaded, or an alias is empty).
    /// "In cell" and "in location alias" ask whether the subject is there.
    pub(crate) fn within_package_location(
        &self,
        subject: FormId,
        actor: FormId,
        loc: package::Location,
        quest: Option<FormId>,
    ) -> Option<bool> {
        match loc.kind {
            LocationKind::InCell(c) => {
                let cell = self.lo.cell(c)?;
                return Some(match (self.ref_current_place(subject)?, cell.world) {
                    (Place::Interior(i), None) => i == c,
                    (Place::Exterior(w, g), Some(world)) => w == world && cell.grid == Some(g),
                    _ => false,
                });
            }
            LocationKind::InLocAlias(alias) => {
                let l = quest.and_then(|q| self.alias_ref(q, alias))?;
                return Some(
                    self.ref_current_location(subject)
                        .is_some_and(|s| self.location_within(s, l)),
                );
            }
            _ => {}
        }
        let (centre, radius) = self.location_target(self.actor_ref(actor)?, loc, quest);
        Some(self.ref_position(subject)?.distance(centre) <= radius)
    }

    /// The cell a reference is in now: the player's, a loaded actor's, a
    /// persistent actor's whereabouts, else where it was placed or moved to.
    fn ref_current_place(&self, r: FormId) -> Option<Place> {
        let grid = |p: Vec3| {
            let size = crate::world::terrain::CELL_SIZE;
            ((p.x / size).floor() as i32, (p.y / size).floor() as i32)
        };
        let loaded = match self.location {
            crate::engine::Location::Interior(c) if r == PLAYER_REF => Some(Place::Interior(c)),
            crate::engine::Location::Exterior { world, .. } if r == PLAYER_REF => {
                Some(Place::Exterior(world, grid(self.ref_position(r)?)))
            }
            crate::engine::Location::Exterior { world, .. } => match self.actor_cells.get(&r) {
                Some(crate::render::CellKey::Exterior(x, y)) => {
                    Some(Place::Exterior(world, (*x, *y)))
                }
                _ => None,
            },
            _ => None,
        };
        if let Some(place) = loaded {
            return Some(place);
        }
        if let Some(crate::render::CellKey::Interior(c)) = self.actor_cells.get(&r) {
            return Some(Place::Interior(*c));
        }
        if let Some((place, _)) = self.whereabouts.of.get(&r) {
            return Some(*place);
        }
        self.place_of_ref(r, self.ref_position(r)?)
    }

    /// The reference a package target input names for actor `a`.
    fn package_target_ref(
        &self,
        a: &ActorRuntime,
        p: &package::Package,
        t: Option<Target>,
    ) -> Option<FormId> {
        self.package_target_of(a.ref_id, p, t)
    }

    /// The reference package target `t` names for actor `achr`.
    pub(crate) fn package_target_of(
        &self,
        achr: FormId,
        p: &package::Package,
        t: Option<Target>,
    ) -> Option<FormId> {
        match t {
            Some(Target::Ref(r)) => Some(r),
            Some(Target::LinkedRef(kw)) => self.linked_ref(achr, kw),
            Some(Target::Alias(alias)) => p.quest.and_then(|q| self.alias_ref(q, alias)),
            // Patrolling from itself: along its own linked references.
            Some(Target::SelfRef) if p.behaviour == Behaviour::Patrol => {
                self.linked_ref(achr, Some(FormId::NULL))
            }
            Some(Target::SelfRef) => Some(achr),
            _ => None,
        }
    }

    /// For a creature sent to furniture: the ways on and off its markers that its
    /// own graph has (`ActionActivate`'s branches for its graph, with the creature
    /// as subject: their conditions name the furniture as its linked reference),
    /// unless found already.
    fn creature_furniture_ways(
        &self,
        a: &ActorRuntime,
        furniture: FormId,
    ) -> Vec<(FormId, furniture::Way)> {
        use crate::condition::{Context, IdleQuery};
        let Some(project) = a
            .graph
            .as_ref()
            .map(|g| g.project())
            .filter(|p| !p.humanoid())
        else {
            return Vec::new();
        };
        let Some(f) = self.furniture.get(furniture) else {
            return Vec::new();
        };
        if f.ways
            .iter()
            .any(|w| w.graph.as_deref() == Some(project.name.as_str()))
        {
            return Vec::new();
        }
        let Some(idles) = self.idles.as_ref() else {
            return Vec::new();
        };
        let Some(root) = idles.find(&self.lo, "ActionActivate") else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for (mi, m) in f.markers.iter().enumerate() {
            let entry = furniture::Entry::of(m.entries)
                .next()
                .map_or(furniture::Entry::Front, |e| e.0);
            let ctx = |state: f32, quick: bool| Context {
                subject: Some(a.ref_id),
                target: Some(furniture),
                idle: Some(IdleQuery {
                    anim_type: m.anim_type,
                    entry: entry.entry_type(),
                    state,
                    quick,
                    child: Some(false),
                    ..Default::default()
                }),
                ..Default::default()
            };
            let Some((_, enter)) = idles.select_for(self, root, ctx(2.0, true), project) else {
                continue;
            };
            let exit = idles
                .select_for(self, root, ctx(4.0, false), project)
                .map(|(_, e)| e)
                .filter(|e| *e != enter);
            log::debug!(
                "{} ({}) uses {furniture} marker {mi}: {enter} / {exit:?}",
                a.ref_id,
                project.name
            );
            out.push((
                furniture,
                furniture::Way {
                    marker: mi as u8,
                    entry,
                    child: false,
                    enter,
                    exit,
                    graph: Some(project.name.clone()),
                    holding: None,
                    instant_exit: None,
                },
            ));
        }
        if out.is_empty() {
            log::debug!("{} ({}): no way into {furniture}", a.ref_id, project.name);
        }
        out
    }

    /// Multi-location sandboxers: once one has been at its stop for the stop's
    /// time, on to the next (in order, or another picked at random), its
    /// package's goal worked out again at once.
    fn advance_sandbox_stops(&mut self) {
        let now = self.scripts.real_time;
        let mut seed = self.rand() | 1;
        for rt in self.cells.values_mut() {
            for a in &mut rt.actors {
                if a.exec.pinned || a.dead || a.exec.combat.is_some() {
                    continue;
                }
                let Some(p) = a.exec.current.and_then(|i| a.exec.packages.get(i)) else {
                    continue;
                };
                let (Some(stop), Some(goal)) = (p.stops.get(a.exec.stop.index), a.exec.goal) else {
                    continue;
                };
                if goal.behaviour != Behaviour::Sandbox {
                    continue;
                }
                let Some(since) = a.exec.stop.arrived else {
                    // Arrived: within the area it sandboxes in.
                    let r = goal.radius.clamp(SANDBOX_MIN, SANDBOX_MAX);
                    if a.exec.pos.truncate().distance(goal.centre.truncate()) <= r {
                        a.exec.stop.arrived = Some(now);
                    }
                    continue;
                };
                if now - since < stop.seconds as f64 {
                    continue;
                }
                let n = p.stops.len();
                a.exec.stop.index = if p.random_stops && n > 1 {
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    (a.exec.stop.index + 1 + seed as usize % (n - 1)) % n
                } else {
                    (a.exec.stop.index + 1) % n
                };
                a.exec.stop.arrived = None;
                a.exec.next_eval = 0.0;
                log::debug!(
                    "{} sandboxes on at stop {} of {}",
                    a.ref_id,
                    a.exec.stop.index,
                    p.editor_id
                );
                a.interrupt(&mut self.furniture);
            }
        }
    }

    /// Choose each actor's package and goal.
    fn evaluate_packages(&mut self, dt: f32) {
        self.refresh_alias_packages();
        let mut seed = self.rand() | 1;
        let mut rand = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let mut decisions = Vec::new();
        let mut forgotten = Vec::new();
        let mut creature_ways = Vec::new();
        for (key, rt) in &self.cells {
            for (i, a) in rt.actors.iter().enumerate() {
                if a.exec.next_eval - dt > 0.0 || a.exiting.is_some() {
                    continue;
                }
                let applies = |p: &package::Package| {
                    let ctx = crate::condition::Context {
                        subject: Some(a.ref_id),
                        quest: p.quest,
                        ..Default::default()
                    };
                    p.schedule.matches(self.hour, self.day)
                        && crate::condition::evaluate(self, &p.conditions, ctx)
                };
                // A completed package that gives way is passed over while it still
                // applies, and runs afresh once it has stopped (and starts again).
                let finished = a
                    .exec
                    .done_once
                    .filter(|&id| a.exec.packages.iter().any(|p| p.id == id && applies(p)));
                if finished.is_none() && a.exec.done_once.is_some() {
                    forgotten.push((*key, i));
                }
                let pick = a
                    .exec
                    .packages
                    .iter()
                    .position(|p| applies(p) && Some(p.id) != finished);
                // A multi-location sandbox starts at its first stop, or a random one.
                let stop = match pick.map(|pi| &a.exec.packages[pi]) {
                    Some(_) if pick == a.exec.current => a.exec.stop,
                    Some(p) if p.random_stops && !p.stops.is_empty() => StopProgress {
                        index: rand() as usize % p.stops.len(),
                        arrived: None,
                    },
                    _ => StopProgress::default(),
                };
                let goal = pick.map(|pi| {
                    let p = &a.exec.packages[pi];
                    let (centre, radius) = match p.stops.get(stop.index) {
                        Some(s) => self.location_target(a, s.location, p.quest),
                        None => self.package_target(a, p),
                    };
                    let target = self.package_target_ref(a, p, p.target);
                    // A force greeter waits (standing, sandboxing or seated) until it
                    // sets off to the player.
                    let (behaviour, target) = match p.greet {
                        Some(g) if g.seated => (Behaviour::Sit, target),
                        Some(_) if a.exec.greeting => (Behaviour::Follow, Some(PLAYER_REF)),
                        Some(g) if g.sandbox => (Behaviour::Sandbox, None),
                        Some(_) => (Behaviour::Travel, None),
                        None => (p.behaviour, target),
                    };
                    // Only Sit / Sleep packages target furniture; a patrol start or follow
                    // target can be an idle marker without being "the package's seat".
                    let furniture = target
                        .filter(|_| {
                            matches!(
                                behaviour,
                                Behaviour::Sit | Behaviour::Sleep | Behaviour::Activate
                            )
                        })
                        .filter(|r| self.furniture.get(*r).is_some());
                    if let Some(f) = furniture {
                        creature_ways.extend(self.creature_furniture_ways(a, f));
                    }
                    let (centre, radius) = match behaviour {
                        Behaviour::Patrol => (centre, p.point_radius),
                        Behaviour::Follow => (
                            target.and_then(|t| self.ref_position(t)).unwrap_or(centre),
                            radius,
                        ),
                        _ => (centre, radius),
                    };
                    // Walking up to the player: stop within the force greet distance.
                    let follow_radius = match p.greet {
                        Some(g) if behaviour == Behaviour::Follow => (g.distance * 0.5, g.distance),
                        _ => p.follow_radius,
                    };
                    let practice = p.use_weapon.map(|u| {
                        let targets = u.targets.map(|t| self.package_target_ref(a, p, t));
                        practice::PracticeGoal {
                            weapon: u.weapon,
                            targets,
                            aim_offsets: targets.map(|t| {
                                t.map_or(Vec3::ZERO, |t| {
                                    practice::aim_point(&self.lo, t, Vec3::ZERO)
                                })
                            }),
                            triggers: u.triggers.map(|t| {
                                t.map(|t| practice::PracticeTrigger {
                                    reference: self.package_target_ref(a, p, Some(t.reference)),
                                    radius: t.radius,
                                })
                            }),
                            aim_only: u.aim_only,
                            always_power_attack: u.always_power_attack,
                            pause: u.pause,
                            attacks: u.attacks,
                            never_end: u.never_end,
                            barrages: u.barrages,
                            max_time: u.max_time,
                        }
                    });
                    let magic = p.use_magic.and_then(|u| {
                        let target = u
                            .target
                            .map_or(Some(a.ref_id), |t| self.package_target_ref(a, p, Some(t)));
                        Some(magic_practice::MagicGoal {
                            spell: u.spell?,
                            target,
                            aim_offset: target.filter(|&t| t != a.ref_id).map_or(Vec3::ZERO, |t| {
                                practice::aim_point(&self.lo, t, Vec3::ZERO)
                            }),
                            hold_when_blocked: u.hold_when_blocked,
                            cast_time: u.cast_time,
                            cooldown: u.cooldown,
                            casts: u.casts,
                            dual: u.dual,
                        })
                    });
                    Goal {
                        practice,
                        magic,
                        activate: p.activate,
                        any_furniture: matches!(
                            p.target,
                            Some(Target::ObjectType(package::OBJECT_TYPE_FURNITURE))
                        ),
                        behaviour,
                        centre,
                        radius,
                        allow: p.allow,
                        energy: p.energy,
                        furniture,
                        target,
                        repeat: p.repeat,
                        start_nearest: p.start_nearest,
                        follow_radius,
                        escort_wait: p.escort_wait,
                        escort_run: p.escort_run,
                        gait: p.gait,
                        sneak: p.sneak,
                    }
                });
                decisions.push((*key, i, pick, goal, stop));
            }
        }
        for rt in self.cells.values_mut() {
            for a in &mut rt.actors {
                a.exec.next_eval -= dt;
            }
        }
        for (key, i) in forgotten {
            if let Some(a) = self.cells.get_mut(&key).and_then(|rt| rt.actors.get_mut(i)) {
                a.exec.done_once = None;
            }
        }
        for (f, way) in creature_ways {
            if let Some(f) = self.furniture.items.iter_mut().find(|x| x.ref_id == f) {
                f.ways.push(way);
            }
        }
        let mut lock_changes = Vec::new();
        let now = self.game_hours_total();
        for (key, i, pick, goal, stop) in decisions {
            let Some(a) = self.cells.get_mut(&key).and_then(|rt| rt.actors.get_mut(i)) else {
                continue;
            };
            // Force greeters keep a closer eye on the player.
            let greeter = pick.is_some_and(|pi| a.exec.packages[pi].greet.is_some());
            a.exec.next_eval = if greeter {
                greet::EVAL_INTERVAL
            } else {
                EVAL_INTERVAL
            };
            if a.exec.pinned || a.dead {
                continue;
            }
            // Home doors follow the package as it starts, not at the next
            // whereabouts refresh.
            let id = pick.map(|pi| a.exec.packages[pi].id);
            let old = match self.whereabouts.lock_package.get(&a.ref_id) {
                Some(&old) => old,
                None => a.exec.current.map(|pi| a.exec.packages[pi].id),
            };
            if old != id {
                lock_changes.push((a.ref_id, old, id));
            }
            if pick != a.exec.current {
                if let Some(p) = pick.map(|pi| &a.exec.packages[pi]) {
                    log::debug!(
                        "{} -> package {} ({}, {:?})",
                        a.ref_id,
                        p.editor_id,
                        p.template,
                        goal
                    );
                }
                // The first package an actor gets doesn't interrupt what it is doing
                // (e.g. an idle a script started on load).
                let first = a.exec.current.is_none();
                let for_scene = self
                    .scenes
                    .packages
                    .get(&a.ref_id)
                    .is_some_and(|(_, _, ps)| ps.iter().any(|p| Some(p.id) == id));
                a.exec.begin_run(pick, now, for_scene);
                a.exec.current = pick;
                a.exec.goal = goal;
                a.turn_after_arrival = arrival_turn_after_package_eval(a.turn_after_arrival, goal);
                a.exec.stop = stop;
                if !first {
                    a.interrupt(&mut self.furniture);
                }
            } else {
                // A force greeter setting off to the player (or giving up) stops
                // what it is doing.
                // One without a goal yet (back from a save) has nothing to stop.
                let changed = a
                    .exec
                    .goal
                    .is_some_and(|g| Some(g.behaviour) != goal.map(|g| g.behaviour));
                a.exec.goal = goal;
                a.turn_after_arrival = arrival_turn_after_package_eval(a.turn_after_arrival, goal);
                if changed {
                    a.interrupt(&mut self.furniture);
                }
            }
        }
        if !lock_changes.is_empty() {
            self.package_door_locks(lock_changes);
        }
    }

    /// Run AI and animation for every loaded actor.
    pub(crate) fn update_actors(&mut self, dt: f32) {
        self.update_force_greets(dt);
        self.advance_sandbox_stops();
        self.evaluate_packages(dt);
        self.update_practice_weapons();
        // Choose before the AI requests landings, so a newly needed heal does
        // not replace the offensive spell that an earlier step already threw.
        self.update_magic_blocks();
        self.update_combat_spells(dt);
        self.update_actor_encumbrance();
        let talking = self.conversation.as_ref().map(|c| c.npc_ref).or(self
            .barks
            .current
            .as_ref()
            .map(|b| b.speaker));
        let player = self.ref_position(PLAYER_REF).unwrap_or_default();
        let eye = self.player.eye();
        // Scene dialogue: whom actors look at (heads), and whether they turn to them.
        let scene_look: std::collections::HashMap<FormId, (Vec3, bool)> = self
            .scene_headtracking()
            .into_iter()
            .filter_map(|(r, (t, face))| {
                // Loaded actors only: others are elsewhere.
                let at = if t == PLAYER_REF {
                    eye
                } else {
                    self.actor_ref(t)
                        .map(|a| a.exec.pos + Vec3::Z * 110.0 * a.scale)?
                };
                Some((r, (at, face)))
            })
            .collect();
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
            match a.exec.goal {
                Some(g) if g.behaviour == Behaviour::Patrol => patrol_starts.extend(g.target),
                Some(g) if matches!(g.behaviour, Behaviour::Follow | Behaviour::Escort) => {
                    if let Some(t) = g.target
                        && let Some(p) = self.ref_position(t)
                    {
                        targets.insert(t, p);
                    }
                }
                // Practice distance checks use reference origins; aim offsets
                // are separate so another actor's follow target stays an origin.
                Some(Goal {
                    practice: Some(pg), ..
                }) => {
                    for t in pg.targets.into_iter().flatten().chain(
                        pg.triggers
                            .into_iter()
                            .flatten()
                            .filter_map(|t| t.reference),
                    ) {
                        if let Some(p) = self.ref_position(t) {
                            targets.insert(t, p);
                        }
                    }
                }
                Some(Goal {
                    magic: Some(mg), ..
                }) => {
                    if let Some(t) = mg.target
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
        let actors: Vec<_> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| !a.dead && a.graph.is_some())
            .map(|a| a.ref_id)
            .collect();
        for actor in actors {
            let actions = self.locomotion_actions(actor);
            if let Some(a) = self.actor_mut(actor) {
                a.actions = actions;
            }
        }
        let nav = std::mem::take(&mut self.nav);
        let mut furniture = std::mem::take(&mut self.furniture);
        let (lo, vfs) = (&self.lo, &self.vfs);
        let clips = Clips {
            vfs,
            anims: &mut self.anims,
            behaviors: &mut self.behaviors,
        };
        let may_use = |npc: FormId, kind: Use, owner: Option<FormId>| {
            furniture::may_use(lo, npc, kind, owner)
        };
        let inventories = &self.inventories;
        let holds =
            |r: FormId, item: FormId| inventories.get(&r).is_some_and(|i| i.count(item) > 0);
        let claimed = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter_map(|a| Some((a.exec.goal?.furniture?, a.ref_id)))
            .collect();
        let mut world = World {
            nav: &nav,
            furniture: &mut furniture,
            clips,
            may_use: &may_use,
            claimed: &claimed,
            patrols: &self.patrol_paths,
            targets: &targets,
            holds: &holds,
            rand: &mut rand,
        };
        let mut moved = Vec::new();
        let mut gone = Vec::new();
        let mut held = Vec::new();
        let mut equip = Vec::new();
        let mut sounds: Vec<(String, Vec3)> = Vec::new();
        let mut anim_events: Vec<(FormId, Vec<String>)> = Vec::new();
        let mut activations: Vec<(FormId, FormId)> = Vec::new();
        let mut steps: Vec<crate::footsteps::Step> = Vec::new();
        let listener = self.camera.position;
        let mut swings: Vec<combat::Swing> = Vec::new();
        // Attacks started this frame (attacker, target): their targets may block.
        let mut started: Vec<(FormId, FormId)> = Vec::new();
        let mut lost: Vec<FormId> = Vec::new();
        // Where everyone is, for fights.
        let mut positions: std::collections::HashMap<FormId, Vec3> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| !a.dead)
            .map(|a| (a.ref_id, a.exec.pos))
            .collect();
        if self.player_died_at.is_none() {
            // The player's feet.
            let feet = self.player.position
                - Vec3::Z * (self.physics.player_half_height + self.physics.player_radius);
            positions.insert(PLAYER_REF, feet);
        }
        // Everyone's position last frame, for walkers to keep clear of.
        let mut bodies: Vec<(FormId, Vec3)> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .map(|a| (a.ref_id, a.exec.pos))
            .collect();
        bodies.push((PLAYER_REF, player));
        // And the footprints of those standing, for fighters to keep out of.
        let footprints: Vec<Footprint> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| !a.dead && !a.in_furniture())
            .map(|a| a.footprint())
            .collect();
        if log::log_enabled!(log::Level::Trace) {
            let free: Vec<&ActorRuntime> = self
                .cells
                .values()
                .flat_map(|rt| &rt.actors)
                .filter(|a| !a.in_furniture())
                .collect();
            let mut overlaps = 0;
            for (i, a) in free.iter().enumerate() {
                for b in &free[i + 1..] {
                    if a.exec.pos.distance(b.exec.pos) < 35.0 {
                        overlaps += 1;
                        log::trace!(
                            "overlap {} {:?} / {} {:?}",
                            a.ref_id,
                            a.state_name(),
                            b.ref_id,
                            b.state_name()
                        );
                    }
                }
            }
            log::trace!("actor overlaps: {overlaps}");
        }
        for (key, rt) in self.cells.iter_mut() {
            let Some(rc) = self.scene.cells.get_mut(key) else {
                continue;
            };
            for (index, (inst, a)) in rc.actors.iter_mut().zip(rt.actors.iter_mut()).enumerate() {
                if a.dead {
                    if let Some((rd, mapping)) = &a.ragdoll {
                        let bodies = self.physics.ragdoll_bodies(rd);
                        inst.pose = mapping.pose(&a.skeleton, &bodies);
                        // Where the body lies (dragged, fallen): its first body's
                        // place, on the lowest of them.
                        if let Some(first) = bodies.first() {
                            let low = bodies
                                .iter()
                                .map(|m| m.w_axis.z)
                                .fold(f32::INFINITY, f32::min);
                            a.exec.pos = first.w_axis.truncate().with_z(low);
                        }
                    }
                    if std::mem::take(&mut a.objects_changed) {
                        held.push((*key, index, Vec::new()));
                    }
                    continue;
                }
                let was = a.exec.pos;
                if talking == Some(a.ref_id) {
                    // Seated actors talk from where they are.
                    if !a.in_furniture() {
                        a.halt(3.0);
                        let d = player - a.exec.pos;
                        a.turn_towards(d.normalize_or_zero(), dt);
                    }
                } else if let Some(&(at, true)) = scene_look.get(&a.ref_id).filter(|_| {
                    self.ai_enabled
                        && a.exec.combat.is_none()
                        && !a.is_walking()
                        && !a.in_furniture()
                }) {
                    // Saying a scene line to someone: stand and face them.
                    a.turn_after_arrival = None;
                    a.halt(1.0);
                    a.turn_towards(
                        (at - a.exec.pos).truncate().extend(0.0).normalize_or_zero(),
                        dt,
                    );
                } else if !self.ai_enabled
                    || a.exec.bleeding.is_some()
                    || self.ai_off.contains(&a.ref_id)
                {
                    a.halt(1.0);
                } else if let Some(target) = a.exec.combat.as_ref().map(|c| c.target) {
                    // Fighters lose a target that goes undetected (`detection`);
                    // fleeing ones leave the fight once safe (`threat`). Here
                    // only one gone (dead, unloaded) is lost.
                    match positions.get(&target) {
                        Some(&tp) => {
                            let step = a.combat_step(dt, &mut world, tp);
                            if let Some(ev) = step.as_deref().filter(|e| e.starts_with("BeginCast"))
                            {
                                // A cast, its events already sent by the caster.
                                log::debug!("{} casts: {ev}", a.ref_id);
                                started.push((a.ref_id, target));
                            } else if let Some(ev) = step {
                                // Attacks the graph has no state for fall back to the basic
                                // one (not a bow's draw, release or lowering).
                                let bow = matches!(
                                    ev.as_str(),
                                    "bowAttackStart" | "attackRelease" | "attackStop"
                                );
                                let own = a.graph_event(&ev, &mut world.clips);
                                let took =
                                    own || (!bow && a.graph_event("attackStart", &mut world.clips));
                                log::debug!(
                                    "{} {}: {ev}{}",
                                    a.ref_id,
                                    if bow { "bow" } else { "swings" },
                                    if took {
                                        ""
                                    } else {
                                        " (the graph won't take it)"
                                    }
                                );
                                if own {
                                    let cost = a.exec.combat.as_ref().map_or(0.0, |c| c.cost);
                                    a.spend_stamina(cost);
                                }
                                if !took && ev != "attackStop" {
                                    // Its graph isn't readied (a stagger cut the draw
                                    // short, say): draw again. A bow still finishing
                                    // its release is out: just try again shortly.
                                    if !bow || !a.weapon_out {
                                        a.drawn = false;
                                    }
                                    if let Some(c) = a.exec.combat.as_mut() {
                                        c.refused();
                                    }
                                }
                                if !bow {
                                    started.push((a.ref_id, target));
                                }
                            }
                            a.keep_apart(&footprints, target, &nav);
                        }
                        None => {
                            log::debug!("{} loses {target} (gone)", a.ref_id);
                            lost.push(a.ref_id)
                        }
                    }
                } else {
                    let before = a.exec.pos;
                    if a.step(dt, &mut world) && a.is_walking() {
                        a.keep_clear(&bodies, &nav, dt, before);
                    }
                }
                a.collide(&self.physics, &nav, was);
                a.velocity = if dt > 0.0 {
                    (a.exec.pos - was) / dt
                } else {
                    Vec3::ZERO
                };
                // NPCs look at the player close by, and while talking to them.
                let near = a.exec.pos.distance(player) < HEAD_TRACK_DISTANCE;
                a.look_at = (near || talking == Some(a.ref_id)).then_some(eye);
                if let Some(&(at, _)) = scene_look.get(&a.ref_id) {
                    a.look_at = Some(at);
                }
                a.stand_on_ground(&self.physics, dt);
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
                let frames = std::mem::take(&mut a.hit_frames);
                if frames != 0 {
                    swings.extend(a.strike(frames));
                }
                for s in a.sounds.drain(..) {
                    sounds.push((s, a.exec.pos + Vec3::Z * 64.0 * a.scale));
                }
                if !a.graph_events.is_empty() {
                    anim_events.push((a.ref_id, std::mem::take(&mut a.graph_events)));
                }
                if let Some(f) = a.activated.take() {
                    activations.push((f, a.ref_id));
                }
                if let Some(set) = a
                    .footstep_set
                    .as_ref()
                    .filter(|_| a.exec.pos.distance(listener) < crate::footsteps::HEARING)
                {
                    let gait = a.gait();
                    steps.extend(a.footsteps.drain(..).map(|tag| crate::footsteps::Step {
                        tag,
                        gait,
                        at: a.exec.pos,
                        set: set.clone(),
                    }));
                }
                a.footsteps.clear();
                if a.is_walking()
                    || matches!(
                        a.state,
                        State::Approach(_) | State::Enter(_) | State::Exit(_) | State::Use(_)
                    )
                    || talking == Some(a.ref_id)
                {
                    moved.push((a.ref_id, a.feet(), a.capsule));
                }
                // Reached the door (or gave up out of sight): leave the cell.
                if a.exiting.is_some()
                    && !a.is_walking()
                    && a.exec.goal.is_none_or(|g| {
                        g.centre.truncate().distance(a.exec.pos.truncate()) < 64.0
                            || a.exec.pos.distance(player) > 2500.0
                    })
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
            self.lose_target(r);
        }
        self.update_guards(dt, &started);
        self.resolve_swings(swings);
        self.update_bleedouts(dt);
        self.update_clear_shots();
        self.loose_arrows();
        self.npc_casts();
        self.update_stamina(dt);
        self.update_threat(dt);
        if self.ai_enabled {
            self.update_detection(dt);
            self.detect_enemies(dt);
            self.update_commanded();
            self.update_arrests(dt);
            self.update_witnesses();
        }
        self.report_combat_states();
        self.complete_arrivals();
        self.report_package_runs();
        for (sound, at) in sounds {
            self.play_sound_at(&sound, at);
        }
        for (r, events) in anim_events {
            self.raise_anim_events(r, &events);
        }
        // Getting into furniture activates it (`CarryFurnitureScript` registers
        // for the user's animation events then).
        for (furniture, actor) in activations {
            log::debug!("{actor} activates {furniture}");
            self.activate_ref(furniture, Some(actor), false);
        }
        for step in &steps {
            self.play_footstep(step);
        }
        self.play_furniture_idles();
        self.play_standing_meals();
        self.play_action_idles();
        self.update_torches();
        self.update_worn();
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
