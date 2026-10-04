//! Conversion of NIF scene graphs into flat, render-ready CPU meshes.

use glam::{Mat3, Mat4, Vec2, Vec3, Vec4};
use nif::{Block, Geometry, Nif, NodeKind, Ref, sf1, sf2};

use crate::world::records::texture_path;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tangent: [f32; 3],
    pub bitangent: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlendMode {
    Opaque,
    /// NiAlphaProperty src/dst blend factors.
    Blend(u16, u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderKind {
    Lit,
    Effect,
}

#[derive(Debug, Clone)]
pub struct MaterialDesc {
    pub kind: ShaderKind,
    pub diffuse: Option<String>,
    pub normal: Option<String>,
    pub glow: Option<String>,
    pub flags1: u32,
    pub flags2: u32,
    pub blend: BlendMode,
    /// Alpha test threshold in [0,1], if alpha testing is enabled.
    pub alpha_test: Option<f32>,
    pub alpha: f32,
    pub uv_offset: Vec2,
    pub uv_scale: Vec2,
    pub emissive: Vec4,
    pub specular: Vec3,
    pub glossiness: f32,
    pub double_sided: bool,
    pub z_write: bool,
    pub z_test: bool,
    /// Effect shader view-angle falloff (start, stop, start opacity, stop opacity).
    pub falloff: Vec4,
}

impl Default for MaterialDesc {
    fn default() -> Self {
        MaterialDesc {
            kind: ShaderKind::Lit,
            diffuse: None,
            normal: None,
            glow: None,
            flags1: 0,
            flags2: 0,
            blend: BlendMode::Opaque,
            alpha_test: None,
            alpha: 1.0,
            uv_offset: Vec2::ZERO,
            uv_scale: Vec2::ONE,
            emissive: Vec4::ZERO,
            specular: Vec3::ZERO,
            glossiness: 1.0,
            double_sided: false,
            z_write: true,
            z_test: true,
            falloff: Vec4::new(1.0, 0.0, 1.0, 1.0),
        }
    }
}

pub struct CpuMesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub material: MaterialDesc,
    pub bound_center: Vec3,
    pub bound_radius: f32,
}

pub struct CpuModel {
    pub meshes: Vec<CpuMesh>,
    pub bound_center: Vec3,
    pub bound_radius: f32,
}

pub fn convert(nif: &Nif) -> CpuModel {
    let mut meshes = Vec::new();
    for &root in &nif.roots {
        walk(nif, Ref(root as i32), Mat4::IDENTITY, &mut meshes, 0);
    }
    let (bound_center, bound_radius) = bounds_of(meshes.iter().map(|m| (m.bound_center, m.bound_radius)));
    CpuModel { meshes, bound_center, bound_radius }
}

pub fn bounds_of(spheres: impl Iterator<Item = (Vec3, f32)>) -> (Vec3, f32) {
    let v: Vec<(Vec3, f32)> = spheres.collect();
    if v.is_empty() {
        return (Vec3::ZERO, 0.0);
    }
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    for (c, r) in &v {
        min = min.min(*c - Vec3::splat(*r));
        max = max.max(*c + Vec3::splat(*r));
    }
    let center = (min + max) * 0.5;
    let radius = v.iter().map(|(c, r)| c.distance(center) + r).fold(0.0, f32::max);
    (center, radius)
}

fn walk(nif: &Nif, r: Ref, parent: Mat4, out: &mut Vec<CpuMesh>, depth: u32) {
    if depth > 64 {
        return;
    }
    let Some(block) = nif.get(r) else { return };
    let Some(av) = block.av() else { return };
    if av.hidden() || av.net.name.to_ascii_lowercase().starts_with("editormarker") {
        return;
    }
    let world = parent * av.transform.to_mat4();
    match block {
        Block::Node(n) => {
            if n.kind == NodeKind::RootCollision {
                return;
            }
            match n.kind {
                NodeKind::Switch { index } => {
                    if let Some(&c) = n.children.get(index as usize) {
                        walk(nif, c, world, out, depth + 1);
                    }
                }
                NodeKind::Lod => {
                    if let Some(&c) = n.children.first() {
                        walk(nif, c, world, out, depth + 1);
                    }
                }
                _ => {
                    for &c in &n.children {
                        walk(nif, c, world, out, depth + 1);
                    }
                }
            }
        }
        Block::TriShape(t) => {
            if t.skin.is_none() && !t.geometry.triangles.is_empty() {
                let mat = material(nif, t.shader, t.alpha);
                if let Some(m) = build_mesh(&t.geometry, world, mat) {
                    out.push(m);
                }
            }
        }
        Block::NiTriShape(g) | Block::NiTriStrips(g) => {
            if g.skin.is_none()
                && let Some(Block::TriShapeData(d)) = nif.get(g.data)
            {
                let mat = material(nif, g.shader, g.alpha);
                if let Some(m) = build_mesh(&d.geometry, world, mat) {
                    out.push(m);
                }
            }
        }
        _ => {}
    }
}

fn tex(s: &str) -> Option<String> {
    let s = s.trim();
    if s.is_empty() { None } else { Some(texture_path(s)) }
}

pub fn material(nif: &Nif, shader: Ref, alpha: Ref) -> MaterialDesc {
    let mut m = MaterialDesc::default();
    match nif.get(shader) {
        Some(Block::LightingShader(s)) => {
            m.flags1 = s.flags1;
            m.flags2 = s.flags2;
            if let Some(Block::TextureSet(t)) = nif.get(s.texture_set) {
                m.diffuse = t.first().and_then(|s| tex(s));
                m.normal = t.get(1).and_then(|s| tex(s));
                if s.flags2 & sf2::GLOW_MAP != 0 {
                    m.glow = t.get(2).and_then(|s| tex(s));
                }
            }
            m.alpha = s.alpha;
            m.uv_offset = s.uv_offset;
            m.uv_scale = s.uv_scale;
            if s.flags1 & sf1::OWN_EMIT != 0 || s.flags2 & sf2::GLOW_MAP != 0 {
                m.emissive = s.emissive_color.extend(s.emissive_multiple);
            }
            if s.flags1 & sf1::SPECULAR != 0 {
                m.specular = s.specular_color * s.specular_strength;
            }
            m.glossiness = s.glossiness;
            m.double_sided = s.flags2 & sf2::DOUBLE_SIDED != 0;
            m.z_write = s.flags2 & sf2::ZBUFFER_WRITE != 0;
            m.z_test = s.flags1 & sf1::ZBUFFER_TEST != 0;
        }
        Some(Block::EffectShader(s)) => {
            m.kind = ShaderKind::Effect;
            m.flags1 = s.flags1;
            m.flags2 = s.flags2;
            m.diffuse = tex(&s.source_texture);
            m.glow = tex(&s.greyscale_texture);
            m.falloff = s.falloff;
            m.uv_offset = s.uv_offset;
            m.uv_scale = s.uv_scale;
            m.emissive = s.emissive_color.truncate().extend(s.emissive_multiple);
            m.alpha = s.emissive_color.w;
            m.double_sided = s.flags2 & sf2::DOUBLE_SIDED != 0;
            m.z_write = s.flags2 & sf2::ZBUFFER_WRITE != 0;
            m.z_test = s.flags1 & sf1::ZBUFFER_TEST != 0;
        }
        _ => {}
    }
    if let Some(Block::Alpha(a)) = nif.get(alpha) {
        if a.blend_enabled() {
            m.blend = BlendMode::Blend(a.src_blend(), a.dst_blend());
        }
        if a.test_enabled() {
            m.alpha_test = Some(a.threshold as f32 / 255.0);
        }
    }
    m
}

fn build_mesh(g: &Geometry, world: Mat4, material: MaterialDesc) -> Option<CpuMesh> {
    let n = g.positions.len();
    if n == 0 {
        return None;
    }
    let nm = Mat3::from_mat4(world).inverse().transpose();
    let rot = Mat3::from_mat4(world);
    let mut vertices = Vec::with_capacity(n);
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    for i in 0..n {
        let p = world.transform_point3(g.positions[i]);
        min = min.min(p);
        max = max.max(p);
        let normal = g.normals.get(i).map(|v| (nm * *v).normalize_or_zero()).unwrap_or(Vec3::Z);
        let tangent = g.tangents.get(i).map(|v| (rot * *v).normalize_or_zero()).unwrap_or(Vec3::X);
        let bitangent = g.bitangents.get(i).map(|v| (rot * *v).normalize_or_zero()).unwrap_or(Vec3::Y);
        let uv = g.uvs.get(i).copied().unwrap_or(Vec2::ZERO);
        let color = g.colors.get(i).copied().unwrap_or(Vec4::ONE);
        vertices.push(Vertex {
            position: p.to_array(),
            normal: normal.to_array(),
            tangent: tangent.to_array(),
            bitangent: bitangent.to_array(),
            uv: uv.to_array(),
            color: color.to_array(),
        });
    }
    let mut indices = Vec::with_capacity(g.triangles.len() * 3);
    for t in &g.triangles {
        if (t[0] as usize) < n && (t[1] as usize) < n && (t[2] as usize) < n {
            indices.extend_from_slice(&[t[0] as u32, t[1] as u32, t[2] as u32]);
        }
    }
    if indices.is_empty() {
        return None;
    }
    // Mirrored transforms flip winding.
    if world.determinant() < 0.0 {
        for t in indices.chunks_exact_mut(3) {
            t.swap(1, 2);
        }
    }
    let center = (min + max) * 0.5;
    let radius = (max - min).length() * 0.5;
    Some(CpuMesh { vertices, indices, material, bound_center: center, bound_radius: radius })
}
