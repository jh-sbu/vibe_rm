//! Actors animated by their behaviour graphs: a [`havok::behavior::runtime::Instance`]
//! per actor, its clip samples blended into a pose, root motion taken from them.

use std::collections::HashMap;
use std::sync::Arc;

use glam::{Mat4, Quat, Vec3};
use havok::behavior::runtime::{Instance, Raised, Shared};
use nif::Transform;

use super::animation::{AnimationLibrary, BoundClip, project_clip_paths};
use super::skeleton::Skeleton;

/// Behaviour projects' shared runtime tables, by project directory.
#[derive(Default)]
pub struct GraphLibrary {
    shared: HashMap<String, Option<Arc<Shared>>>,
}

impl GraphLibrary {
    /// Runtime tables for the character project at `dir` (e.g. `meshes/actors/character`).
    pub fn shared(&mut self, vfs: &vfs::Vfs, dir: &str) -> Option<Arc<Shared>> {
        self.shared
            .entry(dir.to_owned())
            .or_insert_with(|| {
                let t = std::time::Instant::now();
                let p = havok::behavior::Project::load("behaviors/0_master.hkx", |rel| vfs.read(&format!("{dir}/{rel}")));
                if p.graphs.is_empty() {
                    return None;
                }
                let s = Shared::new(Arc::new(p));
                log::debug!("{dir}: behaviour runtime in {:?}", t.elapsed());
                Some(s)
            })
            .clone()
    }
}

/// What a graph update produced.
pub struct Frame {
    pub pose: Vec<Mat4>,
    /// Root motion this update, in the actor's frame (+Y forward), and its
    /// counter-clockwise yaw.
    pub motion: (Vec3, f32),
    /// Events the graph raised (`AnimObjDraw`, `IdleFurnitureExit`...).
    pub raised: Vec<Raised>,
}

/// One actor's running behaviour graph.
pub struct GraphAnim {
    /// For logs (the actor's reference).
    pub label: String,
    inst: Instance,
    project: String,
    skeleton_path: String,
    female: bool,
    clips: HashMap<Arc<str>, Option<Arc<BoundClip>>>,
    bind: Vec<Transform>,
    scratch: Vec<havok::QsTransform>,
}

/// Clip lookup for one actor while the graph runs.
struct Source<'a> {
    clips: &'a mut HashMap<Arc<str>, Option<Arc<BoundClip>>>,
    vfs: &'a vfs::Vfs,
    anims: &'a mut AnimationLibrary,
    skeleton: &'a Skeleton,
    project: &'a str,
    skeleton_path: &'a str,
    female: bool,
}

impl Source<'_> {
    fn clip(&mut self, animation: &str) -> Option<Arc<BoundClip>> {
        if let Some(c) = self.clips.get(animation) {
            return c.clone();
        }
        let c = project_clip_paths(self.project, animation, self.female)
            .iter()
            .find_map(|p| self.anims.clip(self.vfs, p, self.skeleton_path, self.skeleton));
        self.clips.insert(animation.into(), c.clone());
        c
    }
}

impl havok::behavior::runtime::ClipSource for Source<'_> {
    fn duration(&mut self, animation: &str) -> Option<f32> {
        self.clip(animation).map(|c| c.duration())
    }

    fn additive(&mut self, animation: &str) -> bool {
        self.clip(animation).is_some_and(|c| c.additive)
    }
}

impl GraphAnim {
    pub fn new(shared: Arc<Shared>, project: &str, skeleton_path: &str, female: bool, skeleton: &Skeleton, seed: u64) -> GraphAnim {
        GraphAnim {
            label: String::new(),
            inst: Instance::new(shared, seed),
            project: project.to_owned(),
            skeleton_path: skeleton_path.to_owned(),
            female,
            clips: HashMap::new(),
            bind: skeleton.bind_locals(),
            scratch: Vec::new(),
        }
    }

    fn source<'a>(&'a mut self, vfs: &'a vfs::Vfs, anims: &'a mut AnimationLibrary, skeleton: &'a Skeleton) -> (&'a mut Instance, Source<'a>) {
        let GraphAnim { inst, project, skeleton_path, female, clips, .. } = self;
        (inst, Source { clips, vfs, anims, skeleton, project, skeleton_path, female: *female })
    }

    pub fn set_variable(&mut self, name: &str, value: f32) -> bool {
        self.inst.set_variable(name, value)
    }

    /// Queue an event for the next update.
    pub fn send_event(&mut self, name: &str) -> bool {
        self.inst.send_event(name)
    }

    /// Handle an event now; true when the graph took it (changed state).
    pub fn handle_event(&mut self, name: &str, vfs: &vfs::Vfs, anims: &mut AnimationLibrary, skeleton: &Skeleton) -> bool {
        let (inst, mut src) = self.source(vfs, anims, skeleton);
        inst.handle_event(name, &mut src)
    }

    pub fn active_states(&self) -> Vec<String> {
        self.inst.active_states()
    }

    /// Advance the graph and blend its clips.
    pub fn update(&mut self, dt: f32, vfs: &vfs::Vfs, anims: &mut AnimationLibrary, skeleton: &Skeleton) -> Frame {
        let (inst, mut src) = self.source(vfs, anims, skeleton);
        inst.update(dt, &mut src);
        let samples = inst.samples();
        let raised = inst.take_raised();
        let clips: Vec<Option<Arc<BoundClip>>> = samples.iter().map(|s| src.clip(&s.animation)).collect();

        // Per bone: weight, translation, rotation (sign-aligned sum), scale.
        let n = self.bind.len();
        let mut weight = vec![0.0f32; n];
        let mut trans = vec![Vec3::ZERO; n];
        let mut rot = vec![glam::Vec4::ZERO; n];
        let mut scale = vec![0.0f32; n];
        let mut motion = (Vec3::ZERO, 0.0f32);
        let mut motion_weight = 0.0;
        for (s, clip) in samples.iter().zip(&clips) {
            let Some(clip) = clip else { continue };
            if s.additive {
                continue;
            }
            clip.anim.sample(s.time, &mut self.scratch);
            for (track, q) in self.scratch.iter().enumerate() {
                let Some(Some(b)) = clip.track_to_bone.get(track) else { continue };
                let factor = match (&s.mask, clip.track_to_hk.get(track).copied().flatten()) {
                    (Some(m), Some(hk)) => m.get(hk).copied().unwrap_or(0.0),
                    _ => 1.0,
                };
                let w = s.weight * factor;
                if w <= 0.0 {
                    continue;
                }
                let r = if q.rotation.is_finite() { q.rotation } else { Quat::IDENTITY };
                let v = glam::Vec4::from(r);
                let v = if rot[*b].dot(v) < 0.0 { -v } else { v };
                weight[*b] += w;
                trans[*b] += q.translation * w;
                rot[*b] += v * w;
                scale[*b] += q.scale.x * w;
            }
            let (d, yaw) = clip.motion.as_ref().map_or((Vec3::ZERO, 0.0), |m| m.delta(s.prev_time, s.time, s.wrapped, clip.duration()));
            motion.0 += d * s.weight;
            motion.1 += yaw * s.weight;
            motion_weight += s.weight;
        }
        if motion_weight > 1e-4 {
            motion.0 /= motion_weight;
            motion.1 /= motion_weight;
        }
        if log::log_enabled!(log::Level::Trace) {
            let bare: Vec<&str> = (0..n).filter(|&b| weight[b] <= 1e-6).filter_map(|b| skeleton.bone_name(b)).collect();
            let what: Vec<String> = samples
                .iter()
                .zip(&clips)
                .map(|(s, c)| format!("{:.2} {}{}", s.weight, s.animation, if c.is_some() { "" } else { " (missing)" }))
                .collect();
            log::trace!("{}: {what:?}; bones without weight: {bare:?}", self.label);
        }
        let mut locals: Vec<(Vec3, Quat, f32)> = (0..n)
            .map(|b| {
                let w = weight[b];
                if w <= 1e-6 {
                    let t = self.bind[b];
                    return (t.translation, Quat::from_mat3(&t.rotation), t.scale);
                }
                (trans[b] / w, Quat::from_vec4(rot[b]).normalize(), scale[b] / w)
            })
            .collect();
        // Additive clips (body weight offsets...) go on top, scaled by their weight.
        for (s, clip) in samples.iter().zip(&clips) {
            let Some(clip) = clip.as_ref().filter(|_| s.additive) else { continue };
            clip.anim.sample(s.time, &mut self.scratch);
            for (track, q) in self.scratch.iter().enumerate() {
                let Some(Some(b)) = clip.track_to_bone.get(track) else { continue };
                let factor = match (&s.mask, clip.track_to_hk.get(track).copied().flatten()) {
                    (Some(m), Some(hk)) => m.get(hk).copied().unwrap_or(0.0),
                    _ => 1.0,
                };
                // Relative to the layer it sits in: normalised per bone like the poses.
                let w = if weight[*b] > 1e-6 { (s.weight * factor / weight[*b]).clamp(0.0, 1.0) } else { 0.0 };
                if w <= 0.0 {
                    continue;
                }
                let r = if q.rotation.is_finite() { q.rotation } else { Quat::IDENTITY };
                let l = &mut locals[*b];
                l.0 += q.translation * w;
                l.1 = (l.1 * Quat::IDENTITY.slerp(r, w)).normalize();
                l.2 *= 1.0 + (q.scale.x - 1.0) * w;
            }
        }
        let locals: Vec<Transform> =
            locals.into_iter().map(|(t, q, s)| Transform { translation: t, rotation: glam::Mat3::from_quat(q), scale: s }).collect();
        Frame { pose: skeleton.model_space(&locals), motion, raised }
    }
}
