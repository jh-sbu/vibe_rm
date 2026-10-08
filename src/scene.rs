//! Scenes (`SCEN`): quest aliases acting out phases of dialogue, packages and
//! timers. A scene starts from Papyrus (`Start`, `ForceStart`) or with its quest
//! ("Begin on Quest Start"), plays its phases in order and ends after the last.
//!
//! A phase whose start conditions fail is skipped. Entering a phase starts the
//! actions that begin in it; the phase ends when its completion conditions pass,
//! or, without any, when every action ending in it is complete: dialogue once the
//! line is said, a package once it is done (a travel package's actor has arrived,
//! a seat taken, a force greet's conversation held), a timer once it runs out.
//! Looping dialogue repeats until its end phase ends and doesn't hold it up.
//! Ending a phase stops the actions that end in it. Speakers and package actors
//! must be loaded: a scene whose actors are elsewhere waits for them.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use esp::FormId;
use esp::scene::{ActionKind, action_flags, actor_flags, flags};
use glam::Vec3;

use crate::ai::package::{self, Behaviour, Package};
use crate::condition::{self, Condition};
use crate::dialogue::{Info, Response, Topic};
use crate::engine::{Engine, PLAYER_REF};

/// Scene lines are subtitled within this distance of the player.
pub const SUBTITLE_DISTANCE: f32 = 1500.0;
/// Package actions of procedures not run here (shouting, magic, activating...)
/// count as done this long after they start.
const UNSUPPORTED_PACKAGE_SECS: f64 = 1.0;
/// How close to a travel package's location counts as arrived, beyond its radius.
const ARRIVED_SLACK: f32 = 150.0;

/// A scene as read, conditions parsed.
pub struct SceneDef {
    pub scene: esp::scene::Scene,
    start: Vec<Vec<Condition>>,
    completion: Vec<Vec<Condition>>,
    conditions: Vec<Condition>,
    vmad: crate::script::vmad::Vmad,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Waiting,
    Running,
    Done,
    Stopped,
}

/// A line being said.
#[derive(Debug, Clone)]
pub struct SceneLine {
    pub speaker: FormId,
    pub name: String,
    pub text: String,
    voice: Option<crate::audio::VoiceId>,
    ends_at: f64,
}

struct ActionRun {
    state: State,
    actor: Option<FormId>,
    started_at: f64,
    /// Dialogue: the line being said, those left of the INFO, the INFO, and when
    /// a looping action speaks again.
    line: Option<SceneLine>,
    queue: Vec<Response>,
    info: Option<(Topic, Info)>,
    next_loop: Option<f64>,
}

impl ActionRun {
    fn new() -> Self {
        ActionRun { state: State::Waiting, actor: None, started_at: 0.0, line: None, queue: Vec::new(), info: None, next_loop: None }
    }
}

pub struct SceneRun {
    pub scene: FormId,
    pub def: Arc<SceneDef>,
    /// The phase playing; `None` before the first is entered.
    pub phase: Option<usize>,
    begun: bool,
    stopping: bool,
    paused: bool,
    actions: Vec<ActionRun>,
}

impl SceneRun {
    /// The actors filling the scene's aliases.
    fn actors<'a>(&'a self, e: &'a Engine) -> impl Iterator<Item = (FormId, &'a esp::scene::Actor)> + 'a {
        self.def.scene.actors.iter().filter_map(move |a| Some((e.alias_ref(self.def.scene.quest, a.alias)?, a)))
    }
}

#[derive(Default)]
pub struct Scenes {
    defs: HashMap<FormId, Option<Arc<SceneDef>>>,
    by_quest: Option<HashMap<FormId, Vec<FormId>>>,
    pub running: Vec<SceneRun>,
    /// Packages scene package actions give actors, run ahead of all others.
    pub packages: HashMap<FormId, (FormId, u32, Vec<Package>)>,
    /// Actors that force greeted the player while their scene package ran.
    pub(crate) force_greeted: HashSet<FormId>,
    /// Scenes to start again once ended ("Repeat Conditions While True").
    restart: Vec<FormId>,
}

fn conditions(rec: &esp::LoadedRecord<'_>, raw: &[esp::scene::RawCondition]) -> Vec<Condition> {
    raw.iter()
        .filter_map(|c| {
            let mut cond = condition::parse(rec, &c.ctda)?;
            cond.string_p1 = c.cis1.as_deref().map(Into::into);
            cond.string_p2 = c.cis2.as_deref().map(Into::into);
            Some(cond)
        })
        .collect()
}

impl Engine {
    pub fn scene_def(&mut self, id: FormId) -> Option<Arc<SceneDef>> {
        if let Some(d) = self.scenes.defs.get(&id) {
            return d.clone();
        }
        let def = self.lo.get(id).and_then(|rec| {
            let scene = esp::scene::parse(&rec, id)?;
            Some(Arc::new(SceneDef {
                start: scene.phases.iter().map(|p| conditions(&rec, &p.start)).collect(),
                completion: scene.phases.iter().map(|p| conditions(&rec, &p.completion)).collect(),
                conditions: conditions(&rec, &scene.conditions),
                vmad: crate::script::vmad::parse(&rec).unwrap_or_default(),
                scene,
            }))
        });
        self.scenes.defs.insert(id, def.clone());
        def
    }

    /// The scenes a quest owns.
    fn quest_scenes(&mut self, q: FormId) -> Vec<FormId> {
        if self.scenes.by_quest.is_none() {
            let mut m: HashMap<FormId, Vec<FormId>> = HashMap::new();
            for &s in self.lo.ids_of_type(b"SCEN") {
                // The owning quest's PNAM follows the actions' own (packages).
                if let Some(scene) = self.lo.get(s).and_then(|rec| esp::scene::parse(&rec, s)) {
                    m.entry(scene.quest).or_default().push(s);
                }
            }
            self.scenes.by_quest = Some(m);
        }
        self.scenes.by_quest.as_ref().and_then(|m| m.get(&q).cloned()).unwrap_or_default()
    }

    pub fn is_scene_playing(&self, s: FormId) -> bool {
        self.scenes.running.iter().any(|r| r.scene == s && !r.stopping)
    }

    /// The running scene `r` acts in.
    pub fn scene_of_actor(&self, r: FormId) -> Option<&SceneRun> {
        self.scenes.running.iter().find(|run| !run.stopping && run.actors(self).any(|(a, _)| a == r))
    }

    pub fn is_scene_action_complete(&self, s: FormId, index: u32) -> bool {
        self.scenes.running.iter().find(|r| r.scene == s).is_some_and(|run| {
            run.def.scene.actions.iter().zip(&run.actions).any(|(a, ar)| a.index == index && matches!(ar.state, State::Done | State::Stopped))
        })
    }

    /// Whether `r` is saying a scene line.
    pub fn is_scene_speaking(&self, r: FormId) -> bool {
        self.scenes.running.iter().flat_map(|run| &run.actions).any(|a| a.line.as_ref().is_some_and(|l| l.speaker == r))
    }

    /// Scene lines being said near the player, for subtitles.
    pub fn scene_lines(&self) -> Vec<&SceneLine> {
        let player = self.player.position;
        self.scenes
            .running
            .iter()
            .flat_map(|run| &run.actions)
            .filter_map(|a| a.line.as_ref())
            .filter(|l| self.ref_position(l.speaker).is_some_and(|p| p.distance(player) < SUBTITLE_DISTANCE))
            .collect()
    }

    /// The scene packages `r` runs ahead of its own.
    pub(crate) fn scene_packages(&self, r: FormId) -> Vec<Package> {
        self.scenes.packages.get(&r).map(|(_, _, p)| p.clone()).unwrap_or_default()
    }

    /// Start a scene (Papyrus `Start` / `ForceStart`): its quest must be running,
    /// its required actors' aliases filled and, unless forced, its conditions
    /// pass and its actors free of other scenes (forced, it takes them from
    /// those). Returns whether it is playing.
    pub fn start_scene(&mut self, s: FormId, force: bool) -> bool {
        if self.is_scene_playing(s) {
            return true;
        }
        let Some(def) = self.scene_def(s) else { return false };
        let quest = def.scene.quest;
        if !self.scripts.quests.get(&quest).is_some_and(|q| q.running) {
            log::debug!("scene {} not started: quest {quest} isn't running", def.scene.editor_id);
            return false;
        }
        if !force && !condition::evaluate(self, &def.conditions, condition::Context { quest: Some(quest), ..Default::default() }) {
            log::debug!("scene {} not started: conditions fail", def.scene.editor_id);
            return false;
        }
        let mut busy = Vec::new();
        for a in &def.scene.actors {
            match self.alias_ref(quest, a.alias) {
                None if a.flags & actor_flags::OPTIONAL == 0 => {
                    log::debug!("scene {} not started: alias {} is empty", def.scene.editor_id, a.alias);
                    return false;
                }
                None => {}
                Some(r) => {
                    if let Some(other) = self.scene_of_actor(r) {
                        busy.push(other.scene);
                    }
                }
            }
        }
        if !busy.is_empty() {
            if !force {
                log::debug!("scene {} not started: its actors are in {busy:?}", def.scene.editor_id);
                return false;
            }
            for b in busy {
                self.stop_scene(b);
            }
        }
        log::info!("scene {} starts", def.scene.editor_id);
        let actions = def.scene.actions.iter().map(|_| ActionRun::new()).collect();
        self.scenes.running.push(SceneRun { scene: s, def, phase: None, begun: false, stopping: false, paused: false, actions });
        true
    }

    /// Stop a scene: it ends (its end fragment runs) on the next update.
    pub fn stop_scene(&mut self, s: FormId) {
        for run in self.scenes.running.iter_mut().filter(|r| r.scene == s) {
            run.stopping = true;
        }
        self.scenes.restart.retain(|&x| x != s);
    }

    /// Start the scenes of a quest that begin with it.
    pub(crate) fn start_quest_scenes(&mut self, q: FormId) {
        for s in self.quest_scenes(q) {
            if self.scene_def(s).is_some_and(|d| d.scene.flags & flags::BEGIN_ON_QUEST_START != 0) {
                self.start_scene(s, false);
            }
        }
    }

    /// Stop the scenes of a quest that stopped.
    pub(crate) fn stop_quest_scenes(&mut self, q: FormId) {
        let ids: Vec<FormId> = self.scenes.running.iter().filter(|r| r.def.scene.quest == q).map(|r| r.scene).collect();
        for s in ids {
            self.stop_scene(s);
        }
    }

    fn run_scene_fragment(&mut self, def: &SceneDef, func: &str) {
        let Some(frag) = &def.vmad.scene else { return };
        let obj = papyrus::ObjectId::Form(def.scene.id.0);
        let mut vm = std::mem::take(&mut self.vm);
        {
            let mut host = crate::script::EngineHost { engine: self };
            for s in &def.vmad.scripts {
                let props: Vec<(String, papyrus::Value)> = s
                    .properties
                    .iter()
                    .map(|(n, pv)| (n.clone(), crate::script::vmad::to_value(pv, &|f| host.engine.native_class(f))))
                    .collect();
                vm.attach(&mut host, obj, &s.name, &props);
            }
            log::debug!("scene {} fragment {func}", def.scene.editor_id);
            vm.call_method(&mut host, obj, &frag.script, func, vec![]);
        }
        self.vm = vm;
    }

    fn run_phase_fragment(&mut self, def: &SceneDef, phase: usize, completion: bool) {
        let funcs: Vec<String> = def
            .vmad
            .scene
            .iter()
            .flat_map(|f| &f.phases)
            .filter(|(p, c, _)| *p as usize == phase && *c == completion)
            .map(|(_, _, f)| f.clone())
            .collect();
        for f in funcs {
            self.run_scene_fragment(def, &f);
        }
    }

    /// Play the running scenes (called every frame).
    pub(crate) fn update_scenes(&mut self) {
        for s in std::mem::take(&mut self.scenes.restart) {
            self.start_scene(s, false);
        }
        let mut i = 0;
        while i < self.scenes.running.len() {
            if self.step_scene(i) {
                i += 1;
            } else {
                let run = self.scenes.running.remove(i);
                self.end_scene(run);
            }
        }
    }

    /// Advance scene `i`; false when it has ended.
    fn step_scene(&mut self, i: usize) -> bool {
        let now = self.scripts.real_time;
        let def = self.scenes.running[i].def.clone();
        let quest = def.scene.quest;
        if !self.scenes.running[i].begun {
            self.scenes.running[i].begun = true;
            if let Some(f) = def.vmad.scene.as_ref().and_then(|f| f.begin.clone()) {
                self.run_scene_fragment(&def, &f);
            }
        }
        if self.scenes.running[i].stopping || !self.scripts.quests.get(&quest).is_some_and(|q| q.running) {
            return false;
        }
        // Actors' behaviour flags: dead, fighting or talking to the player.
        let mut pause = false;
        let talking = self.conversation.as_ref().map(|c| c.npc_ref);
        let actors: Vec<(FormId, u32)> = self.scenes.running[i].actors(self).map(|(r, a)| (r, a.behaviour)).collect();
        for (r, b) in actors {
            let dead = self.is_dead(r);
            let fighting = self.actor_ref(r).is_some_and(|a| a.combat.is_some());
            let talks = talking == Some(r);
            if (dead && b & actor_flags::DEATH_END != 0) || (fighting && b & actor_flags::COMBAT_END != 0) || (talks && b & actor_flags::DIALOGUE_END != 0) {
                log::info!("scene {} ends: {r} {}", def.scene.editor_id, if dead { "died" } else if fighting { "is fighting" } else { "talks to the player" });
                return false;
            }
            pause |= (fighting && b & actor_flags::COMBAT_PAUSE != 0) || (talks && b & actor_flags::DIALOGUE_PAUSE != 0);
        }
        self.scenes.running[i].paused = pause;
        if pause {
            return true;
        }
        let ctx = condition::Context { quest: Some(quest), ..Default::default() };
        // Several phases can pass in one update (skipped, or with nothing to wait for).
        for _ in 0..=def.scene.phases.len() {
            let phase = match self.scenes.running[i].phase {
                Some(p) => p,
                None => {
                    if !self.enter_phase(i, 0, &def) {
                        return false;
                    }
                    continue;
                }
            };
            self.update_actions(i, &def, phase, now);
            let complete = if def.completion[phase].is_empty() {
                def.scene.actions.iter().zip(&self.scenes.running[i].actions).all(|(a, ar)| {
                    a.end_phase as usize != phase || a.flags & action_flags::LOOPING != 0 || matches!(ar.state, State::Done | State::Stopped)
                })
            } else {
                condition::evaluate(self, &def.completion[phase], ctx)
            };
            if !complete {
                return true;
            }
            log::debug!("scene {} phase {phase} ({}) complete", def.scene.editor_id, def.scene.phases[phase].name);
            self.run_phase_fragment(&def, phase, true);
            // Actions ending here stop (and those that should have ended by now).
            for k in 0..def.scene.actions.len() {
                if def.scene.actions[k].end_phase as usize <= phase {
                    self.stop_action(i, k, &def);
                }
            }
            if self.scenes.running[i].stopping || !self.enter_phase(i, phase + 1, &def) {
                return false;
            }
        }
        true
    }

    /// Enter the first phase from `from` whose start conditions pass; false past
    /// the last.
    fn enter_phase(&mut self, i: usize, from: usize, def: &Arc<SceneDef>) -> bool {
        let ctx = condition::Context { quest: Some(def.scene.quest), ..Default::default() };
        let mut p = from;
        while p < def.scene.phases.len() && !condition::evaluate(self, &def.start[p], ctx) {
            log::debug!("scene {} skips phase {p}", def.scene.editor_id);
            p += 1;
        }
        if p >= def.scene.phases.len() {
            return false;
        }
        log::debug!("scene {} phase {p} ({})", def.scene.editor_id, def.scene.phases[p].name);
        self.scenes.running[i].phase = Some(p);
        self.run_phase_fragment(def, p, false);
        let now = self.scripts.real_time;
        for k in 0..def.scene.actions.len() {
            let a = &def.scene.actions[k];
            if (a.start_phase as usize) <= p && (a.end_phase as usize) >= p && self.scenes.running[i].actions[k].state == State::Waiting {
                let actor = (a.actor >= 0).then(|| self.alias_ref(def.scene.quest, a.actor as u32)).flatten();
                let ar = &mut self.scenes.running[i].actions[k];
                ar.state = State::Running;
                ar.actor = actor;
                ar.started_at = now;
            }
        }
        true
    }

    /// Run the actions of the current phase.
    fn update_actions(&mut self, i: usize, def: &Arc<SceneDef>, phase: usize, now: f64) {
        for k in 0..def.scene.actions.len() {
            if self.scenes.running.get(i).is_none_or(|r| r.actions[k].state != State::Running) {
                continue;
            }
            let a = &def.scene.actions[k];
            let actor = self.scenes.running[i].actions[k].actor;
            let started = self.scenes.running[i].actions[k].started_at;
            let done = match &a.kind {
                ActionKind::Timer { seconds } => now - started >= *seconds as f64,
                ActionKind::Dialogue { topic, .. } => self.update_dialogue_action(i, k, *topic, actor, a.flags & action_flags::LOOPING != 0, now),
                ActionKind::Package { packages } => self.update_package_action(i, k, def, packages, actor, now),
            };
            if done {
                self.scenes.running[i].actions[k].state = State::Done;
                log::debug!("scene {} action {} done (phase {phase})", def.scene.editor_id, a.index);
            }
        }
    }

    /// Say the action's line; true once said (looping lines never finish).
    fn update_dialogue_action(&mut self, i: usize, k: usize, topic: FormId, actor: Option<FormId>, looping: bool, now: f64) -> bool {
        // Head tracking only, or nobody (or the player) to speak.
        let Some(speaker) = actor.filter(|&r| !topic.is_null() && r != PLAYER_REF && !self.is_dead(r)) else { return true };
        // Speakers say their lines where they are loaded.
        if self.actor_ref(speaker).is_none() {
            return false;
        }
        let ar = &self.scenes.running[i].actions[k];
        if let Some(line) = &ar.line {
            let finished = match (line.voice, &self.audio) {
                (Some(v), Some(a)) => !a.is_playing(v),
                _ => now >= line.ends_at,
            };
            if !finished {
                return false;
            }
            self.scenes.running[i].actions[k].line = None;
            if !self.scenes.running[i].actions[k].queue.is_empty() {
                self.next_scene_line(i, k, speaker);
                return false;
            }
            if let Some((_, info)) = self.scenes.running[i].actions[k].info.take() {
                self.run_info_fragment(&info, false, speaker);
            }
            self.end_talking_gestures(speaker);
            if !looping {
                return true;
            }
            let (lo, hi) = match &self.scenes.running[i].def.scene.actions[k].kind {
                ActionKind::Dialogue { loop_min, loop_max, .. } => (*loop_min as f64, (*loop_max).max(*loop_min) as f64),
                _ => (1.0, 1.0),
            };
            let wait = lo + (self.rand() % 1000) as f64 / 1000.0 * (hi - lo);
            self.scenes.running[i].actions[k].next_loop = Some(now + wait);
            return false;
        }
        if ar.next_loop.is_some_and(|t| now < t) {
            return false;
        }
        // Start the line.
        let Some(t) = crate::dialogue::topic(&self.lo, topic) else { return true };
        let Some(info) = self.select_info(&t, speaker) else {
            log::debug!("scene line {}: no INFO for {speaker}", t.editor_id);
            return !looping;
        };
        self.run_info_fragment(&info, true, speaker);
        let ar = &mut self.scenes.running[i].actions[k];
        ar.queue = info.responses.clone();
        ar.info = Some((t, info));
        ar.next_loop = None;
        if ar.queue.is_empty() {
            return !looping;
        }
        self.next_scene_line(i, k, speaker);
        false
    }

    /// Say the next response of the action's INFO.
    fn next_scene_line(&mut self, i: usize, k: usize, speaker: FormId) {
        let ar = &mut self.scenes.running[i].actions[k];
        let r = ar.queue.remove(0);
        let Some((topic, info)) = ar.info.clone() else { return };
        let voice_type = self.actor_voice_type(speaker).unwrap_or_default();
        let path = crate::dialogue::voice_path(&self.lo, voice_type, &topic, info.id, r.number);
        let at = self.ref_position(speaker).map(|p| p + Vec3::Z * 110.0);
        let mut voice = None;
        if let (Some(p), Some(audio)) = (&path, self.audio.as_mut())
            && self.vfs.exists(p)
        {
            voice = audio.play(&self.vfs, p, 1.0, false, at, 400.0, 3000.0);
        }
        let duration = (r.text.split_whitespace().count() as f64 * 0.32).max(2.0);
        let name = self.form_name(speaker);
        log::info!("{name} (scene): {} [{}]", r.text, path.unwrap_or_default());
        let now = self.scripts.real_time;
        self.scenes.running[i].actions[k].line = Some(SceneLine { speaker, name, text: r.text, voice, ends_at: now + duration });
        self.talking_gesture(speaker);
    }

    /// Give the actor the action's packages; true once the one it runs is done.
    fn update_package_action(&mut self, i: usize, k: usize, def: &Arc<SceneDef>, packages: &[FormId], actor: Option<FormId>, now: f64) -> bool {
        let Some(r) = actor.filter(|&r| r != PLAYER_REF && !self.is_dead(r)) else { return true };
        let index = def.scene.actions[k].index;
        if self.scenes.packages.get(&r).is_none_or(|(s, x, _)| (*s, *x) != (def.scene.id, index)) {
            let packs: Vec<Package> = packages
                .iter()
                .flat_map(|&p| package::expand(&self.lo, p))
                .map(|mut p| {
                    p.quest = p.quest.or(Some(def.scene.quest));
                    p
                })
                .collect();
            log::debug!("scene {}: {r} runs {:?}", def.scene.editor_id, packs.iter().map(|p| p.editor_id.as_str()).collect::<Vec<_>>());
            self.scenes.packages.insert(r, (def.scene.id, index, packs));
            self.scenes.force_greeted.remove(&r);
            self.scripts.alias_gen += 1;
            self.scenes.running[i].actions[k].started_at = now;
            return false;
        }
        let started = self.scenes.running[i].actions[k].started_at;
        let talking = self.conversation.as_ref().map(|c| c.npc_ref);
        let greeted = self.scenes.force_greeted.contains(&r);
        let Some(a) = self.actor_ref(r) else { return false };
        let Some(p) = a.current.and_then(|c| a.packages.get(c)).filter(|p| packages.contains(&p.id)) else { return false };
        let Some(goal) = a.goal else { return false };
        match p.behaviour {
            Behaviour::Travel | Behaviour::Escort => {
                // Arrived: in the place the package names (not on the way out of
                // another), standing near its location.
                let here = self.current_place_of(r).map(|x| x.0);
                let there = self.package_place(r, p);
                let in_place = match there {
                    Some((place, _)) => here.is_some_and(|h| crate::ai::schedule::same_space(h, place)),
                    None => true,
                };
                let centre = there.map(|t| t.1).filter(|c| !c.is_nan()).unwrap_or(goal.centre);
                a.exiting.is_none() && in_place && !a.is_walking() && a.pos.truncate().distance(centre.truncate()) <= goal.radius + ARRIVED_SLACK
            }
            Behaviour::Sit | Behaviour::Sleep => a.in_furniture(),
            Behaviour::ForceGreet => greeted && talking != Some(r),
            // Procedures not run here.
            Behaviour::Hold => now - started >= UNSUPPORTED_PACKAGE_SECS,
            // Never done: they run until their end phase ends.
            Behaviour::Sandbox | Behaviour::Follow | Behaviour::Patrol | Behaviour::UseWeapon => false,
        }
    }

    /// Stop action `k` of scene `i` (its end phase ended).
    fn stop_action(&mut self, i: usize, k: usize, def: &SceneDef) {
        let mut ar = std::mem::replace(&mut self.scenes.running[i].actions[k], ActionRun::new());
        self.halt_action(&mut ar, &def.scene.actions[k], def.scene.id);
        self.scenes.running[i].actions[k] = ar;
    }

    /// Stop an action: silence its line, take back its packages.
    fn halt_action(&mut self, ar: &mut ActionRun, a: &esp::scene::Action, scene: FormId) {
        let was = std::mem::replace(&mut ar.state, State::Stopped);
        if let Some(line) = ar.line.take() {
            if let (Some(v), Some(audio)) = (line.voice, &self.audio) {
                audio.stop(v);
            }
            self.end_talking_gestures(line.speaker);
        }
        if matches!(was, State::Running | State::Done)
            && let (ActionKind::Package { .. }, Some(r)) = (&a.kind, ar.actor)
            && self.scenes.packages.get(&r).is_some_and(|(s, x, _)| (*s, *x) == (scene, a.index))
        {
            self.scenes.packages.remove(&r);
            self.scripts.alias_gen += 1;
        }
    }

    fn end_scene(&mut self, mut run: SceneRun) {
        let def = run.def.clone();
        log::info!("scene {} ends", def.scene.editor_id);
        for (a, ar) in def.scene.actions.iter().zip(run.actions.iter_mut()) {
            self.halt_action(ar, a, def.scene.id);
        }
        if let Some(f) = def.vmad.scene.as_ref().and_then(|f| f.end.clone()) {
            self.run_scene_fragment(&def, &f);
        }
        if def.scene.flags & flags::STOP_QUEST_ON_END != 0 {
            self.stop_quest(def.scene.quest);
        } else if def.scene.flags & flags::REPEAT_CONDITIONS_WHILE_TRUE != 0 && !run.stopping {
            self.scenes.restart.push(run.scene);
        }
    }

    /// Who scene dialogue has `r` look at: (target reference, turn to face it).
    pub(crate) fn scene_headtracking(&self) -> HashMap<FormId, (FormId, bool)> {
        let mut out = HashMap::new();
        for run in &self.scenes.running {
            for (a, ar) in run.def.scene.actions.iter().zip(&run.actions) {
                let (ActionKind::Dialogue { headtrack, .. }, Some(r), State::Running | State::Done) = (&a.kind, ar.actor, ar.state) else { continue };
                let target = if a.flags & action_flags::HEADTRACK_PLAYER != 0 {
                    Some(PLAYER_REF)
                } else {
                    headtrack.and_then(|h| self.alias_ref(run.def.scene.quest, h))
                };
                if let Some(t) = target.filter(|&t| t != r) {
                    out.insert(r, (t, a.flags & action_flags::FACE_TARGET != 0 && ar.line.is_some()));
                }
            }
        }
        out
    }

    /// Whether the player may talk to `r`: not while a scene it acts in says no.
    pub fn scene_blocks_activation(&self, r: FormId) -> bool {
        self.scenes.running.iter().filter(|run| !run.stopping).any(|run| run.actors(self).any(|(a, f)| a == r && f.flags & actor_flags::NO_PLAYER_ACTIVATION != 0))
    }

    /// Running scenes, for the console.
    pub fn describe_scenes(&self) -> Vec<String> {
        self.scenes
            .running
            .iter()
            .map(|r| {
                let phase = r.phase.map_or("-".to_string(), |p| format!("{p} {:?}", r.def.scene.phases[p].name));
                let actions: Vec<String> = r
                    .def
                    .scene
                    .actions
                    .iter()
                    .zip(&r.actions)
                    .filter(|(_, ar)| ar.state == State::Running)
                    .map(|(a, ar)| format!("{}:{}", a.index, ar.actor.map_or("-".into(), |x| x.to_string())))
                    .collect();
                format!("{} {} phase {phase}{} running {actions:?}", r.def.scene.editor_id, r.scene, if r.paused { " (paused)" } else { "" })
            })
            .collect()
    }
}
