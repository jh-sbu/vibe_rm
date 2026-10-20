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
                let c = (k - self.start).rem_euclid(2.0 * len);
                self.start + if c > len { 2.0 * len - c } else { c }
            }
            2 => k.clamp(self.start, self.stop),
            _ => self.start + (k - self.start).rem_euclid(len),
        }
    }

    pub fn active(&self) -> bool {
        self.flags & 8 != 0
    }

    /// Timed from when its object is started (`APP_INIT`), as effects'
    /// controllers are, rather than from the application's clock.
    pub fn app_init(&self) -> bool {
        self.flags & 1 != 0
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

/// `BSNiAlphaPropertyTestRefController`: animates a `NiAlphaProperty`'s
/// alpha-test reference in the property's byte range (0..255).
#[derive(Debug, Clone)]
pub struct AlphaTestController {
    pub next: Ref,
    pub timing: Timing,
    pub interpolator: Ref,
}

pub(crate) fn alpha_test_controller(r: &mut Reader) -> Result<AlphaTestController> {
    let (next, timing) = timing(r)?;
    let interpolator = r.block_ref()?;
    Ok(AlphaTestController {
        next,
        timing,
        interpolator,
    })
}

/// What a node's own controller drives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeControllerKind {
    /// `NiTransformController`: its transform, by a transform interpolator.
    Transform,
    /// `NiVisController`: whether it shows, by a bool interpolator.
    Visibility,
}

/// A controller on a node (`NiTransformController`, `NiVisController`),
/// chained from the node's `controller`.
#[derive(Debug, Clone)]
pub struct NodeController {
    pub next: Ref,
    pub timing: Timing,
    pub interpolator: Ref,
    pub kind: NodeControllerKind,
}

pub(crate) fn node_controller(r: &mut Reader, kind: NodeControllerKind) -> Result<NodeController> {
    let (next, timing) = timing(r)?;
    let interpolator = r.block_ref()?;
    Ok(NodeController {
        next,
        timing,
        interpolator,
        kind,
    })
}

/// `NiFloatInterpolator` / `NiPoint3Interpolator`: a pose value and its keys.
#[derive(Debug, Clone)]
pub struct ValueInterpolator {
    pub value: Vec3,
    pub data: Ref,
}

/// `NiFloatData` / `NiPosData` keys (floats in x).
#[derive(Debug, Clone)]
pub struct ValueKeys {
    pub keys: Vec<(f32, Vec3)>,
    /// NIF KeyType: 1 linear, 2 quadratic, 3 TBC, 5 constant.
    pub interpolation: u32,
    /// Quadratic tangents in serialized (forward/incoming, backward/outgoing)
    /// order, one pair per key. TBC keys store generated incoming/outgoing
    /// tangents in the same array. Tangents are in normalized segment units.
    pub tangents: Vec<(Vec3, Vec3)>,
}

impl Default for ValueKeys {
    fn default() -> Self {
        Self {
            keys: Vec::new(),
            interpolation: 1,
            tangents: Vec::new(),
        }
    }
}

impl ValueKeys {
    /// Sample float / colour keys. Boolean controllers sample their preceding
    /// key explicitly, rather than using float CONST_KEY midpoint selection.
    pub fn sample(&self, t: f32) -> Option<Vec3> {
        if t.is_nan() {
            return None;
        }
        if matches!(self.interpolation, 2 | 3) && self.tangents.len() == self.keys.len() {
            let first = self.keys.first()?;
            if t <= first.0 {
                return Some(first.1);
            }
            let i = self.keys.partition_point(|key| key.0 <= t);
            let (t0, a) = self.keys[i - 1];
            return Some(match self.keys.get(i) {
                Some(&(t1, b)) if t1 > t0 => {
                    let u = (t - t0) / (t1 - t0);
                    let u2 = u * u;
                    let u3 = u2 * u;
                    // Public OpenMW / NifSkope use the lower outgoing and
                    // upper incoming tangent directly, without a time scale.
                    a * (2.0 * u3 - 3.0 * u2 + 1.0)
                        + b * (-2.0 * u3 + 3.0 * u2)
                        + self.tangents[i - 1].1 * (u3 - 2.0 * u2 + u)
                        + self.tangents[i].0 * (u3 - u2)
                }
                _ => a,
            });
        }
        lerp_keys(&self.keys, t, |a, b, f| {
            // Public NifSkope / OpenMW float and colour evaluators select
            // the nearest key for CONST_KEY (unlike boolean visibility).
            if self.interpolation == 5 {
                if f < 0.5 { a } else { b }
            } else {
                a.lerp(b, f)
            }
        })
    }
}

pub(crate) fn timing(r: &mut Reader) -> Result<(Ref, Timing)> {
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

/// `NiBoolData`: byte keys, as 0 / 1 in x.
pub(crate) fn bool_keys(r: &mut Reader) -> Result<ValueKeys> {
    let (keys, interpolation, tangents, _) =
        key_group_with_interpolation(r, |r| Ok(Vec3::new(r.u8()? as f32, 0.0, 0.0)))?;
    Ok(ValueKeys {
        keys,
        interpolation,
        tangents,
    })
}

pub(crate) fn value_keys(r: &mut Reader, point3: bool) -> Result<ValueKeys> {
    let (keys, interpolation, mut tangents, tbc) = if point3 {
        key_group_with_interpolation(r, |r| r.vec3())?
    } else {
        key_group_with_interpolation(r, |r| Ok(Vec3::new(r.f32()?, 0.0, 0.0)))?
    };
    if interpolation == 3 {
        tangents = tbc_tangents(&keys, &tbc);
    }
    Ok(ValueKeys {
        keys,
        interpolation,
        tangents,
    })
}

/// Derive normalized Hermite tangents from tension, bias and continuity.
/// Time weighting and one-sided endpoints follow public OpenMW nifkey.hpp.
/// The schema and OpenMW disagree on serialized bias/continuity order; see
/// known_ambiguities/tbc-value-interpolation.md for the chosen schema policy.
fn tbc_tangents(keys: &[(f32, Vec3)], parameters: &[Vec3]) -> Vec<(Vec3, Vec3)> {
    let mut tangents = vec![(Vec3::ZERO, Vec3::ZERO); keys.len()];
    if keys.len() < 2 {
        return tangents;
    }
    for (i, &(time, value)) in keys.iter().enumerate() {
        let Vec3 {
            x: tension,
            y: bias,
            z: continuity,
        } = parameters[i];
        let a = (1.0 - tension) * (1.0 - continuity) * (1.0 + bias);
        let b = (1.0 - tension) * (1.0 + continuity) * (1.0 - bias);
        let c = (1.0 - tension) * (1.0 + continuity) * (1.0 + bias);
        let d = (1.0 - tension) * (1.0 - continuity) * (1.0 - bias);
        tangents[i] = if i == 0 || i + 1 == keys.len() {
            let delta = if i == 0 {
                keys[1].1 - value
            } else {
                value - keys[i - 1].1
            };
            (delta * ((a + b) * 0.5), delta * ((c + d) * 0.5))
        } else {
            let (prev_time, prev_value) = keys[i - 1];
            let (next_time, next_value) = keys[i + 1];
            let span = next_time - prev_time;
            if span == 0.0 {
                continue;
            }
            let prev_delta = value - prev_value;
            let next_delta = next_value - value;
            (
                (prev_delta * a + next_delta * b) * ((time - prev_time) / span),
                (prev_delta * c + next_delta * d) * ((next_time - time) / span),
            )
        };
    }
    tangents
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
    value: impl FnMut(&mut Reader) -> Result<T>,
) -> Result<Vec<(f32, T)>> {
    key_group_with_interpolation(r, value).map(|(keys, _, _, _)| keys)
}

fn key_group_with_interpolation<T>(
    r: &mut Reader,
    mut value: impl FnMut(&mut Reader) -> Result<T>,
) -> Result<(Vec<(f32, T)>, u32, Vec<(T, T)>, Vec<Vec3>)> {
    let n = r.u32()?;
    if n == 0 {
        return Ok((Vec::new(), 1, Vec::new(), Vec::new()));
    }
    let interp = r.u32()?;
    let mut out = Vec::with_capacity(n.min(4096) as usize);
    let mut tangents = Vec::new();
    let mut tbc = Vec::new();
    for _ in 0..n {
        let t = r.f32()?;
        let v = value(r)?;
        match interp {
            2 => {
                tangents.push((value(r)?, value(r)?));
            }
            3 => {
                // Serialized order is tension, bias, continuity (NifTools).
                tbc.push(r.vec3()?);
            }
            _ => {}
        }
        out.push((t, v));
    }
    Ok((out, interp, tangents, tbc))
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

    fn parsed_value_keys(point3: bool, interpolation: u32) -> super::ValueKeys {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&3u32.to_le_bytes());
        bytes.extend_from_slice(&interpolation.to_le_bytes());
        for (time, value) in [(2.0f32, 0.0f32), (6.0, 8.0), (10.0, 16.0)] {
            bytes.extend_from_slice(&time.to_le_bytes());
            for component in 0..if point3 { 3 } else { 1 } {
                bytes.extend_from_slice(&(value + component as f32).to_le_bytes());
            }
            // Nonlinear payloads must consume their extra fields.
            for _ in 0..match interpolation {
                2 => {
                    if point3 {
                        6
                    } else {
                        2
                    }
                }
                3 => 3,
                _ => 0,
            } {
                bytes.extend_from_slice(&0.0f32.to_le_bytes());
            }
        }
        let mut r = crate::reader::Reader::new(&bytes);
        let Some(crate::Block::ValueKeys(keys)) =
            crate::blocks::parse_block(if point3 { "NiPosData" } else { "NiFloatData" }, &mut r)
                .unwrap()
        else {
            panic!("value data not dispatched");
        };
        assert_eq!(r.remaining(), 0);
        assert_eq!(keys.interpolation, interpolation);
        keys
    }

    #[test]
    fn constant_float_and_colour_keys_select_nearest_endpoint() {
        for point3 in [false, true] {
            let keys = parsed_value_keys(point3, 5);
            for (time, expected) in [
                (-1.0, 0.0),
                (2.0, 0.0),
                (3.99, 0.0),
                (4.0, 8.0),
                (4.01, 8.0),
                (6.0, 8.0),
                (7.99, 8.0),
                (8.0, 16.0),
                (10.0, 16.0),
                (20.0, 16.0),
            ] {
                let value = keys.sample(time).unwrap();
                assert_eq!(value.x, expected, "point3={point3}, time={time}");
                if point3 {
                    assert_eq!(value.y, expected + 1.0);
                    assert_eq!(value.z, expected + 2.0);
                }
            }
        }
    }

    #[test]
    fn linear_and_neutral_tbc_collinear_keys_still_blend() {
        for point3 in [false, true] {
            for interpolation in [1, 3] {
                let keys = parsed_value_keys(point3, interpolation);
                assert_eq!(keys.sample(3.0).unwrap().x, 2.0);
                assert_eq!(keys.sample(5.0).unwrap().x, 6.0);
            }
        }
    }

    #[test]
    fn tbc_parser_uses_schema_order_and_nonuniform_tangent_weights() {
        for point3 in [false, true] {
            let mut bytes = [3u32.to_le_bytes(), 3u32.to_le_bytes()].concat();
            // Distinct bias and continuity detect an accidental swap. Different
            // segment lengths detect a missing time weight in interior tangents.
            for (time, value) in [(2.0f32, 0.0f32), (6.0, 8.0), (14.0, 4.0)] {
                bytes.extend_from_slice(&time.to_le_bytes());
                for axis in 0..if point3 { 3 } else { 1 } {
                    bytes.extend_from_slice(&(value * (axis + 1) as f32).to_le_bytes());
                }
                for parameter in [0.5f32, 0.25, -0.5] {
                    // tension, bias, continuity
                    bytes.extend_from_slice(&parameter.to_le_bytes());
                }
            }
            let mut reader = crate::reader::Reader::new(&bytes);
            let keys = super::value_keys(&mut reader, point3).unwrap();
            assert_eq!(reader.remaining(), 0);
            assert_eq!(keys.tangents[0].0.x, 4.5);
            assert_eq!(keys.tangents[0].1.x, 3.5);
            assert_eq!(keys.tangents[1].0.x, 2.25);
            assert!((keys.tangents[1].1.x - 1.0 / 6.0).abs() < 1e-6);
            assert_eq!(keys.tangents[2].0.x, -2.25);
            assert_eq!(keys.tangents[2].1.x, -1.75);
            for (time, expected) in [(-1.0, 0.0), (3.0, 1.63671875), (14.0, 4.0), (30.0, 4.0)] {
                let sampled = keys.sample(time).unwrap();
                assert_eq!(sampled.x, expected);
                if point3 {
                    assert_eq!(
                        sampled,
                        glam::Vec3::new(expected, expected * 2.0, expected * 3.0)
                    );
                }
            }
            assert_eq!(keys.sample(f32::NAN), None);
            assert_eq!(keys.sample(3.0).unwrap().x, 1.63671875); // backward seek
            for length in 0..bytes.len() {
                let mut reader = crate::reader::Reader::new(&bytes[..length]);
                assert!(
                    super::value_keys(&mut reader, point3).is_err(),
                    "length {length}"
                );
            }
        }
    }

    #[test]
    fn tbc_single_key_full_tension_and_zero_time_spans() {
        let mut keys = parsed_value_keys(false, 3);
        // Full tension zeroes both tangents, giving the same eased segment as
        // a zero-tangent quadratic, while neutral TBC remained linear above.
        keys.tangents = super::tbc_tangents(&keys.keys, &[glam::Vec3::X; 3]);
        assert_eq!(keys.sample(3.0).unwrap().x, 1.25);
        keys.keys.truncate(1);
        keys.tangents = super::tbc_tangents(&keys.keys, &[glam::Vec3::ZERO]);
        for t in [-1.0, 2.0, 20.0] {
            assert_eq!(keys.sample(t), Some(glam::Vec3::ZERO));
        }
        assert!(super::tbc_tangents(&[], &[]).is_empty());
        let duplicate = [
            (1.0, glam::Vec3::ZERO),
            (1.0, glam::Vec3::ONE),
            (1.0, glam::Vec3::X),
        ];
        let tangents = super::tbc_tangents(&duplicate, &[glam::Vec3::ZERO; 3]);
        assert_eq!(tangents[1], (glam::Vec3::ZERO, glam::Vec3::ZERO));
    }

    #[test]
    fn neutral_tbc_preserves_linear_ramp_with_unequal_time_spacing() {
        let values = [
            (2.0, glam::Vec3::ZERO),
            (6.0, glam::Vec3::splat(8.0)),
            (14.0, glam::Vec3::splat(24.0)),
        ];
        let keys = super::ValueKeys {
            keys: values.to_vec(),
            interpolation: 3,
            tangents: super::tbc_tangents(&values, &[glam::Vec3::ZERO; 3]),
        };
        assert_eq!(
            keys.tangents[1],
            (glam::Vec3::splat(8.0), glam::Vec3::splat(16.0))
        );
        for t in [2.0, 3.0, 5.0, 6.0, 7.0, 10.0, 13.0, 14.0] {
            assert_eq!(keys.sample(t).unwrap(), glam::Vec3::splat((t - 2.0) * 2.0));
        }
    }

    #[test]
    fn quadratic_zero_tangents_ease_and_hold_endpoints() {
        for point3 in [false, true] {
            let keys = parsed_value_keys(point3, 2);
            assert_eq!(keys.sample(f32::NAN), None);
            for (time, expected) in [
                (-1.0, 0.0),
                (2.0, 0.0),
                (3.0, 1.25),
                (5.0, 6.75),
                (6.0, 8.0),
                (10.0, 16.0),
                (20.0, 16.0),
            ] {
                let value = keys.sample(time).unwrap();
                assert_eq!(value.x, expected);
                if point3 {
                    assert_eq!(value.y, expected + 1.0);
                    assert_eq!(value.z, expected + 2.0);
                }
            }
        }
    }

    #[test]
    fn quadratic_parser_preserves_tangent_order_vectors_and_normalized_units() {
        for point3 in [false, true] {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(&2u32.to_le_bytes());
            bytes.extend_from_slice(&2u32.to_le_bytes());
            // The unused endpoint tangents deliberately differ. Four-second
            // spacing catches accidental multiplication by segment duration.
            for (time, value, forward, backward) in [
                (2.0f32, [0.0f32; 3], [99.0; 3], [8.0, 16.0, 24.0]),
                (6.0, [8.0, 16.0, 24.0], [0.0, 8.0, 16.0], [-99.0; 3]),
            ] {
                bytes.extend_from_slice(&time.to_le_bytes());
                for field in [value, forward, backward] {
                    for component in &field[..if point3 { 3 } else { 1 }] {
                        bytes.extend_from_slice(&component.to_le_bytes());
                    }
                }
            }
            let mut reader = crate::reader::Reader::new(&bytes);
            let keys = super::value_keys(&mut reader, point3).unwrap();
            assert_eq!(reader.remaining(), 0);
            assert_eq!(keys.tangents[0].0.x, 99.0);
            assert_eq!(keys.tangents[0].1.x, 8.0);
            let value = keys.sample(3.0).unwrap();
            assert_eq!(value.x, 2.375);
            if point3 {
                assert_eq!(value, glam::Vec3::new(2.375, 4.375, 6.375));
            }
            // Seeking backwards has no cursor state; each segment stays exact.
            assert_eq!(keys.sample(6.0).unwrap().x, 8.0);
            assert_eq!(keys.sample(3.0).unwrap().x, 2.375);
            let mut truncated = crate::reader::Reader::new(&bytes[..bytes.len() - 1]);
            assert!(super::value_keys(&mut truncated, point3).is_err());
        }
    }

    #[test]
    fn empty_value_group_has_no_interpolation_field() {
        for point3 in [false, true] {
            let bytes = [0u32.to_le_bytes(), 0x12345678u32.to_le_bytes()].concat();
            let mut r = crate::reader::Reader::new(&bytes);
            let keys = super::value_keys(&mut r, point3).unwrap();
            assert_eq!(keys.sample(0.0), None);
            assert_eq!(r.u32().unwrap(), 0x12345678);
        }
    }

    #[test]
    fn one_constant_key_holds_at_all_times() {
        let keys = super::ValueKeys {
            keys: vec![(3.0, glam::Vec3::ONE)],
            interpolation: 5,
            tangents: Vec::new(),
        };
        for time in [-10.0, 3.0, 20.0] {
            assert_eq!(keys.sample(time), Some(glam::Vec3::ONE));
        }
    }

    #[test]
    fn alpha_test_controller_layout() {
        // Synthetic SSE NiTimeController + NiSingleInterpController payload.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&7i32.to_le_bytes());
        bytes.extend_from_slice(&76u16.to_le_bytes());
        for v in [2.0f32, 0.5, 1.0, 4.0] {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        bytes.extend_from_slice(&3i32.to_le_bytes()); // target
        bytes.extend_from_slice(&9i32.to_le_bytes()); // interpolator
        let mut r = crate::reader::Reader::new(&bytes);
        let Some(crate::Block::AlphaTestController(c)) =
            crate::blocks::parse_block("BSNiAlphaPropertyTestRefController", &mut r).unwrap()
        else {
            panic!("alpha-test controller not dispatched");
        };
        assert_eq!(r.remaining(), 0);
        assert_eq!(c.next.0, 7);
        assert_eq!(c.interpolator.0, 9);
        assert_eq!(c.timing.flags, 76);
        assert_eq!(c.timing.key_time(1.0), 2.5);
    }

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

    #[test]
    fn reverse_key_time_uses_start_relative_phase_on_both_legs() {
        let t = Timing {
            start: 3.0,
            stop: 5.0,
            frequency: 2.0,
            phase: 1.0,
            ..timing(1)
        };
        // The scaled clock reaches start at t=1, stop at t=2 and returns
        // to start at t=3. Include times before start and skipped cycles.
        for (clock, expected) in [
            (-1.0, 3.0),
            (-0.5, 4.0),
            (0.0, 5.0),
            (0.5, 4.0),
            (1.0, 3.0),
            (1.5, 4.0),
            (2.0, 5.0),
            (2.5, 4.0),
            (3.0, 3.0),
            (3.5, 4.0),
            (9.5, 4.0),
        ] {
            assert_eq!(t.key_time(clock), expected, "clock={clock}");
        }
    }

    #[test]
    fn key_time_nonzero_loop_clamp_and_degenerate_ranges() {
        let mut t = Timing {
            start: 3.0,
            stop: 5.0,
            frequency: 2.0,
            phase: 1.0,
            ..timing(0)
        };
        assert_eq!(t.key_time(1.0), 3.0);
        assert_eq!(t.key_time(1.5), 4.0);
        assert_eq!(t.key_time(2.0), 3.0);
        t.flags = timing(2).flags;
        assert_eq!(t.key_time(0.0), 3.0);
        assert_eq!(t.key_time(1.5), 4.0);
        assert_eq!(t.key_time(2.5), 5.0);
        for stop in [3.0, 2.0] {
            t.stop = stop;
            for cycle in [0, 1, 2] {
                t.flags = timing(cycle).flags;
                assert_eq!(t.key_time(-100.0), 3.0);
                assert_eq!(t.key_time(100.0), 3.0);
            }
        }
    }
}
