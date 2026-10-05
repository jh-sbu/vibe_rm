//! Behaviour graphs (`hkbBehaviorGraph`), reduced to what is needed to answer
//! "which clips does this animation event play?": the generator tree (state
//! machines, blends, selectors, clips, references to other graphs), state
//! transitions keyed by event, and clip triggers.

use crate::{Packfile, Result};

pub type GenId = usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipMode {
    SinglePlay,
    Looping,
    Other,
}

#[derive(Debug, Clone)]
pub struct Trigger {
    /// Seconds from the start of the clip, or from its end if `from_end`.
    pub time: f32,
    pub from_end: bool,
    /// Index into the graph's event names.
    pub event: i32,
}

#[derive(Debug, Clone, Copy)]
pub struct Transition {
    pub event: i32,
    pub to_state: i32,
    /// State to enter inside the target state's (nested) state machine, if any.
    pub to_nested: Option<i32>,
    pub flags: u16,
}

const FLAG_TO_NESTED_STATE_ID_IS_VALID: u16 = 0x2000;
const FLAG_DISABLED: u16 = 0x20;

#[derive(Debug, Clone)]
pub struct State {
    pub id: i32,
    pub name: String,
    pub generator: Option<GenId>,
    pub transitions: Vec<Transition>,
}

#[derive(Debug, Clone)]
pub enum Generator {
    Clip { name: String, animation: String, mode: ClipMode, speed: f32, triggers: Vec<Trigger> },
    StateMachine { name: String, start: i32, states: Vec<State>, wildcards: Vec<Transition> },
    /// Blends or switches between children; the first is the default / dominant one.
    Group { name: String, children: Vec<GenId> },
    /// Another behaviour file (e.g. `Behaviors\MT_Behavior.hkx`).
    Reference { name: String, behavior: String },
    Other(String),
}

impl Generator {
    pub fn name(&self) -> &str {
        match self {
            Generator::Clip { name, .. }
            | Generator::StateMachine { name, .. }
            | Generator::Group { name, .. }
            | Generator::Reference { name, .. } => name,
            Generator::Other(class) => class,
        }
    }
}

pub struct BehaviorGraph {
    pub name: String,
    pub root: Option<GenId>,
    pub generators: Vec<Generator>,
    pub events: Vec<String>,
}

// Field offsets for hk_2010.2.0-r1 with 64-bit pointers (hkbNode name at 0x38).
const NODE_NAME: u32 = 0x38;
const ARRAY_PTR_SIZE: u32 = 8;

struct Reader<'a> {
    p: &'a Packfile,
    ids: std::collections::HashMap<u32, GenId>,
    generators: Vec<Generator>,
}

impl Reader<'_> {
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
                Transition {
                    event: self.p.i32(t + 0x30),
                    to_state: self.p.i32(t + 0x34),
                    to_nested: (flags & FLAG_TO_NESTED_STATE_ID_IS_VALID != 0).then(|| self.p.i32(t + 0x3C)),
                    flags,
                }
            })
            .collect()
    }

    fn triggers(&self, array_obj: Option<u32>) -> Vec<Trigger> {
        let Some(a) = array_obj else { return Vec::new() };
        let (Some(data), n) = self.p.array(a + 0x10) else { return Vec::new() };
        (0..n as u32)
            .map(|i| {
                let t = data + i * 0x20;
                Trigger { time: self.p.f32(t), event: self.p.i32(t + 8), from_end: self.p.u8(t + 0x18) != 0 }
            })
            .collect()
    }

    /// Read the generator at `o` (memoised; cycles resolve to the same id).
    fn generator(&mut self, o: u32) -> GenId {
        if let Some(&id) = self.ids.get(&o) {
            return id;
        }
        let id = self.generators.len();
        self.ids.insert(o, id);
        self.generators.push(Generator::Other(String::new()));
        let p = self.p;
        let class = p.object_class(o).unwrap_or("").to_owned();
        let name = p.string(o + NODE_NAME).unwrap_or_default();
        let g = match class.as_str() {
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
                    });
                }
                Generator::StateMachine { name, start: p.i32(o + 0x68), states, wildcards: self.transitions(p.ptr(o + 0xA0)) }
            }
            "hkbBlenderGenerator" | "hkbPoseMatchingGenerator" => {
                let children = self
                    .ptr_array(o + 0x60)
                    .into_iter()
                    .filter_map(|c| p.ptr(c + 0x30))
                    .collect::<Vec<_>>()
                    .into_iter()
                    .map(|g| self.generator(g))
                    .collect();
                Generator::Group { name, children }
            }
            "hkbManualSelectorGenerator" => {
                let gens = self.ptr_array(o + 0x48);
                Generator::Group { name, children: gens.into_iter().map(|g| self.generator(g)).collect() }
            }
            "BSBoneSwitchGenerator" => {
                let mut kids: Vec<u32> = p.ptr(o + 0x50).into_iter().collect();
                kids.extend(self.ptr_array(o + 0x58).into_iter().filter_map(|d| p.ptr(d + 0x30)));
                Generator::Group { name, children: kids.into_iter().map(|g| self.generator(g)).collect() }
            }
            "hkbModifierGenerator"
            | "BSSynchronizedClipGenerator"
            | "BSCyclicBlendTransitionGenerator"
            | "BSiStateTaggingGenerator"
            | "BSOffsetAnimationGenerator" => {
                let children = p.ptr(o + 0x50).map(|g| self.generator(g)).into_iter().collect();
                Generator::Group { name, children }
            }
            "hkbBehaviorReferenceGenerator" => Generator::Reference { name, behavior: p.string(o + 0x48).unwrap_or_default() },
            _ => Generator::Other(class),
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
        let events = p
            .ptr(graph + 0x88)
            .and_then(|data| p.ptr(data + 0x78))
            .map(|strings| {
                let (ptr, n) = p.array(strings + 0x10);
                ptr.map(|d| (0..n as u32).map(|i| p.string(d + i * 8).unwrap_or_default()).collect())
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        let mut r = Reader { p: &p, ids: Default::default(), generators: Vec::new() };
        let root = p.ptr(graph + 0x80).map(|g| r.generator(g));
        Ok(BehaviorGraph { name, root, generators: r.generators, events })
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
            for t in targets.filter(|t| t.event == event && t.flags & FLAG_DISABLED == 0) {
                if let Some(s) = states.iter().find(|s| s.id == t.to_state)
                    && !out.iter().any(|(g, x, y)| *g == gi && x.id == s.id && y.to_nested == t.to_nested)
                {
                    out.push((gi, s, *t));
                }
            }
        }
        out
    }
}

/// A clip reached by following an event.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayedClip {
    /// Path relative to the character project, e.g. `Animations\IdleKneeling.hkx`.
    pub animation: String,
    pub mode: ClipMode,
}

/// Clips played for an event and where in the graphs playback ended up.
#[derive(Debug, Clone)]
pub struct Playback {
    pub clips: Vec<PlayedClip>,
    cursor: Cursor,
}

/// Behaviour graphs of one character project, by lowercase relative path
/// (`behaviors\0_master.hkx`), with event lookups across them.
#[derive(Default)]
pub struct Project {
    pub graphs: Vec<(String, BehaviorGraph)>,
}

/// Position while walking a graph: (graph index, state machine stack of (sm, state id)).
type Cursor = (usize, Vec<(GenId, i32)>);

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

    fn graph_index(&self, path: &str) -> Option<usize> {
        let path = path.to_ascii_lowercase().replace('/', "\\");
        self.graphs.iter().position(|(p, _)| *p == path)
    }

    /// Descend from `g` to the first clip, pushing entered state machines onto the cursor.
    /// `nested` picks the state to enter in the first state machine reached (a
    /// transition's nested target) instead of its start state.
    fn descend(&self, gi: usize, g: GenId, stack: &mut Vec<(GenId, i32)>, nested: &mut Option<i32>, depth: usize) -> Option<(usize, GenId)> {
        if depth > 64 {
            return None;
        }
        let graph = &self.graphs[gi].1;
        match &graph.generators[g] {
            Generator::Clip { .. } => Some((gi, g)),
            Generator::StateMachine { start, states, .. } => {
                let want = nested.take().unwrap_or(*start);
                let s = states.iter().find(|s| s.id == want).or(states.first())?;
                stack.push((g, s.id));
                self.descend(gi, s.generator?, stack, nested, depth + 1)
            }
            Generator::Group { children, .. } => children.iter().find_map(|&c| {
                let mark = stack.len();
                let r = self.descend(gi, c, stack, nested, depth + 1);
                if r.is_none() {
                    stack.truncate(mark);
                }
                r
            }),
            Generator::Reference { behavior, .. } => {
                let other = self.graph_index(behavior)?;
                // Entering another graph: its state machines form a fresh stack.
                stack.clear();
                let root = self.graphs[other].1.root?;
                self.descend(other, root, stack, nested, depth + 1)
            }
            Generator::Other(_) => None,
        }
    }

    /// A clip generator named `name` (case-insensitive), e.g. loose idles whose
    /// event names their clip (`idle_A_leg_shift` -> `MT_idle_A_leg_shift`).
    pub fn clip_named(&self, name: &str) -> Option<PlayedClip> {
        self.graphs.iter().flat_map(|(_, g)| &g.generators).find_map(|g| match g {
            Generator::Clip { name: n, animation, mode, .. } if n.eq_ignore_ascii_case(name) => {
                Some(PlayedClip { animation: animation.clone(), mode: *mode })
            }
            _ => None,
        })
    }

    /// Clips played after `event`, in order: the clip the event leads to, then the
    /// clips reached through its triggers (e.g. an enter clip firing `00NextClip`
    /// into a loop), stopping at the first looping clip. One sequence per place in
    /// the project that handles the event.
    pub fn clips_for_event(&self, event: &str) -> Vec<Vec<PlayedClip>> {
        self.play_event(event).into_iter().map(|p| p.clips).collect()
    }

    /// Like [`Project::clips_for_event`], keeping where each sequence ends up so
    /// that a later event (e.g. an exit) can be followed from there.
    pub fn play_event(&self, event: &str) -> Vec<Playback> {
        let mut out: Vec<Playback> = Vec::new();
        for (gi, (_, graph)) in self.graphs.iter().enumerate() {
            let Some(eid) = graph.event_id(event) else { continue };
            for (sm, state, t) in graph.transitions_on(eid) {
                let Some(start) = state.generator else { continue };
                let pb = self.follow((gi, vec![(sm, state.id)]), start, t.to_nested);
                if !pb.clips.is_empty() && !out.iter().any(|o| o.clips == pb.clips) {
                    out.push(pb);
                }
            }
        }
        out
    }

    /// Clips played when `event` arrives while at `from`: the first transition on it
    /// out of the current state (or its state machine's wildcards), innermost first.
    pub fn then_event(&self, from: &Playback, event: &str) -> Option<Playback> {
        let (gi, stack) = &from.cursor;
        let graph = &self.graphs[*gi].1;
        let eid = graph.event_id(event)?;
        for depth in (0..stack.len()).rev() {
            let (sm, cur) = stack[depth];
            let Generator::StateMachine { states, wildcards, .. } = &graph.generators[sm] else { continue };
            let here = states.iter().find(|s| s.id == cur);
            let tr = here
                .into_iter()
                .flat_map(|s| &s.transitions)
                .chain(wildcards)
                .find(|x| x.event == eid && x.to_state != cur && x.flags & FLAG_DISABLED == 0);
            if let Some(tr) = tr
                && let Some(s) = states.iter().find(|s| s.id == tr.to_state)
            {
                let mut st = stack[..depth].to_vec();
                st.push((sm, s.id));
                let pb = self.follow((*gi, st), s.generator?, tr.to_nested);
                return (!pb.clips.is_empty()).then_some(pb);
            }
        }
        None
    }

    fn follow(&self, cursor: Cursor, start: GenId, mut nested: Option<i32>) -> Playback {
        let (mut gi, mut stack) = cursor;
        let mut seq: Vec<PlayedClip> = Vec::new();
        let mut next = Some(start);
        while let Some(g) = next.take() {
            if seq.len() >= 6 {
                break;
            }
            let Some((cgi, clip)) = self.descend(gi, g, &mut stack, &mut nested, 0) else { break };
            gi = cgi;
            let graph = &self.graphs[gi].1;
            let Generator::Clip { animation, mode, triggers, .. } = &graph.generators[clip] else { break };
            let played = PlayedClip { animation: animation.clone(), mode: *mode };
            if seq.contains(&played) {
                break;
            }
            seq.push(played);
            if *mode != ClipMode::SinglePlay {
                break;
            }
            // Find a trigger event that moves an enclosing state machine on.
            'triggers: for t in triggers {
                for depth in (0..stack.len()).rev() {
                    let (sm, cur) = stack[depth];
                    let Generator::StateMachine { states, wildcards, .. } = &graph.generators[sm] else { continue };
                    let here = states.iter().find(|s| s.id == cur);
                    let tr = here
                        .into_iter()
                        .flat_map(|s| &s.transitions)
                        .chain(wildcards)
                        .find(|x| x.event == t.event && x.to_state != cur && x.flags & FLAG_DISABLED == 0);
                    if let Some(tr) = tr
                        && let Some(s) = states.iter().find(|s| s.id == tr.to_state)
                    {
                        stack.truncate(depth);
                        stack.push((sm, s.id));
                        next = s.generator;
                        nested = tr.to_nested;
                        break 'triggers;
                    }
                }
            }
        }
        Playback { clips: seq, cursor: (gi, stack) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(name: &str, mode: ClipMode, triggers: Vec<Trigger>) -> Generator {
        Generator::Clip { name: name.into(), animation: format!("Animations\\{name}.hkx"), mode, speed: 1.0, triggers }
    }

    fn tr(event: i32, to_state: i32, to_nested: Option<i32>) -> Transition {
        Transition { event, to_state, to_nested, flags: 0 }
    }

    fn state(id: i32, generator: GenId, transitions: Vec<Transition>) -> State {
        State { id, name: format!("s{id}"), generator: Some(generator), transitions }
    }

    /// Root SM: default idle (state 0) and a furniture SM (state 1) entered with a
    /// nested target. The furniture SM's states 7/8 are an enter clip whose
    /// `00NextClip` trigger moves on to a loop, and an exit reached by IdleChairExitStart.
    fn project() -> Project {
        let events = ["IdleKneeling", "00NextClip", "IdleChairExitStart", "IdleOther"].map(String::from).to_vec();
        let next = Trigger { time: -0.2, from_end: true, event: 1 };
        let generators = vec![
            /* 0 */
            Generator::StateMachine {
                name: "Root".into(),
                start: 0,
                states: vec![state(0, 1, vec![]), state(1, 2, vec![])],
                wildcards: vec![tr(0, 1, Some(7)), tr(3, 1, None)],
            },
            /* 1 */ clip("MT_Idle", ClipMode::Looping, vec![]),
            /* 2 */
            Generator::StateMachine {
                name: "Furniture".into(),
                start: 5,
                states: vec![
                    state(5, 3, vec![]),
                    state(7, 4, vec![tr(1, 8, None)]),
                    state(8, 5, vec![tr(2, 9, None)]),
                    state(9, 6, vec![]),
                ],
                wildcards: vec![],
            },
            /* 3 */ Generator::Reference { name: "Other".into(), behavior: "Behaviors\\Other.hkx".into() },
            /* 4 */ clip("KneelEnter", ClipMode::SinglePlay, vec![next]),
            /* 5 */ clip("KneelIdle", ClipMode::Looping, vec![]),
            /* 6 */ clip("KneelExit", ClipMode::SinglePlay, vec![]),
        ];
        let master = BehaviorGraph { name: "Master".into(), root: Some(0), generators, events };
        let other = BehaviorGraph {
            name: "Other".into(),
            root: Some(0),
            generators: vec![Generator::Group { name: "Blend".into(), children: vec![1] }, clip("OtherLoop", ClipMode::Looping, vec![])],
            events: vec![],
        };
        Project { graphs: vec![("behaviors\\master.hkx".into(), master), ("behaviors\\other.hkx".into(), other)] }
    }

    fn names(clips: &[PlayedClip]) -> Vec<&str> {
        clips.iter().map(|c| c.animation.as_str()).collect()
    }

    #[test]
    fn nested_target_and_triggered_loop() {
        let p = project();
        let plays = p.play_event("idlekneeling");
        assert_eq!(plays.len(), 1);
        assert_eq!(names(&plays[0].clips), ["Animations\\KneelEnter.hkx", "Animations\\KneelIdle.hkx"]);
        let exit = p.then_event(&plays[0], "IdleChairExitStart").unwrap();
        assert_eq!(names(&exit.clips), ["Animations\\KneelExit.hkx"]);
    }

    #[test]
    fn start_state_follows_references() {
        // Without a nested target the furniture SM starts in state 5: a reference.
        let plays = project().play_event("IdleOther");
        assert_eq!(names(&plays[0].clips), ["Animations\\OtherLoop.hkx"]);
    }

    #[test]
    fn clip_lookup_by_name() {
        assert_eq!(project().clip_named("kneelidle").unwrap().animation, "Animations\\KneelIdle.hkx");
        assert!(project().play_event("Unknown").is_empty());
    }
}
