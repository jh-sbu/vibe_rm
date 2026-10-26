//! EFSH membrane sampling and geometry-sharing overlay materials.
use super::model::{BlendMode, MaterialDesc, ShaderKind};
use crate::spell_fx::records::EffectShader;
use glam::{Vec2, Vec3, Vec4};

#[derive(Debug, Clone, Copy, Default)]
pub struct MembraneSample {
    pub fill: [f32; 4],
    pub edge: [f32; 4],
    /// Edge exponent, holes threshold, holes edge width, inverse edge.
    pub params: [f32; 4],
    pub flags: u32,
}
fn finite(v: f32) -> f32 {
    if v.is_finite() { v } else { 0.0 }
}
fn ramp(t: f32, start: f32, end: f32) -> f32 {
    if end <= start {
        f32::from(t >= end)
    } else {
        ((t - start) / (end - start)).clamp(0.0, 1.0)
    }
}
/// A full-alpha stage followed by a persistent level; stopping always fades
/// from the current level, so a mid-fade stop has no opacity discontinuity.
pub fn envelope(
    age: f32,
    stopped: Option<f32>,
    fade_in: f32,
    full: f32,
    fade_out: f32,
    peak: f32,
    persistent: f32,
    pulse: f32,
    frequency: f32,
) -> f32 {
    let at = |t: f32| {
        let t = t.max(0.0);
        let base = if t < fade_in {
            peak * ramp(t, 0.0, fade_in)
        } else if t < fade_in + full {
            peak
        } else {
            peak + (persistent - peak) * ramp(t, fade_in + full, fade_in + full + fade_out)
        };
        (base * (1.0 + pulse * (t * frequency * std::f32::consts::TAU).sin())).max(0.0)
    };
    let alpha = match stopped {
        Some(stop) => at(stop) * (1.0 - ramp(age, stop, stop + fade_out)),
        None => at(age),
    };
    finite(alpha).clamp(0.0, 1.0)
}
fn color_keys(shader: &EffectShader, age: f32) -> Vec3 {
    let colors = [0x10, 0x138, 0x13C].map(|o| Vec3::from_array(shader.color(o)));
    let scales = [0x140, 0x144, 0x148].map(|o| shader.float(o));
    let times = [0x14C, 0x150, 0x154].map(|o| shader.float(o));
    let keys = std::array::from_fn::<_, 3, _>(|i| {
        colors[i]
            * if shader.data.len() > 0x158 {
                scales[i]
            } else {
                1.0
            }
    });
    let mut result = keys[0];
    for i in 1..3 {
        result = result.lerp(keys[i], ramp(age, times[i - 1], times[i]));
    }
    result
        * if shader.data.len() > 0x158 {
            shader.float(0x158)
        } else {
            1.0
        }
}
pub fn sample(shader: &EffectShader, age: f32, stopped: Option<f32>) -> MembraneSample {
    let env = |base, peak| {
        envelope(
            age,
            stopped,
            shader.float(base),
            shader.float(base + 4),
            shader.float(base + 8),
            shader.float(peak),
            shader.float(base + 12),
            shader.float(base + 16),
            shader.float(base + 20),
        )
    };
    let fill = color_keys(shader, age);
    let edge = Vec3::from_array(shader.color(0x38));
    let holes = shader.float(0x100)
        + (shader.float(0x104) - shader.float(0x100))
            * ramp(age, shader.float(0xF8), shader.float(0xFC));
    MembraneSample {
        fill: fill.extend(env(0x14, 0x54)).to_array(),
        edge: edge.extend(env(0x3C, 0x58)).to_array(),
        params: [
            finite(shader.float(0x34)).max(0.001),
            if shader.textures[2].is_some() {
                holes
            } else {
                0.0
            },
            shader.float(0x108).max(0.001),
            f32::from(shader.flags() & 16 != 0),
        ],
        flags: shader.flags(),
    }
}
pub fn material(shader: &EffectShader, age: f32, stopped: Option<f32>) -> MaterialDesc {
    MaterialDesc {
        kind: ShaderKind::Membrane,
        diffuse: shader.textures[0].clone(),
        normal: shader.textures[2].clone(),
        glow: shader.textures[3].clone(),
        blend: BlendMode::Blend(blend_factor(shader.u32(4)), blend_factor(shader.u32(0x5C))),
        alpha: 1.0,
        emissive: Vec4::ONE,
        uv_offset: Vec2::new(shader.float(0x2C), shader.float(0x30)) * age,
        uv_scale: if shader.data.len() >= 0x18C {
            Vec2::new(shader.float(0x184), shader.float(0x188))
        } else {
            Vec2::ONE
        },
        blend_op: shader.u32(8) as u8,
        depth_func: shader.u32(0xC) as u8,
        z_write: false,
        z_test: shader.u32(0xC) != 8,
        double_sided: true,
        membrane: Some(sample(shader, age, stopped)),
        // Identical posed geometry reproduces depth equality. A nearer bias
        // would incorrectly fail an authored GreaterEqual test after reversal.
        flags1: 0,
        flags2: 0,
        ..Default::default()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn envelope_preserves_stop_level_and_fades_to_zero() {
        let env = |t, stop| envelope(t, stop, 2.0, 1.0, 2.0, 1.0, 0.25, 0.0, 1.0);
        assert_eq!(env(0.0, None), 0.0);
        assert_eq!(env(1.0, None), 0.5);
        assert_eq!(env(2.0, None), 1.0);
        assert_eq!(env(4.0, None), 0.625);
        assert_eq!(env(6.0, None), 0.25);
        assert_eq!(env(1.0, Some(1.0)), 0.5);
        assert_eq!(env(2.0, Some(1.0)), 0.25);
        assert_eq!(env(3.0, Some(1.0)), 0.0);
    }
}

use super::{ActorInstance, ActorMesh, GpuMaterial, GpuModel, GpuPart, GpuSkinnedPart};
use std::sync::Arc;
/// Reuse GPU geometry and bones. No vertices or assets are duplicated on disk.
pub fn overlay_model(model: &GpuModel, material: &Arc<GpuMaterial>) -> Arc<GpuModel> {
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
                material: material.clone(),
                bound_center: p.bound_center,
                bound_radius: p.bound_radius,
            })
            .collect(),
        skinned: model
            .skinned
            .iter()
            .map(|p| {
                // A skinned pipeline shares the material's bind group, but has a
                // distinct pipeline key. It is built separately by overlay_actor.
                GpuSkinnedPart {
                    cpu_vertices: p.cpu_vertices.clone(),
                    cpu_indices: p.cpu_indices.clone(),
                    vbuf: p.vbuf.clone(),
                    ibuf: p.ibuf.clone(),
                    index_count: p.index_count,
                    material: material.clone(),
                    bone_names: p.bone_names.clone(),
                    skin_to_bone: p.skin_to_bone.clone(),
                    name: p.name.clone(),
                }
            })
            .collect(),
        particles: Vec::new(),
        bound_center: model.bound_center,
        bound_radius: model.bound_radius,
    })
}
pub fn overlay_actor(
    actor: &ActorInstance,
    rigid: &Arc<GpuMaterial>,
    skin: &Arc<GpuMaterial>,
    flags: u32,
) -> ActorInstance {
    let skin_only = flags & (1 << 5) != 0;
    let no_weapons = flags & (1 << 10) != 0;
    let meshes = actor
        .meshes
        .iter()
        .filter_map(|mesh| {
            let part = mesh.model.skinned.get(mesh.part)?;
            if skin_only && !part.material.skin {
                return None;
            }
            Some(ActorMesh {
                model: overlay_model(&mesh.model, skin),
                part: mesh.part,
                bone_map: mesh.bone_map.clone(),
            })
        })
        .collect();
    let attachments = actor
        .attachments
        .iter()
        .chain(&actor.equipment)
        .filter(|(m, _, _)| {
            !skin_only && !(no_weapons && m.path.to_ascii_lowercase().contains("weapons/"))
        })
        .map(|(m, b, t)| (overlay_model(m, rigid), *b, *t))
        .collect();
    ActorInstance {
        meshes,
        attachments,
        equipment: Vec::new(),
        held_light: None,
        transform: actor.transform,
        pose: actor.pose.clone(),
        lights: actor.lights,
        radius: actor.radius,
        shadow_only: actor.shadow_only,
        skin_tone: None,
        weight: actor.weight,
    }
}

/// D3DBLEND (EFSH) -> NiAlphaProperty factors (our ordinary pipeline).
/// Microsoft Learn D3D9 documentation; see refs/spell-visuals/d3d9.md.
pub fn blend_factor(value: u32) -> u16 {
    match value {
        1 => 1,
        2 => 0,
        3 => 2,
        4 => 3,
        5 => 6,
        6 => 7,
        7 => 8,
        8 => 9,
        9 => 4,
        10 => 5,
        11 => 10,
        _ => 0,
    }
}
