//! Conversion of NIF Havok collision data into physics shapes (game units).

use std::sync::Arc;

use glam::{Mat4, Quat, Vec3};
use nif::{Block, HAVOK_SCALE, Nif, Ref, Shape};
use rapier3d::prelude::{Pose, SharedShape};

/// One collision part in model space.
#[derive(Clone)]
pub struct CollisionPart {
    pub transform: Mat4,
    pub shape: ShapeDesc,
    pub layer: u8,
    pub dynamic: bool,
    /// The keyframe-animated node this part moves with (a door leaf), if any.
    pub node: Option<String>,
    /// Havok materials (`SKY_HAV_MAT_*`): one per triangle of a mesh, or one for
    /// the whole part.
    pub materials: Arc<[u32]>,
}

/// Unscaled shape description; kept so instances with a scale can rebuild it.
#[derive(Clone)]
pub enum ShapeDesc {
    TriMesh { vertices: Vec<Vec3>, triangles: Vec<[u32; 3]> },
    Convex(Vec<Vec3>),
    Box(Vec3),
    Sphere(f32),
    Capsule(Vec3, Vec3, f32),
}

impl ShapeDesc {
    pub fn build(&self, scale: f32) -> Option<SharedShape> {
        let s = scale;
        match self {
            ShapeDesc::TriMesh { vertices, triangles } => {
                let v: Vec<Vec3> = vertices.iter().map(|p| *p * s).collect();
                SharedShape::trimesh(v, triangles.clone()).ok()
            }
            ShapeDesc::Convex(points) => {
                let p: Vec<Vec3> = points.iter().map(|p| *p * s).collect();
                SharedShape::convex_hull(&p)
            }
            ShapeDesc::Box(h) => Some(SharedShape::cuboid(h.x * s, h.y * s, h.z * s)),
            ShapeDesc::Sphere(r) => Some(SharedShape::ball(r * s)),
            ShapeDesc::Capsule(a, b, r) => Some(SharedShape::capsule(*a * s, *b * s, r * s)),
        }
    }
}

#[derive(Clone, Default)]
pub struct CollisionModel {
    pub parts: Vec<CollisionPart>,
}

pub fn from_nif(nif: &Nif) -> Option<CollisionModel> {
    let mut m = CollisionModel::default();
    let animated = nif.animated_nodes();
    for &root in &nif.roots {
        walk(nif, Ref(root as i32), Mat4::IDENTITY, &mut m, 0, &animated, None);
    }
    if m.parts.is_empty() { None } else { Some(m) }
}

fn walk(
    nif: &Nif,
    r: Ref,
    parent: Mat4,
    out: &mut CollisionModel,
    depth: u32,
    animated: &std::collections::HashSet<String>,
    mut node: Option<String>,
) {
    if depth > 64 {
        return;
    }
    let Some(block) = nif.get(r) else { return };
    let Some(av) = block.av() else { return };
    if depth > 0 && animated.contains(&av.net.name) {
        node = Some(av.net.name.clone());
    }
    let world = parent * crate::render::model::local_transform(av, depth);
    if let Some(Block::CollisionObject(co)) = nif.get(av.collision)
        && let Some(Block::RigidBody(rb)) = nif.get(co.body)
    {
        let mut xf = world;
        if rb.transform_applies {
            xf = xf * Mat4::from_rotation_translation(rb.rotation, rb.translation * HAVOK_SCALE);
        }
        let dynamic = matches!(rb.motion, nif::MotionSystem::Dynamic);
        let before = out.parts.len();
        add_shape(nif, rb.shape, xf, out, 0);
        for p in &mut out.parts[before..] {
            p.layer = rb.layer;
            p.dynamic = dynamic;
            p.node.clone_from(&node);
        }
    }
    if let Block::Node(n) = block {
        for &c in &n.children {
            walk(nif, c, world, out, depth + 1, animated, node.clone());
        }
    }
}

fn push(out: &mut CollisionModel, transform: Mat4, shape: ShapeDesc, materials: &[u32]) {
    out.parts.push(CollisionPart { transform, shape, layer: 0, dynamic: false, node: None, materials: materials.into() });
}

fn add_shape(nif: &Nif, r: Ref, xf: Mat4, out: &mut CollisionModel, depth: u32) {
    if depth > 16 {
        return;
    }
    let Some(Block::Shape(s)) = nif.get(r) else { return };
    let h = HAVOK_SCALE;
    match s {
        Shape::MoppBvTree { shape, .. } => add_shape(nif, *shape, xf, out, depth + 1),
        Shape::CompressedMesh { data, .. } => {
            if let Some(Block::Shape(Shape::CompressedMeshData { vertices, triangles, materials })) = nif.get(*data)
                && !triangles.is_empty()
            {
                let v = vertices.iter().map(|p| *p * h).collect();
                push(out, xf, ShapeDesc::TriMesh { vertices: v, triangles: triangles.clone() }, materials);
            }
        }
        Shape::PackedTriStrips { data, scale } => {
            if let Some(Block::Shape(Shape::PackedTriStripsData { vertices, triangles, materials })) = nif.get(*data)
                && !triangles.is_empty()
            {
                let sc = if scale.x > 0.0 { scale.x } else { 1.0 };
                let v = vertices.iter().map(|p| *p * h * sc).collect();
                push(out, xf, ShapeDesc::TriMesh { vertices: v, triangles: triangles.clone() }, materials);
            }
        }
        Shape::NiTriStrips { strips, material } => {
            // Unlike the Havok shapes, NiTriStripsData vertices are already in game units.
            for s in strips {
                if let Some(Block::TriShapeData(d)) = nif.get(*s) {
                    let v = d.geometry.positions.clone();
                    let t = d.geometry.triangles.iter().map(|t| [t[0] as u32, t[1] as u32, t[2] as u32]).collect();
                    push(out, xf, ShapeDesc::TriMesh { vertices: v, triangles: t }, &[*material]);
                }
            }
        }
        Shape::ConvexVertices { vertices, material, .. } => {
            if vertices.len() >= 4 {
                push(out, xf, ShapeDesc::Convex(vertices.iter().map(|p| *p * h).collect()), &[*material]);
            }
        }
        Shape::Box { half_extents, material, .. } => push(out, xf, ShapeDesc::Box(*half_extents * h), &[*material]),
        Shape::Sphere { radius, material } => push(out, xf, ShapeDesc::Sphere(radius * h), &[*material]),
        Shape::Capsule { radius, p1, p2, material } => push(out, xf, ShapeDesc::Capsule(*p1 * h, *p2 * h, radius * h), &[*material]),
        Shape::List { shapes, .. } | Shape::ConvexList { shapes } => {
            for &c in shapes {
                add_shape(nif, c, xf, out, depth + 1);
            }
        }
        Shape::Transform { shape, transform } => {
            let mut t = *transform;
            t.w_axis = (t.w_axis.truncate() * h).extend(1.0);
            add_shape(nif, *shape, xf * t, out, depth + 1);
        }
        Shape::CompressedMeshData { .. } | Shape::PackedTriStripsData { .. } => {}
    }
}

/// Split a model-space transform into a rapier pose plus uniform scale.
pub fn decompose(m: Mat4) -> (Pose, f32) {
    let (scale, rot, trans) = m.to_scale_rotation_translation();
    let s = (scale.x + scale.y + scale.z) / 3.0;
    let rot = if rot.is_finite() { rot.normalize() } else { Quat::IDENTITY };
    (Pose::from_parts(trans, rot), s)
}
