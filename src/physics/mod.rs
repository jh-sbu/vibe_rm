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

pub struct Physics {
    pub world: PhysicsWorld,
    controller: KinematicCharacterController,
    player_shape: SharedShape,
    /// Collider -> reference FormID that owns it.
    pub owners: HashMap<ColliderHandle, esp::FormId>,
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
    pub fn add_static_tagged(&mut self, model: &CollisionModel, transform: Mat4, owner: esp::FormId) -> Vec<(ColliderHandle, Option<String>)> {
        let mut handles = Vec::new();
        for part in &model.parts {
            if !layer_blocks_player(part.layer) {
                continue;
            }
            let (pose, scale) = shapes::decompose(transform * part.transform);
            let Some(shape) = part.shape.build(scale) else { continue };
            let c = ColliderBuilder::new(shape).position(pose).build();
            let h = self.world.insert_collider(c, None);
            self.owners.insert(h, owner);
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
    pub fn add_actor_capsule(&mut self, feet: Vec3, scale: f32, owner: esp::FormId) -> ColliderHandle {
        let r = 20.0 * scale;
        let half = 40.0 * scale;
        let c = ColliderBuilder::new(SharedShape::capsule_z(half, r))
            .position(Pose::from_translation(feet + Vec3::Z * (half + r)))
            .build();
        let h = self.world.insert_collider(c, None);
        self.owners.insert(h, owner);
        h
    }

    /// Move an actor capsule so that its feet are at `feet`.
    pub fn move_actor_capsule(&mut self, h: ColliderHandle, feet: Vec3) {
        if let Some(c) = self.world.colliders.get_mut(h) {
            let lift = c.shape().as_capsule().map(|cap| cap.half_height() + cap.radius).unwrap_or(0.0);
            c.set_translation(feet + Vec3::Z * lift);
        }
    }

    /// Enable or disable all colliders owned by a reference.
    pub fn set_owner_enabled(&mut self, owner: esp::FormId, enabled: bool) {
        let hs: Vec<ColliderHandle> = self.owners.iter().filter(|(_, o)| **o == owner).map(|(h, _)| *h).collect();
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
                v.push(Vec3::new(o.x + c as f32 * step, o.y + r as f32 * step, land.heights[r * VERTS + c]));
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
        Some(self.world.insert_collider(ColliderBuilder::new(shape).build(), None))
    }

    /// Update the broad phase after colliders were added.
    pub fn step(&mut self, dt: f32) {
        self.world.integration_parameters.dt = dt.clamp(1.0 / 240.0, 1.0 / 30.0);
        self.world.step();
    }

    /// Move the player capsule (centre position) by `desired`. Returns (new position, grounded).
    pub fn move_player(&self, pos: Vec3, desired: Vec3, dt: f32) -> (Vec3, bool) {
        let pose = Pose::from_translation(pos);
        let qp = self.world.query_pipeline();
        let m = self.controller.move_shape(dt, &qp, &*self.player_shape.0, &pose, desired, |_| {});
        (pos + m.translation, m.grounded)
    }

    /// Cast a ray, returning the hit distance and the owning reference (if any).
    pub fn raycast(&self, origin: Vec3, dir: Vec3, max: f32) -> Option<(f32, Option<esp::FormId>)> {
        let ray = Ray::new(origin, dir);
        let (h, toi) = self.world.cast_ray(&ray, max, true, QueryFilter::default())?;
        Some((toi, self.owners.get(&h).copied()))
    }
}

pub type CollisionCache = HashMap<String, Option<Arc<CollisionModel>>>;
