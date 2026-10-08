//! Keyframe animation blocks: controller managers, controller sequences and
//! transform interpolators / data (doors, water wheels, animated statics).

use glam::{Quat, Vec3};

use crate::Result;
use crate::blocks::Ref;
use crate::reader::Reader;

/// One animated target in a sequence.
#[derive(Debug, Clone)]
pub struct ControlledBlock {
    pub interpolator: Ref,
    pub controller: Ref,
    pub node: String,
    pub controller_type: String,
}

/// `NiControllerSequence`: a named animation ("Open", "Close", "Idle"...).
#[derive(Debug, Clone)]
pub struct ControllerSequence {
    pub name: String,
    pub blocks: Vec<ControlledBlock>,
    /// 0 loop, 1 reverse, 2 clamp.
    pub cycle: u32,
    pub frequency: f32,
    pub start: f32,
    pub stop: f32,
    pub text_keys: Ref,
}

#[derive(Debug, Clone)]
pub struct TransformInterpolator {
    /// Pose used for channels without keys (NaN components mean "not set").
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: f32,
    pub data: Ref,
}

#[derive(Debug, Clone, Default)]
pub enum RotationKeys {
    #[default]
    None,
    Quat(Vec<(f32, Quat)>),
    /// Separate X / Y / Z angle curves (radians).
    Euler([Vec<(f32, f32)>; 3]),
}

#[derive(Debug, Clone, Default)]
pub struct TransformData {
    pub rotations: RotationKeys,
    pub translations: Vec<(f32, Vec3)>,
    pub scales: Vec<(f32, f32)>,
}

fn time_controller(r: &mut Reader) -> Result<()> {
    r.block_ref()?; // next controller
    r.u16()?; // flags
    r.f32()?; // frequency
    r.f32()?; // phase
    r.f32()?; // start
    r.f32()?; // stop
    r.i32()?; // target
    Ok(())
}

pub(crate) fn controller_manager(r: &mut Reader) -> Result<Vec<Ref>> {
    time_controller(r)?;
    r.u8()?; // cumulative
    let seqs = r.ref_list()?;
    r.block_ref()?; // object palette
    Ok(seqs)
}

pub(crate) fn multi_target_transform_controller(r: &mut Reader) -> Result<()> {
    time_controller(r)?;
    let n = r.u16()?;
    for _ in 0..n {
        r.i32()?;
    }
    Ok(())
}

pub(crate) fn controller_sequence(r: &mut Reader) -> Result<ControllerSequence> {
    let name = r.string_value()?;
    let n = r.u32()?;
    r.u32()?; // array grow by
    let mut blocks = Vec::with_capacity(n.min(256) as usize);
    for _ in 0..n {
        let interpolator = r.block_ref()?;
        let controller = r.block_ref()?;
        r.u8()?; // priority
        let node = r.string_value()?;
        r.string_value()?; // property type
        let controller_type = r.string_value()?;
        r.string_value()?; // controller id
        r.string_value()?; // interpolator id
        blocks.push(ControlledBlock {
            interpolator,
            controller,
            node,
            controller_type,
        });
    }
    r.f32()?; // weight
    let text_keys = r.block_ref()?;
    let cycle = r.u32()?;
    let frequency = r.f32()?;
    let start = r.f32()?;
    let stop = r.f32()?;
    r.i32()?; // manager
    r.string_value()?; // accum root name
    if r.bs_version > 28 {
        let notes = r.u16()?;
        for _ in 0..notes {
            r.block_ref()?;
        }
    }
    Ok(ControllerSequence {
        name,
        blocks,
        cycle,
        frequency,
        start,
        stop,
        text_keys,
    })
}

fn quat_wxyz(r: &mut Reader) -> Result<Quat> {
    let w = r.f32()?;
    let x = r.f32()?;
    let y = r.f32()?;
    let z = r.f32()?;
    Ok(Quat::from_xyzw(x, y, z, w))
}

pub(crate) fn transform_interpolator(r: &mut Reader) -> Result<TransformInterpolator> {
    let translation = r.vec3()?;
    let rotation = quat_wxyz(r)?;
    let scale = r.f32()?;
    let data = r.block_ref()?;
    Ok(TransformInterpolator {
        translation,
        rotation,
        scale,
        data,
    })
}

/// A key group: count, interpolation, then (time, value[, forward, backward][, tbc]).
fn key_group<T>(
    r: &mut Reader,
    mut value: impl FnMut(&mut Reader) -> Result<T>,
) -> Result<Vec<(f32, T)>> {
    let n = r.u32()?;
    if n == 0 {
        return Ok(Vec::new());
    }
    let interp = r.u32()?;
    let mut out = Vec::with_capacity(n.min(4096) as usize);
    for _ in 0..n {
        let t = r.f32()?;
        let v = value(r)?;
        match interp {
            2 => {
                value(r)?;
                value(r)?;
            }
            3 => {
                r.f32()?;
                r.f32()?;
                r.f32()?;
            }
            _ => {}
        }
        out.push((t, v));
    }
    Ok(out)
}

pub(crate) fn transform_data(r: &mut Reader) -> Result<TransformData> {
    let n = r.u32()?;
    let mut rotations = RotationKeys::None;
    if n > 0 {
        let kind = r.u32()?;
        if kind == 4 {
            let x = key_group(r, |r| r.f32())?;
            let y = key_group(r, |r| r.f32())?;
            let z = key_group(r, |r| r.f32())?;
            rotations = RotationKeys::Euler([x, y, z]);
        } else {
            let mut keys = Vec::with_capacity(n.min(4096) as usize);
            for _ in 0..n {
                let t = r.f32()?;
                let q = quat_wxyz(r)?;
                if kind == 3 {
                    r.f32()?;
                    r.f32()?;
                    r.f32()?;
                }
                keys.push((t, q));
            }
            rotations = RotationKeys::Quat(keys);
        }
    }
    let translations = key_group(r, |r| r.vec3())?;
    let scales = key_group(r, |r| r.f32())?;
    Ok(TransformData {
        rotations,
        translations,
        scales,
    })
}

pub(crate) fn text_key_extra_data(r: &mut Reader) -> Result<Vec<(f32, String)>> {
    r.string_value()?;
    let n = r.u32()?;
    let mut keys = Vec::with_capacity(n.min(256) as usize);
    for _ in 0..n {
        let t = r.f32()?;
        keys.push((t, r.string_value()?));
    }
    Ok(keys)
}

pub(crate) fn default_av_object_palette(r: &mut Reader) -> Result<()> {
    r.i32()?; // scene
    let n = r.u32()?;
    for _ in 0..n {
        r.sized_string()?;
        r.i32()?;
    }
    Ok(())
}

fn lerp_keys<T: Copy>(keys: &[(f32, T)], t: f32, mix: impl Fn(T, T, f32) -> T) -> Option<T> {
    let first = keys.first()?;
    if t <= first.0 {
        return Some(first.1);
    }
    let i = keys.partition_point(|k| k.0 <= t);
    let (t0, a) = keys[i - 1];
    match keys.get(i) {
        Some(&(t1, b)) if t1 > t0 => Some(mix(a, b, (t - t0) / (t1 - t0))),
        _ => Some(a),
    }
}

impl TransformData {
    /// Sampled (rotation, translation, scale) at `t`; `None` for channels without keys.
    pub fn sample(&self, t: f32) -> (Option<Quat>, Option<Vec3>, Option<f32>) {
        let rot = match &self.rotations {
            RotationKeys::None => None,
            RotationKeys::Quat(k) => lerp_keys(k, t, |a, b, f| a.slerp(b, f)),
            RotationKeys::Euler([x, y, z]) => {
                let a =
                    |k: &Vec<(f32, f32)>| lerp_keys(k, t, |a, b, f| a + (b - a) * f).unwrap_or(0.0);
                Some(
                    Quat::from_rotation_z(a(z))
                        * Quat::from_rotation_y(a(y))
                        * Quat::from_rotation_x(a(x)),
                )
            }
        };
        let tr = lerp_keys(&self.translations, t, |a, b, f| a.lerp(b, f));
        let sc = lerp_keys(&self.scales, t, |a, b, f| a + (b - a) * f);
        (rot, tr, sc)
    }
}
