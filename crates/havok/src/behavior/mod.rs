//! Behaviour graphs (`hkbBehaviorGraph`): the generator tree (state machines,
//! blenders, selectors, clips, references to other graphs), state transitions,
//! clip triggers, variables, variable bindings and the modifiers that steer
//! control flow. [`runtime`] runs them; [`resolve`] answers static "which clips
//! does this event play?" questions.

pub mod expr;
pub mod resolve;
pub mod runtime;

pub use resolve::{PlayedClip, Playback, RaisedEvent};

use crate::{Packfile, Result};

pub type GenId = usize;
pub type ModId = usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipMode {
    SinglePlay,
    Looping,
    Other,
}

/// `hkbStateMachine::StartStateMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StartMode {
    /// `startStateId` (or the variable bound to it).
    #[default]
    Default,
    /// The state kept in a graph variable, which follows the current state: a
    /// machine re-entered later resumes where it was (sneaking, sprinting).
    Sync(usize),
    /// A random state.
    Random,
}

#[derive(Debug, Clone)]
pub struct Trigger {
    /// Seconds from the start of the clip, or from its end if `from_end`.
    pub time: f32,
    pub from_end: bool,
    /// Index into the graph's event names.
    pub event: i32,
    /// String payload (`hkbStringEventPayload`), e.g. the ANIO editor id of `AnimObjLoad`.
    pub payload: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Transition {
    /// Event that triggers it, or -1 for one taken when its condition holds.
    pub event: i32,
    pub to_state: i32,
    /// State to enter inside the target state's (nested) state machine, if any.
    pub to_nested: Option<i32>,
    pub flags: u16,
    /// Cross-fade time (`hkbBlendingTransitionEffect`); `None` switches at once.
    pub blend: Option<f32>,
    /// Variable bound to the blend's duration, if any.
    pub blend_variable: Option<usize>,
    /// `hkbExpressionCondition` that must hold.
    pub condition: Option<String>,
    pub priority: i16,
}

impl Transition {
    pub fn new(event: i32, to_state: i32, to_nested: Option<i32>) -> Transition {
        Transition { event, to_state, to_nested, flags: if to_nested.is_some() { FLAG_TO_NESTED_STATE_ID_IS_VALID } else { 0 }, ..Default::default() }
    }

    pub fn disabled(&self) -> bool {
        self.flags & FLAG_DISABLED != 0
    }

    /// A wildcard of a nested state machine that its parent may take into the
    /// state containing it.
    pub fn global_wildcard(&self) -> bool {
        self.flags & FLAG_IS_GLOBAL_WILDCARD != 0
    }

    pub fn allows_self(&self) -> bool {
        self.flags & FLAG_ALLOW_SELF_TRANSITION != 0
    }
}

const FLAG_DISABLED: u16 = 0x20;
const FLAG_ALLOW_SELF_TRANSITION: u16 = 0x200;
const FLAG_IS_GLOBAL_WILDCARD: u16 = 0x400;
const FLAG_TO_NESTED_STATE_ID_IS_VALID: u16 = 0x2000;

/// An event raised by the graph itself, with its optional string payload.
#[derive(Debug, Clone)]
pub struct EventProperty {
    pub event: i32,
    pub payload: Option<String>,
}

#[derive(Debug, Clone)]
pub struct State {
    pub id: i32,
    pub name: String,
    pub generator: Option<GenId>,
    pub transitions: Vec<Transition>,
    /// Events raised on entering / leaving the state (e.g. `AnimObjLoad`).
    pub enter_events: Vec<EventProperty>,
    pub exit_events: Vec<EventProperty>,
}

#[derive(Debug, Clone)]
pub struct BlendChild {
    pub generator: Option<GenId>,
    pub weight: f32,
    /// Variable bound to `weight`.
    pub weight_variable: Option<usize>,
    /// Per-bone weights (Havok skeleton bone order), if the child affects only some bones.
    pub bone_weights: Option<Vec<f32>>,
}

pub const BLEND_SYNC: u16 = 0x1;
pub const BLEND_PARAMETRIC: u16 = 0x10;
pub const BLEND_CYCLIC: u16 = 0x20;

#[derive(Debug, Clone)]
pub enum Generator {
    Clip {
        name: String,
        animation: String,
        mode: ClipMode,
        speed: f32,
        triggers: Vec<Trigger>,
        /// Seconds cut from the start / end of the animation.
        crop_start: f32,
        crop_end: f32,
        start_time: f32,
    },
    StateMachine {
        name: String,
        start: i32,
        /// Graph variable the start state is bound to, if any (overrides `start`).
        start_variable: Option<usize>,
        /// How the start state is picked when the machine activates.
        start_mode: StartMode,
        states: Vec<State>,
        wildcards: Vec<Transition>,
    },
    /// Weighted blend of children (`hkbBlenderGenerator`, `hkbPoseMatchingGenerator`).
    Blender { name: String, parameter: f32, min_cyclic: f32, max_cyclic: f32, flags: u16, children: Vec<BlendChild> },
    /// One child at a time, picked by a (bound) index.
    Selector { name: String, children: Vec<GenId>, index: i8 },
    /// A generator wrapping one child: `hkbModifierGenerator` (running a modifier
    /// alongside it) and Bethesda's tagging / sync / cyclic-blend wrappers.
    Wrap { name: String, class: String, child: Option<GenId>, modifier: Option<ModId> },
    /// `BSBoneSwitchGenerator`: children replace the default on their bones.
    BoneSwitch { name: String, default: Option<GenId>, children: Vec<(GenId, Vec<f32>)> },
    /// Another behaviour file (e.g. `Behaviors\MT_Behavior.hkx`).
    Reference { name: String, behavior: String },
    Other(String),
}

impl Generator {
    pub fn name(&self) -> &str {
        match self {
            Generator::Clip { name, .. }
            | Generator::StateMachine { name, .. }
            | Generator::Blender { name, .. }
            | Generator::Selector { name, .. }
            | Generator::Wrap { name, .. }
            | Generator::BoneSwitch { name, .. }
            | Generator::Reference { name, .. } => name,
            Generator::Other(class) => class,
        }
    }

    /// Child generators other than states, the default / dominant one first.
    pub fn children(&self) -> Vec<GenId> {
        match self {
            Generator::Blender { children, .. } => children.iter().filter_map(|c| c.generator).collect(),
            Generator::Selector { children, .. } => children.clone(),
            Generator::Wrap { child, .. } => child.iter().copied().collect(),
            Generator::BoneSwitch { default, children, .. } => default.iter().copied().chain(children.iter().map(|c| c.0)).collect(),
            _ => Vec::new(),
        }
    }
}

/// Modifiers that steer the graph (others are kept by class name and ignored).
#[derive(Debug, Clone)]
pub enum Modifier {
    List(Vec<ModId>),
    /// `hkbEvaluateExpressionModifier`: assignments and conditional events, every frame.
    Expressions(Vec<String>),
    /// `BSEventEveryNEventsModifier`: raise `send` after every `n` (randomised from
    /// `min..=n`) occurrences of `check`.
    EveryN { check: i32, send: EventProperty, n: u8, min: u8, random: bool },
    /// `hkbTimerModifier`: raise `alarm` `seconds` after activation.
    Timer { seconds: f32, alarm: EventProperty },
    /// `hkbEventDrivenModifier`: `child` runs between `activate` and `deactivate` events.
    EventDriven { child: Option<ModId>, activate: i32, deactivate: i32, active_by_default: bool },
    /// `BSEventOnDeactivateModifier`: raise `event` when deactivated.
    OnDeactivate(EventProperty),
    /// `BSIsActiveModifier`: bound `bIsActiveN` outputs follow whether it runs.
    IsActive { invert: [bool; 5] },
    /// `BSSpeedSamplerModifier`: the bound `speedOut` follows `goalSpeed` (the
    /// locomotion blends' parameter, in units per second).
    SpeedSampler { goal_speed: f32 },
    /// `hkbDampingModifier`: `dampedValue` chases `rawValue` through a PID step
    /// every update.
    Damping { kp: f32, ki: f32, kd: f32, raw: f32, damped: f32 },
    Other(String),
}

/// Variable types (`hkbVariableInfo::type`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarType {
    Bool,
    Int,
    Real,
    Other,
}

/// A generator or modifier member bound to a graph variable.
#[derive(Debug, Clone)]
pub struct Binding {
    /// Member path, e.g. `blendParameter`, `selectedGeneratorIndex`, `bIsActive0`.
    pub member: String,
    pub variable: usize,
}

pub struct BehaviorGraph {
    pub name: String,
    pub root: Option<GenId>,
    pub generators: Vec<Generator>,
    /// Variable bindings of each generator (same indices as `generators`).
    pub bindings: Vec<Vec<Binding>>,
    pub modifiers: Vec<Modifier>,
    pub modifier_bindings: Vec<Vec<Binding>>,
    pub events: Vec<String>,
    /// Graph variables, their types and initial (word) values.
    pub variables: Vec<String>,
    pub variable_types: Vec<VarType>,
    pub variable_defaults: Vec<i32>,
}

impl BehaviorGraph {
    /// A graph built in code (tests, tools): no variables, bindings or modifiers.
    pub fn new(name: &str, root: Option<GenId>, generators: Vec<Generator>, events: Vec<String>) -> BehaviorGraph {
        BehaviorGraph {
            name: name.to_owned(),
            root,
            bindings: vec![Vec::new(); generators.len()],
            generators,
            modifiers: Vec::new(),
            modifier_bindings: Vec::new(),
            events,
            variables: Vec::new(),
            variable_types: Vec::new(),
            variable_defaults: Vec::new(),
        }
    }

    /// Add a variable with its type and initial value; returns its index.
    pub fn add_variable(&mut self, name: &str, ty: VarType, value: f32) -> usize {
        self.variables.push(name.to_owned());
        self.variable_types.push(ty);
        self.variable_defaults.push(if ty == VarType::Real { value.to_bits() as i32 } else { value as i32 });
        self.variables.len() - 1
    }

    /// Initial value of variable `v` as a float (ints and bools converted).
    pub fn variable_default(&self, v: usize) -> f32 {
        let word = self.variable_defaults.get(v).copied().unwrap_or(0);
        match self.variable_types.get(v) {
            Some(VarType::Real) => f32::from_bits(word as u32),
            _ => word as f32,
        }
    }

    /// Variable bound to `member` of generator `g`.
    pub fn bound(&self, g: GenId, member: &str) -> Option<usize> {
        self.bindings.get(g)?.iter().find(|b| b.member == member).map(|b| b.variable)
    }
}

// Field offsets for hk_2010.2.0-r1 with 64-bit pointers (hkbNode name at 0x38).
const NODE_NAME: u32 = 0x38;
const ARRAY_PTR_SIZE: u32 = 8;

struct Reader<'a> {
    p: &'a Packfile,
    classes: std::collections::HashMap<u32, &'a str>,
    ids: std::collections::HashMap<u32, GenId>,
    generators: Vec<Generator>,
    bindings: Vec<Vec<Binding>>,
    mod_ids: std::collections::HashMap<u32, ModId>,
    modifiers: Vec<Modifier>,
    modifier_bindings: Vec<Vec<Binding>>,
}

impl<'a> Reader<'a> {
    fn class(&self, o: u32) -> &'a str {
        self.classes.get(&o).copied().unwrap_or("")
    }

    /// Pointer array (hkArray of object pointers) at `o`.
    fn ptr_array(&self, o: u32) -> Vec<u32> {
        let (Some(data), n) = self.p.array(o) else { return Vec::new() };
        (0..n as u32).filter_map(|i| self.p.ptr(data + i * ARRAY_PTR_SIZE)).collect()
    }

    fn transitions(&self, array_obj: Option<u32>) -> Vec<Transition> {
        let Some(a) = array_obj else { return Vec::new() };
        let (Some(data), n) = self.p.array(a + 0x10) else { return Vec::new() };
        (0..n as u32)
            .map(|i| {
                let t = data + i * 0x48;
                let flags = self.p.u16(t + 0x42);
                let effect = self.p.ptr(t + 0x20).filter(|&e| self.class(e) == "hkbBlendingTransitionEffect");
                let condition = self.p.ptr(t + 0x28).filter(|&c| self.class(c) == "hkbExpressionCondition");
                Transition {
                    event: self.p.i32(t + 0x30),
                    to_state: self.p.i32(t + 0x34),
                    to_nested: (flags & FLAG_TO_NESTED_STATE_ID_IS_VALID != 0).then(|| self.p.i32(t + 0x3C)),
                    flags,
                    blend: effect.map(|e| self.p.f32(e + 0x50)),
                    blend_variable: effect.and_then(|e| self.bindings_of(e).into_iter().find(|b| b.member == "duration")).map(|b| b.variable),
                    condition: condition.and_then(|c| self.p.string(c + 0x10)),
                    priority: self.p.i16(t + 0x40),
                }
            })
            .collect()
    }

    /// Variable bindings of the bindable object at `o`.
    fn bindings_of(&self, o: u32) -> Vec<Binding> {
        let Some(set) = self.p.ptr(o + 0x10) else { return Vec::new() };
        let (Some(data), n) = self.p.array(set + 0x10) else { return Vec::new() };
        (0..n as u32)
            .map(|i| data + i * 0x28)
            // Binding type 0: a variable (1 would be a character property).
            .filter(|&b| self.p.u8(b + 0x21) == 0)
            .filter_map(|b| Some(Binding { member: self.p.string(b)?, variable: usize::try_from(self.p.i32(b + 0x1C)).ok()? }))
            .collect()
    }

    /// String payload of the `hkbEventPayload` pointed to from `slot`.
    fn payload(&self, slot: u32) -> Option<String> {
        self.p.ptr(slot).filter(|&o| self.class(o) == "hkbStringEventPayload").and_then(|o| self.p.string(o + 0x10))
    }

    /// `hkbEventProperty` (id, payload) at `o`.
    fn event_property(&self, o: u32) -> EventProperty {
        EventProperty { event: self.p.i32(o), payload: self.payload(o + 8) }
    }

    fn event_properties(&self, array_obj: Option<u32>) -> Vec<EventProperty> {
        let Some(a) = array_obj else { return Vec::new() };
        let (Some(data), n) = self.p.array(a + 0x10) else { return Vec::new() };
        (0..n as u32).map(|i| self.event_property(data + i * 0x10)).collect()
    }

    fn triggers(&self, array_obj: Option<u32>) -> Vec<Trigger> {
        let Some(a) = array_obj else { return Vec::new() };
        let (Some(data), n) = self.p.array(a + 0x10) else { return Vec::new() };
        (0..n as u32)
            .map(|i| {
                let t = data + i * 0x20;
                Trigger { time: self.p.f32(t), event: self.p.i32(t + 8), from_end: self.p.u8(t + 0x18) != 0, payload: self.payload(t + 0x10) }
            })
            .collect()
    }

    /// `hkbBoneWeightArray` at `o`.
    fn bone_weights(&self, o: Option<u32>) -> Option<Vec<f32>> {
        let o = o?;
        let (Some(data), n) = self.p.array(o + 0x30) else { return None };
        Some((0..n as u32).map(|i| self.p.f32(data + i * 4)).collect())
    }

    /// Read the modifier at `o` (memoised).
    fn modifier(&mut self, o: u32) -> ModId {
        if let Some(&id) = self.mod_ids.get(&o) {
            return id;
        }
        let id = self.modifiers.len();
        self.mod_ids.insert(o, id);
        self.modifiers.push(Modifier::Other(String::new()));
        self.modifier_bindings.push(self.bindings_of(o));
        let p = self.p;
        let class = self.class(o);
        let m = match class {
            "hkbModifierList" => Modifier::List(self.ptr_array(o + 0x50).into_iter().map(|m| self.modifier(m)).collect()),
            "hkbEvaluateExpressionModifier" => {
                let lines = p
                    .ptr(o + 0x50)
                    .map(|a| {
                        let (data, n) = p.array(a + 0x10);
                        data.map(|d| (0..n as u32).filter_map(|i| p.string(d + i * 0x18)).collect()).unwrap_or_default()
                    })
                    .unwrap_or_default();
                Modifier::Expressions(lines)
            }
            "BSEventEveryNEventsModifier" => Modifier::EveryN {
                check: p.i32(o + 0x50),
                send: self.event_property(o + 0x60),
                n: p.u8(o + 0x70),
                min: p.u8(o + 0x71),
                random: p.u8(o + 0x72) != 0,
            },
            "hkbTimerModifier" => Modifier::Timer { seconds: p.f32(o + 0x50), alarm: self.event_property(o + 0x58) },
            "hkbEventDrivenModifier" => Modifier::EventDriven {
                child: p.ptr(o + 0x50).map(|m| self.modifier(m)),
                activate: p.i32(o + 0x58),
                deactivate: p.i32(o + 0x5C),
                active_by_default: p.u8(o + 0x60) != 0,
            },
            "BSEventOnDeactivateModifier" => Modifier::OnDeactivate(self.event_property(o + 0x50)),
            "BSIsActiveModifier" => Modifier::IsActive { invert: std::array::from_fn(|i| p.u8(o + 0x51 + 2 * i as u32) != 0) },
            // After hkbModifier (enable at 0x48): state, direction, goal speed, speed out.
            "BSSpeedSamplerModifier" => Modifier::SpeedSampler { goal_speed: p.f32(o + 0x50) },
            // kP, kI, kD, scalar / vector flags, raw and damped values.
            "hkbDampingModifier" if p.u8(o + 0x5C) != 0 => Modifier::Damping {
                kp: p.f32(o + 0x50),
                ki: p.f32(o + 0x54),
                kd: p.f32(o + 0x58),
                raw: p.f32(o + 0x60),
                damped: p.f32(o + 0x64),
            },
            other => Modifier::Other(other.to_owned()),
        };
        self.modifiers[id] = m;
        id
    }

    /// Read the generator at `o` (memoised; cycles resolve to the same id).
    fn generator(&mut self, o: u32) -> GenId {
        if let Some(&id) = self.ids.get(&o) {
            return id;
        }
        let id = self.generators.len();
        self.ids.insert(o, id);
        self.generators.push(Generator::Other(String::new()));
        self.bindings.push(self.bindings_of(o));
        let p = self.p;
        let class = self.class(o);
        let name = p.string(o + NODE_NAME).unwrap_or_default();
        let g = match class {
            "hkbClipGenerator" => Generator::Clip {
                name,
                animation: p.string(o + 0x48).unwrap_or_default(),
                mode: match p.u8(o + 0x72) {
                    0 => ClipMode::SinglePlay,
                    1 => ClipMode::Looping,
                    _ => ClipMode::Other,
                },
                speed: p.f32(o + 0x64),
                triggers: self.triggers(p.ptr(o + 0x50)),
                crop_start: p.f32(o + 0x58),
                crop_end: p.f32(o + 0x5C),
                start_time: p.f32(o + 0x60),
            },
            "hkbStateMachine" => {
                let mut states = Vec::new();
                for s in self.ptr_array(o + 0x90) {
                    let generator = p.ptr(s + 0x58).map(|g| self.generator(g));
                    states.push(State {
                        id: p.i32(s + 0x68),
                        name: p.string(s + 0x60).unwrap_or_default(),
                        generator,
                        transitions: self.transitions(p.ptr(s + 0x50)),
                        enter_events: self.event_properties(p.ptr(s + 0x40)),
                        exit_events: self.event_properties(p.ptr(s + 0x48)),
                    });
                }
                Generator::StateMachine {
                    name,
                    start: p.i32(o + 0x68),
                    start_variable: self.bindings[id].iter().find(|b| b.member == "startStateId").map(|b| b.variable),
                    // syncVariableIndex at 0x7C; startStateMode at 0x86.
                    start_mode: match (p.u8(o + 0x86), p.i32(o + 0x7C)) {
                        (1, v) if v >= 0 => StartMode::Sync(v as usize),
                        (2, _) => StartMode::Random,
                        _ => StartMode::Default,
                    },
                    states,
                    wildcards: self.transitions(p.ptr(o + 0xA0)),
                }
            }
            "hkbBlenderGenerator" | "hkbPoseMatchingGenerator" => {
                let mut children = Vec::new();
                for c in self.ptr_array(o + 0x60) {
                    let generator = p.ptr(c + 0x30).map(|g| self.generator(g));
                    children.push(BlendChild {
                        generator,
                        weight: p.f32(c + 0x40),
                        weight_variable: self.bindings_of(c).into_iter().find(|b| b.member == "weight").map(|b| b.variable),
                        bone_weights: self.bone_weights(p.ptr(c + 0x38)),
                    });
                }
                Generator::Blender {
                    name,
                    parameter: p.f32(o + 0x4C),
                    min_cyclic: p.f32(o + 0x50),
                    max_cyclic: p.f32(o + 0x54),
                    flags: p.u16(o + 0x5A),
                    children,
                }
            }
            "hkbManualSelectorGenerator" => {
                let gens = self.ptr_array(o + 0x48);
                Generator::Selector { name, children: gens.into_iter().map(|g| self.generator(g)).collect(), index: p.u8(o + 0x58) as i8 }
            }
            "BSBoneSwitchGenerator" => {
                let default = p.ptr(o + 0x50).map(|g| self.generator(g));
                let mut children = Vec::new();
                for d in self.ptr_array(o + 0x58) {
                    if let Some(g) = p.ptr(d + 0x30) {
                        let weights = self.bone_weights(p.ptr(d + 0x38)).unwrap_or_default();
                        children.push((self.generator(g), weights));
                    }
                }
                Generator::BoneSwitch { name, default, children }
            }
            "hkbModifierGenerator" => {
                let modifier = p.ptr(o + 0x48).map(|m| self.modifier(m));
                Generator::Wrap { name, class: class.to_owned(), child: p.ptr(o + 0x50).map(|g| self.generator(g)), modifier }
            }
            "BSSynchronizedClipGenerator" | "BSCyclicBlendTransitionGenerator" | "BSiStateTaggingGenerator" | "BSOffsetAnimationGenerator" => {
                Generator::Wrap { name, class: class.to_owned(), child: p.ptr(o + 0x50).map(|g| self.generator(g)), modifier: None }
            }
            "hkbBehaviorReferenceGenerator" => Generator::Reference { name, behavior: p.string(o + 0x48).unwrap_or_default() },
            _ => Generator::Other(class.to_owned()),
        };
        self.generators[id] = g;
        id
    }
}

impl BehaviorGraph {
    pub fn parse(bytes: &[u8]) -> Result<BehaviorGraph> {
        let p = Packfile::parse(bytes)?;
        let Some(graph) = p.objects_of("hkbBehaviorGraph").next() else {
            return Err(crate::Error::Corrupt("no hkbBehaviorGraph".into()));
        };
        let name = p.string(graph + NODE_NAME).unwrap_or_default();
        let data = p.ptr(graph + 0x88);
        let strings = data.and_then(|d| p.ptr(d + 0x78));
        let string_array = |at: u32| -> Vec<String> {
            let Some(s) = strings else { return Vec::new() };
            let (ptr, n) = p.array(s + at);
            ptr.map(|d| (0..n as u32).map(|i| p.string(d + i * 8).unwrap_or_default()).collect()).unwrap_or_default()
        };
        let events = string_array(0x10);
        let variables = string_array(0x30);
        let variable_types = data
            .map(|d| {
                let (ptr, n) = p.array(d + 0x20);
                ptr.map(|a| {
                    (0..n as u32)
                        .map(|i| match p.u8(a + i * 6 + 4) {
                            0 => VarType::Bool,
                            1..=3 => VarType::Int,
                            4 => VarType::Real,
                            _ => VarType::Other,
                        })
                        .collect()
                })
                .unwrap_or_default()
            })
            .unwrap_or_default();
        let variable_defaults = data
            .and_then(|d| p.ptr(d + 0x70))
            .map(|values| {
                let (ptr, n) = p.array(values + 0x10);
                ptr.map(|d| (0..n as u32).map(|i| p.i32(d + i * 4)).collect()).unwrap_or_default()
            })
            .unwrap_or_default();
        let mut r = Reader {
            p: &p,
            classes: p.objects.iter().map(|o| (o.offset, o.class.as_str())).collect(),
            ids: Default::default(),
            generators: Vec::new(),
            bindings: Vec::new(),
            mod_ids: Default::default(),
            modifiers: Vec::new(),
            modifier_bindings: Vec::new(),
        };
        let root = p.ptr(graph + 0x80).map(|g| r.generator(g));
        Ok(BehaviorGraph {
            name,
            root,
            generators: r.generators,
            bindings: r.bindings,
            modifiers: r.modifiers,
            modifier_bindings: r.modifier_bindings,
            events,
            variables,
            variable_types,
            variable_defaults,
        })
    }

    pub fn event_id(&self, name: &str) -> Option<i32> {
        self.events.iter().position(|e| e.eq_ignore_ascii_case(name)).map(|i| i as i32)
    }

    pub fn event_name(&self, id: i32) -> Option<&str> {
        self.events.get(usize::try_from(id).ok()?).map(String::as_str)
    }

    /// Every (state machine, state reached, transition) for transitions on `event`,
    /// from any state or wildcard.
    pub fn transitions_on(&self, event: i32) -> Vec<(GenId, &State, Transition)> {
        let mut out: Vec<(GenId, &State, Transition)> = Vec::new();
        for (gi, g) in self.generators.iter().enumerate() {
            let Generator::StateMachine { states, wildcards, .. } = g else { continue };
            let targets = wildcards.iter().chain(states.iter().flat_map(|s| &s.transitions));
            for t in targets.filter(|t| t.event == event && !t.disabled()) {
                if let Some(s) = states.iter().find(|s| s.id == t.to_state)
                    && !out.iter().any(|(g, x, y)| *g == gi && x.id == s.id && y.to_nested == t.to_nested)
                {
                    out.push((gi, s, t.clone()));
                }
            }
        }
        out
    }
}

/// Behaviour graphs of one character project, by lowercase relative path
/// (`behaviors\0_master.hkx`), with event lookups across them.
#[derive(Default)]
pub struct Project {
    pub graphs: Vec<(String, BehaviorGraph)>,
    /// The character the project was loaded for, when loaded from a project file.
    pub character: Option<Character>,
}

/// A behaviour project's character (`hkbCharacterStringData`): paths are
/// project-relative with backslashes, as written.
#[derive(Debug, Clone, Default)]
pub struct Character {
    pub name: String,
    /// Havok skeleton (`Character Assets Dog\skeleton.HKX`).
    pub rig: String,
    /// Root behaviour graph (`Behaviors\DogBehavior.hkx`).
    pub behavior: String,
}

/// Strings at `o`: an hkArray of hkStringPtr.
fn string_array(p: &Packfile, o: u32) -> Vec<String> {
    let (data, n) = p.array(o);
    data.map(|d| (0..n as u32).filter_map(|i| p.string(d + i * 8)).collect()).unwrap_or_default()
}

/// The first character file a project file (`hkbProjectStringData`) lists.
pub fn project_character_file(bytes: &[u8]) -> Result<String> {
    let p = Packfile::parse(bytes)?;
    let o = p.objects_of("hkbProjectStringData").next().ok_or_else(|| crate::Error::Corrupt("no hkbProjectStringData".into()))?;
    // hkReferencedObject header, then animation, behavior and character filenames.
    string_array(&p, o + 0x30).into_iter().next().ok_or_else(|| crate::Error::Corrupt("project lists no character".into()))
}

impl Character {
    pub fn parse(bytes: &[u8]) -> Result<Character> {
        let p = Packfile::parse(bytes)?;
        let o = p.objects_of("hkbCharacterStringData").next().ok_or_else(|| crate::Error::Corrupt("no hkbCharacterStringData".into()))?;
        // Seven arrays (skins, animations, properties, retargeting, LODs, mirroring)
        // after the header, then name, rig, ragdoll and behaviour file names.
        let s = |at: u32| p.string(o + at).unwrap_or_default();
        Ok(Character { name: s(0xa0), rig: s(0xa8), behavior: s(0xb8) })
    }
}

impl Project {
    /// Load `root` (e.g. `behaviors\0_master.hkx`) and every graph it references.
    /// `read` takes a project-relative path with forward slashes.
    pub fn load(root: &str, read: impl Fn(&str) -> Option<Vec<u8>>) -> Project {
        let mut project = Project::default();
        let mut queue = vec![root.to_ascii_lowercase().replace('/', "\\")];
        while let Some(rel) = queue.pop() {
            if project.graphs.iter().any(|(p, _)| *p == rel) {
                continue;
            }
            let Some(bytes) = read(&rel.replace('\\', "/")) else { continue };
            let Ok(g) = BehaviorGraph::parse(&bytes) else { continue };
            for node in &g.generators {
                if let Generator::Reference { behavior, .. } = node {
                    queue.push(behavior.to_ascii_lowercase().replace('/', "\\"));
                }
            }
            project.graphs.push((rel, g));
        }
        project
    }

    /// Load a project from its project file (`DogProject.hkx`, relative to the
    /// project directory `read` resolves against): its character, then the
    /// character's root behaviour graph and everything that references.
    pub fn load_project(file: &str, read: impl Fn(&str) -> Option<Vec<u8>>) -> Result<Project> {
        let norm = |p: &str| p.to_ascii_lowercase().replace('\\', "/");
        let bytes = read(&norm(file)).ok_or_else(|| crate::Error::Corrupt(format!("{file} not found")))?;
        let character_file = project_character_file(&bytes)?;
        let bytes = read(&norm(&character_file)).ok_or_else(|| crate::Error::Corrupt(format!("{character_file} not found")))?;
        let character = Character::parse(&bytes)?;
        let mut project = Project::load(&character.behavior, read);
        project.character = Some(character);
        Ok(project)
    }

    pub fn graph_index(&self, path: &str) -> Option<usize> {
        let path = path.to_ascii_lowercase().replace('/', "\\");
        self.graphs.iter().position(|(p, _)| *p == path)
    }
}
