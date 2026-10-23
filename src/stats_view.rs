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
/// The sky of a skill's perks.
const DOME: &str = "meshes/interface/intperkskydome.nif";
/// Where the model's `CameraPosition` node puts the camera.
const CAMERA_AT: Vec3 = Vec3::new(0.0, -198.345, -123.842);

pub struct StatsView {
    /// The ring's sky, if its model loaded.
    stars: Option<Scene>,
    /// The skydome a skill's perks sit in, drawn instead while a tree is
    /// open, if its model loaded.
    dome: Option<Scene>,
    in_perks: bool,
    /// The ring's turn the stars were last turned to, degrees.
    angle: f32,
}

impl StatsView {
    /// Whether the sky drawn is the dome: a tree is open and the dome
    /// loaded, or the stars didn't.
    fn shows_dome(&self) -> bool {
        self.dome.is_some() && (self.in_perks || self.stars.is_none())
    }

    /// The scene to draw.
    pub fn scene(&self) -> &Scene {
        let scene = if self.shows_dome() {
            &self.dome
        } else {
            &self.stars
        };
        scene.as_ref().expect("a stats view has a sky")
    }

    /// Whether a skill's tree is open (its sky) rather than the ring.
    pub fn set_perks(&mut self, open: bool) {
        self.in_perks = open;
    }

    pub fn camera(&self) -> Camera {
        if self.shows_dome() {
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
        if angle == self.angle {
            return;
        }
        self.angle = angle;
        let Some(stars) = &mut self.stars else {
            return;
        };
        let turn = Mat4::from_rotation_z(-angle.to_radians());
        for inst in &mut stars.sky {
            let mut turned = Instance::new(inst.model.clone(), turn);
            turned.sky = true;
            turned.tint = inst.tint;
            *inst = turned;
        }
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
    /// A sky model as the only thing in a scene. None when it can't be
    /// loaded.
    fn sky_scene(&mut self, path: &str) -> Option<Scene> {
        let path = path.to_string();
        self.models
            .load_all(&mut self.renderer, &self.vfs, std::slice::from_ref(&path));
        let mut sky = Instance::new(self.models.get(&path)?, Mat4::IDENTITY);
        sky.sky = true;
        sky.tint = Vec4::ONE;
        Some(Scene {
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
        })
    }

    /// Builds the skills menu's sky: either model alone stands in for both.
    /// None when neither can be loaded.
    pub fn open_stats_view(&mut self) -> Option<()> {
        let stars = self.sky_scene(STARS);
        let dome = self.sky_scene(DOME);
        if stars.is_none() && dome.is_none() {
            return None;
        }
        self.stats_view = Some(StatsView {
            stars,
            dome,
            in_perks: false,
            angle: 0.0,
        });
        Some(())
    }

    pub fn close_stats_view(&mut self) {
        self.stats_view = None;
    }
}
