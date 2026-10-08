//! The Story Manager: events (an NPC striking up a conversation, the player
//! changing location, a kill, a script's `SendStoryEvent`...) are run down the
//! tree of nodes under their event type (`SMEN` -> `SMBN` / `SMQN`), in order
//! or at random, each node's conditions checked against the event's data; the
//! first quest node with a quest that starts takes the event, unless it shares
//! it. Quests started this way keep the event: "from event" aliases are filled
//! with its references and locations, and `GetEventData` / run-on-event-data
//! conditions read it.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use esp::FormId;
use esp::story::{Node, NodeKind, flags};

use crate::condition::{self, Condition};
use crate::engine::{Engine, PLAYER_REF};

/// What an event carries: references (`R1`, `R2`), locations (`L1`, `L2`), a
/// keyword (`K1`: a script event's), values (`V1`, `V2`) and a form (`F1`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StoryEvent {
    pub code: [u8; 4],
    pub refs: [FormId; 2],
    pub locs: [FormId; 2],
    pub keyword: FormId,
    pub values: [f32; 2],
    pub form: FormId,
}

impl StoryEvent {
    pub fn new(code: &[u8; 4]) -> Self {
        StoryEvent {
            code: *code,
            ..Default::default()
        }
    }

    /// The form a member names (`R1`, `L2`, `K1`, `F1`), if set.
    pub fn member_form(&self, m: [u8; 2]) -> Option<FormId> {
        let f = match &m {
            b"R1" => self.refs[0],
            b"R2" => self.refs[1],
            b"L1" => self.locs[0],
            b"L2" => self.locs[1],
            b"K1" => self.keyword,
            b"F1" => self.form,
            _ => FormId::NULL,
        };
        (!f.is_null()).then_some(f)
    }

    pub fn member_value(&self, m: [u8; 2]) -> f32 {
        match &m {
            b"V1" => self.values[0],
            b"V2" => self.values[1],
            _ => self.member_form(m).map_or(0.0, |f| f.0 as f32),
        }
    }

    pub fn name(&self) -> String {
        esp::story::code(&self.code)
    }
}

struct NodeDef {
    node: Node,
    conditions: Vec<Condition>,
}

#[derive(Default)]
pub struct StoryManager {
    built: bool,
    nodes: HashMap<FormId, Arc<NodeDef>>,
    /// Children in their order (each names the sibling before it).
    children: HashMap<FormId, Vec<FormId>>,
    /// Event nodes by event type.
    roots: HashMap<[u8; 4], Vec<FormId>>,
    /// When each quest was last started by the Story Manager (game hours).
    last_run: HashMap<FormId, f64>,
    /// "Do all before repeating": the quests each node has run this round.
    ran: HashMap<FormId, HashSet<FormId>>,
    /// The event being run down the tree (its data for conditions).
    pub active: Option<StoryEvent>,
    /// When each actor next considers striking up a conversation (real seconds).
    next_social: HashMap<FormId, f64>,
    /// The player's location last update (change location events).
    last_location: Option<Option<FormId>>,
    /// Bodies each NPC has found (dead body events), and when to look next.
    found_bodies: HashSet<(FormId, FormId)>,
    next_body_check: f64,
}

/// How far an NPC notices a body, and how often NPCs look (no source: about
/// as far as they notice enemies, once a second). Stands in for detection,
/// which isn't implemented.
const BODY_NOTICE_DISTANCE: f32 = 1000.0;
const BODY_CHECK_INTERVAL: f64 = 1.0;

/// What running a node did.
struct Outcome {
    started: bool,
    /// Started, and the event goes no further.
    consumed: bool,
}

impl Engine {
    fn build_story(&mut self) {
        let sm = &mut self.story;
        if sm.built {
            return;
        }
        sm.built = true;
        for tag in [b"SMEN", b"SMBN", b"SMQN"] {
            for &id in self.lo.ids_of_type(tag) {
                let Some(rec) = self.lo.get(id) else { continue };
                let Some(node) = esp::story::parse(&rec, id) else {
                    continue;
                };
                let conditions = node
                    .conditions
                    .iter()
                    .filter_map(|c| {
                        let mut cond = condition::parse(&rec, &c.ctda)?;
                        cond.string_p1 = c.cis1.as_deref().map(Into::into);
                        cond.string_p2 = c.cis2.as_deref().map(Into::into);
                        Some(cond)
                    })
                    .collect();
                if let Some(e) = node.event() {
                    sm.roots.entry(e).or_default().push(id);
                }
                sm.children.entry(node.parent).or_default().push(id);
                sm.nodes.insert(id, Arc::new(NodeDef { node, conditions }));
            }
        }
        let nodes = &sm.nodes;
        for kids in sm.children.values_mut() {
            let mut ordered = Vec::with_capacity(kids.len());
            let mut prev = FormId::NULL;
            while let Some(i) = kids.iter().position(|k| nodes[k].node.previous == prev) {
                prev = kids.remove(i);
                ordered.push(prev);
            }
            ordered.append(kids);
            *kids = ordered;
        }
        for v in sm.roots.values_mut() {
            v.sort();
        }
        log::info!(
            "story manager: {} nodes, {} event types",
            sm.nodes.len(),
            sm.roots.len()
        );
    }

    /// Run an event down the Story Manager's tree. True if a quest started.
    pub fn send_story_event(&mut self, event: StoryEvent) -> bool {
        self.build_story();
        let roots = self
            .story
            .roots
            .get(&event.code)
            .cloned()
            .unwrap_or_default();
        let outer = self.story.active.replace(event.clone());
        let mut started = false;
        for r in roots {
            let o = self.run_story_node(r, &event);
            started |= o.started;
            if o.consumed {
                break;
            }
        }
        self.story.active = outer;
        if started {
            log::info!("story event {} {:?} started a quest", event.name(), event);
        } else {
            log::trace!("story event {} {:?}: nothing started", event.name(), event);
        }
        started
    }

    fn story_node_passes(&self, def: &NodeDef) -> bool {
        condition::evaluate(
            self,
            &def.conditions,
            condition::Context {
                subject: Some(PLAYER_REF),
                ..Default::default()
            },
        )
    }

    /// Quests of a node and below that are running.
    fn story_running_under(&self, id: FormId) -> usize {
        let Some(def) = self.story.nodes.get(&id) else {
            return 0;
        };
        match &def.node.kind {
            NodeKind::Quest { quests, .. } => quests
                .iter()
                .filter(|q| self.scripts.quests.get(&q.quest).is_some_and(|s| s.running))
                .count(),
            _ => self
                .story
                .children
                .get(&id)
                .map_or(0, |k| k.iter().map(|&c| self.story_running_under(c)).sum()),
        }
    }

    fn run_story_node(&mut self, id: FormId, event: &StoryEvent) -> Outcome {
        let none = Outcome {
            started: false,
            consumed: false,
        };
        let Some(def) = self.story.nodes.get(&id).cloned() else {
            return none;
        };
        let node = &def.node;
        if !self.story_node_passes(&def) {
            return none;
        }
        if node.max_concurrent > 0 && self.story_running_under(id) >= node.max_concurrent as usize {
            return none;
        }
        match &node.kind {
            NodeKind::Event(_) | NodeKind::Branch => {
                let mut kids = self.story.children.get(&id).cloned().unwrap_or_default();
                if node.node_flags & flags::RANDOM != 0 {
                    self.shuffle(&mut kids);
                }
                let mut started = false;
                for k in kids {
                    let o = self.run_story_node(k, event);
                    started |= o.started;
                    if o.consumed {
                        return o;
                    }
                }
                Outcome {
                    started,
                    consumed: false,
                }
            }
            NodeKind::Quest { quests, num_to_run } => {
                let mut order: Vec<usize> = (0..quests.len()).collect();
                if node.node_flags & flags::RANDOM != 0 {
                    self.shuffle(&mut order);
                }
                let want = if node.quest_flags & flags::NUM_QUESTS_TO_RUN != 0 {
                    (*num_to_run).max(1)
                } else {
                    1
                };
                let all_before_repeat = node.quest_flags & flags::DO_ALL_BEFORE_REPEATING != 0;
                if all_before_repeat
                    && quests.iter().all(|q| {
                        self.story
                            .ran
                            .get(&id)
                            .is_some_and(|r| r.contains(&q.quest))
                    })
                {
                    self.story.ran.remove(&id);
                }
                let now = self.game_hours_total();
                let (mut n, mut shares) = (0, node.quest_flags & flags::SHARES_EVENT != 0);
                for i in order {
                    let q = quests[i];
                    if self.scripts.quests.get(&q.quest).is_some_and(|s| s.running) {
                        continue;
                    }
                    if q.reset_hours > 0.0
                        && self
                            .story
                            .last_run
                            .get(&q.quest)
                            .is_some_and(|&t| now - t < q.reset_hours as f64)
                    {
                        continue;
                    }
                    if all_before_repeat
                        && self
                            .story
                            .ran
                            .get(&id)
                            .is_some_and(|r| r.contains(&q.quest))
                    {
                        continue;
                    }
                    if !self.start_quest_with(q.quest, Some(event.clone())) {
                        continue;
                    }
                    log::info!(
                        "story manager: {} starts {} ({})",
                        event.name(),
                        self.lo
                            .get(q.quest)
                            .and_then(|r| r.editor_id())
                            .unwrap_or_default(),
                        node.editor_id
                    );
                    self.story.last_run.insert(q.quest, now);
                    self.story.ran.entry(id).or_default().insert(q.quest);
                    shares |= q.flags & flags::QUEST_SHARES_EVENT != 0;
                    n += 1;
                    if n >= want {
                        break;
                    }
                }
                Outcome {
                    started: n > 0,
                    consumed: n > 0 && !shares,
                }
            }
        }
    }

    fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = (self.rand() % (i as u64 + 1)) as usize;
            v.swap(i, j);
        }
    }

    /// The event a condition reads: the one being run down the tree, else the
    /// one that started the quest.
    pub(crate) fn story_event_for(&self, quest: Option<FormId>) -> Option<&StoryEvent> {
        self.story.active.as_ref().or_else(|| {
            quest
                .and_then(|q| self.scripts.quests.get(&q))
                .and_then(|s| s.event.as_ref())
        })
    }

    /// The Papyrus event a quest started by `e` gets (`Quest.OnStory...`) and
    /// its arguments: the members in the order its parameters name them.
    /// None for `SKIL` (a skill's name, not a member) and unknown types.
    pub(crate) fn story_papyrus_event(
        &self,
        e: &StoryEvent,
    ) -> Option<(&'static str, Vec<papyrus::Value>)> {
        let (name, members): (&str, &[&[u8; 2]]) = match &e.code {
            b"ADCR" => ("OnStoryCrimeGold", &[b"R1", b"R2", b"F1", b"V1", b"V2"]),
            b"ADIA" => ("OnStoryDialogue", &[b"L1", b"R1", b"R2"]),
            b"AFAV" => ("OnStoryActivateActor", &[b"L1", b"R1"]),
            b"AHEL" => ("OnStoryHello", &[b"L1", b"R1", b"R2"]),
            b"AIPL" => ("OnStoryAddToPlayer", &[b"R1", b"R2", b"L1", b"F1", b"V1"]),
            b"ARRT" => ("OnStoryArrest", &[b"R1", b"R2", b"L1", b"V1"]),
            b"ASSU" => ("OnStoryAssaultActor", &[b"R1", b"R2", b"L1", b"V1"]),
            b"BRIB" => ("OnStoryBribeNPC", &[b"R1"]),
            b"CAST" => ("OnStoryCastMagic", &[b"R1", b"R2", b"L1", b"F1"]),
            b"CHRR" => ("OnStoryRelationshipChange", &[b"R1", b"R2", b"V1", b"V2"]),
            b"CLOC" => ("OnStoryChangeLocation", &[b"R1", b"L1", b"L2"]),
            b"CRFT" => ("OnStoryCraftItem", &[b"R1", b"L1", b"F1"]),
            b"CURE" => ("OnStoryCure", &[b"F1"]),
            b"DEAD" => ("OnStoryDiscoverDeadBody", &[b"R1", b"R2", b"L1"]),
            b"ESJA" => ("OnStoryEscapeJail", &[b"L1", b"F1"]),
            b"FLAT" => ("OnStoryFlatterNPC", &[b"R1"]),
            b"INFC" => ("OnStoryInfection", &[b"R1", b"F1"]),
            b"INTM" => ("OnStoryIntimidateNPC", &[b"R1"]),
            b"JAIL" => ("OnStoryJail", &[b"R1", b"F1", b"L1", b"V1"]),
            b"KILL" => ("OnStoryKillActor", &[b"R1", b"R2", b"L1", b"V1", b"V2"]),
            b"LEVL" => ("OnStoryIncreaseLevel", &[b"V1"]),
            b"LOCK" => ("OnStoryPickLock", &[b"R1", b"R2"]),
            b"NVPE" => ("OnStoryNewVoicePower", &[b"R1", b"F1"]),
            b"PFIN" => ("OnStoryPayFine", &[b"R1", b"R2", b"F1", b"V1"]),
            b"PRFV" => ("OnStoryPlayerGetsFavor", &[b"R1"]),
            b"REMP" => (
                "OnStoryRemoveFromPlayer",
                &[b"R1", b"R2", b"L1", b"F1", b"V1"],
            ),
            b"SCPT" => ("OnStoryScript", &[b"K1", b"L1", b"R1", b"R2", b"V1", b"V2"]),
            b"STIJ" => ("OnStoryServedTime", &[b"L1", b"F1", b"V1", b"V2"]),
            b"TRES" => ("OnStoryTrespass", &[b"R1", b"R2", b"L1", b"V1"]),
            _ => return None,
        };
        let args = members
            .iter()
            .map(|&&m| match m[0] {
                b'V' => papyrus::Value::Int(e.member_value(m) as i32),
                _ => e
                    .member_form(m)
                    .map_or(papyrus::Value::None, |f| self.object_value(f)),
            })
            .collect();
        Some((name, args))
    }

    /// Events the world sends by itself (called every frame).
    pub(crate) fn update_story(&mut self) {
        // The player arriving somewhere new.
        let here = self.current_location();
        match self.story.last_location {
            Some(before) if before != here => {
                let mut e = StoryEvent::new(b"CLOC");
                e.refs[0] = PLAYER_REF;
                e.locs = [before.unwrap_or_default(), here.unwrap_or_default()];
                log::debug!("player changes location {before:?} -> {here:?}");
                self.send_story_event(e);
            }
            _ => {}
        }
        self.story.last_location = Some(here);
        if self.ai_enabled {
            self.update_social();
            self.update_found_bodies();
        }
    }

    /// Living NPCs coming upon a body they can see: a dead body event (`DEAD`:
    /// R1 who found it, R2 the body, L1 where), once for each of them.
    fn update_found_bodies(&mut self) {
        let now = self.scripts.real_time;
        if now < self.story.next_body_check {
            return;
        }
        self.story.next_body_check = now + BODY_CHECK_INTERVAL;
        let actors: Vec<(FormId, glam::Vec3, bool, bool)> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .map(|a| {
                (
                    a.ref_id,
                    a.pos,
                    a.dead,
                    a.combat.is_none()
                        && a.bleeding.is_none()
                        && a.graph.as_ref().is_some_and(|g| g.project().humanoid()),
                )
            })
            .collect();
        let mut found = Vec::new();
        for &(body, at, dead, _) in &actors {
            if !dead || self.is_disabled(body) {
                continue;
            }
            for &(finder, from, finder_dead, calm) in &actors {
                if finder_dead
                    || !calm
                    || self.story.found_bodies.contains(&(finder, body))
                    || from.distance(at) > BODY_NOTICE_DISTANCE
                {
                    continue;
                }
                // Nothing in between, eye to body.
                let eye = from + glam::Vec3::Z * 110.0;
                let to = at + glam::Vec3::Z * 20.0 - eye;
                let dist = to.length().max(1.0);
                let seen = match self.physics.raycast_excluding(
                    eye,
                    to / dist,
                    (dist - 20.0).max(0.0),
                    finder,
                ) {
                    Some((_, owner)) => owner == Some(body),
                    None => true,
                };
                if seen {
                    found.push((finder, body));
                }
            }
        }
        for (finder, body) in found {
            self.story.found_bodies.insert((finder, body));
            log::debug!("{finder} finds {body}'s body");
            let mut e = StoryEvent::new(b"DEAD");
            e.refs = [finder, body];
            e.locs[0] = self.ref_current_location(body).unwrap_or_default();
            self.send_story_event(e);
        }
    }

    /// NPCs now and then strike up a conversation with someone near them
    /// (`fAISocialTimerForConversationsMin` / `Max`,
    /// `fAISocialchanceForConversation`, `fAISocialRadiusToTriggerConversation`):
    /// an actor dialogue event, whose quests play the conversation.
    fn update_social(&mut self) {
        let now = self.scripts.real_time;
        let (lo, hi) = (
            crate::ai::combat::gmst_f32(&self.lo, "fAISocialTimerForConversationsMin", 10.0) as f64,
            crate::ai::combat::gmst_f32(&self.lo, "fAISocialTimerForConversationsMax", 30.0) as f64,
        );
        let interior = matches!(self.location, crate::engine::Location::Interior(_));
        let suffix = if interior { "Interior" } else { "" };
        let chance = crate::ai::combat::gmst_f32(
            &self.lo,
            &format!("fAISocialchanceForConversation{suffix}"),
            10.0,
        );
        let radius = crate::ai::combat::gmst_f32(
            &self.lo,
            &format!("fAISocialRadiusToTriggerConversation{suffix}"),
            500.0,
        );
        let talking = self.conversation.as_ref().map(|c| c.npc_ref);
        // Who could talk: loaded humanoids, alive, calm, not in a scene or a conversation.
        let free: Vec<(FormId, glam::Vec3)> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| {
                !a.dead && a.combat.is_none() && a.bleeding.is_none() && a.exiting.is_none()
            })
            .filter(|a| a.graph.as_ref().is_some_and(|g| g.project().humanoid()))
            .filter(|a| talking != Some(a.ref_id))
            .map(|a| (a.ref_id, a.pos))
            .collect();
        let mut due = Vec::new();
        for &(r, _) in &free {
            let first = now + lo + (r.0 % 17) as f64;
            let at = *self.story.next_social.entry(r).or_insert(first);
            if now >= at {
                due.push(r);
            }
        }
        for r in due {
            let wait = lo + (self.rand() % 1000) as f64 / 1000.0 * (hi - lo).max(0.0);
            self.story.next_social.insert(r, now + wait);
            if (self.rand() % 100) as f32 >= chance
                || self.scene_of_actor(r).is_some()
                || self.is_barking(r)
            {
                continue;
            }
            let Some(&(_, pos)) = free.iter().find(|(x, _)| *x == r) else {
                continue;
            };
            let mut near: Vec<(f32, FormId)> = free
                .iter()
                .filter(|(o, _)| *o != r)
                .map(|(o, p)| (p.distance(pos), *o))
                .filter(|(d, o)| *d <= radius && self.scene_of_actor(*o).is_none())
                .collect();
            near.sort_by(|a, b| a.0.total_cmp(&b.0));
            let Some(&(_, other)) = near.first() else {
                continue;
            };
            let mut e = StoryEvent::new(b"ADIA");
            e.refs = [r, other];
            e.locs[0] = self.ref_current_location(r).unwrap_or_default();
            log::debug!("{r} strikes up a conversation with {other}");
            self.send_story_event(e);
        }
        // Forget actors no longer loaded.
        let loaded = &self.actor_cells;
        self.story.next_social.retain(|r, _| loaded.contains_key(r));
    }

    /// The player taking an item: from the world (`source` the reference picked
    /// up) or from a container or body (`container`). A player add item event
    /// (`AIPL`): R1 the owner's reference, R2 the container, L1 where, F1 the
    /// item, V1 how it was acquired (1 stolen, 4 picked up, 5 from a
    /// container, 6 from a body). Taking `count` of something stolen is a
    /// crime, at the item's value.
    pub(crate) fn send_player_add_item(
        &mut self,
        item: FormId,
        count: i32,
        source: FormId,
        container: bool,
    ) {
        let body = container && self.is_body(source);
        let owner = self.source_owner(source, container);
        let how = match () {
            _ if owner.is_some_and(|o| self.is_stealing(item, o)) => 1,
            _ if body => 6,
            _ if container => 5,
            _ => 4,
        };
        let owner_ref = owner
            .filter(|&o| o != FormId(0x7))
            .and_then(|o| self.npc_refs_index().get(&o).copied());
        if how == 1 {
            let value = crate::world::inventory::item_info(&self.lo, item).map_or(0, |i| i.value)
                * count.max(1);
            let faction = owner.filter(|&o| self.lo.tag_of(o).is_some_and(|t| t.0 == *b"FACT"));
            let seen = self.commit_crime(crate::crime::CrimeType::Steal, owner_ref, faction, value);
            if let Some(f) = owner_ref.and_then(|o| self.crime_faction(o)).or(faction) {
                self.add_stolen_value(f, value, seen);
            }
        }
        self.send_player_add_item_as(
            item,
            owner_ref,
            if container { source } else { FormId::NULL },
            how,
        );
    }

    /// A player add item event (`AIPL`, see `send_player_add_item`) with the
    /// way it was acquired given (3: pickpocketed).
    pub(crate) fn send_player_add_item_as(
        &mut self,
        item: FormId,
        owner_ref: Option<FormId>,
        container: FormId,
        how: i32,
    ) {
        let mut e = StoryEvent::new(b"AIPL");
        e.refs = [owner_ref.unwrap_or_default(), container];
        e.locs[0] = self.current_location().unwrap_or_default();
        e.form = item;
        e.values[0] = how as f32;
        self.send_story_event(e);
    }

    /// Whether a container is a body (bodies belong to nobody, not to the
    /// house they lie in).
    pub(crate) fn is_body(&self, r: FormId) -> bool {
        self.actor_cells.contains_key(&r) || self.lo.tag_of(r).is_some_and(|t| t.0 == *b"ACHR")
    }

    /// Who owns what the player takes from `source` (a reference picked up, or
    /// a container or body).
    pub(crate) fn source_owner(&self, source: FormId, container: bool) -> Option<FormId> {
        crate::ai::furniture::owner_of(&self.lo, source)
            .filter(|_| !(container && self.is_body(source)))
    }

    /// Whom `item` taken from `source` is stolen from, if it is stealing.
    pub(crate) fn stolen_from(
        &self,
        item: FormId,
        source: FormId,
        container: bool,
    ) -> Option<FormId> {
        self.source_owner(source, container)
            .filter(|&o| self.is_stealing(item, o))
    }

    /// An actor greeting another (`AHEL`: R1 who says hello, R2 to whom, L1
    /// where), sent before the greeting is picked so the quests it starts can
    /// supply it.
    pub(crate) fn send_actor_hello(&mut self, actor: FormId, to: FormId) {
        let mut e = StoryEvent::new(b"AHEL");
        e.refs = [actor, to];
        e.locs[0] = self.ref_current_location(actor).unwrap_or_default();
        self.send_story_event(e);
    }

    /// The first blow of a fight: an assault event (`ASSU`: R1 the victim, R2
    /// the attacker, L1 where, V1 whether it is a crime: the victim keeps the
    /// law and wasn't hostile). The player's crimes are reported.
    pub(crate) fn send_assault(&mut self, victim: FormId, attacker: FormId, crime: bool) {
        if crime && attacker == crate::engine::PLAYER_REF {
            self.player_assault(victim);
        }
        let mut e = StoryEvent::new(b"ASSU");
        e.refs = [victim, attacker];
        e.locs[0] = self.ref_current_location(victim).unwrap_or_default();
        e.values[0] = crime as u8 as f32;
        self.send_story_event(e);
    }

    /// A kill (`KILL`: R1 the victim, R2 the killer, L1 where, V1 the crime
    /// status: 0 none, 1 an unreported murder, 2 a reported one; V2 the
    /// relationship rank between them before).
    pub(crate) fn send_kill_event(
        &mut self,
        victim: FormId,
        killer: Option<FormId>,
        crime: i32,
        rank: i32,
    ) {
        let mut e = StoryEvent::new(b"KILL");
        e.refs = [victim, killer.unwrap_or_default()];
        e.locs[0] = self.ref_current_location(victim).unwrap_or_default();
        e.values = [crime as f32, rank as f32];
        self.send_story_event(e);
    }

    /// Crime gold added for a crime the player committed (`ADCR`: R1 the
    /// victim, R2 the criminal, F1 the faction, V1 the gold, V2 the crime:
    /// 0 steal, 1 pickpocket, 2 trespass, 3 attack, 4 murder, 5 escape).
    pub(crate) fn send_crime_gold_event(
        &mut self,
        victim: Option<FormId>,
        faction: FormId,
        gold: i32,
        crime: crate::crime::CrimeType,
    ) {
        let mut e = StoryEvent::new(b"ADCR");
        e.refs = [victim.unwrap_or_default(), PLAYER_REF];
        e.form = faction;
        e.values = [gold as f32, crime as i32 as f32];
        self.send_story_event(e);
    }

    /// A guard arrests the player (`ARRT`: R1 the guard, R2 the criminal, L1
    /// where, V1 the crime as for `ADCR`, -1 none). `faction` isn't event
    /// data; it is logged.
    pub(crate) fn send_arrest_event(&mut self, guard: FormId, faction: FormId, crime: i32) {
        log::info!("{guard} arrests the player for {faction} (crime {crime})");
        let mut e = StoryEvent::new(b"ARRT");
        e.refs = [guard, PLAYER_REF];
        e.locs[0] = self.ref_current_location(PLAYER_REF).unwrap_or_default();
        e.values[0] = crime as f32;
        self.send_story_event(e);
    }

    /// The player pays a bounty (`PFIN`: R1 the criminal, R2 the guard, F1
    /// the crime group, V1 the crime gold).
    pub(crate) fn send_pay_fine_event(
        &mut self,
        guard: Option<FormId>,
        faction: FormId,
        gold: i32,
    ) {
        let mut e = StoryEvent::new(b"PFIN");
        e.refs = [PLAYER_REF, guard.unwrap_or_default()];
        e.form = faction;
        e.values[0] = gold as f32;
        self.send_story_event(e);
    }

    /// The player is put in jail (`JAIL`: R1 the guard, F1 the crime group,
    /// L1 the jail's location, V1 the crime gold it is for).
    pub(crate) fn send_jail_event(
        &mut self,
        guard: Option<FormId>,
        faction: FormId,
        jail: Option<FormId>,
        gold: i32,
    ) {
        let mut e = StoryEvent::new(b"JAIL");
        e.refs[0] = guard.unwrap_or_default();
        e.form = faction;
        e.locs[0] = jail.unwrap_or_default();
        e.values[0] = gold as f32;
        self.send_story_event(e);
    }

    /// The player escapes jail (`ESJA`: L1 the jail's location, F1 the crime
    /// group).
    pub(crate) fn send_escape_jail_event(&mut self, faction: FormId, jail: Option<FormId>) {
        let mut e = StoryEvent::new(b"ESJA");
        e.form = faction;
        e.locs[0] = jail.unwrap_or_default();
        self.send_story_event(e);
    }

    /// The player served their sentence (`STIJ`: L1 the jail's location, F1
    /// the crime group, V1 the crime gold, V2 the days).
    pub(crate) fn send_served_time_event(
        &mut self,
        faction: FormId,
        jail: Option<FormId>,
        gold: i32,
        days: i32,
    ) {
        let mut e = StoryEvent::new(b"STIJ");
        e.form = faction;
        e.locs[0] = jail.unwrap_or_default();
        e.values = [gold as f32, days as f32];
        self.send_story_event(e);
    }
}
