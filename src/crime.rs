//! Crime: the player's crimes seen by actors who report them to their crime
//! faction (`CRIF`), which adds crime gold (a bounty) from its crime values
//! (`FACT` `CRVA`). Theft, assault and murder are crimes so far; stolen
//! goods stay marked as their owners' (`Inventory::owned`); scripts and
//! conditions read and change the gold (`GetCrimeGold`, `ModCrimeGold`,
//! `PlayerPayCrimeGold`...).

use std::collections::{BTreeMap, HashMap, HashSet};

use esp::{FormId, LoadOrder};

use crate::engine::{Engine, PLAYER_REF};

/// Crime types, as the engine numbers them (`GetCrime`, CommonLibSSE
/// `PackageNS::CRIME_TYPE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // pickpocketing, escape and werewolves aren't committed yet
pub enum CrimeType {
    Steal = 0,
    Pickpocket = 1,
    Trespass = 2,
    Attack = 3,
    Murder = 4,
    Escape = 5,
    Werewolf = 6,
}

impl CrimeType {
    pub fn violent(self) -> bool {
        matches!(self, CrimeType::Attack | CrimeType::Murder | CrimeType::Werewolf)
    }

    /// The faction flag making its members ignore this crime against
    /// non-members (`DATA`).
    fn ignore_flag(self) -> u32 {
        match self {
            CrimeType::Murder => 0x80,
            CrimeType::Attack => 0x100,
            CrimeType::Steal => 0x200,
            CrimeType::Trespass => 0x400,
            CrimeType::Pickpocket => 0x2000,
            CrimeType::Werewolf => 0x10000,
            CrimeType::Escape => 0,
        }
    }
}

const TRACK_CRIME: u32 = 0x40;
const DO_NOT_REPORT_MEMBERS: u32 = 0x800;
const USE_DEFAULTS: u32 = 0x1000;


/// A crime faction's crime values (`CRVA`), or the defaults when it says to
/// use them (`DATA` 0x1000; the values UESP gives for the holds).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CrimeValues {
    pub flags: u32,
    pub arrest: bool,
    pub attack_on_sight: bool,
    pub murder: i32,
    pub assault: i32,
    pub trespass: i32,
    pub pickpocket: i32,
    pub steal_mult: f32,
    pub escape: i32,
    pub werewolf: i32,
}

impl CrimeValues {
    /// A faction's crime values; `None` unless it tracks crime (`DATA` 0x40).
    pub fn of(lo: &LoadOrder, faction: FormId) -> Option<CrimeValues> {
        let rec = lo.get(faction)?;
        let flags = rec.get(b"DATA").filter(|d| d.len() >= 4).map(|d| u32::from_le_bytes(d[0..4].try_into().unwrap()))?;
        if flags & TRACK_CRIME == 0 {
            return None;
        }
        let d = rec.get(b"CRVA").unwrap_or(&[]);
        let u16_at = |o: usize| d.get(o..o + 2).map_or(0, |b| u16::from_le_bytes([b[0], b[1]]) as i32);
        let mut v = CrimeValues {
            flags,
            arrest: d.first().is_some_and(|&b| b != 0),
            attack_on_sight: d.get(1).is_some_and(|&b| b != 0),
            murder: u16_at(2),
            assault: u16_at(4),
            trespass: u16_at(6),
            pickpocket: u16_at(8),
            steal_mult: d.get(12..16).map_or(0.0, |b| f32::from_le_bytes(b.try_into().unwrap())),
            escape: u16_at(16),
            werewolf: u16_at(18),
        };
        if flags & USE_DEFAULTS != 0 {
            (v.murder, v.assault, v.trespass, v.pickpocket, v.steal_mult, v.escape, v.werewolf) = (1000, 40, 5, 25, 0.5, 100, 1000);
        }
        Some(v)
    }

    /// The crime gold for a crime; `value` is what was stolen.
    pub fn gold(&self, kind: CrimeType, value: i32) -> i32 {
        match kind {
            CrimeType::Steal => (value as f32 * self.steal_mult).floor() as i32,
            CrimeType::Pickpocket => self.pickpocket,
            CrimeType::Trespass => self.trespass,
            CrimeType::Attack => self.assault,
            CrimeType::Murder => self.murder,
            CrimeType::Escape => self.escape,
            CrimeType::Werewolf => self.werewolf,
        }
    }
}

/// The player's standing with a crime faction (CommonLibSSE
/// `CrimeGoldStruct`): crime gold owed, and infamy, all the gold ever added.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Bounty {
    pub violent: i32,
    pub nonviolent: i32,
    pub infamy_violent: i32,
    pub infamy_nonviolent: i32,
}

impl Bounty {
    pub fn total(&self) -> i32 {
        self.violent + self.nonviolent
    }
}

#[derive(Default)]
pub struct Crimes {
    /// The player's bounty with each crime faction.
    pub bounties: BTreeMap<FormId, Bounty>,
    /// Crime factions scripts gave actors (`SetCrimeFaction`; `None` clears it).
    pub crime_factions: HashMap<FormId, Option<FormId>>,
    /// Actors the player assaulted (a crime): killing them is murder.
    assaulted: HashSet<FormId>,
    /// The value of what the player has stolen from each crime faction's
    /// people, unwitnessed and witnessed (CommonLibSSE `StolenItemValueStruct`).
    pub stolen_value: BTreeMap<FormId, (i32, i32)>,
    /// Actors running a package that locks doors, and their homes: private
    /// while they do (see `trespassing`).
    pub private_homes: HashMap<FormId, FormId>,
    /// The player trespassing where someone has seen them.
    pub trespass: Option<Trespass>,
}

/// The player found trespassing in a cell: warned (level 0), warned a last
/// time (1), the crime reported (2); the next step comes at `next`
/// (`GetTrespassWarningLevel`, the Trespass topic's lines).
#[derive(Debug, Clone, Copy)]
pub struct Trespass {
    pub cell: FormId,
    pub level: i32,
    pub next: f64,
}

/// Interior cell flags: Public Area (`DATA`), Off Limits (record header).
const CELL_PUBLIC: u32 = 0x20;
const CELL_OFF_LIMITS: u32 = 0x20000;

impl Engine {
    /// The faction an actor reports crimes to and is protected by: its own
    /// (`CRIF`, through its templates' factions), unless a script changed it.
    pub fn crime_faction(&self, actor: FormId) -> Option<FormId> {
        if actor == PLAYER_REF {
            return None;
        }
        if let Some(&f) = self.crime.crime_factions.get(&actor) {
            return f;
        }
        self.templates_of(actor).and_then(|t| t.form(&self.lo, crate::world::template::FACTIONS, b"CRIF"))
    }

    pub fn set_crime_faction(&mut self, actor: FormId, faction: Option<FormId>) {
        self.crime.crime_factions.insert(actor, faction);
    }

    pub fn bounty(&self, faction: FormId) -> Bounty {
        self.crime.bounties.get(&faction).copied().unwrap_or_default()
    }

    /// Add (or with a negative amount, take off) crime gold; a rise adds to
    /// infamy too.
    pub fn mod_crime_gold(&mut self, faction: FormId, amount: i32, violent: bool) {
        let b = self.crime.bounties.entry(faction).or_default();
        let (gold, infamy) = if violent { (&mut b.violent, &mut b.infamy_violent) } else { (&mut b.nonviolent, &mut b.infamy_nonviolent) };
        *gold = (*gold + amount).max(0);
        if amount > 0 {
            *infamy += amount;
        }
        log::info!("crime gold with {faction}: {} violent, {} nonviolent", b.violent, b.nonviolent);
    }

    /// Set the crime gold of one kind (Papyrus `SetCrimeGold` is the
    /// non-violent, `SetCrimeGoldViolent` the violent).
    pub fn set_crime_gold(&mut self, faction: FormId, gold: i32, violent: bool) {
        let have = self.bounty(faction);
        let now = if violent { have.violent } else { have.nonviolent };
        self.mod_crime_gold(faction, gold.max(0) - now, violent);
    }

    /// The player pays off their bounty with a faction: the gold they carry
    /// goes towards it and it is cleared. With `remove_stolen`, every stolen
    /// item they carry is taken into the faction's stolen goods container
    /// (`STOL`; UESP: confiscated to the jail's evidence chest), and what
    /// they stole from it is forgotten. There is no jail yet.
    pub fn pay_crime_gold(&mut self, faction: FormId, remove_stolen: bool, go_to_jail: bool) -> i32 {
        let owed = self.bounty(faction).total();
        let paid = self.remove_item(PLAYER_REF, GOLD, owed, None);
        if let Some(b) = self.crime.bounties.get_mut(&faction) {
            b.violent = 0;
            b.nonviolent = 0;
        }
        if remove_stolen {
            self.confiscate_stolen(faction);
        }
        log::info!("player pays {paid} of {owed} crime gold to {faction} (remove stolen {remove_stolen}, jail {go_to_jail}: not done)");
        paid
    }

    /// Take every stolen item from the player into a crime faction's stolen
    /// goods container (`STOL`), still marked as their owners'.
    pub fn confiscate_stolen(&mut self, faction: FormId) -> i32 {
        let chest = self.lo.get(faction).and_then(|r| r.get(b"STOL").filter(|d| d.len() >= 4).map(|d| r.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap()))))).filter(|f| !f.is_null());
        let stolen: Vec<(FormId, FormId, i32)> = self.inventory_mut(PLAYER_REF).owned.clone();
        let mut n = 0;
        for (item, owner, count) in stolen {
            n += self.remove_stack(PLAYER_REF, item, count, Some(Some(owner)), chest, None).iter().map(|(_, k)| k).sum::<i32>();
        }
        self.crime.stolen_value.remove(&faction);
        log::info!("{n} stolen items confiscated into {chest:?}");
        n
    }

    /// The player stole `value` worth from `faction`'s people, seen or not.
    pub(crate) fn add_stolen_value(&mut self, faction: FormId, value: i32, witnessed: bool) {
        let v = self.crime.stolen_value.entry(faction).or_default();
        if witnessed { v.1 += value } else { v.0 += value }
    }

    /// Whether the player carries enough gold to pay their bounty with a faction.
    pub fn can_pay_crime_gold(&self, faction: FormId) -> bool {
        self.item_count(PLAYER_REF, GOLD) >= self.bounty(faction).total()
    }

    /// The crime faction of where the player is: the nearest location up from
    /// the current one naming one (`LCTN` `FNAM`: the holds).
    pub fn location_crime_faction(&self) -> Option<FormId> {
        let mut l = self.current_location()?;
        for _ in 0..16 {
            let rec = self.lo.get(l)?;
            if let Some(d) = rec.get(b"FNAM").filter(|d| d.len() >= 4) {
                return Some(rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap()))));
            }
            let d = rec.get(b"PNAM").filter(|d| d.len() >= 4)?;
            l = rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap())));
        }
        None
    }

    /// Whether two actors' crime factions are the same, or one lists the
    /// other in its crime group (`CRGR`).
    pub fn in_shared_crime_faction(&self, a: FormId, b: FormId) -> bool {
        let (Some(fa), Some(fb)) = (self.crime_faction(a), self.crime_faction(b)) else { return false };
        let group = |f: FormId| self.lo.get(f).and_then(|r| r.get(b"CRGR").filter(|d| d.len() >= 4).map(|d| r.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap())))));
        fa == fb || group(fa).is_some_and(|g| self.formlist(g).contains(&fb)) || group(fb).is_some_and(|g| self.formlist(g).contains(&fa))
    }

    /// Living actors who detect the player now (stealth points don't count
    /// for crimes: the CK wiki's Stealth Points page).
    fn witnesses(&self) -> Vec<FormId> {
        self.cells.values().flat_map(|rt| &rt.actors).map(|a| a.ref_id).filter(|&r| self.detects(r, PLAYER_REF)).collect()
    }

    /// The player commits a crime against `victim` (an actor) or against what
    /// `owner` (a faction) owns. Each witness with a crime faction reports it
    /// there, unless its faction ignores the crime against non-members of it
    /// (`DATA` ignore flags) or, the victim being a member, doesn't report
    /// crimes against members (0x800) and the witness isn't the victim; each
    /// faction told adds its crime gold for it once. The victim of an assault
    /// knows of it whether or not it sees the player. Returns whether anyone
    /// reported it.
    pub(crate) fn commit_crime(&mut self, kind: CrimeType, victim: Option<FormId>, owner: Option<FormId>, value: i32) -> bool {
        let victim_faction = victim.and_then(|v| self.crime_faction(v)).or(owner);
        let mut witnesses = self.witnesses();
        if kind == CrimeType::Attack
            && let Some(v) = victim.filter(|v| !witnesses.contains(v) && !self.is_dead(*v))
        {
            witnesses.push(v);
        }
        if kind == CrimeType::Murder {
            witnesses.retain(|&w| Some(w) != victim);
        }
        let mut told: Vec<(FormId, CrimeValues)> = Vec::new();
        for w in witnesses {
            let Some(f) = self.crime_faction(w) else { continue };
            let Some(values) = CrimeValues::of(&self.lo, f) else { continue };
            if told.iter().any(|(t, _)| *t == f) {
                continue;
            }
            let member = victim_faction == Some(f);
            if !member && values.flags & kind.ignore_flag() != 0 {
                continue;
            }
            if member && values.flags & DO_NOT_REPORT_MEMBERS != 0 && Some(w) != victim {
                continue;
            }
            log::info!("{w} reports {kind:?} to {f}");
            told.push((f, values));
        }
        for &(f, values) in &told {
            let gold = values.gold(kind, value);
            if gold > 0 {
                self.mod_crime_gold(f, gold, kind.violent());
                let name = self.form_name(f);
                self.scripts.notify(format!("Bounty added: {gold} ({name})"));
            }
        }
        if !told.is_empty() {
            log::info!("{kind:?} reported to {:?}", told.iter().map(|t| t.0).collect::<Vec<_>>());
        }
        !told.is_empty()
    }

    /// The player assaulted an actor (a crime: see `send_assault`).
    pub(crate) fn player_assault(&mut self, victim: FormId) {
        self.crime.assaulted.insert(victim);
        self.commit_crime(CrimeType::Attack, Some(victim), None, 0);
    }

    /// The player killed an actor. It is murder when they had assaulted it
    /// (attacked it first, unprovoked). The kill event's crime status: 0 not
    /// murder (or the victim has no crime faction), 1 unreported, 2 reported.
    pub(crate) fn player_kill(&mut self, victim: FormId) -> i32 {
        if !self.crime.assaulted.remove(&victim) || self.crime_faction(victim).is_none() {
            return 0;
        }
        if self.commit_crime(CrimeType::Murder, Some(victim), None, 0) { 2 } else { 1 }
    }

    /// Whether the player is trespassing: in an interior that isn't a public
    /// area and is either off limits or private, someone else's home whose
    /// owner (or one of whose household) runs a package that locks the doors
    /// (sleeping, or the shop shut).
    pub fn trespassing(&self) -> bool {
        let crate::engine::Location::Interior(cell) = self.location else { return false };
        let Some(rec) = self.lo.get(cell) else { return false };
        let data = rec.get(b"DATA").map_or(0, |d| d.iter().take(2).enumerate().fold(0u32, |a, (i, b)| a | (*b as u32) << (8 * i)));
        if data & CELL_PUBLIC != 0 {
            return false;
        }
        if rec.flags() & CELL_OFF_LIMITS != 0 {
            return true;
        }
        drop(rec);
        let owned = crate::ai::furniture::owner_of(&self.lo, cell).is_some_and(|o| self.owned_by_other(o));
        owned && self.crime.private_homes.values().any(|&c| c == cell)
    }

    /// The player trespassing: the one in the cell who detects them most (awake
    /// and not fighting) tells them to leave with the Trespass topic, then
    /// gives a last warning `fAITrespassWarningTimer` seconds later, then,
    /// as long again, calls for the guards and the crime is reported (UESP:
    /// one warning, then a bounty). In an off limits cell being seen is the
    /// crime at once. Each step waits for someone to see them; it all starts
    /// over once they are out of the cell.
    pub(crate) fn update_trespass(&mut self) {
        let crate::engine::Location::Interior(cell) = self.location else {
            self.crime.trespass = None;
            return;
        };
        if !self.trespassing() || self.player_dead() {
            self.crime.trespass = None;
            return;
        }
        if self.crime.trespass.is_some_and(|t| t.cell != cell) {
            self.crime.trespass = None;
        }
        if self.crime.trespass.is_some_and(|t| t.level >= 2) {
            return;
        }
        let now = self.scripts.real_time;
        if self.crime.trespass.is_some_and(|t| now < t.next) {
            return;
        }
        let in_cell = |r: FormId| self.actor_cells.get(&r).is_some_and(|k| *k == crate::render::CellKey::Interior(cell));
        let warner = self
            .detection
            .of_player
            .iter()
            .filter(|&(&r, &v)| v > 0.0 && in_cell(r) && self.actor_ref(r).is_some_and(|a| !a.dead && a.combat.is_none()) && self.sit_sleep_state(r, true) != 3.0)
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(&r, _)| r);
        let Some(warner) = warner else { return };
        let off_limits = self.lo.get(cell).is_some_and(|r| r.flags() & CELL_OFF_LIMITS != 0);
        let level = match self.crime.trespass {
            _ if off_limits => 2,
            None => 0,
            Some(t) => t.level + 1,
        };
        let wait = crate::ai::combat::gmst_f32(&self.lo, "fAITrespassWarningTimer", 5.0) as f64;
        self.crime.trespass = Some(Trespass { cell, level, next: now + wait });
        log::info!("{warner} finds the player trespassing in {cell}: warning level {level}");
        self.bark(warner, b"TRES");
        if level >= 2 {
            let owner = crate::ai::furniture::owner_of(&self.lo, cell);
            let faction = owner.filter(|&o| self.lo.tag_of(o).is_some_and(|t| t.0 == *b"FACT"));
            let victim = owner.filter(|_| faction.is_none()).and_then(|o| self.npc_refs_index().get(&o).copied());
            self.commit_crime(CrimeType::Trespass, victim, faction, 0);
        }
    }

    /// The player's trespass warning level (`GetTrespassWarningLevel`).
    pub fn trespass_warning_level(&self) -> i32 {
        self.crime.trespass.map_or(0, |t| t.level)
    }

    /// The bounties owed, for the console.
    pub fn describe_bounties(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .crime
            .bounties
            .iter()
            .filter(|(_, b)| b.total() > 0 || b.infamy_violent + b.infamy_nonviolent > 0)
            .map(|(&f, b)| format!("{} {f}: {} violent, {} nonviolent (infamy {} / {})", self.form_name(f), b.violent, b.nonviolent, b.infamy_violent, b.infamy_nonviolent))
            .collect();
        if out.is_empty() {
            out.push("no bounty".into());
        }
        out
    }
}

/// Gold (`Gold001`).
pub const GOLD: FormId = FormId(0xF);
