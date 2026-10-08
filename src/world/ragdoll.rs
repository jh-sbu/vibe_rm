//! Ragdolls: the rigid bodies and constraints a skeleton's NIF carries
//! (`bhkRigidBody` capsules on bones, `bhkRagdollConstraint` / `bhkLimitedHingeConstraint`
//! between them), simulated when an actor dies.

use std::collections::HashMap;

use glam::{Mat3, Mat4, Quat, Vec3};
use nif::{Block, ConstraintKind, HAVOK_SCALE, Nif, Ref, Shape};

use super::skeleton::Skeleton;

/// A capsule on a bone, in the body's own frame (game units).
#[derive(Debug, Clone)]
pub struct RagdollBody {
    pub bone: usize,
    /// The body's frame relative to its bone (bone model space -> body).
    pub offset: Mat4,
    pub p1: Vec3,
    pub p2: Vec3,
    pub radius: f32,
    pub mass: f32,
}

/// A joint between two bodies, with its frame in each (x: the twist / hinge axis).
#[derive(Debug, Clone)]
pub struct RagdollJoint {
    pub a: usize,
    pub b: usize,
    pub frame_a: (Vec3, Quat),
    pub frame_b: (Vec3, Quat),
    /// Angle limits (radians) about the frame's x, y and z axes; `None` for an axis
    /// that is locked (hinges turn about x only).
    pub limits: [Option<(f32, f32)>; 3],
}

#[derive(Debug, Clone, Default)]
pub struct RagdollDesc {
    pub bodies: Vec<RagdollBody>,
    pub joints: Vec<RagdollJoint>,
}

/// A frame from a pivot and two axes (x, then y made perpendicular to it).
fn frame(pivot: Vec3, x: Vec3, y: Vec3) -> (Vec3, Quat) {
    let x = x.normalize_or(Vec3::X);
    let z = x.cross(y).normalize_or(x.any_orthonormal_vector());
    let y = z.cross(x);
    (
        pivot * HAVOK_SCALE,
        Quat::from_mat3(&Mat3::from_cols(x, y, z)).normalize(),
    )
}

/// The joint a NIF constraint between bodies `a` and `b` makes.
pub(crate) fn joint(kind: &ConstraintKind, a: usize, b: usize) -> RagdollJoint {
    match kind {
        ConstraintKind::Ragdoll {
            pivot,
            twist,
            plane,
            cone_max,
            plane_min,
            plane_max,
            twist_min,
            twist_max,
        } => RagdollJoint {
            a,
            b,
            frame_a: frame(pivot[0], twist[0], plane[0]),
            frame_b: frame(pivot[1], twist[1], plane[1]),
            // Twisting about the twist axis; swinging within the cone, towards
            // the plane axis no further than the plane limits.
            limits: [
                Some((*twist_min, *twist_max)),
                Some((-cone_max, *cone_max)),
                Some((plane_min.max(-cone_max), plane_max.min(*cone_max))),
            ],
        },
        ConstraintKind::Hinge {
            pivot,
            axis,
            perp,
            min,
            max,
        } => RagdollJoint {
            a,
            b,
            frame_a: frame(pivot[0], axis[0], perp[0]),
            frame_b: frame(pivot[1], axis[1], perp[1]),
            limits: [Some((*min, *max)).filter(|_| min.is_finite()), None, None],
        },
    }
}

impl RagdollDesc {
    /// The ragdoll of a skeleton NIF, if it has one.
    pub fn from_nif(nif: &Nif, skeleton: &Skeleton) -> Option<RagdollDesc> {
        let bind = skeleton.model_space(&skeleton.bind_locals());
        let mut desc = RagdollDesc::default();
        let mut by_block: HashMap<i32, usize> = HashMap::new();
        let mut constraints: Vec<Ref> = Vec::new();
        for block in &nif.blocks {
            let Block::Node(n) = block else { continue };
            let Some(Block::CollisionObject(co)) = nif.get(n.av.collision) else {
                continue;
            };
            let Some(Block::RigidBody(rb)) = nif.get(co.body) else {
                continue;
            };
            let Some(Shape::Capsule { radius, p1, p2, .. }) =
                nif.get(rb.shape).and_then(|b| match b {
                    Block::Shape(s) => Some(s.clone()),
                    _ => None,
                })
            else {
                continue;
            };
            let Some(bone) = skeleton.find(&n.av.net.name) else {
                continue;
            };
            let body_model =
                Mat4::from_rotation_translation(rb.rotation, rb.translation * HAVOK_SCALE);
            let offset = bind[bone].inverse() * body_model;
            by_block.insert(co.body.0, desc.bodies.len());
            desc.bodies.push(RagdollBody {
                bone,
                offset,
                p1: p1 * HAVOK_SCALE,
                p2: p2 * HAVOK_SCALE,
                radius: radius * HAVOK_SCALE,
                mass: rb.mass.max(0.5),
            });
            constraints.extend(rb.constraints.iter().copied());
        }
        for c in constraints {
            let Some(Block::Constraint(c)) = nif.get(c) else {
                continue;
            };
            let (Some(&a), Some(&b)) = (
                by_block.get(&c.entities[0].0),
                by_block.get(&c.entities[1].0),
            ) else {
                continue;
            };
            let joint = joint(&c.kind, a, b);
            desc.joints.push(joint);
        }
        (!desc.bodies.is_empty()).then_some(desc)
    }
}

/// Turns a ragdoll's simulated bodies back into a skeleton pose: bones with a body
/// follow it, the others keep their place under their parents as at death.
#[derive(Debug, Clone)]
pub struct RagdollPose {
    /// Bone -> the body that drives it.
    driven: Vec<Option<usize>>,
    /// Each bone relative to its parent at death, and the pose then.
    locals: Vec<Mat4>,
    base: Vec<Mat4>,
    /// World -> the actor's model space (its transform at death).
    inv_actor: Mat4,
    offsets_inv: Vec<Mat4>,
}

impl RagdollPose {
    pub fn new(desc: &RagdollDesc, skeleton: &Skeleton, pose: &[Mat4], actor: Mat4) -> RagdollPose {
        let mut driven = vec![None; skeleton.bones.len()];
        for (i, b) in desc.bodies.iter().enumerate() {
            if let Some(d) = driven.get_mut(b.bone) {
                *d = Some(i);
            }
        }
        let locals = skeleton
            .bones
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let m = pose.get(i).copied().unwrap_or(Mat4::IDENTITY);
                match b.parent {
                    Some(p) => pose.get(p).copied().unwrap_or(Mat4::IDENTITY).inverse() * m,
                    None => m,
                }
            })
            .collect();
        RagdollPose {
            driven,
            locals,
            base: pose.to_vec(),
            inv_actor: actor.inverse(),
            offsets_inv: desc.bodies.iter().map(|b| b.offset.inverse()).collect(),
        }
    }

    /// The model-space pose for bodies now at `bodies` (world, scale included).
    pub fn pose(&self, skeleton: &Skeleton, bodies: &[Mat4]) -> Vec<Mat4> {
        let mut out: Vec<Mat4> = Vec::with_capacity(skeleton.bones.len());
        for (i, b) in skeleton.bones.iter().enumerate() {
            let m = match (
                self.driven[i].and_then(|k| Some((bodies.get(k)?, self.offsets_inv.get(k)?))),
                b.parent,
            ) {
                (Some((w, off)), _) => self.inv_actor * *w * *off,
                (None, Some(p)) if self.has_driven_ancestor(skeleton, i) => out[p] * self.locals[i],
                _ => self.base.get(i).copied().unwrap_or(Mat4::IDENTITY),
            };
            out.push(m);
        }
        out
    }

    fn has_driven_ancestor(&self, skeleton: &Skeleton, mut i: usize) -> bool {
        while let Some(p) = skeleton.bones[i].parent {
            if self.driven[p].is_some() {
                return true;
            }
            i = p;
        }
        false
    }
}
