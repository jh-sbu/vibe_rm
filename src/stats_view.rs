//! The skills menu's sky: the game's own star models (`Interface/
//! INTStatMenuStars.nif`, the starfields and constellations the ring of skills
//! turns under) drawn as the scene behind `statsmenu.swf`. The menu draws its
//! own scene, as the map does; the loaded cells aren't in it. Choices:
//! `known_ambiguities/skills-menu.md`.

use glam::{Mat4, Vec3, Vec4};

use crate::engine::Engine;
use crate::render::{Camera, Environment, Instance, Scene};

/// The ring screen's stars.
///
/// Its mage, thief and warrior constellations' planes (`const01:0`,
/// `const05:0`, `const09:0`) name textures (`INTmageConst`, `INTtheifConst`,
/// `INTwarConst`) that the installed archives don't have, so they are left
/// out.
const STARS: &str = "meshes/interface/intstatmenustars.nif#without=const01:0|const05:0|const09:0";
/// The sky's model scale, as the weather's.
const SCALE: f32 = 1.0;
/// The sky of a skill's perks.
const DOME: &str = "meshes/interface/intperkskydome.nif";
/// Where the model's `CameraPosition` node puts the camera.
const CAMERA_AT: Vec3 = Vec3::new(0.0, -198.345, -123.842);

pub struct StatsView {
    pub scene: Scene,
    /// The skydome a skill's perks sit in, drawn instead while a tree is open.
    perks: Scene,
    in_perks: bool,
    /// The ring's turn, degrees.
    angle: f32,
}

impl StatsView {
    /// The scene to draw.
    pub fn scene(&self) -> &Scene {
        if self.in_perks {
            &self.perks
        } else {
            &self.scene
        }
    }

    /// Whether a skill's tree is open (its sky) rather than the ring.
    pub fn set_perks(&mut self, open: bool) {
        self.in_perks = open;
    }

    pub fn camera(&self) -> Camera {
        if self.in_perks {
            return Camera {
                position: Vec3::ZERO,
                yaw: 0.0,
                pitch: 0.0,
                fov_y: 60f32.to_radians(),
            };
        }
        Camera {
            position: CAMERA_AT,
            yaw: 0.0,
            // The model's `CameraPosition` node is tilted up 30 degrees.
            pitch: 30f32.to_radians(),
            fov_y: 60f32.to_radians(),
        }
    }

    /// Turns the constellations to the ring's angle.
    pub fn set_angle(&mut self, angle: f32) {
        self.angle = angle;
        let turn = Mat4::from_rotation_z(-angle.to_radians());
        for inst in &mut self.scene.sky {
            let mut turned = Instance::new(
                inst.model.clone(),
                turn * Mat4::from_scale(Vec3::splat(SCALE)),
            );
            turned.sky = true;
            turned.tint = inst.tint;
            *inst = turned;
        }
    }
}

/// What a frame draws: the world and the player's view, or the skills
/// menu's sky while it is up.
pub fn frame_parts<'a>(
    stats: &'a Option<StatsView>,
    menu: Option<crate::items::Menu>,
    scene: &'a Scene,
    camera: Camera,
) -> (&'a Scene, Camera) {
    match stats {
        Some(v) if menu == Some(crate::items::Menu::Skills) => (v.scene(), v.camera()),
        _ => (scene, camera),
    }
}

impl Engine {
    /// Builds the skills menu's sky. None when its model can't be loaded.
    pub fn open_stats_view(&mut self) -> Option<()> {
        let path = STARS.to_string();
        self.models
            .load_all(&mut self.renderer, &self.vfs, &[path.clone()]);
        let model = self.models.get(&path)?;
        let mut inst = Instance::new(model, Mat4::IDENTITY);
        inst.sky = true;
        inst.tint = Vec4::ONE;
        let dome_path = DOME.to_string();
        self.models
            .load_all(&mut self.renderer, &self.vfs, &[dome_path.clone()]);
        let mut dome = Instance::new(self.models.get(&dome_path)?, Mat4::IDENTITY);
        dome.sky = true;
        dome.tint = Vec4::ONE;
        let sky = |sky: Instance| Scene {
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
        let scene = sky(inst);
        let perks = sky(dome);
        self.stats_view = Some(StatsView {
            scene,
            perks,
            in_perks: false,
            angle: 0.0,
        });
        Some(())
    }

    pub fn close_stats_view(&mut self) {
        self.stats_view = None;
    }
}
