use glam::{Quat, Vec3, Vec4};

use crate::packfile::Packfile;
use crate::spline::SplineAnimation;
use crate::{Error, Result};

/// Translation, rotation, scale.
#[derive(Debug, Clone, Copy)]
pub struct QsTransform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for QsTransform {
    fn default() -> Self {
        QsTransform { translation: Vec3::ZERO, rotation: Quat::IDENTITY, scale: Vec3::ONE }
    }
}

impl QsTransform {
    pub fn lerp(&self, o: &QsTransform, t: f32) -> QsTransform {
        QsTransform {
            translation: self.translation.lerp(o.translation, t),
            rotation: self.rotation.slerp(o.rotation, t),
            scale: self.scale.lerp(o.scale, t),
        }
    }
}

fn vec4(p: &Packfile, o: u32) -> Vec4 {
    Vec4::new(p.f32(o), p.f32(o + 4), p.f32(o + 8), p.f32(o + 12))
}

fn qs(p: &Packfile, o: u32) -> QsTransform {
    let t = vec4(p, o);
    let r = vec4(p, o + 16);
    let s = vec4(p, o + 32);
    QsTransform { translation: t.truncate(), rotation: Quat::from_xyzw(r.x, r.y, r.z, r.w), scale: s.truncate() }
}

#[derive(Debug, Clone)]
pub struct SkeletonBone {
    pub name: String,
    pub parent: Option<usize>,
    pub reference: QsTransform,
}

#[derive(Debug, Clone)]
pub struct Skeleton {
    pub name: String,
    pub bones: Vec<SkeletonBone>,
}

#[derive(Debug, Clone)]
pub struct Annotation {
    pub time: f32,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct Binding {
    pub skeleton_name: String,
    /// Transform track -> skeleton bone index (empty = identity).
    pub track_to_bone: Vec<i16>,
    /// `blendHint` ADDITIVE: the tracks are offsets to add to a pose, not a pose.
    pub additive: bool,
}

pub enum AnimationData {
    Spline(SplineAnimation),
    Interleaved { frames: usize, transforms: Vec<QsTransform> },
}

pub struct Animation {
    pub duration: f32,
    pub num_tracks: usize,
    pub data: AnimationData,
    pub annotations: Vec<Annotation>,
    pub binding: Option<Binding>,
}

impl Animation {
    /// Sample all transform tracks at time `t` (seconds), clamped to the clip.
    pub fn sample(&self, t: f32, out: &mut Vec<QsTransform>) {
        out.clear();
        out.resize(self.num_tracks, QsTransform::default());
        let t = t.clamp(0.0, self.duration);
        match &self.data {
            AnimationData::Spline(s) => s.sample(t, out),
            AnimationData::Interleaved { frames, transforms } => {
                if *frames == 0 {
                    return;
                }
                let f = if self.duration > 0.0 { t / self.duration * (*frames as f32 - 1.0) } else { 0.0 };
                let f0 = (f.floor() as usize).min(frames - 1);
                let f1 = (f0 + 1).min(frames - 1);
                let a = f - f0 as f32;
                for (i, o) in out.iter_mut().enumerate() {
                    let x = transforms.get(f0 * self.num_tracks + i).copied().unwrap_or_default();
                    let y = transforms.get(f1 * self.num_tracks + i).copied().unwrap_or_default();
                    *o = x.lerp(&y, a);
                }
            }
        }
    }
}

pub struct AnimationContainer {
    pub skeletons: Vec<Skeleton>,
    pub animations: Vec<Animation>,
}

impl AnimationContainer {
    pub fn parse(bytes: &[u8]) -> Result<AnimationContainer> {
        let p = Packfile::parse(bytes)?;
        let ac = p
            .objects_of("hkaAnimationContainer")
            .next()
            .ok_or_else(|| Error::Corrupt("no hkaAnimationContainer".into()))?;
        let mut skeletons = Vec::new();
        let (arr, n) = p.array(ac + 16);
        if let Some(arr) = arr {
            for i in 0..n as u32 {
                if let Some(s) = p.ptr(arr + i * 8) {
                    skeletons.push(read_skeleton(&p, s));
                }
            }
        }
        // Bindings map animations to skeleton bones.
        let mut bindings: Vec<(u32, Binding)> = Vec::new();
        let (arr, n) = p.array(ac + 48);
        if let Some(arr) = arr {
            for i in 0..n as u32 {
                if let Some(b) = p.ptr(arr + i * 8) {
                    let anim = p.ptr(b + 24).unwrap_or(u32::MAX);
                    let (ta, tn) = p.array(b + 32);
                    let track_to_bone = ta.map(|ta| (0..tn as u32).map(|k| p.i16(ta + k * 2)).collect()).unwrap_or_default();
                    let additive = p.u8(b + 64) == 1;
                    bindings.push((anim, Binding { skeleton_name: p.string(b + 16).unwrap_or_default(), track_to_bone, additive }));
                }
            }
        }
        let mut animations = Vec::new();
        let (arr, n) = p.array(ac + 32);
        if let Some(arr) = arr {
            for i in 0..n as u32 {
                let Some(a) = p.ptr(arr + i * 8) else { continue };
                let mut anim = read_animation(&p, a)?;
                anim.binding = bindings.iter().find(|b| b.0 == a).map(|b| b.1.clone());
                animations.push(anim);
            }
        }
        Ok(AnimationContainer { skeletons, animations })
    }
}

fn read_skeleton(p: &Packfile, s: u32) -> Skeleton {
    let name = p.string(s + 16).unwrap_or_default();
    let (pa, pn) = p.array(s + 24);
    let (ba, bn) = p.array(s + 40);
    let (ra, rn) = p.array(s + 56);
    let mut bones = Vec::with_capacity(bn);
    for i in 0..bn as u32 {
        let bname = ba.and_then(|ba| p.string(ba + i * 16)).unwrap_or_default();
        let parent = if (i as usize) < pn { pa.map(|pa| p.i16(pa + i * 2)).unwrap_or(-1) } else { -1 };
        let reference = if (i as usize) < rn { ra.map(|ra| qs(p, ra + i * 48)).unwrap_or_default() } else { QsTransform::default() };
        bones.push(SkeletonBone { name: bname, parent: if parent >= 0 { Some(parent as usize) } else { None }, reference });
    }
    Skeleton { name, bones }
}

fn read_animation(p: &Packfile, a: u32) -> Result<Animation> {
    let class = p.object_class(a).unwrap_or("").to_owned();
    let duration = p.f32(a + 20);
    let num_tracks = p.i32(a + 24).max(0) as usize;
    let num_floats = p.i32(a + 28).max(0) as usize;
    let mut annotations = Vec::new();
    let (ta, tn) = p.array(a + 40);
    if let Some(ta) = ta {
        for i in 0..tn as u32 {
            let track = ta + i * 24;
            let (aa, an) = p.array(track + 8);
            if let Some(aa) = aa {
                for k in 0..an as u32 {
                    let e = aa + k * 16;
                    annotations.push(Annotation { time: p.f32(e), text: p.string(e + 8).unwrap_or_default() });
                }
            }
        }
    }
    annotations.sort_by(|x, y| x.time.total_cmp(&y.time));
    let data = match class.as_str() {
        "hkaSplineCompressedAnimation" => AnimationData::Spline(SplineAnimation::read(p, a, num_tracks, num_floats)?),
        "hkaInterleavedUncompressedAnimation" => {
            let (arr, n) = p.array(a + 56);
            let transforms: Vec<QsTransform> = arr.map(|arr| (0..n as u32).map(|i| qs(p, arr + i * 48)).collect()).unwrap_or_default();
            let frames = if num_tracks > 0 { transforms.len() / num_tracks } else { 0 };
            AnimationData::Interleaved { frames, transforms }
        }
        other => return Err(Error::Unsupported(format!("animation class {other}"))),
    };
    Ok(Animation { duration, num_tracks, data, annotations, binding: None })
}
