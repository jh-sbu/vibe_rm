//! Parsed NIF block types.

use glam::{Mat3, Vec2, Vec3, Vec4};

use crate::Result;
use crate::reader::Reader;

/// Reference to another block (-1 = none).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Ref(pub i32);

impl Ref {
    pub fn index(self) -> Option<usize> {
        if self.0 >= 0 { Some(self.0 as usize) } else { None }
    }
    pub fn is_none(self) -> bool {
        self.0 < 0
    }
}

/// Index into the header string table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StringRef(pub Option<u32>);

#[derive(Debug, Clone, Copy)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Mat3,
    pub scale: f32,
}

impl Default for Transform {
    fn default() -> Self {
        Transform { translation: Vec3::ZERO, rotation: Mat3::IDENTITY, scale: 1.0 }
    }
}

impl Transform {
    pub fn to_mat4(&self) -> glam::Mat4 {
        glam::Mat4::from_translation(self.translation)
            * glam::Mat4::from_mat3(self.rotation * self.scale)
    }
}

#[derive(Debug, Clone, Default)]
pub struct ObjectNet {
    pub name: String,
    pub extra_data: Vec<Ref>,
    pub controller: Ref,
}

#[derive(Debug, Clone, Default)]
pub struct AvObject {
    pub net: ObjectNet,
    pub flags: u32,
    pub transform: Transform,
    pub properties: Vec<Ref>,
    pub collision: Ref,
}

impl AvObject {
    pub fn hidden(&self) -> bool {
        self.flags & 1 != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Plain,
    Fade,
    LeafAnim,
    RootCollision,
    MultiBound,
    Ordered,
    Value,
    Tree,
    /// Only one child is visible: the one at `index`.
    Switch { index: u32 },
    Billboard { mode: u16 },
    /// BSRangeNode family (BSBlastNode, BSDamageStage, BSDebrisNode).
    Range,
    Lod,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub av: AvObject,
    pub kind: NodeKind,
    pub children: Vec<Ref>,
    pub effects: Vec<Ref>,
}

/// Vertex attribute flags of a `BSVertexDesc`.
pub mod vf {
    pub const VERTEX: u16 = 0x1;
    pub const UV: u16 = 0x2;
    pub const UV2: u16 = 0x4;
    pub const NORMAL: u16 = 0x8;
    pub const TANGENT: u16 = 0x10;
    pub const COLORS: u16 = 0x20;
    pub const SKINNED: u16 = 0x40;
    pub const LAND_DATA: u16 = 0x80;
    pub const EYE_DATA: u16 = 0x100;
    pub const INSTANCE: u16 = 0x200;
    pub const FULL_PREC: u16 = 0x400;
}

/// Decoded geometry common to BSTriShape and NiTriShape.
#[derive(Debug, Clone, Default)]
pub struct Geometry {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub tangents: Vec<Vec3>,
    pub bitangents: Vec<Vec3>,
    pub uvs: Vec<Vec2>,
    pub colors: Vec<Vec4>,
    pub bone_weights: Vec<[f32; 4]>,
    pub bone_indices: Vec<[u8; 4]>,
    pub triangles: Vec<[u16; 3]>,
}

#[derive(Debug, Clone)]
pub struct TriShape {
    pub av: AvObject,
    pub bound_center: Vec3,
    pub bound_radius: f32,
    pub skin: Ref,
    pub shader: Ref,
    pub alpha: Ref,
    pub vertex_desc: u64,
    pub geometry: Geometry,
    /// True for BSDynamicTriShape (positions supplied per-frame, e.g. facegen).
    pub dynamic: bool,
}

impl TriShape {
    pub fn vertex_flags(&self) -> u16 {
        ((self.vertex_desc >> 44) & 0xFFF) as u16
    }
}

/// NiTriShape / NiTriStrips (older style geometry referencing a data block).
#[derive(Debug, Clone)]
pub struct NiGeometry {
    pub av: AvObject,
    pub data: Ref,
    pub skin: Ref,
    pub shader: Ref,
    pub alpha: Ref,
}

#[derive(Debug, Clone)]
pub struct LightingShader {
    pub shader_type: u32,
    pub net: ObjectNet,
    pub flags1: u32,
    pub flags2: u32,
    pub uv_offset: Vec2,
    pub uv_scale: Vec2,
    pub texture_set: Ref,
    pub emissive_color: Vec3,
    pub emissive_multiple: f32,
    pub clamp_mode: u32,
    pub alpha: f32,
    pub refraction_strength: f32,
    pub glossiness: f32,
    pub specular_color: Vec3,
    pub specular_strength: f32,
    pub lighting_effect1: f32,
    pub lighting_effect2: f32,
    pub env_map_scale: f32,
    pub skin_tint_color: Vec3,
    pub hair_tint_color: Vec3,
    pub parallax_max_passes: f32,
    pub parallax_scale: f32,
}

/// Skyrim shader property flags.
pub mod sf1 {
    pub const SPECULAR: u32 = 1 << 0;
    pub const SKINNED: u32 = 1 << 1;
    pub const TEMP_REFRACTION: u32 = 1 << 2;
    pub const VERTEX_ALPHA: u32 = 1 << 3;
    pub const GREYSCALE_TO_PALETTE_COLOR: u32 = 1 << 4;
    pub const GREYSCALE_TO_PALETTE_ALPHA: u32 = 1 << 5;
    pub const USE_FALLOFF: u32 = 1 << 6;
    pub const ENVIRONMENT_MAPPING: u32 = 1 << 7;
    pub const RECEIVE_SHADOWS: u32 = 1 << 8;
    pub const CAST_SHADOWS: u32 = 1 << 9;
    pub const FACEGEN_DETAIL_MAP: u32 = 1 << 10;
    pub const PARALLAX: u32 = 1 << 11;
    pub const MODEL_SPACE_NORMALS: u32 = 1 << 12;
    pub const NON_PROJECTIVE_SHADOWS: u32 = 1 << 13;
    pub const LANDSCAPE: u32 = 1 << 14;
    pub const REFRACTION: u32 = 1 << 15;
    pub const FIRE_REFRACTION: u32 = 1 << 16;
    pub const EYE_ENVIRONMENT_MAPPING: u32 = 1 << 17;
    pub const HAIR_SOFT_LIGHTING: u32 = 1 << 18;
    pub const SCREENDOOR_ALPHA_FADE: u32 = 1 << 19;
    pub const LOCALMAP_HIDE_SECRET: u32 = 1 << 20;
    pub const FACEGEN_RGB_TINT: u32 = 1 << 21;
    pub const OWN_EMIT: u32 = 1 << 22;
    pub const PROJECTED_UV: u32 = 1 << 23;
    pub const MULTIPLE_TEXTURES: u32 = 1 << 24;
    pub const REMAPPABLE_TEXTURES: u32 = 1 << 25;
    pub const DECAL: u32 = 1 << 26;
    pub const DYNAMIC_DECAL: u32 = 1 << 27;
    pub const PARALLAX_OCCLUSION: u32 = 1 << 28;
    pub const EXTERNAL_EMITTANCE: u32 = 1 << 29;
    pub const SOFT_EFFECT: u32 = 1 << 30;
    pub const ZBUFFER_TEST: u32 = 1 << 31;
}

pub mod sf2 {
    pub const ZBUFFER_WRITE: u32 = 1 << 0;
    pub const LOD_LANDSCAPE: u32 = 1 << 1;
    pub const LOD_OBJECTS: u32 = 1 << 2;
    pub const NO_FADE: u32 = 1 << 3;
    pub const DOUBLE_SIDED: u32 = 1 << 4;
    pub const VERTEX_COLORS: u32 = 1 << 5;
    pub const GLOW_MAP: u32 = 1 << 6;
    pub const ASSUME_SHADOWMASK: u32 = 1 << 7;
    pub const PACKED_TANGENT: u32 = 1 << 8;
    pub const MULTI_INDEX_SNOW: u32 = 1 << 9;
    pub const VERTEX_LIGHTING: u32 = 1 << 10;
    pub const UNIFORM_SCALE: u32 = 1 << 11;
    pub const FIT_SLOPE: u32 = 1 << 12;
    pub const BILLBOARD: u32 = 1 << 13;
    pub const NO_LOD_LAND_BLEND: u32 = 1 << 14;
    pub const ENVMAP_LIGHT_FADE: u32 = 1 << 15;
    pub const WIREFRAME: u32 = 1 << 16;
    pub const WEAPON_BLOOD: u32 = 1 << 17;
    pub const HIDE_ON_LOCAL_MAP: u32 = 1 << 18;
    pub const PREMULT_ALPHA: u32 = 1 << 19;
    pub const CLOUD_LOD: u32 = 1 << 20;
    pub const ANISOTROPIC_LIGHTING: u32 = 1 << 21;
    pub const NO_TRANSPARENCY_MULTISAMPLING: u32 = 1 << 22;
    pub const UNUSED01: u32 = 1 << 23;
    pub const MULTI_LAYER_PARALLAX: u32 = 1 << 24;
    pub const SOFT_LIGHTING: u32 = 1 << 25;
    pub const RIM_LIGHTING: u32 = 1 << 26;
    pub const BACK_LIGHTING: u32 = 1 << 27;
    pub const UNUSED02: u32 = 1 << 28;
    pub const TREE_ANIM: u32 = 1 << 29;
    pub const EFFECT_LIGHTING: u32 = 1 << 30;
    pub const HD_LOD_OBJECTS: u32 = 1 << 31;
}

#[derive(Debug, Clone)]
pub struct EffectShader {
    pub net: ObjectNet,
    pub flags1: u32,
    pub flags2: u32,
    pub uv_offset: Vec2,
    pub uv_scale: Vec2,
    pub source_texture: String,
    pub clamp_mode: u8,
    pub lighting_influence: u8,
    pub falloff: Vec4,
    pub emissive_color: Vec4,
    pub emissive_multiple: f32,
    pub soft_falloff_depth: f32,
    pub greyscale_texture: String,
}

#[derive(Debug, Clone)]
pub struct AlphaProperty {
    pub net: ObjectNet,
    pub flags: u16,
    pub threshold: u8,
}

impl AlphaProperty {
    pub fn blend_enabled(&self) -> bool {
        self.flags & 1 != 0
    }
    pub fn src_blend(&self) -> u16 {
        (self.flags >> 1) & 0xF
    }
    pub fn dst_blend(&self) -> u16 {
        (self.flags >> 5) & 0xF
    }
    pub fn test_enabled(&self) -> bool {
        self.flags & (1 << 9) != 0
    }
    pub fn test_func(&self) -> u16 {
        (self.flags >> 10) & 0x7
    }
    pub fn no_sort(&self) -> bool {
        self.flags & (1 << 13) != 0
    }
}

#[derive(Debug, Clone)]
pub struct TriShapeData {
    pub geometry: Geometry,
    pub center: Vec3,
    pub radius: f32,
}

#[derive(Debug, Clone)]
pub enum ExtraData {
    String { name: String, value: String },
    Integer { name: String, value: u32 },
    BsxFlags(u32),
    Bound { center: Vec3, dimensions: Vec3 },
    Other { name: String },
}

#[derive(Debug, Clone)]
pub enum Block {
    Node(Node),
    TriShape(TriShape),
    NiTriShape(NiGeometry),
    NiTriStrips(NiGeometry),
    TriShapeData(TriShapeData),
    LightingShader(Box<LightingShader>),
    EffectShader(Box<EffectShader>),
    TextureSet(Vec<String>),
    Alpha(AlphaProperty),
    ExtraData(ExtraData),
    CollisionObject(crate::collision::CollisionObject),
    RigidBody(Box<crate::collision::RigidBody>),
    Shape(crate::collision::Shape),
    Unknown(String),
}

impl Block {
    pub fn av(&self) -> Option<&AvObject> {
        match self {
            Block::Node(n) => Some(&n.av),
            Block::TriShape(t) => Some(&t.av),
            Block::NiTriShape(g) | Block::NiTriStrips(g) => Some(&g.av),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------

fn object_net(r: &mut Reader, lighting_shader: bool) -> Result<(u32, ObjectNet)> {
    let mut shader_type = 0;
    if lighting_shader && r.bs_version >= 83 && r.bs_version <= 139 {
        shader_type = r.u32()?;
    }
    let name = r.string_value()?;
    let extra_data = r.ref_list()?;
    let controller = r.block_ref()?;
    Ok((shader_type, ObjectNet { name, extra_data, controller }))
}

fn av_object(r: &mut Reader) -> Result<AvObject> {
    let (_, net) = object_net(r, false)?;
    let flags = if r.bs_version > 26 { r.u32()? } else { r.u16()? as u32 };
    let translation = r.vec3()?;
    let rotation = r.mat3()?;
    let scale = r.f32()?;
    let properties = if r.bs_version <= 34 { r.ref_list()? } else { Vec::new() };
    let collision = r.block_ref()?;
    Ok(AvObject { net, flags, transform: Transform { translation, rotation, scale }, properties, collision })
}

fn node(r: &mut Reader, kind: NodeKind) -> Result<Node> {
    let av = av_object(r)?;
    let children = r.ref_list()?;
    let effects = if r.bs_version < 130 { r.ref_list()? } else { Vec::new() };
    Ok(Node { av, kind, children, effects })
}

fn bs_tri_shape(r: &mut Reader, ty: &str) -> Result<TriShape> {
    let av = av_object(r)?;
    let bound_center = r.vec3()?;
    let bound_radius = r.f32()?;
    if r.bs_version >= 155 {
        r.skip(24)?; // bound min/max
    }
    let skin = r.block_ref()?;
    let shader = r.block_ref()?;
    let alpha = r.block_ref()?;
    let vertex_desc = r.u64()?;
    let num_triangles = if r.bs_version >= 130 { r.u32()? as usize } else { r.u16()? as usize };
    let num_vertices = r.u16()? as usize;
    let data_size = r.u32()? as usize;
    let mut geometry = Geometry::default();
    if data_size > 0 {
        read_vertex_data(r, vertex_desc, num_vertices, &mut geometry)?;
        geometry.triangles.reserve(num_triangles);
        for _ in 0..num_triangles {
            geometry.triangles.push([r.u16()?, r.u16()?, r.u16()?]);
        }
    }
    if r.bs_version == 100 {
        let particle_size = r.u32()? as usize;
        if particle_size > 0 {
            r.skip(num_vertices * 12 + num_triangles * 6)?;
        }
    }
    let mut dynamic = false;
    match ty {
        "BSDynamicTriShape" => {
            dynamic = true;
            let size = r.u32()? as usize;
            let n = size / 16;
            let mut pos = Vec::with_capacity(n);
            for _ in 0..n {
                let v = r.vec4()?;
                pos.push(v.truncate());
            }
            if pos.len() == geometry.positions.len() || geometry.positions.is_empty() {
                geometry.positions = pos;
            }
        }
        "BSMeshLODTriShape" => {
            r.skip(12)?;
        }
        "BSSubIndexTriShape" => {
            if r.bs_version >= 130 {
                // Fallout 4 segment data; not needed for rendering.
                r.set_pos(r.data.len());
            } else {
                let n = r.u32()? as usize;
                r.skip(n * 9)?;
            }
        }
        _ => {}
    }
    Ok(TriShape { av, bound_center, bound_radius, skin, shader, alpha, vertex_desc, geometry, dynamic })
}

/// Decode packed `BSVertexData` (SSE layout; also used for the skin partition copy).
pub(crate) fn read_vertex_data(r: &mut Reader, desc: u64, n: usize, g: &mut Geometry) -> Result<()> {
    let stride = ((desc & 0xF) * 4) as usize;
    let flags = ((desc >> 44) & 0xFFF) as u16;
    let off = |shift: u32| (((desc >> shift) & 0xF) * 4) as usize;
    let (uv_off, n_off, t_off, c_off, s_off) = (off(8), off(16), off(20), off(24), off(28));
    let full_prec = r.bs_version < 130 || flags & vf::FULL_PREC != 0;
    let data = r.bytes(stride * n)?;
    let f = |b: &[u8], o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let h = |b: &[u8], o: usize| crate::reader::half_to_f32(u16::from_le_bytes([b[o], b[o + 1]]));
    let nb = |v: u8| v as f32 / 127.5 - 1.0;
    let has = |bit: u16| flags & bit != 0;
    for i in 0..n {
        let v = &data[i * stride..(i + 1) * stride];
        let mut bitangent = Vec3::ZERO;
        if has(vf::VERTEX) {
            if full_prec {
                g.positions.push(Vec3::new(f(v, 0), f(v, 4), f(v, 8)));
                bitangent.x = f(v, 12);
            } else {
                g.positions.push(Vec3::new(h(v, 0), h(v, 2), h(v, 4)));
                bitangent.x = h(v, 6);
            }
        }
        if has(vf::UV) {
            g.uvs.push(Vec2::new(h(v, uv_off), h(v, uv_off + 2)));
        }
        if has(vf::NORMAL) {
            g.normals.push(Vec3::new(nb(v[n_off]), nb(v[n_off + 1]), nb(v[n_off + 2])));
            bitangent.y = nb(v[n_off + 3]);
            if has(vf::TANGENT) {
                g.tangents.push(Vec3::new(nb(v[t_off]), nb(v[t_off + 1]), nb(v[t_off + 2])));
                bitangent.z = nb(v[t_off + 3]);
                g.bitangents.push(bitangent);
            }
        }
        if has(vf::COLORS) {
            g.colors.push(Vec4::new(
                v[c_off] as f32 / 255.0,
                v[c_off + 1] as f32 / 255.0,
                v[c_off + 2] as f32 / 255.0,
                v[c_off + 3] as f32 / 255.0,
            ));
        }
        if has(vf::SKINNED) {
            g.bone_weights.push([h(v, s_off), h(v, s_off + 2), h(v, s_off + 4), h(v, s_off + 6)]);
            g.bone_indices.push([v[s_off + 8], v[s_off + 9], v[s_off + 10], v[s_off + 11]]);
        }
    }
    Ok(())
}

fn ni_geometry(r: &mut Reader) -> Result<NiGeometry> {
    let av = av_object(r)?;
    let data = r.block_ref()?;
    let skin = r.block_ref()?;
    let num_materials = r.u32()? as usize;
    r.skip(num_materials * 8)?; // names + extra data
    r.i32()?; // active material
    r.u8()?; // material needs update
    let (shader, alpha) = if r.bs_version > 34 {
        (r.block_ref()?, r.block_ref()?)
    } else {
        (Ref(-1), Ref(-1))
    };
    Ok(NiGeometry { av, data, skin, shader, alpha })
}

fn geometry_data(r: &mut Reader) -> Result<(Geometry, Vec3, f32)> {
    let mut g = Geometry::default();
    r.i32()?; // group id
    let n = r.u16()? as usize;
    r.u8()?; // keep flags
    r.u8()?; // compress flags
    if r.bool()? {
        for _ in 0..n {
            g.positions.push(r.vec3()?);
        }
    }
    let vector_flags = r.u16()?;
    if r.bs_version > 34 {
        r.u32()?; // material crc
    }
    let has_normals = r.bool()?;
    if has_normals {
        for _ in 0..n {
            g.normals.push(r.vec3()?);
        }
        if vector_flags & 0x1000 != 0 {
            for _ in 0..n {
                g.tangents.push(r.vec3()?);
            }
            for _ in 0..n {
                g.bitangents.push(r.vec3()?);
            }
        }
    }
    let center = r.vec3()?;
    let radius = r.f32()?;
    if r.bool()? {
        for _ in 0..n {
            g.colors.push(r.vec4()?);
        }
    }
    let num_uv = if r.bs_version > 0 { vector_flags & 1 } else { vector_flags & 0x3F };
    for set in 0..num_uv {
        for _ in 0..n {
            let uv = r.vec2()?;
            if set == 0 {
                g.uvs.push(uv);
            }
        }
    }
    r.u16()?; // consistency
    r.block_ref()?; // additional data
    Ok((g, center, radius))
}

fn tri_shape_data(r: &mut Reader) -> Result<TriShapeData> {
    let (mut geometry, center, radius) = geometry_data(r)?;
    let num_tris = r.u16()? as usize;
    let _num_points = r.u32()?;
    if r.bool()? {
        for _ in 0..num_tris {
            geometry.triangles.push([r.u16()?, r.u16()?, r.u16()?]);
        }
    }
    let groups = r.u16()?;
    for _ in 0..groups {
        let c = r.u16()? as usize;
        r.skip(c * 2)?;
    }
    Ok(TriShapeData { geometry, center, radius })
}

fn tri_strips_data(r: &mut Reader) -> Result<TriShapeData> {
    let (mut geometry, center, radius) = geometry_data(r)?;
    let _num_tris = r.u16()?;
    let num_strips = r.u16()? as usize;
    let mut lens = Vec::with_capacity(num_strips);
    for _ in 0..num_strips {
        lens.push(r.u16()? as usize);
    }
    if r.bool()? {
        for len in lens {
            let mut pts = Vec::with_capacity(len);
            for _ in 0..len {
                pts.push(r.u16()?);
            }
            for i in 2..len {
                let (a, b, c) = (pts[i - 2], pts[i - 1], pts[i]);
                if a == b || b == c || a == c {
                    continue;
                }
                geometry.triangles.push(if i % 2 == 0 { [a, b, c] } else { [a, c, b] });
            }
        }
    }
    Ok(TriShapeData { geometry, center, radius })
}

fn lighting_shader(r: &mut Reader) -> Result<LightingShader> {
    let (shader_type, net) = object_net(r, true)?;
    let flags1 = r.u32()?;
    let flags2 = r.u32()?;
    let uv_offset = r.vec2()?;
    let uv_scale = r.vec2()?;
    let texture_set = r.block_ref()?;
    let emissive_color = r.vec3()?;
    let emissive_multiple = r.f32()?;
    let clamp_mode = r.u32()?;
    let alpha = r.f32()?;
    let refraction_strength = r.f32()?;
    let glossiness = r.f32()?;
    let specular_color = r.vec3()?;
    let specular_strength = r.f32()?;
    let lighting_effect1 = r.f32()?;
    let lighting_effect2 = r.f32()?;
    let mut s = LightingShader {
        shader_type,
        net,
        flags1,
        flags2,
        uv_offset,
        uv_scale,
        texture_set,
        emissive_color,
        emissive_multiple,
        clamp_mode,
        alpha,
        refraction_strength,
        glossiness,
        specular_color,
        specular_strength,
        lighting_effect1,
        lighting_effect2,
        env_map_scale: 0.0,
        skin_tint_color: Vec3::ONE,
        hair_tint_color: Vec3::ONE,
        parallax_max_passes: 0.0,
        parallax_scale: 0.0,
    };
    match shader_type {
        1 => s.env_map_scale = r.f32()?,
        5 => s.skin_tint_color = r.vec3()?,
        6 => s.hair_tint_color = r.vec3()?,
        7 => {
            s.parallax_max_passes = r.f32()?;
            s.parallax_scale = r.f32()?;
        }
        11 => r.skip(4 + 4 + 8 + 4)?,
        14 => r.skip(16)?,
        16 => r.skip(4 + 12 + 12)?,
        _ => {}
    }
    Ok(s)
}

fn effect_shader(r: &mut Reader) -> Result<EffectShader> {
    let (_, net) = object_net(r, false)?;
    let flags1 = r.u32()?;
    let flags2 = r.u32()?;
    let uv_offset = r.vec2()?;
    let uv_scale = r.vec2()?;
    let source_texture = r.sized_string()?;
    let clamp_mode = r.u8()?;
    let lighting_influence = r.u8()?;
    r.u8()?; // env map min lod
    r.u8()?;
    let falloff = r.vec4()?;
    let emissive_color = r.vec4()?;
    let emissive_multiple = r.f32()?;
    let soft_falloff_depth = r.f32()?;
    let greyscale_texture = r.sized_string()?;
    Ok(EffectShader {
        net,
        flags1,
        flags2,
        uv_offset,
        uv_scale,
        source_texture,
        clamp_mode,
        lighting_influence,
        falloff,
        emissive_color,
        emissive_multiple,
        soft_falloff_depth,
        greyscale_texture,
    })
}

pub(crate) fn parse_block(ty: &str, r: &mut Reader) -> Result<Option<Block>> {
    let node_kind = match ty {
        "NiNode" | "BSFaceGenNiNode" | "NiBone" | "AvoidNode" => {
            Some(NodeKind::Plain)
        }
        "BSFadeNode" => Some(NodeKind::Fade),
        "BSLeafAnimNode" => Some(NodeKind::LeafAnim),
        "RootCollisionNode" => Some(NodeKind::RootCollision),
        _ => None,
    };
    if let Some(kind) = node_kind {
        return Ok(Some(Block::Node(node(r, kind)?)));
    }
    let b = match ty {
        "BSMultiBoundNode" => {
            let n = node(r, NodeKind::MultiBound)?;
            r.block_ref()?;
            if r.bs_version >= 83 {
                r.u32()?;
            }
            Block::Node(n)
        }
        "BSMasterParticleSystem" => {
            let n = node(r, NodeKind::Plain)?;
            r.u16()?;
            r.ref_list()?;
            Block::Node(n)
        }
        "BSOrderedNode" => {
            let n = node(r, NodeKind::Ordered)?;
            r.skip(17)?;
            Block::Node(n)
        }
        "BSValueNode" => {
            let n = node(r, NodeKind::Value)?;
            r.skip(5)?;
            Block::Node(n)
        }
        "BSTreeNode" => {
            let n = node(r, NodeKind::Tree)?;
            r.ref_list()?;
            r.ref_list()?;
            Block::Node(n)
        }
        "NiSwitchNode" => {
            let mut n = node(r, NodeKind::Plain)?;
            r.u16()?;
            let index = r.u32()?;
            n.kind = NodeKind::Switch { index };
            Block::Node(n)
        }
        "NiBillboardNode" => {
            let mut n = node(r, NodeKind::Plain)?;
            let mode = r.u16()?;
            n.kind = NodeKind::Billboard { mode };
            Block::Node(n)
        }
        "BSRangeNode" | "BSBlastNode" | "BSDamageStage" | "BSDebrisNode" => {
            let n = node(r, NodeKind::Range)?;
            r.skip(3)?;
            Block::Node(n)
        }
        "NiLODNode" => {
            let mut n = node(r, NodeKind::Plain)?;
            // NiSwitchNode part, then LOD data ref.
            r.u16()?;
            r.u32()?;
            r.block_ref()?;
            n.kind = NodeKind::Lod;
            Block::Node(n)
        }
        "BSLODTriShape" => {
            let g = ni_geometry(r)?;
            r.skip(12)?;
            Block::NiTriShape(g)
        }
        "BSTriShape" | "BSDynamicTriShape" | "BSMeshLODTriShape" | "BSSubIndexTriShape" => Block::TriShape(bs_tri_shape(r, ty)?),
        "NiTriShape" => Block::NiTriShape(ni_geometry(r)?),
        "NiTriStrips" => Block::NiTriStrips(ni_geometry(r)?),
        "NiTriShapeData" => Block::TriShapeData(tri_shape_data(r)?),
        "NiTriStripsData" => Block::TriShapeData(tri_strips_data(r)?),
        "BSLightingShaderProperty" => Block::LightingShader(Box::new(lighting_shader(r)?)),
        "BSEffectShaderProperty" => Block::EffectShader(Box::new(effect_shader(r)?)),
        "BSShaderTextureSet" => {
            let n = r.u32()? as usize;
            let mut v = Vec::with_capacity(n);
            for _ in 0..n {
                v.push(r.sized_string()?);
            }
            Block::TextureSet(v)
        }
        "NiAlphaProperty" => {
            let (_, net) = object_net(r, false)?;
            let flags = r.u16()?;
            let threshold = r.u8()?;
            Block::Alpha(AlphaProperty { net, flags, threshold })
        }
        "NiStringExtraData" => {
            let name = r.string_value()?;
            let value = r.string_value()?;
            Block::ExtraData(ExtraData::String { name, value })
        }
        "NiIntegerExtraData" => {
            let name = r.string_value()?;
            let value = r.u32()?;
            Block::ExtraData(ExtraData::Integer { name, value })
        }
        "BSXFlags" => {
            r.string_value()?;
            Block::ExtraData(ExtraData::BsxFlags(r.u32()?))
        }
        "BSBound" => {
            r.string_value()?;
            let center = r.vec3()?;
            let dimensions = r.vec3()?;
            Block::ExtraData(ExtraData::Bound { center, dimensions })
        }
        "bhkCollisionObject" | "bhkSPCollisionObject" | "bhkBlendCollisionObject" | "bhkPCollisionObject" => {
            let c = crate::collision::collision_object(r)?;
            if ty == "bhkBlendCollisionObject" {
                r.skip(8)?; // heir gain, vel gain
            }
            Block::CollisionObject(c)
        }
        "bhkRigidBody" => Block::RigidBody(Box::new(crate::collision::rigid_body(r, false)?)),
        "bhkRigidBodyT" => Block::RigidBody(Box::new(crate::collision::rigid_body(r, true)?)),
        _ => match crate::collision::parse_shape(ty, r)? {
            Some(s) => Block::Shape(s),
            None => return Ok(None),
        },
    };
    Ok(Some(b))
}
