//! Behaviour graphs (`hkbBehaviorGraph`): the generator tree (state machines,
//! blenders, selectors, clips, references to other graphs), state transitions,
//! clip triggers, variables, variable bindings and the modifiers that steer
//! control flow. [`runtime`] runs them; [`resolve`] answers static "which clips
//! does this event play?" questions.

pub mod expr;
pub mod resolve;
pub mod runtime;

pub use resolve::{Playback, PlayedClip, RaisedEvent};

use crate::{Packfile, Result};

pub type GenId = usize;
pub type ModId = usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipMode {
    SinglePlay,
    Looping,
    /// Held at `userControlledTimeFraction` of its length (a variable can be
    /// bound to it): sarcophagus lids rest on their trigger clip's first frame.
    UserControlled,
    /// Forwards, then backwards, and again.
    PingPong,
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
    /// When the event is accepted (`FLAG_USE_TRIGGER_INTERVAL`); outside it the
    /// event is ignored.
    pub trigger: Option<Interval>,
    /// When the transition may begin (`FLAG_USE_INITIATE_INTERVAL`); one triggered
    /// earlier waits for it.
    pub initiate: Option<Interval>,
}

/// `hkbStateMachineTimeInterval`: a window in the current state, opened and
/// closed by events or (when both events are -1) by seconds since the state was
/// entered. An exit time of 0 leaves it open.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Interval {
    pub enter_event: i32,
    pub exit_event: i32,
    pub enter_time: f32,
    pub exit_time: f32,
}

impl Interval {
    pub fn timed(&self) -> bool {
        self.enter_event < 0 && self.exit_event < 0
    }

    /// Whether `elapsed` seconds into the state lie within a timed interval.
    pub fn contains_time(&self, elapsed: f32) -> bool {
        elapsed >= self.enter_time && (self.exit_time <= 0.0 || elapsed <= self.exit_time)
    }
}

impl Transition {
    pub fn new(event: i32, to_state: i32, to_nested: Option<i32>) -> Transition {
        Transition {
            event,
            to_state,
            to_nested,
            flags: if to_nested.is_some() {
                FLAG_TO_NESTED_STATE_ID_IS_VALID
            } else {
                0
            },
            ..Default::default()
        }
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

    /// No other transition of the machine may interrupt this one's blend.
    pub fn uninterruptible(&self) -> bool {
        self.flags & FLAG_UNINTERRUPTIBLE_WHILE_PLAYING != 0
    }
}

const FLAG_USE_TRIGGER_INTERVAL: u16 = 0x1;
const FLAG_USE_INITIATE_INTERVAL: u16 = 0x2;
const FLAG_UNINTERRUPTIBLE_WHILE_PLAYING: u16 = 0x4;
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
    /// Character property bound to `boneWeights`, resolved for this character.
    pub bone_weights_property: Option<String>,
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
        /// `userControlledTimeFraction`, for [`ClipMode::UserControlled`].
        user_fraction: f32,
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
        /// Authored notification for state / transition changes.
        change_event: Option<EventProperty>,
        states: Vec<State>,
        wildcards: Vec<Transition>,
    },
    /// Weighted blend of children (`hkbBlenderGenerator`, `hkbPoseMatchingGenerator`).
    Blender {
        name: String,
        parameter: f32,
        min_cyclic: f32,
        max_cyclic: f32,
        flags: u16,
        children: Vec<BlendChild>,
    },
    /// One child at a time, picked by a (bound) index.
    Selector {
        name: String,
        children: Vec<GenId>,
        index: i8,
    },
    /// A generator wrapping one child: `hkbModifierGenerator` (running a modifier
    /// alongside it) and Bethesda's tagging / sync / cyclic-blend wrappers.
    Wrap {
        name: String,
        class: String,
        child: Option<GenId>,
        modifier: Option<ModId>,
    },
    /// `BSBoneSwitchGenerator`: children replace the default on their bones. A
    /// child's weights may instead come from a character property (`properties`,
    /// e.g. the left arm for a torch), as the character file sets it.
    BoneSwitch {
        name: String,
        default: Option<GenId>,
        children: Vec<(GenId, Vec<f32>)>,
        properties: Vec<Option<String>>,
    },
    /// Another behaviour file (e.g. `Behaviors\MT_Behavior.hkx`).
    Reference {
        name: String,
        behavior: String,
    },
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
            Generator::Blender { children, .. } => {
                children.iter().filter_map(|c| c.generator).collect()
            }
            Generator::Selector { children, .. } => children.clone(),
            Generator::Wrap { child, .. } => child.iter().copied().collect(),
            Generator::BoneSwitch {
                default, children, ..
            } => default
                .iter()
                .copied()
                .chain(children.iter().map(|c| c.0))
                .collect(),
            _ => Vec::new(),
        }
    }
}

/// Graph control and pose modifiers (others are kept by class name and ignored).
#[derive(Debug, Clone)]
pub enum Modifier {
    List(Vec<ModId>),
    /// `hkbEvaluateExpressionModifier`: assignments and conditional events, every frame.
    Expressions(Vec<String>),
    /// `BSEventEveryNEventsModifier`: raise `send` after every `n` (randomised from
    /// `min..=n`) occurrences of `check`.
    EveryN {
        check: i32,
        send: EventProperty,
        n: u8,
        min: u8,
        random: bool,
    },
    /// `hkbTimerModifier`: raise `alarm` `seconds` after activation.
    Timer {
        seconds: f32,
        alarm: EventProperty,
    },
    /// `hkbEventDrivenModifier`: `child` runs between `activate` and `deactivate` events.
    EventDriven {
        child: Option<ModId>,
        activate: i32,
        deactivate: i32,
        active_by_default: bool,
    },
    /// `BSEventOnDeactivateModifier`: raise `event` when deactivated.
    OnDeactivate(EventProperty),
    /// `BSIsActiveModifier`: bound `bIsActiveN` outputs follow whether it runs.
    IsActive {
        invert: [bool; 5],
    },
    /// `BSSpeedSamplerModifier`: the bound `speedOut` follows `goalSpeed` (the
    /// locomotion blends' parameter, in units per second).
    SpeedSampler {
        goal_speed: f32,
    },
    /// `hkbDampingModifier`: `dampedValue` chases `rawValue` through a PID step
    /// every update.
    Damping {
        kp: f32,
        ki: f32,
        kd: f32,
        raw: f32,
        damped: f32,
    },
    /// `BSInterpValueModifier`: the bound `result` goes from `source` towards
    /// `target`, the whole way over `duration` seconds (the 1HM graph's
    /// `SpeedDamped` following `Speed`, which picks the attacks on the move).
    InterpValue {
        source: f32,
        target: f32,
        result: f32,
        duration: f32,
    },
    /// `BSLookAtModifier`: turn a chain of bones (spine, neck, head) and the eyes
    /// towards a target. The engine applies it to the pose.
    LookAt(LookAt),
    /// `hkbFootIkControlsModifier`: foot placement runs while it does, with its gains.
    /// The engine applies it to the pose.
    FootIkControls(FootIkGains),
    /// `hkbTwistModifier`: authored axis, angle and bone endpoints.
    Twist(Twist),
    /// `hkbRotateCharacterModifier`: character rotation settings. These are
    /// exposed by the runtime but not yet applied to the actor's world transform.
    RotateCharacter(RotateCharacter),
    Other(String),
}

/// Authored `hkbRotateCharacterModifier` inputs. Coordinate space, composition
/// with root motion, and branch blending remain unresolved; see
/// `known_ambiguities/rotate-character.md`.
#[derive(Debug, Clone)]
pub struct RotateCharacter {
    pub enabled: bool,
    pub degrees_per_second: f32,
    pub speed_multiplier: f32,
    /// Preserve all four serialized components without interpreting the axis.
    pub axis: glam::Vec4,
}

impl RotateCharacter {
    fn read(p: &Packfile, o: u32) -> Self {
        Self {
            enabled: p.u8(o + 0x48) != 0,
            degrees_per_second: p.f32(o + 0x50),
            speed_multiplier: p.f32(o + 0x54),
            axis: glam::Vec4::new(
                p.f32(o + 0x60),
                p.f32(o + 0x64),
                p.f32(o + 0x68),
                p.f32(o + 0x6C),
            ),
        }
    }
}

/// `hkbTwistModifier` settings. Only single-bone, additive, LINEAR twists are
/// currently applied to poses; see `known_ambiguities/twist-modifier.md`.
#[derive(Debug, Clone)]
pub struct Twist {
    pub enabled: bool,
    pub axis: glam::Vec3,
    pub angle_degrees: f32,
    pub start_bone: i16,
    pub end_bone: i16,
    /// Serialized enum: 0 is LINEAR. Other values are retained, not applied.
    pub set_angle_method: u8,
    /// Serialized enum: 0 model coordinates, 1 local coordinates.
    pub axis_coordinates: u8,
    pub additive: bool,
}

impl Twist {
    fn read(p: &Packfile, o: u32) -> Self {
        Self {
            enabled: p.u8(o + 0x48) != 0,
            axis: glam::Vec3::new(p.f32(o + 0x50), p.f32(o + 0x54), p.f32(o + 0x58)),
            angle_degrees: p.f32(o + 0x60),
            start_bone: p.i16(o + 0x64),
            end_bone: p.i16(o + 0x66),
            set_angle_method: p.u8(o + 0x68),
            axis_coordinates: p.u8(o + 0x69),
            additive: p.u8(o + 0x6A) != 0,
        }
    }
}

#[cfg(test)]
mod twist_tests {
    use super::*;

    #[test]
    fn twist_reader_reads_distinct_authored_fields_and_signed_indices() {
        // Entirely synthetic packfile: empty class section and one data block.
        let data_start = 0xB0;
        let mut bytes = vec![0u8; data_start + 0x70];
        let word = |bytes: &mut [u8], at: usize, value: u32| {
            bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
        };
        word(&mut bytes, 0, 0x57E0_E057);
        word(&mut bytes, 4, 0x10C0_C010);
        word(&mut bytes, 12, 8);
        bytes[16] = 8;
        bytes[17] = 1;
        word(&mut bytes, 20, 2);
        bytes[0x40..0x4E].copy_from_slice(b"__classnames__");
        word(&mut bytes, 0x54, data_start as u32);
        bytes[0x70..0x78].copy_from_slice(b"__data__");
        word(&mut bytes, 0x84, data_start as u32);
        for at in [0x88, 0x8C, 0x90, 0x94, 0xA0] {
            word(&mut bytes, at, 0x70);
        }
        let d = &mut bytes[data_start..];
        d[0x48] = 1;
        for (at, value) in [(0x50, 2.0f32), (0x54, 3.0), (0x58, 4.0), (0x60, -35.0)] {
            word(d, at, value.to_bits());
        }
        d[0x64..0x66].copy_from_slice(&(-1i16).to_le_bytes());
        d[0x66..0x68].copy_from_slice(&(7i16).to_le_bytes());
        d[0x68] = 2;
        d[0x69] = 1;
        d[0x6A] = 1;
        let p = Packfile::parse(&bytes).unwrap();
        let t = Twist::read(&p, 0);
        assert!(t.enabled && t.additive);
        assert_eq!(t.axis, glam::Vec3::new(2.0, 3.0, 4.0));
        assert_eq!(t.angle_degrees, -35.0);
        assert_eq!((t.start_bone, t.end_bone), (-1, 7));
        assert_eq!((t.set_angle_method, t.axis_coordinates), (2, 1));
    }
}

#[cfg(test)]
mod rotate_character_tests {
    use super::*;

    #[test]
    fn reader_retains_distinct_rates_aligned_axis_and_static_enable() {
        // Synthetic packfile, with deliberately distinct values and nonzero
        // alignment padding so an incorrect packed-vector offset is caught.
        let data_start = 0xB0;
        let mut bytes = vec![0u8; data_start + 0x70];
        let word = |bytes: &mut [u8], at: usize, value: u32| {
            bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
        };
        word(&mut bytes, 0, 0x57E0_E057);
        word(&mut bytes, 4, 0x10C0_C010);
        word(&mut bytes, 12, 8);
        bytes[16] = 8;
        bytes[17] = 1;
        word(&mut bytes, 20, 2);
        bytes[0x40..0x4E].copy_from_slice(b"__classnames__");
        word(&mut bytes, 0x54, data_start as u32);
        bytes[0x70..0x78].copy_from_slice(b"__data__");
        word(&mut bytes, 0x84, data_start as u32);
        for at in [0x88, 0x8C, 0x90, 0x94, 0xA0] {
            word(&mut bytes, at, 0x70);
        }
        let d = &mut bytes[data_start..];
        d[0x48] = 1;
        for (at, value) in [
            (0x50, -120.0f32),
            (0x54, 0.25),
            (0x58, 99.0),
            (0x5C, 98.0),
            (0x60, 2.0),
            (0x64, 3.0),
            (0x68, 4.0),
            (0x6C, 5.0),
        ] {
            word(d, at, value.to_bits());
        }
        let p = Packfile::parse(&bytes).unwrap();
        let r = RotateCharacter::read(&p, 0);
        assert!(r.enabled);
        assert_eq!(r.degrees_per_second, -120.0);
        assert_eq!(r.speed_multiplier, 0.25);
        assert_eq!(r.axis, glam::Vec4::new(2.0, 3.0, 4.0, 5.0));
        bytes[data_start + 0x48] = 0;
        assert!(!RotateCharacter::read(&Packfile::parse(&bytes).unwrap(), 0).enabled);
    }
}

/// `hkbFootIkGains` (bound members, such as
/// `controlData/gains/worldFromModelFeedbackGain`, are read from the graph's
/// variables at run time). Gains are per frame at 30 Hz.
#[derive(Debug, Clone, Default)]
pub struct FootIkGains {
    /// Fading foot placement in and out.
    pub on_off: f32,
    /// Ground offsets easing up and down.
    pub ground_ascending: f32,
    pub ground_descending: f32,
    /// How much of the leg IK applies to a planted / raised foot.
    pub foot_planted: f32,
    pub foot_raised: f32,
    pub foot_unlock: f32,
    /// The body (model) following the feet's error.
    pub world_from_model_feedback: f32,
    /// How much of that error to take below rather than above (0.5 even).
    pub error_up_down_bias: f32,
    pub align_world_from_model: f32,
    pub hip_orientation: f32,
    pub max_knee_angle_difference: f32,
    pub ankle_orientation: f32,
}

/// `BSLookAtModifier` settings (bound members, such as `lookAtTarget` and
/// `bones:0/enabled`, are read from the graph's variables at run time).
#[derive(Debug, Clone)]
pub struct LookAt {
    pub bones: Vec<LookAtBone>,
    pub eye_bones: Vec<LookAtBone>,
    /// Targets further than this from straight ahead aren't looked at.
    pub limit_degrees: f32,
    pub limit_threshold_degrees: f32,
    pub continue_outside_limit: bool,
    pub on_gain: f32,
    pub off_gain: f32,
    pub use_bone_gains: bool,
    pub look_at_target: bool,
}

/// A bone of a look-at chain (`BSLookAtModifier::BoneData`).
#[derive(Debug, Clone)]
pub struct LookAtBone {
    /// Havok skeleton bone.
    pub index: i16,
    /// The bone's forward axis in its own space.
    pub forward: glam::Vec3,
    pub limit_degrees: f32,
    pub on_gain: f32,
    pub off_gain: f32,
    pub enabled: bool,
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

/// A read-only input bound to a named character property. Property indices are
/// graph-local; names resolve them against the project's character file.
#[derive(Debug, Clone)]
pub struct PropertyBinding {
    pub member: String,
    pub property: String,
}

pub struct BehaviorGraph {
    pub name: String,
    pub root: Option<GenId>,
    pub generators: Vec<Generator>,
    /// Variable bindings of each generator (same indices as `generators`).
    pub bindings: Vec<Vec<Binding>>,
    /// Read-only scalar character inputs, indexed like `generators`.
    pub property_bindings: Vec<Vec<PropertyBinding>>,
    pub modifiers: Vec<Modifier>,
    pub modifier_bindings: Vec<Vec<Binding>>,
    pub modifier_property_bindings: Vec<Vec<PropertyBinding>>,
    pub events: Vec<String>,
    /// Graph variables, their types and initial (word) values.
    pub variables: Vec<String>,
    pub variable_types: Vec<VarType>,
    pub variable_defaults: Vec<i32>,
}

impl BehaviorGraph {
    /// A graph built in code (tests, tools): no variables, bindings or modifiers.
    pub fn new(
        name: &str,
        root: Option<GenId>,
        generators: Vec<Generator>,
        events: Vec<String>,
    ) -> BehaviorGraph {
        BehaviorGraph {
            name: name.to_owned(),
            root,
            bindings: vec![Vec::new(); generators.len()],
            property_bindings: vec![Vec::new(); generators.len()],
            generators,
            modifiers: Vec::new(),
            modifier_bindings: Vec::new(),
            modifier_property_bindings: Vec::new(),
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
        self.variable_defaults.push(if ty == VarType::Real {
            value.to_bits() as i32
        } else {
            value as i32
        });
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
        self.bindings
            .get(g)?
            .iter()
            .find(|b| b.member == member)
            .map(|b| b.variable)
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
    property_bindings: Vec<Vec<PropertyBinding>>,
    mod_ids: std::collections::HashMap<u32, ModId>,
    modifiers: Vec<Modifier>,
    modifier_bindings: Vec<Vec<Binding>>,
    modifier_property_bindings: Vec<Vec<PropertyBinding>>,
    /// The graph's character property names (what type-1 bindings index).
    character_properties: Vec<String>,
}

impl<'a> Reader<'a> {
    fn class(&self, o: u32) -> &'a str {
        self.classes.get(&o).copied().unwrap_or("")
    }

    /// Pointer array (hkArray of object pointers) at `o`.
    fn ptr_array(&self, o: u32) -> Vec<u32> {
        let (Some(data), n) = self.p.array(o) else {
            return Vec::new();
        };
        (0..n as u32)
            .filter_map(|i| self.p.ptr(data + i * ARRAY_PTR_SIZE))
            .collect()
    }

    fn transitions(&self, array_obj: Option<u32>) -> Vec<Transition> {
        let Some(a) = array_obj else {
            return Vec::new();
        };
        let (Some(data), n) = self.p.array(a + 0x10) else {
            return Vec::new();
        };
        (0..n as u32)
            .map(|i| {
                let t = data + i * 0x48;
                let flags = self.p.u16(t + 0x42);
                let effect = self
                    .p
                    .ptr(t + 0x20)
                    .filter(|&e| self.class(e) == "hkbBlendingTransitionEffect");
                let condition = self
                    .p
                    .ptr(t + 0x28)
                    .filter(|&c| self.class(c) == "hkbExpressionCondition");
                Transition {
                    event: self.p.i32(t + 0x30),
                    to_state: self.p.i32(t + 0x34),
                    to_nested: (flags & FLAG_TO_NESTED_STATE_ID_IS_VALID != 0)
                        .then(|| self.p.i32(t + 0x3C)),
                    flags,
                    blend: effect.map(|e| self.p.f32(e + 0x50)),
                    blend_variable: effect
                        .and_then(|e| {
                            self.bindings_of(e)
                                .into_iter()
                                .find(|b| b.member == "duration")
                        })
                        .map(|b| b.variable),
                    condition: condition.and_then(|c| self.p.string(c + 0x10)),
                    priority: self.p.i16(t + 0x40),
                    trigger: (flags & FLAG_USE_TRIGGER_INTERVAL != 0).then(|| self.interval(t)),
                    initiate: (flags & FLAG_USE_INITIATE_INTERVAL != 0)
                        .then(|| self.interval(t + 0x10)),
                }
            })
            .collect()
    }

    fn interval(&self, o: u32) -> Interval {
        Interval {
            enter_event: self.p.i32(o),
            exit_event: self.p.i32(o + 4),
            enter_time: self.p.f32(o + 8),
            exit_time: self.p.f32(o + 0xC),
        }
    }

    /// Variable bindings of the bindable object at `o`.
    fn bindings_of(&self, o: u32) -> Vec<Binding> {
        let Some(set) = self.p.ptr(o + 0x10) else {
            return Vec::new();
        };
        let (Some(data), n) = self.p.array(set + 0x10) else {
            return Vec::new();
        };
        (0..n as u32)
            .map(|i| data + i * 0x28)
            // Binding type 0: a variable (1 would be a character property).
            .filter(|&b| self.p.u8(b + 0x21) == 0)
            .filter_map(|b| {
                Some(Binding {
                    member: self.p.string(b)?,
                    variable: usize::try_from(self.p.i32(b + 0x1C)).ok()?,
                })
            })
            .collect()
    }

    /// The character property bound to `member` of the object at `o`, if any.
    fn character_property(&self, o: u32, member: &str) -> Option<String> {
        let set = self.p.ptr(o + 0x10)?;
        let (Some(data), n) = self.p.array(set + 0x10) else {
            return None;
        };
        (0..n as u32)
            .map(|i| data + i * 0x28)
            .filter(|&b| self.p.u8(b + 0x21) == 1 && self.p.string(b).is_some_and(|m| m == member))
            .find_map(|b| {
                self.character_properties
                    .get(usize::try_from(self.p.i32(b + 0x1C)).ok()?)
                    .cloned()
            })
    }

    fn property_bindings_of(&self, o: u32) -> Vec<PropertyBinding> {
        let Some(set) = self.p.ptr(o + 0x10) else {
            return Vec::new();
        };
        let (Some(data), n) = self.p.array(set + 0x10) else {
            return Vec::new();
        };
        (0..n as u32)
            .map(|i| data + i * 0x28)
            .filter(|&b| self.p.u8(b + 0x21) == 1)
            .filter_map(|b| {
                Some(PropertyBinding {
                    member: self.p.string(b)?,
                    property: self
                        .character_properties
                        .get(usize::try_from(self.p.i32(b + 0x1C)).ok()?)?
                        .to_ascii_lowercase(),
                })
            })
            .collect()
    }

    /// String payload of the `hkbEventPayload` pointed to from `slot`.
    fn payload(&self, slot: u32) -> Option<String> {
        self.p
            .ptr(slot)
            .filter(|&o| self.class(o) == "hkbStringEventPayload")
            .and_then(|o| self.p.string(o + 0x10))
    }

    /// `hkbEventProperty` (id, payload) at `o`.
    fn event_property(&self, o: u32) -> EventProperty {
        EventProperty {
            event: self.p.i32(o),
            payload: self.payload(o + 8),
        }
    }

    fn event_properties(&self, array_obj: Option<u32>) -> Vec<EventProperty> {
        let Some(a) = array_obj else {
            return Vec::new();
        };
        let (Some(data), n) = self.p.array(a + 0x10) else {
            return Vec::new();
        };
        (0..n as u32)
            .map(|i| self.event_property(data + i * 0x10))
            .collect()
    }

    fn triggers(&self, array_obj: Option<u32>) -> Vec<Trigger> {
        let Some(a) = array_obj else {
            return Vec::new();
        };
        let (Some(data), n) = self.p.array(a + 0x10) else {
            return Vec::new();
        };
        (0..n as u32)
            .map(|i| {
                let t = data + i * 0x20;
                Trigger {
                    time: self.p.f32(t),
                    event: self.p.i32(t + 8),
                    from_end: self.p.u8(t + 0x18) != 0,
                    payload: self.payload(t + 0x10),
                }
            })
            .collect()
    }

    /// `hkbBoneWeightArray` at `o`.
    fn bone_weights(&self, o: Option<u32>) -> Option<Vec<f32>> {
        let o = o?;
        let (Some(data), n) = self.p.array(o + 0x30) else {
            return None;
        };
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
        self.modifier_property_bindings
            .push(self.property_bindings_of(o));
        let p = self.p;
        let class = self.class(o);
        let m = match class {
            "hkbModifierList" => Modifier::List(
                self.ptr_array(o + 0x50)
                    .into_iter()
                    .map(|m| self.modifier(m))
                    .collect(),
            ),
            "hkbEvaluateExpressionModifier" => {
                let lines = p
                    .ptr(o + 0x50)
                    .map(|a| {
                        let (data, n) = p.array(a + 0x10);
                        data.map(|d| {
                            (0..n as u32)
                                .filter_map(|i| p.string(d + i * 0x18))
                                .collect()
                        })
                        .unwrap_or_default()
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
            "hkbTimerModifier" => Modifier::Timer {
                seconds: p.f32(o + 0x50),
                alarm: self.event_property(o + 0x58),
            },
            "hkbEventDrivenModifier" => Modifier::EventDriven {
                child: p.ptr(o + 0x50).map(|m| self.modifier(m)),
                activate: p.i32(o + 0x58),
                deactivate: p.i32(o + 0x5C),
                active_by_default: p.u8(o + 0x60) != 0,
            },
            "BSEventOnDeactivateModifier" => Modifier::OnDeactivate(self.event_property(o + 0x50)),
            "BSIsActiveModifier" => Modifier::IsActive {
                invert: std::array::from_fn(|i| p.u8(o + 0x51 + 2 * i as u32) != 0),
            },
            // After hkbModifier (enable at 0x48): state, direction, goal speed, speed out.
            "BSSpeedSamplerModifier" => Modifier::SpeedSampler {
                goal_speed: p.f32(o + 0x50),
            },
            // kP, kI, kD, scalar / vector flags, raw and damped values.
            "hkbDampingModifier" if p.u8(o + 0x5C) != 0 => Modifier::Damping {
                kp: p.f32(o + 0x50),
                ki: p.f32(o + 0x54),
                kd: p.f32(o + 0x58),
                raw: p.f32(o + 0x60),
                damped: p.f32(o + 0x64),
            },
            // After hkbModifier: source, target, result, duration.
            "BSInterpValueModifier" => Modifier::InterpValue {
                source: p.f32(o + 0x50),
                target: p.f32(o + 0x54),
                result: p.f32(o + 0x58),
                duration: p.f32(o + 0x5C),
            },
            "hkbTwistModifier" => Modifier::Twist(Twist::read(p, o)),
            "hkbRotateCharacterModifier" => Modifier::RotateCharacter(RotateCharacter::read(p, o)),
            "BSLookAtModifier" => {
                let bones = |at: u32| {
                    let (data, n) = p.array(o + at);
                    // BoneData: index, forward axis (16-aligned), limit, gains, enabled.
                    data.map(|d| {
                        (0..n as u32)
                            .map(|i| {
                                let b = d + i * 0x40;
                                LookAtBone {
                                    index: p.i16(b),
                                    forward: glam::Vec3::new(
                                        p.f32(b + 0x10),
                                        p.f32(b + 0x14),
                                        p.f32(b + 0x18),
                                    ),
                                    limit_degrees: p.f32(b + 0x20),
                                    on_gain: p.f32(b + 0x24),
                                    off_gain: p.f32(b + 0x28),
                                    enabled: p.u8(b + 0x2C) != 0,
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default()
                };
                Modifier::LookAt(LookAt {
                    look_at_target: p.u8(o + 0x50) != 0,
                    bones: bones(0x58),
                    eye_bones: bones(0x68),
                    limit_degrees: p.f32(o + 0x78),
                    limit_threshold_degrees: p.f32(o + 0x7C),
                    continue_outside_limit: p.u8(o + 0x80) != 0,
                    on_gain: p.f32(o + 0x84),
                    off_gain: p.f32(o + 0x88),
                    use_bone_gains: p.u8(o + 0x8C) != 0,
                })
            }
            // After hkbModifier: hkbFootIkControlData's gains, then the legs.
            "hkbFootIkControlsModifier" => {
                let f = |i: u32| p.f32(o + 0x50 + i * 4);
                Modifier::FootIkControls(FootIkGains {
                    on_off: f(0),
                    ground_ascending: f(1),
                    ground_descending: f(2),
                    foot_planted: f(3),
                    foot_raised: f(4),
                    foot_unlock: f(5),
                    world_from_model_feedback: f(6),
                    error_up_down_bias: f(7),
                    align_world_from_model: f(8),
                    hip_orientation: f(9),
                    max_knee_angle_difference: f(10),
                    ankle_orientation: f(11),
                })
            }
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
        self.property_bindings.push(self.property_bindings_of(o));
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
                    2 => ClipMode::UserControlled,
                    3 => ClipMode::PingPong,
                    _ => ClipMode::Other,
                },
                speed: p.f32(o + 0x64),
                user_fraction: p.f32(o + 0x6C),
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
                    start_variable: self.bindings[id]
                        .iter()
                        .find(|b| b.member == "startStateId")
                        .map(|b| b.variable),
                    // syncVariableIndex at 0x7C; startStateMode at 0x86.
                    start_mode: match (p.u8(o + 0x86), p.i32(o + 0x7C)) {
                        (1, v) if v >= 0 => StartMode::Sync(v as usize),
                        (2, _) => StartMode::Random,
                        _ => StartMode::Default,
                    },
                    change_event: (p.i32(o + 0x48) >= 0).then(|| self.event_property(o + 0x48)),
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
                        weight_variable: self
                            .bindings_of(c)
                            .into_iter()
                            .find(|b| b.member == "weight")
                            .map(|b| b.variable),
                        bone_weights: self.bone_weights(p.ptr(c + 0x38)),
                        bone_weights_property: self.character_property(c, "boneWeights"),
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
                Generator::Selector {
                    name,
                    children: gens.into_iter().map(|g| self.generator(g)).collect(),
                    index: p.u8(o + 0x58) as i8,
                }
            }
            "BSBoneSwitchGenerator" => {
                let default = p.ptr(o + 0x50).map(|g| self.generator(g));
                let mut children = Vec::new();
                let mut properties = Vec::new();
                for d in self.ptr_array(o + 0x58) {
                    if let Some(g) = p.ptr(d + 0x30) {
                        let weights = self.bone_weights(p.ptr(d + 0x38)).unwrap_or_default();
                        properties.push(self.character_property(d, "spBoneWeight"));
                        children.push((self.generator(g), weights));
                    }
                }
                Generator::BoneSwitch {
                    name,
                    default,
                    children,
                    properties,
                }
            }
            "hkbModifierGenerator" => {
                let modifier = p.ptr(o + 0x48).map(|m| self.modifier(m));
                Generator::Wrap {
                    name,
                    class: class.to_owned(),
                    child: p.ptr(o + 0x50).map(|g| self.generator(g)),
                    modifier,
                }
            }
            "BSSynchronizedClipGenerator"
            | "BSCyclicBlendTransitionGenerator"
            | "BSiStateTaggingGenerator"
            | "BSOffsetAnimationGenerator" => Generator::Wrap {
                name,
                class: class.to_owned(),
                child: p.ptr(o + 0x50).map(|g| self.generator(g)),
                modifier: None,
            },
            "hkbBehaviorReferenceGenerator" => Generator::Reference {
                name,
                behavior: p.string(o + 0x48).unwrap_or_default(),
            },
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
            ptr.map(|d| {
                (0..n as u32)
                    .map(|i| p.string(d + i * 8).unwrap_or_default())
                    .collect()
            })
            .unwrap_or_default()
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
                ptr.map(|d| (0..n as u32).map(|i| p.i32(d + i * 4)).collect())
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        let mut r = Reader {
            p: &p,
            classes: p
                .objects
                .iter()
                .map(|o| (o.offset, o.class.as_str()))
                .collect(),
            ids: Default::default(),
            generators: Vec::new(),
            bindings: Vec::new(),
            property_bindings: Vec::new(),
            mod_ids: Default::default(),
            modifiers: Vec::new(),
            modifier_bindings: Vec::new(),
            modifier_property_bindings: Vec::new(),
            character_properties: string_array(0x40),
        };
        let root = p.ptr(graph + 0x80).map(|g| r.generator(g));
        // A BSCyclicBlendTransitionGenerator's fBlendParameter is its blender's
        // parameter (the blender's own is left unbound: locomotion's Direction).
        for g in 0..r.generators.len() {
            let Generator::Wrap {
                class,
                child: Some(c),
                ..
            } = &r.generators[g]
            else {
                continue;
            };
            let c = *c;
            if class != "BSCyclicBlendTransitionGenerator"
                || !matches!(r.generators[c], Generator::Blender { .. })
                || r.bindings[c].iter().any(|b| b.member == "blendParameter")
            {
                continue;
            }
            if let Some(v) = r.bindings[g]
                .iter()
                .find(|b| b.member == "fBlendParameter")
                .map(|b| b.variable)
            {
                r.bindings[c].push(Binding {
                    member: "blendParameter".into(),
                    variable: v,
                });
            }
        }
        Ok(BehaviorGraph {
            name,
            root,
            generators: r.generators,
            bindings: r.bindings,
            property_bindings: r.property_bindings,
            modifiers: r.modifiers,
            modifier_bindings: r.modifier_bindings,
            modifier_property_bindings: r.modifier_property_bindings,
            events,
            variables,
            variable_types,
            variable_defaults,
        })
    }

    pub fn event_id(&self, name: &str) -> Option<i32> {
        self.events
            .iter()
            .position(|e| e.eq_ignore_ascii_case(name))
            .map(|i| i as i32)
    }

    pub fn event_name(&self, id: i32) -> Option<&str> {
        self.events
            .get(usize::try_from(id).ok()?)
            .map(String::as_str)
    }

    /// Every (state machine, state reached, transition) for transitions on `event`,
    /// from any state or wildcard.
    pub fn transitions_on(&self, event: i32) -> Vec<(GenId, &State, Transition)> {
        let mut out: Vec<(GenId, &State, Transition)> = Vec::new();
        for (gi, g) in self.generators.iter().enumerate() {
            let Generator::StateMachine {
                states, wildcards, ..
            } = g
            else {
                continue;
            };
            let targets = wildcards
                .iter()
                .chain(states.iter().flat_map(|s| &s.transitions));
            for t in targets.filter(|t| t.event == event && !t.disabled()) {
                if let Some(s) = states.iter().find(|s| s.id == t.to_state)
                    && !out
                        .iter()
                        .any(|(g, x, y)| *g == gi && x.id == s.id && y.to_nested == t.to_nested)
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
    /// Foot placement settings (`hkbFootIkDriverInfo`), if the character has them.
    pub foot_ik: Option<FootIk>,
    /// Bone weight properties (lowercase name -> weight per bone), which graphs
    /// bind bone switches to.
    pub bone_weights: std::collections::HashMap<String, Vec<f32>>,
    /// Authored scalar properties (lowercase name -> bool/int/real converted to
    /// f32). Pointer/vector properties are excluded; these inputs are immutable.
    pub scalar_properties: std::collections::HashMap<String, f32>,
}

/// `hkbFootIkDriverInfo`: the legs and how far to look for the ground.
#[derive(Debug, Clone)]
pub struct FootIk {
    pub legs: Vec<FootIkLeg>,
    pub raycast_up: f32,
    pub raycast_down: f32,
    pub original_ground_height: f32,
    pub vertical_offset: f32,
    pub forward_align: f32,
    pub sideways_align: f32,
}

/// A leg (`hkbFootIkDriverInfoLeg`); heights in model space, bones by Havok index.
#[derive(Debug, Clone)]
pub struct FootIkLeg {
    pub knee_axis: glam::Vec3,
    pub foot_end: glam::Vec3,
    pub planted_ankle_height: f32,
    pub raised_ankle_height: f32,
    pub max_ankle_height: f32,
    pub min_ankle_height: f32,
    pub max_knee_degrees: f32,
    pub min_knee_degrees: f32,
    pub max_ankle_degrees: f32,
    pub hip: i16,
    pub knee: i16,
    pub ankle: i16,
}

/// Strings at `o`: an hkArray of hkStringPtr.
fn string_array(p: &Packfile, o: u32) -> Vec<String> {
    let (data, n) = p.array(o);
    data.map(|d| (0..n as u32).filter_map(|i| p.string(d + i * 8)).collect())
        .unwrap_or_default()
}

/// The first character file a project file (`hkbProjectStringData`) lists.
pub fn project_character_file(bytes: &[u8]) -> Result<String> {
    let p = Packfile::parse(bytes)?;
    let o = p
        .objects_of("hkbProjectStringData")
        .next()
        .ok_or_else(|| crate::Error::Corrupt("no hkbProjectStringData".into()))?;
    // hkReferencedObject header, then animation, behavior and character filenames.
    string_array(&p, o + 0x30)
        .into_iter()
        .next()
        .ok_or_else(|| crate::Error::Corrupt("project lists no character".into()))
}

impl Character {
    fn read_scalar_properties(
        p: &Packfile,
        data: Option<u32>,
        names: &[String],
    ) -> std::collections::HashMap<String, f32> {
        let mut properties = std::collections::HashMap::new();
        let Some(data) = data else { return properties };
        let (Some(infos), ni) = p.array(data + 0x60) else {
            return properties;
        };
        let Some(values) = p.ptr(data + 0x80) else {
            return properties;
        };
        let (Some(words), nw) = p.array(values + 0x10) else {
            return properties;
        };
        for (i, name) in names.iter().enumerate().take(nw.min(ni) as usize) {
            let word = p.i32(words + i as u32 * 4);
            let value = match p.u8(infos + i as u32 * 6 + 4) {
                0 => f32::from(word != 0),
                1..=3 => word as f32,
                4 => f32::from_bits(word as u32),
                _ => continue,
            };
            properties.insert(name.to_ascii_lowercase(), value);
        }
        properties
    }

    pub fn parse(bytes: &[u8]) -> Result<Character> {
        let p = Packfile::parse(bytes)?;
        let o = p
            .objects_of("hkbCharacterStringData")
            .next()
            .ok_or_else(|| crate::Error::Corrupt("no hkbCharacterStringData".into()))?;
        // Seven arrays (skins, animations, properties, retargeting, LODs, mirroring)
        // after the header, then name, rig, ragdoll and behaviour file names.
        let s = |at: u32| p.string(o + at).unwrap_or_default();
        let foot_ik = p.objects_of("hkbFootIkDriverInfo").next().map(|f| {
            let v = |at: u32| glam::Vec3::new(p.f32(at), p.f32(at + 4), p.f32(at + 8));
            let (data, n) = p.array(f + 0x10);
            let legs = data
                .map(|d| {
                    (0..n as u32)
                        .map(|i| {
                            // prevAnkleRotLS, knee axis, foot end, heights, angles, bones.
                            let l = d + i * 0x60;
                            FootIkLeg {
                                knee_axis: v(l + 0x10),
                                foot_end: v(l + 0x20),
                                planted_ankle_height: p.f32(l + 0x30),
                                raised_ankle_height: p.f32(l + 0x34),
                                max_ankle_height: p.f32(l + 0x38),
                                min_ankle_height: p.f32(l + 0x3C),
                                max_knee_degrees: p.f32(l + 0x40),
                                min_knee_degrees: p.f32(l + 0x44),
                                max_ankle_degrees: p.f32(l + 0x48),
                                hip: p.i16(l + 0x4C),
                                knee: p.i16(l + 0x4E),
                                ankle: p.i16(l + 0x50),
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();
            FootIk {
                legs,
                raycast_up: p.f32(f + 0x20),
                raycast_down: p.f32(f + 0x24),
                original_ground_height: p.f32(f + 0x28),
                vertical_offset: p.f32(f + 0x2C),
                forward_align: p.f32(f + 0x34),
                sideways_align: p.f32(f + 0x38),
            }
        });
        // Property values (`hkbCharacterData`): a word per property, indexing the
        // variants (bone weight arrays) for pointer properties.
        let mut bone_weights = std::collections::HashMap::new();
        let names = string_array(&p, o + 0x50);
        let data = p.objects_of("hkbCharacterData").next();
        let scalar_properties = Self::read_scalar_properties(&p, data, &names);
        // Property types (`hkbVariableInfo`: role, flags, type); 5 is a pointer.
        let (infos, ni) = data.map_or((None, 0), |c| p.array(c + 0x60));
        if let Some(values) = data.and_then(|c| p.ptr(c + 0x80)) {
            let (words, nw) = p.array(values + 0x10);
            let (variants, nv) = p.array(values + 0x30);
            if let (Some(words), Some(variants), Some(infos)) = (words, variants, infos) {
                for (i, name) in names.iter().enumerate().take(nw.min(ni) as usize) {
                    if p.u8(infos + i as u32 * 6 + 4) != 5 {
                        continue;
                    }
                    let v = p.i32(words + i as u32 * 4);
                    if v < 0 || v as usize >= nv as usize {
                        continue;
                    }
                    let Some(obj) = p
                        .ptr(variants + v as u32 * 8)
                        .filter(|&o| p.object_class(o) == Some("hkbBoneWeightArray"))
                    else {
                        continue;
                    };
                    let (data, n) = p.array(obj + 0x30);
                    if let Some(d) = data {
                        bone_weights.insert(
                            name.to_ascii_lowercase(),
                            (0..n as u32).map(|k| p.f32(d + k * 4)).collect(),
                        );
                    }
                }
            }
        }
        Ok(Character {
            name: s(0xa0),
            rig: s(0xa8),
            behavior: s(0xb8),
            foot_ik,
            bone_weights,
            scalar_properties,
        })
    }
}

#[cfg(test)]
mod character_property_tests {
    use super::*;

    fn word(data: &mut [u8], at: usize, value: u32) {
        data[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn fixture() -> Packfile {
        // Synthetic character value tables and modifier bindings. No asset bytes.
        let mut data = vec![0u8; 0x600];
        word(&mut data, 0x68, 7);
        word(&mut data, 0x198, 7);
        for (i, (ty, value)) in [
            (0, -5i32),
            (1, -7),
            (2, -300),
            (3, -40000),
            (4, (-1.25f32).to_bits() as i32),
            (5, 4),
            (6, 5),
        ]
        .into_iter()
        .enumerate()
        {
            data[0x100 + i * 6 + 4] = ty;
            word(&mut data, 0x200 + i * 4, value as u32);
        }
        word(&mut data, 0x318, 4);
        // A blender at 0x280 can also use this binding set on its one child.
        word(&mut data, 0x2E8, 1);
        word(&mut data, 0x5C0, 1.0f32.to_bits());
        let mut pointers = vec![
            (0x60, 0x100),
            (0x80, 0x180),
            (0x190, 0x200),
            (0x290, 0x300),
            (0x310, 0x380),
            (0x2E0, 0x570),
            (0x570, 0x580),
            (0x590, 0x300),
        ];
        for (i, (member, index, kind)) in [
            ("rawValue", 4, 1),
            ("enable", 0, 1),
            ("invalid", 99, 1),
            ("dampedValue", 2, 0),
        ]
        .into_iter()
        .enumerate()
        {
            let b = 0x380 + i * 0x28;
            let s = 0x480 + i * 0x20;
            data[s..s + member.len()].copy_from_slice(member.as_bytes());
            word(&mut data, b + 0x1C, index);
            data[b + 0x20] = 0xFF; // unpartitioned binding
            data[b + 0x21] = kind;
            pointers.push((b as u32, s as u32));
        }
        let start = 0xB0;
        let end = data.len();
        let fixes = end + pointers.len() * 8;
        let mut bytes = vec![0u8; start + fixes];
        word(&mut bytes, 0, 0x57E0_E057);
        word(&mut bytes, 4, 0x10C0_C010);
        word(&mut bytes, 12, 8);
        bytes[16] = 8;
        bytes[17] = 1;
        word(&mut bytes, 20, 2);
        bytes[0x40..0x4E].copy_from_slice(b"__classnames__");
        word(&mut bytes, 0x54, start as u32);
        bytes[0x70..0x78].copy_from_slice(b"__data__");
        word(&mut bytes, 0x84, start as u32);
        word(&mut bytes, 0x88, end as u32);
        for at in [0x8C, 0x90, 0x94, 0xA0] {
            word(&mut bytes, at, fixes as u32);
        }
        bytes[start..start + end].copy_from_slice(&data);
        for (i, (source, target)) in pointers.into_iter().enumerate() {
            word(&mut bytes, start + end + i * 8, source);
            word(&mut bytes, start + end + i * 8 + 4, target);
        }
        Packfile::parse(&bytes).unwrap()
    }

    #[test]
    fn scalar_character_properties_decode_types_and_exclude_non_scalars() {
        let p = fixture();
        let names = [
            "Bool", "Byte", "Int16", "Int32", "Real", "Pointer", "Vector", "Missing",
        ]
        .map(String::from);
        let values = Character::read_scalar_properties(&p, Some(0), &names);
        assert_eq!(values.len(), 5);
        for (name, value) in [
            ("bool", 1.0),
            ("byte", -7.0),
            ("int16", -300.0),
            ("int32", -40000.0),
            ("real", -1.25),
        ] {
            assert_eq!(values[name], value);
        }
        assert!(Character::read_scalar_properties(&p, None, &names).is_empty());
    }

    #[test]
    fn property_bindings_use_graph_names_and_do_not_become_variable_outputs() {
        let p = fixture();
        let mut r = Reader {
            p: &p,
            classes: Default::default(),
            ids: Default::default(),
            generators: vec![],
            bindings: vec![],
            property_bindings: vec![],
            mod_ids: Default::default(),
            modifiers: vec![],
            modifier_bindings: vec![],
            modifier_property_bindings: vec![],
            character_properties: ["ENABLE", "Unused", "Output", "Other", "GAIN"]
                .map(String::from)
                .to_vec(),
        };
        let properties = r.property_bindings_of(0x280);
        assert_eq!(properties.len(), 2);
        assert_eq!(
            (&*properties[0].member, &*properties[0].property),
            ("rawValue", "gain")
        );
        assert_eq!(
            (&*properties[1].member, &*properties[1].property),
            ("enable", "enable")
        );
        let variables = r.bindings_of(0x280);
        assert_eq!(variables.len(), 1);
        assert_eq!(
            (&*variables[0].member, variables[0].variable),
            ("dampedValue", 2)
        );
        // The same bindable layout applies to generators. Registration retains
        // both input tables and memoising a shared node must not append either.
        r.classes.insert(0x280, "hkbClipGenerator");
        let id = r.generator(0x280);
        assert_eq!(r.generator(0x280), id);
        assert_eq!(r.property_bindings.len(), r.generators.len());
        assert_eq!(r.property_bindings[id].len(), 2);
        assert_eq!(r.property_bindings[id][0].property, "gain");
        assert_eq!(r.bindings[id].len(), 1);
    }

    #[test]
    fn blender_children_retain_bone_weight_property_names() {
        let mut p = fixture();
        p.data[0x480..0x48B].copy_from_slice(b"boneWeights");
        let mut r = Reader {
            p: &p,
            classes: [(0x280, "hkbBlenderGenerator")].into(),
            ids: Default::default(),
            generators: vec![],
            bindings: vec![],
            property_bindings: vec![],
            mod_ids: Default::default(),
            modifiers: vec![],
            modifier_bindings: vec![],
            modifier_property_bindings: vec![],
            character_properties: ["Unused", "Other", "Third", "Fourth", "ArmMask"]
                .map(String::from)
                .to_vec(),
        };
        let id = r.generator(0x280);
        let Generator::Blender { children, .. } = &r.generators[id] else {
            panic!("expected blender");
        };
        assert_eq!(children.len(), 1);
        assert_eq!(
            children[0].bone_weights_property.as_deref(),
            Some("ArmMask")
        );
        assert_eq!(children[0].weight, 1.0);
        assert!(children[0].bone_weights.is_none());
        assert!(children[0].weight_variable.is_none());
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
            let Some(bytes) = read(&rel.replace('\\', "/")) else {
                continue;
            };
            let Ok(g) = BehaviorGraph::parse(&bytes) else {
                continue;
            };
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
        let bytes =
            read(&norm(file)).ok_or_else(|| crate::Error::Corrupt(format!("{file} not found")))?;
        let character_file = project_character_file(&bytes)?;
        let bytes = read(&norm(&character_file))
            .ok_or_else(|| crate::Error::Corrupt(format!("{character_file} not found")))?;
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
