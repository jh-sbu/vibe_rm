//! Connect body-independent tree progress to loaded and away package execution.
use super::{
    exec::{ActorExec, Outcome},
    package::{Node, Package},
    procedure::Cursor,
};
use crate::{
    condition::{self, Context},
    engine::Engine,
};
use esp::FormId;

impl ActorExec {
    pub(crate) fn effective<'p>(&self, p: &'p Package) -> std::borrow::Cow<'p, Package> {
        let run = self.run.as_ref().filter(|r| r.id == p.id);
        match (
            p.tree.as_ref(),
            run.and_then(|r| r.cursor.as_ref().map(|c| (r, c))),
        ) {
            (Some(tree), Some((_run, cursor)))
                if cursor.tree_key == p.editor_id && tree.executable() =>
            {
                std::borrow::Cow::Owned(p.for_procedure(cursor.moving(tree).map(|(_, n)| n)))
            }
            _ => std::borrow::Cow::Borrowed(p),
        }
    }

    /// A procedure's end advances its cursor. Only a completed root ends PACK.
    pub(crate) fn complete_procedure(&mut self, gives_way: bool) {
        let has_cursor = self.run.as_ref().is_some_and(|r| r.cursor.is_some());
        let path = self
            .current
            .and_then(|i| self.packages.get(i))
            .and_then(|p| Some((p, p.tree.as_ref()?)))
            .and_then(|(p, tree)| {
                self.run
                    .as_ref()?
                    .cursor
                    .as_ref()
                    .filter(|c| c.tree_key == p.editor_id)?
                    .moving(tree)
                    .map(|(path, _)| path.clone())
            });
        if let Some(path) = path {
            let tree = self
                .current
                .and_then(|i| self.packages.get(i))
                .and_then(|p| p.tree.clone())
                .unwrap();
            let ends = ends_root(&tree, &path);
            if let Some(c) = self.run.as_mut().and_then(|r| r.cursor.as_mut()) {
                c.finish(&path);
                if ends {
                    c.step(&tree, 0.0, false, |_| true, || 0);
                }
            }
            self.next_eval = 0.0;
            if ends {
                let composite = tree.children.len() > 1;
                self.complete_run(gives_way || composite);
            }
        } else if !has_cursor {
            self.complete_run(gives_way);
        }
    }
}

impl Engine {
    /// Enter/recheck a selected branch while evaluating packages. Unsupported
    /// procedures keep the pre-interpreter behaviour until their implementation
    /// steps land, so preparatory Find/Acquire cannot stall existing practice.
    pub(super) fn cursor_for(
        &self,
        actor: FormId,
        exec: &ActorExec,
        p: &Package,
        rand: &mut impl FnMut() -> u64,
    ) -> Option<Cursor> {
        let tree = p.tree.as_ref().filter(|n| n.executable())?;
        let mut cursor = exec
            .run
            .as_ref()
            .filter(|r| r.id == p.id)
            .and_then(|r| r.cursor.clone())
            .filter(|c| c.valid_for(tree, &p.editor_id))
            .unwrap_or_else(|| Cursor {
                tree_key: p.editor_id.clone(),
                ..Default::default()
            });
        let ctx = Context {
            subject: Some(actor),
            quest: p.quest,
            ..Default::default()
        };
        cursor.step(
            tree,
            0.0,
            true,
            |cs| condition::evaluate(self, cs, ctx),
            rand,
        );
        Some(cursor)
    }

    pub(crate) fn advance_loaded_trees(&mut self, dt: f32) {
        let mut seed = self.rand() | 1;
        let mut random = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let mut changes = Vec::new();
        for a in self.cells.values().flat_map(|rt| &rt.actors) {
            if a.dead || a.exec.pinned || a.exec.combat.is_some() {
                continue;
            }
            let Some(run) = a
                .exec
                .run
                .as_ref()
                .filter(|r| r.outcome == Outcome::Running)
            else {
                continue;
            };
            let Some(p) = a.exec.current.and_then(|i| a.exec.packages.get(i)) else {
                continue;
            };
            let Some(tree) = p.tree.as_ref().filter(|n| n.executable()) else {
                continue;
            };
            let Some(mut c) = run.cursor.clone() else {
                continue;
            };
            if !c.valid_for(tree, &p.editor_id) {
                continue;
            }
            let before = c.active.clone();
            let ctx = Context {
                subject: Some(a.ref_id),
                quest: p.quest,
                ..Default::default()
            };
            c.step(
                tree,
                dt as f64,
                false,
                |cs| condition::evaluate(self, cs, ctx),
                &mut random,
            );
            changes.push((a.ref_id, before != c.active, c));
        }
        for (id, changed, cursor) in changes {
            let Some(key) = self.actor_cells.get(&id).copied() else {
                continue;
            };
            let Some(a) = self
                .cells
                .get_mut(&key)
                .and_then(|rt| rt.actors.iter_mut().find(|a| a.ref_id == id))
            else {
                continue;
            };
            let done = cursor.completed;
            if let Some(run) = a.exec.run.as_mut() {
                run.cursor = Some(cursor);
            }
            if done {
                a.exec.complete_run(true);
            }
            if changed {
                a.exec.next_eval = 0.0;
                if let Some(run) = a.exec.run.as_mut() {
                    run.activations = 0;
                }
                a.exec.patrol = None;
                a.interrupt(&mut self.furniture);
            }
        }
    }

    pub(crate) fn advance_away_tree(&mut self, actor: FormId, dt: f32) {
        let Some(exec) = self.away.get(&actor) else {
            return;
        };
        let Some(run) = exec.run.as_ref().filter(|r| r.outcome == Outcome::Running) else {
            return;
        };
        let Some(p) = exec.current.and_then(|i| exec.packages.get(i)) else {
            return;
        };
        let Some(tree) = p.tree.as_ref().filter(|n| n.executable()).cloned() else {
            return;
        };
        let Some(mut c) = run.cursor.clone() else {
            return;
        };
        if !c.valid_for(&tree, &p.editor_id) {
            return;
        }
        let before = c.active.clone();
        let ctx = Context {
            subject: Some(actor),
            quest: p.quest,
            ..Default::default()
        };
        let mut seed = self.rand() | 1;
        c.step(
            &tree,
            dt as f64,
            false,
            |cs| condition::evaluate(self, cs, ctx),
            || {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                seed
            },
        );
        let changed = before != c.active;
        let Some(exec) = self.away.get_mut(&actor) else {
            return;
        };
        let done = c.completed;
        if let Some(run) = exec.run.as_mut() {
            run.cursor = Some(c);
        }
        if changed {
            exec.settled = false;
            exec.ends_at = None;
            exec.jump_at = None;
            exec.strode_at = None;
            exec.on_arrival = None;
            exec.stage = None;
            if let Some(run) = exec.run.as_mut() {
                run.activations = 0;
            }
        }
        if done {
            exec.complete_run(true);
        }
    }
}

impl Node {
    /// Enable trees as their procedures become available. This temporary gate
    /// belongs to the unfinished plan, not to the tree interpreter's semantics.
    pub(crate) fn executable(&self) -> bool {
        (self.procedure.is_empty()
            || matches!(
                self.procedure.as_str(),
                "Travel"
                    | "Sandbox"
                    | "Wait"
                    | "Activate"
                    | "Patrol"
                    | "Follow"
                    | "Escort"
                    | "Sit"
                    | "Sleep"
                    | "HoldPosition"
                    | "UseMagic"
                    | "Eat"
                    | "Guard"
            ))
            && self.children.iter().all(Node::executable)
    }
}

/// Whether completing this leaf can only complete ancestors, without entering
/// another branch or repeating. This preserves immediate arrival events.
fn ends_root(root: &Node, path: &[u32]) -> bool {
    let mut node = root;
    for &i in path {
        if node.flags & 1 != 0 || (node.kind == "Sequence" && i as usize + 1 != node.children.len())
        {
            return false;
        }
        let Some(child) = node.children.get(i as usize) else {
            return false;
        };
        node = child;
    }
    node.flags & 1 == 0
}

#[cfg(test)]
mod tests {
    use super::super::{
        exec::{
            RunEvent,
            tests::{ACTOR, CELL, MARKER, PACKAGE, fixture, loaded_actor},
        },
        package::{Behaviour, Input, Location, LocationKind},
    };
    use super::*;
    use glam::Vec3;
    use std::sync::Arc;

    fn leaf(name: &str, values: Vec<Input>) -> Node {
        Node {
            kind: "Procedure".into(),
            flags: 0,
            conditions: vec![],
            children: vec![],
            procedure: name.into(),
            args: vec![],
            values,
        }
    }
    fn sequence(children: Vec<Node>, repeat: bool) -> Node {
        Node {
            kind: "Sequence".into(),
            flags: u32::from(repeat),
            children,
            ..leaf("", vec![])
        }
    }
    fn location(kind: LocationKind) -> Input {
        Input::Location(Location { kind, radius: 0.0 })
    }
    fn install(e: &mut Engine, tree: Node) {
        e.location = crate::engine::Location::Interior(CELL);
        e.scripts.alias_gen = 7;
        loaded_actor(e);
        let a = e.actor_mut(ACTOR).unwrap();
        let mut p = Package::bare(PACKAGE, Behaviour::Travel);
        p.schedule.hour = -1;
        p.schedule.day_of_week = -1;
        p.tree = Some(Arc::new(tree));
        a.exec.packages = vec![p];
        a.exec.current = None;
        a.exec.run = None;
        a.exec.next_eval = 0.0;
        a.fresh = false;
        e.evaluate_packages(0.0);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn loaded_sequence_uses_each_leafs_goal_and_does_not_end_on_first_arrival() {
        let (_f, mut e) = fixture();
        install(
            &mut e,
            sequence(
                vec![
                    leaf(
                        "Travel",
                        vec![location(LocationKind::NearReference(MARKER))],
                    ),
                    leaf("Wait", vec![Input::Float(5.0)]),
                    leaf("Travel", vec![location(LocationKind::NearEditor)]),
                ],
                true,
            ),
        );
        assert_eq!(
            e.actor_ref(ACTOR).unwrap().exec.goal.unwrap().centre,
            Vec3::new(3000., 0., 0.)
        );
        e.actor_mut(ACTOR).unwrap().exec.pos = Vec3::new(3000., 0., 0.);
        e.complete_arrivals();
        assert!(!e.actor_ref(ACTOR).unwrap().exec.run_completed());
        e.advance_loaded_trees(0.0);
        e.evaluate_packages(0.0);
        assert_eq!(
            e.actor_ref(ACTOR).unwrap().exec.goal.unwrap().behaviour,
            Behaviour::Hold
        );
        e.advance_loaded_trees(4.0);
        assert_eq!(
            e.actor_ref(ACTOR)
                .unwrap()
                .exec
                .run
                .as_ref()
                .unwrap()
                .cursor
                .as_ref()
                .unwrap()
                .active,
            vec![vec![1]]
        );
        e.advance_loaded_trees(1.0);
        e.evaluate_packages(0.0);
        let a = e.actor_ref(ACTOR).unwrap();
        assert_eq!(a.exec.goal.unwrap().behaviour, Behaviour::Travel);
        assert_eq!(a.exec.goal.unwrap().centre, a.editor_pos);
        assert_eq!(a.exec.run_events, vec![RunEvent::Start(PACKAGE)]);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn an_away_wait_keeps_its_loaded_seconds_and_then_selects_the_next_travel() {
        let (_f, mut e) = fixture();
        install(
            &mut e,
            sequence(
                vec![
                    leaf("Wait", vec![Input::Float(5.0)]),
                    leaf(
                        "Travel",
                        vec![location(LocationKind::NearReference(MARKER))],
                    ),
                ],
                false,
            ),
        );
        e.advance_loaded_trees(4.0);
        e.unload_all();
        e.scripts.real_time += 1.0;
        e.step_away(ACTOR);
        let x = &e.away[&ACTOR];
        let p = x.effective(&x.packages[x.current.unwrap()]);
        assert_eq!(p.behaviour, Behaviour::Travel);
        assert!(matches!(p.location.unwrap().kind, LocationKind::NearReference(r) if r == MARKER));
        assert_eq!(
            x.run.as_ref().unwrap().cursor.as_ref().unwrap().active,
            vec![vec![1]]
        );
        assert!(!x.run_completed());
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn a_single_travel_tree_preserves_immediate_arrival_events_and_real_pack_id() {
        let (_f, mut e) = fixture();
        install(
            &mut e,
            sequence(
                vec![leaf(
                    "Travel",
                    vec![location(LocationKind::NearReference(MARKER))],
                )],
                false,
            ),
        );
        e.actor_mut(ACTOR).unwrap().exec.pos = Vec3::new(3000., 0., 0.);
        e.complete_arrivals();
        let x = &e.actor_ref(ACTOR).unwrap().exec;
        assert_eq!(x.package(), Some(PACKAGE));
        assert!(x.run_completed());
        assert_eq!(
            x.run_events,
            vec![RunEvent::Start(PACKAGE), RunEvent::End(PACKAGE)]
        );
        assert!(x.run.as_ref().unwrap().cursor.as_ref().unwrap().completed);
    }
}
