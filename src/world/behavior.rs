//! Actors animated by their behaviour graphs: a [`havok::behavior::runtime::Instance`]
//! per actor, its clip samples blended into a pose, root motion taken from them.

use std::collections::HashMap;
use std::sync::Arc;

use glam::{Mat4, Quat, Vec3};
use havok::behavior::runtime::{Instance, Raised, Shared};
use nif::Transform;

use super::animation::{AnimationLibrary, BoundClip, Motion, project_clip_paths};
use super::footik::{self, FootIkState};
use super::lookat::{self, LookAtState};
use super::skeleton::Skeleton;
use super::twist;

/// Behaviour projects' shared runtime tables, by project file.
#[derive(Default)]
pub struct GraphLibrary {
    projects: HashMap<String, Option<Arc<ProjectRuntime>>>,
    /// By (directory, root graph, character file): character properties and
    /// driver settings stay with their character, even when graphs are shared.
    graphs: HashMap<(String, String, String), Arc<Shared>>,
}

/// Add animation data triggers (clip generator name -> (seconds, `Event[.Payload]`))
/// to the project's clip generators. The data repeats the clips' own triggers (those
/// timed from the end as absolute times), so an event the clip already raises with
/// the same payload is left out. Returns how many were added.
fn add_clip_triggers(
    p: &mut havok::behavior::Project,
    triggers: &HashMap<String, Vec<(f32, String)>>,
) -> usize {
    use havok::behavior::{Generator, Trigger};
    let mut added = 0;
    for (_, g) in &mut p.graphs {
        for gi in 0..g.generators.len() {
            let Generator::Clip {
                name,
                triggers: own,
                ..
            } = &g.generators[gi]
            else {
                continue;
            };
            let Some(list) = triggers.get(&name.to_ascii_lowercase()) else {
                continue;
            };
            let own: Vec<(i32, Option<String>)> =
                own.iter().map(|t| (t.event, t.payload.clone())).collect();
            let mut extra = Vec::new();
            for (time, text) in list {
                let (event, payload) = match text.split_once('.') {
                    Some((e, p)) => (e, Some(p.to_owned())),
                    None => (text.as_str(), None),
                };
                if event.is_empty() {
                    continue;
                }
                let id = match g.events.iter().position(|e| e.eq_ignore_ascii_case(event)) {
                    Some(i) => i as i32,
                    None => {
                        g.events.push(event.to_owned());
                        g.events.len() as i32 - 1
                    }
                };
                if own.iter().any(|(e, p)| {
                    *e == id
                        && p.as_deref().map(str::to_ascii_lowercase)
                            == payload.as_deref().map(str::to_ascii_lowercase)
                }) {
                    continue;
                }
                extra.push(Trigger {
                    time: *time,
                    from_end: false,
                    event: id,
                    payload,
                });
            }
            added += extra.len();
            if let Generator::Clip { triggers: have, .. } = &mut g.generators[gi] {
                have.extend(extra);
            }
        }
    }
    added
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
            let character_file = havok::behavior::project_character_file(&bytes)
                .map_err(|e| log::warn!("{file}: {e}"))
                .ok()?;
            let character = havok::behavior::Character::parse(&read(
                &character_file.to_ascii_lowercase().replace('\\', "/"),
            )?)
            .map_err(|e| log::warn!("{file}: {e}"))
            .ok()?;
            let key = (
                dir.to_owned(),
                character.behavior.to_ascii_lowercase(),
                character_file.to_ascii_lowercase().replace('\\', "/"),
            );
            let shared = match self.graphs.get(&key) {
                Some(s) => s.clone(),
                None => {
                    let mut p = havok::behavior::Project::load(&character.behavior, read);
                    if p.graphs.is_empty() {
                        log::warn!("{file}: no behaviour graphs");
                        return None;
                    }
                    // The annotations the project's animation data lists for its clips
                    // play as clip triggers (weapon draw, hit frames, footsteps, sounds).
                    let stem = name.strip_suffix(".hkx").unwrap_or(name);
                    if let Some(text) = vfs.read(&format!("meshes/animationdata/{stem}.txt")) {
                        let added = add_clip_triggers(
                            &mut p,
                            &crate::world::animation::parse_clip_triggers(
                                &String::from_utf8_lossy(&text),
                            ),
                        );
                        log::debug!("{file}: {added} clip triggers from animation data");
                    }
                    p.character = Some(character);
                    let s = Shared::new(Arc::new(p));
                    self.graphs.insert(key, s.clone());
                    s
                }
            };
            let name = name.strip_suffix(".hkx").unwrap_or(name).to_owned();
            let base = dir
                .strip_prefix("meshes/")
                .unwrap_or(dir)
                .replace('/', "\\");
            let files = shared
                .project
                .graphs
                .iter()
                .map(|(rel, _)| format!("{base}\\{rel}"))
                .collect();
            Some(Arc::new(ProjectRuntime {
                shared,
                dir: dir.to_owned(),
                name,
                files,
            }))
        })();
        log::debug!("{file}: behaviour runtime in {:?}", t.elapsed());
        self.projects.insert(file, p.clone());
        p
    }
}

/// What a graph update produced.
pub struct Frame {
    pub pose: Vec<Mat4>,
    /// The bones' local transforms the pose is made of.
    pub locals: Vec<Transform>,
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
    /// Where to look (model space), for the graph's look-at modifier.
    pub look_target: Option<Vec3>,
    look: LookAtState,
    /// NIF bone of each Havok skeleton bone (look-at bone indices are Havok's).
    hk_bones: Option<Vec<Option<usize>>>,
    /// Foot placement: the ground under each leg's ankle (model space height and
    /// normal, from the AI's rays), and the ankles to cast from next.
    pub ground: Vec<Option<(f32, Vec3)>>,
    pub ankles: Vec<Vec3>,
    feet: FootIkState,
    bind: Vec<Transform>,
    scratch: Vec<havok::QsTransform>,
    /// Variables a script set (`SetAnimationVariable*`): the engine's own writes
    /// of what it had before don't undo them; a change of its own does.
    pinned: ScriptPins,
    /// A second graph posing some of the same skeleton's bones (a tail's).
    aux: Option<Box<AuxGraph>>,
}

/// A graph running beside an actor's own, posing bones of the actor's skeleton
/// that its graph leaves alone (`BSBehaviorGraphExtraData` "controls base
/// skeleton": Argonians' and Khajiit tails).
struct AuxGraph {
    anim: GraphAnim,
    /// The actor graph's variables it shares by name, copied over every update.
    shared: Vec<String>,
    /// The skeleton's bones it poses, once found.
    bones: Option<Vec<usize>>,
}

/// Bones `bones` of `from` replace the same ones of `locals`.
fn take_bones(locals: &mut [Transform], from: &[Transform], bones: &[usize]) {
    for &b in bones {
        if let (Some(l), Some(t)) = (locals.get_mut(b), from.get(b)) {
            *l = *t;
        }
    }
}

/// Graph variables scripts set, matched ignoring case, each with the value the
/// engine had given it before.
#[derive(Default)]
struct ScriptPins(Vec<(String, f32)>);

impl ScriptPins {
    /// A script set `name` over the engine's `before`. A script setting it again
    /// keeps the engine's value from the first time.
    fn pin(&mut self, name: &str, before: f32) {
        if self.find(name).is_none() {
            self.0.push((name.to_ascii_lowercase(), before));
        }
    }

    fn find(&self, name: &str) -> Option<usize> {
        self.0
            .iter()
            .position(|(p, _)| p.eq_ignore_ascii_case(name))
    }

    /// Whether the engine writing `value` to `name` should give way to the
    /// script: it is what the engine had already. Anything else is a change of
    /// the engine's own (an item equipped, a new state) and ends the pin.
    fn holds(&mut self, name: &str, value: f32) -> bool {
        let Some(i) = self.find(name) else {
            return false;
        };
        if self.0[i].1 == value {
            return true;
        }
        self.0.swap_remove(i);
        false
    }
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
        let c = project_clip_paths(&self.project.dir, animation, self.female)
            .iter()
            .find_map(|p| {
                if self.project.humanoid() {
                    self.anims
                        .clip(self.vfs, p, self.skeleton_path, self.skeleton)
                } else {
                    self.anims.clip_in_project(
                        self.vfs,
                        p,
                        self.skeleton_path,
                        self.skeleton,
                        Some(&self.project.name),
                    )
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
    pub fn new(
        project: Arc<ProjectRuntime>,
        skeleton_path: &str,
        female: bool,
        skeleton: &Skeleton,
        seed: u64,
    ) -> GraphAnim {
        GraphAnim {
            label: String::new(),
            inst: Instance::new(project.shared.clone(), seed),
            project,
            skeleton_path: skeleton_path.to_owned(),
            female,
            clips: HashMap::new(),
            motions: HashMap::new(),
            look_target: None,
            look: LookAtState::default(),
            hk_bones: None,
            ground: Vec::new(),
            ankles: Vec::new(),
            feet: FootIkState::default(),
            bind: skeleton.bind_locals(),
            scratch: Vec::new(),
            pinned: ScriptPins::default(),
            aux: None,
        }
    }

    /// Run `aux` beside this graph: it hears this graph's events and shares its
    /// variables, and its poses of the skeleton's bones replace this graph's.
    pub fn attach_aux(&mut self, aux: GraphAnim) {
        let shared = aux
            .project
            .shared
            .variables()
            .map(|(name, _)| name.to_owned())
            .collect();
        self.aux = Some(Box::new(AuxGraph {
            anim: aux,
            shared,
            bones: None,
        }));
    }

    /// Reconcile a changed outfit with its wearer graph. Keep the running
    /// instance when the new models still use the same project and rig.
    pub fn replace_aux(&mut self, aux: Option<GraphAnim>) {
        if let (Some(old), Some(new)) = (&self.aux, &aux)
            && old
                .anim
                .matches(&new.project, &new.skeleton_path, new.female)
        {
            return;
        }
        self.aux = None;
        if let Some(aux) = aux {
            self.attach_aux(aux);
        }
    }

    /// Whether this instance can still animate the same character and rig.
    pub fn matches(&self, project: &Arc<ProjectRuntime>, rig: &str, female: bool) -> bool {
        Arc::ptr_eq(&self.project, project) && self.skeleton_path == rig && self.female == female
    }

    fn source<'a>(
        &'a mut self,
        vfs: &'a vfs::Vfs,
        anims: &'a mut AnimationLibrary,
        skeleton: &'a Skeleton,
    ) -> (&'a mut Instance, Source<'a>) {
        let GraphAnim {
            inst,
            project,
            skeleton_path,
            female,
            clips,
            ..
        } = self;
        let project: &'a Arc<ProjectRuntime> = project;
        (
            inst,
            Source {
                clips,
                vfs,
                anims,
                skeleton,
                project,
                skeleton_path,
                female: *female,
            },
        )
    }

    /// Root motion of the clip a sample plays: the project's animation data names
    /// clips by their generator (`Forward_Walk` plays `WalkForward.hkx`).
    fn motion(
        &mut self,
        graph: usize,
        generator: usize,
        anims: &mut AnimationLibrary,
        vfs: &vfs::Vfs,
    ) -> Option<Arc<Motion>> {
        if let Some(m) = self.motions.get(&(graph, generator)) {
            return m.clone();
        }
        let name = self
            .project
            .shared
            .project
            .graphs
            .get(graph)
            .and_then(|(_, g)| g.generators.get(generator))
            .map(|g| g.name().to_ascii_lowercase());
        let m = name.and_then(|n| {
            anims
                .project_motions(vfs, &self.project.name)
                .get(n.strip_suffix(".hkx").unwrap_or(&n))
                .cloned()
        });
        self.motions.insert((graph, generator), m.clone());
        m
    }

    /// Ground speed of the graph's slowest forward locomotion: a scratch instance of
    /// the graph is set walking and its clips' root motion measured.
    pub fn walk_speed(
        &mut self,
        vfs: &vfs::Vfs,
        anims: &mut AnimationLibrary,
        skeleton: &Skeleton,
    ) -> Option<f32> {
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
            let Some(m) = self.motion(s.graph, s.generator, anims, vfs) else {
                continue;
            };
            let Some(d) = self
                .clips
                .get(&s.animation)
                .cloned()
                .flatten()
                .map(|c| c.duration())
            else {
                continue;
            };
            speed += m.end().0.truncate().length() / d * s.weight;
            weight += s.weight;
        }
        (weight > 0.5)
            .then(|| speed / weight)
            .filter(|v| (10.0..400.0).contains(v))
    }

    /// Run the aux graph, if any, one update on: it hears the events this graph
    /// raised, takes over the variables they share, and its bones' locals go
    /// into `locals`.
    fn pose_aux(
        &mut self,
        dt: f32,
        raised: &[Raised],
        vfs: &vfs::Vfs,
        anims: &mut AnimationLibrary,
        skeleton: &Skeleton,
        locals: &mut [Transform],
    ) {
        let Some(mut aux) = self.aux.take() else {
            return;
        };
        for r in raised {
            aux.anim.inst.send_event(&r.event);
        }
        for name in &aux.shared {
            if let Some(v) = self.inst.variable(name) {
                aux.anim.inst.set_variable(name, v);
            }
        }
        let frame = aux.anim.update(dt, vfs, anims, skeleton);
        let bones = aux.bones.get_or_insert_with(|| {
            let bones: Vec<usize> = anims
                .havok_bone_map(vfs, &aux.anim.skeleton_path, skeleton)
                .into_iter()
                .flatten()
                .collect();
            log::debug!("{}: a second graph poses {} bones", self.label, bones.len());
            bones
        });
        log::trace!("{}: tail {:?}", self.label, aux.anim.active_states());
        take_bones(locals, &frame.locals, bones);
        self.aux = Some(aux);
    }

    pub fn project(&self) -> &Arc<ProjectRuntime> {
        &self.project
    }

    pub fn variable(&self, name: &str) -> Option<f32> {
        self.inst.variable(name)
    }

    /// The engine's own write (movement, combat, equipment...): ignored, and
    /// false, while a script's value stands (`ScriptPins::holds`).
    pub fn set_variable(&mut self, name: &str, value: f32) -> bool {
        !self.pinned.holds(name, value) && self.inst.set_variable(name, value)
    }

    /// A script's write (`SetAnimationVariable*`): the variable keeps the value
    /// while the engine goes on writing what it had before, until the engine's
    /// own value changes.
    pub fn set_script_variable(&mut self, name: &str, value: f32) -> bool {
        let Some(before) = self.inst.variable(name) else {
            return false;
        };
        let known = self.inst.set_variable(name, value);
        if known {
            self.pinned.pin(name, before);
        }
        known
    }

    /// How many times the graph changed state so far (`Instance::changes`).
    pub fn changes(&self) -> u64 {
        self.inst.changes()
    }

    /// Queue an event for the next update.
    pub fn send_event(&mut self, name: &str) -> bool {
        if let Some(a) = self.aux.as_mut() {
            a.anim.send_event(name);
        }
        self.inst.send_event(name)
    }

    /// Handle an event now; true when the graph took it (changed state).
    pub fn handle_event(
        &mut self,
        name: &str,
        vfs: &vfs::Vfs,
        anims: &mut AnimationLibrary,
        skeleton: &Skeleton,
    ) -> bool {
        if let Some(a) = self.aux.as_mut() {
            a.anim.inst.send_event(name);
        }
        let (inst, mut src) = self.source(vfs, anims, skeleton);
        inst.handle_event(name, &mut src)
    }

    /// A transition the graph took is waiting for its moment (initiate interval).
    pub fn waiting(&self) -> bool {
        self.inst.waiting()
    }

    pub fn active_states(&self) -> Vec<String> {
        self.inst.active_states()
    }

    /// Advance the graph and blend its clips.
    pub fn update(
        &mut self,
        dt: f32,
        vfs: &vfs::Vfs,
        anims: &mut AnimationLibrary,
        skeleton: &Skeleton,
    ) -> Frame {
        let (inst, mut src) = self.source(vfs, anims, skeleton);
        inst.update(dt, &mut src);
        let samples = inst.samples();
        let raised = inst.take_raised();
        let turn = inst.character_turn(dt);
        let clips: Vec<Option<Arc<BoundClip>>> =
            samples.iter().map(|s| src.clip(&s.animation)).collect();

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
                let Some(Some(b)) = clip.track_to_bone.get(track) else {
                    continue;
                };
                let factor = match (&s.mask, clip.track_to_hk.get(track).copied().flatten()) {
                    (Some(m), Some(hk)) => m.get(hk).copied().unwrap_or(0.0),
                    _ => 1.0,
                };
                let w = s.weight * factor;
                if w <= 0.0 {
                    continue;
                }
                let r = if q.rotation.is_finite() {
                    q.rotation
                } else {
                    Quat::IDENTITY
                };
                let v = glam::Vec4::from(r);
                let v = if rot[*b].dot(v) < 0.0 { -v } else { v };
                weight[*b] += w;
                trans[*b] += q.translation * w;
                rot[*b] += v * w;
                scale[*b] += q.scale.x * w;
            }
            let m = self
                .motion(s.graph, s.generator, anims, vfs)
                .or_else(|| clip.motion.clone());
            let (d, yaw) = m.map_or((Vec3::ZERO, 0.0), |m| {
                m.delta(s.prev_time, s.time, s.wrapped, clip.duration())
            });
            motion.0 += d * s.weight;
            motion.1 += yaw * s.weight;
            motion_weight += s.weight;
        }
        if motion_weight > 1e-4 {
            motion.0 /= motion_weight;
            motion.1 /= motion_weight;
        }
        // `hkbRotateCharacterModifier` turns the character on top of its clips' root motion.
        motion.1 += turn;
        if log::log_enabled!(log::Level::Trace) {
            let bare: Vec<&str> = (0..n)
                .filter(|&b| weight[b] <= 1e-6)
                .filter_map(|b| skeleton.bone_name(b))
                .collect();
            let what: Vec<String> = samples
                .iter()
                .zip(&clips)
                .map(|(s, c)| {
                    format!(
                        "{:.2} {}{}",
                        s.weight,
                        s.animation,
                        if c.is_some() { "" } else { " (missing)" }
                    )
                })
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
                (
                    trans[b] / w,
                    Quat::from_vec4(rot[b]).normalize(),
                    scale[b] / w,
                )
            })
            .collect();
        // Additive clips (body weight offsets...) go on top, scaled by their weight.
        for (s, clip) in samples.iter().zip(&clips) {
            let Some(clip) = clip.as_ref().filter(|_| s.additive) else {
                continue;
            };
            clip.anim.sample(s.time, &mut self.scratch);
            for (track, q) in self.scratch.iter().enumerate() {
                let Some(Some(b)) = clip.track_to_bone.get(track) else {
                    continue;
                };
                let factor = match (&s.mask, clip.track_to_hk.get(track).copied().flatten()) {
                    (Some(m), Some(hk)) => m.get(hk).copied().unwrap_or(0.0),
                    _ => 1.0,
                };
                // Relative to the layer it sits in: normalised per bone like the poses.
                let w = if weight[*b] > 1e-6 {
                    (s.weight * factor / weight[*b]).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                if w <= 0.0 {
                    continue;
                }
                let r = if q.rotation.is_finite() {
                    q.rotation
                } else {
                    Quat::IDENTITY
                };
                let l = &mut locals[*b];
                l.0 += q.translation * w;
                l.1 = (l.1 * Quat::IDENTITY.slerp(r, w)).normalize();
                l.2 *= 1.0 + (q.scale.x - 1.0) * w;
            }
        }
        let mut locals: Vec<Transform> = locals
            .into_iter()
            .map(|(t, q, s)| Transform {
                translation: t,
                rotation: glam::Mat3::from_quat(q),
                scale: s,
            })
            .collect();
        self.pose_aux(dt, &raised, vfs, anims, skeleton, &mut locals);
        let twists = self.inst.twists();
        if !twists.is_empty() {
            let map = self
                .hk_bones
                .get_or_insert_with(|| anims.havok_bone_map(vfs, &self.skeleton_path, skeleton));
            for t in &twists {
                twist::apply(t, map, skeleton, &mut locals);
            }
        }
        // Feet on the ground (while the graph runs its foot IK controls), then head
        // tracking on top (the first look-at modifier running).
        if let Some(ik) = self
            .project
            .shared
            .project
            .character
            .as_ref()
            .and_then(|c| c.foot_ik.as_ref())
        {
            let map = self
                .hk_bones
                .get_or_insert_with(|| anims.havok_bone_map(vfs, &self.skeleton_path, skeleton));
            let gains = self.inst.foot_ik_controls();
            self.ankles = footik::apply(
                &mut self.feet,
                ik,
                gains.as_ref(),
                map,
                skeleton,
                &mut locals,
                &self.ground,
                dt,
            );
        }
        if let Some(l) = self.inst.look_ats().first() {
            let map = self
                .hk_bones
                .get_or_insert_with(|| anims.havok_bone_map(vfs, &self.skeleton_path, skeleton));
            let outside = lookat::apply(
                &mut self.look,
                l,
                map,
                skeleton,
                &mut locals,
                self.look_target,
                dt,
            );
            self.inst
                .set_variable("LookAtOutOfRange", if outside { 1.0 } else { 0.0 });
        }
        Frame {
            pose: skeleton.model_space(&locals),
            locals,
            motion,
            raised,
        }
    }
}

#[cfg(test)]
mod pin_tests {
    use super::{ScriptPins, take_bones};
    use nif::Transform;

    #[test]
    fn outfit_changes_keep_replace_and_remove_wearer_graphs() {
        use super::{GraphAnim, ProjectRuntime, Shared, Skeleton};
        use havok::behavior::{BehaviorGraph, ClipMode, Generator, Project};
        use std::sync::Arc;

        let project = || {
            let graph = BehaviorGraph::new(
                "SyntheticTail",
                Some(0),
                vec![Generator::Clip {
                    name: "Tail".into(),
                    animation: "Tail".into(),
                    mode: ClipMode::Looping,
                    speed: 1.0,
                    enforced_duration: 0.0,
                    user_fraction: 0.0,
                    triggers: vec![],
                    crop_start: 0.0,
                    crop_end: 0.0,
                    start_time: 0.0,
                }],
                vec![],
            );
            Arc::new(ProjectRuntime {
                shared: Shared::new(Arc::new(Project {
                    graphs: vec![("synthetic".into(), graph)],
                    character: None,
                })),
                dir: "synthetic".into(),
                name: "tail".into(),
                files: vec![],
            })
        };
        let skel = Skeleton::default();
        let first = project();
        let graph = |p, rig, female| GraphAnim::new(p, rig, female, &skel, 1);
        let mut body = graph(first.clone(), "body", false);
        body.replace_aux(Some(graph(first.clone(), "tail", false)));
        let aux = body.aux.as_mut().unwrap();
        aux.anim.inst.update(0.75, &mut |_: &str| Some(2.0));
        aux.bones = Some(vec![3]);
        body.replace_aux(Some(graph(first.clone(), "tail", false)));
        let aux = body.aux.as_ref().unwrap();
        assert_eq!(aux.anim.inst.samples()[0].time, 0.75);
        assert_eq!(aux.bones.as_deref(), Some([3].as_slice()));

        // A different rig or character must discard the old bone map/state.
        body.replace_aux(Some(graph(first.clone(), "other-rig", false)));
        assert!(body.aux.as_ref().unwrap().bones.is_none());
        body.aux.as_mut().unwrap().bones = Some(vec![4]);
        body.replace_aux(Some(graph(first.clone(), "other-rig", true)));
        assert!(body.aux.as_ref().unwrap().bones.is_none());
        let second = project();
        body.replace_aux(Some(graph(second.clone(), "other-rig", true)));
        assert!(Arc::ptr_eq(
            &body.aux.as_ref().unwrap().anim.project,
            &second
        ));
        body.replace_aux(None);
        assert!(body.aux.is_none());
        body.replace_aux(Some(graph(first, "tail", false)));
        assert!(body.aux.is_some());
    }

    #[test]
    fn a_second_graph_replaces_only_its_own_bones() {
        let at = |x: f32| Transform {
            translation: glam::Vec3::X * x,
            ..Transform::default()
        };
        let mut locals = vec![at(1.0), at(2.0), at(3.0)];
        let from = vec![at(10.0), at(20.0), at(30.0)];
        take_bones(&mut locals, &from, &[1, 7]);
        let xs: Vec<f32> = locals.iter().map(|t| t.translation.x).collect();
        assert_eq!(xs, [1.0, 20.0, 3.0]);
    }

    #[test]
    fn pins_match_ignoring_case_and_once() {
        let mut p = ScriptPins::default();
        assert!(!p.holds("Speed", 0.0));
        p.pin("Speed", 0.0);
        p.pin("SPEED", 5.0);
        assert!(p.holds("speed", 0.0));
        assert!(!p.holds("Direction", 0.0));
        assert_eq!(p.0.len(), 1);
    }

    /// `DLC2BookDungeonControllerScript` sets the player's `iLeftHandType` to 0:
    /// it stays while the engine writes the old hand type, and gives way when
    /// something else is equipped.
    #[test]
    fn the_engine_changing_its_value_ends_a_pin() {
        let mut p = ScriptPins::default();
        p.pin("iLeftHandType", 1.0);
        assert!(p.holds("iLeftHandType", 1.0));
        assert!(p.holds("iLeftHandType", 1.0));
        assert!(!p.holds("iLeftHandType", 4.0));
        assert!(!p.holds("iLeftHandType", 1.0));
    }
}
