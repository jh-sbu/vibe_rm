//! Fighting and dying: who is hostile to whom, closing in and attacking through
//! the race's attack data, hits landing on the graph's `HitFrame`, health, death.

use std::collections::HashMap;
use std::sync::Arc;

use esp::{FormId, LoadOrder};
use glam::Vec3;

use super::{ActorRuntime, State, World, uniform};
use crate::engine::{Engine, PLAYER_REF};
use crate::world::ragdoll::RagdollPose;

/// How far actors notice enemies (game units), and how often they look.
const DETECT_DISTANCE: f32 = 1400.0;
const DETECT_INTERVAL: f32 = 1.0;
/// Combat ends when the target gets this far away.
pub(crate) const LOSE_DISTANCE: f32 = 4000.0;
/// Base melee reach (`fCombatDistance`), scaled by the weapon's reach.
const COMBAT_DISTANCE: f32 = 141.0;
/// Seconds an attack keeps the attacker in place.
const SWING_TIME: f32 = 1.1;

/// Block chance per incoming swing for each point of combat style defensiveness
/// (with a shield), and times a second an actor waiting to strike raises its
/// guard at full defensiveness. Tuned by eye: the game's own rule isn't documented.
const BLOCK_CHANCE: f32 = 2.0;
const GUARD_RATE: f32 = 0.6;

/// Chance an attack is a power attack per point of combat style offensiveness
/// (at most 60%). Tuned by eye, like the guard rates.
const POWER_ATTACK_CHANCE: f32 = 0.6;

/// Biped slot 39: shields.
const SHIELD_SLOT: u32 = 1 << 9;

/// Seconds an essential actor stays down before getting back up.
const BLEEDOUT_TIME: f32 = 12.0;

/// Attack data flags (`ATKD`).
const ATK_IGNORE_WEAPON: u32 = 0x1;
const ATK_BASH: u32 = 0x2;
const ATK_POWER: u32 = 0x4;

/// An attack a race can make (`ATKD` + `ATKE`).
#[derive(Debug, Clone)]
pub struct Attack {
    pub event: String,
    pub damage_mult: f32,
    pub chance: f32,
    pub flags: u32,
    /// Degrees either side of straight ahead a hit lands in.
    pub strike_angle: f32,
    pub stagger: f32,
}

/// What an actor fights with.
#[derive(Debug, Clone, Default)]
pub struct CombatStats {
    pub max_health: f32,
    pub attacks: Vec<Attack>,
    pub unarmed_damage: f32,
    pub unarmed_reach: f32,
    /// AI data: 0 unaggressive, 1 aggressive (attacks enemies), 2 very aggressive
    /// (and neutrals), 3 frenzied (anyone).
    pub aggression: u8,
    /// Aggro radius behaviour (`AIDT`): attacks the player coming this close.
    pub aggro_attack: Option<f32>,
    /// Can't die (bleeds out instead); protected ones only die by the player's hand.
    pub essential: bool,
    pub protected: bool,
    pub factions: Vec<FormId>,
    /// Light and heavy armor skills: worn armor protects more with them.
    pub armor_skills: [f32; 2],
    /// Block skill, and how readily it raises its guard (combat style `CSGD`
    /// defensive multiplier).
    pub block_skill: f32,
    pub defensive: f32,
    /// How readily it attacks, and power attacks (`CSGD` offensive multiplier),
    /// and how much more (or less) it power attacks a target behind its guard
    /// (`CSME` power attack blocking multiplier).
    pub offensive: f32,
    pub power_vs_guard: f32,
}

/// A weapon's base damage (`DATA`).
fn weapon_damage(lo: &LoadOrder, weapon: FormId) -> f32 {
    lo.get(weapon).and_then(|r| r.get(b"DATA").filter(|d| d.len() >= 10).map(|d| u16::from_le_bytes([d[8], d[9]]) as f32)).unwrap_or(0.0)
}

fn f32_at(d: &[u8], o: usize) -> f32 {
    d.get(o..o + 4).map_or(0.0, |b| f32::from_le_bytes(b.try_into().unwrap()))
}

/// An NPC's `tag` subrecord, from its template when it takes that part from it
/// (template flag `use_template`) or lacks it.
fn npc_field(lo: &LoadOrder, npc: FormId, tag: &[u8; 4], use_template: u16) -> Option<Vec<u8>> {
    npc_source(lo, npc, tag, use_template).and_then(|r| r.get(tag).map(|d| d.to_vec()))
}

/// The NPC record `npc_field` takes `tag` from (to resolve form ids it holds).
fn npc_source<'a>(lo: &'a LoadOrder, npc: FormId, tag: &[u8; 4], use_template: u16) -> Option<esp::LoadedRecord<'a>> {
    let mut id = npc;
    for _ in 0..8 {
        let rec = lo.get(id)?;
        let tpl_flags = rec.get(b"ACBS").filter(|d| d.len() >= 20).map_or(0, |d| u16::from_le_bytes([d[18], d[19]]));
        let templated = tpl_flags & use_template != 0 && rec.get(b"TPLT").is_some();
        if rec.tag().0 == *b"NPC_" && !templated && rec.get(tag).is_some() {
            return Some(rec);
        }
        let t = rec.get(b"TPLT").filter(|d| d.len() >= 4)?;
        id = rec.fid(FormId(u32::from_le_bytes(t[0..4].try_into().unwrap())));
        id = first_of_leveled(lo, id);
    }
    None
}

/// Leveled NPC lists (nested) down to their first entry: as good as any for the
/// stats they share.
fn first_of_leveled(lo: &LoadOrder, mut id: FormId) -> FormId {
    for _ in 0..8 {
        let Some(l) = lo.get(id).filter(|r| r.tag().0 == *b"LVLN") else { break };
        let Some(first) = l.subrecords().find(|s| s.tag.0 == *b"LVLO" && s.data.len() >= 8) else { break };
        id = l.fid(first.form_id(4));
    }
    id
}

/// An NPC's factions: its own, or its template's when it takes them from one
/// (template flag 0x4) or has none.
fn npc_factions(lo: &LoadOrder, npc: FormId) -> Vec<FormId> {
    let mut id = npc;
    for _ in 0..8 {
        let Some(rec) = lo.get(id) else { break };
        let own: Vec<FormId> = rec.subrecords().filter(|s| s.tag.0 == *b"SNAM" && s.data.len() >= 5).map(|s| rec.fid(s.form_id(0))).collect();
        let tpl_flags = rec.get(b"ACBS").filter(|d| d.len() >= 20).map_or(0, |d| u16::from_le_bytes([d[18], d[19]]));
        let template = rec.get(b"TPLT").filter(|d| d.len() >= 4).map(|d| rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap()))));
        match template {
            Some(t) if tpl_flags & 0x4 != 0 || own.is_empty() => {
                id = t;
                id = first_of_leveled(lo, id);
            }
            _ => return own,
        }
    }
    Vec::new()
}

/// Light and heavy armor skills from an NPC's `DNAM` (skills 6 and 5).
fn armor_skills(dnam: &[u8]) -> [f32; 2] {
    [dnam.get(6).copied().unwrap_or(15) as f32, dnam.get(5).copied().unwrap_or(15) as f32]
}

/// A float game setting (`GMST`), or `default` when the plugins lack it.
fn gmst_f32(lo: &LoadOrder, name: &str, default: f32) -> f32 {
    lo.find_editor_id(name).and_then(|id| lo.get(id)).and_then(|r| r.get(b"DATA").filter(|d| d.len() >= 4).map(|d| f32_at(d, 0))).unwrap_or(default)
}

/// An integer game setting (`GMST`), or `default` when the plugins lack it.
fn gmst_i32(lo: &LoadOrder, name: &str, default: i32) -> i32 {
    lo.find_editor_id(name)
        .and_then(|id| lo.get(id))
        .and_then(|r| r.get(b"DATA").filter(|d| d.len() >= 4).map(|d| i32::from_le_bytes(d[0..4].try_into().unwrap())))
        .unwrap_or(default)
}

/// How armor and blocking protect (`GMST`s). Armor: each piece's rating grows with
/// the wearer's skill (to `npc_max` / `player_max` times base at skill 100), the
/// total takes `scaling` percent per point, each piece worn `per_piece` more, up to
/// `max` percent. Blocking: see `Engine::block_share`.
#[derive(Debug, Clone, Copy)]
pub struct CombatSettings {
    shield_base: f32,
    shield_scaling: f32,
    weapon_base: f32,
    weapon_scaling: f32,
    power_mult: f32,
    /// Block chance without a shield, relative to with one.
    weapon_block_chance: f32,
    /// Percent chance an attack that can stagger does.
    stagger_chance: u64,
    scaling: f32,
    per_piece: f32,
    max: f32,
    rating_base: f32,
    npc_max: f32,
    player_max: f32,
    clothing: f32,
}

impl CombatSettings {
    pub fn load(lo: &LoadOrder) -> CombatSettings {
        CombatSettings {
            shield_base: gmst_f32(lo, "fShieldBaseFactor", 0.45),
            shield_scaling: gmst_f32(lo, "fShieldScalingFactor", 0.2),
            weapon_base: gmst_f32(lo, "fBlockWeaponBase", 0.3),
            weapon_scaling: gmst_f32(lo, "fBlockWeaponScaling", 0.2),
            power_mult: gmst_f32(lo, "fBlockPowerAttackMult", 0.66),
            weapon_block_chance: gmst_f32(lo, "fCombatBlockChanceWeaponMult", 0.25),
            stagger_chance: gmst_i32(lo, "iStaggerAttackChance", 50).clamp(0, 100) as u64,
            scaling: gmst_f32(lo, "fArmorScalingFactor", 0.12),
            per_piece: gmst_f32(lo, "fArmorBaseFactor", 0.03) * 100.0,
            max: gmst_f32(lo, "fMaxArmorRating", 80.0),
            rating_base: gmst_f32(lo, "fArmorRatingBase", 1.0),
            npc_max: gmst_f32(lo, "fArmorRatingMax", 2.5),
            player_max: gmst_f32(lo, "fArmorRatingPCMax", 1.4),
            clothing: gmst_f32(lo, "fClothingArmorScale", 1.0),
        }
    }
}

/// What a set of worn items protects: the total armor rating (as the inventory
/// shows it) and the share of a blow it takes away.
#[derive(Debug, Clone, Copy, Default)]
pub struct Protection {
    pub rating: f32,
    pub pieces: u32,
    pub reduction: f32,
}

/// The protection of `worn` items for a wearer with these light / heavy armor
/// skills: each light or heavy piece (shields too) rates
/// `ceil(base x (1 + k x skill / 100))`, with k 1.5 for NPCs and 0.4 for the
/// player; clothing rates as it is and doesn't count as a piece.
pub fn protection(lo: &LoadOrder, set: &CombatSettings, worn: &[FormId], skills: [f32; 2], player: bool) -> Protection {
    let k = if player { set.player_max } else { set.npc_max } - set.rating_base;
    let mut p = Protection::default();
    for &f in worn {
        let Some(rec) = lo.get(f).filter(|r| r.tag().0 == *b"ARMO") else { continue };
        // DNAM: rating x 100. Armor type: BOD2 (slots, type) or the older BODT
        // (slots, flags, type).
        let base = rec.get(b"DNAM").filter(|d| d.len() >= 4).map_or(0, |d| i32::from_le_bytes(d[0..4].try_into().unwrap())) as f32 / 100.0;
        let kind = match (rec.get(b"BOD2"), rec.get(b"BODT")) {
            (Some(d), _) if d.len() >= 8 => u32::from_le_bytes(d[4..8].try_into().unwrap()),
            (_, Some(d)) if d.len() >= 12 => u32::from_le_bytes(d[8..12].try_into().unwrap()),
            _ => 2,
        };
        match kind {
            0 | 1 => {
                let skill = skills[kind as usize];
                p.rating += (base * (set.rating_base + k * skill / 100.0)).ceil();
                p.pieces += 1;
            }
            _ => p.rating += base * set.clothing,
        }
    }
    p.reduction = ((p.rating * set.scaling + p.pieces as f32 * set.per_piece).min(set.max) / 100.0).clamp(0.0, 1.0);
    p
}

impl CombatStats {
    pub fn of(e: &Engine, npc: FormId, race: FormId) -> CombatStats {
        let lo = &e.lo;
        let mut s = CombatStats { max_health: 50.0, unarmed_damage: 4.0, unarmed_reach: 96.0, aggression: 0, ..Default::default() };
        if let Some(race) = lo.get(race) {
            if let Some(d) = race.get(b"DATA") {
                s.max_health = f32_at(d, 36);
                s.unarmed_damage = f32_at(d, 96).max(1.0);
                s.unarmed_reach = f32_at(d, 100).max(48.0);
            }
            let mut pending: Option<(f32, f32, u32, f32, f32)> = None;
            for sr in race.subrecords() {
                match &sr.tag.0 {
                    b"ATKD" if sr.data.len() >= 28 => {
                        let d = sr.data;
                        pending = Some((f32_at(d, 0), f32_at(d, 4), u32::from_le_bytes(d[12..16].try_into().unwrap()), f32_at(d, 20), f32_at(d, 24)));
                    }
                    b"ATKE" => {
                        if let Some((damage_mult, chance, flags, strike_angle, stagger)) = pending.take() {
                            let event = sr.zstring();
                            // Attacks for situations not handled yet: sprinting, on
                            // horseback, with the left hand, dual wielding, unarmed.
                            let lower = event.to_ascii_lowercase();
                            let situational = ["sprint", "_mc", "lefthand", "dualwield", "h2h"].iter().any(|k| lower.contains(k));
                            if !event.is_empty() && !situational {
                                s.attacks.push(Attack { event, damage_mult, chance, flags, strike_angle, stagger });
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        // Health from the NPC's stats (DNAM: skills, then health / magicka / stamina).
        // ACBS flags: 0x2 essential, 0x800 protected.
        let acbs = npc_field(lo, npc, b"ACBS", 0x80).filter(|d| d.len() >= 4).map_or(0, |d| u32::from_le_bytes(d[0..4].try_into().unwrap()));
        s.essential = acbs & 0x2 != 0;
        s.protected = acbs & 0x800 != 0;
        // Template flags: 0x2 stats, 0x10 AI data.
        if let Some(d) = npc_field(lo, npc, b"DNAM", 0x2).filter(|d| d.len() >= 38) {
            s.max_health += u16::from_le_bytes([d[36], d[37]]) as f32;
            s.armor_skills = armor_skills(&d);
            s.block_skill = d[3] as f32;
        }
        // Combat style (with the AI data, template flag 0x10), else the default one.
        let style = npc_source(lo, npc, b"ZNAM", 0x10)
            .and_then(|r| r.get(b"ZNAM").filter(|d| d.len() >= 4).map(|d| r.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap())))))
            .or_else(|| lo.find_editor_id("DefaultCombatstyle"));
        let style = style.and_then(|st| lo.get(st));
        let csgd = style.as_ref().and_then(|r| r.get(b"CSGD")).filter(|d| d.len() >= 8);
        s.offensive = csgd.map_or(0.25, |d| f32_at(d, 0));
        s.defensive = csgd.map_or(0.25, |d| f32_at(d, 4));
        s.power_vs_guard = style.as_ref().and_then(|r| r.get(b"CSME")).filter(|d| d.len() >= 12).map_or(1.0, |d| f32_at(d, 8));
        s.max_health = s.max_health.max(5.0);
        if let Some(d) = npc_field(lo, npc, b"AIDT", 0x10).filter(|d| d.len() >= 20) {
            s.aggression = d[0];
            if d[6] != 0 {
                s.aggro_attack = Some(u32::from_le_bytes(d[16..20].try_into().unwrap()) as f32).filter(|r| *r > 0.0);
            }
        }
        s.factions = npc_factions(lo, npc);
        s
    }
}

/// An actor's fight with one target.
#[derive(Debug, Clone)]
pub struct Combat {
    pub target: FormId,
    path: Vec<Vec3>,
    next: usize,
    repath: f32,
    /// Seconds until it may attack again, and left of the swing it is in.
    cooldown: f32,
    swing: f32,
    /// The attack being made (index into the stats' attacks), and whether it
    /// has struck (one hit a swing).
    pub attack: Option<usize>,
    pub struck: bool,
    /// Seconds left holding its guard up (blocking), and whether the graph has
    /// raised it (it may be busy finishing a swing or a stagger first).
    pub guard: f32,
    guard_shown: bool,
    /// The target has its guard up (set by the engine each frame).
    pub(crate) target_guarding: bool,
}

impl Combat {
    pub fn new(target: FormId) -> Combat {
        Combat { target, path: Vec::new(), next: 0, repath: 0.0, cooldown: 0.8, swing: 0.0, attack: None, struck: false, guard: 0.0, guard_shown: false, target_guarding: false }
    }

    pub fn swinging(&self) -> bool {
        self.swing > 0.0
    }

    /// The graph wouldn't take the attack (busy flinching, say): no swing; try
    /// again shortly.
    pub fn refused(&mut self) {
        self.swing = 0.0;
        self.attack = None;
        self.cooldown = 0.3;
    }
}

impl ActorRuntime {
    /// How far its attacks reach.
    pub fn reach(&self) -> f32 {
        let r = if self.weapon_reach > 0.0 { COMBAT_DISTANCE * self.weapon_reach } else { self.stats.unarmed_reach };
        r * self.scale
    }

    fn run_speed(&self) -> f32 {
        let walk = self.walk_speed();
        self.moves.map(|(m, _)| m.run).filter(|r| *r > walk).unwrap_or(walk * 3.0)
    }

    /// Raise its guard for `secs` (the AI step has the graph raise it), or lower it.
    pub(crate) fn set_guard(&mut self, secs: f32) {
        let Some(c) = self.combat.as_mut() else { return };
        c.guard = secs.max(0.0);
        if c.guard <= 0.0 && std::mem::take(&mut c.guard_shown)
            && let Some(g) = self.graph.as_mut()
        {
            g.send_event("blockStop");
            g.set_variable("IsBlocking", 0.0);
        }
    }

    /// Its guard is up (raised by the graph): blows from ahead are blocked.
    pub fn guarding(&self) -> bool {
        self.combat.as_ref().is_some_and(|c| c.guard > 0.0 && c.guard_shown)
    }

    /// Fight on: close in on the target at a run, face it, attack when in reach.
    /// Returns the graph event of an attack started this frame.
    pub(crate) fn combat_step(&mut self, dt: f32, w: &mut World, target: Vec3) -> Option<String> {
        let reach = self.reach();
        let run = self.run_speed();
        let pos = self.pos;
        let attacks: Vec<(usize, f32)> = self.stats.attacks.iter().enumerate().filter(|(_, a)| a.flags & ATK_BASH == 0).map(|(i, a)| (i, a.chance.max(0.05))).collect();
        // Humanoids fight with their weapon out (the draw may not have been taken,
        // say mid-flinch: ask again).
        let humanoid = self.graph.as_ref().is_some_and(|g| g.project().humanoid());
        if humanoid && !self.drawn
            && let Some(g) = self.graph.as_mut()
        {
            self.drawn = g.send_event("WeapEquip");
        }
        let unready = humanoid && !self.weapon_out && self.weapon_reach > 0.0;
        let c = self.combat.as_mut()?;
        c.cooldown -= dt;
        c.repath -= dt;
        if unready {
            c.cooldown = c.cooldown.max(0.2);
        }
        let to = target - pos;
        let dist = to.truncate().length();
        if c.swing > 0.0 {
            c.swing -= dt;
            self.speed = 0.0;
            self.state = State::Idle(1.0);
            self.turn_towards(to.normalize_or_zero(), dt * 0.5);
            return None;
        }
        if c.guard > 0.0 {
            // Guard up: stand facing the target (the engine lowers it). Ask the
            // graph until it takes it.
            if !c.guard_shown {
                let shown = self.graph_event("blockStart", &mut w.clips);
                if let Some(c) = self.combat.as_mut() {
                    c.guard_shown = shown;
                }
                if shown && let Some(g) = self.graph.as_mut() {
                    g.set_variable("IsBlocking", 1.0);
                }
            }
            self.speed = 0.0;
            self.state = State::Idle(1.0);
            self.turn_towards(to.normalize_or_zero(), dt);
            return None;
        }
        if dist > reach * 0.85 {
            // Close in along a path, refreshed as the target moves.
            if c.repath <= 0.0 || c.next >= c.path.len() {
                c.path = w.nav.find_path(pos, target).unwrap_or_else(|| vec![target]);
                c.next = 0;
                c.repath = 0.5;
            }
            while c.next < c.path.len() && (c.path[c.next] - pos).truncate().length() < 16.0 {
                c.next += 1;
            }
            let way = c.path.get(c.next).copied().unwrap_or(target);
            let d = (way - pos).truncate();
            let remaining = self.turn_towards(d.normalize_or_zero().extend(0.0), dt);
            let speed = run * remaining.cos().max(0.0).powi(2);
            self.speed = speed;
            self.state = State::Walk { path: Vec::new(), next: 0, budget: 1.0, to_seat: false };
            let fwd = Vec3::new(self.heading.sin(), self.heading.cos(), 0.0);
            let mut p = pos + fwd * (speed * dt).min(d.length().max(1.0));
            // Off the navmesh, keep level rather than heading for the target's height.
            let nz = w.nav.height_at(Vec3::new(p.x, p.y, pos.z));
            p.z = nz.unwrap_or(pos.z);
            log::trace!("{} chases: at {pos:?} target {target:?} way {way:?} nav z {nz:?}", self.ref_id);
            self.pos = p;
            return None;
        }
        // In reach: stand, face and strike.
        self.speed = 0.0;
        self.state = State::Idle(1.0);
        let cooldown = c.cooldown;
        let target_guarding = c.target_guarding;
        let off = self.turn_towards(to.normalize_or_zero(), dt).abs();
        if cooldown > 0.0 || off > 0.35 || attacks.is_empty() {
            return None;
        }
        // A power attack now and then, as offensive as its combat style (more or
        // less so against a raised guard), else a basic one; then which, by
        // their chances.
        let is_power = |i: usize| self.stats.attacks[i].flags & ATK_POWER != 0;
        let power_chance = (self.stats.offensive * POWER_ATTACK_CHANCE * if target_guarding { self.stats.power_vs_guard } else { 1.0 }).clamp(0.0, 0.6);
        let want_power = uniform(w.rand, 0.0, 1.0) < power_chance;
        let kind: Vec<(usize, f32)> = attacks.iter().copied().filter(|&(i, _)| is_power(i) == want_power).collect();
        let pool = if kind.is_empty() { &attacks } else { &kind };
        let total: f32 = pool.iter().map(|a| a.1).sum();
        let mut roll = uniform(w.rand, 0.0, total);
        let mut pick = pool[0].0;
        for (i, ch) in pool {
            if roll < *ch {
                pick = *i;
                break;
            }
            roll -= ch;
        }
        let power = is_power(pick);
        let c = self.combat.as_mut()?;
        c.attack = Some(pick);
        c.struck = false;
        c.swing = SWING_TIME;
        // A power attack takes longer to recover from.
        c.cooldown = SWING_TIME + uniform(w.rand, 0.3, 1.4) + if power { uniform(w.rand, 0.5, 1.0) } else { 0.0 };
        Some(self.stats.attacks[pick].event.clone())
    }
}

/// A swing at its hit frame: who swung (with which attack) at whom, from where.
pub(crate) struct Swing {
    pub attacker: FormId,
    pub target: FormId,
    pub attack: Option<usize>,
    pub pos: Vec3,
    pub heading: f32,
}

impl Engine {
    /// Damage of an attack by an actor: its weapon's (unless the attack ignores
    /// it), else its race's unarmed damage; scaled by the attack's multiplier.
    pub(crate) fn attack_damage(&self, a: &ActorRuntime, attack: Option<&Attack>) -> f32 {
        let ignore = attack.is_some_and(|x| x.flags & ATK_IGNORE_WEAPON != 0);
        let weapon = self.inventories.get(&a.ref_id).and_then(|i| i.weapon(&self.lo));
        let base = match weapon.filter(|_| !ignore).and_then(|w| self.lo.get(w)) {
            Some(rec) => rec.get(b"DATA").filter(|d| d.len() >= 10).map_or(4.0, |d| u16::from_le_bytes([d[8], d[9]]) as f32),
            None => a.stats.unarmed_damage,
        };
        let mult = attack.map_or(1.0, |x| x.damage_mult.max(0.1)) * if attack.is_some_and(|x| x.flags & ATK_POWER != 0) { 1.5 } else { 1.0 };
        log::debug!("{}: {} base {base} x {mult}", a.ref_id, attack.map_or("-", |x| x.event.as_str()));
        base * mult
    }

    /// Whether `a` (an NPC with these stats) would attack `b` on sight. Aggressive
    /// actors attack their enemies, and the player unless they keep the law (belong
    /// to a faction that tracks crime: townsfolk, guards); very aggressive ones
    /// neutrals too; frenzied ones anyone. Allies and friends are left alone.
    fn hostile_to(&mut self, a: &CombatStats, b_factions: &[FormId], b_is_player: bool) -> bool {
        let reaction = self.faction_reaction(&a.factions, b_factions);
        if matches!(reaction, Some(2 | 3)) && a.aggression < 3 {
            return false;
        }
        match a.aggression {
            0 => false,
            1 => reaction == Some(1) || (b_is_player && !self.law_abiding(&a.factions)),
            2 => true,
            _ => true,
        }
    }

    /// In a faction that tracks crime (`DATA` flag 0x40).
    fn law_abiding(&self, factions: &[FormId]) -> bool {
        factions.iter().any(|&f| self.lo.get(f).and_then(|r| r.get(b"DATA").and_then(|d| d.first().copied())).is_some_and(|flags| flags & 0x40 != 0))
    }

    /// The strongest reaction (`XNAM` group combat reaction: 0 neutral, 1 enemy,
    /// 2 ally, 3 friend) any of `ours` has towards any of `theirs`; friends and allies
    /// win over enemies.
    fn faction_reaction(&mut self, ours: &[FormId], theirs: &[FormId]) -> Option<u32> {
        if ours.iter().any(|f| theirs.contains(f)) {
            return Some(2);
        }
        let mut best: Option<u32> = None;
        for &f in ours {
            let rel = self.faction_relations.entry(f).or_insert_with(|| {
                let Some(rec) = self.lo.get(f) else { return Arc::new(Vec::new()) };
                Arc::new(
                    rec.subrecords()
                        .filter(|s| s.tag.0 == *b"XNAM" && s.data.len() >= 12)
                        .map(|s| (rec.fid(s.form_id(0)), u32::from_le_bytes(s.data[8..12].try_into().unwrap())))
                        .collect(),
                )
            });
            for &(other, r) in rel.iter() {
                if theirs.contains(&other) {
                    best = Some(match (best, r) {
                        (Some(2 | 3), _) | (_, 2 | 3) => best.unwrap_or(r).max(r).max(2),
                        (b, r) => b.map_or(r, |b| b.max(r)),
                    });
                }
            }
        }
        best
    }

    /// Start (or switch) an actor's fight with `target`: weapons out.
    pub fn start_combat(&mut self, actor: FormId, target: FormId) -> bool {
        if actor == target {
            return false;
        }
        let Some(key) = self.actor_cells.get(&actor).copied() else { return false };
        let Some(a) = self.cells.get_mut(&key).and_then(|rt| rt.actors.iter_mut().find(|a| a.ref_id == actor)) else { return false };
        if a.dead || a.combat.as_ref().is_some_and(|c| c.target == target) {
            return false;
        }
        a.interrupt(&mut self.furniture);
        a.combat = Some(Combat::new(target));
        let humanoid = a.graph.as_ref().is_some_and(|g| g.project().humanoid());
        log::info!("{actor} attacks {target}");
        if humanoid {
            self.draw_weapon(actor, true);
        } else if let Some(g) = self.cells.get_mut(&key).and_then(|rt| rt.actors.iter_mut().find(|a| a.ref_id == actor)).and_then(|a| a.graph.as_mut()) {
            g.send_event("combatStanceStart");
        }
        true
    }

    /// Stop fighting: weapons away, back to its packages.
    pub fn end_combat(&mut self, actor: FormId) {
        let Some(key) = self.actor_cells.get(&actor).copied() else { return };
        let Some(a) = self.cells.get_mut(&key).and_then(|rt| rt.actors.iter_mut().find(|a| a.ref_id == actor)) else { return };
        if a.combat.is_none() {
            return;
        }
        a.set_guard(0.0);
        a.combat = None;
        a.halt(2.0);
        a.next_eval = 0.0;
        let humanoid = a.graph.as_ref().is_some_and(|g| g.project().humanoid());
        if humanoid {
            self.draw_weapon(actor, false);
        } else if let Some(g) = self.cells.get_mut(&key).and_then(|rt| rt.actors.iter_mut().find(|a| a.ref_id == actor)).and_then(|a| a.graph.as_mut()) {
            g.send_event("combatStanceStop");
        }
        log::info!("{actor} stops fighting");
    }

    /// Living actors look around now and then for enemies to attack.
    pub(crate) fn detect_enemies(&mut self, dt: f32) {
        let player = self.player.position;
        let player_factions = self.player_factions();
        let mut found: Vec<(FormId, FormId)> = Vec::new();
        let others: Vec<(FormId, Vec3, Vec<FormId>)> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| !a.dead && a.bleeding.is_none())
            .map(|a| (a.ref_id, a.pos, a.stats.factions.clone()))
            .collect();
        let mut lookers: Vec<(FormId, Vec3, Arc<CombatStats>)> = Vec::new();
        for rt in self.cells.values_mut() {
            for a in rt.actors.iter_mut().filter(|a| !a.dead && a.bleeding.is_none() && a.combat.is_none() && (a.stats.aggression > 0 || a.stats.aggro_attack.is_some())) {
                a.detect_in -= dt;
                if a.detect_in <= 0.0 {
                    a.detect_in = DETECT_INTERVAL;
                    lookers.push((a.ref_id, a.pos, a.stats.clone()));
                }
            }
        }
        for (r, pos, stats) in lookers {
            let mut best: Option<(f32, FormId)> = None;
            let d = pos.distance(player);
            let aggro = stats.aggro_attack.is_some_and(|r| d < r) && !self.law_abiding(&stats.factions) && !matches!(self.faction_reaction(&stats.factions, &player_factions), Some(2 | 3));
            if d < DETECT_DISTANCE && !self.player_dead() && (aggro || self.hostile_to(&stats, &player_factions, true)) {
                best = Some((d, PLAYER_REF));
            }
            for (o, opos, of) in &others {
                let d = pos.distance(*opos);
                if *o == r || d > DETECT_DISTANCE || best.is_some_and(|b| b.0 <= d) {
                    continue;
                }
                if self.hostile_to(&stats, of, false) {
                    best = Some((d, *o));
                }
            }
            if let Some((_, t)) = best {
                found.push((r, t));
            }
        }
        for (a, t) in found {
            self.start_combat(a, t);
        }
    }

    pub(crate) fn player_factions(&self) -> Vec<FormId> {
        self.npc_factions(FormId(0x7)).into_iter().map(|(f, _)| f).chain(std::iter::once(FormId(0xDB1))).collect()
    }

    /// Resolve swings that reached their hit frame: the target must be within reach
    /// and the attack's strike angle.
    pub(crate) fn resolve_swings(&mut self, swings: Vec<Swing>) {
        for s in swings {
            let Some(a) = self.actor_cells.get(&s.attacker).and_then(|k| self.cells.get(k)).and_then(|rt| rt.actors.iter().find(|a| a.ref_id == s.attacker)) else { continue };
            let attack = s.attack.and_then(|i| a.stats.attacks.get(i)).cloned();
            let damage = self.attack_damage(a, attack.as_ref());
            let reach = a.reach();
            let strike_angle = attack.as_ref().map_or(35.0, |x| x.strike_angle);
            let stagger = attack.as_ref().map_or(0.0, |x| x.stagger);
            let target_pos = if s.target == PLAYER_REF { Some(self.player.position - Vec3::Z * 60.0) } else { self.actor_pose(s.target).map(|p| p.0) };
            let Some(tp) = target_pos else { continue };
            let to = (tp - s.pos).truncate();
            let dist = to.length();
            let fwd = glam::Vec2::new(s.heading.sin(), s.heading.cos());
            let angle = fwd.angle_to(to.normalize_or_zero()).abs().to_degrees();
            if dist > reach * 1.3 || angle > strike_angle.max(25.0) {
                log::debug!("{} misses {} ({dist:.0} / {reach:.0} units, {angle:.0} deg)", s.attacker, s.target);
                continue;
            }
            let power = attack.as_ref().is_some_and(|x| x.flags & ATK_POWER != 0);
            self.hit(s.target, s.attacker, damage, power, stagger);
        }
    }

    /// A blow landing: armor takes its share, then the target's guard if it is
    /// blocking towards the attacker. Blocked blows don't make it flinch, but a
    /// blocked power attack breaks its guard with a stagger.
    pub(crate) fn hit(&mut self, target: FormId, attacker: FormId, damage: f32, power: bool, stagger: f32) {
        let mut damage = self.after_armor(target, damage);
        // Attacks that can stagger do so only some of the time (iStaggerAttackChance).
        let staggers = (self.rand() % 100) < self.combat_settings().stagger_chance;
        let mut stagger = if staggers { stagger } else { 0.0 };
        if let Some(share) = self.block_share(target, attacker, power) {
            log::info!("{target} blocks {attacker}{}: {:.0}% of {damage:.0} stopped", if power { "'s power attack" } else { "" }, share * 100.0);
            damage *= 1.0 - share;
            // A power attack breaks the guard.
            stagger = if power { 0.5f32.max(stagger) } else { 0.0 };
            if target != PLAYER_REF
                && let Some(a) = self.actor_mut(target)
            {
                if power {
                    a.set_guard(0.0);
                } else if let Some(g) = a.graph.as_mut() {
                    g.send_event("blockHitStart");
                }
            }
        }
        self.damage(target, damage, Some(attacker), stagger);
    }

    /// Armor and block game settings.
    pub(crate) fn combat_settings(&self) -> CombatSettings {
        *self.combat_settings.get_or_init(|| CombatSettings::load(&self.lo))
    }

    fn actor_mut(&mut self, actor: FormId) -> Option<&mut ActorRuntime> {
        let key = self.actor_cells.get(&actor).copied()?;
        self.cells.get_mut(&key)?.actors.iter_mut().find(|a| a.ref_id == actor)
    }

    fn actor_ref(&self, actor: FormId) -> Option<&ActorRuntime> {
        self.actor_cells.get(&actor).and_then(|k| self.cells.get(k)).and_then(|rt| rt.actors.iter().find(|a| a.ref_id == actor))
    }

    /// The weapon the player swings: the best one they carry.
    fn player_weapon(&self) -> Option<FormId> {
        self.inventories
            .get(&PLAYER_REF)
            .and_then(|i| i.items.iter().map(|(f, _)| *f).filter(|f| self.lo.tag_of(*f).map(|t| t.0) == Some(*b"WEAP")).max_by_key(|f| weapon_damage(&self.lo, *f) as u32))
    }

    /// Base damage of what an actor (or the player) strikes with; 0 for creatures
    /// and bare hands.
    fn weapon_base(&self, actor: FormId) -> f32 {
        let weapon = if actor == PLAYER_REF { self.player_weapon() } else { self.inventories.get(&actor).and_then(|i| i.weapon(&self.lo)) };
        weapon.map_or(0.0, |w| weapon_damage(&self.lo, w))
    }

    /// The equipped shield's base armor rating, if any.
    fn shield_rating(&self, actor: FormId) -> Option<f32> {
        let inv = self.inventories.get(&actor)?;
        inv.equipped.iter().find_map(|&f| {
            let r = self.lo.get(f).filter(|r| r.tag().0 == *b"ARMO" && crate::world::inventory::armor_slots(r) & SHIELD_SLOT != 0)?;
            Some(r.get(b"DNAM").filter(|d| d.len() >= 4).map_or(0, |d| i32::from_le_bytes(d[0..4].try_into().unwrap())) as f32 / 100.0)
        })
    }

    /// The share of a blow `target`'s guard stops, if it is blocking and faces
    /// the attacker (within 35 degrees): with a shield 45% + 0.2% per point of
    /// its base rating, with a weapon 30% + 0.2% per point of the attacker's
    /// weapon damage, either scaled by 1 + 1.5 x block skill / 100; 0.66 of that
    /// against power attacks; at most 85% (as measured in the game, UESP).
    fn block_share(&self, target: FormId, attacker: FormId, power: bool) -> Option<f32> {
        let set = self.combat_settings();
        let (pos, facing, skill) = if target == PLAYER_REF {
            if !self.player_blocking {
                return None;
            }
            let f = self.camera.forward();
            let skill = npc_field(&self.lo, FormId(0x7), b"DNAM", 0x2).and_then(|d| d.get(3).copied()).unwrap_or(15) as f32;
            (self.player.position, glam::Vec2::new(f.x, f.y).normalize_or_zero(), skill)
        } else {
            let a = self.actor_ref(target).filter(|a| a.guarding())?;
            (a.pos, glam::Vec2::new(a.heading.sin(), a.heading.cos()), a.stats.block_skill)
        };
        let from = if attacker == PLAYER_REF { self.player.position } else { self.actor_ref(attacker)?.pos };
        let angle = facing.angle_to((from - pos).truncate().normalize_or_zero()).abs().to_degrees();
        if angle > 35.0 {
            log::debug!("{target}'s guard faces away from {attacker} ({angle:.0} deg)");
            return None;
        }
        let skill = 1.0 + 1.5 * skill / 100.0;
        let share = match self.shield_rating(target) {
            Some(rating) => set.shield_base + set.shield_scaling * rating * skill / 100.0,
            None => set.weapon_base + set.weapon_scaling * self.weapon_base(attacker) * skill / 100.0,
        };
        Some((share * if power { set.power_mult } else { 1.0 }).min(0.85))
    }

    /// Guards: actors raise theirs against a swing started at them (`started`:
    /// attacker, target), or now and then while waiting to strike, as their
    /// combat style's defensive side has them (less often without a shield);
    /// they lower it after a while.
    pub(crate) fn update_guards(&mut self, dt: f32, started: &[(FormId, FormId)]) {
        let set = self.combat_settings();
        let mut want: Vec<(FormId, f32)> = Vec::new();
        for &(attacker, target) in started {
            let Some(a) = self.actor_ref(target).filter(|a| a.combat.as_ref().is_some_and(|c| c.guard <= 0.0 && !c.swinging())) else { continue };
            let Some(chance) = self.block_chance(a, &set) else { continue };
            let Some(from) = self.actor_ref(attacker).map(|x| x.pos).or((attacker == PLAYER_REF).then_some(self.player.position)) else { continue };
            let facing = glam::Vec2::new(a.heading.sin(), a.heading.cos());
            if facing.angle_to((from - a.pos).truncate().normalize_or_zero()).abs() > 1.0 {
                continue;
            }
            let roll = (self.rand() % 10_000) as f32 / 10_000.0;
            if roll < chance {
                let secs = SWING_TIME * 0.9 + (self.rand() % 500) as f32 / 1000.0;
                want.push((target, secs));
            }
        }
        // Waiting in reach between its own swings, facing the target: now and
        // then put the guard up.
        let waiting: Vec<(FormId, f32)> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| {
                let Some(c) = a.combat.as_ref().filter(|c| c.guard <= 0.0 && !c.swinging() && c.cooldown > 0.4) else { return false };
                let target = if c.target == PLAYER_REF { Some(self.player.position) } else { self.actor_ref(c.target).map(|t| t.pos) };
                target.is_some_and(|t| {
                    let to = (t - a.pos).truncate();
                    let facing = glam::Vec2::new(a.heading.sin(), a.heading.cos());
                    to.length() < a.reach() * 1.5 && facing.angle_to(to.normalize_or_zero()).abs() < 0.6
                })
            })
            .filter_map(|a| Some((a.ref_id, self.block_chance(a, &set)?)))
            .collect();
        for (actor, chance) in waiting {
            let roll = (self.rand() % 10_000) as f32 / 10_000.0;
            if roll < chance * GUARD_RATE * dt {
                let secs = 0.8 + (self.rand() % 800) as f32 / 1000.0;
                want.push((actor, secs));
            }
        }
        for (actor, secs) in want {
            if let Some(a) = self.actor_mut(actor) {
                log::debug!("{actor} raises its guard for {secs:.1}s");
                a.set_guard(secs);
            }
        }
        // Who has a guard up, for attackers choosing their blows.
        let guarding: Vec<FormId> =
            self.cells.values().flat_map(|rt| &rt.actors).filter(|a| a.guarding()).map(|a| a.ref_id).chain(self.player_blocking.then_some(PLAYER_REF)).collect();
        for rt in self.cells.values_mut() {
            for a in rt.actors.iter_mut() {
                if let Some(c) = a.combat.as_mut() {
                    c.target_guarding = guarding.contains(&c.target);
                }
                let Some(c) = a.combat.as_mut().filter(|c| c.guard > 0.0) else { continue };
                let left = c.guard - dt;
                if left <= 0.0 {
                    a.set_guard(0.0);
                } else {
                    c.guard = left;
                }
            }
        }
    }

    /// Have a fighting actor hold its guard up for `secs` (console `guard`).
    pub fn force_guard(&mut self, actor: FormId, secs: f32) -> bool {
        match self.actor_mut(actor).filter(|a| a.combat.is_some()) {
            Some(a) => {
                a.set_guard(secs);
                true
            }
            None => false,
        }
    }

    /// How likely an actor is to block a blow: humanoids with a shield, or a
    /// weapon drawn, as defensive as their combat style.
    fn block_chance(&self, a: &ActorRuntime, set: &CombatSettings) -> Option<f32> {
        if a.dead || a.bleeding.is_some() || !a.graph.as_ref().is_some_and(|g| g.project().humanoid()) {
            return None;
        }
        let chance = (a.stats.defensive * BLOCK_CHANCE).min(0.9);
        if self.shield_rating(a.ref_id).is_some() && a.torch.is_none() {
            Some(chance)
        } else if a.weapon_out && a.weapon_reach > 0.0 {
            Some(chance * set.weapon_block_chance)
        } else {
            None
        }
    }

    /// Take health from an actor (or the player), with a flinch, and fight back.
    pub fn damage(&mut self, target: FormId, amount: f32, attacker: Option<FormId>, stagger: f32) {
        if target == PLAYER_REF {
            if self.player_dead() {
                return;
            }
            self.player_health -= amount;
            log::info!("player takes {amount:.0} damage ({:.0} left)", self.player_health);
            if self.player_health <= 0.0 {
                self.player_health = 0.0;
                self.scripts.notify("You have died.");
                self.player_died_at = Some(self.scripts.real_time);
            }
            return;
        }
        let Some(key) = self.actor_cells.get(&target).copied() else { return };
        let Some(a) = self.cells.get_mut(&key).and_then(|rt| rt.actors.iter_mut().find(|a| a.ref_id == target)) else { return };
        if a.dead {
            return;
        }
        // Down and bleeding out: only the player finishes off a protected actor.
        if a.bleeding.is_some() {
            if !a.stats.essential && attacker == Some(PLAYER_REF) {
                a.bleeding = None;
                self.kill_actor(target);
            }
            return;
        }
        a.health -= amount;
        log::info!("{target} takes {amount:.0} damage ({:.0} / {:.0})", a.health, a.stats.max_health);
        if a.health <= 0.0 {
            // Essential actors (and protected ones, but to the player) bleed out.
            if a.stats.essential || (a.stats.protected && attacker != Some(PLAYER_REF)) {
                self.start_bleedout(target);
                return;
            }
            self.kill_actor(target);
            return;
        }
        // Flinch unless mid-swing or behind its guard; heavy hits stagger.
        let swinging = a.combat.as_ref().is_some_and(|c| c.swing > 0.0) || a.guarding();
        if let Some(g) = a.graph.as_mut() {
            if stagger > 0.0 {
                g.set_variable("staggerMagnitude", stagger.min(1.0));
                g.send_event("staggerStart");
            } else if !swinging {
                g.send_event("recoilStart");
            }
        }
        if let Some(by) = attacker {
            self.start_combat(target, by);
        }
    }

    /// Bring an actor down to bleed out: it drops, stops fighting, and whoever
    /// fought it looks for someone else.
    fn start_bleedout(&mut self, actor: FormId) {
        let Some(a) = self.actor_cells.get(&actor).and_then(|k| self.cells.get_mut(k)).and_then(|rt| rt.actors.iter_mut().find(|a| a.ref_id == actor)) else { return };
        a.health = 0.0;
        a.bleeding = Some(BLEEDOUT_TIME);
        a.combat = None;
        log::info!("{actor} bleeds out (was {})", a.state_name());
        a.fall_out_of_furniture(&mut self.furniture, &self.nav);
        a.halt(BLEEDOUT_TIME);
        if let Some(g) = a.graph.as_mut() {
            g.send_event("bleedOutStart");
        }
        let fighting: Vec<FormId> =
            self.cells.values().flat_map(|rt| &rt.actors).filter(|a| a.combat.as_ref().is_some_and(|c| c.target == actor)).map(|a| a.ref_id).collect();
        for f in fighting {
            self.end_combat(f);
        }
    }

    /// Papyrus `Kill()` and the console's `kill`: essential actors only bleed out
    /// (unless `essential_too`, as `KillEssential()`). False if it isn't a living
    /// loaded actor.
    pub fn kill(&mut self, actor: FormId, essential_too: bool) -> bool {
        let essential = self
            .actor_cells
            .get(&actor)
            .and_then(|k| self.cells.get(k))
            .and_then(|rt| rt.actors.iter().find(|a| a.ref_id == actor))
            .filter(|a| !a.dead)
            .map(|a| a.stats.essential);
        match essential {
            Some(true) if !essential_too => {
                if !self.is_bleeding_out(actor) {
                    self.start_bleedout(actor);
                }
                true
            }
            Some(_) => self.kill_actor(actor),
            None => false,
        }
    }

    /// Essential actors bleeding out get back up after a while, with a quarter of
    /// their health.
    pub(crate) fn update_bleedouts(&mut self, dt: f32) {
        for rt in self.cells.values_mut() {
            for a in rt.actors.iter_mut() {
                let Some(t) = a.bleeding.as_mut() else { continue };
                *t -= dt;
                if *t > 0.0 {
                    a.halt(1.0);
                    continue;
                }
                a.bleeding = None;
                a.health = a.stats.max_health * 0.25;
                if let Some(g) = a.graph.as_mut() {
                    g.send_event("bleedOutStop");
                }
                log::info!("{} recovers", a.ref_id);
            }
        }
    }

    /// What an actor (or the player) is wearing protects against blows.
    pub fn protection(&self, actor: FormId) -> Protection {
        let set = self.combat_settings();
        let worn = self.inventories.get(&actor).map_or(&[][..], |i| &i.equipped[..]);
        let skills = if actor == PLAYER_REF {
            npc_field(&self.lo, FormId(0x7), b"DNAM", 0x2).map_or([15.0; 2], |d| armor_skills(&d))
        } else {
            match self.actor_cells.get(&actor).and_then(|k| self.cells.get(k)).and_then(|rt| rt.actors.iter().find(|a| a.ref_id == actor)) {
                Some(a) => a.stats.armor_skills,
                None => return Protection::default(),
            }
        };
        protection(&self.lo, &set, worn, skills, actor == PLAYER_REF)
    }

    /// Physical damage after the target's armor.
    fn after_armor(&self, target: FormId, damage: f32) -> f32 {
        let p = self.protection(target);
        if p.reduction > 0.0 {
            log::debug!("{target}'s armor ({:.0}, {} pieces) takes {:.0}% of {damage:.0}", p.rating, p.pieces, p.reduction * 100.0);
        }
        damage * (1.0 - p.reduction)
    }

    pub fn is_bleeding_out(&self, actor: FormId) -> bool {
        self.actor_cells.get(&actor).and_then(|k| self.cells.get(k)).and_then(|rt| rt.actors.iter().find(|a| a.ref_id == actor)).is_some_and(|a| a.bleeding.is_some())
    }

    pub fn player_dead(&self) -> bool {
        self.player_died_at.is_some()
    }

    /// The player swings at whatever actor is in front within reach.
    pub fn player_attack(&mut self) {
        if self.player_dead() || self.conversation.is_some() || self.player_blocking {
            return;
        }
        let weapon = self.player_weapon();
        let (damage, reach) = match weapon.and_then(|w| self.lo.get(w)) {
            Some(rec) => (
                rec.get(b"DATA").filter(|d| d.len() >= 10).map_or(4.0, |d| u16::from_le_bytes([d[8], d[9]]) as f32),
                COMBAT_DISTANCE * rec.get(b"DNAM").map_or(1.0, |d| f32_at(d, 8)).max(0.5) + 40.0,
            ),
            None => (4.0, 120.0),
        };
        let hit = self.physics.raycast(self.camera.position, self.camera.forward(), reach);
        if let Some((_, Some(r))) = hit
            && self.lo.tag_of(r).map(|t| t.0) == Some(*b"ACHR")
            && !self.is_dead(r)
        {
            log::info!("player strikes {r} for {damage:.0}");
            self.hit(r, PLAYER_REF, damage, false, 0.0);
        }
    }

    /// Kill an actor: it stops whatever it was doing and falls as a ragdoll (or
    /// just stops, without one). False if it isn't loaded or is already dead.
    pub fn kill_actor(&mut self, actor: FormId) -> bool {
        let Some(key) = self.actor_cells.get(&actor).copied() else { return false };
        let Some(index) = self.cells.get(&key).and_then(|rt| rt.actors.iter().position(|a| a.ref_id == actor)) else { return false };
        let (pose, transform) = match self.scene.cells.get(&key).and_then(|rc| rc.actors.get(index)) {
            Some(inst) => (inst.pose.clone(), inst.transform),
            None => return false,
        };
        let Some(a) = self.cells.get_mut(&key).and_then(|rt| rt.actors.get_mut(index)) else { return false };
        if a.dead {
            return false;
        }
        a.dead = true;
        a.health = 0.0;
        a.combat = None;
        a.objects_changed |= !a.objects.is_empty();
        a.objects.clear();
        let velocity = Vec3::new(a.heading.sin(), a.heading.cos(), 0.0) * a.speed;
        let skeleton = a.skeleton.clone();
        let capsule = a.capsule;
        if let Some(c) = capsule {
            self.physics.set_enabled(&[c], false);
        }
        if let Some(desc) = skeleton.ragdoll.clone() {
            let rd = self.physics.spawn_ragdoll(&desc, |i| transform * pose.get(desc.bodies[i].bone).copied().unwrap_or_default() * desc.bodies[i].offset, velocity, actor);
            let mapping = RagdollPose::new(&desc, &skeleton, &pose, transform);
            if let Some(a) = self.cells.get_mut(&key).and_then(|rt| rt.actors.get_mut(index)) {
                a.ragdoll = Some((rd, mapping));
            }
            log::info!("{actor} dies ({} ragdoll bodies, {} joints)", desc.bodies.len(), desc.joints.len());
        } else {
            log::info!("{actor} dies (no ragdoll)");
        }
        self.furniture.release(actor);
        if self.barks.current.as_ref().is_some_and(|b| b.speaker == actor) {
            self.barks.current = None;
        }
        // Whoever fought it looks for someone else.
        let fighting: Vec<FormId> =
            self.cells.values().flat_map(|rt| &rt.actors).filter(|a| a.combat.as_ref().is_some_and(|c| c.target == actor)).map(|a| a.ref_id).collect();
        for f in fighting {
            self.end_combat(f);
        }
        // Scripts hear of it (killer unknown).
        self.send_script_event(actor, "OnDying", vec![papyrus::Value::None]);
        self.send_script_event(actor, "OnDeath", vec![papyrus::Value::None]);
        true
    }

    pub fn is_dead(&self, actor: FormId) -> bool {
        self.actor_cells.get(&actor).and_then(|k| self.cells.get(k)).and_then(|rt| rt.actors.iter().find(|a| a.ref_id == actor)).is_some_and(|a| a.dead)
    }

    /// A summary of a loaded actor's combat stats (console).
    pub fn combat_summary(&mut self, actor: FormId) -> Vec<String> {
        let Some(a) = self.actor_cells.get(&actor).and_then(|k| self.cells.get(k)).and_then(|rt| rt.actors.iter().find(|a| a.ref_id == actor)) else {
            return vec![format!("{actor} isn't a loaded actor")];
        };
        let stats = a.stats.clone();
        let mut out = vec![
            format!(
                "{actor}: health {:.0} / {:.0}, aggression {}, reach {:.0}, fighting {:?}{}{}",
                a.health,
                stats.max_health,
                stats.aggression,
                a.reach(),
                a.combat.as_ref().map(|c| c.target),
                if stats.essential { ", essential" } else { "" },
                if stats.protected { ", protected" } else { "" }
            ),
            format!("factions {:?}", stats.factions.iter().map(|f| format!("{f} {}", self.lo.get(*f).and_then(|r| r.editor_id().map(|e| e.to_string())).unwrap_or_default())).collect::<Vec<_>>()),
            format!("attacks {:?}", stats.attacks.iter().map(|x| x.event.as_str()).collect::<Vec<_>>()),
        ];
        out.push(format!(
            "offensive {:.2} (power vs guard x{:.2}), block skill {:.0}, defensive {:.2}, guard up {}",
            stats.offensive,
            stats.power_vs_guard,
            stats.block_skill,
            stats.defensive,
            self.actor_ref(actor).is_some_and(|a| a.guarding())
        ));
        let p = self.protection(actor);
        out.push(format!(
            "armor {:.0} ({} pieces, light / heavy skill {:.0} / {:.0}): blows {:.0}% weaker",
            p.rating,
            p.pieces,
            stats.armor_skills[0],
            stats.armor_skills[1],
            p.reduction * 100.0
        ));
        let pf = self.player_factions();
        out.push(format!("towards the player: reaction {:?}, hostile {}", self.faction_reaction(&stats.factions, &pf), self.hostile_to(&stats, &pf, true)));
        out
    }

    /// Health of a loaded actor (current, max).
    pub fn actor_health(&self, actor: FormId) -> Option<(f32, f32)> {
        let a = self.actor_cells.get(&actor).and_then(|k| self.cells.get(k)).and_then(|rt| rt.actors.iter().find(|a| a.ref_id == actor))?;
        Some((a.health, a.stats.max_health))
    }
}

/// Faction relations cache type.
pub type FactionRelations = HashMap<FormId, Arc<Vec<(FormId, u32)>>>;
