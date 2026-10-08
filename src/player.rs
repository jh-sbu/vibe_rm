//! The player: first-person movement driven by the physics character controller.

use glam::Vec3;

use crate::physics::{GRAVITY, Physics};
use crate::render::Camera;

/// Which of the player's controls scripts have disabled
/// (`Game.DisablePlayerControls` / `EnablePlayerControls`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DisabledControls {
    pub movement: bool,
    pub fighting: bool,
    pub cam_switch: bool,
    pub looking: bool,
    pub sneaking: bool,
    pub menu: bool,
    pub activate: bool,
    pub journal: bool,
}

impl DisabledControls {
    /// The controls in Papyrus's argument order (movement, fighting, camera
    /// switch, looking, sneaking, menus, activation, journal tabs).
    pub fn flags_mut(&mut self) -> [&mut bool; 8] {
        [
            &mut self.movement,
            &mut self.fighting,
            &mut self.cam_switch,
            &mut self.looking,
            &mut self.sneaking,
            &mut self.menu,
            &mut self.activate,
            &mut self.journal,
        ]
    }

    pub fn describe(&self) -> String {
        let names = [
            "movement",
            "fighting",
            "camera switch",
            "looking",
            "sneaking",
            "menus",
            "activation",
            "journal",
        ];
        let mut c = *self;
        let off: Vec<&str> = c
            .flags_mut()
            .into_iter()
            .zip(names)
            .filter(|(f, _)| **f)
            .map(|(_, n)| n)
            .collect();
        if off.is_empty() {
            "all player controls enabled".into()
        } else {
            format!("disabled: {}", off.join(", "))
        }
    }
}

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
    /// Sneaking (toggled), and how far down the view has gone for it (0..1).
    pub sneaking: bool,
    pub crouch: f32,
    /// Whether they left the ground by jumping this update.
    pub jumped: bool,
    /// Moving over the ground this update, and running (or sprinting) while at it.
    pub moving: bool,
    pub running: bool,
}

pub const WALK_SPEED: f32 = 110.0;
pub const RUN_SPEED: f32 = 300.0;
pub const SPRINT_SPEED: f32 = 480.0;
pub const JUMP_SPEED: f32 = 330.0;
/// Eye height above the capsule centre.
pub const EYE_OFFSET: f32 = 56.0;
/// How far the eye drops sneaking, and how quickly (crouch fraction per second).
const SNEAK_DROP: f32 = 30.0;
const CROUCH_RATE: f32 = 4.0;

impl Player {
    pub fn new(eye: Vec3) -> Self {
        Player {
            position: eye - Vec3::Z * EYE_OFFSET,
            vertical_velocity: 0.0,
            grounded: false,
            noclip: false,
            sneaking: false,
            crouch: 0.0,
            jumped: false,
            moving: false,
            running: false,
        }
    }

    pub fn eye(&self) -> Vec3 {
        self.position + Vec3::Z * (EYE_OFFSET - SNEAK_DROP * self.crouch)
    }

    /// `sneak_speed`: the share of their usual speed they keep sneaking.
    pub fn update(
        &mut self,
        physics: &mut Physics,
        camera: &Camera,
        input: MoveInput,
        sneak_speed: f32,
        dt: f32,
    ) {
        self.jumped = false;
        let crouch = if self.sneaking && !self.noclip {
            1.0
        } else {
            0.0
        };
        self.crouch += (crouch - self.crouch).clamp(-CROUCH_RATE * dt, CROUCH_RATE * dt);
        if self.noclip {
            let mut speed = 600.0;
            if input.run {
                speed *= 6.0;
            }
            let v = camera.forward() * input.forward
                + camera.right() * input.right
                + Vec3::Z * input.up;
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
        } * if self.sneaking { sneak_speed } else { 1.0 };
        let horizontal = (fwd * input.forward + right * input.right).normalize_or_zero() * speed;
        if self.grounded && input.jump {
            self.vertical_velocity = JUMP_SPEED;
            self.grounded = false;
            self.jumped = true;
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
