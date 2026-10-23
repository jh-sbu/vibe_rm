//! The skills menu's sky: the game's skydome (`Interface/INTPerkSkydome.nif`)
//! drawn as the scene behind `statsmenu.swf`, the ring of skills and a skill's
//! perks alike. Its eighteen constellation images (`cImage00` to `cImage17`)
//! and the `Point00` to `Point17` nodes stand around its origin 20 degrees
//! apart, clockwise from +Y seen from above: the ring's skills, in the ring's
//! order. The camera stands at the origin and faces the selected skill's. The
//! menu draws its own scene, as the map does; the loaded cells aren't in it.
//! Choices: `known_ambiguities/skills-menu.md`.

use glam::{Mat4, Vec3, Vec4};

use crate::engine::Engine;
use crate::render::{Camera, Environment, Instance, Scene};

const DOME: &str = "meshes/interface/intperkskydome.nif";
/// The camera's vertical field of view on the ring and in a skill's perks
/// (closer in on the skill's constellation), degrees.
const RING_FOV: f32 = 60.0;
const PERKS_FOV: f32 = 40.0;
/// How far a skill's perks look up, degrees: to the middle of the
/// constellation images, which stand about 15 units over the origin at a
/// radius of about 90.
const PERKS_PITCH: f32 = 9.5;

pub struct StatsView {
    scene: Scene,
    in_perks: bool,
    /// The ring's turn, degrees: the heading the camera faces.
    angle: f32,
}

impl StatsView {
    /// The scene to draw.
    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    /// Whether a skill's tree is open rather than the ring.
    pub fn set_perks(&mut self, open: bool) {
        self.in_perks = open;
    }

    pub fn camera(&self) -> Camera {
        let (fov, pitch) = if self.in_perks {
            (PERKS_FOV, PERKS_PITCH)
        } else {
            (RING_FOV, 0.0)
        };
        Camera {
            position: Vec3::ZERO,
            yaw: self.angle.to_radians(),
            pitch: pitch.to_radians(),
            fov_y: fov.to_radians(),
        }
    }

    /// Faces the skill at the ring's angle (20 degrees a skill).
    pub fn set_angle(&mut self, angle: f32) {
        self.angle = angle;
    }
}

/// The skills menu's sky and camera while it is up.
pub fn frame_parts(
    stats: &Option<StatsView>,
    menu: Option<crate::items::Menu>,
) -> Option<(&Scene, Camera)> {
    match stats {
        Some(v) if menu == Some(crate::items::Menu::Skills) => Some((v.scene(), v.camera())),
        _ => None,
    }
}

impl Engine {
    /// Builds the skills menu's sky. None when the skydome can't be loaded.
    pub fn open_stats_view(&mut self) -> Option<()> {
        let path = DOME.to_string();
        self.models
            .load_all(&mut self.renderer, &self.vfs, std::slice::from_ref(&path));
        let mut sky = Instance::new(self.models.get(&path)?, Mat4::IDENTITY);
        sky.sky = true;
        sky.tint = Vec4::ONE;
        let scene = Scene {
            env: Environment {
                sun_color: Vec3::ZERO,
                ambient: Vec3::ONE,
                fog_far: 1.0e7,
                clear_color: Vec3::new(0.01, 0.012, 0.02),
                sky: false,
                ..Default::default()
            },
            sky: vec![sky],
            ..Default::default()
        };
        self.stats_view = Some(StatsView {
            scene,
            in_perks: false,
            angle: 0.0,
        });
        Some(())
    }

    pub fn close_stats_view(&mut self) {
        self.stats_view = None;
    }
}
