//! Serialized ragdoll controls. Member order: public MIT Skyrim Behavior Editor;
//! offsets corroborated against SE graph assets (research/living-ragdolls.md).
use crate::Packfile;
use glam::{Mat4, Quat, Vec3};

#[derive(Debug, Clone)]
pub struct PoseMatching {
    pub world_from_model_rotation: Quat,
    pub blend_speed: f32,
    pub min_speed_to_switch: f32,
    pub min_switch_time_no_error: f32,
    pub min_switch_time_full_error: f32,
    pub start_playing_event: i32,
    pub start_matching_event: i32,
    pub bones: [i16; 4],
    pub mode: u8,
}
impl PoseMatching {
    pub(super) fn read(p: &Packfile, o: u32) -> Self {
        Self {
            world_from_model_rotation: Quat::from_xyzw(
                p.f32(o + 0xa0),
                p.f32(o + 0xa4),
                p.f32(o + 0xa8),
                p.f32(o + 0xac),
            ),
            blend_speed: p.f32(o + 0xb0),
            min_speed_to_switch: p.f32(o + 0xb4),
            min_switch_time_no_error: p.f32(o + 0xb8),
            min_switch_time_full_error: p.f32(o + 0xbc),
            start_playing_event: p.i32(o + 0xc0),
            start_matching_event: p.i32(o + 0xc4),
            bones: std::array::from_fn(|i| p.i16(o + 0xc8 + i as u32 * 2)),
            mode: p.u8(o + 0xd0),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GetUp {
    pub enabled: bool,
    pub ground_normal: Vec3,
    pub duration: f32,
    pub align_duration: f32,
    pub bones: [i16; 3],
}
#[derive(Debug, Clone)]
pub struct PoweredRagdoll {
    pub enabled: bool,
    pub max_force: f32,
    pub tau: f32,
    pub damping: f32,
    pub proportional_recovery_velocity: f32,
    pub constant_recovery_velocity: f32,
    pub bones: Vec<i16>,
    pub matching_bones: [i16; 3],
    pub mode: u8,
    pub weights: Vec<f32>,
}
#[derive(Debug, Clone)]
pub struct RigidBodyRagdoll {
    pub enabled: bool,
    pub gains: [f32; 12],
    pub blend: f32,
    pub bones: Vec<i16>,
}
#[derive(Debug, Clone)]
pub struct KeyframeInfo {
    pub position: Vec3,
    pub rotation: Quat,
    pub bone: i16,
    pub valid: bool,
}
#[derive(Debug, Clone)]
pub struct KeyframeBones {
    pub enabled: bool,
    pub bones: Vec<i16>,
    pub info: Vec<KeyframeInfo>,
}
#[derive(Debug, Clone)]
pub struct ContactListener {
    pub enabled: bool,
    pub event: super::EventProperty,
    pub bones: Vec<i16>,
}

pub(super) fn indices(p: &Packfile, at: u32) -> Vec<i16> {
    p.ptr(at)
        .map(|o| {
            let (d, n) = p.array(o + 0x10);
            d.map(|d| (0..n as u32).map(|i| p.i16(d + i * 2)).collect())
                .unwrap_or_default()
        })
        .unwrap_or_default()
}

impl GetUp {
    pub(super) fn read(p: &Packfile, o: u32) -> Self {
        Self {
            enabled: p.u8(o + 0x48) != 0,
            ground_normal: Vec3::new(p.f32(o + 0x50), p.f32(o + 0x54), p.f32(o + 0x58)),
            duration: p.f32(o + 0x60),
            align_duration: p.f32(o + 0x64),
            bones: std::array::from_fn(|i| p.i16(o + 0x68 + i as u32 * 2)),
        }
    }
}
impl PoweredRagdoll {
    pub(super) fn read(p: &Packfile, o: u32) -> Self {
        let weights = p
            .ptr(o + 0x80)
            .map(|o| {
                let (d, n) = p.array(o + 0x10);
                d.map(|d| (0..n as u32).map(|i| p.f32(d + i * 4)).collect())
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        Self {
            enabled: p.u8(o + 0x48) != 0,
            max_force: p.f32(o + 0x50),
            tau: p.f32(o + 0x54),
            damping: p.f32(o + 0x58),
            proportional_recovery_velocity: p.f32(o + 0x5c),
            constant_recovery_velocity: p.f32(o + 0x60),
            bones: indices(p, o + 0x70),
            matching_bones: std::array::from_fn(|i| p.i16(o + 0x78 + i as u32 * 2)),
            mode: p.u8(o + 0x7e),
            weights,
        }
    }
}
impl RigidBodyRagdoll {
    pub(super) fn read(p: &Packfile, o: u32) -> Self {
        Self {
            enabled: p.u8(o + 0x48) != 0,
            gains: std::array::from_fn(|i| p.f32(o + 0x50 + i as u32 * 4)),
            blend: p.f32(o + 0x80),
            bones: indices(p, o + 0x90),
        }
    }
}
impl KeyframeBones {
    pub(super) fn read(p: &Packfile, o: u32) -> Self {
        let (d, n) = p.array(o + 0x50);
        Self {
            enabled: p.u8(o + 0x48) != 0,
            bones: indices(p, o + 0x60),
            info: d
                .map(|d| {
                    (0..n as u32)
                        .map(|i| {
                            let a = d + i * 0x30;
                            KeyframeInfo {
                                position: Vec3::new(p.f32(a), p.f32(a + 4), p.f32(a + 8)),
                                rotation: Quat::from_xyzw(
                                    p.f32(a + 0x10),
                                    p.f32(a + 0x14),
                                    p.f32(a + 0x18),
                                    p.f32(a + 0x1c),
                                ),
                                bone: p.i16(a + 0x20),
                                valid: p.u8(a + 0x22) != 0,
                            }
                        })
                        .collect()
                })
                .unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct FrameOutput {
    pub powered: Vec<PoweredRagdoll>,
    pub rigid: Vec<RigidBodyRagdoll>,
    pub keyframe: Vec<KeyframeBones>,
    pub get_up: Option<(GetUp, f32)>,
    pub contacts: Vec<ContactListener>,
    pub matching_yaw: Option<f32>,
}

/// Compare authored matching bones after the best horizontal alignment, using
/// translations and rotations. Translation is relative to the pelvis, so an
/// airborne or translated character still picks the same facing clip.
pub fn match_error(input: &[Mat4], candidate: &[Mat4], bones: &[i16; 4]) -> Option<(f32, f32)> {
    let pelvis = usize::try_from(bones[3]).ok()?;
    let (a, b) = (
        input.get(pelvis)?.w_axis.truncate(),
        candidate.get(pelvis)?.w_axis.truncate(),
    );
    let pairs: Vec<_> = bones
        .iter()
        .filter_map(|&i| {
            let i = usize::try_from(i).ok()?;
            Some((*input.get(i)?, *candidate.get(i)?))
        })
        .collect();
    if pairs.is_empty() {
        return None;
    }
    let mut dot = 0.0;
    let mut cross = 0.0;
    for (x, y) in &pairs {
        let u = (y.w_axis.truncate() - b).truncate();
        let v = (x.w_axis.truncate() - a).truncate();
        dot += u.dot(v);
        cross += u.perp_dot(v);
    }
    let yaw = cross.atan2(dot);
    let q = Quat::from_rotation_z(yaw);
    let mut error = 0.0;
    for (x, y) in pairs {
        let (_, ra, _) = x.to_scale_rotation_translation();
        let (_, rb, _) = y.to_scale_rotation_translation();
        error += ((x.w_axis.truncate() - a) - q * (y.w_axis.truncate() - b)).length_squared();
        error += 100.0 * ra.angle_between(q * rb).powi(2);
    }
    (error.is_finite() && yaw.is_finite()).then_some((error, yaw))
}
