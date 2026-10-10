//! Perks: what actors have, and the entry points that change the game's numbers.
//!
//! A `PERK` lists sections (`PRKE` .. `PRKF`): a quest stage set when the perk
//! is added, an ability, or an entry point. An entry point section names one of
//! the game's hooks (`BGSEntryPoint::ENTRY_POINT` in CommonLibSSE: Mod Attack
//! Damage, Mod Sneak Attack Mult, Mod Lockpick Sweet Spot...), a function
//! (set, add, multiply, multiply by 1 + an actor value x a factor...) with its
//! value, and conditions on tabs: tab 0 runs on the perk's owner, the others on
//! what the entry point is about (the weapon, the target, the lock...). Where
//! the engine reaches a hook it passes the number through every matching
//! entry of the actor's perks, lowest priority first.
//!
//! Actors' perks are their NPC record's (`PRKR`, a template's when it gives the
//! spell list) with those scripts and the console added or removed. The player
//! buys perks from the skills' perk trees (`AVIF` nodes) with perk points; a
//! perk's own conditions are its requirements. Open questions:
//! `known_gaps/perks.md`.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use esp::FormId;

use crate::condition::{self, Condition, Context};
use crate::engine::{Engine, PLAYER_REF};

/// Entry points used here (CommonLibSSE `BGSEntryPoint::ENTRY_POINTS`).
pub mod ep {
    pub const MOD_SNEAK_ATTACK_MULT: u8 = 18;
    pub const MOD_SPELL_MAGNITUDE: u8 = 29;
    pub const MOD_INCOMING_SPELL_MAGNITUDE: u8 = 41;
    pub const MOD_POWER_ATTACK_STAMINA: u8 = 27;
    pub const MOD_POWER_ATTACK_DAMAGE: u8 = 28;
    pub const MOD_BASHING_DAMAGE: u8 = 26;
    pub const MOD_ATTACK_DAMAGE: u8 = 35;
    pub const MOD_INCOMING_DAMAGE: u8 = 36;
    pub const MOD_TARGET_DAMAGE_RESISTANCE: u8 = 37;
    pub const MOD_PERCENT_BLOCKED: u8 = 39;
    pub const MOD_DETECTION_SNEAK_SKILL: u8 = 57;
    pub const MOD_DETECTION_LIGHT: u8 = 47;
    pub const MOD_DETECTION_MOVEMENT: u8 = 48;
    pub const MOD_PICKPOCKET_CHANCE: u8 = 56;
    pub const MOD_LOCKPICK_SWEET_SPOT: u8 = 59;
    pub const MAKE_LOCKPICKS_UNBREAKABLE: u8 = 65;
    pub const MOD_ARMOR_RATING: u8 = 85;
}

/// An entry point's value (`EPFT` / `EPFD`).
#[derive(Debug, Clone, Copy)]
pub enum Value {
    None,
    One(f32),
    /// Two floats: an actor value and a factor (the actor value multiplier
    /// functions), or a range (Add Range To Value).
    Two(f32, f32),
    /// A leveled list, spell or text: read, not used yet.
    Other,
}

#[derive(Debug, Clone)]
pub enum Section {
    /// Set `stage` of `quest` when the perk is added.
    Quest {
        quest: FormId,
        stage: u16,
    },
    /// An ability spell (no magic yet).
    Ability(FormId),
    Point(EntryPoint),
}

#[derive(Debug, Clone)]
pub struct EntryPoint {
    pub point: u8,
    pub function: u8,
    /// Conditions by tab (0: the owner).
    pub tabs: Vec<Vec<Condition>>,
    pub value: Value,
}

#[derive(Debug, Clone)]
pub struct Entry {
    /// The perk rank it takes (0: the first).
    pub rank: u8,
    pub priority: u8,
    pub section: Section,
}

/// A `PERK` record as read.
#[derive(Debug, Clone, Default)]
pub struct Perk {
    pub name: String,
    pub description: String,
    pub playable: bool,
    pub hidden: bool,
    /// `DATA` ranks: how many records the chain (`NNAM`) has, shown in the tree.
    pub ranks: u8,
    /// The next rank's perk.
    pub next: Option<FormId>,
    /// What it takes to buy it.
    pub conditions: Vec<Condition>,
    pub entries: Vec<Entry>,
}

impl Perk {
    pub fn load(e: &Engine, id: FormId) -> Option<Perk> {
        let rec = e.lo.get(id).filter(|r| r.tag().0 == *b"PERK")?;
        let mut p = Perk::default();
        // Conditions before the first section are the perk's; after a PRKC,
        // that tab's.
        let mut current: Option<Entry> = None;
        let mut tab: Option<usize> = None;
        let mut param_type = 0u8;
        let fid = |d: &[u8]| rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap())));
        for s in rec.subrecords() {
            let d = s.data;
            match &s.tag.0 {
                b"FULL" => p.name = e.lo.lstring(&rec, d),
                b"DESC" => p.description = e.lo.lstring(&rec, d),
                b"DATA" if current.is_none() && d.len() >= 5 => {
                    p.ranks = d[2];
                    p.playable = d[3] != 0;
                    p.hidden = d[4] != 0;
                }
                b"NNAM" if d.len() >= 4 && current.is_none() => {
                    p.next = Some(fid(d)).filter(|f| !f.is_null())
                }
                b"CTDA" => {
                    let Some(c) = condition::parse(&rec, d) else {
                        continue;
                    };
                    match (&mut current, tab) {
                        (None, _) => p.conditions.push(c),
                        (
                            Some(Entry {
                                section: Section::Point(pt),
                                ..
                            }),
                            Some(t),
                        ) => {
                            if pt.tabs.len() <= t {
                                pt.tabs.resize(t + 1, Vec::new());
                            }
                            pt.tabs[t].push(c);
                        }
                        _ => {}
                    }
                }
                b"CIS1" | b"CIS2" => {
                    let last = match (&mut current, tab) {
                        (None, _) => p.conditions.last_mut(),
                        (
                            Some(Entry {
                                section: Section::Point(pt),
                                ..
                            }),
                            Some(t),
                        ) => pt.tabs.get_mut(t).and_then(|v| v.last_mut()),
                        _ => None,
                    };
                    if let Some(c) = last {
                        let text = Some(s.zstring().into());
                        if s.tag.0 == *b"CIS1" {
                            c.string_p1 = text;
                        } else {
                            c.string_p2 = text;
                        }
                    }
                }
                b"PRKE" if d.len() >= 3 => {
                    current = Some(Entry {
                        rank: d[1],
                        priority: d[2],
                        section: match d[0] {
                            0 => Section::Quest {
                                quest: FormId::NULL,
                                stage: 0,
                            },
                            1 => Section::Ability(FormId::NULL),
                            _ => Section::Point(EntryPoint {
                                point: 0,
                                function: 0,
                                tabs: Vec::new(),
                                value: Value::None,
                            }),
                        },
                    });
                    tab = None;
                    param_type = 0;
                }
                b"DATA" => match current.as_mut().map(|c| &mut c.section) {
                    Some(Section::Quest { quest, stage }) if d.len() >= 5 => {
                        *quest = fid(d);
                        *stage = d[4] as u16;
                    }
                    Some(Section::Ability(spell)) if d.len() >= 4 => *spell = fid(d),
                    Some(Section::Point(pt)) if d.len() >= 3 => {
                        pt.point = d[0];
                        pt.function = d[1];
                        pt.tabs = vec![Vec::new(); d[2] as usize];
                    }
                    _ => {}
                },
                b"PRKC" if !d.is_empty() => tab = Some(d[0] as usize),
                b"EPFT" if !d.is_empty() => param_type = d[0],
                b"EPFD" => {
                    if let Some(Section::Point(pt)) = current.as_mut().map(|c| &mut c.section) {
                        let f = |o: usize| f32::from_le_bytes(d[o..o + 4].try_into().unwrap());
                        pt.value = match param_type {
                            1 if d.len() >= 4 => Value::One(f(0)),
                            2 if d.len() >= 8 => Value::Two(f(0), f(4)),
                            0 => Value::None,
                            _ => Value::Other,
                        };
                    }
                }
                b"PRKF" => {
                    if let Some(c) = current.take() {
                        p.entries.push(c);
                    }
                    tab = None;
                }
                _ => {}
            }
        }
        Some(p)
    }

    /// The entry points' numbers this perk changes, for the console.
    pub fn describe_entries(&self) -> Vec<String> {
        self.entries
            .iter()
            .map(|en| match &en.section {
                Section::Quest { quest, stage } => format!("  quest {quest} stage {stage}"),
                Section::Ability(s) => format!("  ability {s} (no magic yet)"),
                Section::Point(pt) => format!(
                    "  entry point {} function {} {:?}, {} conditions{}",
                    pt.point,
                    pt.function,
                    pt.value,
                    pt.tabs.iter().map(Vec::len).sum::<usize>(),
                    if en.rank > 0 {
                        format!(", rank {}", en.rank + 1)
                    } else {
                        String::new()
                    }
                ),
            })
            .collect()
    }
}

/// A node of a skill's perk tree (`AVIF`): its perk (the first rank), where
/// it sits (grid cell plus offset) and the nodes it leads to.
#[derive(Debug, Clone)]
pub struct TreeNode {
    pub index: u32,
    pub perk: FormId,
    pub x: f32,
    pub y: f32,
    pub children: Vec<u32>,
}

/// Who has which perks beyond their records', and what was read.
#[derive(Debug, Default)]
pub struct Perks {
    /// Perks added (with their rank, 1 for the first) by scripts, the console
    /// or the perk trees.
    pub added: HashMap<FormId, Vec<(FormId, u8)>>,
    /// Record perks taken away.
    pub removed: HashMap<FormId, HashSet<FormId>>,
    /// The skill whose perk tree the skills menu shows.
    pub tree: Option<u32>,
    cache: std::cell::RefCell<HashMap<FormId, Option<Arc<Perk>>>>,
    owned: std::cell::RefCell<HashMap<FormId, Arc<Vec<(FormId, u8)>>>>,
}

impl Engine {
    /// A perk as read (cached).
    pub fn perk(&self, id: FormId) -> Option<Arc<Perk>> {
        if let Some(p) = self.perks.cache.borrow().get(&id) {
            return p.clone();
        }
        let p = Perk::load(self, id).map(Arc::new);
        self.perks.cache.borrow_mut().insert(id, p.clone());
        p
    }

    /// An actor's perks and their ranks.
    pub fn actor_perks(&self, actor: FormId) -> Arc<Vec<(FormId, u8)>> {
        if let Some(v) = self.perks.owned.borrow().get(&actor) {
            return v.clone();
        }
        let mut v: Vec<(FormId, u8)> = Vec::new();
        let removed = self.perks.removed.get(&actor);
        if let Some(rec) = self
            .templates_of(actor)
            .and_then(|t| t.record(&self.lo, crate::world::template::SPELLS, b"PRKR"))
        {
            for s in rec
                .subrecords()
                .filter(|s| s.tag.0 == *b"PRKR" && s.data.len() >= 5)
            {
                let id = rec.fid(FormId(s.u32(0)));
                if !removed.is_some_and(|r| r.contains(&id)) {
                    v.push((id, s.data[4].max(1)));
                }
            }
        }
        for &(id, rank) in self.perks.added.get(&actor).into_iter().flatten() {
            match v.iter_mut().find(|p| p.0 == id) {
                Some(p) => p.1 = p.1.max(rank),
                None => v.push((id, rank)),
            }
        }
        let v = Arc::new(v);
        self.perks.owned.borrow_mut().insert(actor, v.clone());
        v
    }

    pub fn has_perk(&self, actor: FormId, perk: FormId) -> bool {
        self.actor_perks(actor).iter().any(|p| p.0 == perk)
    }

    /// `AddPerk`: the perk joins the actor's (setting its quest stages).
    pub fn add_perk(&mut self, actor: FormId, perk: FormId) -> bool {
        let Some(p) = self.perk(perk) else {
            return false;
        };
        if self.has_perk(actor, perk) {
            return true;
        }
        if let Some(r) = self.perks.removed.get_mut(&actor) {
            r.remove(&perk);
        }
        self.perks.added.entry(actor).or_default().push((perk, 1));
        self.perks.owned.borrow_mut().remove(&actor);
        for en in &p.entries {
            if let Section::Quest { quest, stage } = en.section {
                self.scripts.pending_stages.push((quest, stage));
            }
        }
        log::info!("{actor} gains perk {} ({perk})", p.name);
        true
    }

    /// `RemovePerk`.
    pub fn remove_perk(&mut self, actor: FormId, perk: FormId) {
        if let Some(v) = self.perks.added.get_mut(&actor) {
            v.retain(|p| p.0 != perk);
        }
        self.perks.removed.entry(actor).or_default().insert(perk);
        self.perks.owned.borrow_mut().remove(&actor);
    }

    /// Pass `value` through entry point `point` of `owner`'s perks: `subjects`
    /// are what tabs 1, 2... run on (`None`: nothing, failing their conditions).
    pub fn perk_entry_point(
        &self,
        point: u8,
        owner: FormId,
        subjects: &[Option<FormId>],
        value: f32,
    ) -> f32 {
        let perks = self.actor_perks(owner);
        if perks.is_empty() {
            return value;
        }
        let mut matching: Vec<(u8, Arc<Perk>, usize)> = Vec::new();
        for &(id, rank) in perks.iter() {
            let Some(p) = self.perk(id) else { continue };
            for (i, en) in p.entries.iter().enumerate() {
                if matches!(&en.section, Section::Point(pt) if pt.point == point) && en.rank < rank
                {
                    matching.push((en.priority, p.clone(), i));
                }
            }
        }
        if matching.is_empty() {
            return value;
        }
        matching.sort_by_key(|m| m.0);
        let mut v = value;
        for (_, p, i) in matching {
            let Section::Point(pt) = &p.entries[i].section else {
                continue;
            };
            let passes = pt.tabs.iter().enumerate().all(|(t, conds)| {
                if conds.is_empty() {
                    return true;
                }
                let subject = if t == 0 {
                    Some(owner)
                } else {
                    subjects.get(t - 1).copied().flatten()
                };
                subject.is_some()
                    && condition::evaluate(
                        self,
                        conds,
                        Context {
                            subject,
                            target: subjects.last().copied().flatten(),
                            ..Default::default()
                        },
                    )
            });
            if !passes {
                continue;
            }
            let before = v;
            v = self.apply_perk_function(pt, owner, v);
            if v != before {
                log::debug!("{owner}: {} entry point {point}: {before} -> {v}", p.name);
            }
        }
        v
    }

    fn apply_perk_function(&self, pt: &EntryPoint, owner: FormId, v: f32) -> f32 {
        let one = match pt.value {
            Value::One(x) => x,
            _ => 0.0,
        };
        // The actor value functions' first float is the actor value's index.
        let av = |a: f32, m: f32| self.actor_value(owner, a as u32) * m;
        match (pt.function, pt.value) {
            (1, _) => one,
            (2, _) => v + one,
            (3, _) => v * one,
            // Add Range To Value: read as (low, high).
            (4, Value::Two(lo, hi)) => {
                v + lo + (hi - lo) * (self.peek_rand() % 10_000) as f32 / 10_000.0
            }
            (5, Value::Two(a, m)) => v + av(a, m),
            (6, _) => v.abs(),
            (7, _) => -v.abs(),
            (12, Value::Two(a, m)) => av(a, m),
            (13, Value::Two(a, m)) => v * av(a, m),
            (14, Value::Two(a, m)) => v * (1.0 + av(a, m)),
            _ => v,
        }
    }

    /// Whether an entry point that sets a flag (Make Lockpicks Unbreakable...)
    /// is on for `owner`.
    pub fn perk_flag(&self, point: u8, owner: FormId, subjects: &[Option<FormId>]) -> bool {
        self.perk_entry_point(point, owner, subjects, 0.0) != 0.0
    }

    /// A skill's perk tree (`AVIF` `0x446 + skill`), its root (no perk) left out.
    pub fn perk_tree(&self, skill: u32) -> Vec<TreeNode> {
        let Some(rec) = self.lo.get(FormId(0x446 + skill)) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut node: Option<TreeNode> = None;
        let mut started = false;
        for s in rec.subrecords() {
            let d = s.data;
            if d.len() < 4 {
                continue;
            }
            let u = u32::from_le_bytes(d[0..4].try_into().unwrap());
            let f = f32::from_bits(u);
            // The nodes start at the first PNAM; each ends with its INAM.
            match &s.tag.0 {
                b"PNAM" => {
                    started = true;
                    node = Some(TreeNode {
                        index: 0,
                        perk: if u == 0 {
                            FormId::NULL
                        } else {
                            rec.fid(FormId(u))
                        },
                        x: 0.0,
                        y: 0.0,
                        children: Vec::new(),
                    });
                }
                _ if !started => {}
                b"XNAM" => {
                    if let Some(n) = node.as_mut() {
                        n.x += u as f32;
                    }
                }
                b"YNAM" => {
                    if let Some(n) = node.as_mut() {
                        n.y += u as f32;
                    }
                }
                b"HNAM" => {
                    if let Some(n) = node.as_mut() {
                        n.x += f;
                    }
                }
                b"VNAM" => {
                    if let Some(n) = node.as_mut() {
                        n.y += f;
                    }
                }
                b"CNAM" => {
                    if let Some(n) = node.as_mut() {
                        n.children.push(u);
                    }
                }
                b"INAM" => {
                    if let Some(mut n) = node.take() {
                        n.index = u;
                        if !n.perk.is_null() {
                            out.push(n);
                        }
                    }
                }
                _ => {}
            }
        }
        out
    }

    /// The perk chain of a tree node: its ranks in order (`NNAM`).
    pub fn perk_ranks(&self, first: FormId) -> Vec<FormId> {
        let mut out = vec![first];
        while let Some(next) = self.perk(*out.last().unwrap()).and_then(|p| p.next) {
            if out.contains(&next) || out.len() > 16 {
                break;
            }
            out.push(next);
        }
        out
    }

    /// The rank of a tree node the player could take next: the first of its
    /// chain they lack, if its conditions pass.
    pub fn next_perk_rank(&self, first: FormId) -> Option<(FormId, bool)> {
        let next = self
            .perk_ranks(first)
            .into_iter()
            .find(|&p| !self.has_perk(PLAYER_REF, p))?;
        let ok = self.perk(next).is_some_and(|p| {
            condition::evaluate(
                self,
                &p.conditions,
                Context {
                    subject: Some(PLAYER_REF),
                    ..Default::default()
                },
            )
        });
        Some((next, ok))
    }

    /// Spend a perk point on `perk` (a node's next rank) if its requirements pass.
    pub fn buy_perk(&mut self, perk: FormId) -> bool {
        if self.skills.perk_points == 0 || self.has_perk(PLAYER_REF, perk) {
            return false;
        }
        let ok = self.perk(perk).is_some_and(|p| {
            condition::evaluate(
                self,
                &p.conditions,
                Context {
                    subject: Some(PLAYER_REF),
                    ..Default::default()
                },
            )
        });
        if !ok {
            return false;
        }
        self.skills.perk_points -= 1;
        self.add_perk(PLAYER_REF, perk)
    }

    /// The console's `perks`: an actor's perks and what they change.
    pub fn describe_perks(&self, actor: FormId, detail: bool) -> Vec<String> {
        let mut out = vec![format!("{actor}: {} perks", self.actor_perks(actor).len())];
        for &(id, rank) in self.actor_perks(actor).iter() {
            let Some(p) = self.perk(id) else {
                out.push(format!("{id} (not a perk)"));
                continue;
            };
            out.push(format!(
                "{id} {}{}",
                if p.name.is_empty() {
                    self.lo
                        .get(id)
                        .and_then(|r| r.editor_id().map(|s| s.to_string()))
                        .unwrap_or_default()
                } else {
                    p.name.clone()
                },
                if rank > 1 {
                    format!(" (rank {rank})")
                } else {
                    String::new()
                }
            ));
            if detail {
                out.extend(p.describe_entries());
            }
        }
        out
    }
}

/// A weapon's skill (`DNAM` at 76: an actor value index).
pub fn weapon_skill(lo: &esp::LoadOrder, weapon: FormId) -> Option<u32> {
    let rec = lo.get(weapon)?;
    let d = rec.get(b"DNAM").filter(|d| d.len() >= 80)?;
    Some(u32::from_le_bytes(d[76..80].try_into().unwrap()))
}

impl Engine {
    /// How many things `actor` wears (or wields) have keyword `kw`.
    pub fn worn_with_keyword(&self, actor: FormId, kw: FormId) -> usize {
        self.inventories.get(&actor).map_or(0, |i| {
            i.equipped
                .iter()
                .filter(|&&f| self.has_keyword(f, kw))
                .count()
        })
    }

    /// Whether the player or an actor holds its guard up.
    pub fn is_blocking(&self, actor: FormId) -> bool {
        if actor == PLAYER_REF {
            self.player_blocking
        } else {
            self.actor_ref(actor).is_some_and(|a| a.guarding())
        }
    }
}
