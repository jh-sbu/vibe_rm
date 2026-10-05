//! Static event -> clip resolution over a project's graphs: "which clips does this
//! animation event play, and which clips leave that state again?" Used to plan
//! ahead (where an enter animation must start so that it ends on a furniture
//! marker); playback itself runs graphs with [`super::runtime`].

use super::*;

/// A clip reached by following an event.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayedClip {
    /// Path relative to the character project, e.g. `Animations\IdleKneeling.hkx`.
    pub animation: String,
    pub mode: ClipMode,
    /// Playback speed; negative plays the clip backwards (e.g. a sitting variant's
    /// return reusing its enter clip).
    pub speed: f32,
}

/// An event the graph raises while playing a sequence: on entering a state, or
/// from a clip trigger.
#[derive(Debug, Clone, PartialEq)]
pub struct RaisedEvent {
    /// Index into [`Playback::clips`] of the clip playing when it is raised.
    pub clip: usize,
    /// Seconds into that clip (from its end if `from_end`).
    pub time: f32,
    pub from_end: bool,
    pub event: String,
    pub payload: Option<String>,
}

/// Clips played for an event and where in the graphs playback ended up.
#[derive(Debug, Clone)]
pub struct Playback {
    pub clips: Vec<PlayedClip>,
    /// Events raised along the way, in order.
    pub events: Vec<RaisedEvent>,
    cursor: Cursor,
}

/// Position while walking a graph: (graph index, state machine stack of (sm, state id)).
type Cursor = (usize, Vec<(GenId, i32)>);

/// A state entered while walking: (graph index, state machine, state id).
type Entered = (usize, GenId, i32);

impl Project {
    /// Descend from `g` to the first clip, pushing entered state machines onto the cursor.
    /// `nested` picks the state to enter in the first state machine reached (a
    /// transition's nested target) instead of its start state.
    /// States entered on the way are appended to `entered`.
    fn descend(
        &self,
        gi: usize,
        g: GenId,
        stack: &mut Vec<(GenId, i32)>,
        nested: &mut Option<i32>,
        entered: &mut Vec<Entered>,
        depth: usize,
    ) -> Option<(usize, GenId)> {
        if depth > 64 {
            return None;
        }
        let graph = &self.graphs[gi].1;
        match &graph.generators[g] {
            Generator::Clip { .. } => Some((gi, g)),
            Generator::StateMachine { start, start_variable, states, .. } => {
                // A bound start state takes the variable's initial value, which
                // describes a standing third-person actor (`i1stPerson` = 0, ...).
                let bound = start_variable.and_then(|v| graph.variable_defaults.get(v).copied());
                let want = nested.take().or(bound).unwrap_or(*start);
                let s = states.iter().find(|s| s.id == want).or(states.first())?;
                stack.push((g, s.id));
                entered.push((gi, g, s.id));
                self.descend(gi, s.generator?, stack, nested, entered, depth + 1)
            }
            Generator::Blender { .. } | Generator::Selector { .. } | Generator::Wrap { .. } | Generator::BoneSwitch { .. } => {
                graph.generators[g].children().into_iter().find_map(|c| {
                let (mark, emark) = (stack.len(), entered.len());
                let r = self.descend(gi, c, stack, nested, entered, depth + 1);
                if r.is_none() {
                    stack.truncate(mark);
                    entered.truncate(emark);
                }
                r
                })
            }
            Generator::Reference { behavior, .. } => {
                let other = self.graph_index(behavior)?;
                // Entering another graph: its state machines form a fresh stack.
                stack.clear();
                let root = self.graphs[other].1.root?;
                self.descend(other, root, stack, nested, entered, depth + 1)
            }
            Generator::Other(_) => None,
        }
    }

    /// A clip generator named `name` (case-insensitive), e.g. loose idles whose
    /// event names their clip (`idle_A_leg_shift` -> `MT_idle_A_leg_shift`).
    pub fn clip_named(&self, name: &str) -> Option<PlayedClip> {
        self.graphs.iter().flat_map(|(_, g)| &g.generators).find_map(|g| match g {
            Generator::Clip { name: n, animation, mode, speed, .. } if n.eq_ignore_ascii_case(name) => {
                Some(PlayedClip { animation: animation.clone(), mode: *mode, speed: *speed })
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

    /// Events raised on entering each of `entered`, while clip `clip` starts.
    fn enter_events(&self, entered: &[Entered], clip: usize, out: &mut Vec<RaisedEvent>) {
        for &(gi, sm, id) in entered {
            let graph = &self.graphs[gi].1;
            let Generator::StateMachine { states, .. } = &graph.generators[sm] else { continue };
            let Some(state) = states.iter().find(|s| s.id == id) else { continue };
            for e in &state.enter_events {
                let Some(event) = graph.event_name(e.event) else { continue };
                out.push(RaisedEvent { clip, time: 0.0, from_end: false, event: event.to_owned(), payload: e.payload.clone() });
            }
        }
    }

    /// `cursor`'s innermost state has just been entered.
    fn follow(&self, cursor: Cursor, start: GenId, mut nested: Option<i32>) -> Playback {
        let (mut gi, mut stack) = cursor;
        let mut seq: Vec<PlayedClip> = Vec::new();
        let mut events: Vec<RaisedEvent> = Vec::new();
        let mut entered: Vec<Entered> = stack.last().map(|&(sm, id)| (gi, sm, id)).into_iter().collect();
        let mut next = Some(start);
        while let Some(g) = next.take() {
            if seq.len() >= 6 {
                break;
            }
            let Some((cgi, clip)) = self.descend(gi, g, &mut stack, &mut nested, &mut entered, 0) else { break };
            gi = cgi;
            let graph = &self.graphs[gi].1;
            let Generator::Clip { animation, mode, speed, triggers, .. } = &graph.generators[clip] else { break };
            let played = PlayedClip { animation: animation.clone(), mode: *mode, speed: *speed };
            if seq.contains(&played) {
                break;
            }
            self.enter_events(&entered, seq.len(), &mut events);
            entered.clear();
            for t in triggers {
                if let Some(event) = graph.event_name(t.event) {
                    let (time, from_end) = (t.time, t.from_end);
                    events.push(RaisedEvent { clip: seq.len(), time, from_end, event: event.to_owned(), payload: t.payload.clone() });
                }
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
                        entered.push((gi, sm, s.id));
                        next = s.generator;
                        nested = tr.to_nested;
                        break 'triggers;
                    }
                }
            }
        }
        Playback { clips: seq, events, cursor: (gi, stack) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(name: &str, mode: ClipMode, triggers: Vec<Trigger>) -> Generator {
        Generator::Clip {
            name: name.into(),
            animation: format!("Animations\\{name}.hkx"),
            mode,
            speed: 1.0,
            triggers,
            crop_start: 0.0,
            crop_end: 0.0,
            start_time: 0.0,
        }
    }

    fn tr(event: i32, to_state: i32, to_nested: Option<i32>) -> Transition {
        Transition::new(event, to_state, to_nested)
    }

    fn state(id: i32, generator: GenId, transitions: Vec<Transition>) -> State {
        State { id, name: format!("s{id}"), generator: Some(generator), transitions, enter_events: Vec::new(), exit_events: Vec::new() }
    }

    /// Root SM: default idle (state 0) and a furniture SM (state 1) entered with a
    /// nested target. The furniture SM's states 7/8 are an enter clip whose
    /// `00NextClip` trigger moves on to a loop, and an exit reached by IdleChairExitStart.
    fn project() -> Project {
        let events = ["IdleKneeling", "00NextClip", "IdleChairExitStart", "IdleOther"].map(String::from).to_vec();
        let next = Trigger { time: -0.2, from_end: true, event: 1, payload: None };
        let generators = vec![
            /* 0 */
            Generator::StateMachine {
                name: "Root".into(),
                start: 0,
                start_variable: None,
                states: vec![state(0, 1, vec![]), state(1, 2, vec![])],
                wildcards: vec![tr(0, 1, Some(7)), tr(3, 1, None)],
            },
            /* 1 */ clip("MT_Idle", ClipMode::Looping, vec![]),
            /* 2 */
            Generator::StateMachine {
                name: "Furniture".into(),
                start: 5,
                start_variable: None,
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
        let master = BehaviorGraph::new("Master", Some(0), generators, events);
        let blend = Generator::Blender {
            name: "Blend".into(),
            parameter: 0.0,
            min_cyclic: 0.0,
            max_cyclic: 1.0,
            flags: 0,
            children: vec![BlendChild { generator: Some(1), weight: 1.0, weight_variable: None, bone_weights: None }],
        };
        let other = BehaviorGraph::new("Other", Some(0), vec![blend, clip("OtherLoop", ClipMode::Looping, vec![])], vec![]);
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

    #[test]
    fn state_enter_events_and_trigger_payloads() {
        let mut p = project();
        let master = &mut p.graphs[0].1;
        master.events.extend(["AnimObjLoad", "AnimObjDraw"].map(String::from));
        let (load, draw) = (4, 5);
        let Generator::StateMachine { states, .. } = &mut master.generators[2] else { unreachable!() };
        // Entering state 7 loads the object; the loop clip draws it 1.5 s in.
        states[1].enter_events.push(EventProperty { event: load, payload: Some("AnimObjectBroom".into()) });
        let Generator::Clip { triggers, .. } = &mut master.generators[5] else { unreachable!() };
        triggers.push(Trigger { time: 1.5, from_end: false, event: draw, payload: Some("AnimObjectBroom".into()) });

        let plays = p.play_event("IdleKneeling");
        let events: Vec<(usize, &str, Option<&str>)> =
            plays[0].events.iter().map(|e| (e.clip, e.event.as_str(), e.payload.as_deref())).collect();
        assert_eq!(
            events,
            [
                (0, "AnimObjLoad", Some("AnimObjectBroom")),
                (0, "00NextClip", None),
                (1, "AnimObjDraw", Some("AnimObjectBroom")),
            ]
        );
        assert_eq!(plays[0].events[2].time, 1.5);
    }

    #[test]
    fn start_state_bound_to_variable() {
        // The furniture SM starts in state 5 (a reference), but bind its start to a
        // variable whose initial value is 8 (the kneel loop).
        let mut p = project();
        let master = &mut p.graphs[0].1;
        master.add_variable("i1stPerson", VarType::Int, 8.0);
        let Generator::StateMachine { start_variable, .. } = &mut master.generators[2] else { unreachable!() };
        *start_variable = Some(0);
        let plays = p.play_event("IdleOther");
        assert_eq!(names(&plays[0].clips), ["Animations\\KneelIdle.hkx"]);
    }
}
