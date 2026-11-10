//! Living ragdolls and graph-driven recovery. Evidence and provisional policies:
//! research/living-ragdolls.md and known_ambiguities/ragdolls.md.
use esp::FormId;
use glam::{Mat4, Vec3};

use crate::engine::{Engine, PLAYER_REF};
use crate::physics::{Ragdoll, RagdollMode};
use crate::world::ragdoll::RagdollPose;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum KnockState {
    Normal = 0,
    Queued = 5,
    GetUp = 6,
    Down = 7,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum KnockCause {
    Push,
    Area,
    Paralysis,
    Script,
    Fall,
}

pub(crate) struct Knock {
    pub state: KnockState,
    pub ragdoll: Ragdoll,
    pub mapping: RagdollPose,
    pub cause: KnockCause,
    pub down_for: f32,
    pub rest_for: f32,
    pub peak_z: f32,
    pub fall_applied: bool,
    pub script_hold: bool,
    pub getup_requested: bool,
    pub controller: bool,
    pub actor_transform: Mat4,
    pub recovery_pose: Option<Vec<Mat4>>,
    pub recovery_time: f32,
    pub previous_view: Option<bool>,
    /// While paralysed: the pose (model space) it stiffened in when the
    /// paralysis struck, which the motors hold.
    pub hold_pose: Option<Vec<Mat4>>,
}

/// `iGetUpType`: the ordinary get-up (1), or the swimming one (2) with the
/// pelvis under water. Inferred from the selector's child order.
fn get_up_type(water: Option<f32>, pelvis: f32) -> f32 {
    if water.is_some_and(|w| w >= pelvis) {
        2.0
    } else {
        1.0
    }
}

impl Engine {
    pub(crate) fn knocked_state(&self, actor: FormId) -> KnockState {
        if actor == PLAYER_REF {
            self.player_knock.as_ref()
        } else {
            self.actor_ref(actor).and_then(|a| a.knock.as_ref())
        }
        .map_or(KnockState::Normal, |k| k.state)
    }

    pub(crate) fn is_knocked(&self, actor: FormId) -> bool {
        self.knocked_state(actor) != KnockState::Normal
    }

    /// The entry point for pushes, paralysis, and future explosion knockdowns.
    /// Returns false when no recoverable ragdoll can be made.
    pub(crate) fn knock_down(&mut self, actor: FormId, impulse: Vec3, cause: KnockCause) -> bool {
        if !impulse.is_finite() || self.is_dead(actor) || self.is_ethereal(actor) {
            return false;
        }
        if self.actor_ref(actor).is_some_and(|a| a.knock.is_some()) {
            let mut k = self.actor_mut(actor).unwrap().knock.take().unwrap();
            let restart = k.state == KnockState::GetUp || k.getup_requested;
            if restart {
                self.physics.ragdoll_into_corpse(&mut k.ragdoll);
                k.state = KnockState::Down;
                k.down_for = 0.0;
                k.getup_requested = false;
                k.controller = false;
                k.recovery_time = 0.0;
                k.recovery_pose = None;
                self.release_actor_constraints(actor);
                let a = self.actor_mut(actor).unwrap();
                if let Some(g) = a.graph.as_mut() {
                    g.clear_ragdoll_pose();
                    g.send_event("Ragdoll");
                }
                let capsule = a.capsule;
                if let Some(c) = capsule {
                    self.physics.set_enabled(&[c], false);
                }
            }
            self.physics
                .apply_ragdoll_impulse(&k.ragdoll, impulse, None);
            k.rest_for = 0.0;
            // A push mid-fall keeps the height already fallen from.
            if let Some(m) = self.physics.ragdoll_bodies(&k.ragdoll).first() {
                k.peak_z = k.peak_z.max(m.w_axis.z);
            }
            k.fall_applied = false;
            if cause == KnockCause::Script {
                k.script_hold = true;
            }
            if cause == KnockCause::Paralysis {
                k.cause = cause;
            }
            self.actor_mut(actor).unwrap().knock = Some(k);
            return true;
        }
        if actor == PLAYER_REF
            && cause != KnockCause::Paralysis
            && self
                .race_of(actor)
                .and_then(|r| self.lo.get(r))
                .is_some_and(|r| {
                    r.get(b"DATA").filter(|d| d.len() >= 36).is_some_and(|d| {
                        u32::from_le_bytes(d[32..36].try_into().unwrap()) & 0x400000 != 0
                    })
                })
        {
            return false;
        }
        if actor == PLAYER_REF {
            return self.knock_player(impulse, cause);
        }
        if self.passengers.contains_key(&actor) || self.rides.contains_key(&actor) {
            return false;
        }
        let no_knockdowns = self
            .race_of(actor)
            .and_then(|id| self.lo.get(id))
            .is_some_and(|r| {
                r.get(b"DATA").filter(|d| d.len() >= 36).is_some_and(|d| {
                    u32::from_le_bytes(d[32..36].try_into().unwrap()) & 0x400000 != 0
                })
            });
        if cause != KnockCause::Paralysis && no_knockdowns {
            return false;
        }
        let count = self
            .cells
            .values()
            .flat_map(|r| &r.actors)
            .filter(|a| a.knock.is_some())
            .count();
        if count >= 16 {
            return false;
        }
        let Some(key) = self.actor_cells.get(&actor).copied() else {
            return false;
        };
        let Some(index) = self
            .cells
            .get(&key)
            .and_then(|r| r.actors.iter().position(|a| a.ref_id == actor))
        else {
            return false;
        };
        let a = &self.cells[&key].actors[index];
        if a.in_furniture()
            || a.mount_driver.is_some()
            || !a.graph.as_ref().is_some_and(|g| g.can_get_up())
        {
            return false;
        }
        let Some(desc) = a.skeleton.ragdoll.clone() else {
            return false;
        };
        let Some(inst) = self.scene.cells.get(&key).and_then(|r| r.actors.get(index)) else {
            return false;
        };
        let (pose, transform, skeleton, velocity, capsule) = (
            inst.pose.clone(),
            inst.transform,
            a.skeleton.clone(),
            a.velocity,
            a.capsule,
        );
        let r = self.physics.spawn_ragdoll(
            &desc,
            |i| transform * pose[desc.bodies[i].bone] * desc.bodies[i].offset,
            velocity + impulse,
            actor,
            RagdollMode::Limp,
        );
        if let Some(c) = capsule {
            self.physics.set_enabled(&[c], false);
        }
        self.interrupt_cast(actor);
        let a = &mut self.cells.get_mut(&key).unwrap().actors[index];
        a.pause_for_knock();
        a.graph.as_mut().unwrap().send_event("Ragdoll");
        a.knock = Some(Knock {
            state: KnockState::Queued,
            mapping: RagdollPose::new(&desc, &skeleton, &pose, transform),
            ragdoll: r,
            cause,
            down_for: 0.0,
            rest_for: 0.0,
            peak_z: transform.w_axis.z,
            fall_applied: false,
            script_hold: cause == KnockCause::Script,
            getup_requested: false,
            controller: false,
            actor_transform: transform,
            recovery_pose: None,
            recovery_time: 0.0,
            previous_view: None,
            hold_pose: None,
        });
        self.stop_knocked_voice(actor);
        true
    }

    pub(crate) fn request_get_up(&mut self, actor: FormId) -> bool {
        let paralyzed = self.actor_value(actor, esp::actor_value::PARALYSIS) > 0.0;
        let get_up_type = self.knock_get_up_type(actor);
        let knock = if actor == PLAYER_REF {
            self.player_knock.as_mut()
        } else {
            self.actor_mut(actor).and_then(|a| a.knock.as_mut())
        };
        let Some(k) = knock else { return false };
        // ForceRemove releases ForceAdd's hold even if paralysis delays recovery.
        k.script_hold = false;
        if paralyzed {
            return false;
        }
        if actor == PLAYER_REF {
            return self.request_player_get_up(get_up_type);
        }
        let Some(a) = self.actor_mut(actor) else {
            return false;
        };
        let Some(k) = a.knock.as_mut() else {
            return false;
        };
        if k.getup_requested {
            return true;
        }
        k.getup_requested = true;
        if let Some(g) = a.graph.as_mut() {
            g.set_variable("iGetUpType", get_up_type);
            g.send_event("GetUpBegin");
        }
        true
    }

    /// `iGetUpType` for a knocked actor, from its first body and the water.
    fn knock_get_up_type(&self, actor: FormId) -> f32 {
        let knock = if actor == PLAYER_REF {
            self.player_knock.as_ref()
        } else {
            self.actor_ref(actor).and_then(|a| a.knock.as_ref())
        };
        let Some(pelvis) = knock.and_then(|k| {
            self.physics
                .ragdoll_bodies(&k.ragdoll)
                .first()
                .map(|m| m.w_axis.truncate())
        }) else {
            return 1.0;
        };
        get_up_type(self.water_level(pelvis), pelvis.z)
    }

    fn stop_knocked_voice(&mut self, actor: FormId) {
        self.stop_script_say(actor);
        self.stop_scene_voice_for_knock(actor);
        if actor == PLAYER_REF
            || self
                .conversation
                .as_ref()
                .is_some_and(|c| c.npc_ref == actor)
        {
            self.end_conversation();
        }
        if self
            .barks
            .current
            .as_ref()
            .is_some_and(|b| b.speaker == actor)
        {
            if let Some(b) = self.barks.current.take() {
                if let (Some(v), Some(audio)) = (b.voice, &self.audio) {
                    audio.stop(v);
                }
            }
        }
    }
}

/// Result of one physical/graph frame, shared by NPCs and the player.
struct KnockFrame {
    pose: Vec<Mat4>,
    pos: Vec3,
    heading: f32,
    done: bool,
    events: Vec<String>,
    fall: Option<f32>,
}

fn lying_position(bodies: &[Mat4], fallback: Vec3) -> Vec3 {
    let Some(first) = bodies.first() else {
        return fallback;
    };
    first.w_axis.truncate().with_z(
        bodies
            .iter()
            .map(|m| m.w_axis.z)
            .fold(f32::INFINITY, f32::min),
    )
}

#[allow(clippy::too_many_arguments)]
fn advance(
    k: &mut Knock,
    graph: &mut crate::world::behavior::GraphAnim,
    skeleton: &crate::world::skeleton::Skeleton,
    physics: &mut crate::physics::Physics,
    vfs: &vfs::Vfs,
    anims: &mut crate::world::animation::AnimationLibrary,
    dt: f32,
    mut pos: Vec3,
    mut heading: f32,
    paralysis: bool,
    water: Option<f32>,
) -> KnockFrame {
    let bodies = physics.ragdoll_bodies(&k.ragdoll);
    let physical = k.mapping.pose(skeleton, &bodies);
    let low = lying_position(&bodies, pos);
    let pelvis = bodies.first().map_or(low.z, |b| b.w_axis.z);
    k.peak_z = k.peak_z.max(pelvis);
    if (k.down_for + dt).floor() != k.down_for.floor() {
        let velocities = physics.ragdoll_velocities(&k.ragdoll);
        log::debug!(
            "knock down={:.2} rest={:.2} state={:?} request={} max_linear={:.2} max_angular={:.2}",
            k.down_for,
            k.rest_for,
            k.state,
            k.getup_requested,
            velocities
                .iter()
                .map(|(v, _)| v.length())
                .fold(0.0, f32::max),
            velocities
                .iter()
                .map(|(_, w)| w.length())
                .fold(0.0, f32::max)
        );
    }
    k.down_for += dt;
    let mut fall = None;
    if k.state == KnockState::Queued {
        k.state = KnockState::Down;
    }
    if k.state == KnockState::Down {
        pos = low;
        if physics.ragdoll_at_rest(&k.ragdoll, 35.0, 2.0) {
            k.rest_for += dt;
        } else {
            k.rest_for = 0.0;
        }
        // Each landing counts the drop since the last one, so a body that
        // rests on a ledge and then slides off it is hurt by the second fall.
        if k.rest_for == 0.0 && k.fall_applied && pelvis < k.peak_z - 1.0 {
            k.fall_applied = false;
        }
        if k.rest_for >= 0.5 && !k.fall_applied {
            k.fall_applied = true;
            if k.cause != KnockCause::Paralysis && !paralysis && !water.is_some_and(|w| w >= pelvis)
            {
                fall = Some(k.peak_z - pelvis);
            }
            k.peak_z = pelvis;
        }
        if !k.getup_requested
            && !k.script_hold
            && !paralysis
            && k.down_for >= 1.0
            && k.rest_for >= 0.5
        {
            k.getup_requested = true;
            graph.set_variable("iGetUpType", get_up_type(water, pelvis));
            graph.send_event("GetUpBegin");
        }
    }
    let moved = k.mapping.moved_bones(skeleton);
    if paralysis {
        k.hold_pose.get_or_insert_with(|| physical.clone());
    } else {
        k.hold_pose = None;
    }
    graph.set_ragdoll_pose(physical.clone(), &moved, skeleton, vfs, anims);
    let mut frame = graph.update(dt, vfs, anims, skeleton);
    let recovering = frame.ragdoll.get_up.is_some()
        || graph.active_states().iter().any(|s| {
            s.to_ascii_lowercase()
                .replace([' ', '_'], "")
                .contains("getupfromragdoll")
        });
    if k.state != KnockState::GetUp && recovering {
        k.state = KnockState::GetUp;
        heading -= frame.ragdoll.matching_yaw.unwrap_or(0.0);
        let from = low + Vec3::Z * 64.0;
        let (ground, normal) = physics
            .recovery_ground(from, 256.0)
            .map_or((low, Vec3::Z), |(d, n)| (from - Vec3::Z * d, n));
        pos = ground;
        graph.set_ragdoll_ground(glam::Quat::from_rotation_z(heading) * normal);
        let transform = Mat4::from_scale_rotation_translation(
            Vec3::splat(k.ragdoll.scale),
            glam::Quat::from_rotation_z(-heading),
            pos,
        );
        k.mapping.rebase(transform);
        k.actor_transform = transform;
        let source = k.mapping.pose(skeleton, &bodies);
        k.recovery_pose = Some(source.clone());
        graph.reset_get_up_source();
        graph.set_ragdoll_pose(source, &moved, skeleton, vfs, anims);
        let again = graph.update(0.0, vfs, anims, skeleton);
        frame.pose = again.pose;
    }
    let events: Vec<_> = frame.raised.iter().map(|r| r.event.clone()).collect();
    if events.iter().any(|e| {
        e.eq_ignore_ascii_case("AddCharacterControllerToWorld")
            || e.eq_ignore_ascii_case("SwimStartFromRagdoll")
    }) {
        k.controller = true;
    }
    let done = events.iter().any(|e| {
        e.eq_ignore_ascii_case("GetUpEnd")
            || e.eq_ignore_ascii_case("GetUpExit")
            || e.eq_ignore_ascii_case("SwimStartFromRagdoll")
    });
    let pose = if k.state == KnockState::GetUp {
        k.recovery_time += dt;
        let targets = skeleton
            .ragdoll
            .as_ref()
            .map(|d| {
                d.bodies
                    .iter()
                    .map(|b| k.actor_transform * frame.pose[b.bone] * b.offset)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        physics.keyframe_ragdoll(&k.ragdoll, &targets, dt);
        frame.pose
    } else {
        if let Some(desc) = &skeleton.ragdoll {
            let powered = frame.ragdoll.powered.first();
            let target = match (&k.hold_pose, paralysis) {
                (Some(held), true) => held,
                _ => &frame.pose,
            };
            let rotations: Vec<_> = desc
                .bodies
                .iter()
                .map(|b| {
                    (target[b.bone] * b.offset)
                        .to_scale_rotation_translation()
                        .1
                })
                .collect();
            let (mode, weights) = if paralysis {
                (
                    RagdollMode::Powered {
                        tau: 1.0,
                        damping: 2.0,
                        max_force: 1000.0,
                    },
                    vec![1.0; desc.bodies.len()],
                )
            } else if let Some(p) = powered {
                (
                    RagdollMode::Powered {
                        tau: p.tau,
                        damping: p.damping,
                        max_force: p.max_force,
                    },
                    graph.ragdoll_body_weights(skeleton, vfs, anims, &p.weights),
                )
            } else {
                (RagdollMode::Limp, Vec::new())
            };
            physics.set_ragdoll_targets(&mut k.ragdoll, &rotations, &weights, mode);
        }
        k.mapping.pose_animated(skeleton, &bodies, &frame.pose)
    };
    KnockFrame {
        pose,
        pos,
        heading,
        done,
        events,
        fall,
    }
}

impl Engine {
    /// After physics and before AI: physical poses replace locomotion, while
    /// detection, damage, active effects and scripts keep ticking normally.
    pub(crate) fn update_knocks(&mut self, dt: f32) {
        let actors: Vec<_> = self
            .cells
            .values()
            .flat_map(|r| &r.actors)
            .filter(|a| !a.dead)
            .map(|a| a.ref_id)
            .chain([PLAYER_REF])
            .collect();
        for &actor in &actors {
            let knock = if actor == PLAYER_REF {
                self.player_knock.as_ref()
            } else {
                self.actor_ref(actor).and_then(|a| a.knock.as_ref())
            };
            if knock.is_none_or(|k| k.state == KnockState::GetUp || k.getup_requested)
                && self.actor_value(actor, esp::actor_value::PARALYSIS) > 0.0
            {
                self.knock_down(actor, Vec3::ZERO, KnockCause::Paralysis);
            }
        }
        let mut finished = Vec::new();
        let mut notifications = Vec::new();
        let mut falls = Vec::new();
        for actor in actors {
            if actor == PLAYER_REF || !self.is_knocked(actor) {
                continue;
            }
            let paralysis = self.actor_value(actor, esp::actor_value::PARALYSIS) > 0.0;
            if paralysis {
                if let Some(k) = self.actor_mut(actor).and_then(|a| a.knock.as_mut()) {
                    k.cause = KnockCause::Paralysis;
                }
            }
            let water = self
                .actor_ref(actor)
                .and_then(|a| self.water_level(a.exec.pos));
            let Some(key) = self.actor_cells.get(&actor).copied() else {
                continue;
            };
            let Some(rt) = self.cells.get_mut(&key) else {
                continue;
            };
            let Some(index) = rt.actors.iter().position(|a| a.ref_id == actor) else {
                continue;
            };
            let a = &mut rt.actors[index];
            let (Some(k), Some(g)) = (&mut a.knock, &mut a.graph) else {
                continue;
            };
            let mut f = advance(
                k,
                g,
                &a.skeleton,
                &mut self.physics,
                &self.vfs,
                &mut self.anims,
                dt,
                a.exec.pos,
                a.exec.heading,
                paralysis,
                water,
            );
            if k.controller {
                let resolved = self.physics.recover_actor_position(a.capsule, f.pos);
                k.actor_transform.w_axis += (resolved - f.pos).extend(0.0);
                f.pos = resolved;
            }
            a.exec.pos = f.pos;
            a.exec.heading = f.heading;
            if k.controller {
                if let Some(c) = a.capsule {
                    self.physics.move_actor_capsule(c, f.pos);
                    self.physics.set_enabled(&[c], true);
                }
            }
            if let Some(inst) = self
                .scene
                .cells
                .get_mut(&key)
                .and_then(|r| r.actors.get_mut(index))
            {
                inst.pose = f.pose;
                inst.transform = k.actor_transform;
            }
            if f.events
                .iter()
                .any(|e| e.eq_ignore_ascii_case("SwimStartFromRagdoll"))
            {
                a.swimming = true;
            }
            if let Some(drop) = f.fall {
                falls.push((actor, drop));
            }
            notifications.push((actor, f.events));
            if f.done {
                finished.push(actor);
            }
        }
        for actor in finished {
            self.finish_knock(actor);
        }
        for (actor, events) in notifications {
            self.raise_anim_events(actor, &events);
        }
        for (actor, height) in falls {
            self.knocked_landed(actor, height);
        }
        self.update_player_knock(dt);
    }

    pub(crate) fn finish_knock(&mut self, actor: FormId) {
        self.release_actor_constraints(actor);
        let Some(a) = self.actor_mut(actor) else {
            return;
        };
        let Some(k) = a.knock.take() else { return };
        let (capsule, pos) = (a.capsule, a.exec.pos);
        if let Some(g) = a.graph.as_mut() {
            g.clear_ragdoll_pose();
            if a.exec.bleeding.is_some() {
                g.send_event("bleedOutStart");
            }
        }
        self.physics.remove_ragdoll(&k.ragdoll);
        if let Some(c) = capsule {
            self.physics.move_actor_capsule(c, pos);
            self.physics.set_enabled(&[c], true);
        }
    }

    fn knocked_landed(&mut self, actor: FormId, height: f32) {
        if self.is_ethereal(actor) || self.actor_value(actor, esp::actor_value::PARALYSIS) > 0.0 {
            return;
        }
        let mut damage = crate::falling::fall_damage(&self.lo, height, actor != PLAYER_REF);
        if damage <= 0.0 {
            return;
        }
        damage = self.perk_entry_point(crate::perks::ep::MOD_FALLING_DAMAGE, actor, &[], damage);
        if damage > 0.0 {
            self.damage_unscaled(
                actor,
                damage * self.difficulty_multiplier(actor, true),
                None,
                0.0,
            );
        }
    }
}

impl Engine {
    fn knock_player(&mut self, impulse: Vec3, cause: KnockCause) -> bool {
        if let Some(mut k) = self.player_knock.take() {
            if k.state == KnockState::GetUp || k.getup_requested {
                self.release_actor_constraints(PLAYER_REF);
                self.physics.ragdoll_into_corpse(&mut k.ragdoll);
                k.state = KnockState::Down;
                k.down_for = 0.0;
                k.getup_requested = false;
                k.controller = false;
                k.recovery_time = 0.0;
                k.recovery_pose = None;
                if let Some(g) = self.player_body.as_mut().and_then(|b| b.graph_mut()) {
                    g.clear_ragdoll_pose();
                    g.send_event("Ragdoll");
                }
            }
            self.physics
                .apply_ragdoll_impulse(&k.ragdoll, impulse, None);
            k.rest_for = 0.0;
            // A push mid-fall keeps the height already fallen from.
            if let Some(m) = self.physics.ragdoll_bodies(&k.ragdoll).first() {
                k.peak_z = k.peak_z.max(m.w_axis.z);
            }
            k.fall_applied = false;
            if cause == KnockCause::Script {
                k.script_hold = true;
            }
            if cause == KnockCause::Paralysis {
                k.cause = cause;
            }
            self.player_knock = Some(k);
            return true;
        }
        if self.passengers.contains_key(&PLAYER_REF)
            || self.rides.contains_key(&PLAYER_REF)
            || self.first_person_furniture.is_some()
        {
            return false;
        }
        if self.player_body.is_none() {
            self.build_player_body();
        }
        let Some(body) = self.player_body.as_ref() else {
            return false;
        };
        if !body.third_graph().is_some_and(|g| g.can_get_up()) {
            return false;
        }
        let Some(desc) = body.skeleton.ragdoll.clone() else {
            return false;
        };
        let Some(inst) = self.scene.player.as_ref() else {
            return false;
        };
        let (pose, transform, skeleton) =
            (inst.pose.clone(), inst.transform, body.skeleton.clone());
        let r = self.physics.spawn_ragdoll(
            &desc,
            |i| transform * pose[desc.bodies[i].bone] * desc.bodies[i].offset,
            Vec3::Z * self.player.vertical_velocity + impulse,
            PLAYER_REF,
            RagdollMode::Limp,
        );
        let previous = self.third_person;
        self.set_third_person(true);
        self.interrupt_cast(PLAYER_REF);
        self.shouts.interrupt();
        self.player_swing = None;
        self.player_attack_held = None;
        self.player_blocking = false;
        self.player.reset_fall_tracking();
        self.player.moving = false;
        self.player.running = false;
        self.player.sprinting = false;
        self.player_graph_event("Ragdoll");
        self.player_knock = Some(Knock {
            state: KnockState::Queued,
            ragdoll: r,
            mapping: RagdollPose::new(&desc, &skeleton, &pose, transform),
            cause,
            down_for: 0.0,
            rest_for: 0.0,
            peak_z: transform.w_axis.z,
            fall_applied: false,
            script_hold: cause == KnockCause::Script,
            getup_requested: false,
            controller: false,
            actor_transform: transform,
            recovery_pose: None,
            recovery_time: 0.0,
            previous_view: Some(previous),
            hold_pose: None,
        });
        self.stop_knocked_voice(PLAYER_REF);
        true
    }

    fn request_player_get_up(&mut self, get_up_type: f32) -> bool {
        let Some(k) = self.player_knock.as_mut() else {
            return false;
        };
        if k.getup_requested {
            return true;
        }
        k.getup_requested = true;
        if let Some(g) = self.player_body.as_mut().and_then(|b| b.graph_mut()) {
            g.set_variable("iGetUpType", get_up_type);
            g.send_event("GetUpBegin");
        }
        true
    }

    pub(crate) fn jump_during_get_up(&mut self) -> bool {
        if !self
            .player_knock
            .as_ref()
            .is_some_and(|k| k.state == KnockState::GetUp && k.controller)
        {
            return false;
        }
        self.player_graph_event("GetUpExit");
        self.finish_player_knock();
        self.player.grounded = true;
        true
    }

    pub(crate) fn knock_camera_position(&self) -> Option<Vec3> {
        let k = self.player_knock.as_ref()?;
        self.physics
            .ragdoll_bodies(&k.ragdoll)
            .first()
            .map(|m| m.w_axis.truncate())
    }

    pub(crate) fn finish_player_knock(&mut self) {
        let Some(k) = self.player_knock.take() else {
            return;
        };
        self.release_actor_constraints(PLAYER_REF);
        self.player.reset_fall_tracking();
        self.physics.remove_ragdoll(&k.ragdoll);
        if let Some(g) = self.player_body.as_mut().and_then(|b| b.graph_mut()) {
            g.clear_ragdoll_pose();
        }
        if let Some(previous) = k.previous_view {
            self.set_third_person(previous);
        }
    }

    fn update_player_knock(&mut self, dt: f32) {
        let Some(mut k) = self.player_knock.take() else {
            return;
        };
        if self.player_dead() {
            self.physics.ragdoll_into_corpse(&mut k.ragdoll);
            let bodies = self.physics.ragdoll_bodies(&k.ragdoll);
            if let (Some(b), Some(inst)) = (&self.player_body, &mut self.scene.player) {
                inst.pose = k.mapping.pose(&b.skeleton, &bodies);
            }
            self.player_knock = Some(k);
            return;
        }
        let paralysis = self.actor_value(PLAYER_REF, esp::actor_value::PARALYSIS) > 0.0;
        if paralysis {
            k.cause = KnockCause::Paralysis;
        }
        let water = self.water_level(self.player.position);
        let feet = self.player.position
            - Vec3::Z * (self.physics.player_half_height + self.physics.player_radius);
        let Some(body) = self.player_body.as_mut() else {
            self.player_knock = Some(k);
            return;
        };
        let skeleton = body.skeleton.clone();
        let heading = body.heading;
        let Some(graph) = body.graph_mut() else {
            self.player_knock = Some(k);
            return;
        };
        let mut f = advance(
            &mut k,
            graph,
            &skeleton,
            &mut self.physics,
            &self.vfs,
            &mut self.anims,
            dt,
            feet,
            heading,
            paralysis,
            water,
        );
        if k.controller {
            let resolved = self.physics.recover_actor_position(None, f.pos);
            k.actor_transform.w_axis += (resolved - f.pos).extend(0.0);
            f.pos = resolved;
        }
        body.heading = f.heading;
        self.player.position =
            f.pos + Vec3::Z * (self.physics.player_half_height + self.physics.player_radius);
        self.camera.position = self.player.eye();
        if let Some(inst) = self.scene.player.as_mut() {
            inst.pose = f.pose;
            inst.transform = k.actor_transform;
        }
        let done = f.done;
        if done {
            self.release_actor_constraints(PLAYER_REF);
            self.physics.remove_ragdoll(&k.ragdoll);
            if let Some(g) = self.player_body.as_mut().and_then(|b| b.graph_mut()) {
                g.clear_ragdoll_pose();
            }
            self.player.grounded = true;
            self.player.reset_fall_tracking();
            if let Some(previous) = k.previous_view {
                self.set_third_person(previous);
            }
        } else {
            self.player_knock = Some(k);
        }
        self.raise_anim_events(PLAYER_REF, &f.events);
        if let Some(drop) = f.fall {
            self.knocked_landed(PLAYER_REF, drop);
        }
    }
}

impl Engine {
    pub(crate) fn push_actor_away(&mut self, source: FormId, target: FormId, force: f32) -> bool {
        if !force.is_finite() {
            return false;
        }
        let (Some(from), Some(to)) = (self.ref_position(source), self.ref_position(target)) else {
            return false;
        };
        let impulse = push_velocity(from, to, force);
        if self.is_dead(target) {
            let r = self
                .actor_ref(target)
                .and_then(|a| a.ragdoll.as_ref())
                .map(|(r, _)| r.clone());
            if let Some(r) = r {
                self.physics.apply_ragdoll_impulse(&r, impulse, None);
                return true;
            }
            return false;
        }
        if self.knock_down(target, impulse, KnockCause::Push) {
            true
        } else {
            self.stagger_actor(target, 1.5);
            false
        }
    }

    pub(crate) fn stagger_actor(&mut self, actor: FormId, magnitude: f32) {
        if !magnitude.is_finite()
            || self.is_dead(actor)
            || self.is_ethereal(actor)
            || self.is_knocked(actor)
        {
            return;
        }
        // The stagger graph blends its clips over staggerMagnitude 0..1.
        self.set_graph_variable(actor, "staggerMagnitude", magnitude.clamp(0.0, 1.0));
        self.play_animation_event(actor, "staggerStart");
    }

    pub(crate) fn knock_area(&mut self, source: FormId, magnitude: f32, radius: f32) {
        if !magnitude.is_finite() || !radius.is_finite() || radius < 0.0 {
            return;
        }
        let Some(from) = self.ref_position(source) else {
            return;
        };
        let mut actors: Vec<_> = self
            .cells
            .values()
            .flat_map(|r| &r.actors)
            .filter(|a| !a.dead)
            .map(|a| a.ref_id)
            .collect();
        actors.push(PLAYER_REF);
        actors.sort_by(|a, b| {
            self.ref_position(*a)
                .map_or(f32::INFINITY, |p| p.distance_squared(from))
                .total_cmp(
                    &self
                        .ref_position(*b)
                        .map_or(f32::INFINITY, |p| p.distance_squared(from)),
                )
        });
        for actor in actors {
            if actor == source {
                continue;
            }
            let Some(to) = self
                .ref_position(actor)
                .filter(|p| p.distance(from) <= radius)
            else {
                continue;
            };
            if magnitude >= 1.0
                && self.knock_down(
                    actor,
                    push_velocity(from, to, magnitude * 10.0),
                    KnockCause::Area,
                )
            {
                continue;
            }
            self.stagger_actor(actor, magnitude);
        }
    }
}

/// Provisional force scale and lift; isolated for correction by the SE probe.
/// A negative force pulls toward the source, still lifting the body.
fn push_velocity(from: Vec3, to: Vec3, force: f32) -> Vec3 {
    let direction = (to - from).with_z(0.0).normalize_or(Vec3::Y);
    let force = force.clamp(-128.0, 128.0);
    (direction * force + Vec3::Z * 0.35 * force.abs()) * 35.0
}

#[cfg(test)]
#[path = "knock_tests.rs"]
mod tests;
