//! The player: first-person movement driven by the physics character controller.

use glam::Vec3;

use crate::physics::{GRAVITY, Physics};
use crate::render::Camera;

#[derive(Debug, Default, Clone, Copy)]
pub struct MoveInput {
    pub forward: f32,
    pub right: f32,
    pub up: f32,
    pub run: bool,
    pub sprint: bool,
    pub jump: bool,
}

pub struct Player {
    /// Centre of the collision capsule.
    pub position: Vec3,
    pub vertical_velocity: f32,
    pub grounded: bool,
    pub noclip: bool,
}

pub const WALK_SPEED: f32 = 110.0;
pub const RUN_SPEED: f32 = 300.0;
pub const SPRINT_SPEED: f32 = 480.0;
pub const JUMP_SPEED: f32 = 330.0;
/// Eye height above the capsule centre.
pub const EYE_OFFSET: f32 = 56.0;

impl Player {
    pub fn new(eye: Vec3) -> Self {
        Player { position: eye - Vec3::Z * EYE_OFFSET, vertical_velocity: 0.0, grounded: false, noclip: false }
    }

    pub fn eye(&self) -> Vec3 {
        self.position + Vec3::Z * EYE_OFFSET
    }

    pub fn update(&mut self, physics: &Physics, camera: &Camera, input: MoveInput, dt: f32) {
        if self.noclip {
            let mut speed = 600.0;
            if input.run {
                speed *= 6.0;
            }
            let v = camera.forward() * input.forward + camera.right() * input.right + Vec3::Z * input.up;
            self.position += v.normalize_or_zero() * speed * dt;
            self.vertical_velocity = 0.0;
            return;
        }
        let fwd = Vec3::new(camera.yaw.sin(), camera.yaw.cos(), 0.0);
        let right = Vec3::new(camera.yaw.cos(), -camera.yaw.sin(), 0.0);
        let speed = if input.sprint {
            SPRINT_SPEED
        } else if input.run {
            RUN_SPEED
        } else {
            WALK_SPEED
        };
        let horizontal = (fwd * input.forward + right * input.right).normalize_or_zero() * speed;
        if self.grounded && input.jump {
            self.vertical_velocity = JUMP_SPEED;
            self.grounded = false;
        }
        self.vertical_velocity -= GRAVITY * dt;
        self.vertical_velocity = self.vertical_velocity.max(-60.0 * crate::physics::METRE);
        let desired = horizontal * dt + Vec3::Z * self.vertical_velocity * dt;
        let (pos, grounded) = physics.move_player(self.position, desired, dt);
        // Stop vertical motion when blocked (landing or bumping the head).
        let moved_z = pos.z - self.position.z;
        if grounded && self.vertical_velocity < 0.0 {
            self.vertical_velocity = 0.0;
        } else if self.vertical_velocity > 0.0 && moved_z < self.vertical_velocity * dt * 0.5 {
            self.vertical_velocity = 0.0;
        }
        self.grounded = grounded;
        self.position = pos;
    }
}
