//! Foot placement: a character's `hkbFootIkDriverInfo` applied to a pose. The
//! body drops to the lower foot's ground, the other legs bend (two-bone IK) to
//! put their ankles on theirs, and planted feet tilt with the ground.

use glam::{Quat, Vec3};
use havok::behavior::FootIk;
use nif::Transform;

use super::skeleton::Skeleton;

/// Per-actor smoothing of the ground offsets.
#[derive(Default)]
pub struct FootIkState {
    /// Each leg's ground offset (model space) being eased towards.
    offsets: Vec<f32>,
}

/// Fraction to close per update for a per-frame (30 Hz) gain.
fn step(gain: f32, dt: f32) -> f32 {
    1.0 - (1.0 - gain).powf(dt * 30.0)
}

fn rotation(m: &glam::Mat4) -> Quat {
    m.to_scale_rotation_translation().1
}

/// Place the feet of `locals` (one per NIF bone) on `ground`: per leg, the ground
/// height under the ankle and its normal (model space), `None` where nothing was
/// hit or the IK is off. Returns the ankles (model space, before placement) to
/// cast the next rays from.
pub fn apply(
    state: &mut FootIkState,
    ik: &FootIk,
    hk_to_nif: &[Option<usize>],
    skeleton: &Skeleton,
    locals: &mut [Transform],
    ground: &[Option<(f32, Vec3)>],
    dt: f32,
) -> Vec<Vec3> {
    let bone = |i: i16| hk_to_nif.get(i as usize).copied().flatten();
    let legs: Vec<_> = ik
        .legs
        .iter()
        .filter_map(|l| Some((l, bone(l.hip)?, bone(l.knee)?, bone(l.ankle)?)))
        .collect();
    let model = skeleton.model_space(locals);
    let ankles: Vec<Vec3> = legs
        .iter()
        .map(|(_, _, _, a)| model[*a].w_axis.truncate())
        .collect();
    state.offsets.resize(legs.len(), 0.0);
    let ease = step(0.25, dt);
    for (i, (leg, ..)) in legs.iter().enumerate() {
        let want = match ground.get(i).copied().flatten() {
            Some((z, _)) => {
                let h = ankles[i].z - ik.original_ground_height;
                (h + z).clamp(leg.min_ankle_height, leg.max_ankle_height) - h
            }
            None => 0.0,
        };
        state.offsets[i] += (want - state.offsets[i]) * ease;
    }
    // Drop the body so that the lowest foot reaches; the others then bend up.
    let drop = state.offsets.iter().copied().fold(0.0f32, f32::min) + ik.vertical_offset;
    log::trace!("foot IK offsets {:?} drop {drop:.1}", state.offsets);
    if state.offsets.iter().all(|o| o.abs() < 0.01) {
        return ankles;
    }
    if let Some(root) = skeleton.bones.iter().position(|b| b.parent.is_none()) {
        locals[root].translation.z += drop;
    }
    for (i, &(leg, hip, knee, ankle)) in legs.iter().enumerate() {
        // Thigh, calf and foot must be a direct chain.
        if skeleton.bones[knee].parent != Some(hip) || skeleton.bones[ankle].parent != Some(knee) {
            continue;
        }
        let model = skeleton.model_space(locals);
        let (h, k, a) = (
            model[hip].w_axis.truncate(),
            model[knee].w_axis.truncate(),
            model[ankle].w_axis.truncate(),
        );
        let (rh, rk, ra) = (
            rotation(&model[hip]),
            rotation(&model[knee]),
            rotation(&model[ankle]),
        );
        let lift = state.offsets[i] - drop;
        let target = a + Vec3::Z * lift;
        // Knee: open or close it so that hip-to-ankle spans the distance to the target.
        let (la, lb) = ((k - h).length(), (a - k).length());
        if la < 1e-3 || lb < 1e-3 {
            continue;
        }
        let d = (target - h)
            .length()
            .clamp((la - lb).abs() + 0.01, la + lb - 0.01);
        let interior = |c: f32| {
            ((la * la + lb * lb - c * c) / (2.0 * la * lb))
                .clamp(-1.0, 1.0)
                .acos()
        };
        let (min_knee, max_knee) = (
            leg.min_knee_degrees.to_radians(),
            leg.max_knee_degrees.to_radians(),
        );
        let delta = interior(d).clamp(min_knee, max_knee) - interior((a - h).length());
        let thigh = (h - k).normalize();
        let shin = (a - k).normalize();
        let axis = thigh
            .cross(shin)
            .try_normalize()
            .unwrap_or_else(|| (rk * leg.knee_axis).normalize());
        let bend = Quat::from_axis_angle(axis, delta);
        let a2 = k + bend * (a - k);
        // Hip: swing the leg so the ankle lands on the target.
        let swing = Quat::from_rotation_arc((a2 - h).normalize(), (target - h).normalize());
        let rh2 = swing * rh;
        let rk2 = swing * bend * rk;
        // The foot keeps its animated orientation, tilted with the ground when planted.
        let planted = ((leg.raised_ankle_height - (ankles[i].z - ik.original_ground_height))
            / (leg.raised_ankle_height - leg.planted_ankle_height).max(1e-3))
        .clamp(0.0, 1.0);
        let tilt = ground
            .get(i)
            .copied()
            .flatten()
            .map_or(Quat::IDENTITY, |(_, n)| {
                let full = Quat::from_rotation_arc(Vec3::Z, n.normalize());
                let (axis, angle) = full.to_axis_angle();
                let angle =
                    angle.min(leg.max_ankle_degrees.to_radians()) * planted * ik.forward_align;
                if angle.abs() < 1e-4 {
                    Quat::IDENTITY
                } else {
                    Quat::from_axis_angle(axis, angle)
                }
            });
        let ra2 = tilt * ra;
        let parent = skeleton.bones[hip]
            .parent
            .map_or(Quat::IDENTITY, |p| rotation(&model[p]));
        locals[hip].rotation = glam::Mat3::from_quat((parent.inverse() * rh2).normalize());
        locals[knee].rotation = glam::Mat3::from_quat((rh2.inverse() * rk2).normalize());
        locals[ankle].rotation = glam::Mat3::from_quat((rk2.inverse() * ra2).normalize());
    }
    ankles
}
