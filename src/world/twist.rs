//! Additive single-bone `hkbTwistModifier` pose support. Unresolved chain and
//! non-additive modes are retained by the reader but deliberately not applied.

use glam::{Mat3, Quat};
use havok::behavior::Twist;
use nif::Transform;

use super::skeleton::Skeleton;

/// Apply a supported twist in place, preserving local translation and scale.
/// Bone indices belong to Havok and must pass through `hk_to_nif`.
pub fn apply(
    t: &Twist,
    hk_to_nif: &[Option<usize>],
    skeleton: &Skeleton,
    locals: &mut [Transform],
) {
    if !t.enabled
        || !t.additive
        || t.set_angle_method != 0
        || t.axis_coordinates > 1
        || t.start_bone < 0
        || t.start_bone != t.end_bone
        || !t.angle_degrees.is_finite()
    {
        return;
    }
    let Some(axis) = t.axis.try_normalize() else {
        return;
    };
    let Some(b) = hk_to_nif.get(t.start_bone as usize).copied().flatten() else {
        return;
    };
    let (Some(bone), Some(local)) = (skeleton.bones.get(b), locals.get(b)) else {
        return;
    };
    let current = Quat::from_mat3(&local.rotation);
    if !current.is_finite() || current.length_squared() < 1e-8 {
        return;
    }
    let turn = Quat::from_axis_angle(axis, t.angle_degrees.to_radians());
    let rotated = if t.axis_coordinates == 1 {
        // The authored axis follows the bone's current local orientation.
        current * turn
    } else {
        // Convert the model-space turn into the parent's coordinates.
        let parent = bone.parent.map_or(Quat::IDENTITY, |p| {
            skeleton.model_space(locals)[p]
                .to_scale_rotation_translation()
                .1
        });
        if !parent.is_finite() || parent.length_squared() < 1e-8 {
            return;
        }
        let parent = parent.normalize();
        parent.inverse() * turn * parent * current
    };
    if rotated.is_finite() && rotated.length_squared() > 1e-8 {
        locals[b].rotation = Mat3::from_quat(rotated.normalize());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::skeleton::Bone;
    use glam::Vec3;

    fn skeleton() -> Skeleton {
        Skeleton {
            bones: vec![
                Bone {
                    name: "parent".into(),
                    parent: None,
                    bind: Transform::default(),
                },
                Bone {
                    name: "target".into(),
                    parent: Some(0),
                    bind: Transform::default(),
                },
            ],
            ..Default::default()
        }
    }

    fn twist() -> Twist {
        Twist {
            enabled: true,
            additive: true,
            axis: Vec3::X * 2.0,
            angle_degrees: 90.0,
            start_bone: 0,
            end_bone: 0,
            set_angle_method: 0,
            axis_coordinates: 1,
        }
    }

    fn assert_rotation(actual: Mat3, expected: Quat) {
        assert!(actual.abs_diff_eq(Mat3::from_quat(expected), 1e-5));
    }

    #[test]
    fn twist_local_and_model_axes_with_rotated_parent_and_bone() {
        let skeleton = skeleton();
        let parent = Quat::from_rotation_z(0.7);
        let bone = Quat::from_rotation_y(0.4);
        let mut initial = skeleton.bind_locals();
        initial[0].rotation = Mat3::from_quat(parent);
        initial[1].rotation = Mat3::from_quat(bone);
        initial[1].translation = Vec3::new(3.0, 4.0, 5.0);
        initial[1].scale = 2.0;
        let turn = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        for coordinates in [0, 1] {
            let mut locals = initial.clone();
            let mut t = twist();
            t.axis_coordinates = coordinates;
            // Havok bone 0 is NIF bone 1, not the parent at NIF index 0.
            apply(&t, &[Some(1)], &skeleton, &mut locals);
            let expected = if coordinates == 1 {
                bone * turn
            } else {
                parent.inverse() * turn * parent * bone
            };
            assert_rotation(locals[1].rotation, expected);
            assert_rotation(locals[0].rotation, parent);
            assert_eq!(locals[1].translation, initial[1].translation);
            assert_eq!(locals[1].scale, 2.0);
        }
    }

    #[test]
    fn twist_invalid_inputs_and_unsupported_modes_leave_pose_unchanged() {
        let skeleton = skeleton();
        let mut cases = Vec::new();
        for field in 0..11 {
            let mut t = twist();
            match field {
                0 => t.enabled = false,
                1 => t.additive = false,
                2 => t.end_bone = 1,
                3 => t.start_bone = -1,
                4 => t.set_angle_method = 1,
                5 => t.axis_coordinates = 2,
                6 => t.angle_degrees = f32::NAN,
                7 => t.axis = Vec3::ZERO,
                8 => t.axis = Vec3::splat(f32::INFINITY),
                9 => t.axis = Vec3::splat(f32::NAN),
                _ => t.angle_degrees = f32::INFINITY,
            }
            cases.push(t);
        }
        for t in cases {
            let mut locals = skeleton.bind_locals();
            apply(&t, &[Some(1)], &skeleton, &mut locals);
            assert_eq!(locals[1].rotation, Mat3::IDENTITY);
        }
        for map in [vec![], vec![None], vec![Some(99)]] {
            let mut locals = skeleton.bind_locals();
            apply(&twist(), &map, &skeleton, &mut locals);
            assert_eq!(locals[1].rotation, Mat3::IDENTITY);
        }
        let mut locals = vec![];
        apply(&twist(), &[Some(1)], &skeleton, &mut locals);
    }

    #[test]
    fn twist_signed_angles_and_zero_preserve_the_sampled_pose() {
        let skeleton = skeleton();
        let initial = Quat::from_rotation_z(0.3);
        for angle in [0.0, -45.0] {
            let mut locals = skeleton.bind_locals();
            locals[0].rotation = Mat3::from_quat(initial);
            let mut t = twist();
            t.angle_degrees = angle;
            apply(&t, &[Some(0)], &skeleton, &mut locals);
            assert_rotation(
                locals[0].rotation,
                initial * Quat::from_rotation_x(angle.to_radians()),
            );
        }
    }
}
