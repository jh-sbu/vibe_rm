//! The IDLE record tree: which behaviour event an idle plays, which idles belong
//! to furniture with a given keyword, and picking an idle by walking a subtree.

use std::collections::HashMap;

use esp::{FormId, LoadOrder};

use crate::condition::{self, Condition};
use crate::world::behavior::ProjectRuntime;

/// Idles of the humanoid graphs (or inheriting no graph).
fn is_humanoid_graph(behavior: &str) -> bool {
    behavior.is_empty() || behavior.starts_with("actors\\character\\")
}

/// Condition function `HasKeyword`.
const HAS_KEYWORD: u16 = 560;
/// Events that leave furniture rather than enter it.
const EXIT_EVENTS: [&str; 3] = ["idlechairexitstart", "idlefurnitureexit", "idlestop"];

struct Idle {
    parent: FormId,
    event: String,
    /// Behaviour file, lowercase (empty: inherited from the parent).
    behavior: String,
    conditions: Vec<Condition>,
}

#[derive(Default)]
pub struct IdleIndex {
    idles: HashMap<FormId, Idle>,
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
        ix
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
        self.select_in(e, root, ctx, &is_humanoid_graph, 0)
    }

    /// Like [`IdleIndex::select`] for an actor of `project`: idles of its graphs.
    pub fn select_for(
        &self,
        e: &crate::engine::Engine,
        root: FormId,
        ctx: condition::Context,
        project: &ProjectRuntime,
    ) -> Option<(FormId, String)> {
        self.select_in(e, root, ctx, &|b: &str| b.is_empty() || project.plays(b), 0)
    }

    /// Like [`IdleIndex::select`] over `roots` as if they were siblings, checking
    /// their own conditions (e.g. `EatingRoot` then `DrinkingRoot`).
    pub fn select_among(
        &self,
        e: &crate::engine::Engine,
        roots: &[FormId],
        ctx: condition::Context,
    ) -> Option<(FormId, String)> {
        self.select_list(e, roots, ctx, &is_humanoid_graph, 0)
    }

    fn select_in(
        &self,
        e: &crate::engine::Engine,
        root: FormId,
        ctx: condition::Context,
        graphs: &dyn Fn(&str) -> bool,
        depth: u32,
    ) -> Option<(FormId, String)> {
        self.select_list(
            e,
            self.children.get(&root).map_or(&[], Vec::as_slice),
            ctx,
            graphs,
            depth,
        )
    }

    fn select_list(
        &self,
        e: &crate::engine::Engine,
        kids: &[FormId],
        ctx: condition::Context,
        graphs: &dyn Fn(&str) -> bool,
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
            if !condition::evaluate(e, &idle.conditions, ctx) {
                if log::log_enabled!(log::Level::Trace) {
                    log::trace!(
                        "idle {kid} fails: {}",
                        condition::explain(e, &idle.conditions, ctx)
                    );
                }
                continue;
            }
            if let Some(found) = self.select_in(e, kid, ctx, graphs, depth + 1) {
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
