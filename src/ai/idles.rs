//! The IDLE record tree: which behaviour event an idle plays, which idles belong
//! to furniture with a given keyword, and picking an idle by walking a subtree.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use esp::{FormId, LoadOrder};

use crate::condition::{self, Condition};
use crate::world::behavior::ProjectRuntime;

/// Idles of the humanoid graphs (or inheriting no graph).
fn is_humanoid_graph(behavior: &str) -> bool {
    behavior.is_empty() || behavior.starts_with("actors\\character\\")
}

/// Condition function `HasKeyword`.
const HAS_KEYWORD: u16 = 560;
/// Condition function `GetItemCount`.
const GET_ITEM_COUNT: u16 = 47;
/// Events that leave furniture rather than enter it.
const EXIT_EVENTS: [&str; 3] = ["idlechairexitstart", "idlefurnitureexit", "idlestop"];

/// Resolved before borrowing the actor update loop, so conditions read live
/// engine state and the graph can consume the events in this update.
#[derive(Default)]
pub(crate) struct LocomotionActions(pub [Option<Arc<str>>; 10]);
impl LocomotionActions {
    pub(crate) const NAMES: [&'static str; 10] = [
        "ActionMoveStart",
        "ActionMoveStop",
        "ActionTurnLeft",
        "ActionTurnRight",
        "ActionTurnStop",
        "ActionSneak",
        "ActionSprintStart",
        "ActionSprintStop",
        "ActionSwimStateChange",
        "ActionDraw",
    ];
    pub(crate) fn event(&self, action: &str) -> Option<&str> {
        self.0[Self::NAMES.iter().position(|name| *name == action)?].as_deref()
    }
    pub(crate) fn send(&self, graph: &mut crate::world::behavior::GraphAnim, action: &str) {
        if let Some(ev) = self.event(action) {
            graph.send_event(ev);
        }
    }
}

struct Idle {
    parent: FormId,
    event: String,
    /// Behaviour file, lowercase (empty: inherited from the parent).
    behavior: String,
    conditions: Vec<Condition>,
}

/// Graph filtering and unconditional selection are done once per project/action.
enum ActionTree {
    Fixed(Option<Arc<str>>),
    Conditional(Vec<ActionBranch>),
}

struct ActionBranch {
    idle: FormId,
    event: Option<Arc<str>>,
    children: Vec<ActionBranch>,
}

type ProjectActions = HashMap<String, HashMap<FormId, ActionTree>>;

#[derive(Default)]
pub struct IdleIndex {
    idles: HashMap<FormId, Idle>,
    /// Directory -> project name -> root. All lookups borrow existing strings.
    actions: RefCell<HashMap<String, ProjectActions>>,
    locomotion_roots: [Option<FormId>; 10],
    children: HashMap<FormId, Vec<FormId>>,
    /// Keyword -> root idles conditioned on it.
    by_keyword: HashMap<FormId, Vec<FormId>>,
}

impl IdleIndex {
    pub fn build(lo: &LoadOrder) -> IdleIndex {
        let mut ix = IdleIndex::default();
        let mut previous: HashMap<FormId, FormId> = HashMap::new();
        for &id in lo.ids_of_type(b"IDLE") {
            let Some(rec) = lo.get(id) else { continue };
            // ANAM: parent, previous sibling.
            let anam = rec.get(b"ANAM").unwrap_or(&[]);
            let link = |o: usize| {
                let v = anam
                    .get(o..o + 4)
                    .map_or(0, |d| u32::from_le_bytes(d.try_into().unwrap()));
                if v == 0 {
                    FormId::NULL
                } else {
                    rec.fid(FormId(v))
                }
            };
            let parent = link(0);
            previous.insert(id, link(4));
            let text = |tag: &[u8; 4]| rec.get(tag).map(esp::decode_zstring).unwrap_or_default();
            let conditions = condition::parse_all(&rec);
            for c in conditions.iter().filter(|c| c.func == HAS_KEYWORD) {
                ix.by_keyword.entry(FormId(c.p1)).or_default().push(id);
            }
            if !parent.is_null() {
                ix.children.entry(parent).or_default().push(id);
            }
            ix.idles.insert(
                id,
                Idle {
                    parent,
                    event: text(b"ENAM"),
                    behavior: text(b"DNAM").to_ascii_lowercase(),
                    conditions,
                },
            );
        }
        // Put siblings in their authored order by following the previous-sibling links.
        for kids in ix.children.values_mut() {
            let mut ordered: Vec<FormId> = Vec::with_capacity(kids.len());
            let mut cur = FormId::NULL;
            while let Some(&next) = kids.iter().find(|k| {
                previous.get(k).copied().unwrap_or_default() == cur && !ordered.contains(k)
            }) {
                ordered.push(next);
                cur = next;
            }
            // Broken chains (overrides that moved records) keep load order.
            let rest: Vec<FormId> = kids
                .iter()
                .filter(|k| !ordered.contains(k))
                .copied()
                .collect();
            ordered.extend(rest);
            *kids = ordered;
        }
        ix.locomotion_roots = LocomotionActions::NAMES.map(|name| ix.find(lo, name));
        ix
    }

    /// The item an idle's own `GetItemCount` condition asks its subject about
    /// (firewood for a wood pile's pick-up and put-down).
    pub fn counted_item(&self, idle: FormId) -> Option<FormId> {
        self.idles
            .get(&idle)?
            .conditions
            .iter()
            .find(|c| c.func == GET_ITEM_COUNT && c.run_on == 0)
            .map(|c| FormId(c.p1))
    }

    /// An idle, or an action (`AACT`, e.g. `ActionIdle`) whose idles hang off it.
    pub fn find(&self, lo: &LoadOrder, edid: &str) -> Option<FormId> {
        lo.find_editor_id(edid)
            .filter(|id| self.idles.contains_key(id) || self.children.contains_key(id))
    }

    /// Pick an idle under `root` as the game does: the first child (in authored
    /// order) whose conditions pass; a group descends into its children, falling
    /// back to its own event if none of them pass. `root`'s own conditions are not
    /// checked. Returns the idle and its event.
    pub fn select(
        &self,
        e: &crate::engine::Engine,
        root: FormId,
        ctx: condition::Context,
    ) -> Option<(FormId, String)> {
        self.select_in(
            root,
            &is_humanoid_graph,
            &|id, c| Self::evaluate(e, id, c, ctx),
            0,
        )
    }

    /// Like [`IdleIndex::select`] for an actor of `project`: idles of its graphs.
    pub fn select_for(
        &self,
        e: &crate::engine::Engine,
        root: FormId,
        ctx: condition::Context,
        project: &ProjectRuntime,
    ) -> Option<(FormId, String)> {
        self.select_in(
            root,
            &|b: &str| b.is_empty() || project.plays(b),
            &|id, c| Self::evaluate(e, id, c, ctx),
            0,
        )
    }

    pub(crate) fn locomotion_root(&self, name: &str) -> Option<FormId> {
        self.locomotion_roots[LocomotionActions::NAMES.iter().position(|n| *n == name)?]
    }

    /// Cache each project's applicable branches, but evaluate their conditions
    /// against live state. Both cached refusals and events avoid string copies.
    pub fn action(
        &self,
        e: &crate::engine::Engine,
        root: FormId,
        ctx: condition::Context,
        project: &ProjectRuntime,
    ) -> Option<Arc<str>> {
        self.action_in(root, project, &|id, c| Self::evaluate(e, id, c, ctx))
    }

    fn action_in(
        &self,
        root: FormId,
        project: &ProjectRuntime,
        evaluate: &dyn Fn(FormId, &[Condition]) -> bool,
    ) -> Option<Arc<str>> {
        let cached = self.actions.borrow();
        if let Some(tree) = cached
            .get(project.dir.as_str())
            .and_then(|names| names.get(project.name.as_str()))
            .and_then(|roots| roots.get(&root))
        {
            return self.select_action(tree, evaluate);
        }
        drop(cached);
        let graphs = |b: &str| b.is_empty() || project.plays(b);
        let branches = self.action_branches(root, &graphs, 0);
        let tree = if self.unconditional(root, &graphs, 0) {
            ActionTree::Fixed(self.select_branches(&branches, &|_, _| true))
        } else {
            ActionTree::Conditional(branches)
        };
        let event = self.select_action(&tree, evaluate);
        self.actions
            .borrow_mut()
            .entry(project.dir.clone())
            .or_default()
            .entry(project.name.clone())
            .or_default()
            .insert(root, tree);
        event
    }

    fn action_branches(
        &self,
        root: FormId,
        graphs: &dyn Fn(&str) -> bool,
        depth: u32,
    ) -> Vec<ActionBranch> {
        if depth > 16 {
            return Vec::new();
        }
        self.children
            .get(&root)
            .into_iter()
            .flatten()
            .filter_map(|&id| {
                let idle = self.idles.get(&id)?;
                graphs(self.behavior_of(id)).then(|| ActionBranch {
                    idle: id,
                    event: (!idle.event.is_empty()).then(|| Arc::from(idle.event.as_str())),
                    children: self.action_branches(id, graphs, depth + 1),
                })
            })
            .collect()
    }

    fn select_action(
        &self,
        tree: &ActionTree,
        evaluate: &dyn Fn(FormId, &[Condition]) -> bool,
    ) -> Option<Arc<str>> {
        match tree {
            ActionTree::Fixed(event) => event.clone(),
            ActionTree::Conditional(branches) => self.select_branches(branches, evaluate),
        }
    }

    fn select_branches(
        &self,
        branches: &[ActionBranch],
        evaluate: &dyn Fn(FormId, &[Condition]) -> bool,
    ) -> Option<Arc<str>> {
        for branch in branches {
            let idle = self.idles.get(&branch.idle)?;
            if !idle.conditions.is_empty() && !evaluate(branch.idle, &idle.conditions) {
                continue;
            }
            if let Some(event) = self
                .select_branches(&branch.children, evaluate)
                .or_else(|| branch.event.clone())
            {
                return Some(event);
            }
        }
        None
    }

    fn evaluate(
        e: &crate::engine::Engine,
        id: FormId,
        conditions: &[Condition],
        ctx: condition::Context,
    ) -> bool {
        let passed = condition::evaluate(e, conditions, ctx);
        if !passed && log::log_enabled!(log::Level::Trace) {
            log::trace!(
                "idle {id} fails: {}",
                condition::explain(e, conditions, ctx)
            );
        }
        passed
    }

    fn unconditional(&self, root: FormId, graphs: &dyn Fn(&str) -> bool, depth: u32) -> bool {
        depth <= 16
            && self.children.get(&root).into_iter().flatten().all(|&id| {
                !graphs(self.behavior_of(id))
                    || self.idles.get(&id).is_some_and(|i| {
                        i.conditions.is_empty() && self.unconditional(id, graphs, depth + 1)
                    })
            })
    }

    /// Like [`IdleIndex::select`] over `roots` as if they were siblings, checking
    /// their own conditions (e.g. `EatingRoot` then `DrinkingRoot`).
    pub fn select_among(
        &self,
        e: &crate::engine::Engine,
        roots: &[FormId],
        ctx: condition::Context,
    ) -> Option<(FormId, String)> {
        self.select_list(
            roots,
            &is_humanoid_graph,
            &|id, c| Self::evaluate(e, id, c, ctx),
            0,
        )
    }

    fn select_in(
        &self,
        root: FormId,
        graphs: &dyn Fn(&str) -> bool,
        evaluate: &dyn Fn(FormId, &[Condition]) -> bool,
        depth: u32,
    ) -> Option<(FormId, String)> {
        self.select_list(
            self.children.get(&root).map_or(&[], Vec::as_slice),
            graphs,
            evaluate,
            depth,
        )
    }

    fn select_list(
        &self,
        kids: &[FormId],
        graphs: &dyn Fn(&str) -> bool,
        evaluate: &dyn Fn(FormId, &[Condition]) -> bool,
        depth: u32,
    ) -> Option<(FormId, String)> {
        if depth > 16 {
            return None;
        }
        for &kid in kids {
            let Some(idle) = self.idles.get(&kid) else {
                continue;
            };
            if !graphs(self.behavior_of(kid)) {
                continue;
            }
            if !evaluate(kid, &idle.conditions) {
                continue;
            }
            if let Some(found) = self.select_in(kid, graphs, evaluate, depth + 1) {
                return Some(found);
            }
            if !idle.event.is_empty() {
                return Some((kid, idle.event.clone()));
            }
        }
        None
    }

    fn behavior_of(&self, mut id: FormId) -> &str {
        for _ in 0..32 {
            let Some(i) = self.idles.get(&id) else { break };
            if !i.behavior.is_empty() {
                return &i.behavior;
            }
            id = i.parent;
        }
        ""
    }

    fn humanoid(&self, id: FormId) -> bool {
        is_humanoid_graph(self.behavior_of(id))
    }

    /// The event a (humanoid) idle plays.
    pub fn humanoid_event(&self, id: FormId) -> Option<String> {
        let i = self.idles.get(&id)?;
        (!i.event.is_empty() && self.humanoid(id)).then(|| i.event.clone())
    }

    /// The enter event for furniture `base`: the first plain (not quick / instant,
    /// not exit) child of a humanoid idle conditioned on one of its keywords.
    pub fn furniture_event(&self, lo: &LoadOrder, base: &esp::LoadedRecord<'_>) -> Option<String> {
        let keywords = base.get(b"KWDA").unwrap_or(&[]);
        for c in keywords.chunks_exact(4) {
            let kw = base.fid(FormId(u32::from_le_bytes(c.try_into().unwrap())));
            // Only the animation-type keywords (isBlacksmithForge, isBarStool...) pick
            // idles; generic ones like FurnitureSpecial gate other trees. Tables are
            // ordinary seats whose idles come from the chair tree (EnterTable).
            let edid = lo.get(kw).and_then(|k| k.editor_id()).unwrap_or_default();
            if !edid.get(..2).is_some_and(|p| p.eq_ignore_ascii_case("is"))
                || edid.eq_ignore_ascii_case("IsTable")
            {
                continue;
            }
            for &root in self.by_keyword.get(&kw).into_iter().flatten() {
                if !self.humanoid(root) {
                    continue;
                }
                let kids = self.children.get(&root).map(Vec::as_slice).unwrap_or(&[]);
                let enter = kids.iter().filter_map(|k| self.idles.get(k)).find(|i| {
                    let e = i.event.to_ascii_lowercase();
                    !e.is_empty()
                        && !e.contains("instant")
                        && !e.contains("player")
                        && !e.contains("child")
                        && !e.contains("exit")
                        && !EXIT_EVENTS.contains(&e.as_str())
                        // IsEnteringInteractionQuick == 1 marks the quick variant.
                        && !i.conditions.iter().any(|c| c.func == 631 && c.value != 0.0)
                });
                if let Some(i) = enter {
                    log::trace!(
                        "furniture {}: keyword {} -> idle event {}",
                        base.editor_id().unwrap_or_default(),
                        lo.get(kw).and_then(|k| k.editor_id()).unwrap_or_default(),
                        i.event
                    );
                    return Some(i.event.clone());
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actions_select_project_conditions_and_refusal() {
        let mut ix = IdleIndex::default();
        let root = FormId(1);
        for (id, behavior, event, conditions) in [
            (2, "fish", "SwimLeft", vec![]),
            (
                3,
                "human",
                "turnLeft",
                vec![Condition {
                    op: 0,
                    or: false,
                    use_global: false,
                    swap: false,
                    value: 0.0,
                    global: FormId::NULL,
                    func: 286,
                    p1: 0,
                    p2: 0,
                    run_on: 0,
                    reference: FormId::NULL,
                    reference_raw: 0,
                    string_p1: None,
                    string_p2: None,
                    pack_input: None,
                    p1_input: false,
                    package_data: None,
                    p1_alias: false,
                    p1_ref: None,
                    event_member: [0; 2],
                }],
            ),
        ] {
            ix.children.entry(root).or_default().push(FormId(id));
            ix.idles.insert(
                FormId(id),
                Idle {
                    parent: root,
                    event: event.into(),
                    behavior: behavior.into(),
                    conditions,
                },
            );
        }
        let fish = |b: &str| b == "fish";
        let human = |b: &str| b == "human";
        assert_eq!(
            ix.select_in(root, &fish, &|_, _| true, 0).unwrap().1,
            "SwimLeft"
        );
        assert!(ix.unconditional(root, &fish, 0));
        assert!(!ix.unconditional(root, &human, 0));
        assert!(
            ix.select_in(root, &human, &|_, c| c.is_empty(), 0)
                .is_none()
        );
        assert_eq!(
            ix.select_in(root, &human, &|_, _| true, 0).unwrap().1,
            "turnLeft"
        );
        assert!(ix.select_in(root, &|_| false, &|_, _| true, 0).is_none());

        use havok::behavior::{Project, runtime::Shared};
        let project = |name: &str, file: &str| ProjectRuntime {
            shared: Shared::new(Arc::new(Project {
                graphs: vec![],
                character: None,
            })),
            dir: "same directory".into(),
            name: name.into(),
            files: vec![file.into()],
        };
        let human = project("human", "human");
        let fish = project("fish", "fish");
        let fixed = ix.action_in(root, &fish, &|_, _| panic!("unconditional action"));
        assert_eq!(fixed.as_deref(), Some("SwimLeft"));
        let cached = ix.action_in(root, &fish, &|_, _| panic!("cached action"));
        assert!(Arc::ptr_eq(
            fixed.as_ref().unwrap(),
            cached.as_ref().unwrap()
        ));
        // A cached project tree must still react to live condition changes,
        // including a refusal followed by success and another refusal.
        for allowed in [false, true, false, true] {
            let event = ix.action_in(root, &human, &|_, _| allowed);
            assert_eq!(event.as_deref(), allowed.then_some("turnLeft"));
        }
        let other = project("other", "unsupported");
        assert!(
            ix.action_in(root, &other, &|_, _| panic!("refused project"))
                .is_none()
        );
        assert!(
            ix.action_in(root, &other, &|_, _| panic!("cached refusal"))
                .is_none()
        );
    }
}
