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
                std::borrow::Cow::Owned(p.for_procedure(cursor.body(tree)))
            }
            _ => std::borrow::Cow::Borrowed(p),
        }
    }

    /// Remember reaching the final patrol point so its furniture is used once,
    /// then the procedure completes instead of entering that marker again.
    pub(crate) fn reached_last_patrol_point(&mut self) -> bool {
        let path = self
            .current
            .and_then(|i| self.packages.get(i))
            .and_then(|p| {
                self.run
                    .as_ref()?
                    .cursor
                    .as_ref()?
                    .moving(p.tree.as_ref()?)
                    .map(|(path, _)| path.clone())
            });
        let Some(path) = path else {
            return false;
        };
        let Some(progress) = self
            .run
            .as_mut()
            .and_then(|r| r.cursor.as_mut())
            .and_then(|c| c.states.get_mut(&path))
        else {
            return false;
        };
        std::mem::replace(&mut progress.arrived, true)
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
            if tree.at(&path).is_some_and(|n| {
                matches!(n.procedure.as_str(), "HoldPosition" | "DialogueActivate")
            }) {
                return;
            }
            let ends = ends_root(&tree, &path);
            if let Some(c) = self.run.as_mut().and_then(|r| r.cursor.as_mut()) {
                c.finish(&path);
                if ends {
                    c.step(&tree, 0.0, false, |_| true, || 0);
                }
            }
            self.next_eval = 0.0;
            if ends {
                let composite = !tree
                    .single_procedure()
                    .is_some_and(|n| matches!(n.procedure.as_str(), "Travel" | "Escort"));
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
            let before = (c.active.clone(), c.revision);
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
            changes.push((a.ref_id, before != (c.active.clone(), c.revision), c));
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
                    run.practice = None;
                }
                a.exec.patrol = None;
                let waiting = a
                    .exec
                    .current
                    .and_then(|i| a.exec.packages.get(i))
                    .and_then(|p| p.tree.as_ref())
                    .zip(a.exec.run.as_ref().and_then(|r| r.cursor.as_ref()))
                    .is_some_and(|(tree, c)| c.waits_in_place(tree));
                if !waiting {
                    a.interrupt(&mut self.furniture);
                }
                self.whereabouts.next_refresh = 0.0;
                let whole_cell = a.exec.current.and_then(|i| a.exec.packages.get(i)).is_some_and(|p| {
                    let p = a.exec.effective(p);
                    p.behaviour == super::package::Behaviour::Sandbox && matches!((key, p.location),
                        (crate::render::CellKey::Interior(cell), Some(super::package::Location { kind: super::package::LocationKind::InCell(target), .. })) if cell == target)
                });
                if whole_cell {
                    a.fresh = true;
                    a.load_cell = Some(key);
                }
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
        let before = (c.active.clone(), c.revision);
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
        let changed = before != (c.active.clone(), c.revision);
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
                run.practice = None;
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
                    | "LockDoors"
                    | "UnlockDoors"
                    | "Say"
                    | "DialogueActivate"
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
        let mut p = Package::bare(PACKAGE, Behaviour::Travel);
        p.tree = Some(Arc::new(tree));
        install_package(e, p);
    }
    fn install_package(e: &mut Engine, mut p: Package) {
        e.location = crate::engine::Location::Interior(CELL);
        e.scripts.alias_gen = 7;
        loaded_actor(e);
        let a = e.actor_mut(ACTOR).unwrap();
        p.schedule.hour = -1;
        p.schedule.day_of_week = -1;
        p.conditions.clear();
        a.exec.packages = vec![p];
        a.exec.current = None;
        a.exec.run = None;
        a.exec.next_eval = 0.0;
        a.fresh = false;
        e.evaluate_packages(0.0);
    }

    #[test]
    #[ignore = "requires installed Skyrim SE masters and a headless GPU"]
    fn vanilla_guard_travel_and_activation_sequences_advance_loaded_goals() {
        let (_f, mut e) = fixture();
        let data = vfs::locate_data_dir().unwrap();
        let names: Vec<_> = esp::BASE_MASTERS.iter().map(|s| s.to_string()).collect();
        e.lo = esp::LoadOrder::load(&data, &names).unwrap();
        let template = e.lo.find_editor_id("TravelBackAndForthAndGuard").unwrap();
        let p =
            e.lo.ids_of_type(b"PACK")
                .iter()
                .find_map(|&id| {
                    let r = e.lo.get(id)?;
                    let d = r.get(b"PKCU")?.get(4..8)?;
                    (r.fid(FormId(u32::from_le_bytes(d.try_into().ok()?))) == template)
                        .then(|| super::super::package::parse(&e.lo, id))
                        .flatten()
                })
                .unwrap();
        let id = p.id;
        install_package(&mut e, p);
        let first = e.actor_ref(ACTOR).unwrap().exec.goal.unwrap().centre;
        assert!(first.is_finite());
        for expected in [vec![1, 1], vec![1, 0]] {
            let goal = e.actor_ref(ACTOR).unwrap().exec.goal.unwrap();
            e.actor_mut(ACTOR).unwrap().exec.pos = goal.centre;
            e.complete_arrivals();
            e.advance_loaded_trees(0.);
            e.evaluate_packages(0.);
            let x = &e.actor_ref(ACTOR).unwrap().exec;
            assert_eq!(x.package(), Some(id));
            assert_eq!(
                x.run.as_ref().unwrap().cursor.as_ref().unwrap().active,
                vec![expected]
            );
            assert!(!x.run_completed());
        }

        e.cells.values_mut().for_each(|rt| rt.actors.clear());
        e.actor_cells.clear();
        let p = super::super::package::parse(&e.lo, FormId(0x8CABD)).unwrap();
        install_package(&mut e, p);
        let at = e.actor_ref(ACTOR).unwrap().exec.goal.unwrap().centre;
        let cell = e.lo.cell_of_ref(FormId(0x8CAB7)).unwrap();
        let key = crate::render::CellKey::Interior(cell);
        let rt = e
            .cells
            .remove(&crate::render::CellKey::Interior(CELL))
            .unwrap();
        e.cells.insert(key, rt);
        e.actor_cells.insert(ACTOR, key);
        e.location = crate::engine::Location::Interior(cell);
        e.actor_mut(ACTOR).unwrap().exec.pos = at;
        e.complete_arrivals();
        let mut seen = vec![];
        for _ in 0..3 {
            e.advance_loaded_trees(0.);
            e.evaluate_packages(0.);
            let a = e.actor_mut(ACTOR).unwrap();
            let Some(goal) = a.exec.goal else {
                break;
            };
            if goal.behaviour != Behaviour::Activate {
                break;
            }
            seen.push(goal.target.unwrap());
            a.activation_done(&goal);
        }
        assert_eq!(seen, [FormId(0x8CAB7), FormId(0x8CABC)]);
        assert_eq!(
            e.actor_ref(ACTOR).unwrap().exec.done_once,
            Some(FormId(0x8CABD))
        );
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
    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn waiting_advances_the_cursor_without_respawning_at_location_one() {
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
                true,
            ),
        );
        let origin = e.actor_ref(ACTOR).unwrap().exec.pos;
        e.pass_time(5.0);
        let a = e.actor_ref(ACTOR).unwrap();
        assert_eq!(a.exec.pos, origin);
        assert_eq!(a.exec.goal.unwrap().behaviour, Behaviour::Travel);
        assert_eq!(
            a.exec.run.as_ref().unwrap().cursor.as_ref().unwrap().active,
            vec![vec![1]]
        );
        assert_eq!(a.exec.run_events, vec![RunEvent::Start(PACKAGE)]);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn near_package_start_keeps_the_runs_origin_when_the_actor_moves() {
        let (_f, mut e) = fixture();
        install(
            &mut e,
            sequence(
                vec![leaf("Travel", vec![location(LocationKind::NearCurrent)])],
                false,
            ),
        );
        let origin = e.actor_ref(ACTOR).unwrap().exec.pos;
        let a = e.actor_mut(ACTOR).unwrap();
        a.exec.pos += Vec3::X * 2000.;
        a.exec.next_eval = 0.0;
        e.evaluate_packages(0.0);
        assert_eq!(
            e.actor_ref(ACTOR).unwrap().exec.goal.unwrap().centre,
            origin
        );
        assert_eq!(
            e.within_package_location(
                ACTOR,
                ACTOR,
                Location {
                    kind: LocationKind::NearCurrent,
                    radius: 10.0
                },
                None
            ),
            Some(false)
        );
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn arriving_at_a_whole_cell_stop_requests_its_cell_wide_furniture_placement() {
        let (_f, mut e) = fixture();
        install(
            &mut e,
            sequence(
                vec![
                    leaf("Wait", vec![Input::Float(1.0)]),
                    leaf("Sandbox", vec![location(LocationKind::InCell(CELL))]),
                ],
                false,
            ),
        );
        e.advance_loaded_trees(1.0);
        e.evaluate_packages(0.0);
        let a = e.actor_ref(ACTOR).unwrap();
        assert!(a.fresh);
        assert_eq!(a.load_cell, Some(crate::render::CellKey::Interior(CELL)));
        assert_eq!(a.exec.goal.unwrap().behaviour, Behaviour::Sandbox);
    }
    #[test]
    fn non_repeating_patrol_completes_at_its_final_point() {
        use super::super::{ActorRuntime, Clips, Goal, PatrolPoint, PatrolStep, World};
        let mut a = ActorRuntime::new(
            ACTOR,
            super::super::exec::tests::BASE,
            Arc::new(crate::world::skeleton::Skeleton::default()),
            glam::Mat4::IDENTITY,
            vec![],
            0.0,
        );
        let start = MARKER;
        let mut p = Package::bare(PACKAGE, Behaviour::Patrol);
        p.tree = Some(Arc::new(sequence(vec![leaf("Patrol", vec![])], false)));
        let mut c = Cursor {
            tree_key: p.editor_id.clone(),
            ..Default::default()
        };
        c.step(p.tree.as_ref().unwrap(), 0.0, true, |_| true, || 1);
        a.exec.packages = vec![p];
        a.exec.current = Some(0);
        a.exec.begin_run(Some(0), 0.0, false);
        a.exec.run.as_mut().unwrap().cursor = Some(c);
        let goal = Goal {
            behaviour: Behaviour::Patrol,
            target: Some(start),
            repeat: false,
            radius: 50.0,
            ..Goal::travel(Vec3::ZERO)
        };
        let nav = super::super::nav::NavWorld::default();
        let mut furniture = super::super::furniture::FurnitureWorld::default();
        let vfs = vfs::Vfs::new("/nonexistent-patrol-fixture", &[]);
        let mut anims = crate::world::animation::AnimationLibrary::default();
        let mut behaviors = crate::world::animation::BehaviorLibrary::default();
        let patrols = std::collections::HashMap::from([(
            start,
            Arc::new(vec![
                PatrolPoint {
                    ref_id: start,
                    pos: Vec3::ZERO,
                },
                PatrolPoint {
                    ref_id: ACTOR,
                    pos: Vec3::X * 500.0,
                },
            ]),
        )]);
        let mut w = World {
            nav: &nav,
            furniture: &mut furniture,
            clips: Clips {
                vfs: &vfs,
                anims: &mut anims,
                behaviors: &mut behaviors,
            },
            may_use: &|_, _, _| true,
            claimed: &Default::default(),
            patrols: &patrols,
            targets: &Default::default(),
            holds: &|_, _| false,
            rand: &mut || 1,
        };
        assert!(matches!(a.patrol_target(&goal, &mut w), PatrolStep::Busy));
        assert!(!a.exec.run_completed());
        a.exec.pos = Vec3::X * 500.0;
        assert!(matches!(a.patrol_target(&goal, &mut w), PatrolStep::Done));
        assert!(a.exec.run_completed());
        assert_eq!(
            a.exec.run_events,
            vec![RunEvent::Start(PACKAGE), RunEvent::End(PACKAGE)]
        );
    }
    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn hold_position_returns_to_its_origin_without_completing_on_arrival() {
        let (_f, mut e) = fixture();
        install(&mut e, sequence(vec![leaf("HoldPosition", vec![])], false));
        let origin = e.actor_ref(ACTOR).unwrap().exec.pos;
        let a = e.actor_mut(ACTOR).unwrap();
        a.exec.pos += Vec3::X * 1000.;
        a.exec.next_eval = 0.0;
        e.evaluate_packages(0.0);
        let a = e.actor_ref(ACTOR).unwrap();
        assert_eq!(a.exec.goal.unwrap().centre, origin);
        assert_eq!(a.exec.goal.unwrap().behaviour, Behaviour::Travel);
        e.actor_mut(ACTOR).unwrap().exec.pos = origin;
        e.complete_arrivals();
        assert!(!e.actor_ref(ACTOR).unwrap().exec.run_completed());
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn an_away_run_start_keeps_its_space_when_coordinates_overlap_elsewhere() {
        use super::super::schedule::Place;
        let (_f, mut e) = fixture();
        let mut p = Package::bare(PACKAGE, Behaviour::Travel);
        p.location = Some(Location {
            kind: LocationKind::NearCurrent,
            radius: 10.0,
        });
        let mut x = ActorExec::new(vec![p], Vec3::ZERO, 0.0, 0.0);
        x.current = Some(0);
        x.place = Some(Place::Interior(CELL));
        x.begin_run(Some(0), 0.0, false);
        x.place = Some(Place::Exterior(esp::FormId(0x3C), (0, 0)));
        e.away.insert(ACTOR, x);
        let p = &e.away[&ACTOR].packages[0];
        assert_eq!(
            e.package_location_place(ACTOR, p),
            Some((Place::Interior(CELL), Vec3::ZERO))
        );
        assert_eq!(
            e.within_package_location(ACTOR, ACTOR, p.location.unwrap(), None),
            Some(false)
        );
    }
    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn a_new_package_uses_its_own_start_in_the_first_goal() {
        let (_f, mut e) = fixture();
        install(&mut e, sequence(vec![leaf("HoldPosition", vec![])], false));
        let a = e.actor_mut(ACTOR).unwrap();
        a.exec.pos += Vec3::X * 500.0;
        let new_origin = a.exec.pos;
        let mut p = a.exec.packages[0].clone();
        p.id = super::super::exec::tests::PACKAGE2;
        p.editor_id = "second".into();
        a.exec.packages.insert(0, p);
        a.exec.current = Some(1);
        a.exec.next_eval = 0.0;
        e.evaluate_packages(0.0);
        assert_eq!(
            e.actor_ref(ACTOR).unwrap().exec.goal.unwrap().centre,
            new_origin
        );
        assert_eq!(
            e.actor_ref(ACTOR)
                .unwrap()
                .exec
                .run
                .as_ref()
                .unwrap()
                .origin,
            Some(new_origin)
        );
    }
}

impl Engine {
    pub(crate) fn loaded_procedure_locks(&mut self) {
        let actors: Vec<_> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| !a.dead && !a.exec.pinned && a.exec.combat.is_none())
            .map(|a| a.ref_id)
            .collect();
        for actor in actors {
            self.procedure_locks(actor);
        }
    }

    pub(crate) fn procedure_locks(&mut self, actor: FormId) {
        let exec = self
            .actor_ref(actor)
            .map(|a| &a.exec)
            .or_else(|| self.away.get(&actor));
        let Some(exec) = exec else {
            return;
        };
        let Some((p, tree, cursor)) = exec.current.and_then(|i| {
            let p = exec.packages.get(i)?;
            Some((p, p.tree.as_ref()?, exec.run.as_ref()?.cursor.as_ref()?))
        }) else {
            return;
        };
        let actions: Vec<_> = cursor
            .active
            .iter()
            .filter_map(|path| {
                let node = tree.at(path)?;
                let locked = match node.procedure.as_str() {
                    "LockDoors" => true,
                    "UnlockDoors" => false,
                    _ => return None,
                };
                let p = p.for_procedure(Some(node));
                let place = self
                    .package_location_place(actor, &p)
                    .map(|x| x.0)
                    .or_else(|| match p.location.map(|l| l.kind) {
                        Some(
                            super::package::LocationKind::NearCurrent
                            | super::package::LocationKind::NearSelf,
                        ) => self.current_place_of(actor).map(|x| x.0),
                        _ => None,
                    });
                Some((path.clone(), place, locked))
            })
            .collect();
        for (path, place, locked) in actions {
            if let Some(super::schedule::Place::Interior(cell)) = place {
                self.procedure_cell_locks(cell, locked);
            }
            // A location without a lockable load door has nothing to do.
            let exec = match self.actor_mut(actor) {
                Some(a) => Some(&mut a.exec),
                None => self.away.get_mut(&actor),
            };
            if let Some(exec) = exec {
                if let Some(cursor) = exec.run.as_mut().and_then(|r| r.cursor.as_mut()) {
                    cursor.finish(&path);
                }
                exec.next_eval = 0.0;
            }
        }
    }
}
