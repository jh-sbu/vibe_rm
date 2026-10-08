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

/// A ragdoll in the simulation: one body per ragdoll body of the description, and
/// the actor's scale.
pub struct Ragdoll {
    pub bodies: Vec<RigidBodyHandle>,
    pub scale: f32,
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
        for h in hs {
            if let Some(c) = self.world.colliders.get_mut(h) {
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
            let frame = |f: (Vec3, glam::Quat)| Pose::from_parts(f.0 * scale, f.1);
            let mask = if j.limits[1].is_some() {
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
            self.world
                .insert_impulse_joint(bodies[j.a], bodies[j.b], joint.build());
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
    }

    /// Move the player capsule (centre position) by `desired`. Returns (new position, grounded).
    pub fn move_player(&self, pos: Vec3, desired: Vec3, dt: f32) -> (Vec3, bool) {
        let pose = Pose::from_translation(pos);
        let qp = self.world.query_pipeline();
        let m = self
            .controller
            .move_shape(dt, &qp, &*self.player_shape.0, &pose, desired, |_| {});
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
