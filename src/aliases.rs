//! Quest aliases filled when a quest starts. Reference aliases: forced
//! references, unique actors, another quest's alias, a location alias's
//! reference of a location ref type, or the first reference meeting the alias's
//! conditions ("find matching reference"). Location aliases: a specific
//! location, a reference alias's location, another quest's alias, or the first
//! location meeting the conditions. Aliases fill in record order, so conditions
//! see the aliases above them; a quest whose required (not "Optional") alias
//! can't be filled doesn't start. Based on the Creation Kit wiki's *Quest Alias
//! Tab* and UESP *Mod File Format/QUST*.

use std::collections::HashSet;
use std::sync::Arc;

use esp::FormId;

use crate::condition::{self, Condition, Context};
use crate::engine::{Engine, PLAYER_REF};

/// Alias flags (`FNAM`).
pub mod flags {
    pub const RESERVES: u32 = 0x1;
    pub const OPTIONAL: u32 = 0x2;
    pub const ALLOW_REUSE: u32 = 0x8;
    pub const ALLOW_DEAD: u32 = 0x10;
    pub const IN_LOADED_AREA: u32 = 0x20;
    pub const ALLOW_DISABLED: u32 = 0x80;
    pub const ALLOW_RESERVED: u32 = 0x200;
    pub const CLOSEST: u32 = 0x2000;
}

#[derive(Debug, Clone, PartialEq)]
pub enum Fill {
    /// Left for scripts (or nothing to fill it with).
    Empty,
    Forced(FormId),
    Unique(FormId),
    /// Another quest's alias (`ALEQ` + `ALEA`).
    External { quest: FormId, alias: u32 },
    /// Reference alias: a reference of a location ref type (`ALRT`) in a location alias (`ALFA`).
    LocationRef { loc_alias: u32, ref_type: FormId },
    /// Location alias: the location of a reference alias (`ALFA`), or its parent with a keyword (`KNAM`).
    RefLocation { ref_alias: u32, keyword: FormId },
    /// Location alias: a location (`ALFL`).
    Specific(FormId),
    /// The first reference or location the conditions accept.
    Matching,
    /// A new reference to `object` made at (or in) what alias `at` holds (`ALCO` + `ALCA`).
    Create { object: FormId, at: u32, inside: bool },
    /// Kinds not filled yet: "near alias", story manager events.
    Unsupported(&'static str),
}

#[derive(Debug, Clone)]
pub struct AliasSpec {
    pub id: u32,
    pub name: String,
    pub location: bool,
    pub flags: u32,
    pub fill: Fill,
    pub conditions: Vec<Condition>,
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
    let (mut from_alias, mut ref_type, mut keyword, mut ext_quest, mut ext_alias) = (None, FormId::NULL, FormId::NULL, None, 0);
    for sr in quest.subrecords() {
        let tag = &sr.tag.0;
        if matches!(tag, b"ALST" | b"ALLS") {
            cur = Some(AliasSpec { id: sr.u32(0), name: String::new(), location: tag == b"ALLS", flags: 0, fill: Fill::Empty, conditions: Vec::new() });
            (from_alias, ref_type, keyword, ext_quest, ext_alias) = (None, FormId::NULL, FormId::NULL, None, 0);
            continue;
        }
        let Some(a) = cur.as_mut() else { continue };
        let form = || quest.fid(sr.form_id(0));
        match tag {
            b"ALID" => a.name = sr.zstring(),
            b"FNAM" if sr.data.len() >= 4 => a.flags = sr.u32(0),
            b"ALFR" => a.fill = Fill::Forced(form()),
            b"ALUA" => a.fill = Fill::Unique(form()),
            b"ALFL" => a.fill = Fill::Specific(form()),
            b"ALFA" => from_alias = Some(sr.u32(0)),
            b"ALRT" => ref_type = form(),
            b"KNAM" => keyword = form(),
            b"ALEQ" => ext_quest = Some(form()),
            b"ALEA" => ext_alias = sr.u32(0),
            b"ALCO" => a.fill = Fill::Create { object: form(), at: 0, inside: false },
            b"ALCA" => {
                if let Fill::Create { at, inside, .. } = &mut a.fill {
                    let v = sr.u32(0);
                    *at = v & 0xffff;
                    *inside = v & 0x8000_0000 != 0;
                }
            }
            b"ALNA" => a.fill = Fill::Unsupported("near alias"),
            b"ALFE" => a.fill = Fill::Unsupported("from event"),
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
                        (_, Some(quest)) => Fill::External { quest, alias: ext_alias },
                        (Some(alias), _) if a.location => Fill::RefLocation { ref_alias: alias, keyword },
                        (Some(alias), _) => Fill::LocationRef { loc_alias: alias, ref_type },
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

impl Engine {
    /// A quest's aliases, parsed once.
    pub fn alias_specs(&mut self, q: FormId) -> Arc<Vec<AliasSpec>> {
        if let Some(s) = self.alias_spec_cache.get(&q) {
            return s.clone();
        }
        let s = Arc::new(self.lo.get(q).map(|r| parse(&r)).unwrap_or_default());
        self.alias_spec_cache.insert(q, s.clone());
        s
    }

    /// Whether alias `alias` of quest `q` is a location alias.
    pub fn is_location_alias(&mut self, q: FormId, alias: u32) -> bool {
        self.alias_specs(q).iter().any(|a| a.id == alias && a.location)
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

    /// References in the loaded cells (and loaded actors), and the player.
    fn loaded_refs(&self) -> Vec<FormId> {
        let mut out = vec![PLAYER_REF];
        let mut seen = HashSet::new();
        for (key, rt) in &self.cells {
            let cell = match (*key, self.location) {
                (crate::render::CellKey::Interior(c), _) => Some(c),
                (crate::render::CellKey::Exterior(x, y), crate::engine::Location::Exterior { world, .. }) => {
                    self.lo.world(world).and_then(|w| w.cells.get(&(x, y)).copied())
                }
                _ => None,
            };
            if let Some(idx) = cell.and_then(|c| self.lo.cell(c)) {
                out.extend(idx.persistent.iter().chain(&idx.temporary).filter(|r| seen.insert(**r)));
            }
            out.extend(rt.actors.iter().map(|a| a.ref_id).filter(|r| seen.insert(*r)));
        }
        out
    }

    /// References that reserving aliases of other running quests hold.
    fn reserved_refs(&self, except: FormId) -> HashSet<FormId> {
        let mut out = HashSet::new();
        for (q, st) in self.scripts.quests.iter().filter(|(q, st)| **q != except && st.running) {
            let Some(specs) = self.alias_spec_cache.get(q) else { continue };
            for a in specs.iter().filter(|a| a.has(flags::RESERVES)) {
                out.extend(st.aliases.get(&a.id));
            }
        }
        out
    }

    /// Whether reference `r` may fill alias `a` of quest `q`.
    fn eligible(&self, q: FormId, a: &AliasSpec, r: FormId, reserved: &HashSet<FormId>) -> bool {
        (a.has(flags::ALLOW_REUSE) || !self.scripts.quests.get(&q).is_some_and(|st| st.aliases.values().any(|&x| x == r)))
            && (a.has(flags::ALLOW_RESERVED) || !reserved.contains(&r))
            && (a.has(flags::ALLOW_DEAD) || !self.is_dead(r))
            && (a.has(flags::ALLOW_DISABLED) || !self.is_disabled(r))
    }

    /// What alias `a` of quest `q` is filled with now (earlier aliases already in
    /// the quest's fills). `Err` for a fill kind not supported.
    fn fill_alias(&self, q: FormId, a: &AliasSpec, reserved: &HashSet<FormId>) -> Result<Option<FormId>, &'static str> {
        let accepts = |r: FormId| condition::evaluate(self, &a.conditions, Context { subject: Some(r), quest: Some(q), ..Default::default() });
        Ok(match &a.fill {
            Fill::Empty => None,
            Fill::Forced(r) => Some(*r),
            Fill::Unique(npc) if *npc == FormId(0x7) => Some(PLAYER_REF),
            Fill::Unique(npc) => self.npc_refs_index().get(npc).copied(),
            Fill::Specific(l) => Some(*l),
            Fill::External { quest, alias } => self.alias_ref(*quest, *alias),
            Fill::RefLocation { ref_alias, keyword } => self
                .alias_ref(q, *ref_alias)
                .and_then(|r| self.ref_current_location(r))
                .and_then(|l| self.location_with_keyword(l, *keyword)),
            Fill::LocationRef { loc_alias, ref_type } => {
                let Some(l) = self.alias_ref(q, *loc_alias) else { return Ok(None) };
                self.location_refs_of_type(l, *ref_type).into_iter().find(|&r| self.eligible(q, a, r, reserved) && accepts(r))
            }
            Fill::Matching if a.location => self.lo.ids_of_type(b"LCTN").iter().copied().find(|&l| accepts(l)),
            Fill::Matching => {
                let mut found: Vec<FormId> = if a.has(flags::IN_LOADED_AREA) {
                    self.loaded_refs().into_iter().filter(|&r| self.eligible(q, a, r, reserved) && accepts(r)).collect()
                } else {
                    let mut v = Vec::new();
                    for &r in self.persistent_refs() {
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
                        let d = |r| self.ref_position(r).map_or(f32::MAX, |v| v.distance_squared(p));
                        d(x).total_cmp(&d(y))
                    });
                }
                found.first().copied()
            }
            // Made by `fill_quest_aliases`.
            Fill::Create { .. } => None,
            Fill::Unsupported(kind) => return Err(kind),
        })
    }

    /// Fill quest `q`'s aliases in order. False (and nothing filled) when a
    /// required alias finds nothing.
    pub fn fill_quest_aliases(&mut self, q: FormId) -> bool {
        let specs = self.alias_specs(q);
        let reserved = self.reserved_refs(q);
        self.scripts.quests.entry(q).or_default().aliases.clear();
        let edid = self.lo.get(q).and_then(|r| r.editor_id()).unwrap_or_default();
        for a in specs.iter() {
            let filled = match a.fill {
                Fill::Create { object, at, inside } => Ok(self.alias_ref(q, at).and_then(|at| self.create_ref(object, at, inside))),
                _ => self.fill_alias(q, a, &reserved),
            };
            match filled {
                Ok(Some(r)) => {
                    log::trace!("{edid} alias {} = {r}", a.name);
                    self.scripts.quests.entry(q).or_default().aliases.insert(a.id, r);
                }
                Ok(None) if !a.has(flags::OPTIONAL) && a.fill != Fill::Empty => {
                    log::info!("quest {edid} not started: alias {} ({:?}) not filled", a.name, a.fill);
                    self.scripts.quests.entry(q).or_default().aliases.clear();
                    return false;
                }
                Ok(None) => {}
                Err(kind) => log::debug!("{edid} alias {}: {kind} fills aren't supported", a.name),
            }
        }
        true
    }
}
