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
    /// BSLightingShaderProperty shader type (4 = FaceGen, 5 = skin tint, 6 = hair tint, ...).
    pub shader_type: u32,
    /// Skin / hair tint colour.
    pub tint: Vec3,
    /// Distant LOD geometry: clipped where full-detail cells are loaded.
    pub lod: bool,
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
            shader_type: 0,
            tint: Vec3::ONE,
            lod: false,
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

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SkinVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tangent: [f32; 3],
    pub bitangent: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
    pub bones: [u32; 4],
    pub weights: [f32; 4],
}

/// A skinned shape: vertices in skin space, bound to named bones.
pub struct CpuSkinnedMesh {
    pub vertices: Vec<SkinVertex>,
    pub indices: Vec<u32>,
    pub material: MaterialDesc,
    pub bone_names: Vec<String>,
    /// Skin-space -> bone-space transform for each bone.
    pub skin_to_bone: Vec<Mat4>,
    /// Name of the shape, used for e.g. dismemberment and head part matching.
    pub name: String,
}

pub struct CpuModel {
    pub meshes: Vec<CpuMesh>,
    pub skinned: Vec<CpuSkinnedMesh>,
    pub bound_center: Vec3,
    pub bound_radius: f32,
    /// Subtrees under keyframe-animated nodes (door leaves...), drawn separately.
    pub animated: Vec<AnimatedPart>,
    /// The model's controller sequences ("Open", "Close", "Idle"...).
    pub sequences: Vec<Sequence>,
}

/// Meshes under an animated node, in that node's space.
pub struct AnimatedPart {
    pub node: String,
    /// Model-space transform of the node's parent, and the node's own rest transform.
    pub parent: Mat4,
    pub rest: Mat4,
    pub model: CpuModel,
}

/// A keyframe channel animating one node.
pub struct Channel {
    pub node: String,
    pub interpolator: nif::anim::TransformInterpolator,
    pub data: Option<std::sync::Arc<nif::anim::TransformData>>,
}

impl Channel {
    /// The node's local transform at `t`, starting from `rest` for unkeyed parts.
    pub fn sample(&self, t: f32, rest: Mat4) -> Mat4 {
        let (rs, rr, rt) = rest.to_scale_rotation_translation();
        let i = &self.interpolator;
        let (mut rot, mut tr, mut sc) = (rr, rt, rs.x);
        // Interpolator poses use huge / NaN values for "unset".
        if i.rotation.is_finite() && i.rotation.length_squared() > 0.5 {
            rot = i.rotation;
        }
        if i.translation.is_finite() && i.translation.abs().max_element() < 1.0e6 {
            tr = i.translation;
        }
        if i.scale.is_finite() && i.scale.abs() < 1.0e6 {
            sc = i.scale;
        }
        if let Some(d) = &self.data {
            let (r, t, s) = d.sample(t);
            rot = r.unwrap_or(rot);
            tr = t.unwrap_or(tr);
            sc = s.unwrap_or(sc);
        }
        Mat4::from_scale_rotation_translation(Vec3::splat(sc), rot, tr)
    }
}

pub struct Sequence {
    pub name: String,
    /// 0 loop, 1 reverse, 2 clamp.
    pub cycle: u32,
    pub start: f32,
    pub stop: f32,
    pub channels: Vec<Channel>,
}

/// Controller sequences of a model with transform channels, and the nodes they move.
fn sequences(nif: &Nif) -> Vec<Sequence> {
    let mut out = Vec::new();
    for b in &nif.blocks {
        let Block::ControllerSequence(seq) = b else { continue };
        let channels: Vec<Channel> = seq
            .blocks
            .iter()
            .filter(|cb| cb.controller_type.contains("TransformController") && !cb.node.is_empty())
            .filter_map(|cb| {
                let Some(Block::TransformInterpolator(i)) = nif.get(cb.interpolator) else { return None };
                let data = match nif.get(i.data) {
                    Some(Block::TransformData(d)) => Some(std::sync::Arc::new((**d).clone())),
                    _ => None,
                };
                Some(Channel { node: cb.node.clone(), interpolator: i.clone(), data })
            })
            .collect();
        if !channels.is_empty() {
            out.push(Sequence { name: seq.name.clone(), cycle: seq.cycle, start: seq.start, stop: seq.stop, channels });
        }
    }
    out
}

pub fn convert(nif: &Nif) -> CpuModel {
    let sequences = sequences(nif);
    let animated_nodes: std::collections::HashSet<&str> =
        sequences.iter().flat_map(|s| &s.channels).map(|c| c.node.as_str()).collect();
    let mut w = Walk { meshes: Vec::new(), skinned: Vec::new(), animated: Vec::new(), animated_nodes: &animated_nodes };
    for &root in &nif.roots {
        w.walk(nif, Ref(root as i32), Mat4::IDENTITY, 0);
    }
    let (bound_center, bound_radius) = bounds_of(
        w.meshes
            .iter()
            .map(|m| (m.bound_center, m.bound_radius))
            .chain(w.animated.iter().map(|p| ((p.parent * p.rest).transform_point3(p.model.bound_center), p.model.bound_radius))),
    );
    CpuModel { meshes: w.meshes, skinned: w.skinned, bound_center, bound_radius, animated: w.animated, sequences }
}

struct Walk<'a> {
    meshes: Vec<CpuMesh>,
    skinned: Vec<CpuSkinnedMesh>,
    animated: Vec<AnimatedPart>,
    animated_nodes: &'a std::collections::HashSet<&'a str>,
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

impl Walk<'_> {
fn walk(&mut self, nif: &Nif, r: Ref, parent: Mat4, depth: u32) {
    if depth > 64 {
        return;
    }
    let Some(block) = nif.get(r) else { return };
    let Some(av) = block.av() else { return };
    if av.hidden() || av.net.name.to_ascii_lowercase().starts_with("editormarker") {
        return;
    }
    // An animated node (not the root): its subtree becomes a separately drawn part.
    if depth > 0 && matches!(block, Block::Node(_)) && self.animated_nodes.contains(av.net.name.as_str()) {
        let mut sub = Walk { meshes: Vec::new(), skinned: Vec::new(), animated: Vec::new(), animated_nodes: self.animated_nodes };
        if let Block::Node(n) = block {
            for &c in &n.children {
                sub.walk(nif, c, Mat4::IDENTITY, depth + 1);
            }
        }
        let (bound_center, bound_radius) = bounds_of(sub.meshes.iter().map(|m| (m.bound_center, m.bound_radius)));
        let model = CpuModel { meshes: sub.meshes, skinned: sub.skinned, bound_center, bound_radius, animated: sub.animated, sequences: Vec::new() };
        self.animated.push(AnimatedPart { node: av.net.name.clone(), parent, rest: av.transform.to_mat4(), model });
        return;
    }
    let world = parent * av.transform.to_mat4();
    let (out, skinned) = (&mut self.meshes, &mut self.skinned);
    match block {
        Block::Node(n) => {
            if n.kind == NodeKind::RootCollision {
                return;
            }
            match n.kind {
                NodeKind::Switch { index } => {
                    if let Some(&c) = n.children.get(index as usize) {
                        self.walk(nif, c, world, depth + 1);
                    }
                }
                NodeKind::Lod => {
                    if let Some(&c) = n.children.first() {
                        self.walk(nif, c, world, depth + 1);
                    }
                }
                _ => {
                    for &c in &n.children {
                        self.walk(nif, c, world, depth + 1);
                    }
                }
            }
        }
        Block::TriShape(t) if !t.skin.is_none() => {
            let mat = material(nif, t.shader, t.alpha);
            if let Some(m) = build_skinned(nif, t.skin, &t.geometry, &av.net.name, mat) {
                skinned.push(m);
            }
        }
        Block::NiTriShape(g) | Block::NiTriStrips(g) if !g.skin.is_none() => {
            if let Some(Block::TriShapeData(d)) = nif.get(g.data) {
                let mat = material(nif, g.shader, g.alpha);
                if let Some(m) = build_skinned(nif, g.skin, &d.geometry, &av.net.name, mat) {
                    skinned.push(m);
                }
            }
        }
        Block::TriShape(t) => {
            if !t.geometry.triangles.is_empty() {
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
                // FaceGen heads: the tint mask sits in slot 6; reuse the glow binding.
                if s.shader_type == 4 {
                    m.glow = t.get(6).and_then(|s| tex(s));
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
            m.shader_type = s.shader_type;
            m.tint = match s.shader_type {
                5 => s.skin_tint_color,
                6 => s.hair_tint_color,
                _ => Vec3::ONE,
            };
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
    // Screen-space refraction (heat haze, etc.) isn't implemented; its "diffuse" is a normal map.
    if material.flags1 & (sf1::REFRACTION | sf1::FIRE_REFRACTION) != 0 {
        return None;
    }
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

fn build_skinned(nif: &Nif, skin: Ref, shape_geom: &Geometry, name: &str, material: MaterialDesc) -> Option<CpuSkinnedMesh> {
    let Some(Block::SkinInstance(si)) = nif.get(skin) else { return None };
    let Some(Block::SkinData(sd)) = nif.get(si.data) else { return None };
    let partition = match nif.get(si.partition) {
        Some(Block::SkinPartition(p)) => Some(p.as_ref()),
        _ => None,
    };
    let bone_names: Vec<String> =
        si.bones.iter().map(|b| nif.get(*b).and_then(|b| b.av()).map(|a| a.net.name.clone()).unwrap_or_default()).collect();
    let skin_to_bone: Vec<Mat4> = sd.bones.iter().map(|b| b.transform.to_mat4()).collect();

    // SSE keeps the vertex data in the partition; LE in the shape (or its data block).
    // BSDynamicTriShape keeps positions in the shape and everything else in the partition.
    let merged;
    let g = match partition {
        Some(p) if !p.geometry.uvs.is_empty() || !p.geometry.positions.is_empty() => {
            if p.geometry.positions.is_empty() {
                let mut m = p.geometry.clone();
                m.positions = shape_geom.positions.clone();
                merged = m;
                &merged
            } else {
                &p.geometry
            }
        }
        _ => shape_geom,
    };
    let n = g.positions.len();
    if n == 0 {
        return None;
    }
    let mut bones = vec![[0u32; 4]; n];
    let mut weights = vec![[0f32; 4]; n];
    let mut indices: Vec<u32> = Vec::new();
    match partition {
        Some(p) => {
            for part in &p.partitions {
                let map = |i: usize| -> usize { if part.vertex_map.is_empty() { i } else { part.vertex_map[i] as usize } };
                let sse = !p.geometry.uvs.is_empty() || !p.geometry.positions.is_empty();
                for local in 0..part.num_vertices as usize {
                    let gi = map(local);
                    if gi >= n {
                        continue;
                    }
                    let (bi, w) = if sse {
                        (g.bone_indices.get(gi).copied().unwrap_or_default(), g.bone_weights.get(gi).copied().unwrap_or_default())
                    } else {
                        (part.bone_indices.get(local).copied().unwrap_or_default(), part.weights.get(local).copied().unwrap_or_default())
                    };
                    for k in 0..4 {
                        bones[gi][k] = part.bones.get(bi[k] as usize).copied().unwrap_or(0) as u32;
                        weights[gi][k] = w[k];
                    }
                }
                for t in &part.triangles {
                    // SSE triangles index the global buffer; LE ones the partition's vertex map.
                    let tri = if sse { [t[0] as usize, t[1] as usize, t[2] as usize] } else { [map(t[0] as usize), map(t[1] as usize), map(t[2] as usize)] };
                    if tri.iter().all(|&i| i < n) {
                        indices.extend(tri.iter().map(|&i| i as u32));
                    }
                }
            }
        }
        None => {
            // Weights stored per bone in NiSkinData.
            let mut count = vec![0usize; n];
            for (bi, b) in sd.bones.iter().enumerate() {
                for &(vi, w) in &b.weights {
                    let vi = vi as usize;
                    if vi < n && count[vi] < 4 {
                        bones[vi][count[vi]] = bi as u32;
                        weights[vi][count[vi]] = w;
                        count[vi] += 1;
                    }
                }
            }
            for t in &g.triangles {
                indices.extend_from_slice(&[t[0] as u32, t[1] as u32, t[2] as u32]);
            }
        }
    }
    if indices.is_empty() {
        return None;
    }
    let mut vertices = Vec::with_capacity(n);
    for i in 0..n {
        let mut w = weights[i];
        let sum: f32 = w.iter().sum();
        if sum > 0.0 {
            for x in &mut w {
                *x /= sum;
            }
        } else {
            w = [1.0, 0.0, 0.0, 0.0];
        }
        vertices.push(SkinVertex {
            position: g.positions[i].to_array(),
            normal: g.normals.get(i).copied().unwrap_or(Vec3::Z).to_array(),
            tangent: g.tangents.get(i).copied().unwrap_or(Vec3::X).to_array(),
            bitangent: g.bitangents.get(i).copied().unwrap_or(Vec3::Y).to_array(),
            uv: g.uvs.get(i).copied().unwrap_or(Vec2::ZERO).to_array(),
            color: g.colors.get(i).copied().unwrap_or(Vec4::ONE).to_array(),
            bones: bones[i],
            weights: w,
        });
    }
    Some(CpuSkinnedMesh { vertices, indices, material, bone_names, skin_to_bone, name: name.to_owned() })
}
