//! Surface-born EFSH particles. Only sampled vertices are skinned on the CPU.
use super::records::EffectShader;
use crate::render::{
    ActorInstance,
    model::{SkinVertex, Vertex},
};
use glam::{Mat4, Vec3, Vec4};

#[derive(Clone)]
struct Particle {
    position: Vec3,
    velocity: Vec3,
    acceleration: Vec3,
    age: f32,
    life: f32,
    angle: f32,
    spin: f32,
}
#[derive(Default)]
pub struct ShaderParticles {
    particles: Vec<Particle>,
    owed: f32,
    rng: u32,
}
impl ShaderParticles {
    fn random(&mut self) -> f32 {
        let mut x = self.rng.max(1);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }
    fn signed(&mut self) -> f32 {
        self.random() * 2.0 - 1.0
    }
    pub fn step(
        &mut self,
        shader: &EffectShader,
        age: f32,
        stopped: Option<f32>,
        actor: Option<&ActorInstance>,
        dt: f32,
        right: Vec3,
        up: Vec3,
        vertices: &mut Vec<Vertex>,
    ) {
        for p in &mut self.particles {
            p.age += dt;
            p.velocity += p.acceleration * dt;
            p.position += p.velocity * dt;
            p.angle += p.spin * dt;
        }
        self.particles.retain(|p| p.age < p.life);
        if shader.flags() & 8 == 0 {
            let rate = birth_rate(shader, age, stopped);
            self.owed += (rate * dt).max(0.0);
            let births =
                (self.owed.floor() as usize).min(4096usize.saturating_sub(self.particles.len()));
            self.owed -= births as f32;
            if let Some(actor) = actor {
                for _ in 0..births {
                    let seed = self.random();
                    let u = self.random();
                    let v = self.random();
                    let Some((mut position, normal)) =
                        surface(actor, seed, u, v, shader.flags() & (1 << 5) != 0)
                    else {
                        continue;
                    };
                    position +=
                        normal * (shader.float(0x15C) + self.signed() * shader.float(0x160));
                    let life = (shader.float(0x84) + self.signed() * shader.float(0x88)).max(0.01);
                    let velocity = normal
                        * (shader.float(0x8C) + self.signed() * shader.float(0xE0))
                        + Vec3::new(shader.float(0x94), shader.float(0x98), shader.float(0x9C));
                    let acceleration = normal * shader.float(0x90)
                        + Vec3::new(shader.float(0xA0), shader.float(0xA4), shader.float(0xA8));
                    let angle =
                        (shader.float(0xE4) + self.signed() * shader.float(0xE8)).to_radians();
                    let spin =
                        (shader.float(0xEC) + self.signed() * shader.float(0xF0)).to_radians();
                    self.particles.push(Particle {
                        position,
                        velocity,
                        acceleration,
                        age: 0.0,
                        life,
                        angle,
                        spin,
                    });
                }
            }
        }
        for p in &self.particles {
            let t = (p.age / p.life).clamp(0.0, 1.0);
            let phase = lerp_time(t, shader.float(0xB4), shader.float(0xB8));
            let size = shader.float(0xAC) + (shader.float(0xB0) - shader.float(0xAC)) * phase;
            let colors = [0xBC, 0xC0, 0xC4].map(|o| Vec3::from_array(shader.color(o)));
            let alpha = [0xC8, 0xCC, 0xD0].map(|o| shader.float(o));
            let times = [0xD4, 0xD8, 0xDC].map(|o| shader.float(o));
            let mut color = colors[0].extend(alpha[0]);
            for i in 1..3 {
                color = color.lerp(
                    colors[i].extend(alpha[i]),
                    lerp_time(t, times[i - 1], times[i]),
                );
            }
            let (sin, cos) = p.angle.sin_cos();
            let x = (right * cos + up * sin) * size;
            let y = (up * cos - right * sin) * size;
            let cols = shader.u32(0x114).max(1);
            let rows = shader.u32(0x118).max(1);
            let frame = if shader.flags() & (1 << 15) != 0 {
                let start = shader.u32(0x164);
                let end = shader.u32(0x16C).max(start);
                start + ((end - start) as f32 * t) as u32
            } else {
                0
            };
            let uv = |u: f32, v: f32| {
                [
                    (frame % cols) as f32 / cols as f32 + u / cols as f32,
                    (frame / cols % rows) as f32 / rows as f32 + v / rows as f32,
                ]
            };
            let make = |position: Vec3, uv| Vertex {
                position: position.to_array(),
                normal: [0.0, 0.0, 1.0],
                tangent: [1.0, 0.0, 0.0],
                bitangent: [0.0, 1.0, 0.0],
                uv,
                color: color.to_array(),
            };
            let quad = [
                make(p.position - x - y, uv(0.0, 1.0)),
                make(p.position + x - y, uv(1.0, 1.0)),
                make(p.position + x + y, uv(1.0, 0.0)),
                make(p.position - x + y, uv(0.0, 0.0)),
            ];
            for i in [0, 1, 2, 0, 2, 3] {
                vertices.push(quad[i]);
            }
        }
    }
}
fn lerp_time(t: f32, start: f32, end: f32) -> f32 {
    if end <= start {
        f32::from(t >= end)
    } else {
        ((t - start) / (end - start)).clamp(0.0, 1.0)
    }
}
pub fn birth_rate(s: &EffectShader, age: f32, stop: Option<f32>) -> f32 {
    let up = s.float(0x70);
    let full = s.float(0x74);
    let down = s.float(0x78);
    let peak = s.float(0x7C).max(0.0);
    let persist = s.float(0x80).max(0.0);
    let rate = |t| {
        if t < up {
            peak * lerp_time(t, 0.0, up)
        } else if t < up + full {
            peak
        } else {
            peak + (persist - peak) * lerp_time(t, up + full, up + full + down)
        }
    };
    stop.map_or_else(
        || rate(age),
        |stop| rate(stop) * (1.0 - lerp_time(age, stop, stop + down)),
    )
}
fn skin_vertex(
    actor: &ActorInstance,
    skin_to_bone: &[Mat4],
    bone_map: &[usize],
    v: &SkinVertex,
) -> (Vec3, Vec3) {
    let thin = 1.0 - actor.weight.clamp(0.0, 1.0);
    let pos = Vec3::from_array(v.position) + Vec3::from_array(v.thin_position) * thin;
    let normal = Vec3::from_array(v.normal) + Vec3::from_array(v.thin_normal) * thin;
    let mut p = Vec3::ZERO;
    let mut n = Vec3::ZERO;
    for i in 0..4 {
        let b = v.bones[i] as usize;
        let pose = bone_map
            .get(b)
            .and_then(|b| actor.pose.get(*b))
            .copied()
            .unwrap_or(Mat4::IDENTITY);
        let transform =
            actor.transform * pose * skin_to_bone.get(b).copied().unwrap_or(Mat4::IDENTITY);
        p += transform.transform_point3(pos) * v.weights[i];
        n += transform.transform_vector3(normal) * v.weights[i];
    }
    (p, n.normalize_or(Vec3::Z))
}
fn surface(
    actor: &ActorInstance,
    seed: f32,
    u: f32,
    v: f32,
    skin_only: bool,
) -> Option<(Vec3, Vec3)> {
    let meshes: Vec<_> = actor
        .meshes
        .iter()
        .filter_map(|m| {
            m.model
                .skinned
                .get(m.part)
                .filter(|p| !skin_only || p.material.skin)
                .map(|p| (m, p))
        })
        .collect();
    let total: usize = meshes.iter().map(|(_, p)| p.cpu_indices.len() / 3).sum();
    if total == 0 {
        return None;
    }
    let mut triangle = ((seed * total as f32) as usize).min(total - 1);
    for (mesh, part) in meshes {
        let count = part.cpu_indices.len() / 3;
        if triangle >= count {
            triangle -= count;
            continue;
        }
        let indices = part.cpu_indices.get(triangle * 3..triangle * 3 + 3)?;
        let mut points = [Vec3::ZERO; 3];
        let mut normals = [Vec3::ZERO; 3];
        for i in 0..3 {
            (points[i], normals[i]) = skin_vertex(
                actor,
                &part.skin_to_bone,
                &mesh.bone_map,
                part.cpu_vertices.get(indices[i] as usize)?,
            );
        }
        let a = u.sqrt();
        let weights = Vec3::new(1.0 - a, a * (1.0 - v), a * v);
        return Some((
            points[0] * weights.x + points[1] * weights.y + points[2] * weights.z,
            (normals[0] * weights.x + normals[1] * weights.y + normals[2] * weights.z)
                .normalize_or(Vec3::Z),
        ));
    }
    None
}
pub fn material(s: &EffectShader) -> crate::render::model::MaterialDesc {
    let mut flags1 = nif::sf1::VERTEX_ALPHA;
    if s.flags() & (1 << 16) != 0 {
        flags1 |= nif::sf1::GREYSCALE_TO_PALETTE_COLOR;
    }
    if s.flags() & (1 << 17) != 0 {
        flags1 |= nif::sf1::GREYSCALE_TO_PALETTE_ALPHA;
    }
    crate::render::model::MaterialDesc {
        kind: crate::render::model::ShaderKind::Effect,
        diffuse: s.textures[1].clone(),
        glow: s.textures[4].clone(),
        emissive: Vec4::ONE,
        flags1,
        flags2: nif::sf2::VERTEX_COLORS,
        blend: crate::render::model::BlendMode::Blend(
            crate::render::membrane::blend_factor(s.u32(0x60)),
            crate::render::membrane::blend_factor(s.u32(0x6C)),
        ),
        blend_op: s.u32(0x64) as u8,
        depth_func: s.u32(0x68) as u8,
        z_write: false,
        double_sided: true,
        ..Default::default()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sampled_vertices_follow_bones_and_weight_morph() {
        let mut actor = ActorInstance {
            meshes: Vec::new(),
            attachments: Vec::new(),
            equipment: Vec::new(),
            held_light: None,
            transform: Mat4::from_translation(Vec3::X * 10.0),
            pose: vec![Mat4::from_translation(Vec3::Z * 2.0)],
            lights: [0; 8],
            radius: 1.0,
            shadow_only: false,
            skin_tone: None,
            weight: 0.25,
        };
        let vertex = SkinVertex {
            position: [0.0, 1.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            tangent: [0.0; 3],
            bitangent: [0.0; 3],
            uv: [0.0; 2],
            color: [1.0; 4],
            bones: [0; 4],
            weights: [1.0, 0.0, 0.0, 0.0],
            thin_position: [2.0, 0.0, 0.0],
            thin_normal: [0.0; 3],
        };
        let skin_to_bone = [Mat4::from_translation(Vec3::Y * 3.0)];
        let (point, normal) = skin_vertex(&actor, &skin_to_bone, &[0], &vertex);
        assert_eq!(point, Vec3::new(11.5, 4.0, 2.0));
        assert_eq!(normal, Vec3::Z);
        actor.pose[0] = Mat4::from_translation(Vec3::Z * 5.0);
        assert_eq!(
            skin_vertex(&actor, &skin_to_bone, &[0], &vertex).0,
            Vec3::new(11.5, 4.0, 5.0)
        );
    }

    #[test]
    fn birth_ramps_integrate_expected_counts() {
        let mut s = EffectShader {
            textures: Default::default(),
            data: vec![0; 400],
            addon_models: esp::FormId::NULL,
            ambient_sound: esp::FormId::NULL,
        };
        for (o, f) in [
            (0x70, 1.0f32),
            (0x74, 2.0),
            (0x78, 1.0),
            (0x7C, 10.0),
            (0x80, 2.0),
        ] {
            s.data[o..o + 4].copy_from_slice(&f.to_le_bytes());
        }
        let sum = (0..400)
            .map(|i| birth_rate(&s, (i as f32 + 0.5) * 0.01, None) * 0.01)
            .sum::<f32>();
        assert!((sum - 31.0).abs() < 0.001);
        assert_eq!(birth_rate(&s, 6.0, None), 2.0);
        assert_eq!(birth_rate(&s, 5.0, Some(4.0)), 0.0);
    }
}
