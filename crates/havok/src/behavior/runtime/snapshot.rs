//! Saving a running [`Instance`] and putting it back: the active tree (state
//! machines' states, clocks, cross-fades and pending transitions; clips'
//! times; blends, selectors and bone switches; modifiers' timers and
//! counters), the variables, queued and raised events and the random state.
//!
//! The layout is this crate's own. Restoring checks every node and modifier
//! against the instance's project (graph and generator indices, the
//! generator's kind, state ids, child counts) and refuses a snapshot that
//! doesn't fit, leaving the instance unchanged. Ragdoll poses and traces are
//! not kept.

use super::*;

const MAGIC: &[u8; 4] = b"HKBS";
const VERSION: u8 = 1;
/// Deeper than any tree `activate` builds (it stops at 64).
const MAX_DEPTH: usize = 80;

/// Why a snapshot couldn't be restored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotError(pub String);

impl std::fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "behaviour graph snapshot: {}", self.0)
    }
}

impl std::error::Error for SnapshotError {}

type Result<T> = std::result::Result<T, SnapshotError>;

fn bad<T>(what: impl Into<String>) -> Result<T> {
    Err(SnapshotError(what.into()))
}

#[derive(Default)]
struct W(Vec<u8>);

impl W {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn bool(&mut self, v: bool) {
        self.u8(v as u8);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn i32(&mut self, v: i32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn len(&mut self, n: usize) {
        self.u32(n as u32);
    }
    fn str(&mut self, s: &str) {
        self.len(s.len());
        self.0.extend_from_slice(s.as_bytes());
    }
    fn opt_str(&mut self, s: Option<&str>) {
        self.bool(s.is_some());
        if let Some(s) = s {
            self.str(s);
        }
    }
    fn opt_index(&mut self, v: Option<usize>) {
        self.u32(v.map_or(u32::MAX, |v| v as u32));
    }
}

struct R<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> R<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.at.checked_add(n).filter(|&e| e <= self.data.len());
        let Some(end) = end else {
            return bad("truncated");
        };
        let s = &self.data[self.at..end];
        self.at = end;
        Ok(s)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        Ok(self.take(N)?.try_into().unwrap())
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn bool(&mut self) -> Result<bool> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            n => bad(format!("boolean {n}")),
        }
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.array()?))
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    /// A finite float: a clock or weight that isn't a number can't be resumed.
    fn f32(&mut self) -> Result<f32> {
        let v = f32::from_le_bytes(self.array()?);
        if v.is_nan() {
            return bad("NaN");
        }
        Ok(v)
    }
    /// A count of items at least `min` bytes each, checked against what's left.
    fn count(&mut self, min: usize) -> Result<usize> {
        let n = self.u32()? as usize;
        if n.saturating_mul(min.max(1)) > self.data.len() - self.at {
            return bad(format!("count {n} exceeds the data"));
        }
        Ok(n)
    }
    fn str(&mut self) -> Result<String> {
        let n = self.count(1)?;
        String::from_utf8(self.take(n)?.to_vec()).map_err(|_| SnapshotError("not UTF-8".into()))
    }
    fn opt_str(&mut self) -> Result<Option<String>> {
        Ok(if self.bool()? {
            Some(self.str()?)
        } else {
            None
        })
    }
    fn opt_index(&mut self) -> Result<Option<usize>> {
        Ok(match self.u32()? {
            u32::MAX => None,
            v => Some(v as usize),
        })
    }
}

fn curve_code(c: BlendCurve) -> u8 {
    match c {
        BlendCurve::Smooth => 0,
        BlendCurve::Linear => 1,
        BlendCurve::LinearToSmooth => 2,
        BlendCurve::SmoothToLinear => 3,
    }
}

fn curve_from(code: u8) -> Result<BlendCurve> {
    Ok(match code {
        0 => BlendCurve::Smooth,
        1 => BlendCurve::Linear,
        2 => BlendCurve::LinearToSmooth,
        3 => BlendCurve::SmoothToLinear,
        n => return bad(format!("blend curve {n}")),
    })
}

/// What picks a pending transition out among its machine's candidates.
fn put_transition_key(w: &mut W, t: &Transition) {
    w.i32(t.event);
    w.i32(t.to_state);
    w.bool(t.to_nested.is_some());
    w.i32(t.to_nested.unwrap_or(0));
    w.u16(t.flags);
    w.u16(t.priority as u16);
}

fn same_transition(t: &Transition, key: &(i32, i32, Option<i32>, u16, i16)) -> bool {
    (t.event, t.to_state, t.to_nested, t.flags, t.priority) == *key
}

fn put_node(w: &mut W, n: &Node) {
    w.len(n.gi);
    w.len(n.g);
    match &n.kind {
        Kind::Empty => w.u8(0),
        Kind::Clip(c) => {
            w.u8(1);
            w.str(&c.animation);
            for v in [c.t, c.prev, c.length, c.offset] {
                w.f32(v);
            }
            for b in [
                c.reverse,
                c.additive,
                c.looping,
                c.user,
                c.ping_pong,
                c.wrapped,
                c.started,
            ] {
                w.bool(b);
            }
        }
        Kind::Machine(m) => {
            w.u8(2);
            w.i32(m.state);
            w.opt_index(m.sync_variable);
            put_opt_node(w, m.child.as_deref());
            w.bool(m.from.is_some());
            if let Some((from, t, length)) = &m.from {
                put_node(w, from);
                w.f32(*t);
                w.f32(*length);
            }
            w.u8(curve_code(m.from_curve));
            w.f32(m.elapsed);
            w.len(m.seen.len());
            for &(event, seq) in &m.seen {
                w.i32(event);
                w.u32(seq);
            }
            w.u32(m.seq);
            w.bool(m.pending.is_some());
            if let Some(t) = &m.pending {
                put_transition_key(w, t);
            }
            w.bool(m.locked);
            w.bool(m.from_hears);
        }
        Kind::Blend { children, phase } => {
            w.u8(3);
            w.f32(*phase);
            w.len(children.len());
            for c in children {
                put_opt_node(w, c.as_ref());
            }
        }
        Kind::Select { index, child } => {
            w.u8(4);
            w.len(*index);
            put_opt_node(w, child.as_deref());
        }
        Kind::Wrap { child, modifier } => {
            w.u8(5);
            put_opt_node(w, child.as_deref());
            w.bool(modifier.is_some());
            if let Some(m) = modifier {
                put_mod(w, m);
            }
        }
        Kind::Switch { default, children } => {
            w.u8(6);
            put_opt_node(w, default.as_deref());
            w.len(children.len());
            for c in children {
                put_node(w, c);
            }
        }
    }
}

fn put_opt_node(w: &mut W, n: Option<&Node>) {
    w.bool(n.is_some());
    if let Some(n) = n {
        put_node(w, n);
    }
}

fn put_mod(w: &mut W, m: &ModState) {
    w.len(m.gi);
    w.len(m.m);
    match &m.kind {
        ModKind::Plain => w.u8(0),
        ModKind::List(list) => {
            w.u8(1);
            w.len(list.len());
            for c in list {
                put_mod(w, c);
            }
        }
        ModKind::Timer { elapsed, fired } => {
            w.u8(2);
            w.f32(*elapsed);
            w.bool(*fired);
        }
        ModKind::EveryN { count, target } => {
            w.u8(3);
            w.u32(*count);
            w.u32(*target);
        }
        ModKind::Driven { active, child } => {
            w.u8(4);
            w.bool(*active);
            w.bool(child.is_some());
            if let Some(c) = child {
                put_mod(w, c);
            }
        }
        ModKind::Expressions { was_true } => {
            w.u8(5);
            w.len(was_true.len());
            for &b in was_true {
                w.bool(b);
            }
        }
        ModKind::Damping {
            damped,
            error_sum,
            previous,
        } => {
            w.u8(6);
            w.f32(*damped);
            w.f32(*error_sum);
            w.f32(*previous);
        }
    }
}

/// Reads nodes and modifiers, checking them against the project.
struct Check<'a> {
    shared: &'a Shared,
}

impl Check<'_> {
    fn graph(&self, gi: usize) -> Result<&BehaviorGraph> {
        match self.shared.project.graphs.get(gi) {
            Some((_, g)) => Ok(g),
            None => bad(format!("graph {gi}")),
        }
    }

    fn generator(&self, gi: usize, g: GenId) -> Result<&Generator> {
        match self.graph(gi)?.generators.get(g) {
            Some(x) => Ok(x),
            None => bad(format!("generator {g} of graph {gi}")),
        }
    }

    /// A node, which must be generator `want` of graph `want_gi` when given.
    fn node(&self, r: &mut R, want: Option<(usize, GenId)>, depth: usize) -> Result<Node> {
        if depth > MAX_DEPTH {
            return bad("tree too deep");
        }
        let gi = r.u32()? as usize;
        let g = r.u32()? as usize;
        if want.is_some_and(|w| w != (gi, g)) {
            return bad(format!("node ({gi}, {g}) where {want:?} belongs"));
        }
        let generator = self.generator(gi, g)?;
        let kind = match r.u8()? {
            0 => Kind::Empty,
            1 => {
                if !matches!(
                    generator,
                    Generator::Clip { .. } | Generator::Sequence { .. }
                ) {
                    return bad(format!("clip state on {}", generator.name()));
                }
                let animation: Arc<str> = r.str()?.into();
                let (t, prev, length, offset) = (r.f32()?, r.f32()?, r.f32()?, r.f32()?);
                Kind::Clip(ClipState {
                    animation,
                    t,
                    prev,
                    length,
                    offset,
                    reverse: r.bool()?,
                    additive: r.bool()?,
                    looping: r.bool()?,
                    user: r.bool()?,
                    ping_pong: r.bool()?,
                    wrapped: r.bool()?,
                    started: r.bool()?,
                })
            }
            2 => {
                let Generator::StateMachine {
                    states, wildcards, ..
                } = generator
                else {
                    return bad(format!("machine state on {}", generator.name()));
                };
                let state = r.i32()?;
                let Some(here) = states.iter().find(|s| s.id == state) else {
                    return bad(format!("state {state} of {}", generator.name()));
                };
                let sync_variable = r.opt_index()?;
                let variables = self.graph(gi)?.variables.len();
                if sync_variable.is_some_and(|v| v >= variables) {
                    return bad("sync variable");
                }
                let child = self.opt_node(r, here.generator.map(|c| (gi, c)), depth)?;
                let from = if r.bool()? {
                    let node = self.node(r, None, depth + 1)?;
                    if node.gi != gi || !states.iter().any(|s| s.generator == Some(node.g)) {
                        return bad("cross-fade from a node that isn't a state's");
                    }
                    Some((Box::new(node), r.f32()?, r.f32()?))
                } else {
                    None
                };
                let from_curve = curve_from(r.u8()?)?;
                let elapsed = r.f32()?;
                let seen = (0..r.count(8)?)
                    .map(|_| Ok((r.i32()?, r.u32()?)))
                    .collect::<Result<Vec<_>>>()?;
                let seq = r.u32()?;
                let pending = if r.bool()? {
                    let (event, to_state) = (r.i32()?, r.i32()?);
                    let nested = r.bool()?;
                    let target = r.i32()?;
                    let nested = nested.then_some(target);
                    let key = (event, to_state, nested, r.u16()?, r.u16()? as i16);
                    let promoted = self
                        .shared
                        .promoted
                        .get(&(gi, g))
                        .map(Vec::as_slice)
                        .unwrap_or(&[]);
                    let found = here
                        .transitions
                        .iter()
                        .chain(wildcards)
                        .chain(promoted.iter().map(|(_, t)| t))
                        .find(|t| same_transition(t, &key));
                    match found {
                        Some(t) => Some(t.clone()),
                        None => return bad("pending transition not found"),
                    }
                } else {
                    None
                };
                Kind::Machine(MachineState {
                    state,
                    sync_variable,
                    child,
                    from,
                    from_curve,
                    elapsed,
                    seen,
                    seq,
                    pending,
                    locked: r.bool()?,
                    from_hears: r.bool()?,
                })
            }
            3 => {
                let Generator::Blender { children: gens, .. } = generator else {
                    return bad(format!("blend state on {}", generator.name()));
                };
                let phase = r.f32()?;
                if r.count(1)? != gens.len() {
                    return bad("blend children");
                }
                let children = gens
                    .iter()
                    .map(|c| {
                        Ok(self
                            .opt_node(r, c.generator.map(|c| (gi, c)), depth)?
                            .map(|n| *n))
                    })
                    .collect::<Result<Vec<_>>>()?;
                if children
                    .iter()
                    .zip(gens)
                    .any(|(n, c)| n.is_none() && c.generator.is_some())
                {
                    return bad("blend child missing");
                }
                Kind::Blend { children, phase }
            }
            4 => {
                let gens = match generator {
                    Generator::Selector { children, .. }
                    | Generator::PoseMatching { children, .. } => children,
                    _ => return bad(format!("selector state on {}", generator.name())),
                };
                let index = r.u32()? as usize;
                let child = self.opt_node(r, gens.get(index).map(|&c| (gi, c)), depth)?;
                if child.is_some() && index >= gens.len() {
                    return bad("selector index");
                }
                Kind::Select { index, child }
            }
            5 => {
                let (want_child, want_mod) = match generator {
                    Generator::Wrap {
                        child, modifier, ..
                    } => (child.map(|c| (gi, c)), modifier.map(|m| (gi, m))),
                    Generator::Reference { behavior, .. } => {
                        let target = self
                            .shared
                            .project
                            .graph_index(behavior)
                            .and_then(|o| Some((o, self.graph(o).ok()?.root?)));
                        (target, None)
                    }
                    _ => return bad(format!("wrapper state on {}", generator.name())),
                };
                let child = self.opt_node(r, want_child, depth)?;
                let modifier = if r.bool()? {
                    let Some(want) = want_mod else {
                        return bad("modifier on a wrapper without one");
                    };
                    Some(self.modifier(r, Some(want), 0)?)
                } else {
                    None
                };
                Kind::Wrap { child, modifier }
            }
            6 => {
                let Generator::BoneSwitch {
                    default,
                    children: gens,
                    ..
                } = generator
                else {
                    return bad(format!("bone switch state on {}", generator.name()));
                };
                let default = self.opt_node(r, default.map(|c| (gi, c)), depth)?;
                if r.count(1)? != gens.len() {
                    return bad("bone switch children");
                }
                let children = gens
                    .iter()
                    .map(|&(c, _)| self.node(r, Some((gi, c)), depth + 1))
                    .collect::<Result<Vec<_>>>()?;
                Kind::Switch { default, children }
            }
            n => return bad(format!("node kind {n}")),
        };
        Ok(Node { gi, g, kind })
    }

    fn opt_node(
        &self,
        r: &mut R,
        want: Option<(usize, GenId)>,
        depth: usize,
    ) -> Result<Option<Box<Node>>> {
        if !r.bool()? {
            return Ok(None);
        }
        if want.is_none() {
            return bad("a child where the generator has none");
        }
        Ok(Some(Box::new(self.node(r, want, depth + 1)?)))
    }

    fn modifier(&self, r: &mut R, want: Option<(usize, ModId)>, depth: usize) -> Result<ModState> {
        if depth > 32 {
            return bad("modifiers too deep");
        }
        let gi = r.u32()? as usize;
        let m = r.u32()? as usize;
        if want.is_some_and(|w| w != (gi, m)) {
            return bad(format!("modifier ({gi}, {m}) where {want:?} belongs"));
        }
        let Some(modifier) = self.graph(gi)?.modifiers.get(m) else {
            return bad(format!("modifier {m} of graph {gi}"));
        };
        let kind = match r.u8()? {
            0 => ModKind::Plain,
            1 => {
                let Modifier::List(ids) = modifier else {
                    return bad("modifier list");
                };
                if r.count(1)? != ids.len() {
                    return bad("modifier list length");
                }
                ModKind::List(
                    ids.iter()
                        .map(|&c| self.modifier(r, Some((gi, c)), depth + 1))
                        .collect::<Result<_>>()?,
                )
            }
            2 => {
                if !matches!(modifier, Modifier::Timer { .. }) {
                    return bad("timer");
                }
                ModKind::Timer {
                    elapsed: r.f32()?,
                    fired: r.bool()?,
                }
            }
            3 => {
                if !matches!(modifier, Modifier::EveryN { .. }) {
                    return bad("every-n counter");
                }
                ModKind::EveryN {
                    count: r.u32()?,
                    target: r.u32()?,
                }
            }
            4 => {
                let Modifier::EventDriven { child: want, .. } = modifier else {
                    return bad("event-driven modifier");
                };
                let active = r.bool()?;
                let child = if r.bool()? {
                    let Some(c) = want else {
                        return bad("event-driven child");
                    };
                    Some(Box::new(self.modifier(r, Some((gi, *c)), depth + 1)?))
                } else {
                    None
                };
                ModKind::Driven { active, child }
            }
            5 => {
                let Modifier::Expressions(lines) = modifier else {
                    return bad("expressions");
                };
                if r.count(1)? != lines.len() {
                    return bad("expression count");
                }
                ModKind::Expressions {
                    was_true: (0..lines.len()).map(|_| r.bool()).collect::<Result<_>>()?,
                }
            }
            6 => {
                if !matches!(modifier, Modifier::Damping { .. }) {
                    return bad("damping");
                }
                ModKind::Damping {
                    damped: r.f32()?,
                    error_sum: r.f32()?,
                    previous: r.f32()?,
                }
            }
            n => return bad(format!("modifier kind {n}")),
        };
        Ok(ModState { gi, m, kind })
    }
}

impl Instance {
    /// The running graph as bytes, for [`Instance::restore`].
    pub fn snapshot(&self) -> Vec<u8> {
        let mut w = W::default();
        w.0.extend_from_slice(MAGIC);
        w.u8(VERSION);
        w.len(self.values.len());
        for &v in &self.values {
            w.f32(v);
        }
        put_opt_node(&mut w, self.root.as_ref());
        w.len(self.queue.len());
        for e in &self.queue {
            w.len(e.id);
            w.opt_index(e.raised);
        }
        w.len(self.raised.len());
        for r in &self.raised {
            w.str(&r.event);
            w.opt_str(r.payload.as_deref());
            w.bool(r.taken);
        }
        w.u64(self.rng);
        w.u64(self.changes);
        w.bool(self.pose_playing);
        w.bool(self.matching_yaw.is_some());
        w.f32(self.matching_yaw.unwrap_or(0.0));
        let mut times: Vec<_> = self.modifier_times.iter().collect();
        times.sort_by_key(|(k, _)| **k);
        w.len(times.len());
        for (&(gi, m), &t) in times {
            w.len(gi);
            w.len(m);
            w.f32(t);
        }
        w.0
    }

    /// Put back a [`Instance::snapshot`] of an instance of the same project.
    /// Nothing changes if it doesn't fit.
    pub fn restore(&mut self, bytes: &[u8]) -> Result<()> {
        let mut r = R { data: bytes, at: 0 };
        if r.take(4)? != MAGIC {
            return bad("not a snapshot");
        }
        if r.u8()? != VERSION {
            return bad("unknown version");
        }
        let check = Check {
            shared: &self.shared,
        };
        let n = r.count(4)?;
        if n != self.shared.var_defaults.len() {
            return bad(format!(
                "{n} variables where the project has {}",
                self.shared.var_defaults.len()
            ));
        }
        let values = (0..n).map(|_| r.f32()).collect::<Result<Vec<_>>>()?;
        let root = if r.bool()? {
            Some(check.node(&mut r, None, 0)?)
        } else {
            None
        };
        let events = self.shared.event_names.len();
        let queued = (0..r.count(8)?)
            .map(|_| {
                Ok(Event {
                    id: r.u32()? as usize,
                    raised: r.opt_index()?,
                })
            })
            .collect::<Result<VecDeque<_>>>()?;
        let raised = (0..r.count(6)?)
            .map(|_| {
                Ok(Raised {
                    event: r.str()?,
                    payload: r.opt_str()?,
                    taken: r.bool()?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        if queued
            .iter()
            .any(|e| e.id >= events || e.raised.is_some_and(|i| i >= raised.len()))
        {
            return bad("queued event out of range");
        }
        let rng = r.u64()?;
        let changes = r.u64()?;
        let pose_playing = r.bool()?;
        let matching_yaw = if r.bool()? {
            Some(r.f32()?)
        } else {
            r.f32()?;
            None
        };
        let mut modifier_times = HashMap::new();
        for _ in 0..r.count(12)? {
            let (gi, m) = (r.u32()? as usize, r.u32()? as usize);
            if check.graph(gi)?.modifiers.len() <= m {
                return bad(format!("modifier time for ({gi}, {m})"));
            }
            modifier_times.insert((gi, m), r.f32()?);
        }
        if r.at != bytes.len() {
            return bad("trailing data");
        }
        self.values = values;
        self.root = root;
        self.queue = queued;
        self.raised = raised;
        self.rng = rng | 1;
        self.changes = changes;
        self.pose_playing = pose_playing;
        self.matching_yaw = matching_yaw;
        self.modifier_times = modifier_times;
        self.ragdoll_pose.clear();
        Ok(())
    }
}
