//! Keyframe animation blocks: controller managers, controller sequences and
//! transform interpolators / data (doors, water wheels, animated statics);
//! shader property controllers with their float and colour keys (scrolling,
//! pulsing effects: clouds, auroras, magic).

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

/// `NiTimeController`'s timing: how a controller maps the time onto its keys.
#[derive(Debug, Clone, Copy)]
pub struct Timing {
    /// Flags: bits 1-2 the cycle type (0 loop, 1 reverse, 2 clamp), bit 3 active.
    pub flags: u16,
    pub frequency: f32,
    pub phase: f32,
    pub start: f32,
    pub stop: f32,
}

impl Timing {
    /// The key time at `t` seconds: scaled by the frequency, shifted by the
    /// phase and cycled over start..stop by the cycle type.
    pub fn key_time(&self, t: f32) -> f32 {
        let len = self.stop - self.start;
        let k = t * self.frequency + self.phase;
        if len <= 0.0 {
            return self.start;
        }
        match (self.flags >> 1) & 3 {
            // Reverse: back and forth.
            1 => {
                let c = k.rem_euclid(2.0 * len);
                self.start + if c > len { 2.0 * len - c } else { c }
            }
            2 => k.clamp(self.start, self.stop),
            _ => self.start + (k - self.start).rem_euclid(len),
        }
    }

    pub fn active(&self) -> bool {
        self.flags & 8 != 0
    }
}

/// A shader property's float or colour controller (`BSEffectShaderProperty*`,
/// `BSLightingShaderProperty*`).
#[derive(Debug, Clone)]
pub struct ShaderController {
    pub next: Ref,
    pub timing: Timing,
    pub interpolator: Ref,
    /// The float (or colour) it drives: for effect shaders 0 emissive
    /// multiple, 1..4 falloff start / stop angle, start / stop opacity, 5
    /// alpha, 6 U offset, 7 U scale, 8 V offset, 9 V scale; colour 0 the
    /// emissive colour.
    pub variable: u32,
    /// A colour controller (else a float one).
    pub color: bool,
    /// On a lighting shader (else an effect shader).
    pub lighting: bool,
}

/// `NiFloatInterpolator` / `NiPoint3Interpolator`: a pose value and its keys.
#[derive(Debug, Clone)]
pub struct ValueInterpolator {
    pub value: Vec3,
    pub data: Ref,
}

/// `NiFloatData` / `NiPosData` keys (floats in x).
#[derive(Debug, Clone, Default)]
pub struct ValueKeys {
    pub keys: Vec<(f32, Vec3)>,
}

impl ValueKeys {
    pub fn sample(&self, t: f32) -> Option<Vec3> {
        lerp_keys(&self.keys, t, |a, b, f| a.lerp(b, f))
    }
}

fn timing(r: &mut Reader) -> Result<(Ref, Timing)> {
    let next = r.block_ref()?;
    let flags = r.u16()?;
    let frequency = r.f32()?;
    let phase = r.f32()?;
    let start = r.f32()?;
    let stop = r.f32()?;
    r.i32()?; // target
    Ok((
        next,
        Timing {
            flags,
            frequency,
            phase,
            start,
            stop,
        },
    ))
}

fn time_controller(r: &mut Reader) -> Result<()> {
    timing(r)?;
    Ok(())
}

pub(crate) fn shader_controller(
    r: &mut Reader,
    color: bool,
    lighting: bool,
) -> Result<ShaderController> {
    let (next, timing) = timing(r)?;
    let interpolator = r.block_ref()?;
    let variable = r.u32()?;
    Ok(ShaderController {
        next,
        timing,
        interpolator,
        variable,
        color,
        lighting,
    })
}

pub(crate) fn value_interpolator(r: &mut Reader, point3: bool) -> Result<ValueInterpolator> {
    let value = if point3 {
        r.vec3()?
    } else {
        Vec3::new(r.f32()?, 0.0, 0.0)
    };
    let data = r.block_ref()?;
    Ok(ValueInterpolator { value, data })
}

pub(crate) fn value_keys(r: &mut Reader, point3: bool) -> Result<ValueKeys> {
    let keys = if point3 {
        key_group(r, |r| r.vec3())?
    } else {
        key_group(r, |r| Ok(Vec3::new(r.f32()?, 0.0, 0.0)))?
    };
    Ok(ValueKeys { keys })
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

#[cfg(test)]
mod tests {
    use super::Timing;

    fn timing(cycle: u16) -> Timing {
        Timing {
            flags: 8 | (cycle << 1),
            frequency: 1.0,
            phase: 0.0,
            start: 0.0,
            stop: 10.0,
        }
    }

    #[test]
    fn key_time_cycles() {
        assert_eq!(timing(0).key_time(12.0), 2.0);
        assert_eq!(timing(1).key_time(12.0), 8.0);
        assert_eq!(timing(2).key_time(12.0), 10.0);
        let mut t = timing(0);
        t.frequency = 2.0;
        t.phase = 1.0;
        assert_eq!(t.key_time(5.0), 1.0);
    }
}
