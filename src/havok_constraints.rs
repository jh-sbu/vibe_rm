//! `Game.AddHavokBallAndSocketConstraint` and `RemoveHavokConstraints`:
//! scripts joining two references' rigid bodies, named by node (a dead
//! actor's ragdoll bone, a loose object's body), as the hanging corpses'
//! attach points do (`research/pex-natives/cell-batch17-fog-constraints.md`).

use esp::FormId;
use glam::{Mat4, Vec3};
use rapier3d::prelude::{ImpulseJointHandle, RigidBodyHandle};

use crate::engine::Engine;

/// A joint a script made.
pub struct ScriptConstraint {
    pub a: (FormId, String),
    pub b: (FormId, String),
    joint: ImpulseJointHandle,
    /// A fixed body made for a reference with no simulated body there.
    anchor: Option<RigidBodyHandle>,
}

/// Where a joint holds a reference: its simulated body, or a fixed point.
enum Hold {
    Body(RigidBodyHandle),
    Fixed(Mat4),
}

impl Engine {
    /// The rigid body of `r` at `node`: a dead actor's ragdoll body on that
    /// bone, a loose object's body on that node; else, for a placed object
    /// with that node in its model, a fixed point there. Living actors have
    /// no ragdoll here, so nothing.
    fn hold_of(&self, r: FormId, node: &str) -> Option<Hold> {
        if let Some(a) = self.actor_ref(r) {
            let (ragdoll, pose) = a.ragdoll.as_ref()?;
            let bone = a.skeleton.by_name.get(node).copied().or_else(|| {
                a.skeleton
                    .bones
                    .iter()
                    .position(|b| b.name.eq_ignore_ascii_case(node))
            })?;
            let body = pose.body_of_bone(bone)?;
            return ragdoll.bodies.get(body).copied().map(Hold::Body);
        }
        let loose = self
            .cells
            .values()
            .flat_map(|rt| &rt.loose)
            .find(|l| l.ref_id == r);
        if let Some(l) = loose {
            let body = l
                .bodies
                .iter()
                .find(|b| b.node.eq_ignore_ascii_case(node))
                .or_else(|| l.bodies.get(l.root))?;
            return Some(Hold::Body(body.handle));
        }
        let (world, local) = self.ref_node_transform(r, node)?;
        Some(Hold::Fixed(world * local))
    }

    /// `AddHavokBallAndSocketConstraint`: true when both references have a
    /// body (or fixed point) at their nodes and they are joined.
    pub fn add_ball_socket_constraint(
        &mut self,
        a: (FormId, &str),
        b: (FormId, &str),
        offset_a: Vec3,
        offset_b: Vec3,
    ) -> bool {
        let (Some(ha), Some(hb)) = (self.hold_of(a.0, a.1), self.hold_of(b.0, b.1)) else {
            log::debug!("constraint {}:{} - {}:{}: no body", a.0, a.1, b.0, b.1);
            return false;
        };
        let mut anchor = None;
        let mut body = |e: &mut Engine, h: Hold| match h {
            Hold::Body(b) => b,
            Hold::Fixed(at) => {
                let fixed = e.physics.add_anchor(at);
                anchor = Some(fixed);
                fixed
            }
        };
        let (ba, bb) = (body(self, ha), body(self, hb));
        if ba == bb {
            return false;
        }
        let joint = self.physics.add_ball_socket(ba, offset_a, bb, offset_b);
        self.havok_constraints.push(ScriptConstraint {
            a: (a.0, a.1.to_owned()),
            b: (b.0, b.1.to_owned()),
            joint,
            anchor,
        });
        true
    }

    /// `RemoveHavokConstraints`: the joints scripts made between those
    /// nodes, either way round. True if one was removed.
    pub fn remove_havok_constraints(&mut self, a: (FormId, &str), b: (FormId, &str)) -> bool {
        let same =
            |x: &(FormId, String), y: (FormId, &str)| x.0 == y.0 && x.1.eq_ignore_ascii_case(y.1);
        let mut removed = false;
        let mut keep = Vec::new();
        for c in std::mem::take(&mut self.havok_constraints) {
            let matches = (same(&c.a, a) && same(&c.b, b)) || (same(&c.a, b) && same(&c.b, a));
            if matches {
                removed |= self.physics.has_joint(c.joint);
                self.physics.remove_joint(c.joint, c.anchor);
            } else if self.physics.has_joint(c.joint) {
                keep.push(c);
            } else if let Some(anchor) = c.anchor {
                // Its other body went with its cell.
                self.physics.remove_joint(c.joint, Some(anchor));
            }
        }
        self.havok_constraints = keep;
        removed
    }
}

#[cfg(test)]
mod tests {
    use glam::{Mat4, Vec3};
    use rapier3d::prelude::*;

    #[test]
    fn a_ball_socket_joint_keeps_a_body_hanging_from_its_anchor() {
        let mut p = crate::physics::Physics::new();
        let anchor = p.add_anchor(Mat4::from_translation(Vec3::new(0.0, 0.0, 500.0)));
        let body = p
            .world
            .insert_body(RigidBodyBuilder::dynamic().translation(Vec3::new(0.0, 0.0, 450.0)));
        p.world
            .insert_collider(ColliderBuilder::ball(5.0).mass(10.0).build(), Some(body));
        let joint = p.add_ball_socket(anchor, Vec3::ZERO, body, Vec3::new(0.0, 0.0, 50.0));
        for _ in 0..120 {
            p.step(1.0 / 60.0);
        }
        let z = p.world.bodies.get(body).unwrap().translation().z;
        assert!((z - 450.0).abs() < 5.0, "hangs at {z}");
        p.remove_joint(joint, Some(anchor));
        assert!(!p.has_joint(joint));
        for _ in 0..60 {
            p.step(1.0 / 60.0);
        }
        assert!(p.world.bodies.get(body).unwrap().translation().z < 400.0);
    }
}
