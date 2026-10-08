//! Havok collision blocks (Skyrim / havok 2010 as serialized in NIF v20.2.0.7).
//!
//! All positions are in Havok units; multiply by [`HAVOK_SCALE`] for game units.

use glam::{Mat4, Quat, Vec3, Vec4};

use crate::Result;
use crate::blocks::Ref;
use crate::reader::Reader;

/// Game units per Havok unit in Skyrim.
pub const HAVOK_SCALE: f32 = 69.991_24;

#[derive(Debug, Clone)]
pub struct CollisionObject {
    pub target: Ref,
    pub flags: u16,
    pub body: Ref,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionSystem {
    Dynamic,
    Keyframed,
    Fixed,
    Character,
    Other(u8),
}

#[derive(Debug, Clone)]
pub struct RigidBody {
    pub shape: Ref,
    pub layer: u8,
    pub translation: Vec3,
    pub rotation: Quat,
    pub mass: f32,
    pub friction: f32,
    pub restitution: f32,
    pub motion: MotionSystem,
    /// bhkRigidBodyT: the body transform applies to the shape.
    pub transform_applies: bool,
    /// Constraints joining it to other bodies (ragdolls).
    pub constraints: Vec<Ref>,
}

/// A constraint between two rigid bodies; pivots and axes in Havok units, each in
/// its body's space.
#[derive(Debug, Clone)]
pub struct Constraint {
    pub entities: [Ref; 2],
    pub kind: ConstraintKind,
}

#[derive(Debug, Clone)]
pub enum ConstraintKind {
    /// `bhkRagdollConstraint`: a ball joint with a cone around the twist axis, a plane
    /// limit and a twist range (radians).
    Ragdoll {
        pivot: [Vec3; 2],
        twist: [Vec3; 2],
        plane: [Vec3; 2],
        cone_max: f32,
        plane_min: f32,
        plane_max: f32,
        twist_min: f32,
        twist_max: f32,
    },
    /// `bhkLimitedHingeConstraint`: rotation about one axis within an angle range.
    Hinge {
        pivot: [Vec3; 2],
        axis: [Vec3; 2],
        perp: [Vec3; 2],
        min: f32,
        max: f32,
    },
}

pub(crate) fn constraint(r: &mut Reader, ragdoll: bool) -> Result<Constraint> {
    let n = r.u32()?;
    let a = r.block_ref()?;
    let b = r.block_ref()?;
    debug_assert_eq!(n, 2);
    r.u32()?; // priority
    let v = |r: &mut Reader| -> Result<Vec3> { Ok(r.vec4()?.truncate()) };
    let kind = if ragdoll {
        // Twist, plane, motor, pivot for A then B.
        let (ta, pa, _, pva) = (v(r)?, v(r)?, v(r)?, v(r)?);
        let (tb, pb, _, pvb) = (v(r)?, v(r)?, v(r)?, v(r)?);
        let cone_max = r.f32()?;
        let plane_min = r.f32()?;
        let plane_max = r.f32()?;
        let twist_min = r.f32()?;
        let twist_max = r.f32()?;
        r.f32()?; // max friction
        ConstraintKind::Ragdoll {
            pivot: [pva, pvb],
            twist: [ta, tb],
            plane: [pa, pb],
            cone_max,
            plane_min,
            plane_max,
            twist_min,
            twist_max,
        }
    } else {
        // Axis, perpendicular axes 1 and 2, pivot for A then B.
        let (aa, p1a, _, pva) = (v(r)?, v(r)?, v(r)?, v(r)?);
        let (ab, p1b, _, pvb) = (v(r)?, v(r)?, v(r)?, v(r)?);
        let min = r.f32()?;
        let max = r.f32()?;
        r.f32()?; // max friction
        ConstraintKind::Hinge {
            pivot: [pva, pvb],
            axis: [aa, ab],
            perp: [p1a, p1b],
            min,
            max,
        }
    };
    // The motor's settings follow; nothing here uses them.
    Ok(Constraint {
        entities: [a, b],
        kind,
    })
}

#[derive(Debug, Clone)]
pub enum Shape {
    MoppBvTree {
        shape: Ref,
        scale: f32,
    },
    CompressedMesh {
        data: Ref,
        radius: f32,
        scale: Vec4,
    },
    /// Triangles with their Havok material (`SKY_HAV_MAT_*`) each.
    CompressedMeshData {
        vertices: Vec<Vec3>,
        triangles: Vec<[u32; 3]>,
        materials: Vec<u32>,
    },
    ConvexVertices {
        radius: f32,
        vertices: Vec<Vec3>,
        material: u32,
    },
    Box {
        radius: f32,
        half_extents: Vec3,
        material: u32,
    },
    Sphere {
        radius: f32,
        material: u32,
    },
    Capsule {
        radius: f32,
        p1: Vec3,
        p2: Vec3,
        material: u32,
    },
    List {
        shapes: Vec<Ref>,
        material: u32,
    },
    Transform {
        shape: Ref,
        transform: Mat4,
    },
    PackedTriStrips {
        data: Ref,
        scale: Vec4,
    },
    PackedTriStripsData {
        vertices: Vec<Vec3>,
        triangles: Vec<[u32; 3]>,
        materials: Vec<u32>,
    },
    NiTriStrips {
        strips: Vec<Ref>,
        material: u32,
    },
    ConvexList {
        shapes: Vec<Ref>,
    },
}

pub(crate) fn collision_object(r: &mut Reader) -> Result<CollisionObject> {
    let target = r.block_ref()?;
    let flags = r.u16()?;
    let body = r.block_ref()?;
    Ok(CollisionObject {
        target,
        flags,
        body,
    })
}

pub(crate) fn rigid_body(r: &mut Reader, transform_applies: bool) -> Result<RigidBody> {
    let start = r.pos();
    let shape = r.block_ref()?;
    let layer = r.u8()?;
    r.skip(3)?; // flags + group
    r.skip(20)?; // world object cinfo
    r.skip(24)?; // cinfo preamble (filter, response, callback delay...)
    debug_assert_eq!(r.pos() - start, 52);
    let t = r.vec4()?;
    let q = r.vec4()?;
    r.skip(16 + 16 + 48 + 16)?; // velocities, inertia, center
    let mass = r.f32()?;
    r.skip(4 * 4)?; // linear/angular damping, time factor, gravity factor
    let friction = r.f32()?;
    r.f32()?; // rolling friction
    let restitution = r.f32()?;
    r.skip(12)?; // max linear/angular velocity, penetration depth
    let motion = match r.u8()? {
        1..=3 | 6 => MotionSystem::Dynamic,
        4 => MotionSystem::Keyframed,
        5 => MotionSystem::Fixed,
        7 => MotionSystem::Character,
        o => MotionSystem::Other(o),
    };
    r.skip(3)?; // deactivation, solver deactivation, quality
    r.skip(16)?; // unused / reserved
    let n = r.u32()? as usize;
    let mut constraints = Vec::with_capacity(n);
    for _ in 0..n {
        constraints.push(r.block_ref()?);
    }
    if r.bs_version < 76 {
        r.u32()?;
    } else {
        r.u16()?;
    }
    let rotation = Quat::from_xyzw(q.x, q.y, q.z, q.w);
    let rotation = if rotation.length_squared() > 0.5 {
        rotation.normalize()
    } else {
        Quat::IDENTITY
    };
    Ok(RigidBody {
        shape,
        layer,
        translation: t.truncate(),
        rotation,
        mass,
        friction,
        restitution,
        motion,
        transform_applies,
        constraints,
    })
}

fn compressed_mesh_data(r: &mut Reader) -> Result<Shape> {
    r.skip(4 * 4)?; // bits per index, bits per w index, mask w index, mask index
    let _error = r.f32()?;
    r.skip(32)?; // aabb
    r.u8()?; // welding type
    r.u8()?; // material type
    for _ in 0..3 {
        let n = r.u32()? as usize;
        r.skip(n * 4)?;
    }
    let n_chunk_mat = r.u32()? as usize;
    let mut chunk_materials = Vec::with_capacity(n_chunk_mat);
    for _ in 0..n_chunk_mat {
        chunk_materials.push(r.u32()?);
        r.u32()?; // filter
    }
    let material = |i: u32| chunk_materials.get(i as usize).copied().unwrap_or(0);
    r.u32()?; // named materials
    let n_transforms = r.u32()? as usize;
    let mut transforms = Vec::with_capacity(n_transforms);
    for _ in 0..n_transforms {
        let t = r.vec4()?;
        let q = r.vec4()?;
        transforms.push((
            t.truncate(),
            Quat::from_xyzw(q.x, q.y, q.z, q.w).normalize(),
        ));
    }
    let n_big_verts = r.u32()? as usize;
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();
    let mut materials = Vec::new();
    for _ in 0..n_big_verts {
        vertices.push(r.vec4()?.truncate());
    }
    let n_big_tris = r.u32()? as usize;
    for _ in 0..n_big_tris {
        let (a, b, c) = (r.u16()? as u32, r.u16()? as u32, r.u16()? as u32);
        materials.push(material(r.u32()?));
        r.u16()?; // welding
        triangles.push([a, b, c]);
    }
    let n_chunks = r.u32()? as usize;
    for _ in 0..n_chunks {
        let translation = r.vec4()?.truncate();
        let chunk_material = material(r.u32()?);
        r.u16()?; // reference
        let transform_index = r.u16()? as usize;
        let nv = r.u32()? as usize;
        let mut q = Vec::with_capacity(nv);
        for _ in 0..nv {
            q.push(r.u16()?);
        }
        let ni = r.u32()? as usize;
        let mut idx = Vec::with_capacity(ni);
        for _ in 0..ni {
            idx.push(r.u16()? as u32);
        }
        let ns = r.u32()? as usize;
        let mut strips = Vec::with_capacity(ns);
        for _ in 0..ns {
            strips.push(r.u16()? as usize);
        }
        let nw = r.u32()? as usize;
        r.skip(nw * 2)?;

        let base = vertices.len() as u32;
        let xf = transforms.get(transform_index).copied();
        for v in q.chunks_exact(3) {
            let local = translation + Vec3::new(v[0] as f32, v[1] as f32, v[2] as f32) / 1000.0;
            let p = match xf {
                Some((t, rot)) => rot * local + t,
                None => local,
            };
            vertices.push(p);
        }
        let mut off = 0;
        for &len in &strips {
            for i in 2..len {
                let (a, b, c) = (idx[off + i - 2], idx[off + i - 1], idx[off + i]);
                if a == b || b == c || a == c {
                    continue;
                }
                let t = if i % 2 == 0 { [a, b, c] } else { [a, c, b] };
                triangles.push([base + t[0], base + t[1], base + t[2]]);
            }
            off += len;
        }
        for t in idx[off.min(idx.len())..].chunks_exact(3) {
            triangles.push([base + t[0], base + t[1], base + t[2]]);
        }
        materials.resize(triangles.len(), chunk_material);
    }
    r.u32()?; // convex piece count
    Ok(Shape::CompressedMeshData {
        vertices,
        triangles,
        materials,
    })
}

fn packed_tri_strips_data(r: &mut Reader) -> Result<Shape> {
    let nt = r.u32()? as usize;
    let mut triangles = Vec::with_capacity(nt);
    for _ in 0..nt {
        let t = [r.u16()? as u32, r.u16()? as u32, r.u16()? as u32];
        r.u16()?; // welding info
        triangles.push(t);
    }
    let nv = r.u32()? as usize;
    r.u8()?; // compressed
    let mut vertices = Vec::with_capacity(nv);
    for _ in 0..nv {
        vertices.push(r.vec3()?);
    }
    // Sub shapes: consecutive vertex ranges, each with its material.
    let nsub = r.u16()? as usize;
    let mut subs = Vec::with_capacity(nsub);
    let mut first = 0u32;
    for _ in 0..nsub {
        r.u32()?; // filter
        let n = r.u32()?;
        subs.push((first + n, r.u32()?));
        first += n;
    }
    let materials = triangles
        .iter()
        .map(|t| {
            subs.iter()
                .find(|s| t[0] < s.0)
                .or(subs.last())
                .map_or(0, |s| s.1)
        })
        .collect();
    Ok(Shape::PackedTriStripsData {
        vertices,
        triangles,
        materials,
    })
}

pub(crate) fn parse_shape(ty: &str, r: &mut Reader) -> Result<Option<Shape>> {
    let s = match ty {
        "bhkMoppBvTreeShape" => {
            let shape = r.block_ref()?;
            r.skip(12)?;
            let scale = r.f32()?;
            let size = r.u32()? as usize;
            r.skip(16)?; // origin + scale
            r.u8()?; // build type
            r.skip(size)?;
            Shape::MoppBvTree { shape, scale }
        }
        "bhkCompressedMeshShape" => {
            r.block_ref()?; // target
            r.u32()?; // user data
            let radius = r.f32()?;
            r.f32()?;
            let scale = r.vec4()?;
            r.f32()?;
            r.vec4()?;
            let data = r.block_ref()?;
            Shape::CompressedMesh {
                data,
                radius,
                scale,
            }
        }
        "bhkCompressedMeshShapeData" => compressed_mesh_data(r)?,
        "bhkConvexVerticesShape" => {
            let material = r.u32()?;
            let radius = r.f32()?;
            r.skip(24)?;
            let nv = r.u32()? as usize;
            let mut vertices = Vec::with_capacity(nv);
            for _ in 0..nv {
                vertices.push(r.vec4()?.truncate());
            }
            let nn = r.u32()? as usize;
            r.skip(nn * 16)?;
            Shape::ConvexVertices {
                radius,
                vertices,
                material,
            }
        }
        "bhkBoxShape" => {
            let material = r.u32()?;
            let radius = r.f32()?;
            r.skip(8)?;
            let half_extents = r.vec3()?;
            r.f32()?;
            Shape::Box {
                radius,
                half_extents,
                material,
            }
        }
        "bhkSphereShape" => {
            let material = r.u32()?;
            let radius = r.f32()?;
            Shape::Sphere { radius, material }
        }
        "bhkCapsuleShape" => {
            let material = r.u32()?;
            let radius = r.f32()?;
            r.skip(8)?;
            let p1 = r.vec3()?;
            r.f32()?;
            let p2 = r.vec3()?;
            r.f32()?;
            Shape::Capsule {
                radius,
                p1,
                p2,
                material,
            }
        }
        "bhkListShape" => {
            let shapes = r.ref_list()?;
            let material = r.u32()?;
            r.skip(24)?;
            let n = r.u32()? as usize;
            r.skip(n * 4)?;
            Shape::List { shapes, material }
        }
        "bhkConvexTransformShape" | "bhkTransformShape" => {
            let shape = r.block_ref()?;
            r.u32()?; // material
            r.f32()?; // radius
            r.skip(8)?;
            let mut m = [0f32; 16];
            for v in &mut m {
                *v = r.f32()?;
            }
            Shape::Transform {
                shape,
                transform: Mat4::from_cols_array(&m),
            }
        }
        "bhkPackedNiTriStripsShape" => {
            if r.bs_version <= 34 {
                let n = r.u16()? as usize;
                r.skip(n * 12)?;
            }
            r.u32()?; // user data
            r.u32()?; // unused
            r.f32()?; // radius
            r.u32()?;
            let scale = r.vec4()?;
            r.f32()?;
            r.vec4()?;
            let data = r.block_ref()?;
            Shape::PackedTriStrips { data, scale }
        }
        "hkPackedNiTriStripsData" => packed_tri_strips_data(r)?,
        "bhkNiTriStripsShape" => {
            let material = r.u32()?;
            r.f32()?; // radius
            r.skip(20)?;
            r.u32()?; // grow by
            r.vec4()?; // scale
            let strips = r.ref_list()?;
            let n = r.u32()? as usize;
            r.skip(n * 4)?;
            Shape::NiTriStrips { strips, material }
        }
        "bhkConvexListShape" => {
            let shapes = r.ref_list()?;
            r.u32()?;
            r.f32()?;
            r.skip(4 + 1 + 4 + 4 + 4)?;
            Shape::ConvexList { shapes }
        }
        _ => return Ok(None),
    };
    Ok(Some(s))
}
