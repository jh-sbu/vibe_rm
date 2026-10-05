//! Actors animated by their behaviour graphs: a [`havok::behavior::runtime::Instance`]
//! per actor, its clip samples blended into a pose, root motion taken from them.

use std::collections::HashMap;
use std::sync::Arc;

use glam::{Mat4, Quat, Vec3};
use havok::behavior::runtime::{Instance, Raised, Shared};
use nif::Transform;

use super::animation::{AnimationLibrary, BoundClip, Motion, project_clip_paths};
use super::skeleton::Skeleton;

/// Behaviour projects' shared runtime tables, by project file.
#[derive(Default)]
pub struct GraphLibrary {
    projects: HashMap<String, Option<Arc<ProjectRuntime>>>,
    /// By (directory, root graph): projects naming the same graphs share them
    /// (`DefaultMale.hkx` and `DefaultFemale.hkx`).
    graphs: HashMap<(String, String), Arc<Shared>>,
}

/// A loaded behaviour project.
pub struct ProjectRuntime {
    pub shared: Arc<Shared>,
    /// Directory its paths are relative to (`meshes/actors/canine`).
    pub dir: String,
    /// Name of its animation data (`dogproject`: `animationdata/dogproject.txt`).
    pub name: String,
    /// Its graph files as IDLE records name them (`actors\\canine\\behaviors\\dogbehavior.hkx`).
    pub files: Vec<String>,
}

impl ProjectRuntime {
    /// The humanoid project (`meshes/actors/character`): clips under
    /// `animations/male` and `animations/female`.
    pub fn humanoid(&self) -> bool {
        self.dir.ends_with("actors/character")
    }

    /// Whether IDLE records for graph `file` (as their DNAM names it) play on this
    /// project's actors.
    pub fn plays(&self, file: &str) -> bool {
        let file = file.to_ascii_lowercase().replace('/', "\\");
        self.files.iter().any(|f| *f == file)
    }
}

impl GraphLibrary {
    /// The behaviour project in `file` (`meshes/actors/canine/dogproject.hkx`).
    pub fn project(&mut self, vfs: &vfs::Vfs, file: &str) -> Option<Arc<ProjectRuntime>> {
        let file = file.to_ascii_lowercase().replace('\\', "/");
        if let Some(p) = self.projects.get(&file) {
            return p.clone();
        }
        let t = std::time::Instant::now();
        let p = (|| {
            let (dir, name) = file.rsplit_once('/')?;
            let read = |rel: &str| vfs.read(&format!("{dir}/{rel}"));
            let bytes = read(name)?;
            let character_file = havok::behavior::project_character_file(&bytes).map_err(|e| log::warn!("{file}: {e}")).ok()?;
            let character =
                havok::behavior::Character::parse(&read(&character_file.to_ascii_lowercase().replace('\\', "/"))?).map_err(|e| log::warn!("{file}: {e}")).ok()?;
            let key = (dir.to_owned(), character.behavior.to_ascii_lowercase());
            let shared = match self.graphs.get(&key) {
                Some(s) => s.clone(),
                None => {
                    let mut p = havok::behavior::Project::load(&character.behavior, read);
                    if p.graphs.is_empty() {
                        log::warn!("{file}: no behaviour graphs");
                        return None;
                    }
                    p.character = Some(character);
                    let s = Shared::new(Arc::new(p));
                    self.graphs.insert(key, s.clone());
                    s
                }
            };
            let name = name.strip_suffix(".hkx").unwrap_or(name).to_owned();
            let base = dir.strip_prefix("meshes/").unwrap_or(dir).replace('/', "\\");
            let files = shared.project.graphs.iter().map(|(rel, _)| format!("{base}\\{rel}")).collect();
            Some(Arc::new(ProjectRuntime { shared, dir: dir.to_owned(), name, files }))
        })();
        log::debug!("{file}: behaviour runtime in {:?}", t.elapsed());
        self.projects.insert(file, p.clone());
        p
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
    project: Arc<ProjectRuntime>,
    skeleton_path: String,
    female: bool,
    clips: HashMap<Arc<str>, Option<Arc<BoundClip>>>,
    /// Root motion per (graph, clip generator), from the project's animation data.
    motions: HashMap<(usize, usize), Option<Arc<Motion>>>,
    bind: Vec<Transform>,
    scratch: Vec<havok::QsTransform>,
}

/// Clip lookup for one actor while the graph runs.
struct Source<'a> {
    clips: &'a mut HashMap<Arc<str>, Option<Arc<BoundClip>>>,
    vfs: &'a vfs::Vfs,
    anims: &'a mut AnimationLibrary,
    skeleton: &'a Skeleton,
    project: &'a ProjectRuntime,
    skeleton_path: &'a str,
    female: bool,
}

impl Source<'_> {
    fn clip(&mut self, animation: &str) -> Option<Arc<BoundClip>> {
        if let Some(c) = self.clips.get(animation) {
            return c.clone();
        }
        let c = project_clip_paths(&self.project.dir, animation, self.female).iter().find_map(|p| {
            if self.project.humanoid() {
                self.anims.clip(self.vfs, p, self.skeleton_path, self.skeleton)
            } else {
                self.anims.clip_in_project(self.vfs, p, self.skeleton_path, self.skeleton, Some(&self.project.name))
            }
        });
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
    pub fn new(project: Arc<ProjectRuntime>, skeleton_path: &str, female: bool, skeleton: &Skeleton, seed: u64) -> GraphAnim {
        GraphAnim {
            label: String::new(),
            inst: Instance::new(project.shared.clone(), seed),
            project,
            skeleton_path: skeleton_path.to_owned(),
            female,
            clips: HashMap::new(),
            motions: HashMap::new(),
            bind: skeleton.bind_locals(),
            scratch: Vec::new(),
        }
    }

    fn source<'a>(&'a mut self, vfs: &'a vfs::Vfs, anims: &'a mut AnimationLibrary, skeleton: &'a Skeleton) -> (&'a mut Instance, Source<'a>) {
        let GraphAnim { inst, project, skeleton_path, female, clips, .. } = self;
        let project: &'a Arc<ProjectRuntime> = project;
        (inst, Source { clips, vfs, anims, skeleton, project, skeleton_path, female: *female })
    }

    /// Root motion of the clip a sample plays: the project's animation data names
    /// clips by their generator (`Forward_Walk` plays `WalkForward.hkx`).
    fn motion(&mut self, graph: usize, generator: usize, anims: &mut AnimationLibrary, vfs: &vfs::Vfs) -> Option<Arc<Motion>> {
        if let Some(m) = self.motions.get(&(graph, generator)) {
            return m.clone();
        }
        let name = self.project.shared.project.graphs.get(graph).and_then(|(_, g)| g.generators.get(generator)).map(|g| g.name().to_ascii_lowercase());
        let m = name.and_then(|n| anims.project_motions(vfs, &self.project.name).get(n.strip_suffix(".hkx").unwrap_or(&n)).cloned());
        self.motions.insert((graph, generator), m.clone());
        m
    }

    /// Ground speed of the graph's slowest forward locomotion: a scratch instance of
    /// the graph is set walking and its clips' root motion measured.
    pub fn walk_speed(&mut self, vfs: &vfs::Vfs, anims: &mut AnimationLibrary, skeleton: &Skeleton) -> Option<f32> {
        let mut probe = Instance::new(self.project.shared.clone(), 0);
        let samples = {
            let (_, mut src) = self.source(vfs, anims, skeleton);
            probe.set_variable("Speed", 1.0);
            probe.handle_event("moveStart", &mut src);
            probe.handle_event("SprintStop", &mut src);
            for _ in 0..30 {
                probe.update(1.0 / 30.0, &mut src);
            }
            probe.samples()
        };
        let (mut speed, mut weight) = (0.0, 0.0);
        for s in samples.iter().filter(|s| !s.additive && s.mask.is_none()) {
            let Some(m) = self.motion(s.graph, s.generator, anims, vfs) else { continue };
            let Some(d) = self.clips.get(&s.animation).cloned().flatten().map(|c| c.duration()) else { continue };
            speed += m.end().0.truncate().length() / d * s.weight;
            weight += s.weight;
        }
        (weight > 0.5).then(|| speed / weight).filter(|v| (10.0..400.0).contains(v))
    }

    pub fn project(&self) -> &Arc<ProjectRuntime> {
        &self.project
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
            let m = self.motion(s.graph, s.generator, anims, vfs).or_else(|| clip.motion.clone());
            let (d, yaw) = m.map_or((Vec3::ZERO, 0.0), |m| m.delta(s.prev_time, s.time, s.wrapped, clip.duration()));
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
