//! Crime: the player's crimes seen by actors who report them to their crime
//! faction (`CRIF`), which adds crime gold (a bounty) from its crime values
//! (`FACT` `CRVA`). Theft, assault and murder are crimes so far; scripts and
//! conditions read and change the gold (`GetCrimeGold`, `ModCrimeGold`,
//! `PlayerPayCrimeGold`...).

use std::collections::{BTreeMap, HashMap, HashSet};

use esp::{FormId, LoadOrder};
use glam::Vec3;

use crate::engine::{Engine, PLAYER_REF};

/// Crime types, as the engine numbers them (`GetCrime`, CommonLibSSE
/// `PackageNS::CRIME_TYPE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // pickpocketing, trespass, escape and werewolves aren't committed yet
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

/// Who witnesses a crime stands in for detection, which isn't implemented;
/// none of these has a source. How far a witness sees the player: combat's
/// detection distance (itself unsourced), a third of it while the player
/// sneaks (invented).
const WITNESS_DISTANCE: f32 = crate::ai::combat::DETECT_DISTANCE;
/// Within this the witness notices whichever way it faces; beyond it the
/// player must be ahead of it (within `WITNESS_HALF_ANGLE`). Invented.
const WITNESS_NEAR: f32 = 200.0;
const WITNESS_HALF_ANGLE: f32 = 95.0;

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
}

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
    /// goes towards it and it is cleared. Stolen items aren't tracked, and there
    /// is no jail yet.
    pub fn pay_crime_gold(&mut self, faction: FormId, remove_stolen: bool, go_to_jail: bool) -> i32 {
        let owed = self.bounty(faction).total();
        let paid = self.remove_item(PLAYER_REF, GOLD, owed, None);
        if let Some(b) = self.crime.bounties.get_mut(&faction) {
            b.violent = 0;
            b.nonviolent = 0;
        }
        log::info!("player pays {paid} of {owed} crime gold to {faction} (remove stolen {remove_stolen}, jail {go_to_jail}: neither done)");
        paid
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

    /// Living actors who see the player now: within sight (less while
    /// sneaking), ahead of them unless close, nothing in between.
    fn witnesses(&self) -> Vec<FormId> {
        let eye = self.player.eye();
        let reach = if self.player.sneaking { WITNESS_DISTANCE / 3.0 } else { WITNESS_DISTANCE };
        let cos_half = WITNESS_HALF_ANGLE.to_radians().cos();
        let mut out = Vec::new();
        for a in self.cells.values().flat_map(|rt| &rt.actors) {
            if a.dead || a.bleeding.is_some() || self.is_disabled(a.ref_id) {
                continue;
            }
            // Eye height as for finding bodies (no source).
            let from = a.pos + Vec3::Z * 110.0 * a.scale;
            let to = eye - from;
            let dist = to.length().max(1.0);
            if dist > reach {
                continue;
            }
            let facing = Vec3::new(a.heading.sin(), a.heading.cos(), 0.0);
            let flat = Vec3::new(to.x, to.y, 0.0).normalize_or_zero();
            if dist > WITNESS_NEAR && facing.dot(flat) < cos_half {
                continue;
            }
            let clear = match self.physics.raycast_excluding(from, to / dist, (dist - 30.0).max(0.0), a.ref_id) {
                Some((_, owner)) => owner == Some(PLAYER_REF),
                None => true,
            };
            if clear {
                out.push(a.ref_id);
            }
        }
        out
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
