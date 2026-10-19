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
//! generators. Pose settings (twist, foot IK, look-at) are exposed to the engine,
//! which applies them to the sampled pose.

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
    /// Blender children's masks, including character-property bindings.
    blend_masks: HashMap<(usize, GenId), Vec<Option<Arc<[f32]>>>>,
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
            blend_masks: HashMap::new(),
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
                    Generator::Blender { children, .. } => {
                        let masks = children
                            .iter()
                            .map(|c| {
                                c.bone_weights_property
                                    .as_ref()
                                    .and_then(|p| {
                                        s.project
                                            .character
                                            .as_ref()?
                                            .bone_weights
                                            .get(&p.to_ascii_lowercase())
                                    })
                                    .or(c.bone_weights.as_ref())
                                    .map(|w| Arc::from(w.as_slice()))
                            })
                            .collect();
                        s.blend_masks.insert((gi, id), masks);
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

    fn modifier_properties(&self, gi: usize, m: ModId) -> impl Iterator<Item = (&str, f32)> {
        self.graph(gi)
            .modifier_property_bindings
            .get(m)
            .into_iter()
            .flatten()
            .filter_map(|b| {
                let value = self
                    .project
                    .character
                    .as_ref()?
                    .scalar_properties
                    .get(&b.property.to_ascii_lowercase())?;
                Some((b.member.as_str(), *value))
            })
    }

    fn modifier_input(&self, gi: usize, m: ModId, member: &str, values: &[f32]) -> Option<f32> {
        self.graph(gi)
            .modifier_bindings
            .get(m)
            .into_iter()
            .flatten()
            .find(|b| b.member == member)
            .and_then(|b| self.maps[gi].vars.get(b.variable))
            .and_then(|&v| values.get(v))
            .copied()
            .or_else(|| {
                self.modifier_properties(gi, m)
                    .find(|(name, _)| *name == member)
                    .map(|(_, value)| value)
            })
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
    /// Resolved graph-local synchronization target, fixed for this activation.
    sync_variable: Option<usize>,
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
    /// The blend's effect lets the state being left hear events too.
    from_hears: bool,
}

impl MachineState {
    fn new(state: i32, child: Option<Box<Node>>, sync_variable: Option<usize>) -> MachineState {
        MachineState {
            state,
            sync_variable,
            child,
            from: None,
            elapsed: 0.0,
            seen: Vec::new(),
            seq: 0,
            pending: None,
            locked: false,
            from_hears: false,
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
    /// State changes so far (see [`Instance::changes`]).
    changes: u64,
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
    changes: &'a mut u64,
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
            .or_else(|| {
                let binding = self
                    .shared
                    .graph(gi)
                    .property_bindings
                    .get(g)?
                    .iter()
                    .find(|b| b.member == member)?;
                self.shared
                    .project
                    .character
                    .as_ref()?
                    .scalar_properties
                    .get(&binding.property.to_ascii_lowercase())
                    .copied()
            })
    }

    /// Share the same flags between pose weighting and synchronized timing.
    /// Reject invalid integer inputs before casting; a bound variable still
    /// takes precedence over a duplicate property even when it is invalid.
    fn blend_flags(&self, gi: usize, g: GenId, authored: u16) -> u16 {
        self.bound(gi, g, "flags")
            .filter(|v| v.is_finite() && *v >= 0.0 && *v <= u16::MAX as f32 && v.fract() == 0.0)
            .map_or(authored, |v| v as u16)
    }

    /// Publish the state ID (not its index in `states`) through an authored
    /// output binding. Do this before activating the state's child, whose
    /// selectors / start states may read the variable.
    fn set_current_state(&mut self, gi: usize, g: GenId, state: i32) {
        if let Some(v) = self.shared.graph(gi).bound(g, "currentStateId") {
            self.set_var(gi, v, state as f32);
        }
    }

    /// Value of the variable bound to `member` of modifier `m`.
    fn modifier_bound(&self, gi: usize, m: ModId, member: &str) -> Option<f32> {
        self.shared.modifier_input(gi, m, member, self.values)
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
                    // Mode determines the active clip's timing state. Resolve it
                    // once with the other activation inputs; unsupported enum
                    // values keep the authored mode rather than casting to zero.
                    let mode = match self.bound(gi, g, "mode") {
                        Some(0.0) => ClipMode::SinglePlay,
                        Some(1.0) => ClipMode::Looping,
                        Some(2.0) => ClipMode::UserControlled,
                        Some(3.0) => ClipMode::PingPong,
                        _ => *mode,
                    };
                    let speed = self.bound(gi, g, "playbackSpeed").unwrap_or(*speed);
                    let full = self.clips.duration(animation).unwrap_or(0.0);
                    // Crops are activation inputs, like startTime. Keep one resolved
                    // length/offset for samples, triggers and synchronized timing.
                    let resolved_start = self
                        .bound(gi, g, "cropStartAmountLocalTime")
                        .filter(|v| v.is_finite())
                        .unwrap_or(*crop_start);
                    let resolved_end = self
                        .bound(gi, g, "cropEndAmountLocalTime")
                        .filter(|v| v.is_finite())
                        .unwrap_or(*crop_end);
                    let resolved_length = (full - resolved_start - resolved_end).max(0.0);
                    // Invalid bound inputs must not introduce an infinite cycle.
                    // Preserve the authored crop policy when their sum overflows.
                    let (offset, length) = if resolved_length.is_finite() {
                        (resolved_start, resolved_length)
                    } else {
                        (*crop_start, (full - crop_start - crop_end).max(0.0))
                    };
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
                        offset,
                        reverse: speed < 0.0,
                        additive,
                        looping: !matches!(mode, ClipMode::SinglePlay | ClipMode::UserControlled),
                        user: mode == ClipMode::UserControlled,
                        ping_pong: mode == ClipMode::PingPong,
                        wrapped: false,
                        started: false,
                    })
                }
                Generator::StateMachine {
                    start,
                    start_variable,
                    start_mode,
                    sync_variable,
                    states,
                    ..
                } => {
                    // Keep the explicit variable field for graphs built in code;
                    // parsed variable/property bindings use the common input path.
                    let bound = start_variable
                        .map(|v| self.var(gi, v) as i32)
                        .or_else(|| self.bound(gi, g, "startStateId").map(|v| v as i32));
                    let authored_sync = sync_variable.or(match *start_mode {
                        StartMode::Sync(v) => Some(v),
                        _ => None,
                    });
                    // Bindings address this graph's variable table, never the
                    // project's merged table. -1 is the authored no-variable
                    // sentinel; malformed inputs preserve the authored index.
                    let sync_variable = match self.bound(gi, g, "syncVariableIndex") {
                        Some(-1.0) => None,
                        Some(v)
                            if v.is_finite()
                                && v >= 0.0
                                && v.fract() == 0.0
                                && (v as f64) < graph.variables.len() as f64 =>
                        {
                            Some(v as usize)
                        }
                        _ => authored_sync,
                    };
                    let mode = match self.bound(gi, g, "startStateMode") {
                        Some(0.0) => StartMode::Default,
                        Some(1.0) => sync_variable.map_or(StartMode::Default, StartMode::Sync),
                        Some(2.0) => StartMode::Random,
                        // Chooser and malformed enums retain the existing mode.
                        _ => match *start_mode {
                            StartMode::Sync(_) => {
                                sync_variable.map_or(StartMode::Default, StartMode::Sync)
                            }
                            mode => mode,
                        },
                    };
                    let active_sync = match mode {
                        StartMode::Sync(v) => Some(v),
                        _ => None,
                    };
                    let by_mode = match mode {
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
                            let enters = s.enter_events.clone();
                            self.set_current_state(gi, g, id);
                            for e in enters {
                                self.raise(gi, e.event, e.payload);
                            }
                            let child = generator
                                .map(|c| Box::new(self.activate(gi, c, nested, depth + 1)));
                            Kind::Machine(MachineState::new(id, child, active_sync))
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
                let mut any = m.child.as_mut().is_some_and(|c| self.handle(c, ev));
                if m.from_hears
                    && let Some((f, _, _)) = &mut m.from
                {
                    any |= self.handle(f, ev);
                }
                any || took == Took::Pending
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
        let Generator::StateMachine {
            states,
            change_event,
            ..
        } = &graph.generators[sm]
        else {
            return;
        };
        let Kind::Machine(m) = &mut node.kind else {
            return;
        };
        let old = states.iter().find(|s| s.id == m.state);
        let new = states.iter().find(|s| s.id == t.to_state).expect("checked");
        // Into the state it is in: the effect says whether that restarts it.
        let again = t.to_nested.is_none() && old.is_some_and(|s| s.id == new.id);
        if again
            && match t.self_mode {
                SelfTransitionMode::Continue => true,
                SelfTransitionMode::ContinueIfCyclicBlendIfAcyclic => {
                    m.child.as_deref().is_some_and(cyclic)
                }
                _ => false,
            }
        {
            return;
        }
        let exits = old.map(|s| s.exit_events.clone()).unwrap_or_default();
        let (enters, generator, id) = (new.enter_events.clone(), new.generator, new.id);
        let blend = t
            .blend_variable
            .map(|v| self.var(gi, v))
            .or_else(|| {
                self.shared
                    .project
                    .character
                    .as_ref()?
                    .scalar_properties
                    .get(&t.blend_property.as_ref()?.to_ascii_lowercase())
                    .copied()
            })
            .or(t.blend)
            .unwrap_or(0.0);
        let blend = if again && t.self_mode == SelfTransitionMode::Reset {
            0.0
        } else {
            blend
        };
        let old_child = m.child.take();
        let old_from = m.from.take();
        let sync_variable = m.sync_variable;
        for e in exits {
            self.raise(gi, e.event, e.payload);
        }
        for e in enters {
            self.raise(gi, e.event, e.payload);
        }
        *self.changes += 1;
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
        self.set_current_state(gi, sm, id);
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
        *m = MachineState::new(id, child, sync_variable);
        m.from_hears = from.is_some() && t.event_mode == EventMode::ProcessAll;
        m.from = from;
        m.locked = locked;
        // Queue the notification after installing the destination: the event
        // may itself trigger another transition (including in a parent graph).
        // Activation / blend-end notification timing remains unresolved; see
        // known_ambiguities/state-machine-events.md.
        if let Some(e) = change_event {
            self.raise(gi, e.event, e.payload.clone());
        }
        if let Some(v) = sync_variable {
            self.set_var(gi, v, id as f32);
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
                        m.from_hears = false;
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
                if self.blend_flags(gi, g, *flags) & BLEND_SYNC != 0 {
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
                    *self.changes += 1;
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
        if self
            .modifier_bound(gi, m.m, "enable")
            .is_some_and(|v| v == 0.0)
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
                _,
                Modifier::InterpValue {
                    source,
                    target,
                    result,
                    duration,
                },
            ) => {
                // A step of the way each update: dt of the duration (all of it
                // past the end), from wherever `source` is now.
                let from = self.modifier_bound(gi, m.m, "source").unwrap_or(*source);
                let to = self.modifier_bound(gi, m.m, "target").unwrap_or(*target);
                let f = if *duration > 1e-4 {
                    (dt / duration).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                let _ = result;
                self.set_modifier_output(gi, m.m, "result", from + (to - from) * f);
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
        let flags = self.blend_flags(gi, g, *flags);
        let raw: Vec<f32> = children
            .iter()
            .map(|c| {
                c.weight_variable
                    .map(|v| self.var(gi, v))
                    .or_else(|| {
                        let property = c.weight_property.as_ref()?;
                        self.shared
                            .project
                            .character
                            .as_ref()?
                            .scalar_properties
                            .get(&property.to_ascii_lowercase())
                            .copied()
                    })
                    .unwrap_or(c.weight)
            })
            .collect();
        let mut w = vec![0.0; children.len()];
        if flags & BLEND_PARAMETRIC != 0 && !children.is_empty() {
            // Children sit at their weights along the blend parameter; blend the two
            // around it (wrapping for cyclic blends such as direction).
            let mut p = self.bound(gi, g, "blendParameter").unwrap_or(*parameter);
            let min_cyclic = self
                .bound(gi, g, "minCyclicBlendParameter")
                .filter(|v| v.is_finite())
                .unwrap_or(*min_cyclic);
            let max_cyclic = self
                .bound(gi, g, "maxCyclicBlendParameter")
                .filter(|v| v.is_finite())
                .unwrap_or(*max_cyclic);
            let period = max_cyclic - min_cyclic;
            // Invalid ranges retain the existing non-cyclic endpoint policy.
            // Reject overflow as well as non-finite authored endpoints before
            // modulo can contaminate pose weights and synchronized clip times.
            let cyclic = flags & BLEND_CYCLIC != 0
                && min_cyclic.is_finite()
                && max_cyclic.is_finite()
                && period.is_finite()
                && period > 0.0;
            let wide_period = max_cyclic as f64 - min_cyclic as f64;
            if cyclic {
                // Even finite endpoints and a finite period can overflow a
                // shifted f32 parameter. Widen the arithmetic, then return to
                // the finite authored interval before computing child weights.
                p = (min_cyclic as f64 + (p as f64 - min_cyclic as f64).rem_euclid(wide_period))
                    as f32;
            }
            let mut order: Vec<usize> = (0..raw.len()).collect();
            order.sort_by(|&a, &b| raw[a].total_cmp(&raw[b]));
            let first = order[0];
            let last = *order.last().unwrap();
            if p <= raw[first] {
                if cyclic && order.len() > 1 {
                    let span = raw[first] as f64 + wide_period - raw[last] as f64;
                    let f = if span > 0.0 {
                        ((p as f64 + wide_period - raw[last] as f64) / span) as f32
                    } else {
                        1.0
                    };
                    w[last] = 1.0 - f;
                    w[first] = f;
                } else {
                    w[first] = 1.0;
                }
            } else if p >= raw[last] {
                if cyclic && order.len() > 1 {
                    let span = raw[first] as f64 + wide_period - raw[last] as f64;
                    let f = if span > 0.0 {
                        ((p as f64 - raw[last] as f64) / span) as f32
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
                let Some(masks) = self.shared.blend_masks.get(&(gi, g)) else {
                    return;
                };
                let additive: Vec<bool> = children
                    .iter()
                    .map(|c| c.as_ref().is_some_and(is_additive))
                    .collect();
                let w = self.blend_weights(gi, g, &additive);
                for ((c, wi), child_mask) in children.iter().zip(w).zip(masks) {
                    let Some(c) = c else { continue };
                    match child_mask {
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

/// Whether the generator plays on forever: a looping or ping-pong clip, or a
/// wrapper or blend of only such clips.
fn cyclic(node: &Node) -> bool {
    match &node.kind {
        Kind::Clip(c) => c.looping || c.ping_pong,
        Kind::Wrap { child: Some(c), .. } | Kind::Select { child: Some(c), .. } => cyclic(c),
        Kind::Blend { children, .. } => {
            children.iter().flatten().all(cyclic) && children.iter().flatten().next().is_some()
        }
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
            changes: 0,
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
            changes: &mut self.changes,
            clips,
            trace: self.trace.as_mut(),
        }
    }

    /// How many times a state machine changed state, or a selector its
    /// child, so far: a caller keeping the graph in some state (moving,
    /// say) sees when the graph may have left it.
    pub fn changes(&self) -> u64 {
        self.changes
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
                changes: &mut 0,
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

    /// Modifiers running now and enabled (graph, modifier), lists and event-driven
    /// modifiers opened up.
    fn running_modifiers(&self) -> Vec<(usize, ModId)> {
        fn mods(shared: &Shared, m: &ModState, values: &[f32], out: &mut Vec<(usize, ModId)>) {
            let enabled = shared
                .modifier_input(m.gi, m.m, "enable", values)
                .is_none_or(|v| v != 0.0);
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
    }

    /// Counter-clockwise yaw in radians the running character-rotation modifiers turn the
    /// character through in `dt` seconds: rate times multiplier along the vertical part of
    /// each axis (see `known_ambiguities/rotate-character.md`). Unit-length axes are not
    /// assumed; a zero or non-finite axis turns nothing.
    pub fn character_turn(&self, dt: f32) -> f32 {
        self.character_rotations()
            .iter()
            .map(|r| {
                let axis = r.axis.truncate();
                let up = if axis.length() > 1e-6 && axis.is_finite() {
                    axis.normalize().z
                } else {
                    0.0
                };
                let turn = (r.degrees_per_second * r.speed_multiplier).to_radians() * dt * up;
                if turn.is_finite() { turn } else { 0.0 }
            })
            .sum()
    }

    /// Bound members of modifier `m` and their variables' values.
    fn bound_members(&self, gi: usize, m: ModId) -> impl Iterator<Item = (&str, f32)> {
        self.shared.graph(gi).modifier_bindings[m]
            .iter()
            .filter_map(move |b| {
                let v = self.shared.maps[gi]
                    .vars
                    .get(b.variable)
                    .and_then(|&v| self.values.get(v))?;
                Some((b.member.as_str(), *v))
            })
            .chain(
                self.shared
                    .modifier_properties(gi, m)
                    .filter(move |(member, _)| {
                        !self.shared.graph(gi).modifier_bindings[m]
                            .iter()
                            .any(|b| b.member == *member)
                    }),
            )
    }

    /// Running character-rotation modifiers with scalar inputs resolved. Unsupported bindings omit the modifier rather than
    /// exposing an authored value in place of a bound vector.
    pub fn character_rotations(&self) -> Vec<RotateCharacter> {
        self.running_modifiers()
            .into_iter()
            .filter_map(|(gi, m)| {
                let Modifier::RotateCharacter(r) = &self.shared.graph(gi).modifiers[m] else {
                    return None;
                };
                let graph = self.shared.graph(gi);
                let members = graph.modifier_bindings[m]
                    .iter()
                    .map(|b| b.member.as_str())
                    .chain(
                        graph
                            .modifier_property_bindings
                            .get(m)
                            .into_iter()
                            .flatten()
                            .map(|b| b.member.as_str()),
                    );
                for member in members {
                    if !matches!(member, "enable" | "degreesPerSecond" | "speedMultiplier")
                        || self
                            .shared
                            .modifier_input(gi, m, member, &self.values)
                            .is_none()
                    {
                        return None;
                    }
                }
                let mut r = r.clone();
                for (member, v) in self.bound_members(gi, m) {
                    match member {
                        "enable" => r.enabled = v.is_finite() && v != 0.0,
                        "degreesPerSecond" => r.degrees_per_second = v,
                        "speedMultiplier" => r.speed_multiplier = v,
                        _ => return None,
                    }
                }
                r.enabled.then_some(r)
            })
            .collect()
    }

    /// Twist modifiers running now, with bound angle and enable resolved. The
    /// pose consumer chooses which authored modes it can apply.
    pub fn twists(&self) -> Vec<Twist> {
        self.running_modifiers()
            .into_iter()
            .filter_map(|(gi, m)| {
                let Modifier::Twist(t) = &self.shared.graph(gi).modifiers[m] else {
                    return None;
                };
                let mut t = t.clone();
                for (member, v) in self.bound_members(gi, m) {
                    match member {
                        "enable" => t.enabled = v.is_finite() && v != 0.0,
                        "twistAngle" => t.angle_degrees = v,
                        // Other bindings require typed/vector graph variables;
                        // fail closed instead of applying stale authored data.
                        _ => return None,
                    }
                }
                t.enabled.then_some(t)
            })
            .collect()
    }

    /// Look-at modifiers running now (enabled), with their bound members resolved:
    /// target on / off, per-bone `enabled` and gains.
    pub fn look_ats(&self) -> Vec<LookAt> {
        self.running_modifiers()
            .into_iter()
            .filter_map(|(gi, m)| {
                let graph = self.shared.graph(gi);
                let Modifier::LookAt(l) = &graph.modifiers[m] else {
                    return None;
                };
                let mut l = l.clone();
                for (member, v) in self.bound_members(gi, m) {
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

    /// The foot IK controls running now (enabled), with bound gains resolved; foot
    /// placement is off without one.
    pub fn foot_ik_controls(&self) -> Option<FootIkGains> {
        self.running_modifiers().into_iter().find_map(|(gi, m)| {
            let Modifier::FootIkControls(g) = &self.shared.graph(gi).modifiers[m] else {
                return None;
            };
            let mut g = g.clone();
            for (member, v) in self.bound_members(gi, m) {
                let Some(field) = member.strip_prefix("controlData/gains/") else {
                    continue;
                };
                *match field {
                    "onOffGain" => &mut g.on_off,
                    "groundAscendingGain" => &mut g.ground_ascending,
                    "groundDescendingGain" => &mut g.ground_descending,
                    "footPlantedGain" => &mut g.foot_planted,
                    "footRaisedGain" => &mut g.foot_raised,
                    "footUnlockGain" => &mut g.foot_unlock,
                    "worldFromModelFeedbackGain" => &mut g.world_from_model_feedback,
                    "errorUpDownBias" => &mut g.error_up_down_bias,
                    "alignWorldFromModelGain" => &mut g.align_world_from_model,
                    "hipOrientationGain" => &mut g.hip_orientation,
                    "maxKneeAngleDifference" => &mut g.max_knee_angle_difference,
                    "ankleOrientationGain" => &mut g.ankle_orientation,
                    _ => continue,
                } = v;
            }
            Some(g)
        })
    }

    /// Names of the active states, outermost first (for debugging).
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
                sync_variable: None,
                change_event: None,
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

    fn property(member: &str, name: &str) -> PropertyBinding {
        PropertyBinding {
            member: member.into(),
            property: name.into(),
        }
    }

    fn property_instance(g: BehaviorGraph, properties: &[(&str, f32)]) -> Instance {
        let character = Character {
            scalar_properties: properties
                .iter()
                .map(|(name, value)| (name.to_ascii_lowercase(), *value))
                .collect(),
            ..Default::default()
        };
        let mut i = Instance::new(
            Shared::new(Arc::new(Project {
                graphs: vec![("g".into(), g)],
                character: Some(character),
            })),
            1,
        );
        i.update(0.0, &mut durations);
        i
    }

    fn mode_property_graph() -> BehaviorGraph {
        let mut c = clip(
            "Loop",
            ClipMode::SinglePlay,
            vec![Trigger {
                time: 0.0,
                from_end: true,
                event: 0,
                payload: Some("end".into()),
            }],
        );
        if let Generator::Clip { user_fraction, .. } = &mut c {
            *user_fraction = 0.25;
        }
        let mut g = BehaviorGraph::new("Modes", Some(0), vec![c], vec!["End".into()]);
        g.property_bindings[0] = vec![property("mode", "PLAYBACK")];
        g
    }

    #[test]
    fn clip_mode_properties_drive_samples_wrapping_and_end_triggers() {
        for (mode, expected_time, wrapped, end_event) in [
            (0.0, 2.0, false, true),
            (1.0, 0.5, true, true),
            (2.0, 0.5, false, false),
            (3.0, 1.5, true, true),
        ] {
            let mut i = property_instance(mode_property_graph(), &[("playback", mode)]);
            assert_eq!(time(&i), if mode == 2.0 { 0.5 } else { 0.0 });
            i.update(2.5, &mut durations);
            let samples = i.samples();
            assert_eq!(samples[0].time, expected_time, "mode {mode}");
            assert_eq!(samples[0].wrapped, wrapped, "mode {mode}");
            assert_eq!(i.take_raised().len(), usize::from(end_event), "mode {mode}");
            assert!(!i.set_variable("Playback", 0.0));
        }
    }

    #[test]
    fn clip_mode_properties_keep_authored_fallback_for_missing_or_invalid_inputs() {
        let mut absent = instance(mode_property_graph());
        absent.update(2.5, &mut durations);
        assert_eq!(time(&absent), 2.0);
        // A pointer property with the same name cannot be used as an enum.
        let mut non_scalar = Instance::new(
            Shared::new(Arc::new(Project {
                graphs: vec![("g".into(), mode_property_graph())],
                character: Some(Character {
                    bone_weights: [("playback".into(), vec![1.0])].into(),
                    ..Default::default()
                }),
            })),
            1,
        );
        non_scalar.update(2.5, &mut durations);
        assert_eq!(time(&non_scalar), 2.0);
        for properties in [vec![], vec![("unrelated", 1.0)]] {
            let mut i = property_instance(mode_property_graph(), &properties);
            i.update(2.5, &mut durations);
            assert_eq!(time(&i), 2.0);
            assert!(!i.samples()[0].wrapped);
        }
        for mode in [
            -1.0,
            0.5,
            4.0,
            256.0,
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
        ] {
            let mut i = property_instance(mode_property_graph(), &[("playback", mode)]);
            i.update(2.5, &mut durations);
            assert_eq!(time(&i), 2.0, "invalid mode {mode}");
            assert!(!i.samples()[0].wrapped);
        }
    }

    #[test]
    fn clip_mode_variables_take_precedence_and_resample_on_activation() {
        let bound = mode_property_graph();
        let mut g = BehaviorGraph::new(
            "Modes",
            Some(0),
            vec![
                Generator::StateMachine {
                    name: "Root".into(),
                    start: 0,
                    start_variable: None,
                    start_mode: Default::default(),
                    sync_variable: None,
                    change_event: None,
                    states: vec![
                        state(0, 1, vec![Transition::new(0, 1, None)]),
                        state(1, 2, vec![Transition::new(1, 0, None)]),
                    ],
                    wildcards: vec![],
                },
                clip("Loop", ClipMode::SinglePlay, vec![]),
                clip("Idle", ClipMode::Looping, vec![]),
            ],
            vec!["Switch".into(), "Back".into()],
        );
        g.property_bindings[1] = bound.property_bindings[0].clone();
        let variable = g.add_variable("Mode", VarType::Int, 0.0);
        g.bindings[1] = vec![Binding {
            member: "mode".into(),
            variable,
        }];
        let mut i = property_instance(g, &[("playback", 2.0)]);
        i.update(2.5, &mut durations);
        assert_eq!(time(&i), 2.0); // The variable overrides the user-controlled property.
        assert!(i.set_variable("Mode", 1.0));
        i.update(1.0, &mut durations);
        assert_eq!(time(&i), 2.0); // The active clip retains its mode.
        i.handle_event("Switch", &mut durations);
        i.handle_event("Back", &mut durations);
        i.update(2.5, &mut durations);
        assert_eq!(time(&i), 0.5); // Reactivation resolves the looping variable.

        assert!(i.set_variable("Mode", f32::NAN));
        i.handle_event("Switch", &mut durations);
        i.handle_event("Back", &mut durations);
        i.update(2.5, &mut durations);
        assert_eq!(time(&i), 2.0); // An invalid variable keeps authored mode, not the property.
    }

    #[test]
    fn clip_mode_properties_supply_synchronized_looping_state() {
        for mode in [0.0, 1.0] {
            let mut blend = masked_blend(
                "Sync",
                vec![masked_child(1, None, None), masked_child(2, None, None)],
            );
            if let Generator::Blender { flags, .. } = &mut blend {
                *flags = BLEND_SYNC;
            }
            let mut g = BehaviorGraph::new(
                "ModeSync",
                Some(0),
                vec![
                    blend,
                    clip("Enter", ClipMode::SinglePlay, vec![]),
                    clip("Loop", ClipMode::SinglePlay, vec![]),
                ],
                vec![],
            );
            g.property_bindings[1] = vec![property("mode", "Playback")];
            let mut i = property_instance(g, &[("playback", mode)]);
            i.update(2.0, &mut durations);
            let samples = i.samples();
            let phase = if mode == 0.0 { 1.0 } else { 1.0 / 3.0 };
            assert!((samples[0].time - phase).abs() < 1e-6);
            assert!((samples[1].time - phase * 2.0).abs() < 1e-6);
            assert!(samples.iter().all(|s| s.wrapped == (mode == 1.0)));
        }
    }

    #[test]
    fn clip_crop_properties_drive_forward_reverse_samples_and_end_triggers() {
        for (start, end, speed, initial, halfway, final_time) in [
            (0.5, 0.25, 1.0, 0.5, 1.0, 1.75),
            (0.25, 0.5, -1.0, 1.5, 1.0, 0.25),
        ] {
            let mut g = cropped_property_graph(ClipMode::SinglePlay);
            g.events = vec!["End".into()];
            if let Generator::Clip { triggers, .. } = &mut g.generators[0] {
                triggers.push(Trigger {
                    time: if speed < 0.0 { 0.0 } else { 1.25 },
                    from_end: false,
                    event: 0,
                    payload: None,
                });
            }
            let mut i = property_instance(
                g,
                &[("cropstart", start), ("cropend", end), ("speed", speed)],
            );
            assert_eq!(time(&i), initial);
            assert!(i.take_raised().is_empty());
            i.update(0.5, &mut durations);
            assert_eq!(time(&i), halfway);
            assert_eq!(i.samples()[0].prev_time, initial);
            assert!(i.take_raised().is_empty());
            i.update(1.0, &mut durations);
            assert_eq!(time(&i), final_time);
            assert_eq!(i.take_raised()[0].event, "End");
            assert!(!i.set_variable("CropStart", 0.0));
            assert_eq!(i.variable("CropStart"), None);
            assert_eq!(
                i.shared
                    .project
                    .character
                    .as_ref()
                    .unwrap()
                    .scalar_properties["cropstart"],
                start
            );
        }
    }

    fn cropped_property_graph(mode: ClipMode) -> BehaviorGraph {
        let mut c = clip("Loop", mode, vec![]);
        if let Generator::Clip {
            crop_start,
            crop_end,
            user_fraction,
            ..
        } = &mut c
        {
            *crop_start = 0.25;
            *crop_end = 0.25;
            *user_fraction = 0.5;
        }
        let mut g = BehaviorGraph::new("Cropped", Some(0), vec![c], vec![]);
        g.property_bindings[0] = vec![
            property("cropStartAmountLocalTime", "CROPSTART"),
            property("cropEndAmountLocalTime", "CropEnd"),
            property("playbackSpeed", "Speed"),
        ];
        g
    }

    #[test]
    fn clip_crop_properties_keep_authored_fallback_and_existing_length_policy() {
        for mut i in [
            instance(cropped_property_graph(ClipMode::UserControlled)),
            property_instance(cropped_property_graph(ClipMode::UserControlled), &[]),
        ] {
            assert_eq!(time(&i), 1.0); // Authored .25 offset + half of 1.5 s.
            i.update(1.0, &mut durations);
            assert_eq!(time(&i), 1.0);
        }
        let i = property_instance(
            cropped_property_graph(ClipMode::UserControlled),
            &[("cropstart", 0.5), ("cropend", 0.25)],
        );
        assert_eq!(i.samples()[0].time, 1.125);
        let i = property_instance(
            cropped_property_graph(ClipMode::UserControlled),
            &[("cropstart", 1.5), ("cropend", 1.0)],
        );
        assert_eq!(time(&i), 1.5); // Overcropped clips retain a zero usable length.

        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -f32::MAX] {
            let mut i = property_instance(
                cropped_property_graph(ClipMode::SinglePlay),
                &[("cropstart", invalid), ("cropend", invalid)],
            );
            assert_eq!(time(&i), 0.25);
            i.update(0.5, &mut durations);
            assert_eq!(time(&i), 0.75);
        }
        let mut i = property_instance(
            cropped_property_graph(ClipMode::SinglePlay),
            &[("cropstart", -0.25), ("cropend", 0.0)],
        );
        assert_eq!(time(&i), -0.25); // Finite negative crops keep the authored policy.
        i.update(0.5, &mut durations);
        assert_eq!(time(&i), 0.25);
    }

    #[test]
    fn clip_crop_variables_take_precedence_and_resample_on_activation() {
        let cropped = cropped_property_graph(ClipMode::SinglePlay);
        let mut g = BehaviorGraph::new(
            "Cropped",
            Some(0),
            vec![
                Generator::StateMachine {
                    name: "Root".into(),
                    start: 0,
                    start_variable: None,
                    start_mode: Default::default(),
                    sync_variable: None,
                    change_event: None,
                    states: vec![
                        state(0, 1, vec![Transition::new(0, 1, None)]),
                        state(1, 2, vec![Transition::new(1, 0, None)]),
                    ],
                    wildcards: vec![],
                },
                cropped.generators[0].clone(),
                clip("Idle", ClipMode::Looping, vec![]),
            ],
            vec!["Switch".into(), "Back".into()],
        );
        g.property_bindings[1] = cropped.property_bindings[0].clone();
        let start = g.add_variable("Start", VarType::Real, 0.25);
        let end = g.add_variable("End", VarType::Real, 0.25);
        g.bindings[1] = vec![
            Binding {
                member: "cropStartAmountLocalTime".into(),
                variable: start,
            },
            Binding {
                member: "cropEndAmountLocalTime".into(),
                variable: end,
            },
        ];
        let mut i = property_instance(g, &[("cropstart", 0.5), ("cropend", 0.5)]);
        assert_eq!(time(&i), 0.25);
        assert!(i.set_variable("Start", 0.75));
        assert!(i.set_variable("End", 0.75));
        i.update(0.5, &mut durations);
        assert_eq!(time(&i), 0.75); // Active clips keep the activation's crop pair.
        i.handle_event("Switch", &mut durations);
        i.handle_event("Back", &mut durations);
        assert_eq!(time(&i), 0.75);
        i.update(1.0, &mut durations);
        assert_eq!(time(&i), 1.25); // The new activation has only .5 s to play.
    }

    #[test]
    fn clip_crop_properties_supply_synchronized_cycle_lengths() {
        for (start, end, expected_first, expected_second) in
            [(0.5, 0.5, 0.25, 0.75), (0.25, 0.25, 0.2, 0.55)]
        {
            let cropped = cropped_property_graph(ClipMode::Looping);
            let mut blend = masked_blend(
                "Sync",
                vec![masked_child(1, None, None), masked_child(2, None, None)],
            );
            if let Generator::Blender { flags, .. } = &mut blend {
                *flags = BLEND_SYNC;
            }
            let mut g = BehaviorGraph::new(
                "CroppedSync",
                Some(0),
                vec![
                    blend,
                    clip("Enter", ClipMode::Looping, vec![]),
                    cropped.generators[0].clone(),
                ],
                vec![],
            );
            g.property_bindings[2] = cropped.property_bindings[0].clone();
            let mut i = property_instance(g, &[("cropstart", start), ("cropend", end)]);
            i.update(0.25, &mut durations);
            let samples = i.samples();
            assert!((samples[0].time - expected_first).abs() < 1e-6);
            assert!((samples[1].time - expected_second).abs() < 1e-6);
        }
    }

    #[test]
    fn scalar_generator_properties_drive_clip_start_speed_and_user_time() {
        let mut g = BehaviorGraph::new(
            "G",
            Some(0),
            vec![clip("Loop", ClipMode::SinglePlay, vec![])],
            vec![],
        );
        g.property_bindings[0] = vec![
            property("startTime", "START"),
            property("playbackSpeed", "Speed"),
        ];
        let mut i = property_instance(g, &[("start", 0.25), ("speed", -2.0)]);
        assert_eq!(time(&i), 1.75); // Reverse starts at end minus startTime.
        i.update(0.25, &mut durations);
        assert_eq!(time(&i), 1.25);
        assert_eq!(i.samples()[0].prev_time, 1.75);
        assert!(!i.set_variable("Speed", 7.0));
        i.update(0.25, &mut durations);
        assert_eq!(time(&i), 0.75);

        let mut g = BehaviorGraph::new(
            "G",
            Some(0),
            vec![clip("Loop", ClipMode::UserControlled, vec![])],
            vec![],
        );
        g.property_bindings[0] = vec![property("userControlledTimeFraction", "Fraction")];
        let mut i = property_instance(g, &[("fraction", 0.75)]);
        assert_eq!(time(&i), 1.5);
        i.update(3.0, &mut durations);
        assert_eq!(time(&i), 1.5);
    }

    #[test]
    fn scalar_generator_properties_select_children_with_live_variable_precedence() {
        let mut g = BehaviorGraph::new(
            "G",
            Some(0),
            vec![
                Generator::Selector {
                    name: "Select".into(),
                    children: vec![1, 2],
                    index: 0,
                },
                clip("First", ClipMode::Looping, vec![]),
                clip("Second", ClipMode::Looping, vec![]),
            ],
            vec![],
        );
        g.property_bindings[0] = vec![property("selectedGeneratorIndex", "Choice")];
        let mut i = property_instance(g, &[("choice", 1.0)]);
        assert_eq!(playing(&i), [("Second".into(), 1.0)]);
        i.update(0.25, &mut durations);
        assert_eq!(playing(&i), [("Second".into(), 1.0)]);
        let mut g = BehaviorGraph::new(
            "G",
            Some(0),
            vec![
                Generator::Selector {
                    name: "Select".into(),
                    children: vec![1, 2],
                    index: 1,
                },
                clip("First", ClipMode::Looping, vec![]),
                clip("Second", ClipMode::Looping, vec![]),
            ],
            vec![],
        );
        g.property_bindings[0] = vec![property("selectedGeneratorIndex", "Choice")];
        let choice = g.add_variable("Choice", VarType::Int, 0.0);
        g.bindings[0] = vec![Binding {
            member: "selectedGeneratorIndex".into(),
            variable: choice,
        }];
        let mut i = property_instance(g, &[("choice", 1.0)]);
        assert_eq!(playing(&i), [("First".into(), 1.0)]);
        assert!(i.set_variable("Choice", 1.0));
        i.update(0.0, &mut durations);
        assert_eq!(playing(&i), [("Second".into(), 1.0)]);
        assert!(i.set_variable("Choice", 0.0));
        i.update(0.0, &mut durations);
        assert_eq!(playing(&i), [("First".into(), 1.0)]);
    }

    #[test]
    fn scalar_generator_properties_supply_start_states_but_never_receive_outputs() {
        let mut g = graph();
        g.property_bindings[0] = vec![
            property("startStateId", "Start"),
            property("currentStateId", "Output"),
        ];
        let mut i = property_instance(g, &[("start", 2.0), ("output", -7.0)]);
        assert_eq!(i.active_states(), ["s2"]);
        assert!(i.handle_event("Stand", &mut durations));
        i.update(0.6, &mut durations);
        assert_eq!(i.active_states(), ["s0"]);
        assert_eq!(
            i.shared
                .project
                .character
                .as_ref()
                .unwrap()
                .scalar_properties["output"],
            -7.0
        );
        assert_eq!(i.variable("Output"), None);
    }

    fn start_property_graph() -> BehaviorGraph {
        let mut g = BehaviorGraph::new(
            "Starts",
            Some(0),
            vec![
                Generator::StateMachine {
                    name: "Root".into(),
                    start: 10,
                    start_variable: None,
                    start_mode: StartMode::Default,
                    sync_variable: Some(0),
                    change_event: None,
                    states: vec![
                        state(10, 1, vec![Transition::new(0, 20, None)]),
                        state(20, 2, vec![Transition::new(0, 30, None)]),
                        state(30, 3, vec![Transition::new(0, 10, None)]),
                    ],
                    wildcards: vec![],
                },
                clip("Ten", ClipMode::Looping, vec![]),
                clip("Twenty", ClipMode::Looping, vec![]),
                clip("Thirty", ClipMode::Looping, vec![]),
            ],
            vec!["Cycle".into()],
        );
        g.add_variable("KeepA", VarType::Int, 20.0);
        g.add_variable("KeepB", VarType::Int, 30.0);
        g.property_bindings[0] = vec![
            property("startStateMode", "MODE"),
            property("syncVariableIndex", "Index"),
        ];
        g
    }

    #[test]
    fn state_start_properties_select_per_character_modes_and_sync_variables() {
        for (mode, index, expected) in [(0.0, 1.0, "s10"), (1.0, 0.0, "s20"), (1.0, 1.0, "s30")] {
            let mut i =
                property_instance(start_property_graph(), &[("mode", mode), ("index", index)]);
            assert_eq!(i.active_states(), [expected]);
            assert!(!i.set_variable("Mode", 0.0));
            assert!(!i.set_variable("Index", 0.0));
            assert!(i.handle_event("Cycle", &mut durations));
            assert_eq!(
                i.shared
                    .project
                    .character
                    .as_ref()
                    .unwrap()
                    .scalar_properties["mode"],
                mode
            );
            assert_eq!(
                i.shared
                    .project
                    .character
                    .as_ref()
                    .unwrap()
                    .scalar_properties["index"],
                index
            );
            assert_eq!(
                i.variable("KeepA"),
                Some(if mode == 1.0 && index == 0.0 {
                    30.0
                } else {
                    20.0
                })
            );
            assert_eq!(
                i.variable("KeepB"),
                Some(if mode == 1.0 && index == 1.0 {
                    10.0
                } else {
                    30.0
                })
            );
        }
        // A bound random mode follows the existing seeded random algorithm.
        let mut authored = start_property_graph();
        if let Generator::StateMachine { start_mode, .. } = &mut authored.generators[0] {
            *start_mode = StartMode::Random;
        }
        let random = property_instance(start_property_graph(), &[("mode", 2.0)]);
        let authored = instance(authored);
        let mut random = Instance::new(random.shared.clone(), 17);
        let mut authored = Instance::new(authored.shared.clone(), 17);
        random.update(0.0, &mut durations);
        authored.update(0.0, &mut durations);
        assert_eq!(random.active_states(), authored.active_states());
        assert_eq!(random.active_states(), ["s30"]); // Differs from default and sync starts.
    }

    #[test]
    fn state_start_properties_preserve_authored_fallbacks_for_unsupported_values() {
        let synced = || {
            let mut g = start_property_graph();
            if let Generator::StateMachine { start_mode, .. } = &mut g.generators[0] {
                *start_mode = StartMode::Sync(0);
            }
            g
        };
        for invalid in [
            -2.0,
            0.5,
            3.0,
            999.0,
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
        ] {
            assert_eq!(
                property_instance(synced(), &[("mode", invalid)]).active_states(),
                ["s20"]
            );
        }
        for invalid in [
            -2.0,
            0.5,
            2.0,
            999.0,
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
        ] {
            assert_eq!(
                property_instance(synced(), &[("index", invalid)]).active_states(),
                ["s20"]
            );
        }
        for mut i in [instance(synced()), property_instance(synced(), &[])] {
            assert_eq!(i.active_states(), ["s20"]);
            assert!(i.handle_event("Cycle", &mut durations));
            assert_eq!(i.variable("KeepA"), Some(30.0));
        }
        let mut non_scalar = property_instance(synced(), &[]);
        Arc::get_mut(&mut Arc::get_mut(&mut non_scalar.shared).unwrap().project)
            .unwrap()
            .character
            .as_mut()
            .unwrap()
            .bone_weights
            .extend([("mode".into(), vec![0.0]), ("index".into(), vec![1.0])]);
        // Re-activate after installing only array properties.
        non_scalar.root = None;
        non_scalar.update(0.0, &mut durations);
        assert_eq!(non_scalar.active_states(), ["s20"]);

        let mut disabled = property_instance(synced(), &[("index", -1.0)]);
        assert_eq!(disabled.active_states(), ["s10"]);
        assert!(disabled.handle_event("Cycle", &mut durations));
        assert!(disabled.handle_event("Cycle", &mut durations));
        assert_eq!(disabled.variable("KeepA"), Some(20.0));
        let mut default = property_instance(synced(), &[("mode", 0.0)]);
        assert_eq!(default.active_states(), ["s10"]);
        assert!(default.handle_event("Cycle", &mut durations));
        assert!(default.handle_event("Cycle", &mut durations));
        assert_eq!(default.variable("KeepA"), Some(20.0));
        // A sync-mode binding cannot fabricate an absent authored index.
        let mut no_index = start_property_graph();
        if let Generator::StateMachine { sync_variable, .. } = &mut no_index.generators[0] {
            *sync_variable = None;
        }
        assert_eq!(
            property_instance(no_index, &[("mode", 1.0)]).active_states(),
            ["s10"]
        );
    }

    #[test]
    fn state_start_bindings_map_referenced_graph_local_sync_indices() {
        let mut root = BehaviorGraph::new(
            "Root",
            Some(0),
            vec![Generator::Reference {
                name: "Ref".into(),
                behavior: "g".into(),
            }],
            vec![],
        );
        root.add_variable("Unrelated", VarType::Int, 999.0);
        let mut g = start_property_graph();
        let mode = g.add_variable("ModeInput", VarType::Int, 1.0);
        let index = g.add_variable("IndexInput", VarType::Int, 1.0);
        g.bindings[0] = vec![
            Binding {
                member: "startStateMode".into(),
                variable: mode,
            },
            Binding {
                member: "syncVariableIndex".into(),
                variable: index,
            },
        ];
        let shared = Shared::new(Arc::new(Project {
            graphs: vec![("root".into(), root), ("g".into(), g)],
            character: Some(Character {
                scalar_properties: [("mode".into(), 0.0), ("index".into(), 0.0)].into(),
                ..Default::default()
            }),
        }));
        let mut i = Instance::new(shared, 1);
        i.update(0.0, &mut durations);
        assert_eq!(i.active_states(), ["s30"]);
        assert!(i.handle_event("Cycle", &mut durations));
        assert_eq!(i.variable("KeepB"), Some(10.0));
        assert_eq!(i.variable("KeepA"), Some(20.0));
        assert_eq!(i.variable("Unrelated"), Some(999.0));
    }

    #[test]
    fn state_start_variables_keep_precedence_and_resample_only_on_activation() {
        let mut g = start_property_graph();
        let mode = g.add_variable("ModeInput", VarType::Int, 1.0);
        let index = g.add_variable("IndexInput", VarType::Int, 0.0);
        let selected = g.add_variable("Selected", VarType::Int, 0.0);
        g.bindings[0] = vec![
            Binding {
                member: "startStateMode".into(),
                variable: mode,
            },
            Binding {
                member: "syncVariableIndex".into(),
                variable: index,
            },
        ];
        g.generators.push(Generator::Selector {
            name: "Select".into(),
            children: vec![0, 1],
            index: 0,
        });
        g.bindings.push(vec![Binding {
            member: "selectedGeneratorIndex".into(),
            variable: selected,
        }]);
        g.property_bindings.push(vec![]);
        g.root = Some(4);
        let mut i = property_instance(g, &[("mode", 0.0), ("index", 1.0)]);
        assert_eq!(i.active_states(), ["s20"]);
        i.set_variable("ModeInput", 0.0);
        i.set_variable("IndexInput", 1.0);
        i.set_variable("KeepB", 10.0);
        i.update(0.0, &mut durations);
        assert_eq!(i.active_states(), ["s20"]);
        assert!(i.handle_event("Cycle", &mut durations));
        assert_eq!(i.variable("KeepA"), Some(30.0)); // Frozen output slot.
        assert_eq!(i.variable("KeepB"), Some(10.0));
        let reactivate = |i: &mut Instance| {
            i.set_variable("Selected", 1.0);
            i.update(0.0, &mut durations);
            i.set_variable("Selected", 0.0);
            i.update(0.0, &mut durations);
        };
        reactivate(&mut i);
        assert_eq!(i.active_states(), ["s10"]); // Bound default mode now applies.
        i.set_variable("ModeInput", 1.0);
        assert!(i.handle_event("Cycle", &mut durations));
        assert_eq!(i.variable("KeepB"), Some(10.0)); // Default activation has no sync output.
        i.set_variable("KeepB", 30.0);
        reactivate(&mut i);
        assert_eq!(i.active_states(), ["s30"]);
        assert!(i.handle_event("Cycle", &mut durations));
        assert_eq!(i.variable("KeepB"), Some(10.0));
        assert_eq!(i.variable("KeepA"), Some(30.0));
        // Invalid variables retain authored inputs and cannot fall through to
        // duplicate valid properties.
        i.set_variable("ModeInput", 3.0);
        i.set_variable("IndexInput", f32::NAN);
        reactivate(&mut i);
        assert_eq!(i.active_states(), ["s10"]);
    }

    #[test]
    fn state_start_property_changes_wait_for_reactivation_and_preserve_start_id_precedence() {
        let mut g = start_property_graph();
        g.property_bindings[0].push(property("startStateId", "Start"));
        let mut i = property_instance(g, &[("mode", 1.0), ("index", 0.0), ("start", 30.0)]);
        assert_eq!(i.active_states(), ["s30"]); // Explicit start ID still takes precedence.
        let props = &mut Arc::get_mut(&mut Arc::get_mut(&mut i.shared).unwrap().project)
            .unwrap()
            .character
            .as_mut()
            .unwrap()
            .scalar_properties;
        props.insert("mode".into(), 0.0);
        props.insert("index".into(), 1.0);
        props.remove("start");
        i.update(0.0, &mut durations);
        assert_eq!(i.active_states(), ["s30"]);
        assert!(i.handle_event("Cycle", &mut durations));
        assert_eq!(i.variable("KeepA"), Some(10.0));
        assert_eq!(i.variable("KeepB"), Some(30.0));
        i.root = None;
        i.update(0.0, &mut durations);
        assert_eq!(i.active_states(), ["s10"]);
    }

    fn duration_property_graph() -> BehaviorGraph {
        let mut transition = Transition::new(0, 1, None);
        transition.blend = Some(0.75);
        transition.blend_property = Some("DURATION".into());
        BehaviorGraph::new(
            "G",
            Some(0),
            vec![
                Generator::StateMachine {
                    name: "Root".into(),
                    start: 0,
                    start_variable: None,
                    start_mode: Default::default(),
                    sync_variable: None,
                    change_event: None,
                    states: vec![state(0, 1, vec![transition]), state(1, 2, vec![])],
                    wildcards: vec![],
                },
                clip("Old", ClipMode::Looping, vec![]),
                clip("New", ClipMode::Looping, vec![]),
            ],
            vec!["Switch".into()],
        )
    }

    #[test]
    fn transition_duration_properties_are_per_character_and_read_only() {
        // Existing cross-fades use smoothstep, so a quarter of a 1 s blend
        // gives the new clip weight 0.15625 (rounded by playing()).
        for (duration, old_weight, new_weight) in [(1.0, 0.84, 0.16), (0.5, 0.5, 0.5)] {
            let mut i = property_instance(duration_property_graph(), &[("duration", duration)]);
            assert!(i.handle_event("Switch", &mut durations));
            i.update(0.25, &mut durations);
            assert_eq!(
                playing(&i),
                [("Old".into(), old_weight), ("New".into(), new_weight)]
            );
            assert!(!i.set_variable("Duration", 7.0));
            assert_eq!(i.variable("Duration"), None);
            assert_eq!(
                i.shared
                    .project
                    .character
                    .as_ref()
                    .unwrap()
                    .scalar_properties["duration"],
                duration
            );
            i.update(duration, &mut durations);
            assert_eq!(playing(&i), [("New".into(), 1.0)]);
        }
    }

    #[test]
    fn unavailable_transition_duration_properties_preserve_authored_fallback() {
        let mut non_scalar = property_instance(duration_property_graph(), &[]);
        Arc::get_mut(&mut Arc::get_mut(&mut non_scalar.shared).unwrap().project)
            .unwrap()
            .character
            .as_mut()
            .unwrap()
            .bone_weights
            .insert("duration".into(), vec![0.5]);
        for mut i in [
            instance(duration_property_graph()),
            property_instance(duration_property_graph(), &[]),
            non_scalar,
        ] {
            assert!(i.handle_event("Switch", &mut durations));
            i.update(0.375, &mut durations);
            assert_eq!(playing(&i), [("Old".into(), 0.5), ("New".into(), 0.5)]);
        }
    }

    #[test]
    fn transition_duration_variables_win_and_are_sampled_when_pending_transition_commits() {
        let mut g = duration_property_graph();
        let duration = g.add_variable("Duration", VarType::Real, 0.25);
        let Generator::StateMachine { states, .. } = &mut g.generators[0] else {
            panic!("expected state machine");
        };
        let t = &mut states[0].transitions[0];
        t.blend_variable = Some(duration);
        t.initiate = Some(Interval {
            enter_event: -1,
            exit_event: -1,
            enter_time: 0.5,
            exit_time: 0.0,
        });
        let mut i = property_instance(g, &[("duration", 4.0)]);
        i.handle_event("Switch", &mut durations);
        assert_eq!(playing(&i), [("Old".into(), 1.0)]);
        assert!(i.set_variable("Duration", 1.0));
        i.update(0.5, &mut durations);
        assert_eq!(i.active_states(), ["s1"]);
        assert!(i.set_variable("Duration", 0.0));
        i.update(0.5, &mut durations);
        assert_eq!(playing(&i), [("Old".into(), 0.5), ("New".into(), 0.5)]);
        i.update(0.5, &mut durations);
        assert_eq!(playing(&i), [("New".into(), 1.0)]);
    }

    #[test]
    fn scalar_generator_properties_drive_synchronised_parametric_blends() {
        let mut g = BehaviorGraph::new(
            "G",
            Some(0),
            vec![
                Generator::Blender {
                    name: "Blend".into(),
                    parameter: 0.0,
                    min_cyclic: 0.0,
                    max_cyclic: 1.0,
                    flags: BLEND_PARAMETRIC | BLEND_SYNC,
                    children: vec![
                        BlendChild {
                            generator: Some(1),
                            weight: 0.0,
                            weight_variable: None,
                            weight_property: None,
                            bone_weights: None,
                            bone_weights_property: None,
                        },
                        BlendChild {
                            generator: Some(2),
                            weight: 1.0,
                            weight_variable: None,
                            weight_property: None,
                            bone_weights: None,
                            bone_weights_property: None,
                        },
                    ],
                },
                clip("Enter", ClipMode::SinglePlay, vec![]),
                clip("Loop", ClipMode::SinglePlay, vec![]),
            ],
            vec![],
        );
        g.property_bindings[0] = vec![property("blendParameter", "Blend")];
        g.property_bindings[2] = vec![property("playbackSpeed", "Speed")];
        let mut i = property_instance(g, &[("blend", 0.5), ("speed", 2.0)]);
        assert_eq!(playing(&i), [("Enter".into(), 0.5), ("Loop".into(), 0.5)]);
        i.update(0.25, &mut durations);
        let samples = i.samples();
        // Both child cycles are 1 s after the property scales Loop's 2 s clip.
        assert_eq!(samples[0].time, 0.25);
        assert_eq!(samples[1].time, 0.5);
    }

    fn cyclic_property_graph() -> BehaviorGraph {
        let mut g = BehaviorGraph::new(
            "Cyclic",
            Some(0),
            vec![
                Generator::Wrap {
                    name: "Direction".into(),
                    class: "BSCyclicBlendTransitionGenerator".into(),
                    child: Some(1),
                    modifier: None,
                },
                Generator::Blender {
                    name: "Blend".into(),
                    parameter: 0.125,
                    min_cyclic: 0.0,
                    max_cyclic: 1.0,
                    flags: BLEND_PARAMETRIC | BLEND_CYCLIC | BLEND_SYNC,
                    children: vec![
                        BlendChild {
                            weight: 0.0,
                            ..masked_child(2, None, None)
                        },
                        BlendChild {
                            weight: 0.5,
                            ..masked_child(3, None, None)
                        },
                    ],
                },
                clip("Enter", ClipMode::Looping, vec![]),
                clip("Loop", ClipMode::Looping, vec![]),
            ],
            vec![],
        );
        g.property_bindings[0] = vec![property("fBlendParameter", "DIRECTION")];
        g
    }

    #[test]
    fn cyclic_wrapper_properties_drive_per_character_weights_and_synchronization() {
        for (direction, expected) in [(0.25, [0.5, 0.5]), (1.375, [0.25, 0.75])] {
            let mut g = cyclic_property_graph();
            g.forward_cyclic_blend_inputs();
            g.forward_cyclic_blend_inputs(); // Shared nodes must not duplicate bindings.
            assert_eq!(g.property_bindings[1].len(), 1);
            let mut i = property_instance(g, &[("direction", direction)]);
            assert_eq!(
                playing(&i),
                [("Enter".into(), expected[0]), ("Loop".into(), expected[1])]
            );
            assert!(!i.set_variable("Direction", 0.0));
            i.update(0.25, &mut durations);
            let samples = i.samples();
            let cycle = expected[0] + 2.0 * expected[1];
            assert!((samples[0].time - 0.25 / cycle).abs() < 1e-6);
            assert!((samples[1].time - 0.5 / cycle).abs() < 1e-6);
        }
    }

    #[test]
    fn unavailable_cyclic_wrapper_properties_keep_authored_blender_parameter() {
        let graph = || {
            let mut g = cyclic_property_graph();
            g.forward_cyclic_blend_inputs();
            g
        };
        let mut non_scalar = property_instance(graph(), &[]);
        Arc::get_mut(&mut Arc::get_mut(&mut non_scalar.shared).unwrap().project)
            .unwrap()
            .character
            .as_mut()
            .unwrap()
            .bone_weights
            .insert("direction".into(), vec![0.5]);
        for i in [
            instance(graph()),
            property_instance(graph(), &[]),
            non_scalar,
        ] {
            assert_eq!(playing(&i), [("Enter".into(), 0.75), ("Loop".into(), 0.25)]);
        }
    }

    #[test]
    fn cyclic_wrapper_variables_keep_precedence_and_update_live() {
        let mut g = cyclic_property_graph();
        let direction = g.add_variable("Direction", VarType::Real, 0.0);
        g.bindings[0] = vec![Binding {
            member: "fBlendParameter".into(),
            variable: direction,
        }];
        g.forward_cyclic_blend_inputs();
        assert!(g.property_bindings[1].is_empty());
        let mut i = property_instance(g, &[("direction", 0.25)]);
        assert_eq!(playing(&i), [("Enter".into(), 1.0)]);
        assert!(i.set_variable("Direction", 0.5));
        i.update(0.0, &mut durations);
        assert_eq!(playing(&i), [("Loop".into(), 1.0)]);
    }

    fn cyclic_bounds_graph(wrapped: bool) -> BehaviorGraph {
        let mut g = cyclic_property_graph();
        if !wrapped {
            g.root = Some(1);
            g.property_bindings[1] = vec![property("blendParameter", "Direction")];
        }
        g.property_bindings[1].extend([
            property("minCyclicBlendParameter", "MINIMUM"),
            property("maxCyclicBlendParameter", "MAXIMUM"),
        ]);
        g.forward_cyclic_blend_inputs();
        g
    }

    #[test]
    fn cyclic_bound_properties_drive_direct_and_wrapped_weights_and_sync() {
        for wrapped in [false, true] {
            for (minimum, maximum, expected) in [(0.0, 2.0, [0.5, 0.5]), (-0.5, 1.0, [0.75, 0.25])]
            {
                let mut i = property_instance(
                    cyclic_bounds_graph(wrapped),
                    &[
                        ("direction", 1.25),
                        ("minimum", minimum),
                        ("maximum", maximum),
                    ],
                );
                assert_eq!(
                    playing(&i),
                    [("Enter".into(), expected[0]), ("Loop".into(), expected[1])]
                );
                assert!(!i.set_variable("Minimum", 99.0));
                i.update(0.375, &mut durations);
                let samples = i.samples();
                let cycle = expected[0] + 2.0 * expected[1];
                assert!((samples[0].time - 0.375 / cycle).abs() < 1e-6);
                assert!((samples[1].time - 0.75 / cycle).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn cyclic_bound_variables_take_precedence_and_update_live() {
        for wrapped in [false, true] {
            let mut g = cyclic_bounds_graph(wrapped);
            let minimum = g.add_variable("Minimum", VarType::Real, 0.0);
            let maximum = g.add_variable("Maximum", VarType::Real, 2.0);
            g.bindings[1] = vec![
                Binding {
                    member: "minCyclicBlendParameter".into(),
                    variable: minimum,
                },
                Binding {
                    member: "maxCyclicBlendParameter".into(),
                    variable: maximum,
                },
            ];
            let mut i = property_instance(
                g,
                &[("direction", 1.25), ("minimum", -0.5), ("maximum", 1.0)],
            );
            assert_eq!(playing(&i), [("Enter".into(), 0.5), ("Loop".into(), 0.5)]);
            assert!(i.set_variable("Minimum", -0.5));
            assert!(i.set_variable("Maximum", 1.0));
            i.update(0.0, &mut durations);
            assert_eq!(playing(&i), [("Enter".into(), 0.75), ("Loop".into(), 0.25)]);
            i.update(0.3125, &mut durations);
            let samples = i.samples();
            assert_eq!(samples[0].time, 0.25);
            assert_eq!(samples[1].time, 0.5);
            // An invalid variable still wins over its property; its authored
            // endpoint is used instead of falling through to that property.
            assert!(i.set_variable("Minimum", f32::NAN));
            assert!(i.set_variable("Maximum", f32::INFINITY));
            i.update(0.0, &mut durations);
            assert_eq!(playing(&i), [("Enter".into(), 0.5), ("Loop".into(), 0.5)]);
        }
    }

    #[test]
    fn unavailable_cyclic_bound_properties_keep_authored_endpoints() {
        for wrapped in [false, true] {
            for character in [
                None,
                Some(Character::default()),
                Some(Character {
                    bone_weights: [
                        ("minimum".into(), vec![-1.0]),
                        ("maximum".into(), vec![2.0]),
                    ]
                    .into(),
                    ..Default::default()
                }),
            ] {
                let mut g = cyclic_bounds_graph(wrapped);
                // Keep the parameter independent of the unavailable character.
                g.property_bindings[1].retain(|b| b.member != "blendParameter");
                let Generator::Blender { parameter, .. } = &mut g.generators[1] else {
                    unreachable!();
                };
                *parameter = 1.25;
                let shared = Shared::new(Arc::new(Project {
                    graphs: vec![("g".into(), g)],
                    character,
                }));
                let mut i = Instance::new(shared, 1);
                i.update(0.375, &mut durations);
                assert_eq!(playing(&i), [("Enter".into(), 0.5), ("Loop".into(), 0.5)]);
                assert_eq!(i.samples()[0].time, 0.25);
                assert_eq!(i.samples()[1].time, 0.5);
            }
        }
    }

    #[test]
    fn invalid_cyclic_bounds_use_finite_fallbacks_or_disable_wrapping() {
        for wrapped in [false, true] {
            for (minimum, maximum, expected) in [
                (
                    f32::NAN,
                    f32::INFINITY,
                    vec![("Enter".into(), 0.5), ("Loop".into(), 0.5)],
                ),
                (0.0, 0.0, vec![("Loop".into(), 1.0)]),
                (1.0, 0.0, vec![("Loop".into(), 1.0)]),
                (-f32::MAX, f32::MAX, vec![("Loop".into(), 1.0)]),
            ] {
                let mut i = property_instance(
                    cyclic_bounds_graph(wrapped),
                    &[
                        ("direction", 1.25),
                        ("minimum", minimum),
                        ("maximum", maximum),
                    ],
                );
                assert_eq!(playing(&i), expected);
                i.update(0.25, &mut durations);
                assert!(
                    i.samples()
                        .iter()
                        .all(|s| s.weight.is_finite() && s.time.is_finite())
                );
            }
            let mut g = cyclic_bounds_graph(wrapped);
            let Generator::Blender { max_cyclic, .. } = &mut g.generators[1] else {
                unreachable!();
            };
            *max_cyclic = f32::INFINITY;
            let mut i = property_instance(g, &[("direction", 1.25)]);
            assert_eq!(playing(&i), [("Loop".into(), 1.0)]);
            i.update(0.25, &mut durations);
            assert_eq!(i.samples()[0].time, 0.25);
        }
    }

    #[test]
    fn finite_cyclic_bounds_wrap_extreme_parameters_without_overflow() {
        for wrapped in [false, true] {
            // The first case overflows p - minimum in f32. The second
            // overflows p + period and first-child-position + period.
            for (minimum, maximum, direction, positions) in [
                (
                    -f32::MAX,
                    -f32::MAX / 2.0,
                    f32::MAX,
                    [-f32::MAX, -f32::MAX * 0.75],
                ),
                (
                    f32::MAX / 2.0,
                    f32::MAX,
                    f32::MAX * 0.875,
                    [f32::MAX / 2.0, f32::MAX * 0.75],
                ),
            ] {
                let mut g = cyclic_bounds_graph(wrapped);
                let Generator::Blender { children, .. } = &mut g.generators[1] else {
                    unreachable!();
                };
                for (child, position) in children.iter_mut().zip(positions) {
                    child.weight = position;
                }
                let mut i = property_instance(
                    g,
                    &[
                        ("direction", direction),
                        ("minimum", minimum),
                        ("maximum", maximum),
                    ],
                );
                let samples = i.samples();
                assert!(!samples.is_empty());
                assert!(
                    samples
                        .iter()
                        .all(|s| s.weight.is_finite() && s.weight >= 0.0)
                );
                assert!((samples.iter().map(|s| s.weight).sum::<f32>() - 1.0).abs() < 1e-6);
                let cycle: f32 = samples
                    .iter()
                    .map(|s| s.weight * durations(&s.animation).unwrap())
                    .sum();
                i.update(0.25, &mut durations);
                for sample in i.samples() {
                    let expected = 0.25 / cycle * durations(&sample.animation).unwrap();
                    assert!((sample.time - expected).abs() < 1e-6);
                }
            }
        }
    }

    #[test]
    fn cyclic_wrapper_properties_preserve_explicit_child_inputs_and_other_wrappers() {
        let mut direct = cyclic_property_graph();
        direct.property_bindings[1] = vec![property("blendParameter", "Direct")];
        direct.forward_cyclic_blend_inputs();
        assert_eq!(direct.property_bindings[1].len(), 1);
        assert_eq!(
            playing(&property_instance(
                direct,
                &[("direction", 0.25), ("direct", 0.5)]
            )),
            [("Loop".into(), 1.0)]
        );

        let mut variable = cyclic_property_graph();
        let direct = variable.add_variable("Direct", VarType::Real, 0.0);
        variable.bindings[1] = vec![Binding {
            member: "blendParameter".into(),
            variable: direct,
        }];
        variable.forward_cyclic_blend_inputs();
        assert!(variable.property_bindings[1].is_empty());
        assert_eq!(
            playing(&property_instance(variable, &[("direction", 0.25)])),
            [("Enter".into(), 1.0)]
        );

        let mut unrelated = cyclic_property_graph();
        let Generator::Wrap { class, .. } = &mut unrelated.generators[0] else {
            unreachable!();
        };
        *class = "BSiStateTaggingGenerator".into();
        unrelated.forward_cyclic_blend_inputs();
        assert!(unrelated.property_bindings[1].is_empty());

        let mut non_blender = cyclic_property_graph();
        non_blender.generators[1] = clip("Enter", ClipMode::Looping, vec![]);
        non_blender.forward_cyclic_blend_inputs();
        assert!(non_blender.property_bindings[1].is_empty());
    }

    #[test]
    fn unavailable_generator_properties_keep_authored_inputs() {
        let mut c = clip("Loop", ClipMode::SinglePlay, vec![]);
        if let Generator::Clip {
            start_time, speed, ..
        } = &mut c
        {
            *start_time = 0.5;
            *speed = 2.0;
        }
        let mut g = BehaviorGraph::new("G", Some(0), vec![c], vec![]);
        g.property_bindings[0] = vec![
            property("startTime", "Absent"),
            property("playbackSpeed", "NonScalar"),
        ];
        let mut i = property_instance(g, &[]);
        assert_eq!(time(&i), 0.5);
        i.update(0.25, &mut durations);
        assert_eq!(time(&i), 1.0);
    }

    fn masked_blend(name: &str, children: Vec<BlendChild>) -> Generator {
        Generator::Blender {
            name: name.into(),
            parameter: 0.0,
            min_cyclic: 0.0,
            max_cyclic: 1.0,
            flags: 0,
            children,
        }
    }

    fn masked_child(
        generator: GenId,
        property: Option<&str>,
        mask: Option<Vec<f32>>,
    ) -> BlendChild {
        BlendChild {
            generator: Some(generator),
            weight: 1.0,
            weight_variable: None,
            weight_property: None,
            bone_weights: mask,
            bone_weights_property: property.map(String::from),
        }
    }

    fn child_weight_graph(flags: u16) -> BehaviorGraph {
        let mut first = masked_child(1, None, None);
        first.weight_property = Some("WEIGHT".into());
        let mut second = masked_child(2, None, None);
        second.weight = 3.0;
        BehaviorGraph::new(
            "Weights",
            Some(0),
            vec![
                Generator::Blender {
                    name: "Blend".into(),
                    parameter: 1.0,
                    min_cyclic: 0.0,
                    max_cyclic: 4.0,
                    flags,
                    children: vec![first, second],
                },
                clip("Enter", ClipMode::Looping, vec![]),
                clip("Loop", ClipMode::Looping, vec![]),
            ],
            vec![],
        )
    }

    fn flags_property_graph(authored: u16) -> BehaviorGraph {
        let mut g = child_weight_graph(authored);
        let Generator::Blender {
            parameter,
            children,
            ..
        } = &mut g.generators[0]
        else {
            unreachable!();
        };
        *parameter = 0.0;
        children[0].weight_property = None;
        g.property_bindings[0] = vec![property("flags", "BlendFlags")];
        g.events = vec!["First".into(), "Second".into()];
        for (id, time, event) in [(1, 0.2, 0), (2, 0.4, 1)] {
            let Generator::Clip { triggers, .. } = &mut g.generators[id] else {
                unreachable!();
            };
            triggers.push(Trigger {
                time,
                from_end: false,
                event,
                payload: None,
            });
        }
        g
    }

    #[test]
    fn blender_flag_properties_share_pose_sync_and_trigger_inputs() {
        for (flags, weights, times, events) in [
            (
                0,
                vec![("Enter".into(), 0.25), ("Loop".into(), 0.75)],
                vec![0.375, 0.375],
                vec!["First"],
            ),
            (
                BLEND_PARAMETRIC,
                vec![("Enter".into(), 1.0)],
                vec![0.375],
                vec!["First"],
            ),
            (
                BLEND_PARAMETRIC | BLEND_CYCLIC,
                vec![("Enter".into(), 0.5), ("Loop".into(), 0.5)],
                vec![0.375, 0.375],
                vec!["First"],
            ),
            (
                BLEND_SYNC,
                vec![("Enter".into(), 0.25), ("Loop".into(), 0.75)],
                vec![0.375 / 1.75, 0.75 / 1.75],
                vec!["Second"],
            ),
            (
                BLEND_SYNC | BLEND_PARAMETRIC | BLEND_CYCLIC,
                vec![("Enter".into(), 0.5), ("Loop".into(), 0.5)],
                vec![0.25, 0.5],
                vec!["Second"],
            ),
            (
                0x8000,
                vec![("Enter".into(), 0.25), ("Loop".into(), 0.75)],
                vec![0.375, 0.375],
                vec!["First"],
            ),
        ] {
            let mut i = property_instance(flags_property_graph(0), &[("blendflags", flags as f32)]);
            assert!(!i.set_variable("BlendFlags", 0.0));
            assert_eq!(playing(&i), weights);
            i.update(0.375, &mut durations);
            for (sample, expected) in i.samples().iter().zip(times) {
                assert!((sample.time - expected).abs() < 1e-6);
            }
            assert_eq!(
                i.take_raised()
                    .iter()
                    .map(|e| e.event.as_str())
                    .collect::<Vec<_>>(),
                events
            );
            assert_eq!(
                i.shared
                    .project
                    .character
                    .as_ref()
                    .unwrap()
                    .scalar_properties["blendflags"],
                flags as f32
            );
        }

        let mut i = property_instance(
            flags_property_graph(0),
            &[(
                "blendflags",
                (BLEND_SYNC | BLEND_PARAMETRIC | BLEND_CYCLIC) as f32,
            )],
        );
        i.update(1.875, &mut durations);
        assert_eq!(i.samples()[0].time, 0.25);
        assert_eq!(i.samples()[1].time, 0.5);
        assert!(i.samples().iter().all(|s| s.wrapped));
    }

    #[test]
    fn blender_flags_keep_authored_fallbacks_and_live_variable_precedence() {
        for character in [
            None,
            Some(Character::default()),
            Some(Character {
                bone_weights: [("blendflags".into(), vec![0.0])].into(),
                ..Default::default()
            }),
        ] {
            let mut i = Instance::new(
                Shared::new(Arc::new(Project {
                    graphs: vec![("g".into(), flags_property_graph(BLEND_PARAMETRIC))],
                    character,
                })),
                1,
            );
            i.update(0.0, &mut durations);
            assert_eq!(playing(&i), [("Enter".into(), 1.0)]);
        }
        for bad in [
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            -1.0,
            65536.0,
            16.5,
        ] {
            let mut g = flags_property_graph(BLEND_PARAMETRIC);
            let variable = g.add_variable("Flags", VarType::Int, BLEND_PARAMETRIC as f32);
            g.bindings[0] = vec![Binding {
                member: "flags".into(),
                variable,
            }];
            let mut i = property_instance(g, &[("blendflags", 0.0)]);
            assert_eq!(playing(&i), [("Enter".into(), 1.0)]);
            assert!(i.set_variable("Flags", bad));
            assert_eq!(playing(&i), [("Enter".into(), 1.0)]);
            assert!(i.set_variable("Flags", (BLEND_PARAMETRIC | BLEND_CYCLIC) as f32));
            assert_eq!(playing(&i), [("Enter".into(), 0.5), ("Loop".into(), 0.5)]);
            assert!(i.set_variable("Flags", 0.0));
            assert_eq!(playing(&i), [("Enter".into(), 0.25), ("Loop".into(), 0.75)]);

            // Invalid properties also preserve the authored member.
            let i = property_instance(
                flags_property_graph(BLEND_PARAMETRIC),
                &[("blendflags", bad)],
            );
            assert_eq!(playing(&i), [("Enter".into(), 1.0)]);
        }
    }

    #[test]
    fn nested_blender_flag_properties_drive_parent_cycle_and_master_selection() {
        let mut g = flags_property_graph(0);
        g.generators
            .push(masked_blend("Outer", vec![masked_child(0, None, None)]));
        let Generator::Blender { flags, .. } = &mut g.generators[3] else {
            unreachable!()
        };
        *flags = BLEND_SYNC;
        g.bindings.push(vec![]);
        g.property_bindings.push(vec![]);
        g.root = Some(3);
        let mut i = property_instance(
            g,
            &[("blendflags", (BLEND_PARAMETRIC | BLEND_CYCLIC) as f32)],
        );
        i.update(0.375, &mut durations);
        assert_eq!(playing(&i), [("Enter".into(), 0.5), ("Loop".into(), 0.5)]);
        assert_eq!(i.samples()[0].time, 0.25);
        assert_eq!(i.samples()[1].time, 0.5);
        assert_eq!(
            i.take_raised()
                .iter()
                .map(|e| e.event.as_str())
                .collect::<Vec<_>>(),
            ["Second"]
        );
    }

    #[test]
    fn blender_flag_properties_preserve_existing_additive_normalization() {
        struct AdditiveClips;
        impl ClipSource for AdditiveClips {
            fn duration(&mut self, animation: &str) -> Option<f32> {
                durations(animation)
            }
            fn additive(&mut self, animation: &str) -> bool {
                animation == "Loop"
            }
        }
        let shared = Shared::new(Arc::new(Project {
            graphs: vec![("g".into(), flags_property_graph(BLEND_PARAMETRIC))],
            character: Some(Character {
                scalar_properties: [("blendflags".into(), 0.0)].into(),
                ..Default::default()
            }),
        }));
        let mut i = Instance::new(shared, 1);
        i.update(0.0, &mut AdditiveClips);
        let samples = i.samples();
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0].weight, 1.0);
        assert!(!samples[0].additive);
        assert_eq!(samples[1].weight, 1.0); // Existing additive clamp of authored weight 3.
        assert!(samples[1].additive);
    }

    #[test]
    fn scalar_blender_child_properties_normalize_per_character_and_stay_read_only() {
        for (weight, expected) in [(1.0, 0.25), (3.0, 0.5)] {
            let mut i = property_instance(child_weight_graph(0), &[("weight", weight)]);
            assert_eq!(
                playing(&i),
                [("Enter".into(), expected), ("Loop".into(), 1.0 - expected)]
            );
            assert!(!i.set_variable("Weight", 99.0));
            i.update(0.25, &mut durations);
            assert_eq!(i.samples()[0].weight, expected);
            assert_eq!(
                i.shared
                    .project
                    .character
                    .as_ref()
                    .unwrap()
                    .scalar_properties["weight"],
                weight
            );
        }
        let i = property_instance(child_weight_graph(0), &[("weight", -2.0)]);
        assert_eq!(playing(&i), [("Loop".into(), 1.0)]);
    }

    #[test]
    fn scalar_blender_child_properties_preserve_live_variable_precedence() {
        let mut g = child_weight_graph(0);
        let weight = g.add_variable("Weight", VarType::Real, 9.0);
        let Generator::Blender { children, .. } = &mut g.generators[0] else {
            unreachable!();
        };
        children[0].weight_variable = Some(weight);
        let mut i = property_instance(g, &[("weight", 3.0)]);
        assert_eq!(playing(&i), [("Enter".into(), 0.75), ("Loop".into(), 0.25)]);
        assert!(i.set_variable("Weight", 0.0));
        i.update(0.0, &mut durations);
        assert_eq!(playing(&i), [("Loop".into(), 1.0)]);
        assert!(i.set_variable("Weight", 1.0));
        i.update(0.0, &mut durations);
        assert_eq!(playing(&i), [("Enter".into(), 0.25), ("Loop".into(), 0.75)]);
    }

    #[test]
    fn unavailable_scalar_blender_child_properties_keep_authored_weights() {
        for character in [
            None,
            Some(Character::default()),
            Some(Character {
                bone_weights: [("weight".into(), vec![99.0])].into(),
                ..Default::default()
            }),
        ] {
            let shared = Shared::new(Arc::new(Project {
                graphs: vec![("g".into(), child_weight_graph(0))],
                character,
            }));
            let mut i = Instance::new(shared, 1);
            i.update(0.0, &mut durations);
            assert_eq!(playing(&i), [("Enter".into(), 0.25), ("Loop".into(), 0.75)]);
        }
    }

    #[test]
    fn scalar_blender_child_properties_drive_parametric_positions_and_sync() {
        let mut i = property_instance(
            child_weight_graph(BLEND_PARAMETRIC | BLEND_SYNC),
            &[("weight", -1.0)],
        );
        assert_eq!(playing(&i), [("Enter".into(), 0.5), ("Loop".into(), 0.5)]);
        i.update(0.75, &mut durations);
        let samples = i.samples();
        // Halfway through the 1.5 s weighted cycle: 1 s and 2 s clips
        // follow the same phase using the property-derived weights.
        assert!((samples[0].time - 0.5).abs() < 1e-6);
        assert!((samples[1].time - 1.0).abs() < 1e-6);
    }

    #[test]
    fn scalar_blender_child_properties_keep_additive_weights_out_of_normalization() {
        struct AdditiveClips;
        impl ClipSource for AdditiveClips {
            fn duration(&mut self, animation: &str) -> Option<f32> {
                durations(animation)
            }

            fn additive(&mut self, animation: &str) -> bool {
                animation == "Loop"
            }
        }

        for (overlay, expected) in [(0.25, 0.25), (2.0, 1.0), (-1.0, 0.0)] {
            let mut g = child_weight_graph(0);
            let Generator::Blender { children, .. } = &mut g.generators[0] else {
                unreachable!();
            };
            children[1].weight_property = Some("Overlay".into());
            let shared = Shared::new(Arc::new(Project {
                graphs: vec![("g".into(), g)],
                character: Some(Character {
                    scalar_properties: [("weight".into(), 3.0), ("overlay".into(), overlay)].into(),
                    ..Default::default()
                }),
            }));
            let mut i = Instance::new(shared, 1);
            i.update(0.0, &mut AdditiveClips);
            let samples = i.samples();
            assert_eq!(samples[0].animation.as_ref(), "Enter");
            assert_eq!(samples[0].weight, 1.0);
            assert!(!samples[0].additive);
            assert_eq!(samples.get(1).map_or(0.0, |s| s.weight), expected);
            if let Some(additive) = samples.get(1) {
                assert!(additive.additive);
            }
        }
    }

    #[test]
    fn blender_character_masks_replace_authored_masks_and_compose_with_parent() {
        let graph = || {
            BehaviorGraph::new(
                "Masks",
                Some(0),
                vec![
                    masked_blend("Parent", vec![masked_child(1, Some("OUTER"), None)]),
                    masked_blend(
                        "Inner",
                        vec![
                            masked_child(2, Some("Arm"), Some(vec![0.0, 0.0, 0.0])),
                            masked_child(3, None, None),
                        ],
                    ),
                    clip("Arm", ClipMode::Looping, vec![]),
                    clip("Full", ClipMode::Looping, vec![]),
                ],
                vec![],
            )
        };
        // The same graph has a different resolved mask for each character.
        for (arm, expected) in [
            (vec![1.0, 0.5, 0.0], vec![0.5, 0.5, 0.0]),
            (vec![0.0, 0.25, 1.0], vec![0.0, 0.25, 0.25]),
        ] {
            let shared = Shared::new(Arc::new(Project {
                graphs: vec![("g".into(), graph())],
                character: Some(Character {
                    bone_weights: [("outer".into(), vec![0.5, 1.0, 0.25]), ("arm".into(), arm)]
                        .into(),
                    ..Default::default()
                }),
            }));
            let mut i = Instance::new(shared, 1);
            i.update(0.1, &mut durations);
            let samples = i.samples();
            assert_eq!(samples.len(), 2);
            assert_eq!(samples[0].mask.as_deref(), Some(expected.as_slice()));
            assert_eq!(
                samples[1].mask.as_deref(),
                Some([0.5, 1.0, 0.25].as_slice())
            );
            assert_eq!(samples[0].weight, 0.5);
            assert_eq!(samples[1].weight, 0.5);
        }
    }

    #[test]
    fn unavailable_blender_character_masks_keep_authored_fallbacks() {
        for character in [
            None,
            Some(Character::default()),
            Some(Character {
                // A scalar property is not a bone-weight array, even with the same name.
                scalar_properties: [("mask".into(), 1.0)].into(),
                ..Default::default()
            }),
        ] {
            let g = BehaviorGraph::new(
                "Fallback",
                Some(0),
                vec![
                    masked_blend(
                        "Blend",
                        vec![
                            masked_child(1, Some("Mask"), Some(vec![0.25, 1.0])),
                            masked_child(2, Some("Mask"), None),
                        ],
                    ),
                    clip("Authored", ClipMode::Looping, vec![]),
                    clip("Unmasked", ClipMode::Looping, vec![]),
                ],
                vec![],
            );
            let shared = Shared::new(Arc::new(Project {
                graphs: vec![("g".into(), g)],
                character,
            }));
            let mut i = Instance::new(shared, 1);
            i.update(0.0, &mut durations);
            let samples = i.samples();
            assert_eq!(samples[0].mask.as_deref(), Some([0.25, 1.0].as_slice()));
            assert!(samples[1].mask.is_none());
        }
    }

    #[test]
    fn character_rotations_follow_scalar_inputs_and_event_lifecycle() {
        let mut g = BehaviorGraph::new(
            "Rotation",
            Some(0),
            vec![Generator::Wrap {
                name: "Wrapped".into(),
                class: "hkbModifierGenerator".into(),
                child: None,
                modifier: Some(0),
            }],
            vec!["On".into(), "Off".into()],
        );
        let rate = g.add_variable("Rate", VarType::Real, -45.0);
        let multiplier = g.add_variable("Multiplier", VarType::Real, 0.5);
        let enabled = g.add_variable("Enabled", VarType::Bool, 1.0);
        g.modifiers = vec![
            Modifier::EventDriven {
                child: Some(1),
                activate: 0,
                deactivate: 1,
                active_by_default: false,
            },
            Modifier::RotateCharacter(RotateCharacter {
                enabled: false,
                degrees_per_second: 90.0,
                speed_multiplier: 1.0,
                axis: glam::Vec4::new(0.0, 0.0, 1.0, 0.0),
            }),
        ];
        g.modifier_bindings = vec![
            vec![],
            vec![
                Binding {
                    member: "degreesPerSecond".into(),
                    variable: rate,
                },
                Binding {
                    member: "speedMultiplier".into(),
                    variable: multiplier,
                },
                Binding {
                    member: "enable".into(),
                    variable: enabled,
                },
            ],
        ];
        let mut i = instance(g);
        assert!(i.character_rotations().is_empty());
        i.handle_event("On", &mut durations);
        let r = i.character_rotations();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].degrees_per_second, -45.0);
        assert_eq!(r[0].speed_multiplier, 0.5);
        assert_eq!(r[0].axis, glam::Vec4::new(0.0, 0.0, 1.0, 0.0));
        i.set_variable("Rate", 30.0);
        i.set_variable("Multiplier", -2.0);
        assert_eq!(i.character_rotations()[0].degrees_per_second, 30.0);
        assert_eq!(i.character_rotations()[0].speed_multiplier, -2.0);
        i.set_variable("Enabled", 0.0);
        assert!(i.character_rotations().is_empty());
        i.set_variable("Enabled", f32::NAN);
        assert!(i.character_rotations().is_empty());
        i.set_variable("Enabled", 1.0);
        assert_eq!(i.character_rotations().len(), 1);
        i.handle_event("Off", &mut durations);
        assert!(i.character_rotations().is_empty());
    }

    #[test]
    fn character_turn_scales_rate_by_multiplier_axis_and_time() {
        let turn = |rate: f32, mult: f32, axis: glam::Vec4, dt: f32| {
            let mut g = BehaviorGraph::new(
                "Rotation",
                Some(0),
                vec![Generator::Wrap {
                    name: "Wrapped".into(),
                    class: "hkbModifierGenerator".into(),
                    child: None,
                    modifier: Some(0),
                }],
                vec![],
            );
            g.modifiers = vec![Modifier::RotateCharacter(RotateCharacter {
                enabled: true,
                degrees_per_second: rate,
                speed_multiplier: mult,
                axis,
            })];
            g.modifier_bindings = vec![vec![]];
            instance(g).character_turn(dt)
        };
        let up = glam::Vec4::Z;
        assert!((turn(90.0, 1.0, up, 1.0) - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
        assert!((turn(90.0, 0.5, up, 0.5) - std::f32::consts::FRAC_PI_2 / 4.0).abs() < 1e-5);
        assert!(turn(-90.0, 1.0, up, 1.0) < 0.0);
        // A downward axis reverses the turn; a longer one is normalized; a horizontal or
        // zero one does not yaw.
        assert!(turn(90.0, 1.0, -up, 1.0) < 0.0);
        let long = glam::Vec4::new(0.0, 0.0, 4.0, 0.0);
        assert!((turn(90.0, 1.0, long, 1.0) - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
        assert_eq!(turn(90.0, 1.0, glam::Vec4::X, 1.0), 0.0);
        assert_eq!(turn(90.0, 1.0, glam::Vec4::ZERO, 1.0), 0.0);
        assert_eq!(turn(f32::NAN, 1.0, up, 1.0), 0.0);
    }

    #[test]
    fn character_rotations_retain_authored_inputs_and_omit_unsupported_bindings() {
        for case in 0..5 {
            let enabled = case != 1;
            let mut g = BehaviorGraph::new(
                "Rotation",
                Some(0),
                vec![Generator::Wrap {
                    name: "Wrapped".into(),
                    class: "hkbModifierGenerator".into(),
                    child: None,
                    modifier: Some(0),
                }],
                vec![],
            );
            g.modifiers = vec![Modifier::RotateCharacter(RotateCharacter {
                enabled,
                degrees_per_second: -60.0,
                speed_multiplier: 0.75,
                axis: glam::Vec4::new(2.0, 3.0, 4.0, 5.0),
            })];
            let axis = g.add_variable("Axis", VarType::Real, 1.0);
            g.modifier_bindings = vec![if case == 2 {
                vec![Binding {
                    member: "axisOfRotation".into(),
                    variable: axis,
                }]
            } else {
                vec![]
            }];
            g.modifier_property_bindings = vec![match case {
                3 => vec![PropertyBinding {
                    member: "axisOfRotation".into(),
                    property: "Axis".into(),
                }],
                4 => vec![PropertyBinding {
                    member: "degreesPerSecond".into(),
                    property: "MissingRate".into(),
                }],
                _ => vec![],
            }];
            let r = instance(g).character_rotations();
            if case == 0 {
                assert_eq!(r.len(), 1);
                assert_eq!(r[0].degrees_per_second, -60.0);
                assert_eq!(r[0].speed_multiplier, 0.75);
                assert_eq!(r[0].axis, glam::Vec4::new(2.0, 3.0, 4.0, 5.0));
            } else {
                assert!(r.is_empty());
            }
        }
    }

    #[test]
    fn character_rotation_properties_supply_inputs_with_variable_precedence() {
        let mut g = BehaviorGraph::new(
            "Rotation",
            Some(0),
            vec![Generator::Wrap {
                name: "Wrapped".into(),
                class: "hkbModifierGenerator".into(),
                child: None,
                modifier: Some(0),
            }],
            vec![],
        );
        g.modifiers = vec![Modifier::RotateCharacter(RotateCharacter {
            enabled: false,
            degrees_per_second: 90.0,
            speed_multiplier: 1.0,
            axis: glam::Vec4::Z,
        })];
        let multiplier = g.add_variable("Multiplier", VarType::Real, -0.25);
        g.modifier_bindings = vec![vec![Binding {
            member: "speedMultiplier".into(),
            variable: multiplier,
        }]];
        g.modifier_property_bindings = vec![vec![
            PropertyBinding {
                member: "enable".into(),
                property: "Enabled".into(),
            },
            PropertyBinding {
                member: "degreesPerSecond".into(),
                property: "Rate".into(),
            },
            PropertyBinding {
                member: "speedMultiplier".into(),
                property: "Multiplier".into(),
            },
        ]];
        let mut character = Character::default();
        character.scalar_properties = HashMap::from([
            ("enabled".into(), 1.0),
            ("rate".into(), -120.0),
            ("multiplier".into(), 2.0),
        ]);
        let mut i = Instance::new(
            Shared::new(Arc::new(Project {
                graphs: vec![("root.hkx".into(), g)],
                character: Some(character),
            })),
            1,
        );
        i.update(0.0, &mut durations);
        let r = i.character_rotations();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].degrees_per_second, -120.0);
        assert_eq!(r[0].speed_multiplier, -0.25);
        assert_eq!(
            i.variable("Rate"),
            None,
            "property stays separate from variables"
        );
        i.set_variable("Multiplier", 0.5);
        assert_eq!(i.character_rotations()[0].speed_multiplier, 0.5);
    }

    #[test]
    fn twists_resolve_angle_enable_and_event_driven_activation() {
        let mut g = BehaviorGraph::new(
            "Twist",
            Some(0),
            vec![
                Generator::Wrap {
                    name: "Wrapped".into(),
                    class: "hkbModifierGenerator".into(),
                    child: Some(1),
                    modifier: Some(0),
                },
                clip("Loop", ClipMode::Looping, vec![]),
            ],
            vec!["On".into(), "Off".into()],
        );
        let angle = g.add_variable("Angle", VarType::Real, 20.0);
        let enabled = g.add_variable("Enabled", VarType::Bool, 1.0);
        g.modifiers = vec![
            Modifier::EventDriven {
                child: Some(1),
                activate: 0,
                deactivate: 1,
                active_by_default: false,
            },
            Modifier::Twist(Twist {
                enabled: false,
                axis: glam::Vec3::X,
                angle_degrees: 5.0,
                start_bone: 1,
                end_bone: 1,
                set_angle_method: 0,
                axis_coordinates: 0,
                additive: true,
            }),
        ];
        g.modifier_bindings = vec![
            vec![],
            vec![
                Binding {
                    member: "twistAngle".into(),
                    variable: angle,
                },
                Binding {
                    member: "enable".into(),
                    variable: enabled,
                },
            ],
        ];
        let mut i = instance(g);
        assert!(i.twists().is_empty());
        i.handle_event("On", &mut durations);
        assert_eq!(i.twists()[0].angle_degrees, 20.0);
        i.set_variable("Angle", -40.0);
        assert_eq!(i.twists()[0].angle_degrees, -40.0);
        i.set_variable("Enabled", 0.0);
        assert!(i.twists().is_empty());
        i.set_variable("Enabled", 1.0);
        assert_eq!(i.twists().len(), 1);
        i.handle_event("Off", &mut durations);
        assert!(i.twists().is_empty());
    }

    #[test]
    fn character_properties_gate_updates_and_poses_without_becoming_variables() {
        fn character_instance(enabled: bool) -> Instance {
            let mut g = BehaviorGraph::new(
                "Properties",
                Some(0),
                vec![Generator::Wrap {
                    name: "Wrapped".into(),
                    class: "hkbModifierGenerator".into(),
                    child: None,
                    modifier: Some(0),
                }],
                vec![],
            );
            g.add_variable("Result", VarType::Real, 0.0);
            g.modifiers = vec![
                Modifier::List(vec![1, 2]),
                Modifier::Expressions(vec!["Result = 7".into()]),
                Modifier::Twist(Twist {
                    enabled: true,
                    axis: glam::Vec3::Z,
                    angle_degrees: 9.0,
                    start_bone: 0,
                    end_bone: 0,
                    set_angle_method: 0,
                    axis_coordinates: 0,
                    additive: true,
                }),
            ];
            g.modifier_bindings = vec![vec![], vec![], vec![]];
            let gate = || PropertyBinding {
                member: "enable".into(),
                property: "ISMALE".into(),
            };
            g.modifier_property_bindings = vec![
                vec![],
                vec![gate()],
                vec![
                    gate(),
                    PropertyBinding {
                        member: "twistAngle".into(),
                        property: "angle".into(),
                    },
                ],
            ];
            let character = Character {
                scalar_properties: [
                    ("ismale".into(), f32::from(enabled)),
                    ("angle".into(), -25.0),
                ]
                .into_iter()
                .collect(),
                ..Default::default()
            };
            let project = Project {
                graphs: vec![("root.hkx".into(), g)],
                character: Some(character),
            };
            Instance::new(Shared::new(Arc::new(project)), 1)
        }
        let mut enabled = character_instance(true);
        let mut disabled = character_instance(false);
        enabled.update(0.1, &mut durations);
        disabled.update(0.1, &mut durations);
        assert_eq!(enabled.variable("Result"), Some(7.0));
        assert_eq!(disabled.variable("Result"), Some(0.0));
        assert_eq!(enabled.twists()[0].angle_degrees, -25.0);
        assert!(disabled.twists().is_empty());
        assert!(!enabled.set_variable("IsMale", 0.0));
        assert_eq!(enabled.variable("IsMale"), None);
        assert_eq!(enabled.twists().len(), 1);
    }

    #[test]
    fn scalar_modifier_properties_supply_inputs_but_do_not_receive_outputs() {
        let mut g = BehaviorGraph::new(
            "Properties",
            Some(0),
            vec![Generator::Wrap {
                name: "Wrapped".into(),
                class: "hkbModifierGenerator".into(),
                child: None,
                modifier: Some(0),
            }],
            vec![],
        );
        let output = g.add_variable("SpeedOut", VarType::Real, 0.0);
        g.modifiers = vec![Modifier::SpeedSampler { goal_speed: 1.0 }];
        g.modifier_bindings = vec![vec![Binding {
            member: "speedOut".into(),
            variable: output,
        }]];
        g.modifier_property_bindings = vec![vec![
            PropertyBinding {
                member: "goalSpeed".into(),
                property: "speed".into(),
            },
            PropertyBinding {
                member: "speedOut".into(),
                property: "unchanged".into(),
            },
        ]];
        let character = Character {
            scalar_properties: [("speed".into(), 42.0), ("unchanged".into(), 9.0)]
                .into_iter()
                .collect(),
            ..Default::default()
        };
        let mut i = Instance::new(
            Shared::new(Arc::new(Project {
                graphs: vec![("root.hkx".into(), g)],
                character: Some(character),
            })),
            1,
        );
        i.update(0.1, &mut durations);
        assert_eq!(i.variable("SpeedOut"), Some(42.0));
        assert_eq!(
            i.shared
                .project
                .character
                .as_ref()
                .unwrap()
                .scalar_properties["unchanged"],
            9.0
        );
        assert_eq!(
            i.bound_members(0, 0)
                .find(|(m, _)| *m == "speedOut")
                .unwrap()
                .1,
            42.0
        );
    }

    #[test]
    fn twists_honor_unbound_disable_and_reject_unsupported_bindings() {
        let t = Twist {
            enabled: false,
            axis: glam::Vec3::X,
            angle_degrees: 30.0,
            start_bone: 0,
            end_bone: 0,
            set_angle_method: 0,
            axis_coordinates: 0,
            additive: true,
        };
        for bound in [false, true] {
            let mut g = BehaviorGraph::new(
                "Twist",
                Some(0),
                vec![Generator::Wrap {
                    name: "Wrapped".into(),
                    class: "hkbModifierGenerator".into(),
                    child: None,
                    modifier: Some(0),
                }],
                vec![],
            );
            let mut t = t.clone();
            t.enabled = bound;
            g.modifiers = vec![Modifier::Twist(t)];
            let v = g.add_variable("Bone", VarType::Real, f32::NAN);
            g.modifier_bindings = vec![if bound {
                vec![Binding {
                    member: "startBoneIndex".into(),
                    variable: v,
                }]
            } else {
                vec![]
            }];
            assert!(instance(g).twists().is_empty());
        }
    }

    fn time(i: &Instance) -> f32 {
        (i.samples()[0].time * 100.0).round() / 100.0
    }

    #[test]
    fn state_change_event_reaches_the_destination_and_preserves_payload() {
        let mut g = graph();
        let changed = g.events.len() as i32;
        g.events.push("Changed".into());
        if let Generator::StateMachine {
            change_event,
            states,
            ..
        } = &mut g.generators[0]
        {
            *change_event = Some(EventProperty {
                event: changed,
                payload: Some("authored payload".into()),
            });
            // A listener in the destination immediately continues to the loop.
            states[1].transitions = vec![Transition::new(changed, 2, None)];
        }
        let mut i = instance(g);
        assert!(i.handle_event("Sit", &mut durations));
        assert_eq!(playing(&i), [("Loop".into(), 1.0)]);
        assert_eq!(
            i.take_raised(),
            vec![
                Raised {
                    event: "Changed".into(),
                    payload: Some("authored payload".into())
                },
                Raised {
                    event: "Changed".into(),
                    payload: Some("authored payload".into())
                },
            ]
        );
    }

    #[test]
    fn state_change_event_waits_for_initiation_and_fires_at_blend_start() {
        let mut g = windowed(false);
        let changed = g.events.len() as i32;
        g.events.push("Changed".into());
        if let Generator::StateMachine {
            change_event,
            states,
            ..
        } = &mut g.generators[0]
        {
            *change_event = Some(EventProperty {
                event: changed,
                payload: None,
            });
            states[0].transitions[0].blend = Some(0.5);
        }
        let mut i = instance(g);
        i.take_raised();
        assert!(i.handle_event("Out", &mut durations));
        assert!(i.take_raised().is_empty());
        i.update(1.0, &mut durations);
        assert_eq!(
            i.take_raised()
                .iter()
                .filter(|e| e.event == "Changed")
                .count(),
            1
        );
        i.update(0.25, &mut durations);
        assert_eq!(playing(&i), [("Loop".into(), 0.5), ("Exit".into(), 0.5)]);
        assert!(i.take_raised().is_empty());
    }

    #[test]
    fn state_change_event_maps_from_a_referenced_graph() {
        let root = BehaviorGraph::new(
            "Root",
            Some(0),
            vec![Generator::Reference {
                name: "Ref".into(),
                behavior: "g".into(),
            }],
            vec!["Unrelated".into(), "Changed".into()],
        );
        let mut g = graph();
        let changed = g.events.len() as i32;
        g.events.push("Changed".into());
        if let Generator::StateMachine { change_event, .. } = &mut g.generators[0] {
            *change_event = Some(EventProperty {
                event: changed,
                payload: None,
            });
        }
        let shared = Shared::new(Arc::new(Project {
            graphs: vec![("root".into(), root), ("g".into(), g)],
            character: None,
        }));
        let mut i = Instance::new(shared, 1);
        i.update(0.0, &mut durations);
        assert!(i.handle_event("Sit", &mut durations));
        assert_eq!(
            i.take_raised(),
            vec![Raised {
                event: "Changed".into(),
                payload: None
            }]
        );
        i.update(1.1, &mut durations); // The clip-end trigger commits another transition.
        assert_eq!(playing(&i), [("Loop".into(), 1.0)]);
        assert_eq!(
            i.take_raised()
                .iter()
                .filter(|e| e.event == "Changed")
                .count(),
            1
        );
    }

    #[test]
    fn current_state_output_reaches_child_start_states() {
        let mut g = BehaviorGraph::new(
            "G",
            Some(0),
            vec![
                Generator::StateMachine {
                    name: "Parent".into(),
                    start: 42,
                    start_variable: None,
                    start_mode: StartMode::Default,
                    sync_variable: None,
                    change_event: None,
                    states: vec![
                        state(42, 1, vec![Transition::new(0, 7, None)]),
                        state(7, 1, vec![]),
                    ],
                    wildcards: vec![],
                },
                Generator::StateMachine {
                    name: "Child".into(),
                    start: 7,
                    start_variable: Some(0),
                    start_mode: StartMode::Default,
                    sync_variable: None,
                    change_event: None,
                    states: vec![state(7, 2, vec![]), state(42, 3, vec![])],
                    wildcards: vec![],
                },
                clip("Seven", ClipMode::Looping, vec![]),
                clip("FortyTwo", ClipMode::Looping, vec![]),
            ],
            vec!["Change".into()],
        );
        let v = g.add_variable("ParentState", VarType::Int, -1.0);
        g.bindings[0].push(Binding {
            member: "currentStateId".into(),
            variable: v,
        });
        let mut i = instance(g);
        assert_eq!(i.variable("ParentState"), Some(42.0));
        assert_eq!(playing(&i), [("FortyTwo".into(), 1.0)]);
        assert!(i.handle_event("Change", &mut durations));
        assert_eq!(i.variable("ParentState"), Some(7.0));
        assert_eq!(playing(&i), [("Seven".into(), 1.0)]);
    }

    #[test]
    fn current_state_output_waits_for_pending_transition_but_not_blend() {
        let mut g = windowed(false);
        let v = g.add_variable("State", VarType::Int, -1.0);
        g.bindings[0].push(Binding {
            member: "currentStateId".into(),
            variable: v,
        });
        if let Generator::StateMachine { states, .. } = &mut g.generators[0] {
            states[0].transitions[0].blend = Some(0.5);
        }
        let mut i = instance(g);
        assert_eq!(i.variable("State"), Some(0.0));
        assert!(i.handle_event("Out", &mut durations));
        assert_eq!(i.variable("State"), Some(0.0));
        i.update(1.0, &mut durations); // Open trigger starts the pending transition.
        assert_eq!(i.variable("State"), Some(1.0));
        i.update(0.25, &mut durations);
        assert_eq!(playing(&i), [("Loop".into(), 0.5), ("Exit".into(), 0.5)]);
        assert_eq!(i.variable("State"), Some(1.0));
    }

    #[test]
    fn current_state_output_maps_referenced_graph_variables() {
        let mut root = BehaviorGraph::new(
            "Root",
            Some(0),
            vec![Generator::Reference {
                name: "Ref".into(),
                behavior: "g".into(),
            }],
            vec![],
        );
        root.add_variable("Unrelated", VarType::Int, 123.0);
        let mut g = graph();
        let v = g.add_variable("State", VarType::Int, -1.0);
        g.bindings[0].push(Binding {
            member: "currentStateId".into(),
            variable: v,
        });
        // An invalid authored start falls back to the first state's actual ID.
        if let Generator::StateMachine { start, .. } = &mut g.generators[0] {
            *start = 999;
        }
        let shared = Shared::new(Arc::new(Project {
            graphs: vec![("root".into(), root), ("g".into(), g)],
            character: None,
        }));
        let mut i = Instance::new(shared, 1);
        i.update(0.0, &mut durations);
        assert_eq!(i.variable("State"), Some(0.0));
        assert!(i.handle_event("Sit", &mut durations));
        assert_eq!(i.variable("State"), Some(1.0));
        i.update(1.1, &mut durations); // Clip-end trigger advances to the loop.
        assert_eq!(i.variable("State"), Some(2.0));
        assert_eq!(i.variable("Unrelated"), Some(123.0));
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

    /// A machine whose state 0 plays `first` and state 1 a single-play clip, and
    /// whose event `Again` leads back into state 0 over half a second.
    fn self_transition(first: ClipMode, mode: SelfTransitionMode) -> Instance {
        let mut again = Transition::new(0, 0, None);
        again.blend = Some(0.5);
        again.self_mode = mode;
        let gens = vec![
            Generator::StateMachine {
                name: "Root".into(),
                start: 0,
                start_variable: None,
                start_mode: Default::default(),
                sync_variable: None,
                change_event: None,
                states: vec![state(0, 1, vec![again]), state(1, 2, vec![])],
                wildcards: vec![],
            },
            clip("Loop", first, vec![]),
            clip("Once", ClipMode::SinglePlay, vec![]),
        ];
        instance(BehaviorGraph::new("G", Some(0), gens, vec!["Again".into()]))
    }

    #[test]
    fn self_transition_modes_continue_reset_or_blend() {
        use SelfTransitionMode::*;
        let again = |first, mode| {
            let mut i = self_transition(first, mode);
            i.update(1.0, &mut durations);
            assert!(i.handle_event("Again", &mut durations));
            i.update(0.25, &mut durations);
            playing(&i)
        };
        // Continue and a looping state: it plays on, 1.25 s in.
        let mut i = self_transition(ClipMode::Looping, Continue);
        i.update(1.0, &mut durations);
        i.handle_event("Again", &mut durations);
        i.update(0.25, &mut durations);
        assert_eq!(playing(&i), [("Loop".into(), 1.0)]);
        assert_eq!(time(&i), 1.25);
        assert_eq!(i.changes(), 0);
        // Reset restarts at once, with no blend.
        let mut i = self_transition(ClipMode::Looping, Reset);
        i.update(1.0, &mut durations);
        i.handle_event("Again", &mut durations);
        i.update(0.25, &mut durations);
        assert_eq!(playing(&i), [("Loop".into(), 1.0)]);
        assert_eq!(time(&i), 0.25);
        // Blend restarts, the old state fading out.
        assert_eq!(
            again(ClipMode::Looping, Blend),
            [("Loop".into(), 0.5), ("Loop".into(), 0.5)]
        );
        // Continue if cyclic: a loop carries on, a single play blends.
        assert_eq!(
            again(ClipMode::Looping, ContinueIfCyclicBlendIfAcyclic),
            [("Loop".into(), 1.0)]
        );
        assert_eq!(
            again(ClipMode::SinglePlay, ContinueIfCyclicBlendIfAcyclic).len(),
            2
        );
    }

    /// State 0 is a machine of its own (`I0` -> `I1` on `Go`); `Leave` blends
    /// out of it for a second into a clip.
    fn leaving(mode: EventMode) -> Instance {
        let mut leave = Transition::new(0, 1, None);
        leave.blend = Some(1.0);
        leave.event_mode = mode;
        let gens = vec![
            Generator::StateMachine {
                name: "Root".into(),
                start: 0,
                start_variable: None,
                start_mode: Default::default(),
                sync_variable: None,
                change_event: None,
                states: vec![state(0, 1, vec![leave]), state(1, 4, vec![])],
                wildcards: vec![],
            },
            Generator::StateMachine {
                name: "Inner".into(),
                start: 0,
                start_variable: None,
                start_mode: Default::default(),
                sync_variable: None,
                change_event: None,
                states: vec![
                    state(0, 2, vec![Transition::new(1, 1, None)]),
                    state(1, 3, vec![]),
                ],
                wildcards: vec![],
            },
            clip("I0", ClipMode::Looping, vec![]),
            clip("I1", ClipMode::Looping, vec![]),
            clip("B", ClipMode::Looping, vec![]),
        ];
        instance(BehaviorGraph::new(
            "G",
            Some(0),
            gens,
            vec!["Leave".into(), "Go".into()],
        ))
    }

    #[test]
    fn event_mode_lets_the_state_being_left_hear_events_when_asked() {
        for (mode, heard) in [
            (EventMode::Default, "I0"),
            (EventMode::IgnoreFromGenerator, "I0"),
            (EventMode::ProcessAll, "I1"),
        ] {
            let mut i = leaving(mode);
            assert!(i.handle_event("Leave", &mut durations));
            i.update(0.25, &mut durations);
            i.handle_event("Go", &mut durations);
            i.update(0.25, &mut durations);
            let names: Vec<String> = playing(&i).into_iter().map(|p| p.0).collect();
            assert_eq!(names, [heard, "B"], "{mode:?}");
            // The blend over, the old state is gone for good.
            i.update(1.0, &mut durations);
            assert_eq!(playing(&i), [("B".into(), 1.0)]);
        }
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
            weight_property: None,
            bone_weights: None,
            bone_weights_property: None,
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
                sync_variable: None,
                change_event: None,
                states: vec![state(0, 1, vec![]), state(1, 2, vec![])],
                wildcards: vec![],
            },
            clip("Idle", ClipMode::Looping, vec![]),
            Generator::StateMachine {
                name: "Nested".into(),
                start: 5,
                start_variable: None,
                start_mode: Default::default(),
                sync_variable: None,
                change_event: None,
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
                sync_variable: None,
                change_event: None,
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
