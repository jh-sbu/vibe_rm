//! Quest aliases filled when a quest starts. Reference aliases: forced
//! references, unique actors, another quest's alias, a location alias's
//! reference of a location ref type, or the first reference meeting the alias's
//! conditions ("find matching reference"). Location aliases: a specific
//! location, a reference alias's location, another quest's alias, or the first
//! location meeting the conditions. Aliases fill in record order, so conditions
//! see the aliases above them; a quest whose required (not "Optional") alias
//! can't be filled doesn't start. Based on the Creation Kit wiki's *Quest Alias
//! Tab* and UESP *Mod File Format/QUST*.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub(crate) type LocationSources = HashMap<(FormId, u32), (Option<FormId>, Option<FormId>)>;

use esp::FormId;

use crate::condition::{self, Condition, Context};
use crate::created::Created;
use crate::engine::{Engine, PLAYER_REF};

/// Alias flags (`FNAM`).
pub mod flags {
    pub const RESERVES: u32 = 0x1;
    pub const OPTIONAL: u32 = 0x2;
    pub const ALLOW_REUSE: u32 = 0x8;
    pub const ALLOW_DEAD: u32 = 0x10;
    pub const IN_LOADED_AREA: u32 = 0x20;
    /// The reference is essential while it fills the alias.
    pub const ESSENTIAL: u32 = 0x40;
    pub const ALLOW_DISABLED: u32 = 0x80;
    pub const ALLOW_RESERVED: u32 = 0x200;
    /// The reference is protected while it fills the alias.
    pub const PROTECTED: u32 = 0x400;
    pub const CLOSEST: u32 = 0x2000;
}

#[derive(Debug, Clone, PartialEq)]
pub enum Fill {
    /// Left for scripts (or nothing to fill it with).
    Empty,
    Forced(FormId),
    Unique(FormId),
    /// Another quest's alias (`ALEQ` + `ALEA`).
    External {
        quest: FormId,
        alias: u32,
    },
    /// Reference alias: a reference of a location ref type (`ALRT`) in a location alias (`ALFA`).
    LocationRef {
        loc_alias: u32,
        ref_type: FormId,
    },
    /// Location alias: the location of a reference alias (`ALFA`), or its parent with a keyword (`KNAM`).
    RefLocation {
        ref_alias: u32,
        keyword: FormId,
    },
    /// Location alias: a location (`ALFL`).
    Specific(FormId),
    /// The first reference or location the conditions accept.
    Matching,
    /// A new reference to `object` made at (or in) what alias `at` holds (`ALCO` + `ALCA`).
    Create {
        object: FormId,
        at: u32,
        inside: bool,
    },
    /// What the Story Manager event that started the quest names (`ALFE` event
    /// type + `ALFD` member: `R1`, `L2`...).
    FromEvent([u8; 2]),
    /// The first reference the conditions accept near what another alias
    /// holds (`ALNA`; `ALNT` is always 0 in the game's data), nearest first.
    Near(u32),
}

#[derive(Debug, Clone)]
pub struct AliasSpec {
    pub id: u32,
    pub name: String,
    pub location: bool,
    pub flags: u32,
    pub fill: Fill,
    pub conditions: Vec<Condition>,
    /// Temporary memberships of a filled reference alias (`ALFC`).
    pub factions: Vec<FormId>,
    /// Inventory entries (`CNTO`) granted when a reference is newly placed in
    /// this alias. The Creation Kit documentation says these grants are permanent.
    pub inventory: Vec<(FormId, i32)>,
}

/// What happens to a reference an alias created when its quest stops or starts over.
#[derive(Debug, PartialEq)]
enum CleanUp {
    /// Left be: the player has it (carried or picked up), it is an actor, or
    /// the quest only stopped and it lies in the world.
    Keep,
    /// An item made in another container: taken out of it.
    TakeOut(FormId),
    /// An object lying in the world: gone.
    Remove,
}

impl CleanUp {
    /// `stopping`: the quest stops, rather than starting over. An object it
    /// created in the world stays then: in the game it was neither disabled
    /// nor deleted after the alias was cleared, the quest stopped, or
    /// `CompleteQuest` (test-mod autotest22).
    fn of(c: &Created, disabled: bool, stopping: bool) -> Self {
        match c.container {
            _ if c.actor || disabled => Self::Keep,
            Some(PLAYER_REF) => Self::Keep,
            Some(container) => Self::TakeOut(container),
            None if stopping => Self::Keep,
            None => Self::Remove,
        }
    }
}

impl AliasSpec {
    pub fn has(&self, f: u32) -> bool {
        self.flags & f != 0
    }
}

/// The aliases of a quest, in record order.
pub fn parse(quest: &esp::LoadedRecord<'_>) -> Vec<AliasSpec> {
    let mut out = Vec::new();
    let mut cur: Option<AliasSpec> = None;
    // Fill parts that combine: ALFA with ALRT (reference) or KNAM (location), ALEQ with ALEA.
    let (mut from_alias, mut ref_type, mut keyword, mut ext_quest, mut ext_alias) =
        (None, FormId::NULL, FormId::NULL, None, 0);
    for sr in quest.subrecords() {
        let tag = &sr.tag.0;
        if matches!(tag, b"ALST" | b"ALLS") {
            cur = Some(AliasSpec {
                id: sr.u32(0),
                name: String::new(),
                location: tag == b"ALLS",
                flags: 0,
                fill: Fill::Empty,
                conditions: Vec::new(),
                factions: Vec::new(),
                inventory: Vec::new(),
            });
            (from_alias, ref_type, keyword, ext_quest, ext_alias) =
                (None, FormId::NULL, FormId::NULL, None, 0);
            continue;
        }
        let Some(a) = cur.as_mut() else { continue };
        let form = || quest.fid(sr.form_id(0));
        match tag {
            b"ALID" => a.name = sr.zstring(),
            b"FNAM" if sr.data.len() >= 4 => a.flags = sr.u32(0),
            b"ALFC" if !a.location && sr.data.len() >= 4 => {
                let faction = form();
                if !faction.is_null() && !a.factions.contains(&faction) {
                    a.factions.push(faction);
                }
            }
            b"CNTO" if !a.location && sr.data.len() >= 8 => {
                let item = form();
                let raw_count = sr.u32(4);
                if !item.is_null() && raw_count > 0 && raw_count <= i32::MAX as u32 {
                    let count = raw_count as i32;
                    a.inventory.push((item, count));
                }
            }
            b"ALFR" => a.fill = Fill::Forced(form()),
            b"ALUA" => a.fill = Fill::Unique(form()),
            b"ALFL" => a.fill = Fill::Specific(form()),
            b"ALFA" => from_alias = Some(sr.u32(0)),
            b"ALRT" => ref_type = form(),
            b"KNAM" => keyword = form(),
            b"ALEQ" => ext_quest = Some(form()),
            b"ALEA" => ext_alias = sr.u32(0),
            b"ALCO" => {
                a.fill = Fill::Create {
                    object: form(),
                    at: 0,
                    inside: false,
                }
            }
            b"ALCA" => {
                if let Fill::Create { at, inside, .. } = &mut a.fill {
                    let v = sr.u32(0);
                    *at = v & 0xffff;
                    *inside = v & 0x8000_0000 != 0;
                }
            }
            b"ALNA" => a.fill = Fill::Near(sr.u32(0)),
            b"ALFE" => a.fill = Fill::FromEvent([0; 2]),
            b"ALFD" if sr.data.len() >= 2 => {
                if let Fill::FromEvent(m) = &mut a.fill {
                    *m = [sr.data[0], sr.data[1]];
                }
            }
            b"CTDA" => {
                if let Some(c) = condition::parse(quest, sr.data) {
                    a.conditions.push(c);
                }
            }
            b"CIS1" => {
                if let Some(c) = a.conditions.last_mut() {
                    c.string_p1 = Some(sr.zstring().into());
                }
            }
            b"CIS2" => {
                if let Some(c) = a.conditions.last_mut() {
                    c.string_p2 = Some(sr.zstring().into());
                }
            }
            b"ALED" => {
                let mut a = cur.take().unwrap();
                if a.fill == Fill::Empty {
                    a.fill = match (from_alias, ext_quest) {
                        (_, Some(quest)) => Fill::External {
                            quest,
                            alias: ext_alias,
                        },
                        (Some(alias), _) if a.location => Fill::RefLocation {
                            ref_alias: alias,
                            keyword,
                        },
                        (Some(alias), _) => Fill::LocationRef {
                            loc_alias: alias,
                            ref_type,
                        },
                        _ if !a.conditions.is_empty() => Fill::Matching,
                        _ => Fill::Empty,
                    };
                }
                out.push(a);
            }
            _ => {}
        }
    }
    out
}

/// What a condition leaves of the persistent references.
enum Candidates {
    /// Any of them may pass.
    All,
    /// None of them does.
    None,
    /// Only these positions may.
    Some(Vec<u32>),
}

/// See [`Engine::persistent_index`]. Positions ascend in each list.
#[derive(Default)]
pub(crate) struct PersistentIndex {
    by_base: HashMap<FormId, Vec<u32>>,
    by_cell: HashMap<FormId, Vec<u32>>,
    by_editor_location: HashMap<FormId, Vec<u32>>,
    /// Placed actors (`ACHR`).
    actors: Vec<u32>,
    position: HashMap<FormId, u32>,
}

impl Engine {
    /// Apply an alias's authored inventory when a new reference is assigned.
    /// Items stay with the reference when it leaves or is cleared from the alias.
    fn grant_alias_inventory(&mut self, q: FormId, alias: u32, reference: FormId) {
        // Actor and container references have inventories. Ignore invalid or
        // non-inventory ForceRefTo targets rather than creating phantom state.
        let inventory_target = reference == PLAYER_REF
            || self
                .lo
                .tag_of(reference)
                .is_some_and(|tag| tag.0 == *b"ACHR")
            || self.created(reference).is_some_and(|created| created.actor)
            || self
                .base_of(reference)
                .is_some_and(|base| self.lo.tag_of(base).is_some_and(|tag| tag.0 == *b"CONT"));
        if !inventory_target {
            return;
        }
        let inventory = self
            .alias_specs(q)
            .iter()
            .find(|a| a.id == alias && !a.location)
            .map(|a| a.inventory.clone())
            .unwrap_or_default();
        for (item, count) in inventory {
            if count <= 0 {
                continue;
            }
            let items = if self.lo.tag_of(item).is_some_and(|t| t.0 == *b"LVLI") {
                let level = self.actor_level(reference).clamp(1, u16::MAX as i32) as u16;
                let seed = self.rand();
                crate::world::actor::resolve_items_at(
                    &self.lo,
                    item,
                    count,
                    seed,
                    &self.scripts.globals,
                    level,
                    0,
                )
            } else {
                vec![(item, count)]
            };
            for (item, count) in items {
                if count <= 0 || crate::world::inventory::item_info(&self.lo, item).is_none() {
                    continue;
                }
                let held = self.inventory_mut(reference).count(item);
                let granted = count.min(i32::MAX.saturating_sub(held));
                if granted > 0 {
                    self.add_item(reference, item, granted);
                }
            }
        }
    }

    /// Force a reference into an alias. Repeating the same assignment is
    /// idempotent for CNTO; a changed or empty alias receives its grant once.
    pub(crate) fn force_alias_ref(&mut self, q: FormId, alias: u32, reference: FormId) {
        let previous = self.alias_ref(q, alias);
        self.scripts.fill_seq += 1;
        let seq = self.scripts.fill_seq;
        let state = self.scripts.quests.entry(q).or_default();
        state.aliases.insert(alias, reference);
        state.filled_at.insert(alias, seq);
        self.scripts.alias_gen += 1;
        if previous != Some(reference) {
            self.grant_alias_inventory(q, alias, reference);
        }
        self.refresh_ref_location_aliases();
    }

    /// Recompute automatic location aliases only when their source changes.
    /// Remembering the source preserves explicit ForceLocationTo/Clear overrides.
    pub(crate) fn refresh_ref_location_aliases(&mut self) {
        let mut changes = Vec::new();
        let mut valid = HashSet::new();
        let mut quests: Vec<_> = self
            .scripts
            .quests
            .iter()
            .filter(|(_, s)| s.running)
            .map(|(&q, _)| q)
            .collect();
        quests.sort();
        for q in quests {
            for a in self.alias_specs(q).iter() {
                let Fill::RefLocation { ref_alias, keyword } = a.fill else {
                    continue;
                };
                let reference = self.alias_ref(q, ref_alias);
                let location = reference.and_then(|r| self.ref_current_location(r));
                let key = (q, a.id);
                valid.insert(key);
                if self.alias_location_sources.get(&key) == Some(&(reference, location)) {
                    continue;
                }
                self.alias_location_sources
                    .insert(key, (reference, location));
                let filled = location.and_then(|l| self.location_with_keyword(l, keyword));
                if self.alias_ref(q, a.id) != filled {
                    changes.push((q, a.id, filled));
                }
            }
        }
        self.alias_location_sources
            .retain(|key, _| valid.contains(key));
        for (q, alias, location) in changes {
            let state = self.scripts.quests.get_mut(&q).unwrap();
            if let Some(location) = location {
                self.scripts.fill_seq += 1;
                state.aliases.insert(alias, location);
                state.filled_at.insert(alias, self.scripts.fill_seq);
            } else {
                state.aliases.remove(&alias);
                state.filled_at.remove(&alias);
            }
            self.scripts.alias_gen += 1;
        }
    }

    pub(crate) fn remember_alias_locations(&mut self, q: FormId) {
        for a in self.alias_specs(q).iter() {
            if let Fill::RefLocation { ref_alias, .. } = a.fill {
                let r = self.alias_ref(q, ref_alias);
                self.alias_location_sources
                    .insert((q, a.id), (r, r.and_then(|r| self.ref_current_location(r))));
            }
        }
    }

    /// A quest's aliases, parsed once.
    /// Whether `reference` fills a reference alias with `flag` (`flags`) in
    /// a running quest.
    pub(crate) fn in_alias_with(&self, reference: FormId, flag: u32) -> bool {
        self.scripts
            .quests
            .iter()
            .filter(|(_, s)| s.running && s.aliases.values().any(|&r| r == reference))
            .any(|(&quest, state)| {
                self.alias_specs(quest).iter().any(|a| {
                    !a.location && a.has(flag) && state.aliases.get(&a.id) == Some(&reference)
                })
            })
    }

    pub fn alias_specs(&self, q: FormId) -> Arc<Vec<AliasSpec>> {
        if let Some(s) = self.alias_spec_cache.borrow().get(&q) {
            return s.clone();
        }
        let s = Arc::new(self.lo.get(q).map(|r| parse(&r)).unwrap_or_default());
        self.alias_spec_cache.borrow_mut().insert(q, s.clone());
        s
    }

    /// Whether alias `alias` of quest `q` is a location alias.
    pub fn is_location_alias(&mut self, q: FormId, alias: u32) -> bool {
        self.alias_specs(q)
            .iter()
            .any(|a| a.id == alias && a.location)
    }

    /// Persistent references (every cell's), and the player.
    fn persistent_refs(&self) -> &[FormId] {
        self.persistent_refs.get_or_init(|| {
            let mut out = vec![PLAYER_REF];
            for &c in self.lo.ids_of_type(b"CELL") {
                if let Some(idx) = self.lo.cell(c) {
                    out.extend(&idx.persistent);
                }
            }
            out
        })
    }

    /// Positions in `persistent_refs` by base, cell, editor location and
    /// whether they're placed actors, all read from the load order and so
    /// fixed for plugin references. The player (position 0) is left out: its
    /// location changes.
    fn persistent_index(&self) -> &PersistentIndex {
        self.persistent_index.get_or_init(|| {
            let mut index = PersistentIndex::default();
            for (i, &r) in self.persistent_refs().iter().enumerate().skip(1) {
                let i = i as u32;
                if let Some(base) = self.base_of(r) {
                    index.by_base.entry(base).or_default().push(i);
                }
                if let Some(cell) = self.lo.cell_of_ref(r) {
                    index.by_cell.entry(cell).or_default().push(i);
                }
                if let Some(l) = self.editor_location(r) {
                    index.by_editor_location.entry(l).or_default().push(i);
                }
                if self.lo.tag_of(r).is_some_and(|t| t.0 == *b"ACHR") {
                    index.actors.push(i);
                }
                index.position.entry(r).or_insert(i);
            }
            index
        })
    }

    /// The `persistent_refs` positions that can pass alias `a`'s conditions,
    /// in list order, the player's always among them; `None` when no
    /// condition group narrows them. See [`Engine::persistent_anchor`].
    fn persistent_candidates(&self, q: FormId, a: &AliasSpec) -> Option<Vec<u32>> {
        self.persistent_anchor(q, a).map(|(_, mut v)| {
            v.insert(0, 0);
            v
        })
    }

    /// The narrowest condition group (an OR group, or one condition ANDed
    /// alone) whose every member the index answers, as its range in the
    /// alias's conditions, and the positions (the player's left out) that can
    /// pass it. The groups are ANDed, so every other reference fails the
    /// conditions as a whole.
    fn persistent_anchor(
        &self,
        q: FormId,
        a: &AliasSpec,
    ) -> Option<(std::ops::Range<usize>, Vec<u32>)> {
        let conds = &a.conditions;
        let mut best: Option<(std::ops::Range<usize>, Vec<u32>)> = None;
        let mut start = 0;
        while start < conds.len() {
            let end = conds[start..]
                .iter()
                .position(|c| !c.or)
                .map_or(conds.len(), |i| start + i + 1);
            let mut union = Some(Vec::new());
            for c in &conds[start..end] {
                match (self.condition_candidates(q, c), &mut union) {
                    (Candidates::Some(set), Some(u)) => u.extend(set),
                    (Candidates::None, _) => {}
                    (Candidates::All, _) => union = None,
                    (_, None) => {}
                }
            }
            if let Some(mut set) = union {
                set.sort_unstable();
                set.dedup();
                if best.as_ref().is_none_or(|b| set.len() < b.1.len()) {
                    best = Some((start..end, set));
                }
            }
            start = end;
        }
        best
    }

    /// Which persistent references (by position, the player's left out) can
    /// pass condition `c` of quest `q`'s alias, run on the reference.
    fn condition_candidates(&self, q: FormId, c: &Condition) -> Candidates {
        if c.p1_ref.is_some() {
            return Candidates::All;
        }
        // Functions of quests and globals, never of the subject: true for
        // every reference or for none.
        if matches!(c.func, 18 | 56 | 58 | 59 | 74 | 170 | 543) {
            let ctx = Context {
                quest: Some(q),
                ..Default::default()
            };
            return if condition::evaluate_one(self, c, ctx) {
                Candidates::All
            } else {
                Candidates::None
            };
        }
        if c.run_on != 0 || c.swap || c.use_global || c.op != 0 || c.value != 1.0 {
            return Candidates::All;
        }
        let index = self.persistent_index();
        // The first parameter as `function_value` reads it.
        let p1 = if c.p1_alias {
            self.alias_ref(q, c.p1).unwrap_or_default()
        } else {
            FormId(c.p1)
        };
        let within = |l: Option<FormId>| -> Vec<u32> {
            let Some(l) = l else { return Vec::new() };
            index
                .by_editor_location
                .iter()
                .filter(|(at, _)| self.location_within(**at, l))
                .flat_map(|(_, refs)| refs.iter().copied())
                .collect()
        };
        // Only actors have record or alias factions, or record flags; script
        // faction changes are kept by reference.
        let actors = || {
            let mut v = index.actors.clone();
            if c.func == 71 {
                v.extend(
                    self.scripts
                        .faction_changes
                        .keys()
                        .filter_map(|r| index.position.get(r).copied()),
                );
            }
            v
        };
        let position = index.position.get(&p1).copied();
        Candidates::Some(match c.func {
            // GetIsID: the subject's base, or the subject itself.
            72 => {
                let mut v = index.by_base.get(&p1).cloned().unwrap_or_default();
                v.extend(position);
                v
            }
            136 => position.into_iter().collect(), // GetIsReference
            67 => index.by_cell.get(&p1).cloned().unwrap_or_default(), // GetInCell
            71 | 641 => actors(),                  // GetInFaction, IsUnique
            565 => within(Some(p1)),               // GetIsEditorLocation
            // GetIsEditorLocAlias: the alias's location, not `p1`.
            567 => within(self.alias_ref(q, c.p1)),
            _ => return Candidates::All,
        })
    }

    /// References in the loaded cells (and loaded actors), and the player.
    fn loaded_refs(&self) -> Vec<FormId> {
        let mut out = vec![PLAYER_REF];
        let mut seen = HashSet::new();
        for (key, rt) in &self.cells {
            let cell = match (*key, self.location) {
                (crate::render::CellKey::Interior(c), _) => Some(c),
                (
                    crate::render::CellKey::Exterior(x, y),
                    crate::engine::Location::Exterior { world, .. },
                ) => self
                    .lo
                    .world(world)
                    .and_then(|w| w.cells.get(&(x, y)).copied()),
                _ => None,
            };
            if let Some(idx) = cell.and_then(|c| self.lo.cell(c)) {
                out.extend(
                    idx.persistent
                        .iter()
                        .chain(&idx.temporary)
                        .filter(|r| seen.insert(**r)),
                );
            }
            out.extend(
                rt.actors
                    .iter()
                    .map(|a| a.ref_id)
                    .filter(|r| seen.insert(*r)),
            );
        }
        out
    }

    /// References about `r`: the loaded ones when it is loaded, else those of
    /// its cell (and, outdoors, the cells around it).
    fn refs_near(&self, r: FormId) -> Vec<FormId> {
        let loaded = |c: FormId| {
            self.cells.keys().any(|k| match (*k, self.location) {
                (crate::render::CellKey::Interior(i), _) => i == c,
                (
                    crate::render::CellKey::Exterior(x, y),
                    crate::engine::Location::Exterior { world, .. },
                ) => {
                    self.lo
                        .world(world)
                        .and_then(|w| w.cells.get(&(x, y)).copied())
                        == Some(c)
                }
                _ => false,
            })
        };
        let Some(cell) = self.lo.cell_of_ref(r) else {
            return Vec::new();
        };
        if r == PLAYER_REF || self.actor_cells.contains_key(&r) || loaded(cell) {
            return self.loaded_refs();
        }
        let Some(idx) = self.lo.cell(cell) else {
            return Vec::new();
        };
        let mut cells = vec![cell];
        // Outdoors (worldspace-persistent references by where they stand).
        let grid = idx.grid.or_else(|| {
            self.ref_position(r)
                .map(|p| ((p.x / 4096.0).floor() as i32, (p.y / 4096.0).floor() as i32))
        });
        if let (Some(w), Some(grid)) = (idx.world.and_then(|w| self.lo.world(w)), grid) {
            cells.extend(w.persistent_cell.filter(|&c| c != cell));
            for dx in -1..=1 {
                for dy in -1..=1 {
                    cells.extend(
                        w.cells
                            .get(&(grid.0 + dx, grid.1 + dy))
                            .filter(|&&c| c != cell),
                    );
                }
            }
        }
        cells
            .iter()
            .filter_map(|&c| self.lo.cell(c))
            .flat_map(|i| {
                i.persistent
                    .iter()
                    .chain(&i.temporary)
                    .copied()
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// References that reserving aliases of other running quests hold.
    fn reserved_refs(&self, except: FormId) -> HashSet<FormId> {
        let mut out = HashSet::new();
        for (q, st) in self
            .scripts
            .quests
            .iter()
            .filter(|(q, st)| **q != except && st.running)
        {
            let Some(specs) = self.alias_spec_cache.borrow().get(q).cloned() else {
                continue;
            };
            for a in specs.iter().filter(|a| a.has(flags::RESERVES)) {
                out.extend(st.aliases.get(&a.id));
            }
        }
        out
    }

    /// Whether reference `r` may fill alias `a` of quest `q`.
    fn eligible(&self, q: FormId, a: &AliasSpec, r: FormId, reserved: &HashSet<FormId>) -> bool {
        (a.has(flags::ALLOW_REUSE)
            || !self
                .scripts
                .quests
                .get(&q)
                .is_some_and(|st| st.aliases.values().any(|&x| x == r)))
            && (a.has(flags::ALLOW_RESERVED) || !reserved.contains(&r))
            && (a.has(flags::ALLOW_DEAD) || !self.is_dead(r))
            && (a.has(flags::ALLOW_DISABLED) || !self.is_disabled(r))
    }

    /// What alias `a` of quest `q` is filled with now (earlier aliases already in
    /// the quest's fills).
    fn fill_alias(&self, q: FormId, a: &AliasSpec, reserved: &HashSet<FormId>) -> Option<FormId> {
        let accepts = |r: FormId| {
            condition::evaluate(
                self,
                &a.conditions,
                Context {
                    subject: Some(r),
                    quest: Some(q),
                    ..Default::default()
                },
            )
        };
        // Forced references and unique actors too are left empty dead or
        // disabled, unless the alias allows it (`test-mod autotest4`: a
        // forced reference disabled and a unique actor killed first went
        // unfilled).
        let alive_enabled = |r: &FormId| {
            (a.has(flags::ALLOW_DEAD) || !self.is_dead(*r))
                && (a.has(flags::ALLOW_DISABLED) || !self.is_disabled(*r))
        };
        match &a.fill {
            Fill::Empty => None,
            Fill::Forced(r) => Some(*r).filter(alive_enabled),
            Fill::Unique(npc) if *npc == FormId(0x7) => Some(PLAYER_REF),
            Fill::Unique(npc) => self
                .npc_refs_index()
                .get(npc)
                .copied()
                .filter(alive_enabled),
            Fill::Specific(l) => Some(*l),
            // The event's reference or location, if the alias's conditions take it.
            Fill::FromEvent(m) => self
                .story_event_for(Some(q))
                .and_then(|ev| ev.member_form(*m))
                .filter(|&r| accepts(r)),
            Fill::External { quest, alias } => self.alias_ref(*quest, *alias),
            Fill::RefLocation { ref_alias, keyword } => self
                .alias_ref(q, *ref_alias)
                .and_then(|r| self.ref_current_location(r))
                .and_then(|l| self.location_with_keyword(l, *keyword)),
            Fill::LocationRef {
                loc_alias,
                ref_type,
            } => {
                let l = self.alias_ref(q, *loc_alias)?;
                self.location_refs_of_type(l, *ref_type)
                    .into_iter()
                    .find(|&r| self.eligible(q, a, r, reserved) && accepts(r))
            }
            Fill::Matching if a.location => self
                .lo
                .ids_of_type(b"LCTN")
                .iter()
                .copied()
                .find(|&l| accepts(l)),
            Fill::Matching => {
                let mut found: Vec<FormId> = if a.has(flags::IN_LOADED_AREA) {
                    // Both tests are pure; the conditions go first because, over
                    // the whole loaded area, they leave far fewer references.
                    self.loaded_refs()
                        .into_iter()
                        .filter(|&r| accepts(r) && self.eligible(q, a, r, reserved))
                        .collect()
                } else {
                    let mut v = Vec::new();
                    let refs = self.persistent_refs();
                    let candidates: Box<dyn Iterator<Item = FormId>> =
                        match self.persistent_candidates(q, a) {
                            Some(at) => Box::new(at.into_iter().map(|i| refs[i as usize])),
                            None => Box::new(refs.iter().copied()),
                        };
                    for r in candidates {
                        if self.eligible(q, a, r, reserved) && accepts(r) {
                            v.push(r);
                            if !a.has(flags::CLOSEST) {
                                break;
                            }
                        }
                    }
                    v
                };
                if a.has(flags::CLOSEST)
                    && let Some(p) = self.ref_position(PLAYER_REF)
                {
                    found.sort_by(|&x, &y| {
                        let d = |r| {
                            self.ref_position(r)
                                .map_or(f32::MAX, |v| v.distance_squared(p))
                        };
                        d(x).total_cmp(&d(y))
                    });
                }
                found.first().copied()
            }
            Fill::Near(near) => {
                let centre = self.alias_ref(q, *near)?;
                let at = self.ref_position(centre)?;
                let mut found: Vec<(f32, FormId)> = self
                    .refs_near(centre)
                    .into_iter()
                    // Conditions first, as for the loaded area above.
                    .filter(|&r| r != centre && accepts(r) && self.eligible(q, a, r, reserved))
                    .map(|r| {
                        (
                            self.ref_position(r)
                                .map_or(f32::MAX, |p| p.distance_squared(at)),
                            r,
                        )
                    })
                    .collect();
                found.sort_by(|x, y| x.0.total_cmp(&y.0));
                found.first().map(|f| f.1)
            }
            // Made by `fill_quest_aliases`.
            Fill::Create { .. } => None,
        }
    }

    /// Clear away what quest `q`'s aliases created (`ALCO`): items made in a
    /// container leave it, objects lying about are gone when the quest starts
    /// over (not when it only stops: `stopping`). What the player took or was
    /// given, and actors, stay.
    pub(crate) fn clean_up_alias_creations(&mut self, q: FormId, stopping: bool) {
        let specs = self.alias_specs(q);
        for a in specs
            .iter()
            .filter(|a| matches!(a.fill, Fill::Create { .. }))
        {
            let Some(r) = self.alias_ref(q, a.id) else {
                continue;
            };
            let Some(c) = self.created(r).cloned() else {
                continue;
            };
            match CleanUp::of(&c, self.is_disabled(r), stopping) {
                CleanUp::Keep => continue,
                CleanUp::TakeOut(container) => {
                    let inv = self.inventory_mut(container);
                    if inv.count(c.base) > 0 {
                        inv.remove_split(c.base, 1, None);
                    }
                }
                CleanUp::Remove => self.set_disabled(r, true),
            }
            log::debug!("quest {q} alias {} created {r} cleaned up", a.name);
            self.created_refs.refs.remove(&r);
            self.world_state.moved.remove(&r);
        }
    }

    /// Fill quest `q`'s aliases in order. False (and nothing filled) when a
    /// required alias finds nothing.
    pub fn fill_quest_aliases(&mut self, q: FormId) -> bool {
        self.clean_up_alias_creations(q, false);
        let specs = self.alias_specs(q);
        let reserved = self.reserved_refs(q);
        let previous_aliases = self
            .scripts
            .quests
            .get(&q)
            .map(|state| state.aliases.clone())
            .unwrap_or_default();
        self.scripts.quests.entry(q).or_default().aliases.clear();
        let mut grants = Vec::new();
        let edid = self
            .lo
            .get(q)
            .and_then(|r| r.editor_id())
            .unwrap_or_default();
        for a in specs.iter() {
            let filled = match a.fill {
                Fill::Create { object, at, inside } => self
                    .alias_ref(q, at)
                    .and_then(|at| self.create_ref(object, at, inside)),
                _ => self.fill_alias(q, a, &reserved),
            };
            match filled {
                Some(r) => {
                    log::trace!("{edid} alias {} = {r}", a.name);
                    self.scripts.fill_seq += 1;
                    let seq = self.scripts.fill_seq;
                    let st = self.scripts.quests.entry(q).or_default();
                    st.aliases.insert(a.id, r);
                    st.filled_at.insert(a.id, seq);
                    if previous_aliases.get(&a.id) != Some(&r) && !a.location {
                        grants.push((a.id, r));
                    }
                }
                None if !a.has(flags::OPTIONAL) && a.fill != Fill::Empty => {
                    // Story Manager attempts fail all the time (the event's actors aren't the ones).
                    let level = if self
                        .scripts
                        .quests
                        .get(&q)
                        .is_some_and(|st| st.event.is_some())
                    {
                        log::Level::Debug
                    } else {
                        log::Level::Info
                    };
                    log::log!(
                        level,
                        "quest {edid} not started: alias {} ({:?}) not filled",
                        a.name,
                        a.fill
                    );
                    self.scripts.quests.entry(q).or_default().aliases.clear();
                    return false;
                }
                None => {}
            }
        }
        for (alias, reference) in grants {
            self.grant_alias_inventory(q, alias, reference);
        }
        self.remember_alias_locations(q);
        true
    }
}

#[cfg(test)]
#[path = "alias_depth_tests.rs"]
mod depth_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    /// Every persistent reference `persistent_anchor` leaves out fails the
    /// condition group it narrows by, evaluated as usual. Location aliases are
    /// filled with a spread of locations so `GetIsEditorLocAlias` is exercised.
    /// Reads installed assets in place, never copies them.
    #[test]
    #[ignore = "needs Skyrim SE Data and a headless GPU adapter"]
    fn persistent_candidates_leave_out_only_references_that_fail() {
        let data = vfs::locate_data_dir().expect("Skyrim SE Data");
        let names = esp::LoadOrder::default_plugin_list(&data, None);
        let mut e = crate::stats::action_tests::headless_engine_plugins(&data, &names);
        let refs = e.persistent_refs().to_vec();
        let mut locations: Vec<FormId> = e
            .persistent_index()
            .by_editor_location
            .keys()
            .copied()
            .collect();
        locations.sort();
        let locations: Vec<Option<FormId>> = std::iter::once(None)
            .chain(locations.iter().step_by(40).copied().map(Some))
            .collect();
        let (mut checked, mut seen) = (HashMap::<u16, usize>::new(), HashSet::new());
        for q in e.lo.ids_of_type(b"QUST").to_vec() {
            let specs = e.alias_specs(q);
            let location_aliases: Vec<u32> =
                specs.iter().filter(|a| a.location).map(|a| a.id).collect();
            for &l in &locations {
                let fills = &mut e.scripts.quests.entry(q).or_default().aliases;
                for &id in &location_aliases {
                    match l {
                        Some(l) => fills.insert(id, l),
                        None => fills.remove(&id),
                    };
                }
                for a in specs.iter().filter(|a| {
                    a.fill == Fill::Matching && !a.location && !a.has(flags::IN_LOADED_AREA)
                }) {
                    let Some((range, set)) = e.persistent_anchor(q, a) else {
                        continue;
                    };
                    let group = &a.conditions[range.clone()];
                    let key: Vec<_> = group
                        .iter()
                        .map(|c| {
                            let target = if c.func == 567 || c.p1_alias {
                                e.alias_ref(q, c.p1)
                            } else {
                                Some(FormId(c.p1))
                            };
                            (c.func, target, c.op, c.value.to_bits())
                        })
                        .collect();
                    if !seen.insert(key) {
                        continue;
                    }
                    let members: HashSet<u32> = set.iter().copied().collect();
                    for (at, &r) in refs.iter().enumerate().skip(1) {
                        if members.contains(&(at as u32)) {
                            continue;
                        }
                        let ctx = Context {
                            subject: Some(r),
                            quest: Some(q),
                            ..Default::default()
                        };
                        assert!(
                            !condition::evaluate(&e, group, ctx),
                            "{} alias {} conditions {range:?} pass {r}, left out",
                            q,
                            a.name,
                        );
                    }
                    for c in group {
                        *checked.entry(c.func).or_default() += 1;
                    }
                }
            }
            e.scripts.quests.remove(&q);
        }
        eprintln!("anchors checked by function: {checked:?}");
        for (func, least) in [(71, 10), (72, 10), (567, 10), (136, 3)] {
            assert!(
                checked.get(&func).is_some_and(|&n| n >= least),
                "{checked:?}"
            );
        }
    }

    fn made(container: Option<FormId>, actor: bool) -> Created {
        Created {
            base: FormId(0x1),
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            origin: None,
            place: None,
            container,
            location: None,
            actor,
            count: 1,
            owner: None,
        }
    }

    #[test]
    fn creations_are_cleaned_up_unless_the_player_has_them() {
        let chest = FormId(0x1234);
        assert_eq!(
            CleanUp::of(&made(None, false), false, false),
            CleanUp::Remove
        );
        // Stopping leaves objects in the world (test-mod autotest22).
        assert_eq!(CleanUp::of(&made(None, false), false, true), CleanUp::Keep);
        assert_eq!(
            CleanUp::of(&made(Some(chest), false), false, true),
            CleanUp::TakeOut(chest)
        );
        // Given to the player, picked up, or an actor: kept.
        assert_eq!(
            CleanUp::of(&made(Some(PLAYER_REF), false), false, false),
            CleanUp::Keep
        );
        assert_eq!(CleanUp::of(&made(None, false), true, false), CleanUp::Keep);
        assert_eq!(CleanUp::of(&made(None, true), false, false), CleanUp::Keep);
    }
}
