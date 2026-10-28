//! wgpu-based forward renderer.

pub mod dds;
pub mod decal;
pub mod distance;
pub mod membrane;
pub mod model;
pub mod particles;
pub mod post;
pub mod precip;
pub mod shadow;
pub mod sky;
pub mod terrain;
pub mod texture;
pub mod water;

use std::collections::HashMap;
use std::sync::Arc;

use glam::{Mat4, Vec3, Vec4};
use wgpu::util::DeviceExt;

use model::{BlendMode, CpuModel, MaterialDesc, ShaderKind, SkinVertex, Vertex};
use texture::{GpuTexture, TextureCache};

pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
pub const MAX_LIGHTS: usize = 4096;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct FrameUniform {
    view_proj: [[f32; 4]; 4],
    cam_pos: [f32; 4],
    sun_dir: [f32; 4],
    sun_color: [f32; 4],
    ambient: [f32; 4],
    fog_near_color: [f32; 4],
    fog_far_color: [f32; 4],
    fog: [f32; 4],
    misc: [f32; 4],
    amb: [[f32; 4]; 6],
    lod_clip: [f32; 4],
    /// Sun shadow cascades: light view-projections, view depths where each ends,
    /// (enabled, texel size, ...), and the camera's forward axis.
    shadow_vp: [[[f32; 4]; 4]; shadow::CASCADES],
    shadow_splits: [f32; 4],
    shadow_params: [f32; 4],
    cam_fwd: [f32; 4],
    /// The wind's velocity (units a second), where grass starts fading and
    /// over how far.
    wind: [f32; 4],
    /// Clip space back to the world (decals find surfaces from depth).
    inv_view_proj: [[f32; 4]; 4],
    /// The target's size in pixels.
    screen: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuLight {
    pub pos_radius: [f32; 4],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct MaterialUniform {
    uv: [f32; 4],
    emissive: [f32; 4],
    specular: [f32; 4],
    params: [f32; 4],
    flags: [u32; 4],
    falloff: [f32; 4],
    tint: [f32; 4],
    /// A billboard's model-space origin and mode + 1 (0: not one).
    billboard: [f32; 4],
    /// Environment map scale, has a cube map, has an environment mask; and a
    /// soft effect's falloff depth (0 for none).
    env: [f32; 4],
    /// A decal's parallax scale and passes, and whether it's a hard-edged cutout.
    decal: [f32; 4],
    membrane_fill: [f32; 4],
    membrane_edge: [f32; 4],
    membrane_params: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct InstanceData {
    model: [[f32; 4]; 4],
    lights: [u32; 4],
    /// Colour and opacity the instance is drawn with ([`Instance::tint`]).
    tint: [f32; 4],
    /// How much fog it takes (0 for the sky's), unused.
    params: [f32; 4],
}

/// One grass blade's instance ([`crate::world::grass::Blade`]).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GrassInstance {
    /// Position, scale across.
    pos_scale: [f32; 4],
    /// Up axis, turn about it.
    up_yaw: [f32; 4],
    /// Tint, wave period.
    tint_wave: [f32; 4],
    /// Scale up.
    scale_up: [f32; 4],
}

/// One grass's blades over a cell, in an instance buffer of their own.
pub struct GrassBatch {
    pub model: Arc<GpuModel>,
    buf: wgpu::Buffer,
    count: u32,
    pub center: Vec3,
    pub radius: f32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SkinInstanceData {
    palette_base: u32,
    lights: [u32; 4],
    /// The actor's skin tone and 1 ([`ActorInstance::skin_tone`]), or 0s.
    skin_tone: [f32; 4],
    /// How far toward its thin models it's drawn: 1 - [`ActorInstance::weight`].
    thin: f32,
    fade: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PipelineKey {
    pub skinned: bool,
    pub blend: BlendMode,
    pub double_sided: bool,
    pub z_write: bool,
    pub z_test: bool,
    pub blend_op: u8,
    pub depth_func: u8,
    /// Grass, instanced by [`GrassInstance`].
    pub grass: bool,
    /// Drawn over the surface it lies on (`sf1::DECAL` / `DYNAMIC_DECAL`):
    /// its depth biased towards the camera.
    pub decal: bool,
}

pub struct GpuMaterial {
    pub bind_group: wgpu::BindGroup,
    membrane_uniform: Option<(wgpu::Buffer, MaterialUniform)>,
    pub key: PipelineKey,
    /// Cut out by alpha testing (casts shadows through its texture).
    pub alpha_test: bool,
    /// Controllers animating its uniform, from its values at rest.
    anim: Option<Box<MaterialClock>>,
    /// Turned to the camera in the vertex shader (casts no shadow).
    pub billboard: bool,
    /// Fades where it meets the scene behind it: needs the opaque scene's depth
    /// copied before it's drawn.
    pub soft: bool,
    pub skin: bool,
}

/// An animated material's controllers and what they animate.
struct MaterialClock {
    anim: Arc<model::MaterialAnim>,
    /// Its values at rest.
    base: MaterialUniform,
    buf: wgpu::Buffer,
    /// What it was made from (for one on a clock of its own).
    desc: MaterialDesc,
    /// When it started (an effect's, all its controllers running from
    /// then); else the started controllers run on the renderer's clock.
    start: Option<f32>,
    /// Set only on a placed object's private material copy. Retains a
    /// completed sequence's terminal sample until the next selection.
    sequence: MaterialSequenceClock,
}

#[derive(Default)]
struct MaterialSequenceClock {
    selected: std::sync::Mutex<Vec<(String, f32, f32)>>,
}

impl MaterialSequenceClock {
    fn select(&self, name: &str, time: f32) {
        *self.selected.lock().unwrap() = vec![(name.to_owned(), time, 1.0)];
    }

    fn apply(&self, anim: &model::MaterialAnim, u: MaterialUniform) -> MaterialUniform {
        let selected = self.selected.lock().unwrap();
        let mut result = u;
        let mut total = 0.0f32;
        for (name, t, w) in selected.iter().filter(|(_, _, w)| *w > 0.0) {
            let sample = u.sequenced(anim, name, *t);
            let fraction = *w / (total + *w);
            total += *w;
            result = result.blended(sample, fraction);
        }
        result
    }
}

impl MaterialClock {
    fn at(&self, now: f32) -> MaterialUniform {
        let u = match self.start {
            Some(s) => self.base.animated(&self.anim, now - s, true),
            None => self.base.animated(&self.anim, now, false),
        };
        self.sequence.apply(&self.anim, u)
    }
}

impl MaterialUniform {
    fn blended(mut self, other: Self, weight: f32) -> Self {
        let blend =
            |a: [f32; 4], b: [f32; 4]| std::array::from_fn(|i| a[i] + (b[i] - a[i]) * weight);
        self.uv = blend(self.uv, other.uv);
        self.emissive = blend(self.emissive, other.emissive);
        self.specular = blend(self.specular, other.specular);
        self.params = blend(self.params, other.params);
        self.falloff = blend(self.falloff, other.falloff);
        self.tint = blend(self.tint, other.tint);
        self.env = blend(self.env, other.env);
        self.decal = blend(self.decal, other.decal);
        self
    }

    /// The values with a material's controllers applied at `t` seconds (only
    /// those started unless `all`).
    fn animated(&self, anim: &model::MaterialAnim, t: f32, all: bool) -> MaterialUniform {
        self.apply_channels(anim.channels.iter().filter(|c| all || c.started), t, true)
    }

    fn sequenced(&self, anim: &model::MaterialAnim, name: &str, t: f32) -> MaterialUniform {
        let Some(seq) = anim
            .sequences
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(name))
        else {
            return *self;
        };
        // `t` is already a selected sequence's key time. Apply its frequency
        // and cycle once, in AnimatedObject, as for transform channels.
        self.apply_channels(seq.channels.iter(), t, false)
    }

    fn apply_channels<'a>(
        &self,
        channels: impl Iterator<Item = &'a model::MaterialChannel>,
        t: f32,
        standalone: bool,
    ) -> MaterialUniform {
        let mut u = *self;
        for c in channels {
            let Some(v) = c
                .keys
                .sample(if standalone { c.timing.key_time(t) } else { t })
            else {
                continue;
            };
            let f = v.x;
            if !f.is_finite() || (c.color && !v.is_finite()) {
                continue;
            }
            if c.alpha_test {
                // The controller keys share NiAlphaProperty's byte range.
                // Keep the negative sentinel when alpha testing is disabled.
                if u.params[1] >= 0.0 && f.is_finite() {
                    u.params[1] = (f / 255.0).clamp(0.0, 1.0);
                }
                continue;
            }
            match (c.lighting, c.color, c.variable) {
                // Effect shaders.
                (false, false, 0) => u.emissive[3] = f,
                // Falloff angles are keyed in degrees, kept as cosines.
                (false, false, 1) => u.falloff[0] = f.to_radians().cos(),
                (false, false, 2) => u.falloff[1] = f.to_radians().cos(),
                (false, false, 3) => u.falloff[2] = f,
                (false, false, 4) => u.falloff[3] = f,
                (false, false, 5) => u.params[0] = f,
                (false, false, 6) => u.uv[0] = f,
                (false, false, 7) => u.uv[2] = f,
                (false, false, 8) => u.uv[1] = f,
                (false, false, 9) => u.uv[3] = f,
                (false, true, 0) => u.emissive[..3].copy_from_slice(&v.to_array()),
                // Lighting shaders.
                (true, false, 8) => u.env[0] = f,
                (true, false, 9) => u.specular[3] = f,
                (true, false, 11) => u.emissive[3] = f,
                (true, false, 12) => u.params[0] = f,
                (true, false, 20) => u.uv[0] = f,
                (true, false, 21) => u.uv[2] = f,
                (true, false, 22) => u.uv[1] = f,
                (true, false, 23) => u.uv[3] = f,
                (true, true, 1) => u.emissive[..3].copy_from_slice(&v.to_array()),
                _ => {}
            }
        }
        u
    }
}

#[cfg(test)]
mod effect_shader_tests;

#[cfg(test)]
mod alpha_test_tests {
    use super::*;

    fn animation(started: bool) -> model::MaterialAnim {
        model::MaterialAnim {
            sequences: Vec::new(),
            channels: vec![model::MaterialChannel {
                timing: nif::anim::Timing {
                    flags: 8 | 4,
                    frequency: 2.0,
                    phase: 0.0,
                    start: 0.0,
                    stop: 2.0,
                },
                keys: nif::anim::ValueKeys {
                    interpolation: 1,
                    tangents: Vec::new(),
                    keys: vec![(0.0, Vec3::ZERO), (2.0, Vec3::new(255.0, 0.0, 0.0))],
                },
                variable: 0,
                color: false,
                lighting: false,
                alpha_test: true,
                started,
            }],
        }
    }

    #[test]
    fn precipitation_map_serves_a_camera_until_it_leaves_the_margin() {
        let reach = 700.0;
        let at = |p: Vec3, age| PrecipMap {
            cascade: shadow::precipitation_map(p, reach + shadow::OCCLUSION_MARGIN),
            centre: p,
            age,
        };
        let start = Vec3::new(5.0, 6.0, 7.0);
        let map = at(start, 0);
        assert!(map.serves(start, reach));
        let half = shadow::OCCLUSION_MARGIN * 0.5;
        assert!(map.serves(start + Vec3::new(half - 1.0, -half + 1.0, 0.0), reach));
        assert!(!map.serves(start + Vec3::new(half + 1.0, 0.0, 0.0), reach));
        assert!(!map.serves(start + Vec3::new(0.0, 0.0, -half - 1.0), reach));
        // It's redrawn now and then, and when the falling box grows or shrinks.
        assert!(!at(start, PrecipMap::MAX_AGE).serves(start, reach));
        assert!(!map.serves(start, reach + shadow::OCCLUSION_MARGIN));
        assert!(!map.serves(start, reach * 0.5));
    }

    #[test]
    fn constant_material_keys_preserve_uv_colour_and_cutout_steps() {
        let mut base = <MaterialUniform as bytemuck::Zeroable>::zeroed();
        base.params[1] = 0.25;
        let mut anim = animation(true);
        let alpha = &mut anim.channels[0];
        alpha.keys.interpolation = 5;
        let mut uv = alpha.clone();
        uv.alpha_test = false;
        uv.variable = 6;
        uv.keys.keys = vec![(0.0, Vec3::ZERO), (2.0, Vec3::splat(0.5))];
        let mut colour = uv.clone();
        colour.variable = 0;
        colour.color = true;
        colour.keys.keys = vec![(0.0, Vec3::X), (2.0, Vec3::Y)];
        anim.channels.extend([uv, colour]);
        // Controller frequency 2 maps clock 0.5 to the key midpoint 1.0.
        for (clock, expected_uv, expected_alpha, expected_colour) in [
            (-1.0, 0.0, 0.0, Vec3::X),
            (0.49, 0.0, 0.0, Vec3::X),
            (0.5, 0.5, 1.0, Vec3::Y),
            (0.51, 0.5, 1.0, Vec3::Y),
            (1.0, 0.5, 1.0, Vec3::Y),
            (20.0, 0.5, 1.0, Vec3::Y),
        ] {
            let u = base.animated(&anim, clock, false);
            assert_eq!(u.uv[0], expected_uv);
            assert_eq!(u.params[1], expected_alpha);
            assert_eq!(&u.emissive[..3], &expected_colour.to_array());
        }
        anim.sequences.push(model::MaterialSequence {
            name: "Atlas".into(),
            channels: anim.channels.clone(),
        });
        // Sequence time has already been mapped; frequency must not apply twice.
        assert_eq!(base.sequenced(&anim, "Atlas", 0.75).uv[0], 0.0);
        assert_eq!(base.sequenced(&anim, "Atlas", 1.0).uv[0], 0.5);
    }

    #[test]
    fn quadratic_material_keys_reach_standalone_and_sequence_uniforms() {
        let mut base = <MaterialUniform as bytemuck::Zeroable>::zeroed();
        base.params[1] = 0.25;
        let mut anim = animation(true);
        let alpha = &mut anim.channels[0];
        alpha.keys.interpolation = 2;
        alpha.keys.tangents = vec![(Vec3::ZERO, Vec3::ZERO); 2];
        let mut uv = alpha.clone();
        uv.alpha_test = false;
        uv.variable = 6;
        uv.keys.keys = vec![(0.0, Vec3::ZERO), (2.0, Vec3::splat(0.5))];
        let mut colour = uv.clone();
        colour.color = true;
        colour.variable = 0;
        colour.keys.keys = vec![(0.0, Vec3::X), (2.0, Vec3::Y)];
        anim.channels.extend([uv, colour]);
        anim.sequences.push(model::MaterialSequence {
            name: "Ease".into(),
            channels: anim.channels.clone(),
        });
        // Clock 0.25 maps to key 0.5 (u=0.25), once per standalone clock.
        for uniform in [
            base.animated(&anim, 0.25, false),
            base.sequenced(&anim, "Ease", 0.5),
        ] {
            assert_eq!(uniform.params[1], 0.15625);
            assert_eq!(uniform.uv[0], 0.078125);
            assert_eq!(&uniform.emissive[..3], &[0.84375, 0.15625, 0.0]);
        }
    }

    #[test]
    fn alpha_threshold_uses_controller_clock_and_byte_range() {
        let mut base = <MaterialUniform as bytemuck::Zeroable>::zeroed();
        base.params = [0.7, 0.25, 0.0, 0.0];
        let a = animation(true);
        let u = base.animated(&a, 0.5, false);
        assert_eq!(u.params[1], 0.5);
        assert_eq!(u.params[0], 0.7);
        assert_eq!(u.emissive, base.emissive);
        assert_eq!(base.animated(&a, 10.0, false).params[1], 1.0);
        let a = animation(false);
        assert_eq!(base.animated(&a, 0.5, false).params[1], 0.25);
        assert_eq!(base.animated(&a, 0.5, true).params[1], 0.5);

        let mut a = animation(true);
        let mut shader = a.channels[0].clone();
        shader.alpha_test = false;
        shader.variable = 6; // effect-shader U offset
        shader.keys.keys = vec![(0.0, Vec3::new(0.75, 0.0, 0.0))];
        a.channels.insert(0, shader);
        let u = base.animated(&a, 0.5, false);
        assert_eq!(u.uv[0], 0.75);
        assert_eq!(u.params[1], 0.5);
    }

    #[test]
    fn alpha_threshold_preserves_disabled_and_rejects_nonfinite() {
        let mut base = <MaterialUniform as bytemuck::Zeroable>::zeroed();
        let mut a = animation(true);
        base.params[1] = -1.0;
        assert_eq!(base.animated(&a, 0.5, true).params[1], -1.0);
        base.params[1] = 0.25;
        for v in [f32::NAN, f32::INFINITY] {
            a.channels[0].keys.keys = vec![(0.0, Vec3::splat(v))];
            assert_eq!(base.animated(&a, 0.5, true).params[1], 0.25);
        }
        a.channels[0].keys.keys = vec![(0.0, Vec3::splat(-100.0))];
        assert_eq!(base.animated(&a, 0.5, true).params[1], 0.0);
    }

    #[test]
    fn standalone_reverse_material_channels_respect_nonzero_start() {
        let base = <MaterialUniform as bytemuck::Zeroable>::zeroed();
        let mut anim = animation(true);
        let alpha = &mut anim.channels[0];
        alpha.timing.flags = 8 | 2;
        alpha.timing.phase = 1.0;
        alpha.timing.start = 3.0;
        alpha.timing.stop = 5.0;
        alpha.keys.keys = vec![(3.0, Vec3::ZERO), (5.0, Vec3::X * 255.0)];
        let mut uv = alpha.clone();
        uv.alpha_test = false;
        uv.variable = 6;
        uv.keys.keys = vec![(3.0, Vec3::ZERO), (5.0, Vec3::X)];
        let mut color = uv.clone();
        color.color = true;
        color.variable = 0;
        color.keys.keys = vec![(3.0, Vec3::ZERO), (5.0, Vec3::ONE)];
        anim.channels.extend([uv, color]);
        for (time, expected) in [
            (1.0, 0.0),
            (1.5, 0.5),
            (2.0, 1.0),
            (2.5, 0.5),
            (3.0, 0.0),
            (3.5, 0.5),
            (9.5, 0.5),
        ] {
            let u = base.animated(&anim, time, false);
            assert_eq!(u.params[1], expected, "alpha at {time}");
            assert_eq!(u.uv[0], expected, "U offset at {time}");
            assert_eq!(u.emissive[..3], [expected; 3], "colour at {time}");
        }
    }

    #[test]
    fn sequence_material_copies_keep_independent_clocks_and_terminal_samples() {
        let mut base = <MaterialUniform as bytemuck::Zeroable>::zeroed();
        base.params[1] = 0.25;
        let mut anim = animation(true);
        let mut alpha = anim.channels.remove(0);
        // Sequence time is already mapped; this unrelated standalone clock
        // must neither multiply it nor clamp it to the controller's range.
        alpha.timing.frequency = 7.0;
        alpha.timing.phase = 9.0;
        alpha.timing.start = 10.0;
        alpha.timing.stop = 20.0;
        let mut uv = alpha.clone();
        uv.alpha_test = false;
        uv.variable = 6;
        uv.keys.keys = vec![(0.0, Vec3::ZERO), (2.0, Vec3::X)];
        anim.sequences.push(model::MaterialSequence {
            name: "Fade".into(),
            channels: vec![alpha],
        });
        anim.channels.push(uv);
        let anim = Arc::new(anim); // Shared animation data, independent cursors.
        let first = MaterialSequenceClock::default();
        let second = MaterialSequenceClock::default();
        let standalone = base.animated(&anim, 0.1, false);
        assert_eq!(standalone.uv[0], 1.0);
        first.select("Fade", 0.5);
        assert_eq!(first.apply(&anim, standalone).params[1], 0.25);
        assert_eq!(second.apply(&anim, standalone).params[1], 0.25);
        second.select("fade", 1.5);
        assert_eq!(second.apply(&anim, standalone).params[1], 0.75);
        assert_eq!(first.apply(&anim, standalone).params[1], 0.25);
        first.select("Fade", 2.0);
        assert_eq!(first.apply(&anim, standalone).params[1], 1.0);
        // No new selection after sequence completion: terminal state holds.
        assert_eq!(
            first
                .apply(&anim, base.animated(&anim, 100.0, false))
                .params[1],
            1.0
        );
        // Selecting a sequence without this material's channel restores its
        // base + standalone values, rather than leaking the previous sequence.
        first.select("Open", 0.0);
        assert_eq!(first.apply(&anim, standalone).params[1], 0.25);
        assert_eq!(first.apply(&anim, standalone).uv[0], 1.0);
    }

    #[test]
    fn reverse_sequence_samples_material_channels_on_both_legs() {
        let base = <MaterialUniform as bytemuck::Zeroable>::zeroed();
        let mut anim = animation(false);
        let mut alpha = anim.channels.remove(0);
        alpha.keys.keys = vec![(3.0, Vec3::ZERO), (5.0, Vec3::X * 255.0)];
        // Property timing must not remap the sequence's sampled key time.
        alpha.timing.frequency = 7.0;
        alpha.timing.phase = 20.0;
        let mut uv = alpha.clone();
        uv.alpha_test = false;
        uv.variable = 6;
        uv.keys.keys = vec![(3.0, Vec3::ZERO), (5.0, Vec3::X)];
        let mut color = uv.clone();
        color.color = true;
        color.variable = 0;
        color.keys.keys = vec![(3.0, Vec3::ZERO), (5.0, Vec3::ONE)];
        anim.sequences.push(model::MaterialSequence {
            name: "Idle".into(),
            channels: vec![alpha, uv, color],
        });
        let seq = model::Sequence {
            name: "Idle".into(),
            cycle: 1,
            start: 3.0,
            stop: 5.0,
            frequency: 2.0,
            channels: Vec::new(),
        };
        let clock = MaterialSequenceClock::default();
        let mut phase = seq.start;
        for expected in [0.5, 1.0, 0.5, 0.0, 0.5] {
            let (time, done) = seq.advance(&mut phase, 0.5);
            assert!(!done);
            clock.select(&seq.name, time);
            let u = clock.apply(&anim, base);
            assert_eq!(u.params[1], expected);
            assert_eq!(u.uv[0], expected);
            assert_eq!(u.emissive[..3], [expected; 3]);
        }
    }
}

pub struct GpuPart {
    pub cpu_vertices: Arc<[Vertex]>,
    pub cpu_indices: Arc<[u32]>,
    pub vbuf: wgpu::Buffer,
    pub ibuf: wgpu::Buffer,
    pub index_count: u32,
    pub material: Arc<GpuMaterial>,
    pub bound_center: Vec3,
    pub bound_radius: f32,
}

/// Something drawn with an object material: a model's part, or particle
/// quads (a range of the frame's particle vertices).
#[derive(Clone)]
enum Draw<'a> {
    Part(&'a GpuPart),
    Quads(&'a GpuMaterial, std::ops::Range<u32>),
}

impl Draw<'_> {
    fn material(&self) -> &GpuMaterial {
        match self {
            Draw::Part(p) => &p.material,
            Draw::Quads(m, _) => m,
        }
    }
}

pub struct GpuSkinnedPart {
    pub cpu_vertices: Arc<[SkinVertex]>,
    pub cpu_indices: Arc<[u32]>,
    pub vbuf: wgpu::Buffer,
    pub ibuf: wgpu::Buffer,
    pub index_count: u32,
    pub material: Arc<GpuMaterial>,
    pub bone_names: Vec<String>,
    pub skin_to_bone: Vec<Mat4>,
    pub name: String,
}

pub struct GpuModel {
    pub path: String,
    pub parts: Vec<GpuPart>,
    pub skinned: Vec<GpuSkinnedPart>,
    pub particles: Vec<particles::GpuParticles>,
    pub bound_center: Vec3,
    pub bound_radius: f32,
}

impl GpuModel {
    pub fn set_material_samples(&self, samples: &[(String, f32, f32)]) {
        for part in &self.parts {
            if let Some(clock) = &part.material.anim {
                if !clock.anim.sequences.is_empty() {
                    *clock.sequence.selected.lock().unwrap() = samples.to_vec();
                }
            }
        }
    }

    /// Select the material sample on a model created by `sequence_model`.
    /// Renderer animation uploads this state before ordinary and shadow draws.
    pub fn set_material_sequence(&self, name: &str, time: f32) {
        for part in &self.parts {
            if let Some(clock) = &part.material.anim {
                if !clock.anim.sequences.is_empty() {
                    clock.sequence.select(name, time);
                }
            }
        }
    }
}

pub struct Instance {
    pub fade_class: distance::Class,
    pub ref_id: u32,
    /// The reference's base form (0 when it has none).
    pub base: u32,
    pub hidden: bool,
    /// Multiplies its colour and opacity (sky statics fading with the
    /// weather); drawn only while the opacity is above 0.
    pub tint: Vec4,
    /// Part of the sky (an aurora): placed relative to the camera, always
    /// drawn and not fogged.
    pub sky: bool,
    /// Its model's particle systems running (started when first near).
    pub particles: Vec<particles::ParticleState>,
    pub model: Arc<GpuModel>,
    pub transform: Mat4,
    pub lights: [u16; 8],
    pub world_center: Vec3,
    pub world_radius: f32,
}

impl Instance {
    pub fn new(model: Arc<GpuModel>, transform: Mat4) -> Self {
        let world_center = transform.transform_point3(model.bound_center);
        let scale = transform
            .x_axis
            .truncate()
            .length()
            .max(transform.y_axis.truncate().length())
            .max(transform.z_axis.truncate().length());
        let world_radius = model.bound_radius * scale;
        Instance {
            fade_class: Default::default(),
            ref_id: 0,
            base: 0,
            hidden: false,
            tint: Vec4::ONE,
            sky: false,
            particles: Vec::new(),
            model,
            transform,
            lights: [0xFFFF; 8],
            world_center,
            world_radius,
        }
    }
}

/// Global lighting environment for a frame.
#[derive(Clone, Copy, Debug)]
pub struct Environment {
    pub sun_dir: Vec3,
    pub sun_color: Vec3,
    pub ambient: Vec3,
    pub fog_near_color: Vec3,
    pub fog_far_color: Vec3,
    pub fog_near: f32,
    pub fog_far: f32,
    pub fog_power: f32,
    pub fog_max: f32,
    pub clear_color: Vec3,
    /// Directional ambient (X+, X-, Y+, Y-, Z+, Z-); replaces `ambient` when set.
    pub dalc: Option<[Vec3; 6]>,
    /// Draw the weather sky instead of the clear colour.
    pub sky: bool,
}

impl Default for Environment {
    fn default() -> Self {
        Environment {
            sun_dir: Vec3::new(0.3, 0.4, 0.8).normalize(),
            sun_color: Vec3::splat(0.6),
            ambient: Vec3::splat(0.3),
            fog_near_color: Vec3::splat(0.5),
            fog_far_color: Vec3::splat(0.5),
            fog_near: 0.0,
            fog_far: 500_000.0,
            fog_power: 1.0,
            fog_max: 0.0,
            clear_color: Vec3::new(0.4, 0.5, 0.6),
            dalc: None,
            sky: false,
        }
    }
}

/// One skinned shape of an actor, with its bones mapped onto the actor's skeleton.
pub struct ActorMesh {
    pub model: Arc<GpuModel>,
    pub part: usize,
    /// Mesh bone index -> skeleton bone index.
    pub bone_map: Vec<usize>,
}

/// A point light hanging from an actor's bone.
#[derive(Debug, Clone, Copy)]
pub struct HeldLight {
    pub bone: usize,
    /// Position in the bone's space.
    pub offset: Vec3,
    pub radius: f32,
    pub color: Vec3,
}

/// A posed, skinned character.
pub struct ActorInstance {
    pub meshes: Vec<ActorMesh>,
    /// Rigid (non-skinned) models attached to skeleton bones, e.g. weapons.
    pub attachments: Vec<(Arc<GpuModel>, usize, Mat4)>,
    /// Rigid equipment (weapons, shields, torches), the same way.
    pub equipment: Vec<(Arc<GpuModel>, usize, Mat4)>,
    /// A light carried (a torch's flame).
    pub held_light: Option<HeldLight>,
    pub transform: Mat4,
    /// Model-space bone matrices for the current pose.
    pub pose: Vec<Mat4>,
    pub lights: [u16; 8],
    pub radius: f32,
    /// Cast a shadow but aren't drawn (the player's body seen from inside it).
    pub shadow_only: bool,
    /// The colour its skin-tint shapes take in place of their model's own.
    pub skin_tone: Option<Vec3>,
    /// Its weight (`NAM7` / 100): 0 draws its models' thin (`_0`) shapes, 1
    /// their heavy (`_1`) ones, those without a thin one always as they are.
    pub weight: f32,
}

impl ActorInstance {
    /// World position of the light it carries.
    pub fn held_light_pos(&self) -> Option<Vec3> {
        let l = self.held_light?;
        Some(
            (self.transform * self.pose.get(l.bone).copied().unwrap_or(Mat4::IDENTITY))
                .transform_point3(l.offset),
        )
    }

    /// Its bounding sphere in the world: about its bones as posed (a ragdoll
    /// lies wherever it fell, its transform staying where it died), reaching
    /// past them for the flesh and gear on them, and at least `radius`.
    pub fn bounds(&self) -> (Vec3, f32) {
        if self.pose.is_empty() {
            let center = self.transform.transform_point3(Vec3::new(0.0, 0.0, 64.0));
            return (center, self.radius);
        }
        let (lo, hi) = self.pose.iter().fold(
            (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            |(lo, hi), m| (lo.min(m.w_axis.truncate()), hi.max(m.w_axis.truncate())),
        );
        let scale = self.transform.x_axis.truncate().length();
        let center = self.transform.transform_point3((lo + hi) * 0.5);
        let reach = ((hi - lo).length() * 0.5 + 40.0) * scale;
        (center, reach.max(self.radius))
    }
}

/// Identifies a loaded cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CellKey {
    Interior(esp::FormId),
    Exterior(i32, i32),
}

/// Render data belonging to one loaded cell.
#[derive(Default)]
pub struct RenderCell {
    pub instances: Vec<Instance>,
    pub actors: Vec<ActorInstance>,
    pub terrain: Vec<terrain::TerrainChunk>,
    pub water: Vec<water::WaterPlane>,
    pub grass: Vec<GrassBatch>,
    pub decals: Vec<decal::GpuDecal>,
}

#[derive(Default)]
pub struct Scene {
    pub cells: HashMap<CellKey, RenderCell>,
    /// The player's body, outside any cell.
    pub player: Option<ActorInstance>,
    /// Distant LOD instances.
    pub lod: Vec<Instance>,
    /// Moving instances outside any cell (arrows), rebuilt each frame.
    pub dynamic: Vec<Instance>,
    /// The sky's models about the camera (auroras).
    pub sky: Vec<Instance>,
    /// The worldspace's far references, loaded with it rather than with
    /// their cells (sky statics).
    pub far: Vec<Instance>,
    pub tree_lod: Vec<Instance>,
    /// Particles to draw this frame.
    pub particles: Vec<particles::ParticleBatch>,
    /// Decals on actors' bodies, placed by this frame's poses.
    pub actor_decals: Vec<decal::GpuDecal>,
    /// Posed membrane overlays, rebuilt from live visual effects.
    pub membranes: Vec<ActorInstance>,
    /// World-space XY rectangle (min x, min y, max x, max y) where LOD is hidden.
    pub lod_clip: [f32; 4],
    pub lights: Vec<GpuLight>,
    pub env: Environment,
    /// Image space effects over the frame.
    pub post: post::PostEffect,
}

impl Scene {
    /// Debug: instances whose bounding sphere intersects a ray, nearest first.
    pub fn pick(&self, origin: Vec3, dir: Vec3) -> Vec<(f32, String)> {
        let mut hits = Vec::new();
        for i in self.instances() {
            let oc = i.world_center - origin;
            let t = oc.dot(dir);
            let d2 = oc.length_squared() - t * t;
            if t > 0.0 && d2 < i.world_radius * i.world_radius {
                hits.push((t, i.model.path.clone()));
            }
        }
        for a in self.actors().filter(|a| !a.shadow_only) {
            let (center, radius) = a.bounds();
            let oc = center - origin;
            let t = oc.dot(dir);
            let d2 = oc.length_squared() - t * t;
            if t > 0.0 && d2 < radius * radius {
                let names: Vec<&str> = a
                    .meshes
                    .iter()
                    .map(|m| m.model.skinned[m.part].name.as_str())
                    .collect();
                hits.push((t, format!("actor: {names:?}")));
            }
        }
        hits.sort_by(|a, b| a.0.total_cmp(&b.0));
        hits
    }

    /// The cells' actors and the player.
    pub fn actors(&self) -> impl Iterator<Item = &ActorInstance> {
        self.cells
            .values()
            .flat_map(|c| c.actors.iter())
            .chain(self.player.iter())
    }

    pub fn instances(&self) -> impl Iterator<Item = &Instance> {
        self.cells
            .values()
            .flat_map(|c| c.instances.iter())
            .chain(self.lod.iter())
            .chain(self.dynamic.iter())
            .chain(self.sky.iter())
            .chain(self.far.iter())
            .chain(self.tree_lod.iter())
    }
    pub fn instance_count(&self) -> usize {
        self.cells.values().map(|c| c.instances.len()).sum()
    }

    /// Assign up to 8 point lights to each instance based on bounding-sphere overlap.
    pub fn assign_lights(&mut self) {
        let lights = &self.lights;
        for inst in self.cells.values_mut().flat_map(|c| c.instances.iter_mut()) {
            let mut cands: Vec<(f32, u16)> = Vec::new();
            for (i, l) in lights.iter().enumerate() {
                let p = Vec3::new(l.pos_radius[0], l.pos_radius[1], l.pos_radius[2]);
                let r = l.pos_radius[3];
                let d = p.distance(inst.world_center);
                if d < r + inst.world_radius {
                    let lum = l.color[0] + l.color[1] + l.color[2];
                    // Prefer bright, close lights.
                    let score = lum * (1.0 - (d / (r + inst.world_radius)).min(1.0));
                    cands.push((-score, i as u16));
                }
            }
            cands.sort_by(|a, b| a.0.total_cmp(&b.0));
            inst.lights = [0xFFFF; 8];
            for (slot, (_, i)) in cands.iter().take(8).enumerate() {
                inst.lights[slot] = *i;
            }
        }
        for a in self
            .cells
            .values_mut()
            .flat_map(|c| c.actors.iter_mut())
            .chain(self.player.iter_mut())
        {
            let (center, radius) = a.bounds();
            a.lights = pick_lights(lights, center, radius);
        }
        for d in self.cells.values_mut().flat_map(|c| c.decals.iter_mut()) {
            d.lights = pick_lights(lights, d.center, d.radius);
        }
    }
}

impl Scene {
    /// Re-pick lights for what is lit, or was lit, by the lights from `first` on
    /// (lights that move: carried torches).
    pub fn assign_moving_lights(&mut self, first: usize) {
        let lights = &self.lights;
        let moving = &lights[first.min(lights.len())..];
        let touched = |center: Vec3, radius: f32, current: &[u16; 8]| {
            current.iter().any(|&i| i != 0xFFFF && i as usize >= first)
                || moving.iter().any(|l| {
                    Vec3::new(l.pos_radius[0], l.pos_radius[1], l.pos_radius[2]).distance(center)
                        < l.pos_radius[3] + radius
                })
        };
        for inst in self.cells.values_mut().flat_map(|c| c.instances.iter_mut()) {
            if touched(inst.world_center, inst.world_radius, &inst.lights) {
                inst.lights = pick_lights(lights, inst.world_center, inst.world_radius);
            }
        }
        for a in self
            .cells
            .values_mut()
            .flat_map(|c| c.actors.iter_mut())
            .chain(self.player.iter_mut())
        {
            let (center, radius) = a.bounds();
            if touched(center, radius, &a.lights) {
                a.lights = pick_lights(lights, center, radius);
            }
        }
    }
}

pub(crate) fn pick_lights(lights: &[GpuLight], center: Vec3, radius: f32) -> [u16; 8] {
    let mut cands: Vec<(f32, u16)> = Vec::new();
    for (i, l) in lights.iter().enumerate() {
        let p = Vec3::new(l.pos_radius[0], l.pos_radius[1], l.pos_radius[2]);
        let r = l.pos_radius[3];
        let d = p.distance(center);
        if d < r + radius {
            let lum = l.color[0] + l.color[1] + l.color[2];
            cands.push((-(lum * (1.0 - (d / (r + radius)).min(1.0))), i as u16));
        }
    }
    cands.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out = [0xFFFF; 8];
    for (slot, (_, i)) in cands.iter().take(8).enumerate() {
        out[slot] = *i;
    }
    out
}

pub struct Camera {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub fov_y: f32,
}

impl Camera {
    pub fn forward(&self) -> Vec3 {
        Vec3::new(
            self.yaw.sin() * self.pitch.cos(),
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
        )
    }
    pub fn right(&self) -> Vec3 {
        Vec3::new(self.yaw.cos(), -self.yaw.sin(), 0.0)
    }
    pub fn view(&self) -> Mat4 {
        glam::camera::rh::view::look_to_mat4(self.position, self.forward(), Vec3::Z)
    }
    pub fn proj(&self, aspect: f32) -> Mat4 {
        glam::camera::rh::proj::directx::perspective_infinite_reverse(self.fov_y, aspect, 5.0)
    }
}

struct Frustum {
    planes: [Vec4; 5],
}

impl Frustum {
    fn from_matrix(m: Mat4) -> Self {
        let r0 = m.row(0);
        let r1 = m.row(1);
        let r2 = m.row(2);
        let r3 = m.row(3);
        let norm = |p: Vec4| p / p.truncate().length();
        // Reverse-Z infinite: near plane is z <= w  (r3 - r2), no far plane.
        Frustum {
            planes: [
                norm(r3 + r0),
                norm(r3 - r0),
                norm(r3 + r1),
                norm(r3 - r1),
                norm(r3 - r2),
            ],
        }
    }
    fn sphere_visible(&self, c: Vec3, r: f32) -> bool {
        self.planes.iter().all(|p| p.truncate().dot(c) + p.w >= -r)
    }
}

/// The top-down depth map that keeps rain and snow from falling under roofs,
/// and where and when it was drawn.
struct PrecipMap {
    cascade: shadow::Cascade,
    centre: Vec3,
    /// Frames since it was drawn.
    age: u32,
}

impl PrecipMap {
    /// Frames before it's drawn again, whatever the camera does (what the
    /// world loads and moves in it shows up by then).
    const MAX_AGE: u32 = 120;

    /// Whether a map drawn about `self.centre` to `radius` still serves a
    /// camera at `camera` needing `reach` units about it: it recentres once
    /// the camera has used up the margin (or the reach grew).
    fn serves(&self, camera: Vec3, reach: f32) -> bool {
        let moved = camera - self.centre;
        let radius = reach + shadow::OCCLUSION_MARGIN;
        self.age < Self::MAX_AGE
            && self.cascade.radius >= reach + shadow::OCCLUSION_MARGIN * 0.5
            && self.cascade.radius <= radius * 1.25
            && moved.x.abs().max(moved.y.abs()) < shadow::OCCLUSION_MARGIN * 0.5
            && moved.z.abs() < shadow::OCCLUSION_MARGIN * 0.5
    }
}

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub color_format: wgpu::TextureFormat,
    pub gamma: f32,
    pub distances: distance::Distances,
    pub width: u32,
    pub height: u32,
    depth_tex: wgpu::Texture,
    depth_view: wgpu::TextureView,
    /// A copy of the opaque scene's depth for soft effects to sample while
    /// the depth buffer itself is written ([`Renderer::render`]), and the
    /// group 2 binding of it every object pipeline's layout has.
    soft_depth: wgpu::Texture,
    soft_bg: wgpu::BindGroup,
    depth_bgl: wgpu::BindGroupLayout,
    shadows: shadow::ShadowMaps,
    decals: decal::DecalPipeline,
    decal_buf: wgpu::Buffer,
    decal_cap: usize,
    /// The cascades as last rendered (far ones are refreshed every few frames), and
    /// frames rendered with shadows.
    shadow_cascades: Option<[shadow::Cascade; shadow::CASCADES]>,
    /// The precipitation occlusion map as last drawn, while rain or snow falls.
    precip_map: Option<PrecipMap>,
    /// Sun shadows on (`VRM_NO_SHADOWS` turns them off; console `tsh` toggles).
    pub shadows_enabled: bool,
    shadow_frame: u64,
    shadow_instance_buf: wgpu::Buffer,
    shadow_instance_cap: usize,
    frame_buf: wgpu::Buffer,
    light_buf: wgpu::Buffer,
    frame_bg: wgpu::BindGroup,
    frame_bgl: wgpu::BindGroupLayout,
    palette_buf: wgpu::Buffer,
    palette_cap: usize,
    skin_instance_buf: wgpu::Buffer,
    skin_instance_cap: usize,
    material_bgl: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    shader: wgpu::ShaderModule,
    pipelines: HashMap<PipelineKey, wgpu::RenderPipeline>,
    pub(crate) sampler: wgpu::Sampler,
    pub(crate) terrain: terrain::TerrainPipeline,
    pub(crate) water: water::WaterPipeline,
    instance_buf: wgpu::Buffer,
    instance_cap: usize,
    pub textures: TextureCache,
    pub(crate) white: Arc<GpuTexture>,
    pub(crate) flat_normal: Arc<GpuTexture>,
    pub(crate) black: Arc<GpuTexture>,
    /// Bound as the cube map of materials without environment mapping.
    black_cube: Arc<GpuTexture>,
    pub stats: FrameStats,
    pub sky: sky::SkyRenderer,
    /// Rain and snow (drawn under the sky).
    pub precip: precip::PrecipRenderer,
    post: post::PostPass,
    /// Seconds since start, for animated effects.
    pub time: f32,
    /// The wind's velocity (units a second), swaying grass.
    pub wind: glam::Vec2,
    /// Cycles grass waves have run, at a rate rising with the wind.
    pub grass_clock: f32,
    /// Materials whose controllers animate them, updated each frame.
    animated: std::sync::Mutex<Vec<std::sync::Weak<GpuMaterial>>>,
    /// The frame's particle quads, and how many vertices it holds.
    particle_vbuf: wgpu::Buffer,
    particle_cap: usize,
}

#[derive(Default, Debug, Clone, Copy)]
pub struct FrameStats {
    pub draws: u32,
    pub instances: u32,
    pub culled: u32,
}

impl Renderer {
    pub fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        color_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("object"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/object.wgsl").into()),
        });
        let frame_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("frame"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });
        let tex_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let material_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("material"),
            entries: &[
                tex_entry(0),
                tex_entry(1),
                tex_entry(2),
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
                tex_entry(6),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let depth_bgl = decal::depth_layout(&device);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("object"),
            bind_group_layouts: &[Some(&frame_bgl), Some(&material_bgl), Some(&depth_bgl)],
            immediate_size: 0,
        });
        let frame_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame"),
            size: std::mem::size_of::<FrameUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let light_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lights"),
            size: (MAX_LIGHTS * std::mem::size_of::<GpuLight>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let palette_cap = 4096;
        let palette_buf = Self::make_palette(&device, palette_cap);
        let shadows = shadow::ShadowMaps::new(
            &device,
            &material_bgl,
            std::mem::size_of::<terrain::TerrainVertex>() as u64,
        );
        let frame_bg = Self::make_frame_bg(
            &device,
            &frame_bgl,
            &frame_buf,
            &light_buf,
            &palette_buf,
            &shadows,
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("main"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 16,
            ..Default::default()
        });
        let instance_cap = 16384;
        let instance_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("instances"),
            size: (instance_cap * std::mem::size_of::<InstanceData>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let white = Arc::new(texture::solid(
            &device,
            &queue,
            [255, 255, 255, 255],
            "white",
        ));
        let flat_normal = Arc::new(texture::solid(
            &device,
            &queue,
            [128, 128, 255, 0],
            "flat_normal",
        ));
        let black = Arc::new(texture::solid(&device, &queue, [0, 0, 0, 255], "black"));
        let black_cube = Arc::new(texture::solid_cube(
            &device,
            &queue,
            [0, 0, 0, 255],
            "black cube",
        ));
        let (depth_tex, depth_view) = Self::make_depth(&device, width, height);
        let (soft_depth, soft_view) = Self::make_depth_copy(&device, width, height);
        let soft_bg = Self::bind_depth(&device, &depth_bgl, &soft_view);
        let skin_instance_buf = Self::make_vbuf(
            &device,
            1024 * std::mem::size_of::<SkinInstanceData>(),
            "skin instances",
        );
        let terrain = terrain::TerrainPipeline::new(&device, &frame_bgl, color_format);
        let sky = sky::SkyRenderer::new(&device, color_format);
        let post = post::PostPass::new(&device, color_format);
        let decals = decal::DecalPipeline::new(
            &device,
            &shader,
            &frame_bgl,
            &material_bgl,
            color_format,
            depth_bgl.clone(),
            &depth_view,
        );
        let water = water::WaterPipeline::new(&device, &frame_bgl, color_format);
        let precip = precip::PrecipRenderer::new(
            &device,
            &frame_bgl,
            color_format,
            &shadows.layers[shadow::OCCLUSION],
        );
        let shadow_instance_buf = Self::make_vbuf(
            &device,
            16384 * std::mem::size_of::<InstanceData>(),
            "shadow instances",
        );
        Renderer {
            decals,
            decal_buf: Self::make_vbuf(
                &device,
                64 * std::mem::size_of::<InstanceData>(),
                "decal instances",
            ),
            decal_cap: 64,
            water,
            sky,
            precip,
            post,
            time: 0.0,
            wind: glam::Vec2::ZERO,
            grass_clock: 0.0,
            animated: Default::default(),
            particle_vbuf: Self::make_vbuf(
                &device,
                4096 * std::mem::size_of::<model::Vertex>(),
                "particles",
            ),
            particle_cap: 4096,
            terrain,
            device,
            queue,
            color_format,
            gamma: 1.0,
            distances: Default::default(),
            width,
            height,
            depth_tex,
            depth_view,
            soft_depth,
            soft_bg,
            depth_bgl,
            shadows,
            shadow_cascades: None,
            precip_map: None,
            shadows_enabled: std::env::var_os("VRM_NO_SHADOWS").is_none(),
            shadow_frame: 0,
            shadow_instance_buf,
            shadow_instance_cap: 16384,
            frame_buf,
            light_buf,
            frame_bg,
            frame_bgl,
            palette_buf,
            palette_cap,
            skin_instance_buf,
            skin_instance_cap: 1024,
            material_bgl,
            pipeline_layout,
            shader,
            pipelines: HashMap::new(),
            sampler,
            instance_buf,
            instance_cap,
            textures: TextureCache::default(),
            white,
            flat_normal,
            black,
            black_cube,
            stats: FrameStats::default(),
        }
    }

    fn make_palette(device: &wgpu::Device, cap: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bone palette"),
            size: (cap * 64) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn make_vbuf(device: &wgpu::Device, size: usize, label: &str) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: size as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn make_frame_bg(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        frame: &wgpu::Buffer,
        lights: &wgpu::Buffer,
        palette: &wgpu::Buffer,
        shadows: &shadow::ShadowMaps,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frame"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: frame.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: lights.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: palette.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&shadows.array_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&shadows.sampler),
                },
            ],
        })
    }

    fn make_depth(device: &wgpu::Device, w: u32, h: u32) -> (wgpu::Texture, wgpu::TextureView) {
        let t = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size: wgpu::Extent3d {
                width: w.max(1),
                height: h.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let v = t.create_view(&Default::default());
        (t, v)
    }

    /// The texture [`Renderer::render`] copies the opaque scene's depth to.
    fn make_depth_copy(
        device: &wgpu::Device,
        w: u32,
        h: u32,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let t = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth copy"),
            size: wgpu::Extent3d {
                width: w.max(1),
                height: h.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let v = t.create_view(&Default::default());
        (t, v)
    }

    fn bind_depth(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene depth"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(view),
            }],
        })
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.width = w.max(1);
        self.height = h.max(1);
        (self.depth_tex, self.depth_view) = Self::make_depth(&self.device, self.width, self.height);
        self.decals.rebind(&self.device, &self.depth_view);
        let (copy, view) = Self::make_depth_copy(&self.device, self.width, self.height);
        self.soft_bg = Self::bind_depth(&self.device, &self.depth_bgl, &view);
        self.soft_depth = copy;
    }

    fn pipeline(&mut self, key: PipelineKey) -> &wgpu::RenderPipeline {
        if !self.pipelines.contains_key(&key) {
            let p = self.create_pipeline(key);
            self.pipelines.insert(key, p);
        }
        &self.pipelines[&key]
    }

    fn create_pipeline(&self, key: PipelineKey) -> wgpu::RenderPipeline {
        let vertex_attrs = wgpu::vertex_attr_array![
            0 => Float32x3, 1 => Float32x3, 2 => Float32x3, 3 => Float32x3, 4 => Float32x2, 5 => Float32x4
        ];
        let inst_attrs = wgpu::vertex_attr_array![
            6 => Float32x4, 7 => Float32x4, 8 => Float32x4, 9 => Float32x4, 10 => Uint32x4,
            15 => Float32x4, 16 => Float32x4
        ];
        let skin_attrs = wgpu::vertex_attr_array![
            0 => Float32x3, 1 => Float32x3, 2 => Float32x3, 3 => Float32x3, 4 => Float32x2, 5 => Float32x4,
            11 => Uint32x4, 12 => Float32x4, 6 => Float32x3, 7 => Float32x3
        ];
        let skin_inst_attrs = wgpu::vertex_attr_array![
            13 => Uint32, 14 => Uint32x4, 15 => Float32x4, 10 => Float32, 16 => Float32
        ];
        let grass_attrs = wgpu::vertex_attr_array![6 => Float32x4, 7 => Float32x4, 8 => Float32x4, 9 => Float32x4];
        let static_buffers = [
            Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Vertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &vertex_attrs,
            }),
            Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<InstanceData>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &inst_attrs,
            }),
        ];
        let skinned_buffers = [
            Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<SkinVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &skin_attrs,
            }),
            Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<SkinInstanceData>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &skin_inst_attrs,
            }),
        ];
        let grass_buffers = [
            static_buffers[0].clone(),
            Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<GrassInstance>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &grass_attrs,
            }),
        ];
        let blend = match key.blend {
            BlendMode::Opaque => None,
            BlendMode::Blend(src, dst) => Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: blend_factor(src),
                    dst_factor: blend_factor(dst),
                    operation: match key.blend_op {
                        2 => wgpu::BlendOperation::Subtract,
                        3 => wgpu::BlendOperation::ReverseSubtract,
                        4 => wgpu::BlendOperation::Min,
                        5 => wgpu::BlendOperation::Max,
                        _ => wgpu::BlendOperation::Add,
                    },
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
            }),
        };
        self.device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("object"),
                layout: Some(&self.pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: Some(if key.skinned {
                        "vs_skinned"
                    } else if key.grass {
                        "vs_grass"
                    } else {
                        "vs_main"
                    }),
                    compilation_options: Default::default(),
                    buffers: if key.skinned {
                        &skinned_buffers
                    } else if key.grass {
                        &grass_buffers
                    } else {
                        &static_buffers
                    },
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: if key.double_sided {
                        None
                    } else {
                        Some(wgpu::Face::Back)
                    },
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(key.z_write),
                    depth_compare: Some(match key.depth_func {
                        1 => wgpu::CompareFunction::Never,
                        2 => wgpu::CompareFunction::Greater,
                        3 => wgpu::CompareFunction::Equal,
                        4 => wgpu::CompareFunction::GreaterEqual,
                        5 => wgpu::CompareFunction::Less,
                        6 => wgpu::CompareFunction::NotEqual,
                        7 => wgpu::CompareFunction::LessEqual,
                        8 => wgpu::CompareFunction::Always,
                        _ if key.z_test => wgpu::CompareFunction::GreaterEqual,
                        _ => wgpu::CompareFunction::Always,
                    }),
                    stencil: Default::default(),
                    // Reversed depth: nearer is greater.
                    bias: if key.decal {
                        wgpu::DepthBiasState {
                            constant: 8,
                            slope_scale: 2.0,
                            clamp: 0.0,
                        }
                    } else {
                        Default::default()
                    },
                }),
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: self.color_format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
    }

    /// Upload a parsed DDS into the texture cache.
    pub fn add_texture(&mut self, path: &str, d: Option<&dds::Dds>) {
        let t = d.map(|d| Arc::new(texture::upload_dds(&self.device, &self.queue, d, path)));
        self.textures.insert(path.to_owned(), t);
    }

    /// The texture at `path`, or `fallback` if it's missing or isn't viewed
    /// like it (a cube map named in a 2D slot, or the reverse).
    fn texture_or(
        &self,
        path: &Option<String>,
        fallback: &Arc<GpuTexture>,
    ) -> (Arc<GpuTexture>, bool) {
        match path.as_ref().and_then(|p| self.textures.get(p)).flatten() {
            Some(t) if t.dimension == fallback.dimension => (t, true),
            _ => (fallback.clone(), false),
        }
    }

    pub fn create_material(&self, m: &MaterialDesc) -> GpuMaterial {
        let (diffuse, _) = self.texture_or(&m.diffuse, &self.white);
        let (normal, has_normal) = self.texture_or(&m.normal, &self.flat_normal);
        let (glow, has_glow) = self.texture_or(&m.glow, &self.black);
        let (mut env, mut has_env) = self.texture_or(&m.env, &self.black_cube);
        if std::env::var_os("VRM_NO_ENVMAP").is_some() {
            (env, has_env) = (self.black_cube.clone(), false);
        }
        let (env_mask, has_env_mask) = self.texture_or(&m.env_mask, &self.white);
        // The opaque scene's depth is copied before the blended draws: the
        // opaque ones (and skinned ones, drawn with them) see none of it.
        let soft_depth =
            if m.blend == BlendMode::Opaque || std::env::var_os("VRM_NO_SOFT_EFFECTS").is_some() {
                0.0
            } else {
                m.soft_depth
            };
        let mut u = MaterialUniform {
            uv: [m.uv_offset.x, m.uv_offset.y, m.uv_scale.x, m.uv_scale.y],
            emissive: m.emissive.to_array(),
            specular: [m.specular.x, m.specular.y, m.specular.z, m.glossiness],
            params: [
                m.alpha,
                m.alpha_test.unwrap_or(-1.0),
                if has_normal && m.kind == ShaderKind::Lit {
                    1.0
                } else {
                    0.0
                },
                if has_glow && m.shader_type != 4 {
                    1.0
                } else if has_glow {
                    2.0
                } else {
                    0.0
                },
            ],
            flags: [
                m.flags1,
                m.flags2,
                if m.kind == ShaderKind::Membrane {
                    3
                } else if m.kind == ShaderKind::Effect {
                    1
                } else if m.lod {
                    2
                } else {
                    0
                },
                m.shader_type,
            ],
            falloff: m.falloff.to_array(),
            tint: m.tint.extend(1.0).to_array(),
            billboard: m
                .billboard
                .map_or([0.0; 4], |(p, mode)| p.extend(mode as f32 + 1.0).to_array()),
            env: [
                m.env_scale,
                has_env as u32 as f32,
                has_env_mask as u32 as f32,
                soft_depth,
            ],
            decal: [m.parallax.x, m.parallax.y, m.hard_edge as u32 as f32, 0.0],
            membrane_fill: m.membrane.map_or([0.0; 4], |m| m.fill),
            membrane_edge: m.membrane.map_or([0.0; 4], |m| m.edge),
            membrane_params: m.membrane.map_or([0.0; 4], |m| m.params),
        };
        if let Some(sample) = m.membrane {
            u.flags[3] = sample.flags;
        }
        let ubuf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("material"),
                contents: bytemuck::bytes_of(&u),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("material"),
            layout: &self.material_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&diffuse.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&normal.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&glow.view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: ubuf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&env.view),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::TextureView(&env_mask.view),
                },
            ],
        });
        let key = PipelineKey {
            skinned: false,
            blend: m.blend,
            double_sided: m.double_sided,
            z_write: m.z_write || m.blend == BlendMode::Opaque,
            z_test: m.z_test || m.blend == BlendMode::Opaque,
            blend_op: m.blend_op,
            depth_func: m.depth_func,
            decal: m.flags1 & (nif::sf1::DECAL | nif::sf1::DYNAMIC_DECAL) != 0,
            grass: m.grass,
        };
        GpuMaterial {
            bind_group,
            membrane_uniform: m.membrane.map(|_| (ubuf.clone(), u)),
            key,
            alpha_test: m.alpha_test.is_some(),
            anim: m.anim.clone().map(|anim| {
                Box::new(MaterialClock {
                    anim,
                    base: u,
                    buf: ubuf,
                    desc: m.clone(),
                    start: None,
                    sequence: MaterialSequenceClock::default(),
                })
            }),
            billboard: m.billboard.is_some(),
            soft: soft_depth > 0.0,
            skin: matches!(m.shader_type, 4 | 5),
        }
    }

    pub fn update_membrane_material(&self, material: &GpuMaterial, desc: &MaterialDesc) {
        let (Some((buf, base)), Some(sample)) = (&material.membrane_uniform, desc.membrane) else {
            return;
        };
        let mut uniform = *base;
        uniform.uv = [
            desc.uv_offset.x,
            desc.uv_offset.y,
            desc.uv_scale.x,
            desc.uv_scale.y,
        ];
        uniform.membrane_fill = sample.fill;
        uniform.membrane_edge = sample.edge;
        uniform.membrane_params = sample.params;
        uniform.flags[3] = sample.flags;
        self.queue
            .write_buffer(buf, 0, bytemuck::bytes_of(&uniform));
    }

    /// One grass's blades over a cell, for [`RenderCell::grass`].
    pub fn build_grass(
        &self,
        model: Arc<GpuModel>,
        blades: &[crate::world::grass::Blade],
    ) -> Option<GrassBatch> {
        if blades.is_empty() {
            return None;
        }
        let data: Vec<GrassInstance> = blades
            .iter()
            .map(|b| GrassInstance {
                pos_scale: b.pos.extend(b.scale.x).to_array(),
                up_yaw: b.up.extend(b.yaw).to_array(),
                tint_wave: b.tint.extend(b.wave).to_array(),
                scale_up: [b.scale.y, 0.0, 0.0, 0.0],
            })
            .collect();
        let (lo, hi) = blades.iter().fold(
            (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            |(lo, hi), b| (lo.min(b.pos), hi.max(b.pos)),
        );
        let reach = model.bound_center.length() + model.bound_radius * 1.5;
        let buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("grass"),
                contents: bytemuck::cast_slice(&data),
                usage: wgpu::BufferUsages::VERTEX,
            });
        Some(GrassBatch {
            model,
            buf,
            count: data.len() as u32,
            center: (lo + hi) * 0.5,
            radius: (hi - lo).length() * 0.5 + reach,
        })
    }

    /// A material for a model's part, kept for animating if it has controllers.
    fn shared_material(&self, m: &MaterialDesc, skinned: bool) -> Arc<GpuMaterial> {
        let mut mat = if skinned && m.soft_depth > 0.0 {
            self.create_material(&MaterialDesc {
                soft_depth: 0.0,
                ..m.clone()
            })
        } else {
            self.create_material(m)
        };
        mat.key.skinned = skinned;
        let mat = Arc::new(mat);
        if mat.anim.as_ref().is_some_and(|a| a.anim.started()) {
            self.animated.lock().unwrap().push(Arc::downgrade(&mat));
        }
        mat
    }

    /// Share geometry but give sequence-controlled materials to this reference
    /// alone. Static materials and standalone-only animations stay shared.
    pub fn sequence_model(&self, model: &Arc<GpuModel>) -> Arc<GpuModel> {
        if !model.parts.iter().any(|p| {
            p.material
                .anim
                .as_ref()
                .is_some_and(|a| !a.anim.sequences.is_empty())
        }) {
            return model.clone();
        }
        let copy = |m: &Arc<GpuMaterial>| {
            let Some(clock) = m.anim.as_ref().filter(|a| !a.anim.sequences.is_empty()) else {
                return m.clone();
            };
            let mut mat = self.create_material(&clock.desc);
            mat.key = m.key;
            let mat = Arc::new(mat);
            self.animated.lock().unwrap().push(Arc::downgrade(&mat));
            mat
        };
        Arc::new(GpuModel {
            path: model.path.clone(),
            parts: model
                .parts
                .iter()
                .map(|p| GpuPart {
                    cpu_vertices: p.cpu_vertices.clone(),
                    cpu_indices: p.cpu_indices.clone(),
                    vbuf: p.vbuf.clone(),
                    ibuf: p.ibuf.clone(),
                    index_count: p.index_count,
                    material: copy(&p.material),
                    bound_center: p.bound_center,
                    bound_radius: p.bound_radius,
                })
                .collect(),
            skinned: model
                .skinned
                .iter()
                .map(|p| GpuSkinnedPart {
                    cpu_vertices: p.cpu_vertices.clone(),
                    cpu_indices: p.cpu_indices.clone(),
                    vbuf: p.vbuf.clone(),
                    ibuf: p.ibuf.clone(),
                    index_count: p.index_count,
                    material: p.material.clone(),
                    bone_names: p.bone_names.clone(),
                    skin_to_bone: p.skin_to_bone.clone(),
                    name: p.name.clone(),
                })
                .collect(),
            particles: model
                .particles
                .iter()
                .map(|p| particles::GpuParticles {
                    desc: p.desc.clone(),
                    material: p.material.clone(),
                })
                .collect(),
            bound_center: model.bound_center,
            bound_radius: model.bound_radius,
        })
    }

    /// A model sharing `model`'s geometry, its animated materials on a clock
    /// of their own, `at` seconds in now, with all their controllers: an
    /// effect's.
    pub fn started_model(&self, model: &GpuModel, at: f32) -> GpuModel {
        let restart = |m: &Arc<GpuMaterial>| -> Arc<GpuMaterial> {
            let Some(clock) = &m.anim else {
                return m.clone();
            };
            let mut mat = self.create_material(&clock.desc);
            mat.key = m.key;
            if let Some(c) = mat.anim.as_mut() {
                c.start = Some(self.time - at);
                self.queue
                    .write_buffer(&c.buf, 0, bytemuck::bytes_of(&c.at(self.time)));
            }
            let mat = Arc::new(mat);
            self.animated.lock().unwrap().push(Arc::downgrade(&mat));
            mat
        };
        GpuModel {
            path: model.path.clone(),
            parts: model
                .parts
                .iter()
                .map(|p| GpuPart {
                    cpu_vertices: p.cpu_vertices.clone(),
                    cpu_indices: p.cpu_indices.clone(),
                    vbuf: p.vbuf.clone(),
                    ibuf: p.ibuf.clone(),
                    index_count: p.index_count,
                    material: restart(&p.material),
                    bound_center: p.bound_center,
                    bound_radius: p.bound_radius,
                })
                .collect(),
            skinned: Vec::new(),
            particles: model
                .particles
                .iter()
                .map(|p| particles::GpuParticles {
                    desc: p.desc.clone(),
                    material: p.material.clone(),
                })
                .collect(),
            bound_center: model.bound_center,
            bound_radius: model.bound_radius,
        }
    }

    /// Bring the animated materials still in use to the time now.
    fn animate_materials(&self) {
        let mut list = self.animated.lock().unwrap();
        list.retain(|w| {
            let Some(m) = w.upgrade() else { return false };
            if let Some(c) = &m.anim {
                self.queue
                    .write_buffer(&c.buf, 0, bytemuck::bytes_of(&c.at(self.time)));
            }
            true
        });
    }

    pub fn upload_model(&self, cpu: &CpuModel) -> GpuModel {
        let mut parts = Vec::with_capacity(cpu.meshes.len());
        for m in &cpu.meshes {
            let vbuf = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: bytemuck::cast_slice(&m.vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
            let ibuf = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: bytemuck::cast_slice(&m.indices),
                    usage: wgpu::BufferUsages::INDEX,
                });
            parts.push(GpuPart {
                cpu_vertices: m.vertices.clone().into(),
                cpu_indices: m.indices.clone().into(),
                vbuf,
                ibuf,
                index_count: m.indices.len() as u32,
                material: self.shared_material(&m.material, false),
                bound_center: m.bound_center,
                bound_radius: m.bound_radius,
            });
        }
        let mut skinned = Vec::with_capacity(cpu.skinned.len());
        for m in &cpu.skinned {
            let vbuf = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: bytemuck::cast_slice(&m.vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
            let ibuf = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: bytemuck::cast_slice(&m.indices),
                    usage: wgpu::BufferUsages::INDEX,
                });
            skinned.push(GpuSkinnedPart {
                cpu_vertices: m.vertices.clone().into(),
                cpu_indices: m.indices.clone().into(),
                vbuf,
                ibuf,
                index_count: m.indices.len() as u32,
                material: self.shared_material(&m.material, true),
                bone_names: m.bone_names.clone(),
                skin_to_bone: m.skin_to_bone.clone(),
                name: m.name.clone(),
            });
        }
        let particles = cpu
            .particles
            .iter()
            .map(|d| particles::GpuParticles {
                desc: d.clone(),
                material: self.shared_material(&d.material, false),
            })
            .collect();
        GpuModel {
            path: String::new(),
            parts,
            skinned,
            particles,
            bound_center: cpu.bound_center,
            bound_radius: cpu.bound_radius,
        }
    }

    pub fn render(&mut self, scene: &Scene, camera: &Camera, target: &wgpu::TextureView) {
        self.animate_materials();
        let aspect = self.width as f32 / self.height as f32;
        let view_proj = camera.proj(aspect) * camera.view();
        let frustum = Frustum::from_matrix(view_proj);
        let env = &scene.env;
        // The sun casts shadows outdoors.
        let shadowed = env.sky && self.shadows_enabled && self.distances.shadows > 0.;
        let fresh = shadow::cascades(camera, aspect, env.sun_dir, self.distances.shadows);
        // Near cascades follow every frame, far ones every 2nd / 4th (their shadows
        // barely change); each is sampled with the view it was rendered with.
        let mut update = [true; shadow::CASCADES];
        let mut cascades = fresh;
        if let (Some(last), true) = (self.shadow_cascades, shadowed) {
            for (i, u) in update.iter_mut().enumerate().skip(2) {
                let every = 1u64 << (i - 1);
                *u = self.shadow_frame % every == (i as u64 - 1) % every;
                if !*u {
                    cascades[i] = last[i];
                }
            }
        }
        if shadowed {
            self.shadow_cascades = Some(cascades);
            self.shadow_frame += 1;
        } else {
            self.shadow_cascades = None;
        }
        let (shadow_vp, shadow_splits, shadow_params) =
            shadow::frame_data(&cascades, shadowed, self.distances.shadows);
        // Rain and snow stay off what has a roof over it: while any falls under
        // the sky, a top-down depth map about the camera, redrawn as it moves
        // off-centre and now and then.
        let reach = if env.sky && std::env::var_os("VRM_NO_PRECIP_OCCLUSION").is_none() {
            self.precip.reach()
        } else {
            0.0
        };
        let mut occlusion_fresh = false;
        if reach > 0.0 {
            match &mut self.precip_map {
                Some(m) if m.serves(camera.position, reach) => m.age += 1,
                map => {
                    *map = Some(PrecipMap {
                        cascade: shadow::precipitation_map(
                            camera.position,
                            reach + shadow::OCCLUSION_MARGIN,
                        ),
                        centre: camera.position,
                        age: 0,
                    });
                    occlusion_fresh = true;
                }
            }
        } else {
            self.precip_map = None;
        }
        self.precip.set_occlusion(
            &self.queue,
            self.precip_map.as_ref().map(|m| {
                (
                    m.cascade.view_proj,
                    shadow::precipitation_range(m.cascade.radius),
                )
            }),
        );
        let fu = FrameUniform {
            view_proj: view_proj.to_cols_array_2d(),
            cam_pos: camera.position.extend(1.0).to_array(),
            sun_dir: env.sun_dir.normalize_or(Vec3::Z).extend(0.0).to_array(),
            sun_color: env.sun_color.extend(1.0).to_array(),
            ambient: env.ambient.extend(1.0).to_array(),
            fog_near_color: env.fog_near_color.extend(1.0).to_array(),
            fog_far_color: env.fog_far_color.extend(1.0).to_array(),
            fog: [env.fog_near, env.fog_far, env.fog_power, env.fog_max],
            misc: [
                self.time,
                scene.lights.len() as f32,
                self.distances.specular,
                self.grass_clock,
            ],
            amb: match env.dalc {
                Some(d) => {
                    let mut a = [[0f32; 4]; 6];
                    for i in 0..6 {
                        a[i] = d[i].extend(1.0).to_array();
                    }
                    a
                }
                None => [[0f32; 4]; 6],
            },
            lod_clip: scene.lod_clip,
            shadow_vp,
            shadow_splits,
            shadow_params,
            cam_fwd: camera.forward().extend(0.0).to_array(),
            wind: [
                self.wind.x,
                self.wind.y,
                self.distances.grass,
                crate::world::grass::FADE_RANGE,
            ],
            inv_view_proj: view_proj.inverse().to_cols_array_2d(),
            screen: [self.width as f32, self.height as f32, 0.0, 0.0],
        };
        self.queue
            .write_buffer(&self.frame_buf, 0, bytemuck::bytes_of(&fu));
        let nl = scene.lights.len().min(MAX_LIGHTS);
        if nl > 0 {
            let lights: Vec<_> = scene.lights[..nl]
                .iter()
                .map(|l| {
                    let mut light = *l;
                    let distance = Vec3::from_array(l.pos_radius[..3].try_into().unwrap())
                        .distance(camera.position);
                    let gain = distance::fade_from(distance, self.distances.lights);
                    for c in &mut light.color[..3] {
                        *c *= gain;
                    }
                    light
                })
                .collect();
            self.queue
                .write_buffer(&self.light_buf, 0, bytemuck::cast_slice(&lights));
        }

        // Batch visible instances by (model, part).
        let mut stats = FrameStats::default();
        let mut opaque: HashMap<(*const GpuModel, usize), (&GpuPart, Vec<InstanceData>)> =
            HashMap::new();
        let mut blended: Vec<(f32, Draw, InstanceData)> = Vec::new();
        // Particles: their quads in one vertex buffer, drawn among the blended.
        let mut quads: Vec<model::Vertex> = Vec::new();
        for b in &scene.particles {
            let start = quads.len() as u32;
            quads.extend_from_slice(&b.vertices);
            let data = InstanceData {
                model: Mat4::IDENTITY.to_cols_array_2d(),
                lights: [0xFFFF_FFFF; 4],
                tint: [1.0; 4],
                params: [1.0, 0.0, 0.0, 0.0],
            };
            let d = b.center.distance_squared(camera.position);
            blended.push((d, Draw::Quads(&b.material, start..quads.len() as u32), data));
        }
        if quads.len() > self.particle_cap {
            self.particle_cap = quads.len().next_power_of_two();
            self.particle_vbuf = Self::make_vbuf(
                &self.device,
                self.particle_cap * std::mem::size_of::<model::Vertex>(),
                "particles",
            );
        }
        if !quads.is_empty() {
            self.queue
                .write_buffer(&self.particle_vbuf, 0, bytemuck::cast_slice(&quads));
        }
        let tree_replacements: std::collections::HashSet<_> = scene
            .tree_lod
            .iter()
            .filter(|i| !i.hidden)
            .map(|i| i.ref_id)
            .collect();
        let full_trees: std::collections::HashSet<_> = scene
            .cells
            .values()
            .flat_map(|c| &c.instances)
            .filter(|i| !i.hidden && i.fade_class == distance::Class::Tree)
            .map(|i| i.ref_id)
            .collect();
        for inst in scene.instances() {
            if inst.hidden || inst.tint.w <= 0.0 {
                continue;
            }
            if !inst.sky && !frustum.sphere_visible(inst.world_center, inst.world_radius) {
                stats.culled += 1;
                continue;
            }
            let fade = if inst.sky {
                1.
            } else if inst.fade_class == distance::Class::TreeLod {
                if full_trees.contains(&inst.ref_id) {
                    1. - self.distances.instance(
                        distance::Class::Tree,
                        inst.world_center.distance(camera.position),
                        inst.world_radius,
                    )
                } else {
                    1.
                }
            } else if inst.fade_class == distance::Class::Tree
                && !tree_replacements.contains(&inst.ref_id)
            {
                1.
            } else {
                self.distances.instance(
                    inst.fade_class,
                    inst.world_center.distance(camera.position),
                    inst.world_radius,
                )
            };
            if fade <= 0. {
                stats.culled += 1;
                continue;
            }
            let xf = if inst.sky {
                Mat4::from_translation(camera.position) * inst.transform
            } else {
                inst.transform
            };
            let mp = Arc::as_ptr(&inst.model);
            let l = inst.lights;
            let data = InstanceData {
                model: xf.to_cols_array_2d(),
                lights: pack_lights(l),
                tint: inst.tint.to_array(),
                params: [
                    if inst.sky { 0.0 } else { 1.0 },
                    1.0 - fade,
                    if inst.fade_class == distance::Class::TreeLod {
                        1.
                    } else {
                        0.
                    },
                    0.0,
                ],
            };
            for (pi, part) in inst.model.parts.iter().enumerate() {
                if part.material.key.blend == BlendMode::Opaque {
                    opaque
                        .entry((mp, pi))
                        .or_insert_with(|| (part, Vec::new()))
                        .1
                        .push(data);
                } else {
                    // The sky's behind everything else.
                    let c = xf.transform_point3(part.bound_center);
                    let far = if inst.sky { 1.0e20 } else { 0.0 };
                    blended.push((
                        c.distance_squared(camera.position) + far,
                        Draw::Part(part),
                        data,
                    ));
                }
            }
        }
        // Rigid attachments of actors (weapons, etc.) go through the static path.
        for actor in scene.actors().chain(&scene.membranes) {
            let (center, radius) = actor.bounds();
            let fade = self
                .distances
                .actor(center.distance(camera.position), radius);
            if fade <= 0. || actor.shadow_only || !frustum.sphere_visible(center, radius) {
                continue;
            }
            for (model, bone, local) in actor.attachments.iter().chain(&actor.equipment) {
                let xf = actor.transform
                    * actor.pose.get(*bone).copied().unwrap_or(Mat4::IDENTITY)
                    * *local;
                let data = InstanceData {
                    model: xf.to_cols_array_2d(),
                    lights: pack_lights(actor.lights),
                    tint: [1.0; 4],
                    params: [1.0, 1.0 - fade, 0.0, 0.0],
                };
                for (pi, part) in model.parts.iter().enumerate() {
                    if part.material.key.blend == BlendMode::Opaque {
                        opaque
                            .entry((Arc::as_ptr(model), pi))
                            .or_insert_with(|| (part, Vec::new()))
                            .1
                            .push(data);
                    } else {
                        let c = xf.transform_point3(part.bound_center);
                        blended.push((c.distance_squared(camera.position), Draw::Part(part), data));
                    }
                }
            }
        }
        blended.sort_by(|a, b| b.0.total_cmp(&a.0));

        // Skinned actors: build the bone palette (of all of them: those out of view
        // may still cast shadows into it).
        let mut palette: Vec<Mat4> = Vec::new();
        let mut skin_inst: Vec<SkinInstanceData> = Vec::new();
        let mut skin_draws: Vec<(&GpuSkinnedPart, u32)> = Vec::new();
        let mut shadow_skin: Vec<(&GpuSkinnedPart, u32, Vec3, f32)> = Vec::new();
        for actor in scene.actors().chain(&scene.membranes) {
            let (center, radius) = actor.bounds();
            let fade = self
                .distances
                .actor(center.distance(camera.position), radius);
            let visible = fade > 0. && !actor.shadow_only && frustum.sphere_visible(center, radius);
            if !visible {
                stats.culled += 1;
                if !shadowed {
                    continue;
                }
            }
            for mesh in &actor.meshes {
                let Some(part) = mesh.model.skinned.get(mesh.part) else {
                    continue;
                };
                let base = palette.len() as u32;
                for (i, &b) in mesh.bone_map.iter().enumerate() {
                    let s2b = part.skin_to_bone.get(i).copied().unwrap_or(Mat4::IDENTITY);
                    // Bones missing from the skeleton stay in their bind position.
                    let m = match actor.pose.get(b) {
                        Some(bone) => actor.transform * *bone * s2b,
                        None => actor.transform,
                    };
                    palette.push(m);
                }
                if mesh.bone_map.is_empty() {
                    palette.push(actor.transform);
                }
                skin_inst.push(SkinInstanceData {
                    palette_base: base,
                    lights: pack_lights(actor.lights),
                    skin_tone: actor
                        .skin_tone
                        .map_or([0.0; 4], |c| c.extend(1.0).to_array()),
                    thin: 1.0 - actor.weight.clamp(0.0, 1.0),
                    fade,
                });
                let i = skin_inst.len() as u32 - 1;
                if visible {
                    skin_draws.push((part, i));
                }
                if part.material.key.blend == BlendMode::Opaque {
                    shadow_skin.push((part, i, center, radius));
                }
            }
        }
        if palette.len() > self.palette_cap {
            self.palette_cap = palette.len().next_power_of_two();
            self.palette_buf = Self::make_palette(&self.device, self.palette_cap);
            self.frame_bg = Self::make_frame_bg(
                &self.device,
                &self.frame_bgl,
                &self.frame_buf,
                &self.light_buf,
                &self.palette_buf,
                &self.shadows,
            );
        }
        if !palette.is_empty() {
            self.queue
                .write_buffer(&self.palette_buf, 0, bytemuck::cast_slice(&palette));
        }
        if skin_inst.len() > self.skin_instance_cap {
            self.skin_instance_cap = skin_inst.len().next_power_of_two();
            self.skin_instance_buf = Self::make_vbuf(
                &self.device,
                self.skin_instance_cap * std::mem::size_of::<SkinInstanceData>(),
                "skin instances",
            );
        }
        if !skin_inst.is_empty() {
            self.queue
                .write_buffer(&self.skin_instance_buf, 0, bytemuck::cast_slice(&skin_inst));
        }
        for (part, _) in &skin_draws {
            self.pipeline(part.material.key);
        }

        // Flatten into one instance buffer.
        let mut all: Vec<InstanceData> = Vec::new();
        let mut draws: Vec<(Draw, std::ops::Range<u32>)> = Vec::new();
        let mut opaque_sorted: Vec<_> = opaque.into_values().collect();
        opaque_sorted
            .sort_by_key(|(part, _)| (part.material.key.double_sided, part.material.key.z_write));
        for (part, v) in opaque_sorted {
            let start = all.len() as u32;
            all.extend(v);
            draws.push((Draw::Part(part), start..all.len() as u32));
        }
        let opaque_count = draws.len();
        for (_, d, data) in &blended {
            let start = all.len() as u32;
            all.push(*data);
            draws.push((d.clone(), start..start + 1));
        }
        if all.len() > self.instance_cap {
            self.instance_cap = all.len().next_power_of_two();
            self.instance_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("instances"),
                size: (self.instance_cap * std::mem::size_of::<InstanceData>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        if !all.is_empty() {
            self.queue
                .write_buffer(&self.instance_buf, 0, bytemuck::cast_slice(&all));
        }
        // Grass near enough to show, and in view.
        let grass_reach = self.distances.grass + crate::world::grass::FADE_RANGE;
        let grass: Vec<(&GpuPart, &GrassBatch)> = scene
            .cells
            .values()
            .flat_map(|c| c.grass.iter())
            .filter(|b| {
                b.center.distance(camera.position) - b.radius < grass_reach
                    && frustum.sphere_visible(b.center, b.radius)
            })
            .flat_map(|b| b.model.parts.iter().map(move |p| (p, b)))
            .collect();
        // Make sure all pipelines exist before borrowing them in the pass.
        for (d, _) in &draws {
            self.pipeline(d.material().key);
        }
        for (part, _) in &grass {
            self.pipeline(part.material.key);
        }

        // Decals in view, in one instance buffer.
        let decals: Vec<(&decal::GpuDecal, InstanceData)> = scene
            .cells
            .values()
            .flat_map(|c| c.decals.iter())
            .chain(&scene.actor_decals)
            .filter(|d| !d.hidden && frustum.sphere_visible(d.center, d.radius))
            .map(|d| (d, d.instance()))
            .collect();
        if decals.len() > self.decal_cap {
            self.decal_cap = decals.len().next_power_of_two();
            self.decal_buf = Self::make_vbuf(
                &self.device,
                self.decal_cap * std::mem::size_of::<InstanceData>(),
                "decal instances",
            );
        }
        if !decals.is_empty() {
            let data: Vec<InstanceData> = decals.iter().map(|d| d.1).collect();
            self.queue
                .write_buffer(&self.decal_buf, 0, bytemuck::cast_slice(&data));
        }

        let mut enc = self.device.create_command_encoder(&Default::default());
        if shadowed || occlusion_fresh {
            let layers = std::array::from_fn(|i| match (i, &self.precip_map) {
                (shadow::OCCLUSION, Some(m)) => m.cascade,
                (i, _) => cascades[i.min(shadow::CASCADES - 1)],
            });
            let update = std::array::from_fn(|i| {
                if i == shadow::OCCLUSION {
                    occlusion_fresh
                } else {
                    shadowed && update[i]
                }
            });
            self.shadow_passes(&mut enc, scene, &layers, update, &shadow_skin);
        }
        // With image space effects the frame goes through the post pass.
        let pre_post = (!scene.post.is_neutral() || (self.gamma - 1.).abs() > 0.001)
            .then(|| self.post.target(&self.device, self.width, self.height));
        {
            let c = env.clear_color;
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: pre_post.as_ref().unwrap_or(target),
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: c.x as f64,
                            g: c.y as f64,
                            b: c.z as f64,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if env.sky && self.sky.is_active() {
                self.sky.draw(
                    &self.queue,
                    &mut pass,
                    view_proj,
                    camera.position,
                    self.time,
                );
            }
            pass.set_bind_group(0, &self.frame_bg, &[]);
            pass.set_bind_group(2, &self.soft_bg, &[]);
            if scene.cells.values().any(|c| !c.terrain.is_empty()) {
                pass.set_pipeline(&self.terrain.pipeline);
                pass.set_index_buffer(self.terrain.ibuf.slice(..), wgpu::IndexFormat::Uint16);
                for chunk in scene.cells.values().flat_map(|c| c.terrain.iter()) {
                    if !frustum.sphere_visible(chunk.center, chunk.radius) {
                        continue;
                    }
                    pass.set_bind_group(1, &chunk.bind_group, &[]);
                    pass.set_vertex_buffer(0, chunk.vbuf.slice(..));
                    pass.draw_indexed(0..self.terrain.index_count, 0, 0..1);
                    stats.draws += 1;
                }
            }
            pass.set_vertex_buffer(1, self.instance_buf.slice(..));
            let current: std::cell::Cell<Option<PipelineKey>> = std::cell::Cell::new(None);
            let particle_vbuf = &self.particle_vbuf;
            let mut draw = |pass: &mut wgpu::RenderPass, d: &Draw, range: std::ops::Range<u32>| {
                let material = d.material();
                if current.get() != Some(material.key) {
                    pass.set_pipeline(&self.pipelines[&material.key]);
                    current.set(Some(material.key));
                }
                pass.set_bind_group(1, &material.bind_group, &[]);
                match d {
                    Draw::Part(part) => {
                        pass.set_vertex_buffer(0, part.vbuf.slice(..));
                        pass.set_index_buffer(part.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                        pass.draw_indexed(0..part.index_count, 0, range.clone());
                    }
                    Draw::Quads(_, vertices) => {
                        pass.set_vertex_buffer(0, particle_vbuf.slice(..));
                        pass.draw(vertices.clone(), range.clone());
                    }
                }
                stats.draws += 1;
                stats.instances += range.end - range.start;
            };
            for (d, range) in &draws[..opaque_count] {
                draw(&mut pass, d, range.clone());
            }
            let (mut grass_draws, mut grass_instances) = (0, 0);
            for (part, batch) in &grass {
                if current.get() != Some(part.material.key) {
                    pass.set_pipeline(&self.pipelines[&part.material.key]);
                    current.set(Some(part.material.key));
                }
                pass.set_bind_group(1, &part.material.bind_group, &[]);
                pass.set_vertex_buffer(0, part.vbuf.slice(..));
                pass.set_vertex_buffer(1, batch.buf.slice(..));
                pass.set_index_buffer(part.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..part.index_count, 0, 0..batch.count);
                grass_draws += 1;
                grass_instances += batch.count;
            }
            pass.set_vertex_buffer(1, self.instance_buf.slice(..));
            let mut skinned_draws = 0;
            if !skin_draws.is_empty() {
                pass.set_vertex_buffer(1, self.skin_instance_buf.slice(..));
                for (part, i) in &skin_draws {
                    if current.get() != Some(part.material.key) {
                        pass.set_pipeline(&self.pipelines[&part.material.key]);
                        current.set(Some(part.material.key));
                    }
                    pass.set_bind_group(1, &part.material.bind_group, &[]);
                    pass.set_vertex_buffer(0, part.vbuf.slice(..));
                    pass.set_index_buffer(part.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..part.index_count, 0, *i..*i + 1);
                    skinned_draws += 1;
                }
                pass.set_vertex_buffer(1, self.instance_buf.slice(..));
            }
            // Soft effects fade where they meet the opaque scene: its depth is
            // copied for them to sample while the blended draws write the
            // depth buffer.
            let soft = draws[opaque_count..].iter().any(|(d, _)| d.material().soft);
            // Decals: the opaque scene's depth read back, so in a pass of
            // their own with it attached read-only; then the rest.
            if !decals.is_empty() || soft {
                drop(pass);
                let color = pre_post.as_ref().unwrap_or(target);
                if !decals.is_empty() {
                    let mut dpass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("decals"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: color,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &self.depth_view,
                            depth_ops: None,
                            stencil_ops: None,
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    dpass.set_pipeline(&self.decals.pipeline);
                    dpass.set_bind_group(0, &self.frame_bg, &[]);
                    dpass.set_bind_group(2, &self.decals.depth_bg, &[]);
                    dpass.set_vertex_buffer(0, self.decals.cube.slice(..));
                    dpass.set_vertex_buffer(1, self.decal_buf.slice(..));
                    for (i, (d, _)) in decals.iter().enumerate() {
                        dpass.set_bind_group(1, &d.material.bind_group, &[]);
                        dpass.draw(0..36, i as u32..i as u32 + 1);
                    }
                    drop(dpass);
                }
                if soft {
                    enc.copy_texture_to_texture(
                        self.depth_tex.as_image_copy(),
                        self.soft_depth.as_image_copy(),
                        wgpu::Extent3d {
                            width: self.width,
                            height: self.height,
                            depth_or_array_layers: 1,
                        },
                    );
                }
                pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("main, after decals"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: color,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_bind_group(0, &self.frame_bg, &[]);
                pass.set_bind_group(2, &self.soft_bg, &[]);
                current.set(None);
            }
            if scene.cells.values().any(|c| !c.water.is_empty()) {
                pass.set_pipeline(&self.water.pipeline);
                for w in scene.cells.values().flat_map(|c| c.water.iter()) {
                    if !frustum.sphere_visible(w.center, w.radius) {
                        continue;
                    }
                    pass.set_bind_group(1, &w.bind_group, &[]);
                    pass.set_vertex_buffer(0, w.vbuf.slice(..));
                    pass.draw(0..6, 0..1);
                }
                current.set(None);
            }
            pass.set_vertex_buffer(1, self.instance_buf.slice(..));
            for (d, range) in &draws[opaque_count..] {
                draw(&mut pass, d, range.clone());
            }
            drop(draw);
            stats.draws += skinned_draws + grass_draws;
            stats.instances += grass_instances;
            if env.sky {
                self.precip.draw(&self.queue, &mut pass, self.time);
            }
        }
        if pre_post.is_some() {
            self.post
                .draw(&self.queue, &mut enc, target, &scene.post, self.gamma);
        }
        self.queue.submit([enc.finish()]);
        self.stats = stats;
    }

    /// Render the shadow casters into each cascade: opaque objects and actors'
    /// rigid attachments (alpha-tested ones through their cutouts), skinned actors
    /// and the landscape.
    fn shadow_passes(
        &mut self,
        enc: &mut wgpu::CommandEncoder,
        scene: &Scene,
        cascades: &[shadow::Cascade; shadow::LAYERS],
        update: [bool; shadow::LAYERS],
        skin: &[(&GpuSkinnedPart, u32, Vec3, f32)],
    ) {
        let mut all: Vec<InstanceData> = Vec::new();
        let mut per_cascade: Vec<Vec<(&GpuPart, std::ops::Range<u32>)>> = Vec::new();
        let attachments: Vec<(Mat4, &Arc<GpuModel>, [u16; 8])> = scene
            .actors()
            .flat_map(|a| {
                a.attachments
                    .iter()
                    .chain(&a.equipment)
                    .map(move |(m, bone, local)| {
                        (
                            a.transform
                                * a.pose.get(*bone).copied().unwrap_or(Mat4::IDENTITY)
                                * *local,
                            m,
                            a.lights,
                        )
                    })
            })
            .collect();
        for (ci, c) in cascades.iter().enumerate() {
            if !update[ci] {
                per_cascade.push(Vec::new());
                continue;
            }
            // Things small next to the cascade's texels don't cast into it, nor
            // clutter into the far cascades.
            let min_radius =
                (c.radius * 2.0 / shadow::SIZE as f32 * 6.0).max(shadow::MIN_CASTER[ci]);
            let mut batches: HashMap<(*const GpuModel, usize), (&GpuPart, Vec<InstanceData>)> =
                HashMap::new();
            for inst in scene.cells.values().flat_map(|c| c.instances.iter()) {
                if inst.hidden
                    || inst.tint.w <= 0.0
                    || inst.world_radius < min_radius
                    || !c.sees(inst.world_center, inst.world_radius)
                {
                    continue;
                }
                let data = InstanceData {
                    model: inst.transform.to_cols_array_2d(),
                    lights: [0; 4],
                    tint: [1.0; 4],
                    params: [1.0, 0.0, 0.0, 0.0],
                };
                for (pi, part) in inst.model.parts.iter().enumerate() {
                    if part.material.key.blend == BlendMode::Opaque && !part.material.billboard {
                        batches
                            .entry((Arc::as_ptr(&inst.model), pi))
                            .or_insert_with(|| (part, Vec::new()))
                            .1
                            .push(data);
                    }
                }
            }
            // Actors and what they carry don't keep the rain off.
            for (xf, model, _) in attachments.iter().filter(|_| ci != shadow::OCCLUSION) {
                if !c.sees(xf.transform_point3(model.bound_center), model.bound_radius) {
                    continue;
                }
                let data = InstanceData {
                    model: xf.to_cols_array_2d(),
                    lights: [0; 4],
                    tint: [1.0; 4],
                    params: [1.0, 0.0, 0.0, 0.0],
                };
                for (pi, part) in model.parts.iter().enumerate() {
                    if part.material.key.blend == BlendMode::Opaque && !part.material.billboard {
                        batches
                            .entry((Arc::as_ptr(model), pi))
                            .or_insert_with(|| (part, Vec::new()))
                            .1
                            .push(data);
                    }
                }
            }
            let mut draws = Vec::new();
            for (part, v) in batches.into_values() {
                let start = all.len() as u32;
                all.extend(v);
                draws.push((part, start..all.len() as u32));
            }
            // Depth-only casters first, then the alpha-tested ones.
            draws.sort_by_key(|(p, _)| p.material.alpha_test);
            per_cascade.push(draws);
        }
        if all.len() > self.shadow_instance_cap {
            self.shadow_instance_cap = all.len().next_power_of_two();
            self.shadow_instance_buf = Self::make_vbuf(
                &self.device,
                self.shadow_instance_cap * std::mem::size_of::<InstanceData>(),
                "shadow instances",
            );
        }
        if !all.is_empty() {
            self.queue
                .write_buffer(&self.shadow_instance_buf, 0, bytemuck::cast_slice(&all));
        }
        if log::log_enabled!(log::Level::Trace) {
            for (ci, d) in per_cascade.iter().enumerate() {
                let inst: u32 = d.iter().map(|(_, r)| r.end - r.start).sum();
                let tris: u64 = d
                    .iter()
                    .map(|(p, r)| (p.index_count / 3) as u64 * (r.end - r.start) as u64)
                    .sum();
                log::trace!(
                    "shadow cascade {ci}: {} draws, {inst} instances, {tris} triangles",
                    d.len()
                );
            }
        }
        let sh = &self.shadows;
        for (ci, c) in cascades.iter().enumerate() {
            if !update[ci] {
                continue;
            }
            self.queue.write_buffer(
                &sh.uniforms[ci],
                0,
                bytemuck::bytes_of(&c.view_proj.to_cols_array_2d()),
            );
            let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("shadow pass"),
                layout: &sh.bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: sh.uniforms[ci].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.palette_buf.as_entire_binding(),
                    },
                ],
            });
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &sh.layers[ci],
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &bg, &[]);
            pass.set_pipeline(&sh.depth_pipeline);
            pass.set_vertex_buffer(1, self.shadow_instance_buf.slice(..));
            let mut cutout = false;
            for (part, range) in &per_cascade[ci] {
                if part.material.alpha_test && !cutout {
                    pass.set_pipeline(&sh.static_pipeline);
                    cutout = true;
                }
                pass.set_bind_group(1, &part.material.bind_group, &[]);
                pass.set_vertex_buffer(0, part.vbuf.slice(..));
                pass.set_index_buffer(part.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..part.index_count, 0, range.clone());
            }
            pass.set_pipeline(&sh.skinned_pipeline);
            pass.set_vertex_buffer(1, self.skin_instance_buf.slice(..));
            for (part, i, centre, radius) in skin.iter().filter(|_| ci != shadow::OCCLUSION) {
                if !c.sees(*centre, *radius) {
                    continue;
                }
                pass.set_bind_group(1, &part.material.bind_group, &[]);
                pass.set_vertex_buffer(0, part.vbuf.slice(..));
                pass.set_index_buffer(part.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..part.index_count, 0, *i..*i + 1);
            }
            pass.set_pipeline(&sh.terrain_pipeline);
            pass.set_index_buffer(self.terrain.ibuf.slice(..), wgpu::IndexFormat::Uint16);
            for chunk in scene.cells.values().flat_map(|c| c.terrain.iter()) {
                if !c.sees(chunk.center, chunk.radius) {
                    continue;
                }
                pass.set_vertex_buffer(0, chunk.vbuf.slice(..));
                pass.draw_indexed(0..self.terrain.index_count, 0, 0..1);
            }
        }
    }

    /// Render `frames` frames offscreen and return the average wall time per frame (GPU-synchronised).
    pub fn bench(&mut self, scene: &Scene, camera: &Camera, frames: u32) -> std::time::Duration {
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("bench"),
            size: wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.color_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        self.render(scene, camera, &view);
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        let t = std::time::Instant::now();
        let mut cpu = std::time::Duration::ZERO;
        for _ in 0..frames {
            let c = std::time::Instant::now();
            self.render(scene, camera, &view);
            cpu += c.elapsed();
            let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        }
        log::info!("bench: cpu {:?}/frame", cpu / frames.max(1));
        t.elapsed() / frames.max(1)
    }

    /// Render a frame into an offscreen texture and return RGBA8 pixels.
    pub fn render_to_image(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        overlay: impl FnOnce(&mut Self, &wgpu::TextureView),
    ) -> Vec<u8> {
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen"),
            size: wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.color_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        self.render(scene, camera, &view);
        overlay(self, &view);
        let bpp = self.color_format.block_copy_size(None).unwrap_or(4);
        let bpr = (self.width * bpp).div_ceil(256) * 256;
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (bpr * self.height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: Default::default(),
                aspect: Default::default(),
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bpr),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([enc.finish()]);
        let slice = buf.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        let data = slice.get_mapped_range().expect("readback buffer mapped");
        let mut out = Vec::with_capacity((self.width * self.height * 4) as usize);
        use wgpu::TextureFormat as F;
        let unorm = |x: f32| (x.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        for y in 0..self.height {
            let row = &data[(y * bpr) as usize..(y * bpr + self.width * bpp) as usize];
            for px in row.chunks_exact(bpp as usize) {
                let rgb = match self.color_format {
                    F::Bgra8Unorm | F::Bgra8UnormSrgb => [px[2], px[1], px[0]],
                    // Window surfaces may be 10 bits per channel (R in the low bits) or half floats.
                    F::Rgb10a2Unorm => {
                        let v = u32::from_le_bytes([px[0], px[1], px[2], px[3]]);
                        let c = |shift: u32| ((v >> shift & 0x3ff) >> 2) as u8;
                        [c(0), c(10), c(20)]
                    }
                    F::Rgba16Float => {
                        let c = |i: usize| {
                            unorm(f16_to_f32(u16::from_le_bytes([px[i * 2], px[i * 2 + 1]])))
                        };
                        [c(0), c(1), c(2)]
                    }
                    _ => [px[0], px[1], px[2]],
                };
                out.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
            }
        }
        out
    }
}

fn f16_to_f32(h: u16) -> f32 {
    let (sign, exp, man) = (
        (h >> 15) as u32,
        (h >> 10 & 0x1f) as i32,
        (h & 0x3ff) as f32,
    );
    let mag = match exp {
        0 => man * 2f32.powi(-24),
        31 => f32::INFINITY,
        e => (1.0 + man / 1024.0) * 2f32.powi(e - 15),
    };
    if sign == 1 { -mag } else { mag }
}

fn pack_lights(l: [u16; 8]) -> [u32; 4] {
    [
        l[0] as u32 | (l[1] as u32) << 16,
        l[2] as u32 | (l[3] as u32) << 16,
        l[4] as u32 | (l[5] as u32) << 16,
        l[6] as u32 | (l[7] as u32) << 16,
    ]
}

fn blend_factor(f: u16) -> wgpu::BlendFactor {
    use wgpu::BlendFactor as B;
    match f {
        0 => B::One,
        1 => B::Zero,
        2 => B::Src,
        3 => B::OneMinusSrc,
        4 => B::Dst,
        5 => B::OneMinusDst,
        6 => B::SrcAlpha,
        7 => B::OneMinusSrcAlpha,
        8 => B::DstAlpha,
        9 => B::OneMinusDstAlpha,
        10 => B::SrcAlphaSaturated,
        _ => B::One,
    }
}
