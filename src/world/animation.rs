//! Playing Havok animation clips on actor skeletons.

use std::collections::HashMap;
use std::sync::Arc;

use glam::Quat;
use nif::Transform;

use super::skeleton::Skeleton;

/// A clip whose tracks are already mapped onto a NIF skeleton's bones.
pub struct BoundClip {
    pub anim: havok::Animation,
    /// Transform track -> NIF skeleton bone index.
    pub track_to_bone: Vec<Option<usize>>,
}

/// Runtime playback state for one actor.
pub struct ActorAnim {
    pub clip: Arc<BoundClip>,
    pub time: f32,
    pub speed: f32,
    scratch: Vec<havok::QsTransform>,
    locals: Vec<Transform>,
}

impl ActorAnim {
    pub fn new(clip: Arc<BoundClip>, skeleton: &Skeleton, start: f32) -> Self {
        let duration = clip.anim.duration.max(0.001);
        ActorAnim { clip, time: start % duration, speed: 1.0, scratch: Vec::new(), locals: skeleton.bind_locals() }
    }

    /// Advance and return the new model-space pose.
    pub fn update(&mut self, skeleton: &Skeleton, dt: f32) -> Vec<glam::Mat4> {
        let d = self.clip.anim.duration.max(0.001);
        self.time = (self.time + dt * self.speed) % d;
        self.clip.anim.sample(self.time, &mut self.scratch);
        for (track, q) in self.scratch.iter().enumerate() {
            if let Some(Some(b)) = self.clip.track_to_bone.get(track) {
                self.locals[*b] = Transform {
                    translation: q.translation,
                    rotation: glam::Mat3::from_quat(if q.rotation.is_finite() { q.rotation } else { Quat::IDENTITY }),
                    scale: q.scale.x,
                };
            }
        }
        skeleton.model_space(&self.locals)
    }
}

/// Loads and caches clips per (clip path, skeleton path).
#[derive(Default)]
pub struct AnimationLibrary {
    havok_skeletons: HashMap<String, Option<Arc<havok::Skeleton>>>,
    clips: HashMap<(String, String), Option<Arc<BoundClip>>>,
}

impl AnimationLibrary {
    fn havok_skeleton(&mut self, vfs: &vfs::Vfs, path: &str) -> Option<Arc<havok::Skeleton>> {
        if let Some(s) = self.havok_skeletons.get(path) {
            return s.clone();
        }
        let s = vfs
            .read(path)
            .and_then(|b| havok::AnimationContainer::parse(&b).map_err(|e| log::warn!("{path}: {e}")).ok())
            .and_then(|c| c.skeletons.into_iter().next())
            .map(Arc::new);
        self.havok_skeletons.insert(path.to_owned(), s.clone());
        s
    }

    /// Load `clip` for an actor whose NIF skeleton lives at `nif_skeleton_path`.
    pub fn clip(&mut self, vfs: &vfs::Vfs, clip: &str, nif_skeleton_path: &str, skeleton: &Skeleton) -> Option<Arc<BoundClip>> {
        let key = (clip.to_owned(), nif_skeleton_path.to_owned());
        if let Some(c) = self.clips.get(&key) {
            return c.clone();
        }
        let hkx_skel = nif_skeleton_path.strip_suffix(".nif").map(|s| format!("{s}.hkx"));
        let hk = hkx_skel.and_then(|p| self.havok_skeleton(vfs, &p));
        let result = (|| {
            let bytes = vfs.read(clip)?;
            let container = havok::AnimationContainer::parse(&bytes).map_err(|e| log::warn!("{clip}: {e}")).ok()?;
            let anim = container.animations.into_iter().next()?;
            let hk = hk.as_ref()?;
            let track_to_bone = (0..anim.num_tracks)
                .map(|t| {
                    let bone = match &anim.binding {
                        Some(b) if !b.track_to_bone.is_empty() => *b.track_to_bone.get(t)? as usize,
                        _ => t,
                    };
                    let name = &hk.bones.get(bone)?.name;
                    skeleton.find(name)
                })
                .collect();
            Some(Arc::new(BoundClip { anim, track_to_bone }))
        })();
        if result.is_none() {
            log::debug!("could not load clip {clip} for {nif_skeleton_path}");
        }
        self.clips.insert(key, result.clone());
        result
    }
}

/// Default idle clip for an actor given its skeleton path.
pub fn idle_clip(skeleton_path: &str, female: bool) -> Vec<String> {
    let base = skeleton_path.split("/character assets").next().unwrap_or("").to_owned();
    if base.ends_with("actors/character") {
        let g = if female { "female" } else { "male" };
        vec![format!("{base}/animations/{g}/mt_idle.hkx"), format!("{base}/animations/mt_idle_a_base.hkx")]
    } else {
        vec![format!("{base}/animations/mt_idle.hkx"), format!("{base}/animations/idle.hkx")]
    }
}
