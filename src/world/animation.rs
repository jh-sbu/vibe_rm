//! Playing Havok animation clips on actor skeletons.

use std::collections::HashMap;
use std::sync::Arc;

use glam::{Quat, Vec3};
use nif::Transform;

use super::behavior::ProjectRuntime;
use super::skeleton::Skeleton;

/// A clip whose tracks are already mapped onto a NIF skeleton's bones.
pub struct BoundClip {
    pub anim: Arc<havok::Animation>,
    /// Transform track -> NIF skeleton bone index.
    pub track_to_bone: Vec<Option<usize>>,
    /// Transform track -> Havok skeleton bone index (the order of behaviour bone weights).
    pub track_to_hk: Vec<Option<usize>>,
    /// Root motion from the project's animation data, if the clip moves the actor.
    pub motion: Option<Arc<Motion>>,
    /// Played backwards (a behaviour clip generator with negative speed).
    pub reversed: bool,
    /// Offsets to add to a pose rather than a pose.
    pub additive: bool,
}

impl BoundClip {
    pub fn duration(&self) -> f32 {
        self.anim.duration.max(0.001)
    }

    /// The same clip played backwards, with its root motion seen from its end pose.
    pub fn reverse(&self) -> BoundClip {
        BoundClip {
            anim: self.anim.clone(),
            track_to_bone: self.track_to_bone.clone(),
            track_to_hk: self.track_to_hk.clone(),
            motion: self
                .motion
                .as_ref()
                .map(|m| Arc::new(m.reversed(self.duration()))),
            reversed: !self.reversed,
            additive: self.additive,
        }
    }
}

/// Root motion of a clip: translation (actor space, +Y forward, +X right) and
/// counter-clockwise yaw relative to the pose at the start of the clip.
#[derive(Debug, Default)]
pub struct Motion {
    pub translations: Vec<(f32, Vec3)>,
    pub rotations: Vec<(f32, f32)>,
}

impl Motion {
    /// Offset and yaw at time `t`, interpolated from rest at t = 0.
    pub fn sample(&self, t: f32) -> (Vec3, f32) {
        fn interp<T: Copy>(keys: &[(f32, T)], t: f32, zero: T, lerp: impl Fn(T, T, f32) -> T) -> T {
            let i = keys.partition_point(|k| k.0 <= t);
            let (t0, a) = if i == 0 { (0.0, zero) } else { keys[i - 1] };
            match keys.get(i) {
                Some(&(t1, b)) if t1 > t0 => lerp(a, b, (t - t0) / (t1 - t0)),
                _ => a,
            }
        }
        let p = interp(&self.translations, t, Vec3::ZERO, |a, b, f| a.lerp(b, f));
        let yaw = interp(&self.rotations, t, 0.0, |a, b, f| a + (b - a) * f);
        (p, yaw)
    }

    pub fn end(&self) -> (Vec3, f32) {
        self.sample(f32::MAX)
    }

    /// Movement from time `from` to `to` (`wrapped` when a loop restarted in
    /// between, over a clip of `duration`), in the actor's frame at `from`.
    pub fn delta(&self, from: f32, to: f32, wrapped: bool, duration: f32) -> (Vec3, f32) {
        let step = |a: f32, b: f32| {
            let (pa, ya) = self.sample(a);
            let (pb, yb) = self.sample(b);
            // Into the frame at `a`: undo its counter-clockwise yaw (a clockwise turn).
            let d = pb - pa;
            let (s, c) = ya.sin_cos();
            (
                Vec3::new(d.x * c + d.y * s, -d.x * s + d.y * c, d.z),
                yb - ya,
            )
        };
        if !wrapped {
            return step(from, to);
        }
        // To the end, then from the start: chain the two.
        let (t1, y1) = step(from, duration);
        let (t2, y2) = step(0.0, to);
        let (s, c) = y1.sin_cos();
        (
            t1 + Vec3::new(t2.x * c - t2.y * s, t2.x * s + t2.y * c, t2.z),
            y1 + y2,
        )
    }

    /// Motion of the clip played backwards over `duration`: relative to the pose the
    /// forward clip ends in, so it starts at rest and ends where the forward clip began.
    pub fn reversed(&self, duration: f32) -> Motion {
        let (end, end_yaw) = self.end();
        // Into the end pose's frame: undo its offset, then its counter-clockwise yaw
        // (a clockwise turn by that yaw, which is what `to_world` does).
        let (s, c) = end_yaw.sin_cos();
        let local = |p: Vec3| {
            let d = p - end;
            Vec3::new(d.x * c + d.y * s, -d.x * s + d.y * c, d.z)
        };
        // Keys at forward time t land at duration - t; forward rest (t = 0) is the last one.
        let rev = |t: f32| (duration - t).max(0.0);
        let mut translations: Vec<(f32, Vec3)> = self
            .translations
            .iter()
            .map(|&(t, p)| (rev(t), local(p)))
            .collect();
        let mut rotations: Vec<(f32, f32)> = self
            .rotations
            .iter()
            .map(|&(t, y)| (rev(t), y - end_yaw))
            .collect();
        if !self.translations.is_empty() {
            translations.push((duration, local(Vec3::ZERO)));
        }
        if !self.rotations.is_empty() {
            rotations.push((duration, -end_yaw));
        }
        translations.sort_by(|a, b| a.0.total_cmp(&b.0));
        rotations.sort_by(|a, b| a.0.total_cmp(&b.0));
        Motion {
            translations,
            rotations,
        }
    }
}

/// Parse a behaviour project's clip list (`animationdata/<project>.txt`): clip
/// generator name (lowercase, without extension) -> clip id.
fn parse_clip_ids(text: &str) -> HashMap<String, u32> {
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let mut out = HashMap::new();
    let Some(files) = lines.get(1).and_then(|l| l.parse::<usize>().ok()) else {
        return out;
    };
    // Header: 1, file count, files, has-clip-data flag. Then blank-separated entries:
    // name, id, speed, crop start, crop end, trigger count, triggers.
    let mut i = 2 + files + 1;
    while i + 5 < lines.len() {
        if lines[i].is_empty() {
            i += 1;
            continue;
        }
        let name = lines[i].to_ascii_lowercase();
        let (Ok(id), Ok(triggers)) = (lines[i + 1].parse::<u32>(), lines[i + 5].parse::<usize>())
        else {
            break;
        };
        out.insert(name.strip_suffix(".hkx").unwrap_or(&name).to_owned(), id);
        i += 6 + triggers;
    }
    out
}

/// Parse a behaviour project's clip triggers (`animationdata/<project>.txt`): clip
/// generator name (lowercase) -> (seconds, `Event` or `Event.Payload`), the
/// annotations the game raises as the clips play.
pub fn parse_clip_triggers(text: &str) -> HashMap<String, Vec<(f32, String)>> {
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let mut out: HashMap<String, Vec<(f32, String)>> = HashMap::new();
    let Some(files) = lines.get(1).and_then(|l| l.parse::<usize>().ok()) else {
        return out;
    };
    let mut i = 2 + files + 1;
    while i + 5 < lines.len() {
        if lines[i].is_empty() {
            i += 1;
            continue;
        }
        let (Ok(_), Ok(n)) = (lines[i + 1].parse::<u32>(), lines[i + 5].parse::<usize>()) else {
            break;
        };
        let triggers = lines[i + 6..(i + 6 + n).min(lines.len())]
            .iter()
            .filter_map(|l| {
                let (text, t) = l.rsplit_once(':')?;
                Some((t.parse::<f32>().ok()?, text.to_owned()))
            })
            .collect::<Vec<_>>();
        if !triggers.is_empty() {
            out.entry(lines[i].to_ascii_lowercase())
                .or_default()
                .extend(triggers);
        }
        i += 6 + n;
    }
    out
}

/// Parse root motion (`animationdata/boundanims/anims_<project>.txt`): id, duration,
/// translation keys "t x y z", rotation keys "t qx qy qz qw".
fn parse_motions(text: &str) -> HashMap<u32, Motion> {
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
    let mut out = HashMap::new();
    let nums = |l: &str| {
        l.split_whitespace()
            .filter_map(|x| x.parse::<f32>().ok())
            .collect::<Vec<_>>()
    };
    while let Some(id) = lines.next().and_then(|l| l.parse::<u32>().ok()) {
        let _duration = lines.next();
        let mut m = Motion::default();
        let Some(n) = lines.next().and_then(|l| l.parse::<usize>().ok()) else {
            break;
        };
        for l in lines.by_ref().take(n) {
            if let [t, x, y, z] = nums(l)[..] {
                m.translations.push((t, Vec3::new(x, y, z)));
            }
        }
        let Some(n) = lines.next().and_then(|l| l.parse::<usize>().ok()) else {
            break;
        };
        let mut last = 0.0f32;
        for l in lines.by_ref().take(n) {
            if let [t, _, _, z, w] = nums(l)[..] {
                // Unwrap so interpolation never takes the long way round.
                let mut yaw = 2.0 * z.atan2(w);
                while yaw - last > std::f32::consts::PI {
                    yaw -= std::f32::consts::TAU;
                }
                while yaw - last < -std::f32::consts::PI {
                    yaw += std::f32::consts::TAU;
                }
                last = yaw;
                m.rotations.push((t, yaw));
            }
        }
        out.insert(id, m);
    }
    out
}

/// Runtime playback state for one actor.
pub struct ActorAnim {
    pub clip: Arc<BoundClip>,
    pub time: f32,
    pub speed: f32,
    /// Clip being faded out, its time, and fade progress (0..1) with duration.
    fading: Option<(Arc<BoundClip>, f32)>,
    fade: f32,
    fade_len: f32,
    scratch: Vec<havok::QsTransform>,
    locals: Vec<Transform>,
    from: Vec<Transform>,
}

impl ActorAnim {
    pub fn new(clip: Arc<BoundClip>, skeleton: &Skeleton, start: f32) -> Self {
        let duration = clip.anim.duration.max(0.001);
        ActorAnim {
            clip,
            time: start % duration,
            speed: 1.0,
            fading: None,
            fade: 1.0,
            fade_len: 0.0,
            scratch: Vec::new(),
            locals: skeleton.bind_locals(),
            from: Vec::new(),
        }
    }

    /// Switch to `clip`, cross-fading from the current one over `fade` seconds.
    pub fn play(&mut self, clip: Arc<BoundClip>, fade: f32) {
        if Arc::ptr_eq(&clip, &self.clip) {
            return;
        }
        let old = std::mem::replace(&mut self.clip, clip);
        self.fading = Some((old, self.time));
        self.time = 0.0;
        self.fade = 0.0;
        self.fade_len = fade.max(1e-3);
    }

    fn apply(
        clip: &BoundClip,
        t: f32,
        scratch: &mut Vec<havok::QsTransform>,
        locals: &mut [Transform],
    ) {
        let t = if clip.reversed {
            clip.duration() - t
        } else {
            t
        };
        clip.anim.sample(t, scratch);
        for (track, q) in scratch.iter().enumerate() {
            if let Some(Some(b)) = clip.track_to_bone.get(track) {
                locals[*b] = Transform {
                    translation: q.translation,
                    rotation: glam::Mat3::from_quat(if q.rotation.is_finite() {
                        q.rotation
                    } else {
                        Quat::IDENTITY
                    }),
                    scale: q.scale.x,
                };
            }
        }
    }

    /// Advance and return the new model-space pose.
    pub fn update(&mut self, skeleton: &Skeleton, dt: f32) -> Vec<glam::Mat4> {
        let d = self.clip.duration();
        self.time += dt * self.speed;
        self.time %= d;
        if let Some((old, t)) = &mut self.fading {
            *t = (*t + dt) % old.duration();
            Self::apply(old, *t, &mut self.scratch, &mut self.locals);
            self.from.clone_from(&self.locals);
        }
        Self::apply(&self.clip, self.time, &mut self.scratch, &mut self.locals);
        if self.fading.is_some() {
            self.fade += dt / self.fade_len;
            if self.fade >= 1.0 {
                self.fading = None;
            } else {
                let w = self.fade * self.fade * (3.0 - 2.0 * self.fade);
                for (l, f) in self.locals.iter_mut().zip(&self.from) {
                    let q = Quat::from_mat3(&f.rotation).slerp(Quat::from_mat3(&l.rotation), w);
                    *l = Transform {
                        translation: f.translation.lerp(l.translation, w),
                        rotation: glam::Mat3::from_quat(q),
                        scale: f.scale + (l.scale - f.scale) * w,
                    };
                }
            }
        }
        skeleton.model_space(&self.locals)
    }
}

/// Cached event -> clip sequence lookups in behaviour projects.
#[derive(Default)]
pub struct BehaviorLibrary {
    events: HashMap<(String, String, String), Option<Arc<EventClips>>>,
}

/// Clips a behaviour event plays, and the clips that leave that state again.
pub struct EventClips {
    pub clips: Vec<havok::behavior::PlayedClip>,
    pub exit: Vec<havok::behavior::PlayedClip>,
    /// The loop plays once: its own trigger leads on to `exit`.
    pub loop_once: bool,
}

impl BehaviorLibrary {
    /// Clips an animation event plays on actors of `project`, ending in a loop
    /// where there is one, and the clips played from there by the first of `exits`
    /// the graph handles.
    pub fn event_clips(
        &mut self,
        project: &ProjectRuntime,
        event: &str,
        exits: &[&str],
    ) -> Option<Arc<EventClips>> {
        use havok::behavior::ClipMode;
        let key = (
            project.name.clone(),
            event.to_ascii_lowercase(),
            exits.join(",").to_ascii_lowercase(),
        );
        if let Some(r) = self.events.get(&key) {
            return r.clone();
        }
        let dir = &project.dir;
        let project = &project.shared.project;
        let plays = project.play_event(event);
        // Prefer a sequence that settles into a loop.
        let play = plays
            .iter()
            .find(|p| p.clips.last().is_some_and(|c| c.mode == ClipMode::Looping))
            .or(plays.first());
        let r = match play {
            Some(p) => {
                let mut exit = exits
                    .iter()
                    .find_map(|e| project.then_event(p, e))
                    .map(|x| {
                        // A looping exit clip that leaves the furniture by its own
                        // trigger (woodchopping's stop) plays once.
                        let mut clips = x.clips;
                        for e in x
                            .events
                            .iter()
                            .filter(|e| e.event.eq_ignore_ascii_case("IdleFurnitureExit"))
                        {
                            if let Some(c) = clips.get_mut(e.clip) {
                                c.mode = ClipMode::SinglePlay;
                            }
                        }
                        clips
                    })
                    .unwrap_or_default();
                // Otherwise the loop may move on by itself (a sitting variant's
                // `NextClip` at the end of its loop, into the way back); a trigger
                // that only picks another loop (eating variants) isn't a way out.
                let last = p.clips.len().saturating_sub(1);
                let mut loop_once = false;
                if exit.is_empty() && p.clips.last().is_some_and(|c| c.mode == ClipMode::Looping) {
                    let own = p
                        .events
                        .iter()
                        .filter(|e| e.clip == last)
                        .filter_map(|e| project.then_event(p, &e.event));
                    if let Some(x) = own
                        .map(|x| x.clips)
                        .find(|c| c.first().is_some_and(|c| c.mode == ClipMode::SinglePlay))
                    {
                        exit = x;
                        loop_once = true;
                    }
                }
                Some(EventClips {
                    clips: p.clips.clone(),
                    exit,
                    loop_once,
                })
            }
            // No graph handles it (some IDLE records name events nothing listens to).
            None => None,
        };
        if r.is_none() {
            log::debug!("{dir}: no clips for animation event {event:?}");
        }
        let r = r.map(Arc::new);
        self.events.insert(key, r.clone());
        r
    }
}

/// Candidate files for a behaviour clip path (`Animations\male\MT_Idle.HKX`) in the
/// project at `dir`, female variants first for female actors.
pub fn project_clip_paths(dir: &str, animation: &str, female: bool) -> Vec<String> {
    let rel = animation.to_ascii_lowercase().replace('\\', "/");
    let path = format!("{dir}/{rel}");
    let mut out = Vec::new();
    if female && path.contains("/animations/male/") {
        out.push(path.replace("/animations/male/", "/animations/female/"));
    }
    out.push(path);
    out
}

/// Loads and caches clips per (clip path, skeleton path).
#[derive(Default)]
pub struct AnimationLibrary {
    havok_skeletons: HashMap<String, Option<Arc<havok::Skeleton>>>,
    clips: HashMap<(String, String), Option<Arc<BoundClip>>>,
    /// Root motion per behaviour project, by clip name.
    motions: HashMap<String, Arc<HashMap<String, Arc<Motion>>>>,
}

impl AnimationLibrary {
    fn havok_skeleton(&mut self, vfs: &vfs::Vfs, path: &str) -> Option<Arc<havok::Skeleton>> {
        if let Some(s) = self.havok_skeletons.get(path) {
            return s.clone();
        }
        let s = vfs
            .read(path)
            .and_then(|b| {
                havok::AnimationContainer::parse(&b)
                    .map_err(|e| log::warn!("{path}: {e}"))
                    .ok()
            })
            .and_then(|c| c.skeletons.into_iter().next())
            .map(Arc::new);
        self.havok_skeletons.insert(path.to_owned(), s.clone());
        s
    }

    /// NIF bone for each bone of the Havok skeleton next to `nif_skeleton_path`.
    pub fn havok_bone_map(
        &mut self,
        vfs: &vfs::Vfs,
        nif_skeleton_path: &str,
        skeleton: &Skeleton,
    ) -> Vec<Option<usize>> {
        let hk = nif_skeleton_path
            .strip_suffix(".nif")
            .and_then(|s| self.havok_skeleton(vfs, &format!("{s}.hkx")));
        hk.map(|hk| hk.bones.iter().map(|b| skeleton.find(&b.name)).collect())
            .unwrap_or_default()
    }

    /// Root motion of a behaviour project's clips (`animationdata/boundanims/anims_<project>.txt`),
    /// by clip generator name (lowercase, without extension).
    pub fn project_motions(
        &mut self,
        vfs: &vfs::Vfs,
        project: &str,
    ) -> Arc<HashMap<String, Arc<Motion>>> {
        if let Some(m) = self.motions.get(project) {
            return m.clone();
        }
        let read = |p: String| {
            vfs.read(&p)
                .map(|b| String::from_utf8_lossy(&b).into_owned())
        };
        let ids = read(format!("meshes/animationdata/{project}.txt"))
            .map(|t| parse_clip_ids(&t))
            .unwrap_or_default();
        let motions: HashMap<u32, Arc<Motion>> = read(format!(
            "meshes/animationdata/boundanims/anims_{project}.txt"
        ))
        .map(|t| parse_motions(&t))
        .unwrap_or_default()
        .into_iter()
        .map(|(id, m)| (id, Arc::new(m)))
        .collect();
        // Several clip generators can share one clip id (e.g. Chair_FrontExit / Chair_FrontQuickExit).
        let by_name: HashMap<String, Arc<Motion>> = ids
            .into_iter()
            .filter_map(|(name, id)| Some((name, motions.get(&id)?.clone())))
            .collect();
        log::debug!("{project}: root motion for {} clips", by_name.len());
        let m = Arc::new(by_name);
        self.motions.insert(project.to_owned(), m.clone());
        m
    }

    /// Load `clip` for an actor whose NIF skeleton lives at `nif_skeleton_path`.
    pub fn clip(
        &mut self,
        vfs: &vfs::Vfs,
        clip: &str,
        nif_skeleton_path: &str,
        skeleton: &Skeleton,
    ) -> Option<Arc<BoundClip>> {
        // Humanoid clips take root motion from the matching default project.
        let project = clip.contains("actors/character/").then(|| {
            if clip.contains("/female/") {
                "defaultfemale"
            } else {
                "defaultmale"
            }
        });
        self.clip_in_project(vfs, clip, nif_skeleton_path, skeleton, project)
    }

    /// Load `clip` with root motion from the animation data of the behaviour project
    /// named `project` (`dogproject`), if any.
    pub fn clip_in_project(
        &mut self,
        vfs: &vfs::Vfs,
        clip: &str,
        nif_skeleton_path: &str,
        skeleton: &Skeleton,
        project: Option<&str>,
    ) -> Option<Arc<BoundClip>> {
        let key = (
            format!("{clip}@{}", project.unwrap_or_default()),
            nif_skeleton_path.to_owned(),
        );
        if let Some(c) = self.clips.get(&key) {
            return c.clone();
        }
        let hkx_skel = nif_skeleton_path
            .strip_suffix(".nif")
            .map(|s| format!("{s}.hkx"));
        let hk = hkx_skel.and_then(|p| self.havok_skeleton(vfs, &p));
        let result = (|| {
            let bytes = vfs.read(clip)?;
            let container = havok::AnimationContainer::parse(&bytes)
                .map_err(|e| log::warn!("{clip}: {e}"))
                .ok()?;
            let anim = container.animations.into_iter().next()?;
            let hk = hk.as_ref()?;
            let track_to_hk: Vec<Option<usize>> = (0..anim.num_tracks)
                .map(|t| match &anim.binding {
                    Some(b) if !b.track_to_bone.is_empty() => {
                        b.track_to_bone.get(t).map(|&x| x as usize)
                    }
                    _ => Some(t),
                })
                .collect();
            let track_to_bone = track_to_hk
                .iter()
                .map(|b| skeleton.find(&hk.bones.get((*b)?)?.name))
                .collect();
            Some((anim, track_to_bone, track_to_hk))
        })()
        .map(|(anim, track_to_bone, track_to_hk)| {
            let motion = project.map(|project| {
                let stem = clip
                    .rsplit('/')
                    .next()
                    .unwrap_or(clip)
                    .trim_end_matches(".hkx");
                let m = self.project_motions(vfs, project).get(stem).cloned();
                log::trace!("{clip}: {project} motion for {stem:?}: {}", m.is_some());
                m
            });
            let additive = anim.binding.as_ref().is_some_and(|b| b.additive);
            Arc::new(BoundClip {
                anim: Arc::new(anim),
                track_to_bone,
                track_to_hk,
                motion: motion.flatten(),
                reversed: false,
                additive,
            })
        });
        if result.is_none() {
            log::debug!("could not load clip {clip} for {nif_skeleton_path}");
        }
        self.clips.insert(key, result.clone());
        result
    }

    /// A clip of `project`'s graphs (root motion from its animation data), played
    /// backwards when `speed` is negative.
    pub fn project_clip(
        &mut self,
        vfs: &vfs::Vfs,
        clip: &str,
        nif_skeleton_path: &str,
        skeleton: &Skeleton,
        speed: f32,
        project: &ProjectRuntime,
    ) -> Option<Arc<BoundClip>> {
        let forward = |lib: &mut Self| {
            if project.humanoid() {
                lib.clip(vfs, clip, nif_skeleton_path, skeleton)
            } else {
                lib.clip_in_project(vfs, clip, nif_skeleton_path, skeleton, Some(&project.name))
            }
        };
        if speed >= 0.0 {
            return forward(self);
        }
        let key = (
            format!("{clip}@{}#reversed", project.name),
            nif_skeleton_path.to_owned(),
        );
        if let Some(c) = self.clips.get(&key) {
            return c.clone();
        }
        let r = forward(self).map(|c| Arc::new(c.reverse()));
        self.clips.insert(key, r.clone());
        r
    }
}

/// Forward walk (or run) clip candidates for an actor given its skeleton path.
pub fn locomotion_clip(skeleton_path: &str, female: bool, run: bool) -> Vec<String> {
    let base = skeleton_path
        .split("/character assets")
        .next()
        .unwrap_or("")
        .to_owned();
    let gait = if run { "run" } else { "walk" };
    if base.ends_with("actors/character") {
        let g = if female { "female" } else { "male" };
        vec![
            format!("{base}/animations/{g}/mt_{gait}forward.hkx"),
            format!("{base}/animations/male/mt_{gait}forward.hkx"),
        ]
    } else {
        vec![
            format!("{base}/animations/{gait}forward.hkx"),
            format!("{base}/animations/mt_{gait}forward.hkx"),
            format!("{base}/animations/{gait}.hkx"),
        ]
    }
}

/// Default idle clip for an actor given its skeleton path.
pub fn idle_clip(skeleton_path: &str, female: bool) -> Vec<String> {
    let base = skeleton_path
        .split("/character assets")
        .next()
        .unwrap_or("")
        .to_owned();
    if base.ends_with("actors/character") {
        let g = if female { "female" } else { "male" };
        vec![
            format!("{base}/animations/{g}/mt_idle.hkx"),
            format!("{base}/animations/mt_idle_a_base.hkx"),
        ]
    } else {
        vec![
            format!("{base}/animations/mt_idle.hkx"),
            format!("{base}/animations/idle.hkx"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROJECT: &str = "1\n2\nBehaviors\\A.hkx\nCharacters\\B.hkx\n1\n\
        Chair_FrontExit\n470\n1\n0\n0\n2\nSoundPlay:0.03\nIdleFurnitureExit:1.4\n\n\
        Chair_FrontQuickExit\n470\n1\n0\n0\n0\n\n\
        Wall_IdleBackExit.HKX\n1311\n1\n0\n0\n0\n";

    #[test]
    fn clip_ids_share_ids_and_drop_extensions() {
        let ids = parse_clip_ids(PROJECT);
        assert_eq!(ids.get("chair_frontexit"), Some(&470));
        assert_eq!(ids.get("chair_frontquickexit"), Some(&470));
        assert_eq!(ids.get("wall_idlebackexit"), Some(&1311));
        assert_eq!(ids.len(), 3);
    }

    #[test]
    fn motion_parses_and_unwraps_yaw() {
        // Half a turn clockwise, crossing the +-180 degree boundary between keys.
        let text = "469\n2\n1\n2 0 63.5 0\n2\n1 0 0 -0.7071 0.7071\n2 0 0 -0.9999 -0.0141\n\n470\n1\n0\n0\n";
        let m = parse_motions(text);
        let (t, yaw) = m[&469].end();
        assert!((t - Vec3::new(0.0, 63.5, 0.0)).length() < 1e-4);
        assert!((yaw + std::f32::consts::PI).abs() < 0.05, "{yaw}");
        let (half, _) = m[&469].sample(1.0);
        assert!((half.y - 31.75).abs() < 1e-3);
        assert!(m[&470].translations.is_empty());
    }

    #[test]
    fn motion_delta_across_a_loop() {
        // One unit forward per second, turning a quarter counter-clockwise over 2 s.
        let m = Motion {
            translations: vec![(2.0, Vec3::new(0.0, 2.0, 0.0))],
            rotations: vec![(2.0, std::f32::consts::FRAC_PI_2)],
        };
        let (t, yaw) = m.delta(0.5, 1.5, false, 2.0);
        assert!((yaw - std::f32::consts::FRAC_PI_4).abs() < 1e-5);
        assert!((t.length() - 1.0).abs() < 1e-5);
        // 1.5 -> end -> 0.5 turns as far as 1.5 -> 2.5 would.
        let (_, yaw) = m.delta(1.5, 0.5, true, 2.0);
        assert!((yaw - std::f32::consts::FRAC_PI_4).abs() < 1e-5);
        // A straight walk carries on straight across the wrap.
        let walk = Motion {
            translations: vec![(2.0, Vec3::new(0.0, 2.0, 0.0))],
            rotations: vec![],
        };
        let (t, _) = walk.delta(1.5, 0.5, true, 2.0);
        assert!((t - Vec3::new(0.0, 1.0, 0.0)).length() < 1e-5, "{t}");
        // Turning in place by a quarter every loop: half a loop after a wrap faces on.
        let turn = Motion {
            translations: vec![],
            rotations: vec![(2.0, std::f32::consts::FRAC_PI_2)],
        };
        assert!((turn.delta(1.0, 1.0, true, 2.0).1 - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
    }

    #[test]
    fn reversed_motion_returns_to_the_start() {
        // Two steps forward, turning a quarter counter-clockwise on the way.
        let m = Motion {
            translations: vec![
                (1.0, Vec3::new(0.0, 5.0, 0.0)),
                (2.0, Vec3::new(0.0, 10.0, 0.0)),
            ],
            rotations: vec![(2.0, std::f32::consts::FRAC_PI_2)],
        };
        let r = m.reversed(2.0);
        assert_eq!(r.sample(0.0), (Vec3::ZERO, 0.0));
        let (p, yaw) = r.end();
        // Facing left at the end, the start lies 10 units to the actor's left.
        assert!((p - Vec3::new(-10.0, 0.0, 0.0)).length() < 1e-4, "{p}");
        assert!((yaw + std::f32::consts::FRAC_PI_2).abs() < 1e-5);
        // Half way back is the forward clip's half way point.
        assert!((r.sample(1.0).0 - Vec3::new(-5.0, 0.0, 0.0)).length() < 1e-4);
    }
}
