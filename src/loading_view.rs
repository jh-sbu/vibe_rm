//! The loading screen's scene, behind the loading menu (CommonLibSSE's "Mist
//! Menu"): the chosen screen's model (`NNAM`) at the origin, scaled
//! (`SNAM`), turned (`RNAM`) and moved (`XNAM`), seen from its camera path
//! (`MOD2`: the `CameraPath` sequence moving `CameraPathNode`), in the
//! interface's fog particles over black, lit as the developer cell
//! `LoadScreenTesting` lights its model, and fading in and out with the
//! movie. The player turns the model with a mouse button held, within
//! `ONAM`. What is inferred rather than established is in
//! `known_ambiguities/loading-screen.md`.

use glam::{Mat4, Vec3};

use crate::engine::Engine;
use crate::load_screens::LoadScreen;
use crate::render::particles::{ParticleBatch, ParticleState};
use crate::render::{Camera, Environment, GpuLight, Instance, Scene};

/// `STAT INTMenuFogParticles`, read as the mist.
const MIST: &str = "meshes/interface/intmenufogparticles.nif#world";
/// The camera path a screen without one uses: the most common one (208 of
/// the 374 installed screens).
const DEFAULT_CAMERA: &str = "meshes/cameras/lscamerapanzoominsmall.nif";
/// The node the camera rides.
const CAMERA_NODE: &str = "CameraPathNode";
/// The camera's vertical field of view, degrees (not in the data).
const FOV: f32 = 50.0;
/// How far the model turns per pixel of mouse movement, degrees.
const TURN_PER_PIXEL: f32 = 0.4;

/// `LoadScreenTesting`'s lights: position, radius, colour (0–255).
const LIGHTS: [([f32; 3], f32, [f32; 3]); 3] = [
    ([160.0, -160.0, 128.0], 1500.0, [247.0, 245.0, 210.0]),
    ([-160.0, 96.0, 160.0], 1024.0, [179.0, 208.0, 204.0]),
    ([-128.0, -160.0, -96.0], 1024.0, [255.0, 255.0, 255.0]),
];

pub struct LoadingView {
    scene: Scene,
    /// The camera path: its model's animation, the sequence and channel
    /// moving the camera node, and the sequence's length.
    path: Option<(
        std::sync::Arc<crate::world::loader::ModelAnim>,
        usize,
        usize,
        f32,
    )>,
    clock: f32,
    camera: Camera,
    /// The model's and the mist's instances in `scene.dynamic`.
    model: Option<usize>,
    mist_instance: Option<usize>,
    /// The model's placement before the player's turn.
    model_base: Mat4,
    /// The player's turn about Z, degrees, and its range.
    turn: f32,
    turn_range: (f32, f32),
    mist: Vec<ParticleState>,
}

impl LoadingView {
    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn camera(&self) -> Camera {
        let c = &self.camera;
        Camera {
            position: c.position,
            yaw: c.yaw,
            pitch: c.pitch,
            fov_y: c.fov_y,
        }
    }

    /// Turns the model by a mouse movement (pixels), within its range.
    pub fn turn(&mut self, dx: f32) {
        let (lo, hi) = self.turn_range;
        self.turn = (self.turn + dx * TURN_PER_PIXEL).clamp(lo, hi);
    }

    /// How far the scene shows (0 black, 1 full), as the movie fades.
    pub fn set_shown(&mut self, shown: f32) {
        self.scene.post.fade = [0.0, 0.0, 0.0, 1.0 - shown.clamp(0.0, 1.0)];
    }

    /// The camera moves along its path; the mist drifts.
    pub fn update(&mut self, dt: f32) {
        self.clock += dt;
        if let Some((anim, si, ci, length)) = &self.path {
            let t = self.clock % length.max(1e-3);
            let ch = &anim.sequences[*si].channels[*ci];
            let pos = ch.sample(t, Mat4::IDENTITY).w_axis.truncate();
            self.camera.position = pos;
            // The model stands at the origin, ahead of the path along +Y.
            self.camera.yaw = 0.0;
            self.camera.pitch = 0.0;
        }
        if let Some(model) = self.model.and_then(|i| self.scene.dynamic.get_mut(i)) {
            model.transform = Mat4::from_rotation_z(-self.turn.to_radians()) * self.model_base;
        }
        self.update_mist(dt);
    }

    fn update_mist(&mut self, dt: f32) {
        let Some(mist) = self.mist_instance.and_then(|i| self.scene.dynamic.get(i)) else {
            return;
        };
        let forward = self.camera.forward();
        let right = forward.cross(Vec3::Z).normalize_or(Vec3::X);
        let up = right.cross(forward);
        let model_from_world = glam::Mat3::from_mat4(mist.transform).inverse();
        let mut batches = Vec::new();
        for (state, sys) in self.mist.iter_mut().zip(&mist.model.particles) {
            // Its emission is in its `AutoPlay` sequence.
            let desc: &crate::render::particles::ParticleDesc =
                sys.desc.sequences.get("autoplay").map_or(&sys.desc, |d| d);
            state.step(desc, dt, model_from_world);
            let mut vertices = Vec::new();
            state.quads(desc, mist.transform, right, up, &mut vertices);
            if !vertices.is_empty() {
                batches.push(ParticleBatch {
                    material: sys.material.clone(),
                    vertices,
                    center: mist.transform.transform_point3(desc.bound_center),
                });
            }
        }
        self.scene.particles = batches;
    }
}

/// The loading screen's scene and camera while its screen is up.
pub fn frame_parts(view: &Option<LoadingView>) -> Option<(&Scene, Camera)> {
    view.as_ref().map(|v| (v.scene(), v.camera()))
}

impl Engine {
    /// Builds the scene for a screen (none: the mist alone).
    pub fn open_loading_view(&mut self, screen: Option<&LoadScreen>) {
        let model_path = screen
            .and_then(|s| s.model)
            .and_then(|m| self.lo.get(m))
            .and_then(|r| crate::world::records::model_path(&r))
            .map(|p| format!("{}#world", crate::world::records::mesh_path(&p)));
        let camera_path = screen
            .and_then(|s| s.camera.as_deref())
            .map(crate::world::records::mesh_path)
            .unwrap_or_else(|| DEFAULT_CAMERA.into());
        let camera_key = format!("{camera_path}#world");
        let mut paths = vec![MIST.to_string(), camera_key.clone()];
        paths.extend(model_path.clone());
        self.models.load_all(&mut self.renderer, &self.vfs, &paths);

        let model_base = screen.map_or(Mat4::IDENTITY, |s| {
            let r = s.rotation.map(|d| (d as f32).to_radians());
            Mat4::from_scale_rotation_translation(
                Vec3::splat(s.scale),
                crate::world::records::rotation_from_euler(Vec3::new(r[0], r[1], r[2])),
                s.offset,
            )
        });
        let mut dynamic = Vec::new();
        let mut model = None;
        match model_path.as_deref().and_then(|p| self.models.get(p)) {
            Some(m) => {
                model = Some(dynamic.len());
                let mut inst = Instance::new(m, model_base);
                // Every light reaches it.
                inst.lights = [0, 1, 2, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF];
                dynamic.push(inst);
            }
            None => {
                if let Some(p) = &model_path {
                    log::warn!("loading screen model {p} didn't load");
                }
            }
        }
        let mut mist = Vec::new();
        let mut mist_instance = None;
        if let Some(m) = self.models.get(MIST) {
            mist = (0..m.particles.len())
                .map(|i| ParticleState::new(0x10AD_5C2E ^ i as u32))
                .collect();
            mist_instance = Some(dynamic.len());
            dynamic.push(Instance::new(m, Mat4::IDENTITY));
        }
        let path = self.models.anim(&camera_key).and_then(|a| {
            let si = a.sequences.iter().position(|s| s.name == "CameraPath")?;
            let seq = &a.sequences[si];
            let ci = seq.channels.iter().position(|c| c.node == CAMERA_NODE)?;
            let length = seq.stop - seq.start;
            Some((a, si, ci, length))
        });
        if path.is_none() {
            log::warn!("camera path {camera_path} has no {CAMERA_NODE} sequence");
        }
        let lights = LIGHTS
            .iter()
            .map(|&(p, radius, c)| GpuLight {
                pos_radius: [p[0], p[1], p[2], radius],
                color: [c[0] / 255.0, c[1] / 255.0, c[2] / 255.0, 1.0],
            })
            .collect();
        let scene = Scene {
            env: Environment {
                sun_color: Vec3::ZERO,
                ambient: Vec3::splat(0.05),
                fog_far: 1.0e7,
                clear_color: Vec3::ZERO,
                sky: false,
                ..Default::default()
            },
            dynamic,
            lights,
            post: self.load_screen_post(),
            ..Default::default()
        };
        let mut view = LoadingView {
            scene,
            path,
            clock: 0.0,
            camera: Camera {
                position: Vec3::new(0.0, -300.0, 16.0),
                yaw: 0.0,
                pitch: 0.0,
                fov_y: FOV.to_radians(),
            },
            model,
            mist_instance,
            model_base,
            turn: 0.0,
            turn_range: screen.map_or((0.0, 0.0), |s| (s.turn.0 as f32, s.turn.1 as f32)),
            mist,
        };
        // The mist has been drifting a while when the screen comes up.
        for _ in 0..60 {
            view.update_mist(0.05);
        }
        view.update(0.0);
        view.set_shown(0.0);
        self.loading_view = Some(view);
    }

    pub fn close_loading_view(&mut self) {
        self.loading_view = None;
    }

    /// `LoadScreenIS` (the default object `LSIS`)'s saturation, brightness,
    /// contrast and tint.
    fn load_screen_post(&mut self) -> crate::render::post::PostEffect {
        let imgs = self
            .lo
            .default_objects()
            .get(b"LSIS")
            .copied()
            .and_then(|f| self.imgs_of(f))
            .unwrap_or_default();
        crate::render::post::PostEffect {
            saturation: imgs.cinematic[0],
            brightness: imgs.cinematic[1],
            contrast: imgs.cinematic[2],
            tint: [imgs.tint[1], imgs.tint[2], imgs.tint[3], imgs.tint[0]],
            ..Default::default()
        }
    }
}
