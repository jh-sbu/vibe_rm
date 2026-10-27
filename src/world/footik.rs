//! Foot placement: a character's `hkbFootIkDriverInfo` applied to a pose while
//! the graph runs an `hkbFootIkControlsModifier`, eased by its gains. The body
//! drops to the lower foot's ground, the other legs bend (two-bone IK) to put
//! their ankles on theirs, and planted feet tilt with the ground.

use glam::{Mat4, Quat, Vec3};
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
    pins: Vec<Option<Vec3>>,
    locks: Vec<f32>,
    alignment: Quat,
}

/// Fraction to close per update for a per-frame (30 Hz) gain.
fn step(gain: f32, dt: f32) -> f32 {
    1.0 - (1.0 - gain.clamp(0.0, 1.0)).powf(dt.max(0.0) * 30.0)
}

fn rotation(m: &glam::Mat4) -> Quat {
    m.to_scale_rotation_translation().1
}

/// Fit z = ax + by + c through support hits. A degenerate footprint uses the
/// hit normals, since two feet alone cannot determine a full support plane.
fn support_normal(ankles: &[Vec3], ground: &[Option<(f32, Vec3)>]) -> Vec3 {
    let points: Vec<_> = ankles
        .iter()
        .zip(ground)
        .filter_map(|(a, hit)| hit.map(|(z, n)| (a.with_z(z), n)))
        .collect();
    if points.is_empty() {
        return Vec3::Z;
    }
    let centre = points.iter().map(|(p, _)| *p).sum::<Vec3>() / points.len() as f32;
    let (mut xx, mut yy, mut xy, mut xz, mut yz) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for (p, _) in &points {
        let d = *p - centre;
        xx += d.x * d.x;
        yy += d.y * d.y;
        xy += d.x * d.y;
        xz += d.x * d.z;
        yz += d.y * d.z;
    }
    let determinant = xx * yy - xy * xy;
    if determinant.abs() > 1e-4 {
        Vec3::new(
            -(xz * yy - yz * xy) / determinant,
            -(yz * xx - xz * xy) / determinant,
            1.0,
        )
        .normalize()
    } else {
        points
            .iter()
            .map(|(_, n)| *n)
            .sum::<Vec3>()
            .normalize_or(Vec3::Z)
    }
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
    world_from_model: Mat4,
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
    if legs.is_empty() {
        return ankles;
    }
    state.pins.resize(legs.len(), None);
    state.locks.resize(legs.len(), 0.0);
    if gains.is_none() {
        state.pins.fill(None);
        state.locks.fill(0.0);
    }
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
        state.alignment = Quat::IDENTITY;
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
    let normal = support_normal(&ankles, ground);
    let desired = if g.align_world_from_model > 0.0 {
        let rotation = Quat::from_rotation_arc(Vec3::Z, normal);
        let (axis, angle) = rotation.to_axis_angle();
        let limit = ik
            .legs
            .iter()
            .map(|l| l.max_ankle_degrees.to_radians())
            .fold(std::f32::consts::FRAC_PI_2, f32::min)
            .max(0.0);
        Quat::from_axis_angle(axis, angle.min(limit))
    } else {
        Quat::IDENTITY
    };
    state.alignment = state
        .alignment
        .slerp(
            desired,
            step(
                if g.align_world_from_model > 0.0 {
                    g.align_world_from_model
                } else {
                    on_off
                },
                dt,
            ),
        )
        .normalize();
    if let Some(root) = skeleton.bones.iter().position(|b| b.parent.is_none()) {
        let align = Quat::IDENTITY.slerp(state.alignment, w);
        locals[root].rotation = glam::Mat3::from_quat(align) * locals[root].rotation;
        locals[root].translation = align * locals[root].translation + Vec3::Z * drop;
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
        let mut target = a + Vec3::Z * lift;
        if gains.is_some() && ground.get(i).is_some_and(Option::is_some) {
            let animated = world_from_model.transform_point3(ankles[i]);
            if state.pins[i].is_some_and(|pin| pin.distance(animated) > (a - h).length() * 2.0) {
                state.pins[i] = None; // A teleport or discontinuous animation.
            }
            if state.pins[i].is_none() && planted >= 0.8 {
                state.pins[i] = Some(world_from_model.transform_point3(target));
                state.locks[i] = planted;
            }
            state.locks[i] += (planted - state.locks[i]) * step(g.foot_unlock, dt);
            if planted <= 0.01 && state.locks[i] <= 0.01 {
                state.pins[i] = None;
            }
            if let Some(pin) = state.pins[i] {
                let pinned = world_from_model.inverse().transform_point3(pin);
                target = target.lerp(pinned, state.locks[i] * w);
            }
        } else {
            state.pins[i] = None;
            state.locks[i] = 0.0;
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::skeleton::Bone;
    use havok::behavior::FootIkLeg;

    fn fixture() -> (Skeleton, FootIk, FootIkGains, Vec<Transform>) {
        let mut locals = vec![Transform::default(); 3];
        locals[0].translation = Vec3::Z * 80.0;
        locals[1].translation = Vec3::new(0.0, 10.0, -40.0);
        locals[2].translation = Vec3::new(0.0, -10.0, -40.0);
        let skeleton = Skeleton {
            bones: locals
                .iter()
                .enumerate()
                .map(|(i, &bind)| Bone {
                    name: i.to_string(),
                    parent: i.checked_sub(1),
                    bind,
                })
                .collect(),
            ..Default::default()
        };
        let ik = FootIk {
            legs: vec![FootIkLeg {
                knee_axis: Vec3::X,
                foot_end: Vec3::ZERO,
                planted_ankle_height: 0.0,
                raised_ankle_height: 20.0,
                min_ankle_height: -100.0,
                max_ankle_height: 100.0,
                min_knee_degrees: 0.0,
                max_knee_degrees: 180.0,
                max_ankle_degrees: 45.0,
                hip: 0,
                knee: 1,
                ankle: 2,
            }],
            raycast_up: 100.0,
            raycast_down: 100.0,
            original_ground_height: 0.0,
            vertical_offset: 0.0,
            forward_align: 1.0,
            sideways_align: 0.0,
        };
        let gains = FootIkGains {
            on_off: 1.0,
            ground_ascending: 1.0,
            ground_descending: 1.0,
            foot_planted: 1.0,
            foot_raised: 1.0,
            foot_unlock: 1.0,
            world_from_model_feedback: 1.0,
            error_up_down_bias: 1.0,
            ankle_orientation: 1.0,
            ..Default::default()
        };
        (skeleton, ik, gains, locals)
    }

    #[test]
    fn planted_foot_stays_in_world_space_then_unlocks_when_lifted() {
        let (skeleton, ik, gains, bind) = fixture();
        let mut state = FootIkState::default();
        let map = [Some(0), Some(1), Some(2)];
        let ground = [Some((0.0, Vec3::Z))];
        let mut locals = bind.clone();
        apply(
            &mut state,
            &ik,
            Some(&gains),
            &map,
            &skeleton,
            &mut locals,
            &ground,
            Mat4::IDENTITY,
            1.0 / 30.0,
        );
        let first = skeleton.model_space(&locals)[2].w_axis.truncate();
        let moved = Mat4::from_translation(Vec3::X * 10.0);
        locals = bind.clone();
        apply(
            &mut state,
            &ik,
            Some(&gains),
            &map,
            &skeleton,
            &mut locals,
            &ground,
            moved,
            1.0 / 30.0,
        );
        let foot = moved.transform_point3(skeleton.model_space(&locals)[2].w_axis.truncate());
        assert!(foot.distance(first) < 0.05, "{foot:?} vs {first:?}");
        locals = bind;
        locals[2].translation.z += 30.0;
        apply(
            &mut state,
            &ik,
            Some(&gains),
            &map,
            &skeleton,
            &mut locals,
            &ground,
            moved,
            1.0 / 30.0,
        );
        assert!(state.pins[0].is_none());
        let foot = moved.transform_point3(skeleton.model_space(&locals)[2].w_axis.truncate());
        assert!((foot.x - 10.0).abs() < 0.05);
    }
    #[test]
    fn support_plane_handles_a_step_and_degenerate_contacts() {
        let ankles = [
            Vec3::new(-10.0, -10.0, 0.0),
            Vec3::new(10.0, -10.0, 0.0),
            Vec3::new(-10.0, 10.0, 0.0),
            Vec3::new(10.0, 10.0, 0.0),
        ];
        let ground = [
            Some((0.0, Vec3::Z)),
            Some((0.0, Vec3::Z)),
            Some((10.0, Vec3::Z)),
            Some((10.0, Vec3::Z)),
        ];
        assert!(
            support_normal(&ankles, &ground).distance(Vec3::new(0.0, -0.5, 1.0).normalize()) < 1e-5
        );
        assert_eq!(support_normal(&[], &[]), Vec3::Z);
    }

    #[test]
    fn alignment_follows_slope_with_gain_and_keeps_zero_gain_upright() {
        for gain in [0.0, 0.15] {
            let (mut skeleton, mut ik, mut gains, bind) = fixture();
            let mut locals = vec![Transform::default()];
            locals.extend(bind);
            skeleton.bones.insert(
                0,
                Bone {
                    name: "root".into(),
                    parent: None,
                    bind: Transform::default(),
                },
            );
            for b in &mut skeleton.bones[1..] {
                b.parent = Some(b.parent.map_or(0, |p| p + 1));
            }
            ik.legs[0].hip = 1;
            ik.legs[0].knee = 2;
            ik.legs[0].ankle = 3;
            gains.align_world_from_model = gain;
            let ground = [Some((0.0, Vec3::new(0.0, -0.3, 1.0).normalize()))];
            let mut state = FootIkState::default();
            let initial = locals.clone();
            for _ in 0..60 {
                locals = initial.clone();
                apply(
                    &mut state,
                    &ik,
                    Some(&gains),
                    &[Some(0), Some(1), Some(2), Some(3)],
                    &skeleton,
                    &mut locals,
                    &ground,
                    Mat4::IDENTITY,
                    1.0 / 30.0,
                );
            }
            let up = locals[0].rotation * Vec3::Z;
            if gain == 0.0 {
                assert!(up.distance(Vec3::Z) < 1e-5);
            } else {
                assert!(up.distance(ground[0].unwrap().1) < 0.001);
            }
        }
    }
}
