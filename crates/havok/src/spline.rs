//! hkaSplineCompressedAnimation decoding.
//!
//! Each block holds, per transform track, a quantisation/type mask followed by
//! position, rotation and scale data. Animated channels are B-splines whose
//! knots are frame indices within the block; static channels are constants.

use glam::{Quat, Vec3};

use crate::anim::QsTransform;
use crate::packfile::Packfile;
use crate::{Error, Result};

#[derive(Debug, Clone)]
enum VecTrack {
    Constant(Vec3),
    Spline { degree: usize, knots: Vec<f32>, points: Vec<Vec3> },
}

#[derive(Debug, Clone)]
enum RotTrack {
    Constant(Quat),
    Spline { degree: usize, knots: Vec<f32>, points: Vec<Quat> },
}

#[derive(Debug, Clone)]
struct Track {
    pos: VecTrack,
    rot: RotTrack,
    scale: VecTrack,
}

pub struct SplineAnimation {
    num_blocks: usize,
    block_duration: f32,
    frame_duration: f32,
    blocks: Vec<Vec<Track>>,
}

const ROT_ALIGN: [usize; 6] = [4, 1, 2, 1, 2, 4];

struct Cursor<'a> {
    d: &'a [u8],
    p: usize,
}

impl Cursor<'_> {
    fn need(&self, n: usize) -> Result<()> {
        if self.p + n > self.d.len() { Err(Error::Corrupt("spline data overrun".into())) } else { Ok(()) }
    }
    fn u8(&mut self) -> Result<u8> {
        self.need(1)?;
        self.p += 1;
        Ok(self.d[self.p - 1])
    }
    fn u16(&mut self) -> Result<u16> {
        self.need(2)?;
        let v = u16::from_le_bytes([self.d[self.p], self.d[self.p + 1]]);
        self.p += 2;
        Ok(v)
    }
    fn f32(&mut self) -> Result<f32> {
        self.need(4)?;
        let v = f32::from_le_bytes(self.d[self.p..self.p + 4].try_into().unwrap());
        self.p += 4;
        Ok(v)
    }
    fn bytes(&mut self, n: usize) -> Result<&[u8]> {
        self.need(n)?;
        self.p += n;
        Ok(&self.d[self.p - n..self.p])
    }
    /// Alignment is relative to the start of the data array.
    fn align(&mut self, a: usize) {
        if a > 1 {
            self.p = self.p.div_ceil(a) * a;
        }
    }
}

fn read_vec(c: &mut Cursor, quant: u8, types: u8, default: f32) -> Result<VecTrack> {
    let spline = (types >> 4) & 7;
    let stat = types & 7;
    if spline != 0 {
        let n = c.u16()? as usize;
        let degree = c.u8()? as usize;
        let knots: Vec<f32> = c.bytes(n + degree + 2)?.iter().map(|&k| k as f32).collect();
        c.align(4);
        let mut min = [0f32; 3];
        let mut max = [0f32; 3];
        let mut base = [default; 3];
        for axis in 0..3 {
            if spline & (1 << axis) != 0 {
                min[axis] = c.f32()?;
                max[axis] = c.f32()?;
            } else if stat & (1 << axis) != 0 {
                base[axis] = c.f32()?;
            }
        }
        let mut points = Vec::with_capacity(n + 1);
        for _ in 0..=n {
            let mut p = base;
            for axis in 0..3 {
                if spline & (1 << axis) != 0 {
                    let q = if quant == 0 { c.u8()? as f32 / 255.0 } else { c.u16()? as f32 / 65535.0 };
                    p[axis] = min[axis] + (max[axis] - min[axis]) * q;
                }
            }
            points.push(Vec3::from_array(p));
        }
        c.align(4);
        Ok(VecTrack::Spline { degree, knots, points })
    } else {
        let mut v = [default; 3];
        for (axis, x) in v.iter_mut().enumerate() {
            if stat & (1 << axis) != 0 {
                *x = c.f32()?;
            }
        }
        c.align(4);
        Ok(VecTrack::Constant(Vec3::from_array(v)))
    }
}

fn reorder(shift: u64, a: f32, b: f32, c3: f32, w: f32) -> Quat {
    match shift {
        0 => Quat::from_xyzw(w, a, b, c3),
        1 => Quat::from_xyzw(a, w, b, c3),
        2 => Quat::from_xyzw(a, b, w, c3),
        _ => Quat::from_xyzw(a, b, c3, w),
    }
}

fn read_quat(c: &mut Cursor, quant: u8) -> Result<Quat> {
    let q = match quant {
        0 => {
            // POLAR32
            let v = u32::from_le_bytes(c.bytes(4)?.try_into().unwrap());
            polar32(v)
        }
        1 => {
            // THREECOMP40
            let b = c.bytes(5)?;
            let v = b[0] as u64 | (b[1] as u64) << 8 | (b[2] as u64) << 16 | (b[3] as u64) << 24 | (b[4] as u64) << 32;
            const MASK: u64 = (1 << 12) - 1;
            const FRAC: f32 = std::f32::consts::FRAC_1_SQRT_2 / 2047.0;
            let x = ((v & MASK) as f32 - 2047.0) * FRAC;
            let y = (((v >> 12) & MASK) as f32 - 2047.0) * FRAC;
            let z = (((v >> 24) & MASK) as f32 - 2047.0) * FRAC;
            let shift = (v >> 36) & 3;
            let mut w = (1.0 - x * x - y * y - z * z).max(0.0).sqrt();
            if (v >> 38) & 1 != 0 {
                w = -w;
            }
            reorder(shift, x, y, z, w)
        }
        2 => {
            // THREECOMP48
            let a = c.u16()? as u64;
            let b = c.u16()? as u64;
            let d = c.u16()? as u64;
            const FRAC: f32 = std::f32::consts::FRAC_1_SQRT_2 / 16383.0;
            let shift = ((b >> 14) & 2) | ((a >> 15) & 1);
            let sign = (d >> 15) != 0;
            let x = ((a & 0x7FFF) as f32 - 16383.0) * FRAC;
            let y = ((b & 0x7FFF) as f32 - 16383.0) * FRAC;
            let z = ((d & 0x7FFF) as f32 - 16383.0) * FRAC;
            let mut w = (1.0 - x * x - y * y - z * z).max(0.0).sqrt();
            if sign {
                w = -w;
            }
            reorder(shift, x, y, z, w)
        }
        3 => {
            // THREECOMP24: 3 x 7 bits + 2 bit shift + sign
            let b = c.bytes(3)?;
            let v = b[0] as u64 | (b[1] as u64) << 8 | (b[2] as u64) << 16;
            const FRAC: f32 = std::f32::consts::FRAC_1_SQRT_2 / 63.0;
            let x = ((v & 0x7F) as f32 - 63.0) * FRAC;
            let y = (((v >> 7) & 0x7F) as f32 - 63.0) * FRAC;
            let z = (((v >> 14) & 0x7F) as f32 - 63.0) * FRAC;
            let shift = (v >> 21) & 3;
            let mut w = (1.0 - x * x - y * y - z * z).max(0.0).sqrt();
            if (v >> 23) & 1 != 0 {
                w = -w;
            }
            reorder(shift, x, y, z, w)
        }
        5 => Quat::from_xyzw(c.f32()?, c.f32()?, c.f32()?, c.f32()?),
        other => return Err(Error::Unsupported(format!("rotation quantisation {other}"))),
    };
    Ok(q.normalize())
}

/// POLAR32: 10-bit radius-like magnitude plus two angles (after Havok's
/// `hkaSplineCompressedAnimation` 32-bit polar packing).
fn polar32(v: u32) -> Quat {
    const R_MASK: u32 = (1 << 10) - 1;
    const R_FRAC: f32 = 1.0 / R_MASK as f32;
    const PHI_FRAC: f32 = std::f32::consts::FRAC_PI_2 / 511.0;
    let mut r = ((v >> 18) & R_MASK) as f32 * R_FRAC;
    r = 1.0 - r * r;
    let mut phi_theta = (v & 0x3FFFF) as f32;
    let phi = (phi_theta.sqrt()).floor();
    let mut theta = 0.0;
    if phi > 0.0 {
        theta = std::f32::consts::FRAC_PI_4 * (phi_theta - phi * phi) / phi;
        phi_theta = phi * PHI_FRAC;
    } else {
        phi_theta = 0.0;
    }
    let mag = (1.0 - r * r).max(0.0).sqrt();
    let mut x = phi_theta.sin() * theta.cos() * mag;
    let mut y = phi_theta.sin() * theta.sin() * mag;
    let mut z = phi_theta.cos() * mag;
    let mut w = r;
    if v & (1 << 28) != 0 {
        x = -x;
    }
    if v & (1 << 29) != 0 {
        y = -y;
    }
    if v & (1 << 30) != 0 {
        z = -z;
    }
    if v & (1 << 31) != 0 {
        w = -w;
    }
    Quat::from_xyzw(x, y, z, w)
}

fn read_rot(c: &mut Cursor, quant: u8, types: u8) -> Result<RotTrack> {
    let align = ROT_ALIGN.get(quant as usize).copied().unwrap_or(4);
    let t = if types & 0xF0 != 0 {
        let n = c.u16()? as usize;
        let degree = c.u8()? as usize;
        let knots: Vec<f32> = c.bytes(n + degree + 2)?.iter().map(|&k| k as f32).collect();
        c.align(align);
        let mut points = Vec::with_capacity(n + 1);
        for _ in 0..=n {
            points.push(read_quat(c, quant)?);
        }
        // Keep neighbouring control points in the same hemisphere.
        for i in 1..points.len() {
            if points[i].dot(points[i - 1]) < 0.0 {
                points[i] = -points[i];
            }
        }
        RotTrack::Spline { degree, knots, points }
    } else if types & 0x0F != 0 {
        c.align(align);
        RotTrack::Constant(read_quat(c, quant)?)
    } else {
        RotTrack::Constant(Quat::IDENTITY)
    };
    c.align(4);
    Ok(t)
}

fn find_span(n: usize, p: usize, u: f32, k: &[f32]) -> usize {
    if u >= k[n + 1] {
        return n;
    }
    if u <= k[p] {
        return p;
    }
    let (mut low, mut high) = (p, n + 1);
    let mut mid = (low + high) / 2;
    while u < k[mid] || u >= k[mid + 1] {
        if u < k[mid] {
            high = mid;
        } else {
            low = mid;
        }
        let next = (low + high) / 2;
        if next == mid {
            break;
        }
        mid = next;
    }
    mid
}

fn basis(span: usize, u: f32, p: usize, k: &[f32], n_out: &mut [f32; 8]) {
    let mut left = [0f32; 8];
    let mut right = [0f32; 8];
    n_out[0] = 1.0;
    for j in 1..=p {
        left[j] = u - k[span + 1 - j];
        right[j] = k[span + j] - u;
        let mut saved = 0.0;
        for r in 0..j {
            let denom = right[r + 1] + left[j - r];
            let temp = if denom != 0.0 { n_out[r] / denom } else { 0.0 };
            n_out[r] = saved + right[r + 1] * temp;
            saved = left[j - r] * temp;
        }
        n_out[j] = saved;
    }
}

fn eval_vec(t: &VecTrack, u: f32) -> Vec3 {
    match t {
        VecTrack::Constant(v) => *v,
        VecTrack::Spline { degree, knots, points } => {
            let n = points.len() - 1;
            let p = (*degree).min(7).min(n);
            if p == 0 {
                return points[0];
            }
            let span = find_span(n, p, u, knots);
            let mut b = [0f32; 8];
            basis(span, u, p, knots, &mut b);
            let mut out = Vec3::ZERO;
            for i in 0..=p {
                out += points[span - p + i] * b[i];
            }
            out
        }
    }
}

fn eval_rot(t: &RotTrack, u: f32) -> Quat {
    match t {
        RotTrack::Constant(q) => *q,
        RotTrack::Spline { degree, knots, points } => {
            let n = points.len() - 1;
            let p = (*degree).min(7).min(n);
            if p == 0 {
                return points[0];
            }
            let span = find_span(n, p, u, knots);
            let mut b = [0f32; 8];
            basis(span, u, p, knots, &mut b);
            let mut out = glam::Vec4::ZERO;
            for i in 0..=p {
                out += glam::Vec4::from(points[span - p + i]) * b[i];
            }
            Quat::from_vec4(out).normalize()
        }
    }
}

impl SplineAnimation {
    pub fn read(p: &Packfile, a: u32, num_tracks: usize, _num_floats: usize) -> Result<SplineAnimation> {
        let num_blocks = p.i32(a + 60).max(0) as usize;
        let mask_size = p.i32(a + 68).max(0) as usize;
        let block_duration = p.f32(a + 72);
        let frame_duration = p.f32(a + 80);
        let (bo, bn) = p.array(a + 88);
        let (da, dn) = p.array(a + 152);
        let data = match da {
            Some(da) => p.data.get(da as usize..da as usize + dn).ok_or_else(|| Error::Corrupt("data out of range".into()))?,
            None => return Err(Error::Corrupt("no spline data".into())),
        };
        let mut blocks = Vec::with_capacity(num_blocks);
        for b in 0..num_blocks.min(bn) {
            let start = p.u32(bo.unwrap() + b as u32 * 4) as usize;
            let masks: Vec<[u8; 4]> =
                (0..num_tracks).map(|t| data.get(start + t * 4..start + t * 4 + 4).map(|m| m.try_into().unwrap()).unwrap_or([0; 4])).collect();
            let mut c = Cursor { d: data, p: start + mask_size };
            let mut tracks = Vec::with_capacity(num_tracks);
            for m in &masks {
                let quant = m[0];
                let pos = read_vec(&mut c, quant & 3, m[1], 0.0)?;
                let rot = read_rot(&mut c, (quant >> 2) & 0xF, m[2])?;
                let scale = read_vec(&mut c, (quant >> 6) & 3, m[3], 1.0)?;
                tracks.push(Track { pos, rot, scale });
            }
            blocks.push(tracks);
        }
        Ok(SplineAnimation { num_blocks: blocks.len(), block_duration, frame_duration, blocks })
    }

    pub fn sample(&self, t: f32, out: &mut [QsTransform]) {
        if self.blocks.is_empty() {
            return;
        }
        let b = if self.block_duration > 0.0 { ((t / self.block_duration) as usize).min(self.num_blocks - 1) } else { 0 };
        let local = t - b as f32 * self.block_duration;
        let u = if self.frame_duration > 0.0 { local / self.frame_duration } else { 0.0 };
        for (o, tr) in out.iter_mut().zip(self.blocks[b].iter()) {
            o.translation = eval_vec(&tr.pos, u);
            o.rotation = eval_rot(&tr.rot, u);
            o.scale = eval_vec(&tr.scale, u);
        }
    }
}
