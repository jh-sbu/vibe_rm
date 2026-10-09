//! Physics: static world collision and the player character controller (via rapier3d).

pub mod shapes;

use std::collections::HashMap;
use std::sync::Arc;

use glam::{Mat4, Vec3};
use rapier3d::control::{CharacterAutostep, CharacterLength, KinematicCharacterController};
use rapier3d::prelude::*;

use shapes::CollisionModel;

/// One metre in game units.
pub const METRE: f32 = 70.0;
pub const GRAVITY: f32 = 9.81 * METRE;

/// Whether a Havok collision layer (Skyrim `SKYL_*`) blocks the player's character controller.
fn layer_blocks_player(layer: u8) -> bool {
    matches!(
        layer,
        0 // unidentified
        | 1 // static
        | 2 // anim static
        | 3 // transparent
        | 4 // clutter
        | 9 // trees
        | 10 // props
        | 13 // terrain
        | 14 // trap
        | 17 // ground
        | 20 // debris large
        | 27 // invisible wall
        | 31 // stair helper
    )
}

/// Collision groups: actors' capsules, and ragdoll bodies (which don't collide with
/// either, only with the world).
const ACTOR_GROUP: Group = Group::GROUP_3;
const RAGDOLL_GROUP: Group = Group::GROUP_2;

/// The player's mass pushing loose objects about (kg; no source).
const PLAYER_MASS: f32 = 80.0;

/// How a loose object's body moves (Papyrus `SetMotionType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    /// Simulated: falls, rolls, is pushed.
    Dynamic,
    /// Held where it is put, pushing others aside (scripts and animations move it).
    Keyframed,
    /// Immovable.
    Fixed,
}

/// A ragdoll in the simulation: one body per ragdoll body of the description, and
/// the actor's scale.
pub struct Ragdoll {
    pub bodies: Vec<RigidBodyHandle>,
    pub scale: f32,
}

/// A joint from a NIF constraint (x: the twist / hinge axis), its pivots
/// scaled as the bodies are: ball joints for ragdoll constraints, hinges for
/// hinges; the bodies it joins don't collide.
fn joint(j: &crate::world::ragdoll::RagdollJoint, scale: f32) -> GenericJoint {
    use crate::world::ragdoll::JointKind;
    let frame = |f: (Vec3, glam::Quat)| Pose::from_parts(f.0 * scale, f.1);
    if let JointKind::Rope(length) = j.kind {
        return RopeJointBuilder::new(length * scale)
            .local_anchor1(j.frame_a.0 * scale)
            .local_anchor2(j.frame_b.0 * scale)
            .contacts_enabled(false)
            .build()
            .data;
    }
    let mask = if j.limits[1].is_some() || j.kind == JointKind::Ball {
        JointAxesMask::LOCKED_SPHERICAL_AXES
    } else {
        JointAxesMask::LOCKED_REVOLUTE_AXES
    };
    let mut joint = GenericJointBuilder::new(mask)
        .local_frame1(frame(j.frame_a))
        .local_frame2(frame(j.frame_b))
        .contacts_enabled(false);
    for (axis, limit) in [JointAxis::AngX, JointAxis::AngY, JointAxis::AngZ]
        .into_iter()
        .zip(j.limits)
    {
        if let Some((lo, hi)) = limit {
            joint = joint.limits(axis, [lo.min(hi), hi.max(lo)]);
        }
    }
    joint.build()
}

/// What a collider is made of.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Surface {
    /// A Havok material (`SKY_HAV_MAT_*`).
    Havok(u32),
    /// The landscape: its texture at that point decides.
    Terrain,
}

/// A collider found by [`Physics::probe_ray`].
pub struct ProbeHit {
    pub handle: ColliderHandle,
    pub toi: f32,
    pub in_broad_phase: bool,
    pub enabled: bool,
    pub owner: Option<esp::FormId>,
    /// The collider's world bounds (min, max).
    pub bounds: (Vec3, Vec3),
}

pub struct Physics {
    pub world: PhysicsWorld,
    controller: KinematicCharacterController,
    player_shape: SharedShape,
    /// Collider -> reference FormID that owns it.
    pub owners: HashMap<ColliderHandle, esp::FormId>,
    /// Actors' capsules (not ground to stand on).
    capsules: std::collections::HashSet<ColliderHandle>,
    /// Havok materials of the world's colliders (per triangle for meshes), and
    /// the landscape's colliders (whose material is their texture's).
    materials: HashMap<ColliderHandle, Arc<[u32]>>,
    terrain: std::collections::HashSet<ColliderHandle>,
    pub player_radius: f32,
    pub player_half_height: f32,
    /// Joints that break past an impulse (game units: kg units / s), whose
    /// object and joint index they are; and those that broke in the last step.
    breakable: Vec<(ImpulseJointHandle, f32, esp::FormId, usize)>,
    pub broken: Vec<(esp::FormId, usize)>,
    /// Loose objects' bodies their NIF fixes in place (a sign's bracket, a
    /// beehive's mount): `set_motion` leaves them be.
    anchored: std::collections::HashSet<RigidBodyHandle>,
    /// The body the player holds (`crate::grab`): the character controller
    /// passes through it rather than standing on it or pushing it.
    pub held: Option<RigidBodyHandle>,
}

impl Physics {
    pub fn new() -> Self {
        let mut world = PhysicsWorld::new();
        world.gravity = Vec3::new(0.0, 0.0, -GRAVITY);
        world.integration_parameters.length_unit = METRE;
        let controller = KinematicCharacterController {
            up: Vec3::Z,
            offset: CharacterLength::Absolute(1.0),
            slide: true,
            autostep: Some(CharacterAutostep {
                max_height: CharacterLength::Absolute(36.0),
                min_width: CharacterLength::Absolute(8.0),
                include_dynamic_bodies: false,
            }),
            max_slope_climb_angle: 50f32.to_radians(),
            min_slope_slide_angle: 55f32.to_radians(),
            snap_to_ground: Some(CharacterLength::Absolute(16.0)),
            normal_nudge_factor: 1.0e-4,
        };
        let player_radius = 22.0;
        let player_half_height = 42.0;
        Physics {
            world,
            controller,
            player_shape: SharedShape::capsule_z(player_half_height, player_radius),
            owners: HashMap::new(),
            capsules: Default::default(),
            materials: HashMap::new(),
            terrain: Default::default(),
            player_radius,
            player_half_height,
            breakable: Vec::new(),
            broken: Vec::new(),
            anchored: Default::default(),
            held: None,
        }
    }

    pub fn clear(&mut self) {
        *self = Physics::new();
    }

    /// Add the static collision of a placed object.
    /// Add a model's static collision. Returns the colliders with the animated node
    /// (e.g. a door leaf) each one moves with, if any.
    pub fn add_static_tagged(
        &mut self,
        model: &CollisionModel,
        transform: Mat4,
        owner: esp::FormId,
    ) -> Vec<(ColliderHandle, Option<String>)> {
        let mut handles = Vec::new();
        for part in &model.parts {
            if !layer_blocks_player(part.layer) {
                continue;
            }
            let (pose, scale) = shapes::decompose(transform * part.transform);
            let Some(shape) = part.shape.build(scale) else {
                continue;
            };
            let c = ColliderBuilder::new(shape).position(pose).build();
            let h = self.world.insert_collider(c, None);
            self.owners.insert(h, owner);
            self.materials.insert(h, part.materials.clone());
            handles.push((h, part.node.clone()));
        }
        handles
    }

    /// Add a loose object (`CollisionModel::is_loose`) at `transform` (scale
    /// included): a body for each of the model's rigid bodies, joined as its
    /// constraints join them, the simulated ones held in place until
    /// `release`d. Masses are the NIF's (kg). Returns the bodies, by the model's
    /// body index (none for one without shapes).
    pub fn add_loose(
        &mut self,
        model: &CollisionModel,
        transform: Mat4,
        owner: esp::FormId,
        broken: &[usize],
    ) -> Vec<Option<RigidBodyHandle>> {
        let scale = shapes::decompose(transform).1;
        let mut out = Vec::with_capacity(model.bodies.len());
        for (i, desc) in model.bodies.iter().enumerate() {
            let (pose, _) = shapes::decompose(transform * desc.frame);
            let inv = pose.to_mat4().inverse();
            let parts: Vec<_> = model
                .parts
                .iter()
                .filter(|p| p.body == i && (desc.dynamic || layer_blocks_player(p.layer)))
                .filter_map(|p| {
                    let (local, s) = shapes::decompose(inv * transform * p.transform);
                    let shape = if desc.dynamic {
                        p.shape.build_solid(s)
                    } else {
                        p.shape.build(s)
                    }?;
                    Some((p, local, shape))
                })
                .collect();
            if parts.is_empty() {
                out.push(None);
                continue;
            }
            let body = if desc.dynamic {
                RigidBodyBuilder::dynamic()
                    .linear_damping(0.1)
                    .angular_damping(0.5)
                    .ccd_enabled(true)
                    .locked_axes(LockedAxes::all())
                    .sleeping(true)
            } else {
                RigidBodyBuilder::fixed()
            };
            let h = self.world.insert_body(body.pose(pose));
            if !desc.dynamic {
                self.anchored.insert(h);
            }
            let mass = desc.mass.max(0.1) / parts.len() as f32;
            for (p, local, shape) in parts {
                let c = ColliderBuilder::new(shape)
                    .position(local)
                    .mass(mass)
                    .friction(p.friction.clamp(0.0, 1.5))
                    .restitution(p.restitution.clamp(0.0, 0.9))
                    .build();
                let ch = self.world.insert_collider(c, Some(h));
                self.owners.insert(ch, owner);
                self.materials.insert(ch, p.materials.clone());
            }
            out.push(Some(h));
        }
        for (i, j) in model.joints.iter().enumerate() {
            if broken.contains(&i) {
                continue;
            }
            if let (Some(Some(a)), Some(Some(b))) = (out.get(j.a), out.get(j.b)) {
                let h = self.world.insert_impulse_joint(*a, *b, joint(j, scale));
                if let Some(t) = j.breaks {
                    self.breakable.push((h, t * METRE, owner, i));
                }
            }
        }
        out
    }

    /// Remove a loose object's body and its colliders.
    pub fn remove_body(&mut self, h: RigidBodyHandle) {
        let colliders: Vec<ColliderHandle> = self
            .world
            .bodies
            .get(h)
            .map(|b| b.colliders().to_vec())
            .unwrap_or_default();
        self.world.remove_body_with_colliders(h, true);
        self.anchored.remove(&h);
        for c in colliders {
            self.owners.remove(&c);
            self.materials.remove(&c);
        }
    }

    /// Where a body is (no scale), and whether it is asleep.
    pub fn body_pose(&self, h: RigidBodyHandle) -> Option<(Mat4, bool)> {
        let b = self.world.bodies.get(h)?;
        Some((b.position().to_mat4(), b.is_sleeping()))
    }

    /// Put a body somewhere (no scale), at rest.
    pub fn set_body_pose(&mut self, h: RigidBodyHandle, m: Mat4) {
        if let Some(b) = self.world.bodies.get_mut(h) {
            let (pose, _) = shapes::decompose(m);
            b.set_position(pose, true);
            b.set_linvel(Vec3::ZERO, true);
            b.set_angvel(Vec3::ZERO, true);
        }
    }

    /// Let a loose object's body move, asleep (`asleep`) until disturbed or
    /// moving already.
    pub fn release(&mut self, h: RigidBodyHandle, asleep: bool) {
        if let Some(b) = self.world.bodies.get_mut(h) {
            b.set_locked_axes(LockedAxes::empty(), !asleep);
            if asleep {
                b.sleep();
            }
        }
    }

    /// The bodies of what `owner` owns (its loose object).
    fn owner_bodies(&self, owner: esp::FormId) -> Vec<RigidBodyHandle> {
        let mut out: Vec<RigidBodyHandle> = self
            .owners
            .iter()
            .filter(|(_, o)| **o == owner)
            .filter_map(|(h, _)| self.world.colliders.get(*h)?.parent())
            .collect();
        out.sort_unstable_by_key(|h| h.into_raw_parts());
        out.dedup();
        out
    }

    /// Push what `owner` owns: an impulse (kg game units / s) through its centre
    /// of mass. False if it has no body.
    pub fn apply_impulse(&mut self, owner: esp::FormId, impulse: Vec3) -> bool {
        let bodies = self.owner_bodies(owner);
        let mass_of = |w: &PhysicsWorld, h: RigidBodyHandle| {
            w.bodies
                .get(h)
                .filter(|b| b.is_dynamic())
                .map_or(0.0, |b| b.mass())
        };
        let total: f32 = bodies.iter().map(|&h| mass_of(&self.world, h)).sum();
        for &h in &bodies {
            let share = mass_of(&self.world, h) / total.max(1e-3);
            if let Some(b) = self.world.bodies.get_mut(h)
                && b.is_dynamic()
            {
                // Shared by the object's bodies by mass (all of it for one).
                let impulse = impulse * share;
                b.apply_impulse(impulse, true);
                log::debug!(
                    "{owner}: impulse {impulse:?} on {:.2} kg: {:?}",
                    b.mass(),
                    b.linvel()
                );
            }
        }
        !bodies.is_empty()
    }

    /// Change how what `owner` owns moves (but the bodies its NIF fixes). False
    /// if it has no body.
    pub fn set_motion(&mut self, owner: esp::FormId, motion: Motion) -> bool {
        let bodies = self.owner_bodies(owner);
        for &h in bodies.iter().filter(|h| !self.anchored.contains(h)) {
            if let Some(b) = self.world.bodies.get_mut(h) {
                let t = match motion {
                    Motion::Dynamic => RigidBodyType::Dynamic,
                    Motion::Keyframed => RigidBodyType::KinematicPositionBased,
                    Motion::Fixed => RigidBodyType::Fixed,
                };
                b.set_body_type(t, true);
            }
        }
        !bodies.is_empty()
    }

    /// The middle of what `owner` owns that moves (its bodies' mean position).
    pub fn owner_center(&self, owner: esp::FormId) -> Option<Vec3> {
        let at: Vec<Vec3> = self
            .owner_bodies(owner)
            .iter()
            .filter_map(|&h| self.world.bodies.get(h))
            .filter(|b| !b.is_fixed())
            .map(|b| b.center_of_mass())
            .collect();
        (!at.is_empty()).then(|| at.iter().sum::<Vec3>() / at.len() as f32)
    }

    /// Whether what `owner` owns is a simulated (dynamic) body.
    pub fn is_dynamic_owner(&self, owner: esp::FormId) -> bool {
        self.owner_bodies(owner)
            .iter()
            .any(|&h| self.world.bodies.get(h).is_some_and(|b| b.is_dynamic()))
    }

    /// A body's mass (kg) and speed (game units / s).
    pub fn body_mass_speed(&self, h: RigidBodyHandle) -> (f32, f32) {
        self.world
            .bodies
            .get(h)
            .map_or((0.0, 0.0), |b| (b.mass(), b.linvel().length()))
    }

    /// How what `owner` owns moves, if it has a body.
    pub fn motion(&self, owner: esp::FormId) -> Option<Motion> {
        let bodies = self.owner_bodies(owner);
        let h = bodies
            .iter()
            .find(|h| !self.anchored.contains(h))
            .or(bodies.first())?;
        let b = self.world.bodies.get(*h)?;
        Some(match b.body_type() {
            RigidBodyType::Dynamic => Motion::Dynamic,
            RigidBodyType::Fixed => Motion::Fixed,
            _ => Motion::Keyframed,
        })
    }

    /// Turn colliders on or off (e.g. an open door's leaves).
    pub fn set_enabled(&mut self, handles: &[ColliderHandle], enabled: bool) {
        for &h in handles {
            if let Some(c) = self.world.colliders.get_mut(h) {
                c.set_enabled(enabled);
            }
        }
    }

    /// A capsule standing at `feet`, used for actors.
    pub fn add_actor_capsule(
        &mut self,
        feet: Vec3,
        scale: f32,
        owner: esp::FormId,
    ) -> ColliderHandle {
        let r = 20.0 * scale;
        let half = 40.0 * scale;
        let c = ColliderBuilder::new(SharedShape::capsule_z(half, r))
            .position(Pose::from_translation(feet + Vec3::Z * (half + r)))
            .collision_groups(InteractionGroups::new(
                ACTOR_GROUP,
                Group::ALL,
                InteractionTestMode::And,
            ))
            .build();
        let h = self.world.insert_collider(c, None);
        self.owners.insert(h, owner);
        self.capsules.insert(h);
        h
    }

    /// Move an actor capsule so that its feet are at `feet`.
    pub fn move_actor_capsule(&mut self, h: ColliderHandle, feet: Vec3) {
        if let Some(c) = self.world.colliders.get_mut(h) {
            let lift = c
                .shape()
                .as_capsule()
                .map(|cap| cap.half_height() + cap.radius)
                .unwrap_or(0.0);
            c.set_translation(feet + Vec3::Z * lift);
            let (pose, shape) = (*c.position(), c.shared_shape().clone());
            self.wake_touching(pose, &shape);
        }
    }

    /// Wake the sleeping loose objects a shape overlaps (an actor walking into
    /// them): the physics engine doesn't for colliders moved by hand.
    fn wake_touching(&mut self, pose: Pose, shape: &SharedShape) {
        let qp = self.world.query_pipeline();
        let hs: Vec<RigidBodyHandle> = qp
            .intersect_shape(pose, &*shape.0)
            .filter_map(|(_, c)| c.parent())
            .collect();
        for h in hs {
            if self
                .world
                .bodies
                .get(h)
                .is_some_and(|b| b.is_dynamic() && b.is_sleeping())
            {
                self.world.wake_up(h, true);
            }
        }
    }

    /// Move all colliders owned by a reference by `delta` (a rigid transform).
    pub fn transform_owner(&mut self, owner: esp::FormId, delta: Mat4) {
        let hs: Vec<ColliderHandle> = self
            .owners
            .iter()
            .filter(|(_, o)| **o == owner)
            .map(|(h, _)| *h)
            .collect();
        for h in self.owner_bodies(owner) {
            if let Some(b) = self.world.bodies.get(h) {
                let m = delta * b.position().to_mat4();
                self.set_body_pose(h, m);
            }
        }
        for h in hs {
            if let Some(c) = self.world.colliders.get_mut(h)
                && c.parent().is_none()
            {
                let (pose, _) = shapes::decompose(delta * c.position().to_mat4());
                c.set_position(pose);
            }
        }
    }

    /// Enable or disable all colliders owned by a reference.
    pub fn set_owner_enabled(&mut self, owner: esp::FormId, enabled: bool) {
        let hs: Vec<ColliderHandle> = self
            .owners
            .iter()
            .filter(|(_, o)| **o == owner)
            .map(|(h, _)| *h)
            .collect();
        for h in hs {
            if let Some(c) = self.world.colliders.get_mut(h) {
                c.set_enabled(enabled);
            }
        }
        for h in self.owner_bodies(owner) {
            if let Some(b) = self.world.bodies.get_mut(h) {
                b.set_enabled(enabled);
            }
        }
    }

    pub fn remove_colliders(&mut self, handles: &[ColliderHandle]) {
        for &h in handles {
            self.world.remove_collider(h);
            self.owners.remove(&h);
            self.capsules.remove(&h);
            self.materials.remove(&h);
            self.terrain.remove(&h);
        }
    }

    /// Add a landscape cell as a triangle mesh.
    pub fn add_terrain(&mut self, land: &crate::world::terrain::Land) -> Option<ColliderHandle> {
        use crate::world::terrain::{CELL_SIZE, VERTS};
        let o = land.origin();
        let step = CELL_SIZE / 32.0;
        let mut v = Vec::with_capacity(VERTS * VERTS);
        for r in 0..VERTS {
            for c in 0..VERTS {
                v.push(Vec3::new(
                    o.x + c as f32 * step,
                    o.y + r as f32 * step,
                    land.heights[r * VERTS + c],
                ));
            }
        }
        let mut t = Vec::with_capacity(32 * 32 * 2);
        for r in 0..32u32 {
            for c in 0..32u32 {
                let a = r * VERTS as u32 + c;
                let b = a + 1;
                let cc = a + VERTS as u32 + 1;
                let d = a + VERTS as u32;
                t.push([a, b, cc]);
                t.push([a, cc, d]);
            }
        }
        let shape = SharedShape::trimesh(v, t).ok()?;
        let h = self
            .world
            .insert_collider(ColliderBuilder::new(shape).build(), None);
        self.terrain.insert(h);
        Some(h)
    }

    /// Update the broad phase after colliders were added.
    /// Drop a ragdoll in: each body placed where `body_world` (its bone's world
    /// transform times its offset, scale included) puts it.
    pub fn spawn_ragdoll(
        &mut self,
        desc: &crate::world::ragdoll::RagdollDesc,
        body_world: impl Fn(usize) -> Mat4,
        velocity: Vec3,
        owner: esp::FormId,
    ) -> Ragdoll {
        let groups = InteractionGroups::new(
            RAGDOLL_GROUP,
            Group::ALL ^ RAGDOLL_GROUP ^ ACTOR_GROUP,
            InteractionTestMode::And,
        );
        let mut scale = 1.0;
        let mut bodies = Vec::with_capacity(desc.bodies.len());
        for (i, b) in desc.bodies.iter().enumerate() {
            let (pose, s) = shapes::decompose(body_world(i));
            scale = s;
            let body = RigidBodyBuilder::dynamic()
                .pose(pose)
                .linvel(velocity)
                .linear_damping(0.2)
                .angular_damping(1.0)
                .ccd_enabled(true);
            let collider =
                ColliderBuilder::new(SharedShape::capsule(b.p1 * s, b.p2 * s, b.radius * s))
                    .mass(b.mass)
                    .friction(0.8)
                    .collision_groups(groups);
            let (h, c) = self.world.insert(body, collider);
            self.owners.insert(c, owner);
            bodies.push(h);
        }
        for j in &desc.joints {
            self.world
                .insert_impulse_joint(bodies[j.a], bodies[j.b], joint(j, scale));
        }
        Ragdoll { bodies, scale }
    }

    /// Where a ragdoll's bodies are now (world, with the actor's scale).
    pub fn ragdoll_bodies(&self, r: &Ragdoll) -> Vec<Mat4> {
        r.bodies
            .iter()
            .map(|&h| {
                self.world.bodies.get(h).map_or(Mat4::IDENTITY, |b| {
                    b.position().to_mat4() * Mat4::from_scale(Vec3::splat(r.scale))
                })
            })
            .collect()
    }

    pub fn remove_ragdoll(&mut self, r: &Ragdoll) {
        for &h in &r.bodies {
            self.world.remove_body_with_colliders(h, true);
        }
    }

    pub fn step(&mut self, dt: f32) {
        self.world.integration_parameters.dt = dt.clamp(1.0 / 240.0, 1.0 / 30.0);
        self.world.step();
        self.break_joints();
    }

    /// Breakable joints whose last step's linear impulse passed their
    /// threshold come apart (`broken`).
    fn break_joints(&mut self) {
        let mut gone = Vec::new();
        self.breakable.retain(|&(h, threshold, owner, i)| {
            let Some(j) = self.world.impulse_joints.get(h) else {
                return false;
            };
            // Locked axes' impulses and the linear limits' (a rope's length).
            let locked = Vec3::new(j.impulses[0], j.impulses[1], j.impulses[2]);
            let limits = Vec3::new(
                j.data.limits[0].impulse,
                j.data.limits[1].impulse,
                j.data.limits[2].impulse,
            );
            let impulse = (locked.length_squared() + limits.length_squared()).sqrt();
            if impulse > 1.0 {
                log::trace!("{owner} joint {i} impulse {impulse:.0}");
            }
            if impulse <= threshold {
                return true;
            }
            log::info!("{owner}: joint {i} breaks ({impulse:.0} > {threshold:.0})");
            gone.push((h, owner, i));
            false
        });
        for (h, owner, i) in gone {
            self.world.remove_impulse_joint(h);
            self.broken.push((owner, i));
        }
    }

    /// Move the player capsule (centre position) by `desired`. Returns (new position, grounded).
    /// Loose objects in the way are pushed aside.
    pub fn move_player(&mut self, pos: Vec3, desired: Vec3, dt: f32) -> (Vec3, bool) {
        let pose = Pose::from_translation(pos);
        let mut hits = Vec::new();
        let filter = match self.held {
            Some(h) => QueryFilter::default().exclude_rigid_body(h),
            None => QueryFilter::default(),
        };
        let m = {
            let qp = self.world.query_pipeline_with_filter(filter);
            self.controller
                .move_shape(dt, &qp, &*self.player_shape.0, &pose, desired, |c| {
                    hits.push(c)
                })
        };
        if !hits.is_empty() {
            let w = &mut self.world;
            let mut qp = w.broad_phase.as_query_pipeline_mut(
                w.narrow_phase.query_dispatcher(),
                &mut w.bodies,
                &mut w.colliders,
                filter,
            );
            self.controller.solve_character_collision_impulses(
                dt,
                &mut qp,
                &*self.player_shape.0,
                PLAYER_MASS,
                &hits,
            );
        }
        (pos + m.translation, m.grounded)
    }

    /// Cast a ray at the ground and what stands on it, ignoring actors: the hit
    /// distance and surface normal.
    pub fn ground_ray(&self, origin: Vec3, dir: Vec3, max: f32) -> Option<(f32, Vec3)> {
        let ray = Ray::new(origin, dir);
        let not_actor = |h: ColliderHandle, _: &Collider| !self.capsules.contains(&h);
        let (_, hit) = self.world.cast_ray_and_get_normal(
            &ray,
            max,
            true,
            QueryFilter::default().predicate(&not_actor),
        )?;
        Some((hit.time_of_impact, hit.normal))
    }

    /// What the ground straight below `origin` (within `max`) is made of, ignoring
    /// actors: the hit distance and the surface.
    pub fn surface_below(&self, origin: Vec3, max: f32) -> Option<(f32, Surface)> {
        let ray = Ray::new(origin, -Vec3::Z);
        let not_actor = |h: ColliderHandle, _: &Collider| !self.capsules.contains(&h);
        let (h, hit) = self.world.cast_ray_and_get_normal(
            &ray,
            max,
            true,
            QueryFilter::default().predicate(&not_actor),
        )?;
        if self.terrain.contains(&h) {
            return Some((hit.time_of_impact, Surface::Terrain));
        }
        let m = self.materials.get(&h)?;
        // Mesh faces are numbered past the triangle count when hit from behind.
        let i = match hit.feature {
            FeatureId::Face(i) if m.len() > 1 => i as usize % m.len(),
            _ => 0,
        };
        Some((hit.time_of_impact, Surface::Havok(*m.get(i)?)))
    }

    /// Cast a ray past what `owner` owns (its capsule, its ragdoll), returning
    /// the hit distance and the owning reference (if any).
    pub fn raycast_excluding(
        &self,
        origin: Vec3,
        dir: Vec3,
        max: f32,
        owner: esp::FormId,
    ) -> Option<(f32, Option<esp::FormId>)> {
        let ray = Ray::new(origin, dir);
        let not_owner = |h: ColliderHandle, _: &Collider| self.owners.get(&h) != Some(&owner);
        let (h, toi) = self.world.cast_ray(
            &ray,
            max,
            true,
            QueryFilter::default().predicate(&not_owner),
        )?;
        Some((toi, self.owners.get(&h).copied()))
    }

    /// Diagnostics: every collider a ray passes through, tested one by one rather
    /// than through the broad phase, nearest first. Each hit says whether scene
    /// queries (the broad phase) also see it, and whether it is enabled.
    pub fn probe_ray(&self, origin: Vec3, dir: Vec3, max: f32) -> Vec<ProbeHit> {
        let ray = Ray::new(origin, dir);
        let seen: std::collections::HashSet<ColliderHandle> = self
            .world
            .intersect_ray(ray, max, true, QueryFilter::default())
            .map(|(h, _, _)| h)
            .collect();
        let mut hits: Vec<ProbeHit> = self
            .world
            .colliders
            .iter()
            .filter_map(|(h, co)| {
                let hit = co
                    .shape()
                    .cast_ray_and_get_normal(co.position(), &ray, max, true)?;
                Some(ProbeHit {
                    handle: h,
                    toi: hit.time_of_impact,
                    in_broad_phase: seen.contains(&h),
                    enabled: co.is_enabled(),
                    owner: self.owners.get(&h).copied(),
                    bounds: {
                        let b = co.compute_aabb();
                        (b.mins, b.maxs)
                    },
                })
            })
            .collect();
        hits.sort_by(|a, b| a.toi.total_cmp(&b.toi));
        hits
    }

    /// Whether the collider still exists.
    pub fn has_collider(&self, h: ColliderHandle) -> bool {
        self.world.colliders.contains(h)
    }

    /// Cast a ray: the hit distance, the owning reference (if any) and the
    /// body hit (none for the world's fixed collision).
    pub fn raycast_body(
        &self,
        origin: Vec3,
        dir: Vec3,
        max: f32,
    ) -> Option<(f32, Option<esp::FormId>, Option<RigidBodyHandle>)> {
        let ray = Ray::new(origin, dir);
        let (h, toi) = self
            .world
            .cast_ray(&ray, max, true, QueryFilter::default())?;
        let body = self.world.colliders.get(h).and_then(|c| c.parent());
        Some((toi, self.owners.get(&h).copied(), body))
    }

    /// Whether a body is simulated, and its point at `world` in its own frame.
    pub fn body_local_point(&self, h: RigidBodyHandle, world: Vec3) -> Option<(bool, Vec3)> {
        let b = self.world.bodies.get(h)?;
        Some((b.is_dynamic(), b.position().inverse_transform_point(world)))
    }

    /// Pull a held body's point `local` (in its frame) towards `target` for the
    /// next step of `dt`, as if `mass` hung there (the whole object, or the
    /// whole ragdoll), with at most `max_force`, its weight held up within it;
    /// its spin damped. Returns how far the point is from the target, none if
    /// the body is gone.
    pub fn drive_held(
        &mut self,
        h: RigidBodyHandle,
        local: Vec3,
        target: Vec3,
        mass: f32,
        max_force: f32,
        dt: f32,
    ) -> Option<f32> {
        /// How fast the point closes on the target (per second), its top speed
        /// (game units / s), and the spin kept per step (no source).
        const GAIN: f32 = 12.0;
        const TOP_SPEED: f32 = 1200.0;
        const SPIN_KEPT: f32 = 0.85;
        let b = self.world.bodies.get_mut(h)?;
        let p = b.position().transform_point(local);
        let off = target.distance(p);
        if !b.is_dynamic() {
            return Some(off);
        }
        let desired = ((target - p) * GAIN).clamp_length_max(TOP_SPEED);
        let need = (desired - b.velocity_at_point(p)) * mass + Vec3::Z * GRAVITY * mass * dt;
        b.apply_impulse_at_point(need.clamp_length_max(max_force * dt), p, true);
        b.set_angvel(b.angvel() * SPIN_KEPT, true);
        Some(off)
    }

    /// What body `h` touches (within `margin`) of the actors' capsules and the
    /// player's (centred at `player`, if given): the actor (none for the
    /// player), the contact point, the body's velocity there (game units / s)
    /// and the Havok material of its collider.
    pub fn body_touches(
        &self,
        h: RigidBodyHandle,
        player: Option<Vec3>,
        margin: f32,
    ) -> Vec<(Option<esp::FormId>, Vec3, Vec3, u32)> {
        let Some(b) = self.world.bodies.get(h) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for &ch in b.colliders() {
            self.collider_touches(ch, player, margin, &|p| b.velocity_at_point(p), &mut out);
        }
        out
    }

    /// [`Physics::body_touches`] for one collider, its velocity at a point
    /// given by `vel`; added to `out`, one touch per actor (or the player).
    pub fn collider_touches(
        &self,
        ch: ColliderHandle,
        player: Option<Vec3>,
        margin: f32,
        vel: &dyn Fn(Vec3) -> Vec3,
        out: &mut Vec<(Option<esp::FormId>, Vec3, Vec3, u32)>,
    ) {
        let Some(c) = self.world.colliders.get(ch) else {
            return;
        };
        let aabb = c.compute_aabb().loosened(margin);
        let material = self
            .materials
            .get(&ch)
            .and_then(|m| m.first().copied())
            .unwrap_or(0);
        let mut touch = |who: Option<esp::FormId>, pose: &Pose, shape: &dyn Shape| {
            if out.iter().any(|(w, ..)| *w == who) {
                return;
            }
            if let Ok(Some(k)) =
                rapier3d::parry::query::contact(c.position(), c.shape(), pose, shape, margin)
            {
                out.push((who, k.point1, vel(k.point1), material));
            }
        };
        for &cap in &self.capsules {
            let Some(o) = self.world.colliders.get(cap).filter(|o| o.is_enabled()) else {
                continue;
            };
            if o.compute_aabb().intersects(&aabb) {
                touch(self.owners.get(&cap).copied(), o.position(), o.shape());
            }
        }
        if let Some(p) = player {
            let pose = Pose::from_translation(p);
            if self.player_shape.compute_aabb(&pose).intersects(&aabb) {
                touch(None, &pose, &*self.player_shape.0);
            }
        }
    }

    /// Slow a body to at most `speed` (game units / s).
    pub fn cap_speed(&mut self, h: RigidBodyHandle, speed: f32) {
        if let Some(b) = self.world.bodies.get_mut(h) {
            let v = b.linvel().clamp_length_max(speed);
            b.set_linvel(v, true);
        }
    }

    /// The mass of a body and of the bodies joined to it, all told (kg).
    pub fn joined_mass(&self, h: RigidBodyHandle) -> f32 {
        let mut seen = vec![h];
        let mut i = 0;
        while let Some(&b) = seen.get(i) {
            i += 1;
            for (a, c, _, _) in self.world.impulse_joints.attached_joints(b) {
                for o in [a, c] {
                    if !seen.contains(&o) {
                        seen.push(o);
                    }
                }
            }
        }
        seen.iter()
            .filter_map(|&b| self.world.bodies.get(b))
            .filter(|b| b.is_dynamic())
            .map(|b| b.mass())
            .sum()
    }

    /// Cast a ray, returning the hit distance and the owning reference (if any).
    pub fn raycast(&self, origin: Vec3, dir: Vec3, max: f32) -> Option<(f32, Option<esp::FormId>)> {
        let ray = Ray::new(origin, dir);
        let (h, toi) = self
            .world
            .cast_ray(&ray, max, true, QueryFilter::default())?;
        Some((toi, self.owners.get(&h).copied()))
    }
}

pub type CollisionCache = HashMap<String, Option<Arc<CollisionModel>>>;

#[cfg(test)]
mod tests {
    use super::*;

    /// A free ball of `mass` at the origin, and the world.
    fn ball(mass: f32) -> (Physics, RigidBodyHandle) {
        let mut p = Physics::new();
        let h = p.world.insert_body(RigidBodyBuilder::dynamic());
        p.world
            .insert_collider(ColliderBuilder::ball(5.0).mass(mass).build(), Some(h));
        (p, h)
    }

    /// Hold the ball's centre towards `target` for `steps` steps; where it ends.
    fn hold(p: &mut Physics, h: RigidBodyHandle, target: Vec3, max_force: f32, steps: u32) -> Vec3 {
        let dt = 1.0 / 60.0;
        for _ in 0..steps {
            let mass = p.joined_mass(h);
            p.drive_held(h, Vec3::ZERO, target, mass, max_force, dt);
            p.step(dt);
        }
        p.body_pose(h).unwrap().0.w_axis.truncate()
    }

    #[test]
    fn held_body_comes_to_the_target() {
        let (mut p, h) = ball(5.0);
        let target = Vec3::new(30.0, 0.0, 40.0);
        let at = hold(&mut p, h, target, 150.0 * GRAVITY, 120);
        assert!(at.distance(target) < 2.0, "{at:?}");
    }

    #[test]
    fn touching_capsules_and_the_player() {
        let (mut p, h) = ball(5.0);
        let actor = esp::FormId(0x1234);
        // An actor standing beside the ball, the player further off.
        p.add_actor_capsule(Vec3::new(24.0, 0.0, -40.0), 1.0, actor);
        p.set_body_pose(h, Mat4::IDENTITY);
        let far = Vec3::new(-200.0, 0.0, 0.0);
        let touches = p.body_touches(h, Some(far), 2.0);
        assert_eq!(touches.len(), 1);
        assert_eq!(touches[0].0, Some(actor));
        // The player's capsule right on it.
        let touches = p.body_touches(h, Some(Vec3::new(0.0, 25.0, 0.0)), 2.0);
        assert!(touches.iter().any(|t| t.0.is_none()));
    }

    #[test]
    fn too_heavy_to_lift() {
        let (mut p, h) = ball(300.0);
        let at = hold(&mut p, h, Vec3::new(0.0, 0.0, 40.0), 150.0 * GRAVITY, 60);
        assert!(at.z < 0.0, "{at:?}");
    }
}
