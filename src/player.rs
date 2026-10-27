//! The player: first-person movement driven by the physics character controller.

use glam::Vec3;

use crate::physics::{GRAVITY, Physics};
use crate::render::Camera;

/// HUD visibility groups supported by the game's HUD movie.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum HudMode {
    #[default]
    All,
    Cart,
    MovementDisabled,
    Dialogue,
}

impl HudMode {
    /// Movement disabling temporarily overrides cart mode without clearing it.
    /// Cart mode takes precedence over dialogue here; see player-controls.md.
    pub fn select(cart: bool, movement_disabled: bool, dialogue: bool) -> Self {
        if movement_disabled {
            Self::MovementDisabled
        } else if cart {
            Self::Cart
        } else if dialogue {
            Self::Dialogue
        } else {
            Self::All
        }
    }

    pub fn movie_name(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Cart => "CartMode",
            Self::MovementDisabled => "MovementDisabled",
            Self::Dialogue => "DialogueMode",
        }
    }
}

/// Which of the player's controls scripts have disabled
/// (`Game.DisablePlayerControls` / `EnablePlayerControls`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DisabledControls {
    pub movement: bool,
    pub fighting: bool,
    /// Camera switching disabled by scripts (POV type 0).
    cam_switch: bool,
    /// Separate POV lock owned by beast transformations (POV type 1).
    werewolf_cam_switch: bool,
    pub looking: bool,
    pub sneaking: bool,
    pub menu: bool,
    pub activate: bool,
    pub journal: bool,
}

impl DisabledControls {
    /// Persist the two independent POV-lock owners separately.
    pub(crate) fn saved_bits(self) -> u16 {
        let flags = [
            self.movement,
            self.fighting,
            self.cam_switch,
            self.looking,
            self.sneaking,
            self.menu,
            self.activate,
            self.journal,
            self.werewolf_cam_switch,
        ];
        flags
            .into_iter()
            .enumerate()
            .fold(0, |bits, (i, flag)| bits | ((flag as u16) << i))
    }
    pub(crate) fn from_saved_bits(bits: u16) -> Self {
        let mut controls = Self::default();
        for (i, flag) in controls.flags_mut().into_iter().enumerate() {
            *flag = bits & (1 << i) != 0;
        }
        controls.werewolf_cam_switch = bits & (1 << 8) != 0;
        controls
    }

    /// Whether scripts have disabled any control.
    pub fn any(&self) -> bool {
        *self != Self::default()
    }

    /// Apply the selected controls, preserving flags whose argument is false.
    /// Only the camera flag is scoped by Papyrus's `aiDisablePOVType`.
    pub fn apply(&mut self, disable: bool, selected: [bool; 8], pov_type: i32) {
        for (i, flag) in self.flags_mut().into_iter().enumerate() {
            if i != 2 && selected[i] {
                *flag = disable;
            }
        }
        if selected[2] {
            match pov_type {
                0 => self.cam_switch = disable,
                1 => self.werewolf_cam_switch = disable,
                // No public contract exists for other values. Do not release
                // another system's camera lock for an unknown owner.
                _ => (),
            }
        }
    }

    pub fn camera_switch_disabled(&self) -> bool {
        self.cam_switch || self.werewolf_cam_switch
    }

    /// The controls in Papyrus's argument order (movement, fighting, camera
    /// switch, looking, sneaking, menus, activation, journal tabs).
    fn flags_mut(&mut self) -> [&mut bool; 8] {
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
        c.cam_switch = self.camera_switch_disabled();
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

#[cfg(test)]
mod control_tests {
    use super::{DisabledControls, HudMode};

    #[test]
    fn cart_mode_returns_after_movement_is_enabled() {
        let mut controls = DisabledControls::default();
        let movement = [true, false, false, false, false, false, false, false];
        assert_eq!(
            HudMode::select(true, controls.movement, false),
            HudMode::Cart
        );
        controls.apply(true, movement, 0);
        assert_eq!(
            HudMode::select(true, controls.movement, false),
            HudMode::MovementDisabled
        );
        controls.apply(false, movement, 0);
        assert_eq!(
            HudMode::select(true, controls.movement, false),
            HudMode::Cart
        );
        assert_eq!(
            HudMode::select(false, controls.movement, false),
            HudMode::All
        );
    }

    #[test]
    fn clearing_cart_mode_restores_the_other_active_mode() {
        assert_eq!(
            HudMode::select(false, true, false),
            HudMode::MovementDisabled
        );
        assert_eq!(HudMode::select(false, false, true), HudMode::Dialogue);
        assert_eq!(HudMode::select(true, false, true), HudMode::Cart);
        assert_eq!(HudMode::select(true, true, true), HudMode::MovementDisabled);
    }

    const CAMERA: [bool; 8] = [false, false, true, false, false, false, false, false];

    #[test]
    fn enabling_one_pov_owner_preserves_the_other_owners_lock() {
        for (first, second) in [(0, 1), (1, 0)] {
            let mut controls = DisabledControls::default();
            controls.apply(true, CAMERA, first);
            controls.apply(true, CAMERA, second);
            controls.apply(false, CAMERA, first);
            assert!(controls.camera_switch_disabled());
            assert_eq!(controls.describe(), "disabled: camera switch");
            controls.apply(false, CAMERA, second);
            assert!(!controls.camera_switch_disabled());
            assert_eq!(controls.describe(), "all player controls enabled");
        }
    }

    #[test]
    fn false_camera_argument_preserves_both_pov_locks() {
        let mut controls = DisabledControls::default();
        controls.apply(true, CAMERA, 1);
        let mut selected = [true; 8];
        selected[2] = false;
        controls.apply(false, selected, 1);
        assert!(controls.camera_switch_disabled());
        controls.apply(false, CAMERA, 1);
        assert!(!controls.camera_switch_disabled());
    }

    #[test]
    fn other_controls_are_shared_across_pov_owners() {
        let mut controls = DisabledControls::default();
        controls.apply(true, [true; 8], 1);
        controls.apply(false, [true; 8], 0);
        assert!(controls.camera_switch_disabled());
        assert!(!controls.movement);
        assert!(!controls.fighting);
        assert!(!controls.looking);
        assert!(!controls.sneaking);
        assert!(!controls.menu);
        assert!(!controls.activate);
        assert!(!controls.journal);
        // The developer console's full reset clears both owners.
        controls = DisabledControls::default();
        assert!(!controls.camera_switch_disabled());
    }

    #[test]
    fn unknown_pov_owners_preserve_camera_locks_but_apply_other_controls() {
        let mut controls = DisabledControls::default();
        controls.apply(true, CAMERA, 0);
        controls.apply(true, CAMERA, 1);
        for unknown in [-1, 2, i32::MAX] {
            controls.apply(false, [true; 8], unknown);
            assert!(controls.camera_switch_disabled());
            assert!(!controls.movement);
            controls.apply(true, [true; 8], unknown);
            assert!(controls.movement);
        }
        controls.apply(false, CAMERA, 0);
        assert!(controls.camera_switch_disabled());
        controls.apply(false, CAMERA, 1);
        assert!(!controls.camera_switch_disabled());
        controls.apply(true, CAMERA, 2);
        assert!(!controls.camera_switch_disabled());
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
    pub sprinting: bool,
    pub swimming: bool,
    pub(crate) swim_surface: Option<f32>,
    /// The highest the capsule has been since it left the ground.
    fall_top: Option<f32>,
    /// How far it came down, if it landed this update.
    pub landed_from: Option<f32>,
    /// Where the last update left it (a jump from there is a teleport).
    last: Vec3,
    /// Carrying more than they can (`crate::encumbrance`).
    pub overencumbered: bool,
}

/// Moved further than this between updates, the player was put somewhere
/// (a load door, `MoveTo`): a fall in progress doesn't count from before.
const TELEPORT: f32 = 256.0;

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
            sprinting: false,
            swimming: false,
            swim_surface: None,
            fall_top: None,
            landed_from: None,
            last: eye - Vec3::Z * EYE_OFFSET,
            overencumbered: false,
        }
    }

    pub fn eye(&self) -> Vec3 {
        self.position + Vec3::Z * (EYE_OFFSET - SNEAK_DROP * self.crouch)
    }

    /// `sneak_speed`: the share of their usual speed they keep sneaking;
    /// `armor_speed`: the share their worn armor leaves them.
    pub fn update(
        &mut self,
        physics: &mut Physics,
        camera: &Camera,
        input: MoveInput,
        sneak_speed: f32,
        armor_speed: f32,
        dt: f32,
    ) {
        self.jumped = false;
        self.landed_from = None;
        if self.noclip || self.position.distance(self.last) > TELEPORT {
            self.fall_top = None;
        }
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
            self.last = self.position;
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
        } * if self.sneaking { sneak_speed } else { 1.0 }
            * armor_speed;
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
        self.last = pos;
        if !grounded {
            let top = self.fall_top.get_or_insert(pos.z);
            *top = top.max(pos.z);
        } else if let Some(top) = self.fall_top.take() {
            self.landed_from = Some(top - pos.z);
        }
    }
}
