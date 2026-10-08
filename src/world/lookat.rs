//! Head tracking: a behaviour graph's `BSLookAtModifier` applied to a pose. A
//! chain of bones (spine, neck, head) and the eyes turn towards a target, each
//! within its own limit, easing in and out with the modifier's gains.

use glam::{Quat, Vec3};
use havok::behavior::{LookAt, LookAtBone};
use nif::Transform;

use super::skeleton::Skeleton;

/// Per-actor smoothing: the rotation (model space) each bone of the chain is
/// currently turned by.
#[derive(Default)]
pub struct LookAtState {
    turned: Vec<Quat>,
    /// The target was inside the limits at the last update (hysteresis).
    in_range: bool,
}

/// Fraction to close per update for a per-frame (30 Hz) gain.
fn step(gain: f32, dt: f32) -> f32 {
    1.0 - (1.0 - gain.clamp(0.0, 1.0)).powf(dt * 30.0)
}

/// Turn `locals` (one per NIF bone) towards `target` (model space; `None` eases
/// back). `hk_to_nif` maps the modifier's Havok bone indices to NIF bones.
/// Returns whether the target is outside the limits.
pub fn apply(
    state: &mut LookAtState,
    l: &LookAt,
    hk_to_nif: &[Option<usize>],
    skeleton: &Skeleton,
    locals: &mut [Transform],
    target: Option<Vec3>,
    dt: f32,
) -> bool {
    let bones: Vec<&LookAtBone> = l.bones.iter().chain(&l.eye_bones).collect();
    state.turned.resize(bones.len(), Quat::IDENTITY);
    let target = target.filter(|_| l.look_at_target);
    // Straight ahead is +Y in model space; past the limit (with some hysteresis)
    // the actor stops looking.
    let mut outside = false;
    let target = target.filter(|t| {
        let Some(head) = bones
            .last()
            .and_then(|b| hk_to_nif.get(b.index as usize).copied().flatten())
        else {
            return false;
        };
        let pos = skeleton.model_space(locals)[head].w_axis.truncate();
        let to = (*t - pos).truncate().normalize_or_zero();
        let angle = to.angle_to(glam::Vec2::Y).abs().to_degrees();
        let limit = if state.in_range {
            l.limit_degrees + l.limit_threshold_degrees
        } else {
            l.limit_degrees
        };
        state.in_range = angle <= limit;
        outside = !state.in_range;
        state.in_range || l.continue_outside_limit
    });
    for (i, bone) in bones.iter().enumerate() {
        let Some(Some(b)) = hk_to_nif.get(bone.index as usize).copied() else {
            continue;
        };
        let model = skeleton.model_space(locals);
        let (_, rot, pos) = model[b].to_scale_rotation_translation();
        let desired = match target.filter(|_| bone.enabled) {
            Some(t) => {
                let fwd = (rot * bone.forward).normalize_or_zero();
                let to = (t - pos).normalize_or_zero();
                let full = Quat::from_rotation_arc(fwd, to);
                let (axis, angle) = full.to_axis_angle();
                let limit = bone.limit_degrees.to_radians();
                if angle > limit {
                    Quat::from_axis_angle(axis, limit)
                } else {
                    full
                }
            }
            None => Quat::IDENTITY,
        };
        let gain = match (target.is_some() && bone.enabled, l.use_bone_gains) {
            (true, true) => bone.on_gain,
            (true, false) => l.on_gain,
            (false, true) => bone.off_gain,
            (false, false) => l.off_gain,
        };
        let q = state.turned[i].slerp(desired, step(gain, dt)).normalize();
        state.turned[i] = q;
        if q.angle_between(Quat::IDENTITY) < 1e-4 {
            continue;
        }
        // Turn in model space, then back into the parent's space.
        let parent = skeleton.bones[b].parent.map_or(Quat::IDENTITY, |p| {
            model[p].to_scale_rotation_translation().1
        });
        let local = (parent.inverse() * q * rot).normalize();
        locals[b].rotation = glam::Mat3::from_quat(local);
    }
    outside
}
