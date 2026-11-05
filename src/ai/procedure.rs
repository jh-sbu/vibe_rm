//! Durable procedure-tree progress, independent of a loaded actor's body.
//!
//! The template supplies structure and bound inputs; the caller supplies world
//! conditions, randomness and completion observations. A frame never spins on
//! an empty or immediately completing repeat.

use std::collections::BTreeMap;

use super::package::Node;
use crate::condition::Condition;

pub(crate) type Path = Vec<u32>;

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Progress {
    pub started: bool,
    pub finished: bool,
    pub next: u32,
    pub selected: Option<u32>,
    /// Kept when a repeating subtree restarts, to avoid the previous Random pick.
    pub last: Option<u32>,
    pub elapsed: f64,
    pub count: u32,
    pub arrived: bool,
    pub spoken: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Cursor {
    /// Expanded Stacked branches share a PACK id; distinguish their progress.
    pub tree_key: String,
    /// Distinguish a repeating node re-entering the same leaf path.
    pub revision: u64,
    pub states: BTreeMap<Path, Progress>,
    pub active: Vec<Path>,
    pub completed: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Status {
    Running,
    Complete,
    Skipped,
}

impl Cursor {
    /// Re-evaluate Stacked branches only on the package evaluation cadence.
    /// Other nodes test their conditions when entered, and on that cadence.
    /// `dt` is elapsed real seconds; a newly entered Wait starts at zero.
    pub(crate) fn step(
        &mut self,
        root: &Node,
        dt: f64,
        recheck: bool,
        mut passes: impl FnMut(&[Condition]) -> bool,
        mut rand: impl FnMut() -> u64,
    ) {
        if self.completed {
            return;
        }
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        let mut active = Vec::new();
        let status = self.visit(
            root,
            &mut Vec::new(),
            dt,
            recheck,
            &mut passes,
            &mut rand,
            &mut active,
        );
        self.completed = status == Status::Complete;
        // A repeat that just finished is entered again without spending the same
        // frame's time twice. Two passes bound even a zero-second infinite loop.
        if !self.completed && active.is_empty() {
            self.visit(
                root,
                &mut Vec::new(),
                0.0,
                false,
                &mut passes,
                &mut rand,
                &mut active,
            );
        }
        self.active = active;
    }

    pub(crate) fn finish(&mut self, path: &[u32]) {
        if let Some(p) = self.states.get_mut(path) {
            p.finished = true;
        }
        self.active.retain(|p| p.as_slice() != path);
    }

    /// A different version of a template or a different selected branch must
    /// not use paths and indices from the old tree.
    pub(crate) fn valid_for(&self, root: &Node, key: &str) -> bool {
        self.tree_key == key
            && self.states.iter().all(|(path, state)| {
                root.at(path).is_some_and(|node| {
                    state.next as usize <= node.children.len()
                        && state
                            .selected
                            .is_none_or(|i| (i as usize) < node.children.len())
                        && state
                            .last
                            .is_none_or(|i| (i as usize) < node.children.len())
                })
            })
            && self
                .active
                .iter()
                .all(|path| root.at(path).is_some_and(|n| !n.procedure.is_empty()))
    }

    /// First body-moving procedure, in the old main_procedure priority order.
    /// Wait and the observer procedures never take control of the body.
    pub(crate) fn moving<'a>(&self, root: &'a Node) -> Option<(&Path, &'a Node)> {
        self.active
            .iter()
            .filter_map(|path| Some((path, root.at(path)?)))
            .filter_map(|(path, n)| {
                let rank = match n.procedure.as_str() {
                    "Patrol" => 0,
                    "Follow" | "FollowTo" => 1,
                    "Sit" => 2,
                    "Sleep" => 3,
                    "Sandbox" | "Wander" => 4,
                    "Escort" => 5,
                    "Travel" => 6,
                    "HoldPosition" => 7,
                    "Activate" | "UseWeapon" | "UseMagic" | "UseIdleMarker" | "Flee" | "Orbit"
                    | "Hover" | "Shout" | "Say" | "DialogueActivate" | "ForceGreet" | "Acquire"
                    | "FlightGrab" => 8,
                    _ => return None,
                };
                Some((rank, path, n))
            })
            .min_by_key(|(rank, _, _)| *rank)
            .map(|(_, path, n)| (path, n))
    }

    fn reset(&mut self, path: &[u32]) {
        self.revision = self.revision.saturating_add(1);
        for (p, state) in &mut self.states {
            if p.starts_with(path) {
                *state = Progress {
                    last: state.last,
                    ..Default::default()
                };
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn visit(
        &mut self,
        node: &Node,
        path: &mut Path,
        dt: f64,
        recheck: bool,
        passes: &mut impl FnMut(&[Condition]) -> bool,
        rand: &mut impl FnMut() -> u64,
        active: &mut Vec<Path>,
    ) -> Status {
        // Guard's restricted-area rules are explicitly blocked in ai-depth.
        if node.procedure == "Guard" {
            return Status::Skipped;
        }
        let state = self.states.entry(path.clone()).or_default();
        if state.finished {
            return Status::Complete;
        }
        if (!state.started || recheck) && !passes(&node.conditions) {
            return Status::Skipped;
        }
        let entered = !state.started;
        state.started = true;
        if !node.procedure.is_empty() {
            if !entered {
                state.elapsed += dt;
            }
            if node.procedure == "Wait" && state.elapsed >= node.seconds() {
                state.finished = true;
                return Status::Complete;
            }
            active.push(path.clone());
            return Status::Running;
        }
        let status = match node.kind.as_str() {
            "Sequence" => {
                let mut next = state.next as usize;
                while let Some(child) = node.children.get(next) {
                    path.push(next as u32);
                    let result = self.visit(
                        child,
                        path,
                        if entered { 0.0 } else { dt },
                        recheck,
                        passes,
                        rand,
                        active,
                    );
                    path.pop();
                    if result == Status::Running {
                        break;
                    }
                    next += 1;
                    // Following children enter at the end of this frame. A completed
                    // child's elapsed seconds are not applied to the next one.
                }
                self.states.get_mut(path).unwrap().next = next as u32;
                if next == node.children.len() {
                    Status::Complete
                } else {
                    Status::Running
                }
            }
            "Stacked" | "Random" => {
                let mut pick = state.selected;
                if entered || (node.kind == "Stacked" && recheck) {
                    let mut candidates: Vec<u32> = node
                        .children
                        .iter()
                        .enumerate()
                        .filter(|(_, n)| passes(&n.conditions))
                        .map(|(i, _)| i as u32)
                        .collect();
                    pick = if node.kind == "Stacked" {
                        candidates.first().copied()
                    } else {
                        if candidates.len() > 1 {
                            candidates.retain(|i| Some(*i) != state.last);
                        }
                        (!candidates.is_empty())
                            .then(|| candidates[rand() as usize % candidates.len()])
                    };
                    let old = state.selected;
                    if old != pick {
                        if let Some(i) = old {
                            path.push(i);
                            self.reset(path);
                            path.pop();
                        }
                    }
                    let state = self.states.get_mut(path).unwrap();
                    state.selected = pick;
                    if node.kind == "Random" {
                        state.last = pick.or(state.last);
                    }
                }
                match pick.and_then(|i| node.children.get(i as usize).map(|n| (i, n))) {
                    Some((i, child)) => {
                        path.push(i);
                        let result = self.visit(
                            child,
                            path,
                            if entered { 0.0 } else { dt },
                            recheck,
                            passes,
                            rand,
                            active,
                        );
                        path.pop();
                        result
                    }
                    // A Stacked tree with no eligible branch waits for its next
                    // evaluation. A Random with no children is skipped.
                    None if node.kind == "Stacked" => Status::Running,
                    None => Status::Skipped,
                }
            }
            "Simultaneous" => {
                let first = active.len();
                let mut any = false;
                let mut done = false;
                for (i, child) in node.children.iter().enumerate() {
                    path.push(i as u32);
                    let result = self.visit(
                        child,
                        path,
                        if entered { 0.0 } else { dt },
                        recheck,
                        passes,
                        rand,
                        active,
                    );
                    path.pop();
                    any |= result != Status::Skipped;
                    done |= result == Status::Complete;
                }
                if done || !any {
                    active.truncate(first);
                    Status::Complete
                } else {
                    Status::Running
                }
            }
            // Unrecognised node kinds wait; silently completing one would skip
            // unknown behaviour and could advance a quest.
            _ => Status::Running,
        };
        if status == Status::Complete && node.flags & 1 != 0 {
            self.reset(path);
            active.retain(|p| !p.starts_with(path));
            Status::Running
        } else {
            if status == Status::Complete {
                self.states.get_mut(path).unwrap().finished = true;
            }
            status
        }
    }
}
#[cfg(test)]
mod tests {
    use super::super::package::Input;
    use super::*;

    fn leaf(name: &str) -> Node {
        Node {
            kind: "Procedure".into(),
            flags: 0,
            conditions: vec![],
            children: vec![],
            procedure: name.into(),
            args: vec![],
            values: vec![],
        }
    }
    fn wait(seconds: f32) -> Node {
        Node {
            values: vec![Input::Float(seconds)],
            ..leaf("Wait")
        }
    }
    fn branch(kind: &str, children: Vec<Node>, repeat: bool) -> Node {
        Node {
            kind: kind.into(),
            children,
            flags: u32::from(repeat),
            ..leaf("")
        }
    }
    fn tick(c: &mut Cursor, n: &Node, dt: f64) {
        c.step(n, dt, true, |_| true, || 0);
    }

    #[test]
    fn sequence_advances_skips_and_completes_without_reusing_time() {
        let mut skipped = wait(100.0);
        skipped.conditions.push(Condition::default());
        let n = branch("Sequence", vec![wait(2.0), skipped, wait(3.0)], false);
        let mut c = Cursor::default();
        c.step(&n, 0.0, true, |v| v.is_empty(), || 0);
        assert_eq!(c.active, vec![vec![0]]);
        c.step(&n, 2.0, true, |v| v.is_empty(), || 0);
        assert_eq!(c.active, vec![vec![2]]);
        assert_eq!(c.states[&vec![2]].elapsed, 0.0);
        c.step(&n, 3.0, true, |v| v.is_empty(), || 0);
        assert!(c.completed);
        assert!(c.active.is_empty());
    }

    #[test]
    fn simultaneous_wait_ends_sandbox_and_body_priority_matches_main_procedure() {
        let n = branch(
            "Simultaneous",
            vec![leaf("Travel"), leaf("Sandbox"), wait(2.0)],
            false,
        );
        let mut c = Cursor::default();
        tick(&mut c, &n, 0.0);
        assert_eq!(c.active.len(), 3);
        assert_eq!(c.moving(&n).unwrap().1.procedure, "Sandbox");
        tick(&mut c, &n, 2.0);
        assert!(c.completed);
        assert!(c.active.is_empty());
    }

    #[test]
    fn stacked_switches_only_when_evaluation_is_due_and_restarts_the_old_branch() {
        let mut first = leaf("Follow");
        first.conditions.push(Condition::default());
        let n = branch("Stacked", vec![first, leaf("Travel")], false);
        let mut c = Cursor::default();
        tick(&mut c, &n, 0.0);
        assert_eq!(c.active, vec![vec![0]]);
        c.step(&n, 1.0, false, |v| v.is_empty(), || 0);
        assert_eq!(c.active, vec![vec![0]]);
        c.step(&n, 1.0, true, |v| v.is_empty(), || 0);
        assert_eq!(c.active, vec![vec![1]]);
        tick(&mut c, &n, 1.0);
        assert_eq!(c.active, vec![vec![0]]);
        assert_eq!(c.states[&vec![0]].elapsed, 0.0);
    }

    #[test]
    fn repeating_random_excludes_last_pick_and_single_child_can_repeat() {
        let n = branch("Random", vec![wait(1.0), wait(1.0), wait(1.0)], true);
        let mut c = Cursor::default();
        tick(&mut c, &n, 0.0);
        assert_eq!(c.active, vec![vec![0]]);
        tick(&mut c, &n, 1.0);
        assert_eq!(c.active, vec![vec![1]]);
        assert!(!c.completed);
        tick(&mut c, &n, 1.0);
        assert_eq!(c.active, vec![vec![0]]);
        let n = branch("Random", vec![wait(1.0)], true);
        let mut c = Cursor::default();
        tick(&mut c, &n, 0.0);
        tick(&mut c, &n, 1.0);
        assert_eq!(c.active, vec![vec![0]]);
        assert!(!c.completed);
    }

    #[test]
    fn repeated_sequences_restart_and_immediate_repeats_are_bounded() {
        let n = branch("Sequence", vec![leaf("Travel"), wait(1.0)], true);
        let mut c = Cursor::default();
        tick(&mut c, &n, 0.0);
        c.finish(&[0]);
        tick(&mut c, &n, 0.0);
        assert_eq!(c.active, vec![vec![1]]);
        tick(&mut c, &n, 1.0);
        assert_eq!(c.active, vec![vec![0]]);
        let empty = branch("Sequence", vec![wait(0.0)], true);
        tick(&mut c, &empty, 1000.0);
        assert!(!c.completed);
    }

    #[test]
    fn false_simultaneous_children_do_not_end_the_branch() {
        let mut skipped = wait(0.0);
        skipped.conditions.push(Condition::default());
        let n = branch("Simultaneous", vec![skipped, leaf("Sandbox")], false);
        let mut c = Cursor::default();
        c.step(&n, 0.0, true, |v| v.is_empty(), || 0);
        assert!(!c.completed);
        assert_eq!(c.active, vec![vec![1]]);
    }

    #[test]
    fn no_stacked_candidate_waits_and_an_unknown_procedure_never_completes_by_accident() {
        let mut child = leaf("Travel");
        child.conditions.push(Condition::default());
        let n = branch("Stacked", vec![child], false);
        let mut c = Cursor::default();
        c.step(&n, 0.0, true, |v| v.is_empty(), || 0);
        assert!(!c.completed);
        assert!(c.active.is_empty());
        let n = leaf("Unsupported");
        let mut c = Cursor::default();
        tick(&mut c, &n, 1e9);
        assert!(!c.completed);
        assert!(c.moving(&n).is_none());
    }

    #[test]
    fn progress_survives_cloning_and_an_external_completion_ends_once() {
        let n = branch("Sequence", vec![wait(5.0), leaf("Activate")], false);
        let mut c = Cursor::default();
        tick(&mut c, &n, 0.0);
        tick(&mut c, &n, 2.0);
        let mut away = c.clone();
        tick(&mut away, &n, 3.0);
        assert_eq!(away.active, vec![vec![1]]);
        assert_ne!(away, c);
        away.finish(&[1]);
        tick(&mut away, &n, 0.0);
        assert!(away.completed);
        let saved = away.clone();
        tick(&mut away, &n, 10.0);
        assert_eq!(saved, away);
    }
}
