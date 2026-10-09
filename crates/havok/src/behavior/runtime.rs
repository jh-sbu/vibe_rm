//! Running behaviour graphs: one [`Instance`] per character, fed events and
//! variables by the engine, producing weighted clip samples and the events the
//! graph raises (`AnimObjDraw`, `SoundPlay`...).
//!
//! The model follows Havok Behavior: active state machines take transitions on
//! events (their current state's, their wildcards, and global wildcards of state
//! machines nested in their states) or when a transition's condition holds,
//! cross-fading over the transition's blend time; clips advance and fire their
//! triggers; blenders weight their children by (bound) variables; selectors pick a
//! child by a variable; modifiers that steer control flow run alongside their
//! generators. Pose modifiers (IK, look-at, ragdoll) are not run.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use super::expr::{self, Expr, Statement};
use super::*;

/// Supplies clip lengths (seconds) for animation paths relative to the project
/// (`Animations\male\MT_Idle.hkx`).
pub trait ClipSource {
    fn duration(&mut self, animation: &str) -> Option<f32>;

    /// Whether the animation is additive (an offset to apply on top of a pose,
    /// `hkaAnimationBinding::blendHint`).
    fn additive(&mut self, _animation: &str) -> bool {
        false
    }
}

impl<F: FnMut(&str) -> Option<f32>> ClipSource for F {
    fn duration(&mut self, animation: &str) -> Option<f32> {
        self(animation)
    }
}

/// One clip contributing to the pose.
#[derive(Debug, Clone)]
pub struct Sample {
    pub graph: usize,
    pub generator: GenId,
    /// Animation path as written in the graph.
    pub animation: Arc<str>,
    /// Time in the animation file now and at the previous update (crop and
    /// backwards playback applied); `wrapped` when a loop restarted in between.
    pub time: f32,
    pub prev_time: f32,
    pub wrapped: bool,
    pub weight: f32,
    /// Added on top of the blended pose (with `weight`) rather than blended into it.
    pub additive: bool,
    /// Per-bone factors (Havok skeleton order) when only some bones take part.
    pub mask: Option<Arc<[f32]>>,
}

/// An event the graph raised, for the engine (anim objects, sounds, head tracking...).
#[derive(Debug, Clone, PartialEq)]
pub struct Raised {
    pub event: String,
    pub payload: Option<String>,
}

#[derive(Debug, Clone)]
struct Event {
    id: usize,
}

/// Graph-local -> project-wide indices.
struct Maps {
    vars: Vec<usize>,
    events: Vec<usize>,
}

/// Project-wide tables shared by all instances of a project.
pub struct Shared {
    pub project: Arc<Project>,
    maps: Vec<Maps>,
    var_names: Vec<String>,
    var_index: HashMap<String, usize>,
    var_defaults: Vec<f32>,
    event_names: Vec<String>,
    event_index: HashMap<String, usize>,
    /// Parsed conditions and expression lines.
    conditions: HashMap<String, Option<Expr>>,
    statements: HashMap<String, Option<Statement>>,
    /// Per (graph, state machine): global wildcards of state machines nested in its
    /// states, as (state containing them, transition with nested target).
    promoted: HashMap<(usize, GenId), Vec<(i32, Transition)>>,
    /// Per (graph, bone switch): mask for its default child.
    default_masks: HashMap<(usize, GenId), Arc<[f32]>>,
    /// Bone switches' children's weights, character properties resolved.
    switch_weights: HashMap<(usize, GenId), Vec<Arc<[f32]>>>,
}

impl Shared {
    pub fn new(project: Arc<Project>) -> Arc<Shared> {
        let mut s = Shared {
            maps: Vec::new(),
            var_names: Vec::new(),
            var_index: HashMap::new(),
            var_defaults: Vec::new(),
            event_names: Vec::new(),
            event_index: HashMap::new(),
            conditions: HashMap::new(),
            statements: HashMap::new(),
            promoted: HashMap::new(),
            default_masks: HashMap::new(),
            switch_weights: HashMap::new(),
            project: project.clone(),
        };
        for (gi, (_, g)) in project.graphs.iter().enumerate() {
            let vars = g
                .variables
                .iter()
                .enumerate()
                .map(|(i, name)| {
                    let key = name.to_ascii_lowercase();
                    *s.var_index.entry(key).or_insert_with(|| {
                        s.var_names.push(name.clone());
                        s.var_defaults.push(g.variable_default(i));
                        s.var_names.len() - 1
                    })
                })
                .collect();
            let events = g
                .events
                .iter()
                .map(|name| {
                    *s.event_index
                        .entry(name.to_ascii_lowercase())
                        .or_insert_with(|| {
                            s.event_names.push(name.clone());
                            s.event_names.len() - 1
                        })
                })
                .collect();
            s.maps.push(Maps { vars, events });
            for (id, node) in g.generators.iter().enumerate() {
                match node {
                    Generator::StateMachine { states, .. } => {
                        let mut promoted = Vec::new();
                        for st in states {
                            for (_, nested) in nested_machines(g, st.generator) {
                                let Generator::StateMachine { wildcards, .. } =
                                    &g.generators[nested]
                                else {
                                    continue;
                                };
                                for w in wildcards
                                    .iter()
                                    .filter(|w| w.global_wildcard() && !w.disabled())
                                {
                                    let mut t = w.clone();
                                    t.to_nested = Some(w.to_state);
                                    t.to_state = st.id;
                                    promoted.push((st.id, t));
                                }
                            }
                            for t in st.transitions.iter().chain(wildcards_of(node)) {
                                if let Some(c) = &t.condition {
                                    s.conditions
                                        .entry(c.clone())
                                        .or_insert_with(|| expr::parse(c));
                                }
                            }
                        }
                        if !promoted.is_empty() {
                            s.promoted.insert((gi, id), promoted);
                        }
                    }
                    Generator::BoneSwitch {
                        children,
                        properties,
                        ..
                    } => {
                        let character = s.project.character.as_ref();
                        let weights: Vec<Arc<[f32]>> = children
                            .iter()
                            .enumerate()
                            .map(|(i, c)| {
                                let bound =
                                    properties.get(i).and_then(|p| p.as_ref()).and_then(|p| {
                                        character?.bone_weights.get(&p.to_ascii_lowercase())
                                    });
                                bound.unwrap_or(&c.1).as_slice().into()
                            })
                            .collect();
                        let n = weights.iter().map(|w| w.len()).max().unwrap_or(0);
                        let mask: Vec<f32> = (0..n)
                            .map(|b| {
                                1.0 - weights
                                    .iter()
                                    .map(|w| w.get(b).copied().unwrap_or(0.0))
                                    .fold(0.0, f32::max)
                            })
                            .collect();
                        s.default_masks.insert((gi, id), mask.into());
                        s.switch_weights.insert((gi, id), weights);
                    }
                    _ => {}
                }
            }
            for m in &g.modifiers {
                if let Modifier::Expressions(lines) = m {
                    for l in lines {
                        s.statements
                            .entry(l.clone())
                            .or_insert_with(|| expr::parse_statement(l));
                    }
                }
            }
        }
        Arc::new(s)
    }

    /// Variables shared across the project's graphs, with their defaults.
    pub fn variables(&self) -> impl Iterator<Item = (&str, f32)> {
        self.var_names
            .iter()
            .map(String::as_str)
            .zip(self.var_defaults.iter().copied())
    }

    pub fn event_id(&self, name: &str) -> Option<usize> {
        self.event_index.get(&name.to_ascii_lowercase()).copied()
    }

    pub fn variable_id(&self, name: &str) -> Option<usize> {
        self.var_index.get(&name.to_ascii_lowercase()).copied()
    }

    fn graph(&self, gi: usize) -> &BehaviorGraph {
        &self.project.graphs[gi].1
    }
}

fn wildcards_of(g: &Generator) -> &[Transition] {
    match g {
        Generator::StateMachine { wildcards, .. } => wildcards,
        _ => &[],
    }
}

/// State machines directly below `g` (not inside other state machines), with depth.
fn nested_machines(graph: &BehaviorGraph, g: Option<GenId>) -> Vec<(usize, GenId)> {
    let mut out = Vec::new();
    let mut stack: Vec<(GenId, usize)> = g.into_iter().map(|g| (g, 0)).collect();
    while let Some((g, d)) = stack.pop() {
        if d > 32 {
            continue;
        }
        match &graph.generators[g] {
            Generator::StateMachine { .. } => out.push((d, g)),
            other => stack.extend(other.children().into_iter().map(|c| (c, d + 1))),
        }
    }
    out
}

#[derive(Debug)]
struct Node {
    gi: usize,
    g: GenId,
    kind: Kind,
}

#[derive(Debug)]
enum Kind {
    Clip(ClipState),
    Machine(MachineState),
    Blend {
        children: Vec<Option<Node>>,
        phase: f32,
    },
    Select {
        index: usize,
        child: Option<Box<Node>>,
    },
    Wrap {
        child: Option<Box<Node>>,
        modifier: Option<ModState>,
    },
    Switch {
        default: Option<Box<Node>>,
        children: Vec<Node>,
    },
    Empty,
}

#[derive(Debug, Clone)]
struct ClipState {
    animation: Arc<str>,
    /// Progress through the (cropped) clip, 0..length.
    t: f32,
    prev: f32,
    length: f32,
    offset: f32,
    reverse: bool,
    additive: bool,
    looping: bool,
    /// Held where `userControlledTimeFraction` puts it.
    user: bool,
    /// Turns round at either end.
    ping_pong: bool,
    wrapped: bool,
    started: bool,
}

#[derive(Debug)]
struct MachineState {
    state: i32,
    child: Option<Box<Node>>,
    /// The state being left: its node, seconds into the blend, blend length.
    from: Option<(Box<Node>, f32, f32)>,
    /// Seconds since the state was entered.
    elapsed: f32,
    /// Graph-local events seen in this state, with the order they last came in
    /// (for event-bounded transition intervals).
    seen: Vec<(i32, u32)>,
    seq: u32,
    /// A transition triggered outside its initiate interval, waiting for it.
    pending: Option<Transition>,
    /// An uninterruptible transition's blend is playing.
    locked: bool,
}

impl MachineState {
    fn new(state: i32, child: Option<Box<Node>>) -> MachineState {
        MachineState {
            state,
            child,
            from: None,
            elapsed: 0.0,
            seen: Vec::new(),
            seq: 0,
            pending: None,
            locked: false,
        }
    }

    fn see(&mut self, event: i32) {
        self.seq += 1;
        match self.seen.iter_mut().find(|(e, _)| *e == event) {
            Some(s) => s.1 = self.seq,
            None => self.seen.push((event, self.seq)),
        }
    }

    /// Whether `i` is open now: by time in the state, or between its enter event
    /// (or the state's start, without one) and its exit event.
    fn within(&self, i: &Interval) -> bool {
        if i.timed() {
            return i.contains_time(self.elapsed);
        }
        let last = |e: i32| self.seen.iter().find(|(x, _)| *x == e).map(|s| s.1);
        let opened = if i.enter_event < 0 {
            Some(0)
        } else {
            last(i.enter_event)
        };
        opened.is_some_and(|o| last(i.exit_event).is_none_or(|x| x < o))
    }
}

/// What a state machine did with an event.
#[derive(PartialEq)]
enum Took {
    Nothing,
    /// Changed state.
    Transition,
    /// Triggered a transition that waits for its initiate interval.
    Pending,
}

#[derive(Debug)]
struct ModState {
    gi: usize,
    m: ModId,
    kind: ModKind,
}

#[derive(Debug)]
enum ModKind {
    List(Vec<ModState>),
    Timer {
        elapsed: f32,
        fired: bool,
    },
    EveryN {
        count: u32,
        target: u32,
    },
    Driven {
        active: bool,
        child: Option<Box<ModState>>,
    },
    Expressions {
        was_true: Vec<bool>,
    },
    Damping {
        damped: f32,
        error_sum: f32,
        previous: f32,
    },
    Plain,
}

/// A running behaviour graph.
pub struct Instance {
    shared: Arc<Shared>,
    values: Vec<f32>,
    root: Option<Node>,
    queue: VecDeque<Event>,
    raised: Vec<Raised>,
    rng: u64,
    /// Transitions taken, when tracing.
    trace: Option<Vec<String>>,
}

/// Passed down while activating / updating.
struct Ctx<'a> {
    shared: &'a Shared,
    values: &'a mut Vec<f32>,
    queue: &'a mut VecDeque<Event>,
    raised: &'a mut Vec<Raised>,
    rng: &'a mut u64,
    clips: &'a mut dyn ClipSource,
    trace: Option<&'a mut Vec<String>>,
}

impl Ctx<'_> {
    fn var(&self, gi: usize, local: usize) -> f32 {
        self.shared.maps[gi]
            .vars
            .get(local)
            .and_then(|&v| self.values.get(v))
            .copied()
            .unwrap_or(0.0)
    }

    fn set_var(&mut self, gi: usize, local: usize, value: f32) {
        if let Some(&v) = self.shared.maps[gi].vars.get(local) {
            self.values[v] = value;
        }
    }

    fn bound(&self, gi: usize, g: GenId, member: &str) -> Option<f32> {
        self.shared
            .graph(gi)
            .bound(g, member)
            .map(|v| self.var(gi, v))
    }

    /// Value of the variable bound to `member` of modifier `m`.
    fn modifier_bound(&self, gi: usize, m: ModId, member: &str) -> Option<f32> {
        let v = self.shared.graph(gi).modifier_bindings[m]
            .iter()
            .find(|b| b.member == member)?
            .variable;
        Some(self.var(gi, v))
    }

    /// Write `value` to the variable bound to `member` of modifier `m`, if any.
    fn set_modifier_output(&mut self, gi: usize, m: ModId, member: &str, value: f32) {
        if let Some(v) = self.shared.graph(gi).modifier_bindings[m]
            .iter()
            .find(|b| b.member == member)
            .map(|b| b.variable)
        {
            self.set_var(gi, v, value);
        }
    }

    fn by_name(&self, name: &str) -> f32 {
        self.shared
            .variable_id(name)
            .map_or(0.0, |v| self.values[v])
    }

    fn condition(&self, c: &str) -> bool {
        match self.shared.conditions.get(c) {
            Some(Some(e)) => e.test(&|n| self.by_name(n)),
            // Unparsed conditions don't block.
            _ => true,
        }
    }

    /// Raise a graph event: handled by the graph next, and reported to the engine.
    fn raise(&mut self, gi: usize, local: i32, payload: Option<String>) {
        let Some(&id) = usize::try_from(local)
            .ok()
            .and_then(|l| self.shared.maps[gi].events.get(l))
        else {
            return;
        };
        self.raised.push(Raised {
            event: self.shared.event_names[id].clone(),
            payload,
        });
        self.queue.push_back(Event { id });
    }

    fn rand(&mut self) -> u64 {
        *self.rng ^= *self.rng << 13;
        *self.rng ^= *self.rng >> 7;
        *self.rng ^= *self.rng << 17;
        *self.rng
    }

    fn local_event(&self, gi: usize, global: usize) -> Option<i32> {
        self.shared.maps[gi]
            .events
            .iter()
            .position(|&e| e == global)
            .map(|i| i as i32)
    }

    // ------------------------------------------------------------ activation

    fn activate(&mut self, gi: usize, g: GenId, nested: &mut Option<i32>, depth: usize) -> Node {
        let graph = self.shared.graph(gi);
        let kind = if depth > 64 {
            Kind::Empty
        } else {
            match &graph.generators[g] {
                Generator::Clip {
                    animation,
                    mode,
                    speed,
                    crop_start,
                    crop_end,
                    start_time,
                    ..
                } => {
                    let speed = self.bound(gi, g, "playbackSpeed").unwrap_or(*speed);
                    let full = self.clips.duration(animation).unwrap_or(0.0);
                    let length = (full - crop_start - crop_end).max(0.0);
                    let start = self
                        .bound(gi, g, "startTime")
                        .unwrap_or(*start_time)
                        .clamp(0.0, length);
                    let additive = self.clips.additive(animation);
                    Kind::Clip(ClipState {
                        animation: animation.as_str().into(),
                        t: start,
                        prev: start,
                        length,
                        offset: *crop_start,
                        reverse: speed < 0.0,
                        additive,
                        looping: !matches!(mode, ClipMode::SinglePlay | ClipMode::UserControlled),
                        user: *mode == ClipMode::UserControlled,
                        ping_pong: *mode == ClipMode::PingPong,
                        wrapped: false,
                        started: false,
                    })
                }
                Generator::StateMachine {
                    start,
                    start_variable,
                    start_mode,
                    states,
                    ..
                } => {
                    // A bound start state takes the variable's value (`i1stPerson`...).
                    let bound = start_variable.map(|v| self.var(gi, v) as i32);
                    let by_mode = match *start_mode {
                        StartMode::Sync(v) => Some(self.var(gi, v) as i32),
                        StartMode::Random if !states.is_empty() => {
                            Some(states[(self.rand() % states.len() as u64) as usize].id)
                        }
                        _ => None,
                    };
                    let want = nested.take().or(bound).or(by_mode).unwrap_or(*start);
                    match states.iter().find(|s| s.id == want).or(states.first()) {
                        Some(s) => {
                            let (id, generator) = (s.id, s.generator);
                            for e in s.enter_events.clone() {
                                self.raise(gi, e.event, e.payload);
                            }
                            let child = generator
                                .map(|c| Box::new(self.activate(gi, c, nested, depth + 1)));
                            Kind::Machine(MachineState::new(id, child))
                        }
                        None => Kind::Empty,
                    }
                }
                Generator::Blender { children, .. } => {
                    let gens: Vec<Option<GenId>> = children.iter().map(|c| c.generator).collect();
                    // A nested target applies down the first (dominant) child only.
                    let mut first = true;
                    let children = gens
                        .into_iter()
                        .map(|c| {
                            let mut none = None;
                            let n = if std::mem::take(&mut first) {
                                &mut *nested
                            } else {
                                &mut none
                            };
                            c.map(|c| self.activate(gi, c, n, depth + 1))
                        })
                        .collect();
                    Kind::Blend {
                        children,
                        phase: 0.0,
                    }
                }
                Generator::Selector {
                    children, index, ..
                } => {
                    let i = self
                        .bound(gi, g, "selectedGeneratorIndex")
                        .map_or(*index as i32, |v| v as i32);
                    let i = (i.max(0) as usize).min(children.len().saturating_sub(1));
                    let child = children
                        .get(i)
                        .copied()
                        .map(|c| Box::new(self.activate(gi, c, nested, depth + 1)));
                    Kind::Select { index: i, child }
                }
                Generator::Wrap {
                    child, modifier, ..
                } => {
                    let (child, modifier) = (*child, *modifier);
                    let modifier = modifier.map(|m| self.activate_modifier(gi, m, 0));
                    Kind::Wrap {
                        child: child.map(|c| Box::new(self.activate(gi, c, nested, depth + 1))),
                        modifier,
                    }
                }
                Generator::BoneSwitch {
                    default, children, ..
                } => {
                    let kids: Vec<GenId> = children.iter().map(|c| c.0).collect();
                    let default = *default;
                    Kind::Switch {
                        default: default.map(|c| Box::new(self.activate(gi, c, nested, depth + 1))),
                        children: kids
                            .into_iter()
                            .map(|c| self.activate(gi, c, &mut None, depth + 1))
                            .collect(),
                    }
                }
                Generator::Reference { behavior, .. } => {
                    let target = self
                        .shared
                        .project
                        .graph_index(behavior)
                        .and_then(|o| Some((o, self.shared.graph(o).root?)));
                    match target {
                        Some((o, root)) => {
                            let n = self.activate(o, root, nested, depth + 1);
                            Kind::Wrap {
                                child: Some(Box::new(n)),
                                modifier: None,
                            }
                        }
                        None => Kind::Empty,
                    }
                }
                Generator::Other(_) => Kind::Empty,
            }
        };
        Node { gi, g, kind }
    }

    fn activate_modifier(&mut self, gi: usize, m: ModId, depth: usize) -> ModState {
        let graph = self.shared.graph(gi);
        let kind = match &graph.modifiers[m] {
            _ if depth > 16 => ModKind::Plain,
            Modifier::List(list) => {
                let list = list.clone();
                ModKind::List(
                    list.into_iter()
                        .map(|c| self.activate_modifier(gi, c, depth + 1))
                        .collect(),
                )
            }
            Modifier::Timer { .. } => ModKind::Timer {
                elapsed: 0.0,
                fired: false,
            },
            Modifier::EveryN { n, min, random, .. } => {
                let (n, min, random) = (*n, *min, *random);
                ModKind::EveryN {
                    count: 0,
                    target: self.every_n_target(n, min, random),
                }
            }
            Modifier::EventDriven {
                child,
                active_by_default,
                ..
            } => {
                let (child, active) = (*child, *active_by_default);
                ModKind::Driven {
                    active,
                    child: child.map(|c| Box::new(self.activate_modifier(gi, c, depth + 1))),
                }
            }
            Modifier::Expressions(lines) => ModKind::Expressions {
                was_true: vec![false; lines.len()],
            },
            Modifier::IsActive { .. } => {
                self.is_active_outputs(gi, m, true);
                ModKind::Plain
            }
            Modifier::Damping { damped, .. } => {
                // Picks up from the bound output (a re-entered state carries on smoothly).
                let damped = self.modifier_bound(gi, m, "dampedValue").unwrap_or(*damped);
                ModKind::Damping {
                    damped,
                    error_sum: 0.0,
                    previous: 0.0,
                }
            }
            _ => ModKind::Plain,
        };
        ModState { gi, m, kind }
    }

    fn every_n_target(&mut self, n: u8, min: u8, random: bool) -> u32 {
        let (lo, hi) = (min.max(1).min(n.max(1)) as u32, n.max(1) as u32);
        if random && hi > lo {
            lo + (self.rand() % (hi - lo + 1) as u64) as u32
        } else {
            hi
        }
    }

    fn is_active_outputs(&mut self, gi: usize, m: ModId, active: bool) {
        let graph = self.shared.graph(gi);
        let Modifier::IsActive { invert } = &graph.modifiers[m] else {
            return;
        };
        let invert = *invert;
        let outs: Vec<(usize, usize)> = graph.modifier_bindings[m]
            .iter()
            .filter_map(|b| {
                Some((
                    b.member.strip_prefix("bIsActive")?.parse::<usize>().ok()?,
                    b.variable,
                ))
            })
            .collect();
        for (i, v) in outs {
            let on = active != invert.get(i).copied().unwrap_or(false);
            self.set_var(gi, v, if on { 1.0 } else { 0.0 });
        }
    }

    // ---------------------------------------------------------- deactivation

    fn deactivate(&mut self, node: Node) {
        match node.kind {
            Kind::Machine(m) => {
                if let Some(c) = m.child {
                    self.deactivate(*c);
                }
                if let Some((f, _, _)) = m.from {
                    self.deactivate(*f);
                }
            }
            Kind::Blend { children, .. } => children
                .into_iter()
                .flatten()
                .for_each(|c| self.deactivate(c)),
            Kind::Select { child, .. } => {
                if let Some(c) = child {
                    self.deactivate(*c);
                }
            }
            Kind::Wrap { child, modifier } => {
                if let Some(m) = modifier {
                    self.deactivate_modifier(m);
                }
                if let Some(c) = child {
                    self.deactivate(*c);
                }
            }
            Kind::Switch { default, children } => {
                if let Some(c) = default {
                    self.deactivate(*c);
                }
                children.into_iter().for_each(|c| self.deactivate(c));
            }
            Kind::Clip(_) | Kind::Empty => {}
        }
    }

    fn deactivate_modifier(&mut self, m: ModState) {
        match &self.shared.graph(m.gi).modifiers[m.m] {
            Modifier::OnDeactivate(e) => {
                let e = e.clone();
                self.raise(m.gi, e.event, e.payload);
            }
            Modifier::IsActive { .. } => self.is_active_outputs(m.gi, m.m, false),
            _ => {}
        }
        match m.kind {
            ModKind::List(list) => list.into_iter().for_each(|c| self.deactivate_modifier(c)),
            ModKind::Driven { child: Some(c), .. } => self.deactivate_modifier(*c),
            _ => {}
        }
    }

    // --------------------------------------------------------------- events

    /// Offer `ev` to the active tree, outer state machines first. True when some
    /// state machine changed state.
    fn handle(&mut self, node: &mut Node, ev: &Event) -> bool {
        let gi = node.gi;
        match &mut node.kind {
            Kind::Machine(m) => {
                if let Some(local) = self.local_event(gi, ev.id) {
                    m.see(local);
                }
                // The event may have opened a pending transition's window.
                if self.take_pending(node) {
                    return true;
                }
                let took = self.try_transition(node, Some(ev.id));
                if took == Took::Transition {
                    return true;
                }
                let Kind::Machine(m) = &mut node.kind else {
                    unreachable!()
                };
                m.child.as_mut().is_some_and(|c| self.handle(c, ev)) || took == Took::Pending
            }
            Kind::Blend { children, .. } => children
                .iter_mut()
                .flatten()
                .fold(false, |acc, c| self.handle(c, ev) | acc),
            Kind::Select { child, .. } => child.as_mut().is_some_and(|c| self.handle(c, ev)),
            Kind::Wrap { child, modifier } => {
                if let Some(m) = modifier {
                    self.modifier_event(m, ev);
                }
                child.as_mut().is_some_and(|c| self.handle(c, ev))
            }
            Kind::Switch { default, children } => {
                let mut any = default.as_mut().is_some_and(|c| self.handle(c, ev));
                for c in children {
                    any |= self.handle(c, ev);
                }
                any
            }
            Kind::Clip(_) | Kind::Empty => {
                let _ = gi;
                false
            }
        }
    }

    fn modifier_event(&mut self, m: &mut ModState, ev: &Event) {
        let gi = m.gi;
        let local = self.local_event(gi, ev.id);
        match (&mut m.kind, &self.shared.graph(gi).modifiers[m.m]) {
            (ModKind::List(list), _) => list.iter_mut().for_each(|c| self.modifier_event(c, ev)),
            (
                ModKind::EveryN { count, target },
                Modifier::EveryN {
                    check,
                    send,
                    n,
                    min,
                    random,
                },
            ) if local == Some(*check) => {
                *count += 1;
                if *count >= *target {
                    let (send, n, min, random) = (send.clone(), *n, *min, *random);
                    *count = 0;
                    *target = self.every_n_target(n, min, random);
                    self.raise(gi, send.event, send.payload);
                }
            }
            (
                ModKind::Driven { active, child },
                Modifier::EventDriven {
                    activate,
                    deactivate,
                    ..
                },
            ) => {
                if local == Some(*activate) {
                    *active = true;
                } else if local == Some(*deactivate) {
                    *active = false;
                }
                if *active && let Some(c) = child {
                    self.modifier_event(c, ev);
                }
            }
            _ => {}
        }
    }

    /// Take the machine's pending transition if its initiate interval has opened.
    fn take_pending(&mut self, node: &mut Node) -> bool {
        let Kind::Machine(m) = &mut node.kind else {
            return false;
        };
        match &m.pending {
            Some(t) if !m.locked && t.initiate.as_ref().is_none_or(|i| m.within(i)) => {
                let t = m.pending.take().unwrap();
                self.transition(node, &t);
                true
            }
            _ => false,
        }
    }

    /// Take a transition of the state machine at `node` on `event` (or, for `None`,
    /// a condition-only transition whose condition holds).
    fn try_transition(&mut self, node: &mut Node, event: Option<usize>) -> Took {
        let (gi, sm) = (node.gi, node.g);
        let Kind::Machine(m) = &node.kind else {
            return Took::Nothing;
        };
        if m.locked {
            return Took::Nothing;
        }
        let graph = self.shared.graph(gi);
        let Generator::StateMachine {
            states, wildcards, ..
        } = &graph.generators[sm]
        else {
            return Took::Nothing;
        };
        let local = match event {
            Some(e) => match self.local_event(gi, e) {
                Some(l) => l,
                None => return Took::Nothing,
            },
            None => -1,
        };
        let current = m.state;
        let here = states.iter().find(|s| s.id == current);
        let promoted = self
            .shared
            .promoted
            .get(&(gi, sm))
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let candidates = here
            .into_iter()
            .flat_map(|s| s.transitions.iter().map(|t| (t, false)))
            .chain(wildcards.iter().map(|t| (t, true)))
            // A nested machine already running handles its own wildcards.
            .chain(
                promoted
                    .iter()
                    .filter(|(s, _)| *s != current)
                    .map(|(_, t)| (t, true)),
            );
        let mut chosen: Option<Transition> = None;
        for (t, wildcard) in candidates {
            if t.event != local || t.disabled() {
                continue;
            }
            // Wildcards don't re-enter the current state unless allowed to.
            if wildcard && t.to_state == current && !t.allows_self() && t.to_nested.is_none() {
                continue;
            }
            if event.is_none() && t.condition.is_none() {
                continue;
            }
            if let Some(c) = &t.condition
                && (event.is_none() || t.flags & 0x100 == 0)
                && !self.condition(c)
            {
                continue;
            }
            if !states.iter().any(|s| s.id == t.to_state) {
                continue;
            }
            // Events outside the trigger interval are ignored.
            if t.trigger.as_ref().is_some_and(|i| !m.within(i)) {
                continue;
            }
            // Condition transitions wait by not being taken yet.
            if event.is_none() && t.initiate.as_ref().is_some_and(|i| !m.within(i)) {
                continue;
            }
            chosen = Some(t.clone());
            break;
        }
        let Some(t) = chosen else {
            return Took::Nothing;
        };
        if t.initiate.as_ref().is_some_and(|i| !m.within(i)) {
            let Kind::Machine(m) = &mut node.kind else {
                unreachable!()
            };
            m.pending = Some(t);
            return Took::Pending;
        }
        self.transition(node, &t);
        Took::Transition
    }

    fn transition(&mut self, node: &mut Node, t: &Transition) {
        let (gi, sm) = (node.gi, node.g);
        let graph = self.shared.graph(gi);
        let Generator::StateMachine { states, .. } = &graph.generators[sm] else {
            return;
        };
        let Kind::Machine(m) = &mut node.kind else {
            return;
        };
        let old = states.iter().find(|s| s.id == m.state);
        let new = states.iter().find(|s| s.id == t.to_state).expect("checked");
        let exits = old.map(|s| s.exit_events.clone()).unwrap_or_default();
        let (enters, generator, id) = (new.enter_events.clone(), new.generator, new.id);
        let blend = t
            .blend_variable
            .map(|v| self.var(gi, v))
            .or(t.blend)
            .unwrap_or(0.0);
        let old_child = m.child.take();
        let old_from = m.from.take();
        for e in exits {
            self.raise(gi, e.event, e.payload);
        }
        for e in enters {
            self.raise(gi, e.event, e.payload);
        }
        if let Some(trace) = self.trace.as_deref_mut() {
            let name = |id: i32| {
                states
                    .iter()
                    .find(|s| s.id == id)
                    .map_or("?", |s| s.name.as_str())
            };
            let why = match graph.event_name(t.event) {
                Some(e) => e.to_owned(),
                None => format!("if {:?}", t.condition.as_deref().unwrap_or("")),
            };
            trace.push(format!(
                "{}: {} -> {} ({why}, blend {blend})",
                graph.generators[sm].name(),
                old.map_or("?", |s| s.name.as_str()),
                name(id)
            ));
        }
        let mut nested = t.to_nested;
        let child = generator.map(|c| Box::new(self.activate(gi, c, &mut nested, 1)));
        if let Some((f, _, _)) = old_from {
            self.deactivate(*f);
        }
        let from = match old_child {
            Some(c) if blend > 0.0 => Some((c, 0.0, blend)),
            Some(c) => {
                self.deactivate(*c);
                None
            }
            None => None,
        };
        let locked = t.uninterruptible() && from.is_some();
        let Kind::Machine(m) = &mut node.kind else {
            return;
        };
        *m = MachineState::new(id, child);
        m.from = from;
        m.locked = locked;
        if let Generator::StateMachine {
            start_mode: StartMode::Sync(v),
            ..
        } = &graph.generators[sm]
        {
            self.set_var(gi, *v, id as f32);
        }
    }

    // ---------------------------------------------------------------- update

    fn advance(&mut self, node: &mut Node, dt: f32) {
        let (gi, g) = (node.gi, node.g);
        match &mut node.kind {
            Kind::Clip(c) => self.advance_clip(gi, g, c, dt),
            Kind::Machine(m) => {
                m.elapsed += dt;
                if let Some((f, t, len)) = &mut m.from {
                    *t += dt;
                    if *t >= *len {
                        let (f, _, _) = m.from.take().unwrap();
                        m.locked = false;
                        self.deactivate(*f);
                    } else {
                        self.advance(f, dt);
                    }
                }
                let Kind::Machine(m) = &mut node.kind else {
                    unreachable!()
                };
                if let Some(c) = &mut m.child {
                    self.advance(c, dt);
                }
                if !self.take_pending(node) {
                    self.try_transition(node, None);
                }
            }
            Kind::Blend { children, phase } => {
                let graph = self.shared.graph(gi);
                let Generator::Blender { flags, .. } = &graph.generators[g] else {
                    return;
                };
                if flags & BLEND_SYNC != 0 {
                    // Synchronised cyclic children share a phase; the cycle length is
                    // the weighted mix of theirs.
                    let weights = self.blend_weights(gi, g, &[]);
                    let lengths: Vec<f32> = children
                        .iter()
                        .map(|c| c.as_ref().map_or(0.0, |c| self.cycle_length(c)))
                        .collect();
                    let total: f32 = weights.iter().zip(&lengths).map(|(w, l)| w * l).sum();
                    if total > 1e-4 {
                        *phase += dt / total;
                        // Single-play children (attacks) end rather than cycle.
                        let looping = children.iter().flatten().any(|c| self.loops(c));
                        let wrapped = looping && *phase >= 1.0;
                        *phase = if looping {
                            *phase % 1.0
                        } else {
                            phase.min(1.0)
                        };
                        let p = *phase;
                        // The heaviest child raises the blend's triggers (as Havok's
                        // sync master), the others just follow its phase.
                        let master = weights
                            .iter()
                            .enumerate()
                            .max_by(|a, b| a.1.total_cmp(b.1))
                            .map(|(i, _)| i);
                        for (i, c) in children.iter_mut().enumerate() {
                            if let Some(c) = c {
                                self.set_phase(c, p, wrapped, Some(i) == master);
                            }
                        }
                        return;
                    }
                }
                for c in children.iter_mut().flatten() {
                    self.advance(c, dt);
                }
            }
            Kind::Select { index, child } => {
                let graph = self.shared.graph(gi);
                let Generator::Selector {
                    children,
                    index: fixed,
                    ..
                } = &graph.generators[g]
                else {
                    return;
                };
                let want = self
                    .bound(gi, g, "selectedGeneratorIndex")
                    .map_or(*fixed as i32, |v| v as i32);
                let want = (want.max(0) as usize).min(children.len().saturating_sub(1));
                if want != *index
                    && let Some(&c) = children.get(want)
                {
                    if let Some(trace) = self.trace.as_deref_mut() {
                        let var = graph
                            .bound(g, "selectedGeneratorIndex")
                            .and_then(|v| graph.variables.get(v));
                        trace.push(format!(
                            "{}: child {} -> {want} ({var:?})",
                            graph.generators[g].name(),
                            *index
                        ));
                    }
                    if let Some(old) = child.take() {
                        self.deactivate(*old);
                    }
                    *child = Some(Box::new(self.activate(gi, c, &mut None, 1)));
                    *index = want;
                }
                if let Some(c) = child {
                    self.advance(c, dt);
                }
            }
            Kind::Wrap { child, modifier } => {
                if let Some(m) = modifier {
                    self.advance_modifier(m, dt);
                }
                if let Some(c) = child {
                    self.advance(c, dt);
                }
            }
            Kind::Switch { default, children } => {
                if let Some(c) = default {
                    self.advance(c, dt);
                }
                for c in children {
                    self.advance(c, dt);
                }
            }
            Kind::Empty => {}
        }
    }

    fn advance_clip(&mut self, gi: usize, g: GenId, c: &mut ClipState, dt: f32) {
        let graph = self.shared.graph(gi);
        let Generator::Clip {
            speed,
            user_fraction,
            ..
        } = &graph.generators[g]
        else {
            return;
        };
        let speed = self.bound(gi, g, "playbackSpeed").unwrap_or(*speed).abs();
        let first = !c.started;
        c.started = true;
        c.prev = c.t;
        c.wrapped = false;
        let mut t = c.t + dt * speed;
        if c.length <= 0.0 {
            t = 0.0;
        } else if c.user {
            let f = self
                .bound(gi, g, "userControlledTimeFraction")
                .unwrap_or(*user_fraction);
            t = f.clamp(0.0, 1.0) * c.length;
        } else if c.ping_pong && t >= c.length {
            // Back the other way from the end.
            t = (t - c.length) % c.length;
            c.reverse = !c.reverse;
            c.wrapped = true;
        } else if c.looping && t >= c.length {
            t %= c.length;
            c.wrapped = true;
        } else {
            t = t.min(c.length);
        }
        c.t = t;
        self.clip_triggers(gi, g, c, first);
    }

    /// Raise a clip's triggers between its previous and current time (a trigger at 0
    /// fires on start).
    fn clip_triggers(&mut self, gi: usize, g: GenId, c: &ClipState, first: bool) {
        let graph = self.shared.graph(gi);
        let Generator::Clip { triggers, .. } = &graph.generators[g] else {
            return;
        };
        let mut due = Vec::new();
        for tr in triggers {
            // Trigger times are clip-local; played backwards, local time runs down
            // from the end (a trigger at 0 fires as the clip finishes).
            let local = if tr.from_end {
                c.length + tr.time
            } else {
                tr.time
            }
            .clamp(0.0, c.length);
            let at = if c.reverse { c.length - local } else { local };
            let hit = if c.wrapped {
                at > c.prev || at <= c.t
            } else {
                (at > c.prev || (first && at <= c.prev)) && at <= c.t
            };
            if hit {
                due.push((tr.event, tr.payload.clone()));
            }
        }
        for (e, p) in due {
            self.raise(gi, e, p);
        }
    }

    /// Move a node in a synchronised blend to `phase`; `fire` raises the triggers
    /// its clips pass on the way.
    fn set_phase(&mut self, node: &mut Node, phase: f32, wrapped: bool, fire: bool) {
        let (gi, g) = (node.gi, node.g);
        match &mut node.kind {
            Kind::Clip(c) => {
                let first = !c.started;
                c.prev = c.t;
                c.t = phase * c.length;
                c.wrapped = wrapped;
                c.started = true;
                if fire {
                    let c = c.clone();
                    self.clip_triggers(gi, g, &c, first);
                }
            }
            Kind::Wrap {
                child: Some(c),
                modifier,
            } => {
                if let Some(m) = modifier {
                    self.advance_modifier(m, 0.0);
                }
                self.set_phase(c, phase, wrapped, fire);
            }
            Kind::Select { child: Some(c), .. } => self.set_phase(c, phase, wrapped, fire),
            // A blend inside a synchronised one (left / centre / right leans) follows its phase.
            Kind::Blend {
                children,
                phase: own,
            } => {
                *own = phase;
                let weights = self.blend_weights(gi, g, &[]);
                let master = weights
                    .iter()
                    .enumerate()
                    .max_by(|a, b| a.1.total_cmp(b.1))
                    .map(|(i, _)| i);
                for (i, c) in children.iter_mut().enumerate() {
                    if let Some(c) = c {
                        self.set_phase(c, phase, wrapped, fire && Some(i) == master);
                    }
                }
            }
            _ => self.advance(node, 0.0),
        }
    }

    /// Seconds a node takes to play one cycle: a clip's length over its playback
    /// speed, a blend's children mixed by weight.
    /// Whether any clip under `node` loops.
    fn loops(&self, node: &Node) -> bool {
        match &node.kind {
            Kind::Clip(c) => c.looping,
            Kind::Wrap { child: Some(c), .. } | Kind::Select { child: Some(c), .. } => {
                self.loops(c)
            }
            Kind::Blend { children, .. } => children.iter().flatten().any(|c| self.loops(c)),
            _ => false,
        }
    }

    fn cycle_length(&self, node: &Node) -> f32 {
        match &node.kind {
            Kind::Clip(c) => {
                let Generator::Clip { speed, .. } = &self.shared.graph(node.gi).generators[node.g]
                else {
                    return c.length;
                };
                let speed = self
                    .bound(node.gi, node.g, "playbackSpeed")
                    .unwrap_or(*speed)
                    .abs();
                if speed > 1e-3 { c.length / speed } else { 0.0 }
            }
            Kind::Wrap { child: Some(c), .. } | Kind::Select { child: Some(c), .. } => {
                self.cycle_length(c)
            }
            Kind::Blend { children, .. } => {
                let weights = self.blend_weights(node.gi, node.g, &[]);
                children
                    .iter()
                    .zip(&weights)
                    .map(|(c, w)| c.as_ref().map_or(0.0, |c| self.cycle_length(c)) * w)
                    .sum()
            }
            _ => 0.0,
        }
    }

    fn advance_modifier(&mut self, m: &mut ModState, dt: f32) {
        let gi = m.gi;
        let graph = self.shared.graph(gi);
        if let Some(v) = graph.modifier_bindings[m.m]
            .iter()
            .find(|b| b.member == "enable")
            .map(|b| b.variable)
            && self.var(gi, v) == 0.0
        {
            return;
        }
        match (&mut m.kind, &graph.modifiers[m.m]) {
            (ModKind::List(list), _) => list.iter_mut().for_each(|c| self.advance_modifier(c, dt)),
            (ModKind::Timer { elapsed, fired }, Modifier::Timer { seconds, alarm }) => {
                *elapsed += dt;
                if !*fired && *elapsed >= *seconds {
                    *fired = true;
                    let alarm = alarm.clone();
                    self.raise(gi, alarm.event, alarm.payload);
                }
            }
            (
                ModKind::Driven {
                    active: true,
                    child: Some(c),
                },
                _,
            ) => self.advance_modifier(c, dt),
            (_, Modifier::SpeedSampler { goal_speed }) => {
                let goal = self
                    .modifier_bound(gi, m.m, "goalSpeed")
                    .unwrap_or(*goal_speed);
                self.set_modifier_output(gi, m.m, "speedOut", goal);
            }
            (
                ModKind::Damping {
                    damped,
                    error_sum,
                    previous,
                },
                Modifier::Damping {
                    kp, ki, kd, raw, ..
                },
            ) => {
                // Havok's step is per frame at 30 Hz: take as many as the update covers.
                let raw = self.modifier_bound(gi, m.m, "rawValue").unwrap_or(*raw);
                let steps = (dt * 30.0)
                    .round()
                    .clamp(if dt > 0.0 { 1.0 } else { 0.0 }, 8.0)
                    as usize;
                for _ in 0..steps {
                    let error = raw - *damped;
                    *error_sum += error;
                    *damped += kp * error + ki * *error_sum + kd * (error - *previous);
                    *previous = error;
                }
                let d = *damped;
                self.set_modifier_output(gi, m.m, "dampedValue", d);
            }
            (ModKind::Expressions { was_true }, Modifier::Expressions(lines)) => {
                for (i, line) in lines.iter().enumerate() {
                    let Some(Some(st)) = self.shared.statements.get(line) else {
                        continue;
                    };
                    match st {
                        Statement::Assign(var, e) => {
                            let v = e.eval(&|n| self.by_name(n));
                            if let Some(id) = self.shared.variable_id(var) {
                                self.values[id] = v;
                            }
                        }
                        Statement::Raise(event, cond) => {
                            let now = cond.test(&|n| self.by_name(n));
                            if now
                                && !was_true[i]
                                && let Some(id) = self.shared.event_id(event)
                            {
                                self.raised.push(Raised {
                                    event: self.shared.event_names[id].clone(),
                                    payload: None,
                                });
                                self.queue.push_back(Event { id });
                            }
                            was_true[i] = now;
                        }
                        Statement::Test(_) => {}
                    }
                }
            }
            _ => {}
        }
    }

    /// Normalised weights of a blender's children. Children flagged in `additive`
    /// keep their own weight (0..1) and don't take part in the normalisation.
    fn blend_weights(&self, gi: usize, g: GenId, additive: &[bool]) -> Vec<f32> {
        let graph = self.shared.graph(gi);
        let Generator::Blender {
            parameter,
            min_cyclic,
            max_cyclic,
            flags,
            children,
            ..
        } = &graph.generators[g]
        else {
            return Vec::new();
        };
        let raw: Vec<f32> = children
            .iter()
            .map(|c| c.weight_variable.map_or(c.weight, |v| self.var(gi, v)))
            .collect();
        let mut w = vec![0.0; children.len()];
        if flags & BLEND_PARAMETRIC != 0 && !children.is_empty() {
            // Children sit at their weights along the blend parameter; blend the two
            // around it (wrapping for cyclic blends such as direction).
            let mut p = self.bound(gi, g, "blendParameter").unwrap_or(*parameter);
            if flags & BLEND_CYCLIC != 0 && max_cyclic > min_cyclic {
                p = min_cyclic + (p - min_cyclic).rem_euclid(max_cyclic - min_cyclic);
            }
            let mut order: Vec<usize> = (0..raw.len()).collect();
            order.sort_by(|&a, &b| raw[a].total_cmp(&raw[b]));
            let first = order[0];
            let last = *order.last().unwrap();
            if p <= raw[first] {
                if flags & BLEND_CYCLIC != 0 && max_cyclic > min_cyclic && order.len() > 1 {
                    let span = raw[first] + (max_cyclic - min_cyclic) - raw[last];
                    let f = if span > 0.0 {
                        (p + (max_cyclic - min_cyclic) - raw[last]) / span
                    } else {
                        1.0
                    };
                    w[last] = 1.0 - f;
                    w[first] = f;
                } else {
                    w[first] = 1.0;
                }
            } else if p >= raw[last] {
                if flags & BLEND_CYCLIC != 0 && max_cyclic > min_cyclic && order.len() > 1 {
                    let span = raw[first] + (max_cyclic - min_cyclic) - raw[last];
                    let f = if span > 0.0 {
                        (p - raw[last]) / span
                    } else {
                        0.0
                    };
                    w[last] = 1.0 - f;
                    w[first] = f;
                } else {
                    w[last] = 1.0;
                }
            } else {
                for pair in order.windows(2) {
                    let (a, b) = (pair[0], pair[1]);
                    if p >= raw[a] && p <= raw[b] {
                        let f = if raw[b] > raw[a] {
                            (p - raw[a]) / (raw[b] - raw[a])
                        } else {
                            0.0
                        };
                        w[a] = 1.0 - f;
                        w[b] = f;
                        break;
                    }
                }
            }
        } else {
            let add = |i: usize| additive.get(i).copied().unwrap_or(false);
            let total: f32 = raw
                .iter()
                .enumerate()
                .filter(|(i, _)| !add(*i))
                .map(|(_, x)| x.max(0.0))
                .sum();
            for (i, (o, r)) in w.iter_mut().zip(&raw).enumerate() {
                *o = if add(i) {
                    r.clamp(0.0, 1.0)
                } else if total > 1e-6 {
                    r.max(0.0) / total
                } else {
                    0.0
                };
            }
            if total <= 1e-6
                && let Some(i) = (0..w.len()).find(|&i| !add(i))
            {
                w[i] = 1.0;
            }
        }
        w
    }

    // ---------------------------------------------------------------- output

    fn collect(&self, node: &Node, weight: f32, mask: Option<&Arc<[f32]>>, out: &mut Vec<Sample>) {
        if weight <= 1e-4 {
            return;
        }
        let (gi, g) = (node.gi, node.g);
        match &node.kind {
            Kind::Clip(c) => {
                let file_time = |t: f32| {
                    if c.reverse {
                        c.offset + c.length - t
                    } else {
                        c.offset + t
                    }
                };
                out.push(Sample {
                    graph: gi,
                    generator: g,
                    animation: c.animation.clone(),
                    time: file_time(c.t),
                    prev_time: file_time(c.prev),
                    wrapped: c.wrapped,
                    weight,
                    additive: c.additive,
                    mask: mask.cloned(),
                });
            }
            Kind::Machine(m) => {
                let f = match &m.from {
                    Some((from, t, len)) => {
                        let x = (t / len).clamp(0.0, 1.0);
                        let f = x * x * (3.0 - 2.0 * x);
                        self.collect(from, weight * (1.0 - f), mask, out);
                        f
                    }
                    None => 1.0,
                };
                if let Some(c) = &m.child {
                    self.collect(c, weight * f, mask, out);
                }
            }
            Kind::Blend { children, .. } => {
                let graph = self.shared.graph(gi);
                let Generator::Blender { children: defs, .. } = &graph.generators[g] else {
                    return;
                };
                let additive: Vec<bool> = children
                    .iter()
                    .map(|c| c.as_ref().is_some_and(is_additive))
                    .collect();
                let w = self.blend_weights(gi, g, &additive);
                for ((c, wi), def) in children.iter().zip(w).zip(defs) {
                    let Some(c) = c else { continue };
                    match &def.bone_weights {
                        Some(bw) if !bw.is_empty() => {
                            let m = combine(mask, bw);
                            self.collect(c, weight * wi, Some(&m), out);
                        }
                        _ => self.collect(c, weight * wi, mask, out),
                    }
                }
            }
            Kind::Select { child, .. } => {
                if let Some(c) = child {
                    self.collect(c, weight, mask, out);
                }
            }
            Kind::Wrap { child, .. } => {
                if let Some(c) = child {
                    self.collect(c, weight, mask, out);
                }
            }
            Kind::Switch { default, children } => {
                let Some(weights) = self.shared.switch_weights.get(&(gi, g)) else {
                    return;
                };
                if let Some(d) = default {
                    match self.shared.default_masks.get(&(gi, g)) {
                        Some(dm) if !dm.is_empty() => {
                            let m = combine(mask, dm);
                            self.collect(d, weight, Some(&m), out);
                        }
                        _ => self.collect(d, weight, mask, out),
                    }
                }
                for (c, bw) in children.iter().zip(weights) {
                    let m = combine(mask, bw);
                    self.collect(c, weight, Some(&m), out);
                }
            }
            Kind::Empty => {}
        }
    }
}

fn combine(outer: Option<&Arc<[f32]>>, inner: &[f32]) -> Arc<[f32]> {
    match outer {
        None => inner.into(),
        Some(o) => (0..o.len().max(inner.len()))
            .map(|b| o.get(b).copied().unwrap_or(1.0) * inner.get(b).copied().unwrap_or(1.0))
            .collect(),
    }
}

/// A clip (or a wrapper around one) whose animation is additive.
fn is_additive(node: &Node) -> bool {
    match &node.kind {
        Kind::Clip(c) => c.additive,
        Kind::Wrap { child: Some(c), .. } | Kind::Select { child: Some(c), .. } => is_additive(c),
        _ => false,
    }
}

/// Cycle length of a synchronised blend child (its first clip's).
impl Instance {
    /// An instance of `shared`'s project. It starts at the root graph (the first
    /// loaded, `0_master`) on the first update or event, so that variables set
    /// before then (`i1stPerson`, `IsNPC`...) pick its start states.
    pub fn new(shared: Arc<Shared>, seed: u64) -> Instance {
        let values = shared.var_defaults.clone();
        Instance {
            shared,
            values,
            root: None,
            queue: VecDeque::new(),
            raised: Vec::new(),
            rng: seed | 1,
            trace: None,
        }
    }

    /// The active tree, activating the root graph the first time.
    fn take_root(&mut self, clips: &mut dyn ClipSource) -> Option<Node> {
        if self.root.is_none() {
            let r = self
                .shared
                .project
                .graphs
                .first()
                .and_then(|(_, g)| g.root)?;
            let mut ctx = self.ctx(clips);
            return Some(ctx.activate(0, r, &mut None, 0));
        }
        self.root.take()
    }

    fn ctx<'a>(&'a mut self, clips: &'a mut dyn ClipSource) -> Ctx<'a> {
        Ctx {
            shared: &self.shared,
            values: &mut self.values,
            queue: &mut self.queue,
            raised: &mut self.raised,
            rng: &mut self.rng,
            clips,
            trace: self.trace.as_mut(),
        }
    }

    /// Record transitions taken (see [`Instance::take_trace`]).
    pub fn set_tracing(&mut self, on: bool) {
        self.trace = on.then(Vec::new);
    }

    pub fn take_trace(&mut self) -> Vec<String> {
        self.trace.as_mut().map(std::mem::take).unwrap_or_default()
    }

    pub fn shared(&self) -> &Arc<Shared> {
        &self.shared
    }

    pub fn set_variable(&mut self, name: &str, value: f32) -> bool {
        match self.shared.variable_id(name) {
            Some(v) => {
                self.values[v] = value;
                true
            }
            None => false,
        }
    }

    pub fn variable(&self, name: &str) -> Option<f32> {
        self.shared.variable_id(name).map(|v| self.values[v])
    }

    /// Queue an event for the next update. False if no graph of the project knows it.
    pub fn send_event(&mut self, name: &str) -> bool {
        match self.shared.event_id(name) {
            Some(id) => {
                self.queue.push_back(Event { id });
                true
            }
            None => false,
        }
    }

    /// Handle an event now (with everything it sets off). True when it changed
    /// some state machine's state: the graph accepted it.
    pub fn handle_event(&mut self, name: &str, clips: &mut dyn ClipSource) -> bool {
        let Some(id) = self.shared.event_id(name) else {
            return false;
        };
        let Some(mut root) = self.take_root(clips) else {
            return false;
        };
        let accepted = {
            let mut ctx = self.ctx(clips);
            let r = ctx.handle(&mut root, &Event { id });
            ctx.drain(&mut root);
            r
        };
        self.root = Some(root);
        accepted
    }

    /// Advance by `dt` seconds: queued events, clips, triggers, modifiers,
    /// condition transitions.
    pub fn update(&mut self, dt: f32, clips: &mut dyn ClipSource) {
        let Some(mut root) = self.take_root(clips) else {
            return;
        };
        {
            let mut ctx = self.ctx(clips);
            ctx.drain(&mut root);
            ctx.advance(&mut root, dt);
            ctx.drain(&mut root);
        }
        self.root = Some(root);
    }

    /// Clips making up the pose now, with weights summing to 1 (per bone, where masked).
    pub fn samples(&self) -> Vec<Sample> {
        let mut out = Vec::new();
        if let Some(root) = &self.root {
            let ctx = Ctx {
                shared: &self.shared,
                values: &mut self.values.clone(),
                queue: &mut VecDeque::new(),
                raised: &mut Vec::new(),
                rng: &mut self.rng.clone(),
                clips: &mut |_: &str| None,
                trace: None,
            };
            ctx.collect(root, 1.0, None, &mut out);
        }
        out
    }

    /// Whether some state machine holds a transition waiting for its initiate
    /// interval (an exit finishing its swing first...).
    pub fn waiting(&self) -> bool {
        fn walk(n: &Node) -> bool {
            match &n.kind {
                Kind::Machine(m) => m.pending.is_some() || m.child.as_deref().is_some_and(walk),
                Kind::Blend { children, .. } => children.iter().flatten().any(walk),
                Kind::Select { child, .. } | Kind::Wrap { child, .. } => {
                    child.as_deref().is_some_and(walk)
                }
                Kind::Switch { default, children } => {
                    default.as_deref().is_some_and(walk) || children.iter().any(walk)
                }
                Kind::Clip(_) | Kind::Empty => false,
            }
        }
        self.root.as_ref().is_some_and(walk)
    }

    /// Events raised since the last call.
    pub fn take_raised(&mut self) -> Vec<Raised> {
        std::mem::take(&mut self.raised)
    }

    /// Names of the active states, outermost first (for debugging).
    /// Look-at modifiers running now (enabled), with their bound members resolved:
    /// target on / off, per-bone `enabled` and gains.
    pub fn look_ats(&self) -> Vec<LookAt> {
        fn mods(shared: &Shared, m: &ModState, values: &[f32], out: &mut Vec<(usize, ModId)>) {
            let enabled = shared.graph(m.gi).modifier_bindings[m.m]
                .iter()
                .find(|b| b.member == "enable")
                .is_none_or(|b| {
                    shared.maps[m.gi]
                        .vars
                        .get(b.variable)
                        .and_then(|&v| values.get(v))
                        .is_some_and(|v| *v != 0.0)
                });
            if !enabled {
                return;
            }
            match &m.kind {
                ModKind::List(l) => l.iter().for_each(|c| mods(shared, c, values, out)),
                ModKind::Driven {
                    active: true,
                    child: Some(c),
                } => mods(shared, c, values, out),
                _ => out.push((m.gi, m.m)),
            }
        }
        fn walk(shared: &Shared, n: &Node, values: &[f32], out: &mut Vec<(usize, ModId)>) {
            match &n.kind {
                Kind::Machine(m) => {
                    if let Some(c) = &m.child {
                        walk(shared, c, values, out);
                    }
                }
                Kind::Blend { children, .. } => children
                    .iter()
                    .flatten()
                    .for_each(|c| walk(shared, c, values, out)),
                Kind::Select { child: Some(c), .. } => walk(shared, c, values, out),
                Kind::Wrap { child, modifier } => {
                    if let Some(m) = modifier {
                        mods(shared, m, values, out);
                    }
                    if let Some(c) = child {
                        walk(shared, c, values, out);
                    }
                }
                Kind::Switch { default, children } => {
                    if let Some(d) = default {
                        walk(shared, d, values, out);
                    }
                    children.iter().for_each(|c| walk(shared, c, values, out));
                }
                _ => {}
            }
        }
        let mut active = Vec::new();
        if let Some(r) = &self.root {
            walk(&self.shared, r, &self.values, &mut active);
        }
        active
            .into_iter()
            .filter_map(|(gi, m)| {
                let graph = self.shared.graph(gi);
                let Modifier::LookAt(l) = &graph.modifiers[m] else {
                    return None;
                };
                let mut l = l.clone();
                for b in &graph.modifier_bindings[m] {
                    let Some(v) = self.shared.maps[gi]
                        .vars
                        .get(b.variable)
                        .and_then(|&v| self.values.get(v))
                        .copied()
                    else {
                        continue;
                    };
                    let member = b.member.as_str();
                    match member {
                        "lookAtTarget" => l.look_at_target = v != 0.0,
                        "onGain" => l.on_gain = v,
                        "offGain" => l.off_gain = v,
                        "limitAngleDegrees" => l.limit_degrees = v,
                        _ => {
                            // bones:N/field, eyeBones:N/field
                            let Some((list, rest)) = member.split_once(':') else {
                                continue;
                            };
                            let Some((i, field)) = rest.split_once('/') else {
                                continue;
                            };
                            let Ok(i) = i.parse::<usize>() else { continue };
                            let bones = if list == "eyeBones" {
                                &mut l.eye_bones
                            } else {
                                &mut l.bones
                            };
                            let Some(bone) = bones.get_mut(i) else {
                                continue;
                            };
                            match field {
                                "enabled" => bone.enabled = v != 0.0,
                                "onGain" => bone.on_gain = v,
                                "offGain" => bone.off_gain = v,
                                "limitAngleDegrees" => bone.limit_degrees = v,
                                _ => {}
                            }
                        }
                    }
                }
                Some(l)
            })
            .collect()
    }

    pub fn active_states(&self) -> Vec<String> {
        fn walk(shared: &Shared, n: &Node, out: &mut Vec<String>) {
            match &n.kind {
                Kind::Machine(m) => {
                    let graph = shared.graph(n.gi);
                    if let Generator::StateMachine { states, .. } = &graph.generators[n.g]
                        && let Some(s) = states.iter().find(|s| s.id == m.state)
                    {
                        out.push(s.name.clone());
                    }
                    if let Some(c) = &m.child {
                        walk(shared, c, out);
                    }
                }
                Kind::Blend { children, .. } => {
                    children.iter().flatten().for_each(|c| walk(shared, c, out))
                }
                Kind::Select { child: Some(c), .. } | Kind::Wrap { child: Some(c), .. } => {
                    walk(shared, c, out)
                }
                Kind::Switch { default, children } => {
                    if let Some(d) = default {
                        walk(shared, d, out);
                    }
                    children.iter().for_each(|c| walk(shared, c, out));
                }
                _ => {}
            }
        }
        let mut out = Vec::new();
        if let Some(r) = &self.root {
            walk(&self.shared, r, &mut out);
        }
        out
    }
}

impl Ctx<'_> {
    /// Handle queued events (and those they raise), up to a limit per update.
    fn drain(&mut self, root: &mut Node) {
        for _ in 0..64 {
            let Some(ev) = self.queue.pop_front() else {
                return;
            };
            self.handle(root, &ev);
        }
        self.queue.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(name: &str, mode: ClipMode, triggers: Vec<Trigger>) -> Generator {
        Generator::Clip {
            name: name.into(),
            animation: name.into(),
            mode,
            speed: 1.0,
            user_fraction: 0.0,
            triggers,
            crop_start: 0.0,
            crop_end: 0.0,
            start_time: 0.0,
        }
    }

    fn state(id: i32, g: GenId, transitions: Vec<Transition>) -> State {
        State {
            id,
            name: format!("s{id}"),
            generator: Some(g),
            transitions,
            enter_events: vec![],
            exit_events: vec![],
        }
    }

    fn durations(name: &str) -> Option<f32> {
        Some(match name {
            "Enter" => 1.0,
            "Loop" => 2.0,
            "Exit" => 0.5,
            _ => 1.0,
        })
    }

    /// Idle (0) -> Sit event -> Enter clip whose end trigger moves on to a Loop;
    /// Stand event blends (0.5 s) to an Exit clip, which returns to idle at its end.
    fn graph() -> BehaviorGraph {
        let events = ["Sit", "Next", "Stand", "Done", "Draw"]
            .map(String::from)
            .to_vec();
        let next = Trigger {
            time: 0.0,
            from_end: true,
            event: 1,
            payload: None,
        };
        let done = Trigger {
            time: 0.0,
            from_end: true,
            event: 3,
            payload: None,
        };
        let draw = Trigger {
            time: 0.5,
            from_end: false,
            event: 4,
            payload: Some("Tankard".into()),
        };
        let mut stand = Transition::new(2, 3, None);
        stand.blend = Some(0.5);
        let gens = vec![
            Generator::StateMachine {
                name: "Root".into(),
                start: 0,
                start_variable: None,
                start_mode: Default::default(),
                states: vec![
                    state(0, 1, vec![Transition::new(0, 1, None)]),
                    state(1, 2, vec![Transition::new(1, 2, None)]),
                    state(2, 3, vec![stand]),
                    state(3, 4, vec![Transition::new(3, 0, None)]),
                ],
                wildcards: vec![],
            },
            clip("Idle", ClipMode::Looping, vec![]),
            clip("Enter", ClipMode::SinglePlay, vec![next, draw]),
            clip("Loop", ClipMode::Looping, vec![]),
            clip("Exit", ClipMode::SinglePlay, vec![done]),
        ];
        // States map to generators 1..=4 via their index + 1.
        let mut g = BehaviorGraph::new("G", Some(0), gens, events);
        if let Generator::StateMachine { states, .. } = &mut g.generators[0] {
            for (i, s) in states.iter_mut().enumerate() {
                s.generator = Some(i + 1);
            }
        }
        g
    }

    fn instance(g: BehaviorGraph) -> Instance {
        let shared = Shared::new(Arc::new(Project {
            graphs: vec![("g".into(), g)],
            character: None,
        }));
        let mut i = Instance::new(shared, 1);
        i.update(0.0, &mut durations);
        i
    }

    fn playing(i: &Instance) -> Vec<(String, f32)> {
        i.samples()
            .iter()
            .map(|s| (s.animation.to_string(), (s.weight * 100.0).round() / 100.0))
            .collect()
    }

    /// A graph of one clip in `mode`, 2 s long.
    fn one_clip(mode: ClipMode, user_fraction: f32) -> Instance {
        let mut c = clip("Loop", mode, vec![]);
        if let Generator::Clip {
            user_fraction: f, ..
        } = &mut c
        {
            *f = user_fraction;
        }
        instance(BehaviorGraph::new("G", Some(0), vec![c], vec![]))
    }

    fn time(i: &Instance) -> f32 {
        (i.samples()[0].time * 100.0).round() / 100.0
    }

    #[test]
    fn user_controlled_clips_hold_their_fraction() {
        // A sarcophagus lid rests on its trigger clip's first frame.
        let mut i = one_clip(ClipMode::UserControlled, 0.0);
        i.update(0.7, &mut durations);
        i.update(3.0, &mut durations);
        assert_eq!(time(&i), 0.0);
        let mut i = one_clip(ClipMode::UserControlled, 0.25);
        i.update(0.7, &mut durations);
        assert_eq!(time(&i), 0.5);
    }

    #[test]
    fn ping_pong_clips_turn_round() {
        let mut i = one_clip(ClipMode::PingPong, 0.0);
        i.update(1.5, &mut durations);
        assert_eq!(time(&i), 1.5);
        // Past the end by 1 s: back 1 s from it.
        i.update(1.5, &mut durations);
        assert_eq!(time(&i), 1.0);
        i.update(0.5, &mut durations);
        assert_eq!(time(&i), 0.5);
    }

    #[test]
    fn sit_loop_and_stand_with_blend() {
        let mut i = instance(graph());
        assert_eq!(playing(&i), [("Idle".into(), 1.0)]);
        assert!(i.handle_event("Sit", &mut durations));
        assert_eq!(playing(&i), [("Enter".into(), 1.0)]);
        // Half way through the enter clip its draw trigger fires.
        i.update(0.6, &mut durations);
        assert_eq!(
            i.take_raised(),
            [Raised {
                event: "Draw".into(),
                payload: Some("Tankard".into())
            }]
        );
        // Its end trigger moves on to the loop.
        i.update(0.5, &mut durations);
        assert_eq!(playing(&i), [("Loop".into(), 1.0)]);
        assert!(
            !i.handle_event("Sit", &mut durations),
            "no transition on Sit from the loop"
        );
        // Stand blends over 0.5 s.
        i.update(1.0, &mut durations);
        assert!(i.handle_event("Stand", &mut durations));
        i.update(0.25, &mut durations);
        assert_eq!(playing(&i), [("Loop".into(), 0.5), ("Exit".into(), 0.5)]);
        i.update(0.3, &mut durations);
        // Exit ended (0.55 s > 0.5 s): back to idle.
        assert_eq!(playing(&i), [("Idle".into(), 1.0)]);
        assert_eq!(i.active_states(), ["s0"]);
    }

    #[test]
    fn condition_transitions_and_variables() {
        let mut g = graph();
        let speed = g.add_variable("Speed", VarType::Real, 0.0);
        let _ = speed;
        // Idle -> loop (state 2) when Speed > 1, no event.
        if let Generator::StateMachine { states, .. } = &mut g.generators[0] {
            let mut t = Transition::new(-1, 2, None);
            t.condition = Some("Speed > 1".into());
            states[0].transitions.push(t);
        }
        let mut i = instance(g);
        i.update(0.1, &mut durations);
        assert_eq!(i.active_states(), ["s0"]);
        assert!(i.set_variable("speed", 2.0));
        i.update(0.1, &mut durations);
        assert_eq!(i.active_states(), ["s2"]);
    }

    #[test]
    fn parametric_blend_and_reverse_clip() {
        // A parametric blend of three clips at 0, 1 and 2 driven by Direction.
        let child = |g, w| BlendChild {
            generator: Some(g),
            weight: w,
            weight_variable: None,
            bone_weights: None,
        };
        let mut back = clip("Back", ClipMode::SinglePlay, vec![]);
        if let Generator::Clip { speed, .. } = &mut back {
            *speed = -1.0;
        }
        let gens = vec![
            Generator::Blender {
                name: "B".into(),
                parameter: 0.0,
                min_cyclic: 0.0,
                max_cyclic: 1.0,
                flags: BLEND_PARAMETRIC,
                children: vec![child(1, 0.0), child(2, 1.0), child(3, 2.0)],
            },
            clip("A", ClipMode::Looping, vec![]),
            back,
            clip("C", ClipMode::Looping, vec![]),
        ];
        let mut g = BehaviorGraph::new("G", Some(0), gens, vec![]);
        let v = g.add_variable("Direction", VarType::Real, 1.5);
        g.bindings[0].push(Binding {
            member: "blendParameter".into(),
            variable: v,
        });
        let mut i = instance(g);
        assert_eq!(playing(&i), [("Back".into(), 0.5), ("C".into(), 0.5)]);
        i.update(0.25, &mut durations);
        let back = i
            .samples()
            .into_iter()
            .find(|s| &*s.animation == "Back")
            .unwrap();
        assert!(
            (back.time - 0.75).abs() < 1e-5,
            "reversed clip runs from its end: {}",
            back.time
        );
    }

    #[test]
    fn global_wildcard_enters_nested_state() {
        // Root: idle (0) and a nested machine (1) whose global wildcard on Kneel
        // leads to its state 7.
        let mut kneel = Transition::new(0, 7, None);
        kneel.flags |= 0x400;
        let gens = vec![
            Generator::StateMachine {
                name: "Root".into(),
                start: 0,
                start_variable: None,
                start_mode: Default::default(),
                states: vec![state(0, 1, vec![]), state(1, 2, vec![])],
                wildcards: vec![],
            },
            clip("Idle", ClipMode::Looping, vec![]),
            Generator::StateMachine {
                name: "Nested".into(),
                start: 5,
                start_variable: None,
                start_mode: Default::default(),
                states: vec![state(5, 3, vec![]), state(7, 4, vec![])],
                wildcards: vec![kneel],
            },
            clip("Other", ClipMode::Looping, vec![]),
            clip("Kneel", ClipMode::Looping, vec![]),
        ];
        let mut i = instance(BehaviorGraph::new("G", Some(0), gens, vec!["Kneel".into()]));
        assert!(i.handle_event("kneel", &mut durations));
        assert_eq!(playing(&i), [("Kneel".into(), 1.0)]);
    }

    /// A loop (0) raising Open at 1 s and Close at 1.5 s of its 2 s cycle, left on
    /// Out for state 1 (an exit clip) within Open .. Close.
    fn windowed(trigger: bool) -> BehaviorGraph {
        let events = ["Out", "Open", "Close", "Kill"].map(String::from).to_vec();
        let at = |time, event| Trigger {
            time,
            from_end: false,
            event,
            payload: None,
        };
        let window = Interval {
            enter_event: 1,
            exit_event: 2,
            enter_time: 0.0,
            exit_time: 0.0,
        };
        let mut out = Transition::new(0, 1, None);
        if trigger {
            out.trigger = Some(window);
        } else {
            out.initiate = Some(window);
        }
        let mut kill = Transition::new(3, 2, None);
        kill.flags |= 0x4;
        kill.blend = Some(0.5);
        let gens = vec![
            Generator::StateMachine {
                name: "Root".into(),
                start: 0,
                start_variable: None,
                start_mode: Default::default(),
                states: vec![
                    state(0, 1, vec![out]),
                    state(1, 2, vec![]),
                    state(2, 3, vec![]),
                ],
                wildcards: vec![kill],
            },
            clip("Loop", ClipMode::Looping, vec![at(1.0, 1), at(1.5, 2)]),
            clip("Exit", ClipMode::Looping, vec![]),
            clip("Dying", ClipMode::Looping, vec![]),
        ];
        BehaviorGraph::new("G", Some(0), gens, events)
    }

    #[test]
    fn initiate_interval_delays_the_transition() {
        let mut i = instance(windowed(false));
        i.update(0.2, &mut durations);
        // Triggered before the window: accepted, but waits for Open.
        assert!(i.handle_event("Out", &mut durations));
        assert_eq!(i.active_states(), ["s0"]);
        i.update(0.5, &mut durations);
        assert_eq!(i.active_states(), ["s0"]);
        i.update(0.4, &mut durations);
        assert_eq!(i.active_states(), ["s1"], "taken when the window opens");
    }

    #[test]
    fn trigger_interval_ignores_events_outside_it() {
        let mut i = instance(windowed(true));
        i.update(0.2, &mut durations);
        assert!(!i.handle_event("Out", &mut durations));
        i.update(1.0, &mut durations);
        assert!(
            i.handle_event("Out", &mut durations),
            "inside Open .. Close"
        );
        assert_eq!(i.active_states(), ["s1"]);
        // After Close it's ignored again.
        let mut i = instance(windowed(true));
        i.update(1.7, &mut durations);
        assert!(!i.handle_event("Out", &mut durations));
    }

    #[test]
    fn uninterruptible_blend_holds_off_other_transitions() {
        let mut g = windowed(false);
        if let Generator::StateMachine { wildcards, .. } = &mut g.generators[0] {
            wildcards[0].flags |= 0x200;
        }
        let mut i = instance(g);
        assert!(i.handle_event("Kill", &mut durations));
        i.update(0.2, &mut durations);
        assert!(
            !i.handle_event("Kill", &mut durations),
            "locked while blending"
        );
        i.update(0.4, &mut durations);
        assert_eq!(playing(&i), [("Dying".into(), 1.0)]);
        assert!(
            i.handle_event("Kill", &mut durations),
            "free once the blend ends"
        );
    }
}
