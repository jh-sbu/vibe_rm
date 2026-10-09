//! Foot placement: a character's `hkbFootIkDriverInfo` applied to a pose while
//! the graph runs an `hkbFootIkControlsModifier`, eased by its gains. The body
//! drops to the lower foot's ground, the other legs bend (two-bone IK) to put
//! their ankles on theirs, and planted feet tilt with the ground.

use glam::{Quat, Vec3};
use havok::behavior::{FootIk, FootIkGains};
use nif::Transform;

use super::skeleton::Skeleton;

/// Per-actor smoothing of the ground offsets.
#[derive(Default)]
pub struct FootIkState {
    /// How far placement is faded in (0 off, 1 on).
    weight: f32,
    /// Each leg's ground offset (model space) being eased towards.
    offsets: Vec<f32>,
    /// The body's drop (model space), following the feet by the feedback gain.
    drop: f32,
    /// Each leg's ankle tilt with the ground.
    tilts: Vec<Quat>,
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
/// hit. `gains` are the running controls modifier's, `None` fading placement out.
/// Returns the ankles (model space, before placement) to cast the next rays from.
#[allow(clippy::too_many_arguments)]
pub fn apply(
    state: &mut FootIkState,
    ik: &FootIk,
    gains: Option<&FootIkGains>,
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
    state.tilts.resize(legs.len(), Quat::IDENTITY);
    // Off: fade out at the last gain the graph gave (the vanilla graphs all use 0.2).
    let on_off = gains.map_or(0.2, |g| g.on_off);
    let on = if gains.is_some() { 1.0 } else { 0.0 };
    state.weight += (on - state.weight) * step(on_off, dt);
    if gains.is_none() && state.weight < 0.01 {
        state.weight = 0.0;
        state.offsets.iter_mut().for_each(|o| *o = 0.0);
        state.tilts.iter_mut().for_each(|t| *t = Quat::IDENTITY);
        state.drop = 0.0;
        return ankles;
    }
    let g = gains.cloned().unwrap_or(FootIkGains {
        ground_ascending: 1.0,
        ground_descending: 1.0,
        foot_planted: 1.0,
        foot_raised: 1.0,
        world_from_model_feedback: 1.0,
        error_up_down_bias: 1.0,
        ..Default::default()
    });
    for (i, (leg, ..)) in legs.iter().enumerate() {
        let want = match ground.get(i).copied().flatten() {
            Some((z, _)) => {
                let h = ankles[i].z - ik.original_ground_height;
                (h + z).clamp(leg.min_ankle_height, leg.max_ankle_height) - h
            }
            None => 0.0,
        };
        let gain = if want > state.offsets[i] {
            g.ground_ascending
        } else {
            g.ground_descending
        };
        state.offsets[i] += (want - state.offsets[i]) * step(gain, dt);
    }
    // Drop the body so that the lowest foot reaches (the others then bend up),
    // or raise it by the bias's share where every foot is higher.
    let lowest = state.offsets.iter().copied().fold(f32::INFINITY, f32::min);
    let want = if lowest < 0.0 {
        lowest
    } else {
        lowest * (1.0 - g.error_up_down_bias).clamp(0.0, 1.0)
    };
    state.drop += (want - state.drop) * step(g.world_from_model_feedback, dt);
    let w = state.weight;
    let drop = (state.drop + ik.vertical_offset) * w;
    log::trace!(
        "foot IK offsets {:?} drop {drop:.1} weight {w:.2}",
        state.offsets
    );
    if state.offsets.iter().all(|o| o.abs() < 0.01) && drop.abs() < 0.01 {
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
        // Planted (1) to raised (0), by the animated ankle's height.
        let planted = ((leg.raised_ankle_height - (ankles[i].z - ik.original_ground_height))
            / (leg.raised_ankle_height - leg.planted_ankle_height).max(1e-3))
        .clamp(0.0, 1.0);
        let leg_gain = g.foot_raised + (g.foot_planted - g.foot_raised) * planted;
        let lift = (state.offsets[i] * w - drop) * leg_gain.clamp(0.0, 1.0);
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
        // The foot keeps its animated orientation, tilted with the ground when
        // planted, turning at the ankle orientation gain.
        let want = ground
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
        state.tilts[i] = state.tilts[i]
            .slerp(want, step(g.ankle_orientation, dt))
            .normalize();
        let tilt = Quat::IDENTITY.slerp(state.tilts[i], w);
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
