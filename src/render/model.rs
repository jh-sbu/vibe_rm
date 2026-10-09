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
    /// The shader property's controllers (scrolling, pulsing).
    pub anim: Option<std::sync::Arc<MaterialAnim>>,
}

/// A material's animated values: its shader property's float and colour
/// controllers, sampled over time.
#[derive(Debug, Clone)]
pub struct MaterialAnim {
    pub channels: Vec<MaterialChannel>,
}

#[derive(Debug, Clone)]
pub struct MaterialChannel {
    pub timing: nif::anim::Timing,
    pub keys: nif::anim::ValueKeys,
    /// The controlled variable ([`nif::anim::ShaderController::variable`]).
    pub variable: u32,
    pub color: bool,
    pub lighting: bool,
}

/// The active controllers chained from a shader property's `controller`.
fn material_anim(nif: &Nif, mut controller: Ref) -> Option<std::sync::Arc<MaterialAnim>> {
    let mut channels = Vec::new();
    let mut seen = 0;
    while let Some(Block::ShaderController(c)) = nif.get(controller) {
        seen += 1;
        if seen > 64 {
            break;
        }
        controller = c.next;
        if !c.timing.active() {
            continue;
        }
        let Some(Block::ValueInterpolator(i)) = nif.get(c.interpolator) else {
            continue;
        };
        let Some(Block::ValueKeys(k)) = nif.get(i.data) else {
            continue;
        };
        if k.keys.is_empty() {
            continue;
        }
        channels.push(MaterialChannel {
            timing: c.timing,
            keys: k.clone(),
            variable: c.variable,
            color: c.color,
            lighting: c.lighting,
        });
    }
    (!channels.is_empty()).then(|| std::sync::Arc::new(MaterialAnim { channels }))
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
            anim: None,
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
        let Block::ControllerSequence(seq) = b else {
            continue;
        };
        let channels: Vec<Channel> = seq
            .blocks
            .iter()
            .filter(|cb| cb.controller_type.contains("TransformController") && !cb.node.is_empty())
            .filter_map(|cb| {
                let Some(Block::TransformInterpolator(i)) = nif.get(cb.interpolator) else {
                    return None;
                };
                let data = match nif.get(i.data) {
                    Some(Block::TransformData(d)) => Some(std::sync::Arc::new((**d).clone())),
                    _ => None,
                };
                Some(Channel {
                    node: cb.node.clone(),
                    interpolator: i.clone(),
                    data,
                })
            })
            .collect();
        if !channels.is_empty() {
            out.push(Sequence {
                name: seq.name.clone(),
                cycle: seq.cycle,
                start: seq.start,
                stop: seq.stop,
                channels,
            });
        }
    }
    out
}

pub fn convert(nif: &Nif) -> CpuModel {
    convert_filtered(nif, &|_| true)
}

/// [`convert`], keeping only the shapes whose names pass `keep`.
pub fn convert_filtered(nif: &Nif, keep: &dyn Fn(&str) -> bool) -> CpuModel {
    convert_split(nif, keep, &Default::default())
}

/// [`convert_filtered`], also drawing the subtrees of the `split` nodes apart
/// (as animated nodes' are): the bodies of a loose object of several.
pub fn convert_split(
    nif: &Nif,
    keep: &dyn Fn(&str) -> bool,
    split: &std::collections::HashSet<String>,
) -> CpuModel {
    let sequences = sequences(nif);
    let animated_nodes: std::collections::HashSet<&str> = sequences
        .iter()
        .flat_map(|s| &s.channels)
        .map(|c| c.node.as_str())
        .chain(split.iter().map(|s| s.as_str()))
        .collect();
    let mut w = Walk {
        meshes: Vec::new(),
        skinned: Vec::new(),
        animated: Vec::new(),
        animated_nodes: &animated_nodes,
        keep,
    };
    for &root in &nif.roots {
        w.walk(nif, Ref(root as i32), Mat4::IDENTITY, 0);
    }
    // Parts within parts (a sign hung from a chain of rings, each a body of
    // its own) are drawn as parts of the model too.
    w.animated = flatten_parts(std::mem::take(&mut w.animated));
    // Rigid models hung from a bone (bows) and trees (skinned to branch bones for
    // wind) may be skinned to bones of their own: drawn in their rest pose.
    if !w.skinned.is_empty() && (has_parent_bone(nif) || is_tree(nif)) {
        let nodes = node_transforms(nif);
        for m in std::mem::take(&mut w.skinned) {
            if let Some(mesh) = rigidify(&m, &nodes) {
                w.meshes.push(mesh);
            }
        }
    }
    let (bound_center, bound_radius) = bounds_of(
        w.meshes
            .iter()
            .map(|m| (m.bound_center, m.bound_radius))
            .chain(w.animated.iter().map(|p| {
                (
                    (p.parent * p.rest).transform_point3(p.model.bound_center),
                    p.model.bound_radius,
                )
            })),
    );
    CpuModel {
        meshes: w.meshes,
        skinned: w.skinned,
        bound_center,
        bound_radius,
        animated: w.animated,
        sequences,
    }
}

/// A node's transform relative to its parent. The engine overwrites the root's with
/// the reference's placement, so a rotated root (the Riverwood signpost's Riften and
/// Helgen arms) doesn't turn the model.
pub fn local_transform(av: &nif::AvObject, depth: u32) -> Mat4 {
    if depth == 0 {
        Mat4::IDENTITY
    } else {
        av.transform.to_mat4()
    }
}

/// The root names the bone it hangs from (`Prn`): a weapon, shield or anim object.
fn has_parent_bone(nif: &Nif) -> bool {
    let Some(root) = nif
        .roots
        .first()
        .and_then(|&r| nif.get(Ref(r as i32)))
        .and_then(|b| b.av())
    else {
        return false;
    };
    root.net.extra_data.iter().any(|&e| matches!(nif.get(e), Some(Block::ExtraData(nif::ExtraData::String { name, .. })) if name == "Prn"))
}

/// The root is a `BSTreeNode` (SpeedTree-exported trees).
fn is_tree(nif: &Nif) -> bool {
    matches!(nif.roots.first().and_then(|&r| nif.get(Ref(r as i32))), Some(Block::Node(n)) if n.kind == NodeKind::Tree)
}

/// Model-space rest transforms of the named nodes.
pub fn node_transforms(nif: &Nif) -> std::collections::HashMap<String, Mat4> {
    fn visit(
        nif: &Nif,
        r: Ref,
        parent: Mat4,
        out: &mut std::collections::HashMap<String, Mat4>,
        depth: u32,
    ) {
        let Some(block) = nif.get(r) else { return };
        let Some(av) = block.av() else { return };
        let world = parent * local_transform(av, depth);
        out.entry(av.net.name.clone()).or_insert(world);
        if let (Block::Node(n), true) = (block, depth < 64) {
            for &c in &n.children {
                visit(nif, c, world, out, depth + 1);
            }
        }
    }
    let mut out = std::collections::HashMap::new();
    for &root in &nif.roots {
        visit(nif, Ref(root as i32), Mat4::IDENTITY, &mut out, 0);
    }
    out
}

/// A skinned mesh posed by its bones' rest transforms, as a static mesh.
fn rigidify(
    m: &CpuSkinnedMesh,
    nodes: &std::collections::HashMap<String, Mat4>,
) -> Option<CpuMesh> {
    if m.vertices.len() > u16::MAX as usize {
        return None;
    }
    let palette: Vec<Mat4> = m
        .bone_names
        .iter()
        .zip(&m.skin_to_bone)
        .map(|(n, s)| nodes.get(n).copied().unwrap_or(Mat4::IDENTITY) * *s)
        .collect();
    let mut g = Geometry::default();
    for v in &m.vertices {
        let mut xf = Mat4::ZERO;
        for k in 0..4 {
            if v.weights[k] > 0.0 {
                xf += palette
                    .get(v.bones[k] as usize)
                    .copied()
                    .unwrap_or(Mat4::IDENTITY)
                    * v.weights[k];
            }
        }
        let dir = |d: [f32; 3]| xf.transform_vector3(Vec3::from(d)).normalize_or_zero();
        g.positions
            .push(xf.transform_point3(Vec3::from(v.position)));
        g.normals.push(dir(v.normal));
        g.tangents.push(dir(v.tangent));
        g.bitangents.push(dir(v.bitangent));
        g.uvs.push(Vec2::from(v.uv));
        g.colors.push(Vec4::from(v.color));
    }
    g.triangles = m
        .indices
        .chunks_exact(3)
        .map(|t| [t[0] as u16, t[1] as u16, t[2] as u16])
        .collect();
    build_mesh(&g, Mat4::IDENTITY, m.material.clone())
}

struct Walk<'a> {
    meshes: Vec<CpuMesh>,
    skinned: Vec<CpuSkinnedMesh>,
    animated: Vec<AnimatedPart>,
    animated_nodes: &'a std::collections::HashSet<&'a str>,
    keep: &'a dyn Fn(&str) -> bool,
}

/// Parts and the parts within them, side by side: the inner ones' parent
/// transforms made model-space.
fn flatten_parts(parts: Vec<AnimatedPart>) -> Vec<AnimatedPart> {
    let mut out = Vec::new();
    for mut p in parts {
        let inner = std::mem::take(&mut p.model.animated);
        let base = p.parent * p.rest;
        out.push(p);
        out.extend(flatten_parts(inner).into_iter().map(|mut c| {
            c.parent = base * c.parent;
            c
        }));
    }
    out
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
    let radius = v
        .iter()
        .map(|(c, r)| c.distance(center) + r)
        .fold(0.0, f32::max);
    (center, radius)
}

impl Walk<'_> {
    fn walk(&mut self, nif: &Nif, r: Ref, parent: Mat4, depth: u32) {
        if depth > 64 {
            return;
        }
        let Some(block) = nif.get(r) else { return };
        let Some(av) = block.av() else { return };
        // Weapons' blood shapes only show once the blade has drawn blood; projectiles'
        // tracers only for the shots the projectile's tracer chance picks.
        let name = av.net.name.to_ascii_lowercase();
        if av.hidden()
            || name.starts_with("editormarker")
            || name.starts_with("blood")
            || name == "tracerroot"
        {
            return;
        }
        // An animated node (not the root): its subtree becomes a separately drawn part.
        if depth > 0
            && matches!(block, Block::Node(_))
            && self.animated_nodes.contains(av.net.name.as_str())
        {
            let mut sub = Walk {
                meshes: Vec::new(),
                skinned: Vec::new(),
                animated: Vec::new(),
                animated_nodes: self.animated_nodes,
                keep: self.keep,
            };
            if let Block::Node(n) = block {
                for &c in &n.children {
                    sub.walk(nif, c, Mat4::IDENTITY, depth + 1);
                }
            }
            let (bound_center, bound_radius) =
                bounds_of(sub.meshes.iter().map(|m| (m.bound_center, m.bound_radius)));
            let model = CpuModel {
                meshes: sub.meshes,
                skinned: sub.skinned,
                bound_center,
                bound_radius,
                animated: sub.animated,
                sequences: Vec::new(),
            };
            self.animated.push(AnimatedPart {
                node: av.net.name.clone(),
                parent,
                rest: av.transform.to_mat4(),
                model,
            });
            return;
        }
        let world = parent * local_transform(av, depth);
        if !matches!(block, Block::Node(_)) && !(self.keep)(&av.net.name) {
            return;
        }
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
    if s.is_empty() {
        None
    } else {
        Some(texture_path(s))
    }
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
            m.anim = material_anim(nif, s.net.controller);
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
            m.anim = material_anim(nif, s.net.controller);
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
        let normal = g
            .normals
            .get(i)
            .map(|v| (nm * *v).normalize_or_zero())
            .unwrap_or(Vec3::Z);
        let tangent = g
            .tangents
            .get(i)
            .map(|v| (rot * *v).normalize_or_zero())
            .unwrap_or(Vec3::X);
        let bitangent = g
            .bitangents
            .get(i)
            .map(|v| (rot * *v).normalize_or_zero())
            .unwrap_or(Vec3::Y);
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
    Some(CpuMesh {
        vertices,
        indices,
        material,
        bound_center: center,
        bound_radius: radius,
    })
}

fn build_skinned(
    nif: &Nif,
    skin: Ref,
    shape_geom: &Geometry,
    name: &str,
    material: MaterialDesc,
) -> Option<CpuSkinnedMesh> {
    let Some(Block::SkinInstance(si)) = nif.get(skin) else {
        return None;
    };
    let Some(Block::SkinData(sd)) = nif.get(si.data) else {
        return None;
    };
    let partition = match nif.get(si.partition) {
        Some(Block::SkinPartition(p)) => Some(p.as_ref()),
        _ => None,
    };
    let bone_names: Vec<String> = si
        .bones
        .iter()
        .map(|b| {
            nif.get(*b)
                .and_then(|b| b.av())
                .map(|a| a.net.name.clone())
                .unwrap_or_default()
        })
        .collect();
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
                let map = |i: usize| -> usize {
                    if part.vertex_map.is_empty() {
                        i
                    } else {
                        part.vertex_map[i] as usize
                    }
                };
                let sse = !p.geometry.uvs.is_empty() || !p.geometry.positions.is_empty();
                for local in 0..part.num_vertices as usize {
                    let gi = map(local);
                    if gi >= n {
                        continue;
                    }
                    let (bi, w) = if sse {
                        (
                            g.bone_indices.get(gi).copied().unwrap_or_default(),
                            g.bone_weights.get(gi).copied().unwrap_or_default(),
                        )
                    } else {
                        (
                            part.bone_indices.get(local).copied().unwrap_or_default(),
                            part.weights.get(local).copied().unwrap_or_default(),
                        )
                    };
                    for k in 0..4 {
                        bones[gi][k] = part.bones.get(bi[k] as usize).copied().unwrap_or(0) as u32;
                        weights[gi][k] = w[k];
                    }
                }
                for t in &part.triangles {
                    // SSE triangles index the global buffer; LE ones the partition's vertex map.
                    let tri = if sse {
                        [t[0] as usize, t[1] as usize, t[2] as usize]
                    } else {
                        [map(t[0] as usize), map(t[1] as usize), map(t[2] as usize)]
                    };
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
    Some(CpuSkinnedMesh {
        vertices,
        indices,
        material,
        bone_names,
        skin_to_bone,
        name: name.to_owned(),
    })
}
