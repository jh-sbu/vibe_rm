//! Fighting and dying: who is hostile to whom, closing in and attacking through
//! the race's attack data, hits landing on the graph's `HitFrame`, health, death.

use std::collections::HashMap;
use std::sync::Arc;

use esp::{FormId, LoadOrder};
use glam::{Mat4, Vec3};

use super::{ActorRuntime, State, World, uniform};
use crate::engine::{Engine, PLAYER_REF};
use crate::perks::ep;
use crate::world::ragdoll::RagdollPose;
use crate::world::template::{self, Sources};

/// How often actors look for enemies (no source). Whom they notice is
/// detection's (`crate::detection`).
const DETECT_INTERVAL: f32 = 1.0;
/// Actors don't start fights, or join them, further off than this (no source).
/// Fights end by detection (`crate::detection`).
pub(crate) const ENGAGE_DISTANCE: f32 = 4000.0;
/// Base melee reach (`fCombatDistance`), scaled by the weapon's reach.
const COMBAT_DISTANCE: f32 = 141.0;
/// The push of the player's blow on what it strikes (kg m/s; twice that for a
/// power attack; no source).
const BLOW_PUSH: f32 = 4.0;
/// Seconds an attack keeps the attacker in place.
const SWING_TIME: f32 = 1.1;

/// Block chance per incoming swing for each point of combat style defensiveness
/// (with a shield), and times a second an actor waiting to strike raises its
/// guard at full defensiveness. Tuned by eye: the game's own rule isn't documented.
const BLOCK_CHANCE: f32 = 2.0;
const GUARD_RATE: f32 = 0.6;

/// Chance a fighter in reach bashes rather than swings, per point of its combat
/// style's bash multiplier: against a raised guard, and otherwise. Tuned by eye.
const BASH_VS_GUARD: f32 = 0.6;
const BASH_CHANCE: f32 = 0.08;

/// Chance an attack is a power attack per point of combat style offensiveness
/// (at most 60%). Tuned by eye, like the guard rates.
const POWER_ATTACK_CHANCE: f32 = 0.6;

/// Biped slot 39: shields.
const SHIELD_SLOT: u32 = 1 << 9;

/// Seconds the player holds the attack button for a power attack.
const POWER_ATTACK_HOLD: f32 = 0.35;

/// How far a fleeing actor looks for somewhere to run to, how many places it
/// weighs, and how near counts as there.
const FLEE_STEP: f32 = 1200.0;
const FLEE_TRIES: usize = 8;
const FLEE_ARRIVED: f32 = 64.0;

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
    /// Times the power attack stamina cost.
    pub stamina_mult: f32,
}

/// What an actor fights with.
#[derive(Debug, Clone, Default)]
pub struct CombatStats {
    pub max_health: f32,
    /// Stamina, and the percent of it that comes back each second out of combat.
    pub max_stamina: f32,
    pub stamina_regen: f32,
    pub attacks: Vec<Attack>,
    pub unarmed_damage: f32,
    pub unarmed_reach: f32,
    /// AI data: 0 unaggressive, 1 aggressive (attacks enemies), 2 very aggressive
    /// (and neutrals), 3 frenzied (anyone).
    pub aggression: u8,
    /// AI data assistance: 0 helps nobody, 1 helps allies, 2 helps friends and allies.
    pub assistance: u8,
    /// Aggro radius behaviour (`AIDT`): attacks the player coming this close.
    pub aggro_attack: Option<f32>,
    /// AI data confidence (0 cowardly .. 4 foolhardy), and the threat ratio below
    /// which it flees (`fConfidenceCowardly` .. `fConfidenceFoolhardy`; see `threat`).
    pub confidence: u8,
    pub confidence_value: f32,
    /// Percent of its health that comes back each second out of combat.
    pub health_regen: f32,
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
    /// How readily it bashes (`CSME` bash multiplier), and bashes a swing coming
    /// at it from behind its guard (bash attack / bash power attack multipliers).
    pub bash: f32,
    pub bash_vs_attack: f32,
    pub bash_vs_power: f32,
    /// How readily it backs off from a target close in (`CSCR` fallback multiplier).
    pub fallback: f32,
}

/// A weapon's weight (`DATA`).
fn weapon_weight(lo: &LoadOrder, weapon: FormId) -> f32 {
    lo.get(weapon)
        .and_then(|r| {
            r.get(b"DATA")
                .filter(|d| d.len() >= 8)
                .map(|d| f32_at(d, 4))
        })
        .unwrap_or(0.0)
}

/// A weapon's base damage (`DATA`).
pub(crate) fn weapon_damage(lo: &LoadOrder, weapon: FormId) -> f32 {
    lo.get(weapon)
        .and_then(|r| {
            r.get(b"DATA")
                .filter(|d| d.len() >= 10)
                .map(|d| u16::from_le_bytes([d[8], d[9]]) as f32)
        })
        .unwrap_or(0.0)
}

fn f32_at(d: &[u8], o: usize) -> f32 {
    d.get(o..o + 4)
        .map_or(0.0, |b| f32::from_le_bytes(b.try_into().unwrap()))
}

/// The player's stats (`DNAM`: skills, then health / magicka / stamina).

/// Attacks listed in a race or NPC record (`ATKD` + `ATKE` pairs).
fn attacks_of(rec: &esp::LoadedRecord<'_>) -> Vec<Attack> {
    let mut out = Vec::new();
    let mut pending: Option<Attack> = None;
    for sr in rec.subrecords() {
        match &sr.tag.0 {
            b"ATKD" if sr.data.len() >= 28 => {
                let d = sr.data;
                pending = Some(Attack {
                    event: String::new(),
                    damage_mult: f32_at(d, 0),
                    chance: f32_at(d, 4),
                    flags: u32::from_le_bytes(d[12..16].try_into().unwrap()),
                    strike_angle: f32_at(d, 20),
                    stagger: f32_at(d, 24),
                    stamina_mult: if d.len() >= 44 { f32_at(d, 40) } else { 1.0 },
                });
            }
            b"ATKE" => {
                if let Some(mut attack) = pending.take() {
                    attack.event = sr.zstring();
                    if !attack.event.is_empty() {
                        out.push(attack);
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// Light and heavy armor skills from an NPC's `DNAM` (skills 6 and 5).
fn armor_skills(dnam: &[u8]) -> [f32; 2] {
    [
        dnam.get(6).copied().unwrap_or(15) as f32,
        dnam.get(5).copied().unwrap_or(15) as f32,
    ]
}

/// A float game setting (`GMST`), or `default` when the plugins lack it.
pub(crate) fn gmst_f32(lo: &LoadOrder, name: &str, default: f32) -> f32 {
    lo.find_editor_id(name)
        .and_then(|id| lo.get(id))
        .and_then(|r| {
            r.get(b"DATA")
                .filter(|d| d.len() >= 4)
                .map(|d| f32_at(d, 0))
        })
        .unwrap_or(default)
}

/// An integer game setting (`GMST`), or `default` when the plugins lack it.
pub(crate) fn gmst_i32(lo: &LoadOrder, name: &str, default: i32) -> i32 {
    lo.find_editor_id(name)
        .and_then(|id| lo.get(id))
        .and_then(|r| {
            r.get(b"DATA")
                .filter(|d| d.len() >= 4)
                .map(|d| i32::from_le_bytes(d[0..4].try_into().unwrap()))
        })
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
    /// Stamina: a power attack costs `attack_base + attack_mult x weapon weight`
    /// (times the attack's own multiplier); a blocked blow `block_base +
    /// block_mult x the damage stopped`; sprinting `sprint x (sprint_base +
    /// sprint_weight x armor weight)` a second. It comes back `combat_regen` as
    /// fast in combat, `regen_delay` seconds after being spent.
    stamina_attack_base: f32,
    stamina_attack_mult: f32,
    stamina_block_base: f32,
    stamina_block_mult: f32,
    sprint: f32,
    sprint_base: f32,
    sprint_weight: f32,
    combat_regen: f32,
    regen_delay: f32,
    /// Health comes back this much as fast in combat.
    combat_health_regen: f32,
    /// Bashing: stamina for a bash and a power bash (times the attack's own
    /// multiplier), how far one reaches, and the share of the shield's rating
    /// (or the weapon's damage) it strikes for, from `min` at block skill 0 to
    /// `max` at 100 (`pc_max` for the player's shield).
    bash_stamina: f32,
    power_bash_stamina: f32,
    bash_reach: f32,
    shield_bash_min: f32,
    shield_bash_max: f32,
    shield_bash_pc_max: f32,
    weapon_bash_min: f32,
    weapon_bash_max: f32,
    /// Sneak attack damage multipliers by weapon animation type (`WEAP` `DNAM`:
    /// 0 hand to hand, 1 sword, 2 dagger, 3 war axe, 4 mace, 5 greatsword,
    /// 6 battleaxe); see [`SNEAK_BOW_MULT`] for shots.
    sneak_mult: [f32; 7],
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
            stamina_attack_base: gmst_f32(lo, "fStaminaAttackWeaponBase", 20.0),
            stamina_attack_mult: gmst_f32(lo, "fStaminaAttackWeaponMult", 1.0),
            stamina_block_base: gmst_f32(lo, "fStaminaBlockBase", 0.0),
            stamina_block_mult: gmst_f32(lo, "fStaminaBlockDmgMult", 0.25),
            sprint: gmst_f32(lo, "fSprintStaminaDrainMult", 7.0),
            sprint_base: gmst_f32(lo, "fSprintStaminaWeightBase", 1.0),
            sprint_weight: gmst_f32(lo, "fSprintStaminaWeightMult", 0.02),
            combat_regen: gmst_f32(lo, "fCombatStaminaRegenRateMult", 0.35),
            regen_delay: gmst_f32(lo, "fDamagedStaminaRegenDelay", 0.5),
            combat_health_regen: gmst_f32(lo, "fCombatHealthRegenRateMult", 0.0),
            bash_stamina: gmst_f32(lo, "fStaminaBashBase", 35.0),
            power_bash_stamina: gmst_f32(lo, "fStaminaPowerBashBase", 55.0),
            bash_reach: gmst_f32(lo, "fCombatBashReach", 141.0),
            shield_bash_min: gmst_f32(lo, "fShieldBashMin", 0.05),
            shield_bash_max: gmst_f32(lo, "fShieldBashMax", 0.25),
            shield_bash_pc_max: gmst_f32(lo, "fShieldBashPCMax", 0.25),
            weapon_bash_min: gmst_f32(lo, "fWeaponBashMin", 0.05),
            weapon_bash_max: gmst_f32(lo, "fWeaponBashMax", 0.25),
            sneak_mult: [
                gmst_f32(lo, "fCombatSneakHandMult", 2.0),
                gmst_f32(lo, "fCombatSneak1HSwordMult", 3.0),
                gmst_f32(lo, "fCombatSneak1HDaggerMult", 3.0),
                gmst_f32(lo, "fCombatSneak1HAxeMult", 3.0),
                gmst_f32(lo, "fCombatSneak1HMaceMult", 3.0),
                gmst_f32(lo, "fCombatSneak2HSwordMult", 2.0),
                gmst_f32(lo, "fCombatSneak2HAxeMult", 2.0),
            ],
        }
    }
}

/// Sneak attack multiplier for bows and crossbows: no game setting holds it;
/// UESP gives double damage before the Deadly Aim perk (`known_gaps/sneak-attacks.md`).
const SNEAK_BOW_MULT: f32 = 2.0;

/// Block XP for the player's bash landing (UESP; no game setting holds it).
const PLAYER_BASH_XP: f32 = 5.0;

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
/// player, then `perk` (the wearer's armor perks, by piece); clothing rates as
/// it is and doesn't count as a piece.
pub fn protection(
    lo: &LoadOrder,
    set: &CombatSettings,
    worn: &[FormId],
    skills: [f32; 2],
    player: bool,
    perk: &dyn Fn(FormId, f32) -> f32,
) -> Protection {
    let k = if player { set.player_max } else { set.npc_max } - set.rating_base;
    let mut p = Protection::default();
    for &f in worn {
        let Some(rec) = lo.get(f).filter(|r| r.tag().0 == *b"ARMO") else {
            continue;
        };
        // DNAM: rating x 100. Armor type: BOD2 (slots, type) or the older BODT
        // (slots, flags, type).
        let base =
            rec.get(b"DNAM")
                .filter(|d| d.len() >= 4)
                .map_or(0, |d| i32::from_le_bytes(d[0..4].try_into().unwrap())) as f32
                / 100.0;
        let kind = match (rec.get(b"BOD2"), rec.get(b"BODT")) {
            (Some(d), _) if d.len() >= 8 => u32::from_le_bytes(d[4..8].try_into().unwrap()),
            (_, Some(d)) if d.len() >= 12 => u32::from_le_bytes(d[8..12].try_into().unwrap()),
            _ => 2,
        };
        match kind {
            0 | 1 => {
                let skill = skills[kind as usize];
                p.rating += perk(f, base * (set.rating_base + k * skill / 100.0)).ceil();
                p.pieces += 1;
            }
            _ => p.rating += perk(f, base * set.clothing),
        }
    }
    p.reduction = ((p.rating * set.scaling + p.pieces as f32 * set.per_piece).min(set.max) / 100.0)
        .clamp(0.0, 1.0);
    p
}

impl CombatStats {
    pub fn of(e: &Engine, src: &Sources, race: FormId) -> CombatStats {
        let lo = &e.lo;
        let mut s = CombatStats {
            max_health: 50.0,
            unarmed_damage: 4.0,
            unarmed_reach: 96.0,
            aggression: 0,
            confidence: 2,
            ..Default::default()
        };
        if let Some(race) = lo.get(race) {
            if let Some(d) = race.get(b"DATA") {
                s.max_health = f32_at(d, 36);
                s.max_stamina = f32_at(d, 44);
                s.health_regen = f32_at(d, 84);
                s.stamina_regen = f32_at(d, 92);
                s.unarmed_damage = f32_at(d, 96).max(1.0);
                s.unarmed_reach = f32_at(d, 100).max(48.0);
            }
        }
        // Attacks: the attack race's (`ATKR`, with the attack data), else its own
        // race's; attacks the NPC lists itself replace the race's of the same event.
        let attack_race = src.form(lo, template::ATTACK_DATA, b"ATKR").unwrap_or(race);
        let mut attacks = lo
            .get(attack_race)
            .map(|r| attacks_of(&r))
            .unwrap_or_default();
        if let Some(own) = src.record(lo, template::ATTACK_DATA, b"ATKD") {
            for a in attacks_of(&own) {
                attacks.retain(|x| !x.event.eq_ignore_ascii_case(&a.event));
                attacks.push(a);
            }
        }
        // Attacks for situations not handled yet: sprinting, on horseback, with the
        // left hand, dual wielding, unarmed.
        attacks.retain(|a| {
            let lower = a.event.to_ascii_lowercase();
            !["sprint", "_mc", "lefthand", "dualwield", "h2h"]
                .iter()
                .any(|k| lower.contains(k))
        });
        s.attacks = attacks;
        // Health and stamina from the NPC's stats (DNAM: skills, then health /
        // magicka / stamina).
        // ACBS flags: 0x2 essential, 0x800 protected.
        let acbs = src
            .field(lo, template::BASE_DATA, b"ACBS")
            .filter(|d| d.len() >= 4)
            .map_or(0, |d| u32::from_le_bytes(d[0..4].try_into().unwrap()));
        s.essential = acbs & 0x2 != 0;
        s.protected = acbs & 0x800 != 0;
        if let Some(d) = src
            .field(lo, template::STATS, b"DNAM")
            .filter(|d| d.len() >= 38)
        {
            s.max_health += u16::from_le_bytes([d[36], d[37]]) as f32;
            if let Some(st) = d.get(40..42) {
                s.max_stamina += u16::from_le_bytes([st[0], st[1]]) as f32;
            }
            s.armor_skills = armor_skills(&d);
            s.block_skill = d[3] as f32;
        }
        // Combat style (with the AI data, template flag 0x10), else the default one.
        let style = src
            .form(lo, template::AI_DATA, b"ZNAM")
            .or_else(|| lo.find_editor_id("DefaultCombatstyle"));
        let style = style.and_then(|st| lo.get(st));
        let csgd = style
            .as_ref()
            .and_then(|r| r.get(b"CSGD"))
            .filter(|d| d.len() >= 8);
        s.offensive = csgd.map_or(0.25, |d| f32_at(d, 0));
        s.defensive = csgd.map_or(0.25, |d| f32_at(d, 4));
        // CSME: attack staggered, power attack staggered, power attack blocking,
        // bash, bash recoil, bash attack, bash power attack multipliers.
        let csme = style.as_ref().and_then(|r| r.get(b"CSME"));
        s.power_vs_guard = csme.filter(|d| d.len() >= 12).map_or(1.0, |d| f32_at(d, 8));
        (s.bash, s.bash_vs_attack, s.bash_vs_power) = csme
            .filter(|d| d.len() >= 28)
            .map_or((0.5, 0.25, 0.25), |d| {
                (f32_at(d, 12), f32_at(d, 20), f32_at(d, 24))
            });
        s.fallback = style
            .as_ref()
            .and_then(|r| r.get(b"CSCR"))
            .filter(|d| d.len() >= 8)
            .map_or(0.11, |d| f32_at(d, 4));
        s.max_health = s.max_health.max(5.0);
        s.max_stamina = s.max_stamina.max(10.0);
        if let Some(d) = src
            .field(lo, template::AI_DATA, b"AIDT")
            .filter(|d| d.len() >= 20)
        {
            s.aggression = d[0];
            s.confidence = d[1];
            s.assistance = d[5];
            if d[6] != 0 {
                s.aggro_attack = Some(u32::from_le_bytes(d[16..20].try_into().unwrap()) as f32)
                    .filter(|r| *r > 0.0);
            }
        }
        s.confidence_value = super::threat::confidence_value(lo, s.confidence);
        s.factions = src.member_of(lo);
        s
    }
}

/// An actor's fight with one target.
#[derive(Debug, Clone)]
pub struct Combat {
    pub target: FormId,
    path: Vec<Vec3>,
    next: usize,
    pub(crate) repath: f32,
    /// Where the target was when the path couldn't reach it (the path then ends
    /// as near as the navmesh goes).
    unreachable: Option<Vec3>,
    /// Seconds until it may attack again, and left of the swing it is in.
    pub(crate) cooldown: f32,
    swing: f32,
    /// The attack being made (index into the stats' attacks), and whether it
    /// has struck (one hit a swing).
    pub attack: Option<usize>,
    pub struck: bool,
    /// Stamina the attack started this frame costs (paid once the graph takes it).
    pub cost: f32,
    /// Seconds left holding its guard up (blocking), and whether the graph has
    /// raised it (it may be busy finishing a swing or a stagger first).
    pub guard: f32,
    guard_shown: bool,
    /// The target has its guard up (set by the engine each frame).
    pub(crate) target_guarding: bool,
    /// Whether it can bash (with a shield or a melee weapon out), what with
    /// (set by the engine each frame), and whether to bash the swing coming at
    /// it from behind its guard.
    pub(crate) bash: Option<Bash>,
    pub(crate) counter: bool,
    /// Archers: where they are in their shot, and whether they have a clear line
    /// to the target (set by the engine each frame).
    pub(crate) draw: super::archery::Draw,
    pub(crate) clear_shot: bool,
    /// Running from the target rather than fighting it; where it is heading away
    /// from the target to (fleeing, or an archer backing off).
    pub fleeing: bool,
    pub(crate) away_to: Option<Vec3>,
    /// This fight's adjustment to its confidence, seconds to its next flee check
    /// (sooner when `recheck` is set), and, fleeing: how far it runs, how long it
    /// has been that far, and whether the threat was close last frame.
    pub(crate) confidence_mod: f32,
    pub(crate) threat_check: f32,
    pub(crate) recheck: bool,
    pub(crate) flee_distance: f32,
    pub(crate) safe: f32,
    threat_near: bool,
    /// Seconds the target has gone undetected, and where it was last detected.
    pub(crate) unseen: f32,
    pub(crate) last_seen: Option<Vec3>,
}

/// What an actor's bashes cost (bash, power bash; before the attack's own
/// multiplier) and how far they reach.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Bash {
    pub cost: [f32; 2],
    pub reach: f32,
}

/// Papyrus combat states (`GetCombatState`, `OnCombatStateChanged`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CombatState {
    #[default]
    None = 0,
    Fighting = 1,
    Searching = 2,
}

impl ActorRuntime {
    /// Its combat state and target: fighting, or searching for the target
    /// it lost (an alert actor that hasn't found anyone yet isn't in combat).
    pub(crate) fn combat_state(&self) -> (CombatState, Option<FormId>) {
        match (&self.combat, &self.search) {
            (Some(c), _) => (CombatState::Fighting, Some(c.target)),
            (None, Some(s)) if s.lost => (CombatState::Searching, Some(s.target)),
            _ => (CombatState::None, None),
        }
    }
}

impl Combat {
    pub fn new(target: FormId) -> Combat {
        Combat {
            target,
            path: Vec::new(),
            next: 0,
            repath: 0.0,
            unreachable: None,
            cooldown: 0.8,
            swing: 0.0,
            attack: None,
            struck: false,
            cost: 0.0,
            guard: 0.0,
            guard_shown: false,
            target_guarding: false,
            bash: None,
            counter: false,
            draw: Default::default(),
            clear_shot: false,
            fleeing: false,
            away_to: None,
            confidence_mod: 0.0,
            threat_check: 0.0,
            recheck: false,
            flee_distance: 0.0,
            safe: 0.0,
            threat_near: false,
            unseen: 0.0,
            last_seen: None,
        }
    }

    pub fn swinging(&self) -> bool {
        self.swing > 0.0
    }

    /// The graph wouldn't take the attack (busy flinching, say): no swing; try
    /// again shortly.
    pub fn refused(&mut self) {
        self.swing = 0.0;
        self.attack = None;
        self.draw = Default::default();
        self.cooldown = 0.3;
    }
}

impl ActorRuntime {
    /// How far its attacks reach.
    pub fn reach(&self) -> f32 {
        let r = if self.weapon_reach > 0.0 {
            COMBAT_DISTANCE * self.weapon_reach
        } else {
            self.stats.unarmed_reach
        };
        r * self.scale
    }

    pub(crate) fn run_speed(&self) -> f32 {
        let walk = self.walk_speed();
        self.moves
            .map(|(m, _)| m.run)
            .filter(|r| *r > walk)
            .unwrap_or(walk * 3.0)
    }

    /// Use up stamina; it doesn't come back for a moment (`update_stamina`).
    pub(crate) fn spend_stamina(&mut self, amount: f32) {
        if amount > 0.0 {
            self.stamina = (self.stamina - amount).max(0.0);
            self.stamina_spent = true;
            log::debug!(
                "{} spends {amount:.0} stamina ({:.0} left)",
                self.ref_id,
                self.stamina
            );
        }
    }

    /// Start a bash (a power bash if wanted and it has the stamina), by the
    /// race's bash attacks' chances. None if it has none it can pay for.
    fn start_bash(&mut self, b: Bash, want_power: bool, w: &mut World) -> Option<String> {
        let cost = |a: &Attack| b.cost[(a.flags & ATK_POWER != 0) as usize] * a.stamina_mult;
        let affordable: Vec<(usize, f32)> = self
            .stats
            .attacks
            .iter()
            .enumerate()
            .filter(|(_, a)| a.flags & ATK_BASH != 0 && cost(a) <= self.stamina)
            .map(|(i, a)| (i, a.chance.max(0.05)))
            .collect();
        let kind: Vec<(usize, f32)> = affordable
            .iter()
            .copied()
            .filter(|&(i, _)| (self.stats.attacks[i].flags & ATK_POWER != 0) == want_power)
            .collect();
        let pool = if kind.is_empty() { &affordable } else { &kind };
        let pick = pick_weighted(pool, w)?;
        let cost = cost(&self.stats.attacks[pick]);
        let c = self.combat.as_mut()?;
        c.attack = Some(pick);
        c.struck = false;
        c.cost = cost;
        c.swing = SWING_TIME * 0.8;
        c.cooldown = SWING_TIME + uniform(w.rand, 0.4, 1.2);
        Some(self.stats.attacks[pick].event.clone())
    }

    /// Close in on `target` at `run` speed along a path, refreshed as it moves;
    /// where it can't be reached (up on a rock, off the navmesh), as near as the
    /// navmesh goes. Only an actor off the navmesh itself heads straight for it.
    pub(crate) fn chase(&mut self, dt: f32, w: &mut World, target: Vec3, run: f32) {
        let pos = self.pos;
        let Some(c) = self.combat.as_mut() else {
            return;
        };
        // Out of reach where it was: searching again won't help until it moves.
        let still_unreachable = c.unreachable.is_some_and(|u| u.distance(target) < 64.0);
        if c.repath <= 0.0 || (c.next >= c.path.len() && !still_unreachable) {
            let (path, reached) = w
                .nav
                .path_towards(pos, target)
                .unwrap_or_else(|| (vec![target], true));
            c.path = path;
            c.next = 0;
            c.unreachable = (!reached).then_some(target);
            c.repath = if reached { 0.5 } else { 2.0 };
        }
        while c.next < c.path.len() && (c.path[c.next] - pos).truncate().length() < 16.0 {
            c.next += 1;
        }
        if c.next >= c.path.len() && c.unreachable.is_some() {
            // As near as it can get: wait there facing the target.
            self.speed = 0.0;
            self.state = State::Idle(1.0);
            self.turn_towards((target - pos).with_z(0.0).normalize_or_zero(), dt);
            return;
        }
        let way = c.path.get(c.next).copied().unwrap_or(target);
        let d = (way - pos).truncate();
        let remaining = self.turn_towards(d.normalize_or_zero().extend(0.0), dt);
        let speed = run * remaining.cos().max(0.0).powi(2);
        self.speed = speed;
        self.state = State::Walk {
            path: Vec::new(),
            next: 0,
            budget: 1.0,
            to_seat: false,
        };
        let fwd = Vec3::new(self.heading.sin(), self.heading.cos(), 0.0);
        let mut p = pos + fwd * (speed * dt).min(d.length().max(1.0));
        // Off the navmesh, keep level rather than heading for the target's height.
        let nz = w.nav.height_at(Vec3::new(p.x, p.y, pos.z));
        p.z = nz.unwrap_or(pos.z);
        log::trace!(
            "{} chases: at {pos:?} target {target:?} way {way:?} nav z {nz:?}",
            self.ref_id
        );
        self.pos = p;
    }

    /// Raise its guard for `secs` (the AI step has the graph raise it), or lower it.
    pub(crate) fn set_guard(&mut self, secs: f32) {
        let Some(c) = self.combat.as_mut() else {
            return;
        };
        c.guard = secs.max(0.0);
        if c.guard <= 0.0
            && std::mem::take(&mut c.guard_shown)
            && let Some(g) = self.graph.as_mut()
        {
            g.send_event("blockStop");
            g.set_variable("IsBlocking", 0.0);
        }
    }

    /// Its guard is up (raised by the graph): blows from ahead are blocked.
    pub fn guarding(&self) -> bool {
        self.combat
            .as_ref()
            .is_some_and(|c| c.guard > 0.0 && c.guard_shown)
    }

    /// Run from the target: to the place on the navmesh near by that is furthest
    /// from it, then on to the next, until its flee distance away, where it
    /// waits facing the threat. The threat coming close has it think again
    /// (`threat`). False when cornered with the threat close: it fights.
    fn flee_step(&mut self, dt: f32, w: &mut World, threat: Vec3) -> bool {
        let reach = self.reach();
        let pos = self.pos;
        let run = self.run_speed();
        let Some(c) = self.combat.as_mut() else {
            return true;
        };
        c.repath -= dt;
        let dist = pos.distance(threat);
        let near = dist < super::threat::FLEE_THREAT_NEAR;
        if near && !std::mem::replace(&mut c.threat_near, near) {
            c.recheck = true;
        }
        c.threat_near = near;
        if dist >= c.flee_distance {
            c.safe += dt;
            c.away_to = None;
            self.speed = 0.0;
            self.state = State::Idle(1.0);
            self.turn_towards((threat - pos).normalize_or_zero(), dt);
            return true;
        }
        c.safe = 0.0;
        // Somewhere new once there, or when the threat has come closer to the
        // place than the actor is.
        if c.away_to.is_none_or(|t| arrived_away(pos, t, threat)) {
            c.away_to = place_away(w, pos, threat, FLEE_STEP);
            c.repath = 0.0;
        }
        match c.away_to {
            Some(to) => self.chase(dt, w, to, run),
            None if pos.distance(threat) < reach * 2.0 => return false,
            // Cornered: stand facing the threat.
            None => {
                self.speed = 0.0;
                self.state = State::Idle(1.0);
                self.turn_towards((threat - pos).normalize_or_zero(), dt);
            }
        }
        true
    }

    /// Fight on: close in on the target at a run, face it, attack when in reach
    /// (or run from it, fleeing). Returns the graph event of an attack started
    /// this frame.
    pub(crate) fn combat_step(&mut self, dt: f32, w: &mut World, target: Vec3) -> Option<String> {
        // A swing under way is finished first.
        if self
            .combat
            .as_ref()
            .is_some_and(|c| c.fleeing && !c.swinging())
            && self.flee_step(dt, w, target)
        {
            return None;
        }
        let reach = self.reach();
        let run = self.run_speed();
        let pos = self.pos;
        let attacks: Vec<(usize, f32)> = self
            .stats
            .attacks
            .iter()
            .enumerate()
            .filter(|(_, a)| a.flags & ATK_BASH == 0)
            .map(|(i, a)| (i, a.chance.max(0.05)))
            .collect();
        // Humanoids fight with their weapon out (the draw may not have been taken,
        // say mid-flinch: ask again).
        let humanoid = self.graph.as_ref().is_some_and(|g| g.project().humanoid());
        if humanoid
            && !self.drawn
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
            // A swing coming at it: bash it from behind the guard, in reach and
            // with the stamina.
            if std::mem::take(&mut c.counter)
                && c.guard_shown
                && let Some(b) = c.bash.filter(|b| dist <= b.reach * self.scale)
            {
                c.guard = 0.0;
                c.guard_shown = false;
                if let Some(g) = self.graph.as_mut() {
                    g.set_variable("IsBlocking", 0.0);
                }
                let want_power =
                    uniform(w.rand, 0.0, 1.0) < self.stats.offensive * POWER_ATTACK_CHANCE;
                if let Some(ev) = self.start_bash(b, want_power, w) {
                    log::debug!("{} bashes back from its guard", self.ref_id);
                    return Some(ev);
                }
                // Short of stamina: keep the guard up.
                if let Some(c) = self.combat.as_mut() {
                    c.guard = 0.5;
                    c.guard_shown = true;
                }
                if let Some(g) = self.graph.as_mut() {
                    g.set_variable("IsBlocking", 1.0);
                }
            }
            let c = self.combat.as_mut()?;
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
        if self.bow {
            return self.archer_step(dt, w, target, dist);
        }
        if dist > reach * 0.85 {
            self.chase(dt, w, target, run);
            return None;
        }
        let c = self.combat.as_mut()?;
        // In reach: stand, face and strike.
        self.speed = 0.0;
        self.state = State::Idle(1.0);
        let cooldown = c.cooldown;
        let target_guarding = c.target_guarding;
        let bash = c.bash.filter(|b| dist <= b.reach * self.scale);
        let off = self.turn_towards(to.normalize_or_zero(), dt).abs();
        if cooldown > 0.0 || off > 0.35 || attacks.is_empty() {
            return None;
        }
        // A bash, as readily as its combat style bashes: mostly to break a
        // raised guard.
        if let Some(b) = bash {
            let chance = self.stats.bash
                * if target_guarding {
                    BASH_VS_GUARD
                } else {
                    BASH_CHANCE
                };
            if uniform(w.rand, 0.0, 1.0) < chance {
                let want_power =
                    uniform(w.rand, 0.0, 1.0) < self.stats.offensive * POWER_ATTACK_CHANCE;
                if let Some(ev) = self.start_bash(b, want_power, w) {
                    return Some(ev);
                }
            }
        }
        // A power attack now and then, as offensive as its combat style (more or
        // less so against a raised guard) and if it has the stamina for one, else
        // a basic one; then which, by their chances.
        let is_power = |i: usize| self.stats.attacks[i].flags & ATK_POWER != 0;
        let cost = |i: usize| self.power_cost * self.stats.attacks[i].stamina_mult;
        let power_chance = (self.stats.offensive
            * POWER_ATTACK_CHANCE
            * if target_guarding {
                self.stats.power_vs_guard
            } else {
                1.0
            })
        .clamp(0.0, 0.6);
        let want_power = uniform(w.rand, 0.0, 1.0) < power_chance;
        let kind: Vec<(usize, f32)> = attacks
            .iter()
            .copied()
            .filter(|&(i, _)| is_power(i) == want_power && (!want_power || cost(i) <= self.stamina))
            .collect();
        let attacks: Vec<(usize, f32)> = attacks
            .iter()
            .copied()
            .filter(|&(i, _)| !is_power(i) || cost(i) <= self.stamina)
            .collect();
        if attacks.is_empty() {
            return None;
        }
        let pool = if kind.is_empty() { &attacks } else { &kind };
        let pick = pick_weighted(pool, w)?;
        let power = is_power(pick);
        let cost = if power { cost(pick) } else { 0.0 };
        let c = self.combat.as_mut()?;
        c.attack = Some(pick);
        c.cost = cost;
        c.struck = false;
        c.swing = SWING_TIME;
        // A power attack takes longer to recover from.
        c.cooldown = SWING_TIME
            + uniform(w.rand, 0.3, 1.4)
            + if power {
                uniform(w.rand, 0.5, 1.0)
            } else {
                0.0
            };
        Some(self.stats.attacks[pick].event.clone())
    }
}

/// Somewhere on the navmesh within `step` of `pos`, as far from `threat` as a
/// few tries find, and further from it than `pos` is.
pub(crate) fn place_away(w: &mut World, pos: Vec3, threat: Vec3, step: f32) -> Option<Vec3> {
    let away = (pos - threat).truncate().normalize_or(glam::Vec2::X);
    let centre = pos + (away * step * 0.5).extend(0.0);
    let rand = &mut *w.rand;
    (0..FLEE_TRIES)
        .filter_map(|_| w.nav.random_point(centre, step, &mut *rand))
        .max_by(|a, b| a.distance(threat).total_cmp(&b.distance(threat)))
        .filter(|p| p.distance(threat) > pos.distance(threat))
}

/// Whether an actor heading to `to`, away from `threat`, is there, or the threat
/// has come closer to it than the actor is.
pub(crate) fn arrived_away(pos: Vec3, to: Vec3, threat: Vec3) -> bool {
    (to - pos).truncate().length() < FLEE_ARRIVED || to.distance(threat) < to.distance(pos)
}

/// One of `pool` (index, chance) by their chances.
fn pick_weighted(pool: &[(usize, f32)], w: &mut World) -> Option<usize> {
    let total: f32 = pool.iter().map(|a| a.1).sum();
    let mut roll = uniform(w.rand, 0.0, total);
    for &(i, ch) in pool {
        if roll < ch {
            return Some(i);
        }
        roll -= ch;
    }
    pool.last().map(|a| a.0)
}

/// A swing at its hit frame: who swung (with which attack) at whom, from where.
pub(crate) struct Swing {
    pub attacker: FormId,
    pub target: FormId,
    pub attack: Option<usize>,
    /// The attacker's feet, and the height its blow comes from (its chest).
    pub pos: Vec3,
    pub chest: f32,
    pub heading: f32,
}

impl Engine {
    /// Damage of an attack by an actor: its weapon's (unless the attack ignores
    /// it), else its race's unarmed damage; scaled by the attack's multiplier.
    pub(crate) fn attack_damage(&self, a: &ActorRuntime, attack: Option<&Attack>) -> f32 {
        let ignore = attack.is_some_and(|x| x.flags & ATK_IGNORE_WEAPON != 0);
        let weapon = self
            .inventories
            .get(&a.ref_id)
            .and_then(|i| i.weapon(&self.lo));
        let base = match weapon.filter(|_| !ignore).and_then(|w| self.lo.get(w)) {
            Some(rec) => rec
                .get(b"DATA")
                .filter(|d| d.len() >= 10)
                .map_or(4.0, |d| u16::from_le_bytes([d[8], d[9]]) as f32),
            None => a.stats.unarmed_damage,
        };
        let mult = attack.map_or(1.0, |x| x.damage_mult.max(0.1))
            * if attack.is_some_and(|x| x.flags & ATK_POWER != 0) {
                1.5
            } else {
                1.0
            };
        log::debug!(
            "{}: {} base {base} x {mult}",
            a.ref_id,
            attack.map_or("-", |x| x.event.as_str())
        );
        base * mult
    }

    /// Whether `a` (an NPC with these stats) would attack `b` on sight. Aggressive
    /// actors attack their enemies, and the player unless they keep the law (belong
    /// to a faction that tracks crime: townsfolk, guards); very aggressive ones
    /// neutrals too; frenzied ones anyone. Allies and friends are left alone.
    pub(crate) fn hostile_to(
        &mut self,
        a: &CombatStats,
        b_factions: &[FormId],
        b_is_player: bool,
    ) -> bool {
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
    pub(crate) fn law_abiding(&self, factions: &[FormId]) -> bool {
        factions.iter().any(|&f| {
            self.lo
                .get(f)
                .and_then(|r| r.get(b"DATA").and_then(|d| d.first().copied()))
                .is_some_and(|flags| flags & 0x40 != 0)
        })
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
                let Some(rec) = self.lo.get(f) else {
                    return Arc::new(Vec::new());
                };
                Arc::new(
                    rec.subrecords()
                        .filter(|s| s.tag.0 == *b"XNAM" && s.data.len() >= 12)
                        .map(|s| {
                            (
                                rec.fid(s.form_id(0)),
                                u32::from_le_bytes(s.data[8..12].try_into().unwrap()),
                            )
                        })
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
        let Some(key) = self.actor_cells.get(&actor).copied() else {
            return false;
        };
        let rolls = (self.rand(), self.rand());
        let Some(a) = self
            .cells
            .get_mut(&key)
            .and_then(|rt| rt.actors.iter_mut().find(|a| a.ref_id == actor))
        else {
            return false;
        };
        if a.dead || a.combat.as_ref().is_some_and(|c| c.target == target) {
            return false;
        }
        // Searching for it (alert, or having lost it) or not: the detection
        // topic for the change.
        let topic = match a.search.as_ref() {
            Some(s) if s.lost => b"LOTC",
            Some(_) => b"ALTC",
            None => b"NOTC",
        };
        a.end_search();
        a.interrupt(&mut self.furniture);
        a.combat = Some(super::threat::new_combat(&self.lo, target, &a.stats, rolls));
        let humanoid = a.graph.as_ref().is_some_and(|g| g.project().humanoid());
        // Cowards run without drawing (unless cornered).
        let coward = a.stats.confidence == 0;
        log::info!("{actor} attacks {target}");
        self.bark(actor, topic);
        if humanoid && !coward {
            self.draw_weapon(actor, true);
        } else if !coward
            && let Some(g) = self
                .cells
                .get_mut(&key)
                .and_then(|rt| rt.actors.iter_mut().find(|a| a.ref_id == actor))
                .and_then(|a| a.graph.as_mut())
        {
            g.send_event("combatStanceStart");
        }
        true
    }

    /// Stop fighting: weapons away, back to its packages.
    pub fn end_combat(&mut self, actor: FormId) {
        self.stop_fighting(actor, false);
    }

    /// Stop fighting, putting weapons away unless `keep_weapon` (to search).
    pub(crate) fn stop_fighting(&mut self, actor: FormId, keep_weapon: bool) {
        let Some(key) = self.actor_cells.get(&actor).copied() else {
            return;
        };
        let Some(a) = self
            .cells
            .get_mut(&key)
            .and_then(|rt| rt.actors.iter_mut().find(|a| a.ref_id == actor))
        else {
            return;
        };
        if a.combat.is_none() {
            return;
        }
        a.set_guard(0.0);
        a.combat = None;
        a.halt(2.0);
        a.next_eval = 0.0;
        let humanoid = a.graph.as_ref().is_some_and(|g| g.project().humanoid());
        if keep_weapon {
        } else if humanoid {
            self.draw_weapon(actor, false);
        } else if let Some(g) = self
            .cells
            .get_mut(&key)
            .and_then(|rt| rt.actors.iter_mut().find(|a| a.ref_id == actor))
            .and_then(|a| a.graph.as_mut())
        {
            g.send_event("combatStanceStop");
        }
        log::info!("{actor} stops fighting");
    }

    /// Living actors look around now and then for enemies to attack, or for a
    /// fight near by to join on their allies' (or friends') side, as their AI
    /// data's assistance has it.
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
        // Fights going on: who, where, and against whom. The player fights whoever
        // fights them.
        let mut fights: Vec<(FormId, Vec3, FormId)> = Vec::new();
        for a in self.cells.values().flat_map(|rt| &rt.actors) {
            if let Some(c) = a
                .combat
                .as_ref()
                .filter(|c| !c.fleeing && !a.dead && a.bleeding.is_none())
            {
                fights.push((a.ref_id, a.pos, c.target));
                if c.target == PLAYER_REF {
                    fights.push((PLAYER_REF, player, a.ref_id));
                }
            }
        }
        let mut lookers: Vec<(FormId, Vec3, Arc<CombatStats>)> = Vec::new();
        for rt in self.cells.values_mut() {
            for a in rt.actors.iter_mut().filter(|a| {
                !a.dead
                    && a.bleeding.is_none()
                    && a.combat.is_none()
                    && (a.stats.aggression > 0
                        || a.stats.aggro_attack.is_some()
                        || a.stats.assistance > 0)
            }) {
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
            if d < ENGAGE_DISTANCE && self.finds_player(r) && self.would_attack_player(r) {
                best = Some((d, PLAYER_REF));
            }
            for (o, opos, of) in &others {
                let d = pos.distance(*opos);
                if *o == r || d > ENGAGE_DISTANCE || best.is_some_and(|b| b.0 <= d) {
                    continue;
                }
                if self.hostile_to(&stats, of, false) && self.finds(r, *o) {
                    best = Some((d, *o));
                }
            }
            if best.is_none() && stats.assistance > 0 {
                best = self.fight_to_join(r, pos, &stats, &fights, &others, &player_factions);
            }
            if let Some((_, t)) = best {
                found.push((r, t));
            }
        }
        for (a, t) in found {
            self.start_combat(a, t);
        }
    }

    /// Whether `r` would attack the player on detecting them: hostile to them
    /// (`hostile_to`), or they are within its aggro radius and it neither keeps
    /// the law nor likes them.
    pub(crate) fn would_attack_player(&mut self, r: FormId) -> bool {
        if self.player_dead() {
            return false;
        }
        let Some(a) = self
            .actor_ref(r)
            .filter(|a| !a.dead && a.bleeding.is_none())
        else {
            return false;
        };
        let (stats, d) = (a.stats.clone(), a.pos.distance(self.player.position));
        let player_factions = self.player_factions();
        let aggro = stats.aggro_attack.is_some_and(|r| d < r)
            && !self.law_abiding(&stats.factions)
            && !matches!(
                self.faction_reaction(&stats.factions, &player_factions),
                Some(2 | 3)
            );
        aggro || self.hostile_to(&stats, &player_factions, true) || self.crime_hostile(r)
    }

    /// The nearest fight `helper` detects (either side of it) that it would join, and whom it would
    /// attack: one where an ally (sharing a faction, or an allied one) or, for
    /// those who help friends too, a friend is fighting someone it isn't friendly
    /// with.
    fn fight_to_join(
        &mut self,
        helper: FormId,
        pos: Vec3,
        stats: &CombatStats,
        fights: &[(FormId, Vec3, FormId)],
        others: &[(FormId, Vec3, Vec<FormId>)],
        player_factions: &[FormId],
    ) -> Option<(f32, FormId)> {
        let factions_of = |who: FormId| -> Option<&[FormId]> {
            if who == PLAYER_REF {
                Some(player_factions)
            } else {
                others.iter().find(|o| o.0 == who).map(|o| o.2.as_slice())
            }
        };
        let mut best: Option<(f32, FormId, FormId)> = None;
        for &(fighter, fpos, target) in fights {
            let d = pos.distance(fpos);
            if fighter == helper
                || target == helper
                || d > ENGAGE_DISTANCE
                || best.is_some_and(|b| b.0 <= d)
            {
                continue;
            }
            if target == PLAYER_REF && self.player_dead() {
                continue;
            }
            let (Some(ff), Some(tf)) = (factions_of(fighter), factions_of(target)) else {
                continue;
            };
            let need = match self.faction_reaction(&stats.factions, ff) {
                Some(2) => 1,
                Some(3) => 2,
                _ => continue,
            };
            if stats.assistance < need
                || matches!(self.faction_reaction(&stats.factions, tf), Some(2 | 3))
            {
                continue;
            }
            if !self.detects(helper, fighter) && !self.detects(helper, target) {
                continue;
            }
            best = Some((d, target, fighter));
        }
        let (d, target, fighter) = best?;
        log::info!("{helper} comes to {fighter}'s aid against {target}");
        Some((d, target))
    }

    pub(crate) fn player_factions(&self) -> Vec<FormId> {
        self.npc_factions(FormId(0x7))
            .into_iter()
            .map(|(f, _)| f)
            .chain(std::iter::once(FormId(0xDB1)))
            .collect()
    }

    /// Resolve swings that reached their hit frame: the target must be within reach
    /// and the attack's strike angle.
    pub(crate) fn resolve_swings(&mut self, swings: Vec<Swing>) {
        for s in swings {
            let Some(a) = self
                .actor_cells
                .get(&s.attacker)
                .and_then(|k| self.cells.get(k))
                .and_then(|rt| rt.actors.iter().find(|a| a.ref_id == s.attacker))
            else {
                continue;
            };
            let attack = s.attack.and_then(|i| a.stats.attacks.get(i)).cloned();
            let bash = attack.as_ref().is_some_and(|x| x.flags & ATK_BASH != 0);
            let (damage, reach) = if bash {
                (
                    self.bash_damage(s.attacker, a.stats.block_skill)
                        * attack.as_ref().map_or(1.0, |x| x.damage_mult),
                    self.combat_settings().bash_reach * a.scale,
                )
            } else {
                (self.attack_damage(a, attack.as_ref()), a.reach())
            };
            let strike_angle = attack.as_ref().map_or(35.0, |x| x.strike_angle);
            let stagger = attack.as_ref().map_or(0.0, |x| x.stagger);
            let Some((tp, th)) = self.actor_body(s.target) else {
                continue;
            };
            let to = (tp - s.pos).truncate();
            // From the attacker's chest to the nearest of the target's body: one
            // up on a ledge is out of reach of a wolf, not of a giant.
            let rise = s.chest.clamp(tp.z, tp.z + th) - s.chest;
            let dist = to.length().hypot(rise);
            let fwd = glam::Vec2::new(s.heading.sin(), s.heading.cos());
            let angle = fwd.angle_to(to.normalize_or_zero()).abs().to_degrees();
            if dist > reach * 1.3 || angle > strike_angle.max(25.0) {
                log::debug!(
                    "{} misses {} ({dist:.0} / {reach:.0} units, {:.0} across, {rise:.0} up, {angle:.0} deg)",
                    s.attacker,
                    s.target,
                    to.length()
                );
                continue;
            }
            let power = attack.as_ref().is_some_and(|x| x.flags & ATK_POWER != 0);
            if bash {
                self.bash_hit(s.target, s.attacker, damage, power, stagger);
            } else {
                self.hit(s.target, s.attacker, damage, power, stagger, None);
            }
        }
    }

    /// A blow landing: armor takes its share, then the target's guard if it is
    /// blocking towards the attacker. Blocked blows don't make it flinch, but a
    /// blocked power attack breaks its guard with a stagger. `projectile`: the
    /// arrow's, for a shot.
    pub(crate) fn hit(
        &mut self,
        target: FormId,
        attacker: FormId,
        damage: f32,
        power: bool,
        stagger: f32,
        projectile: Option<FormId>,
    ) {
        // The attacker's perks (Armsman, Overdraw, Barbarian...), power attacks'
        // (Savage Strike...).
        let weapon = self.weapon_of(attacker);
        let about = [weapon, Some(target)];
        let mut damage = self.perk_entry_point(ep::MOD_ATTACK_DAMAGE, attacker, &about, damage);
        if power {
            damage = self.perk_entry_point(ep::MOD_POWER_ATTACK_DAMAGE, attacker, &about, damage);
        }
        let sneak = self
            .sneak_attack_mult(target, attacker, projectile.is_some())
            .map(|m| self.perk_entry_point(ep::MOD_SNEAK_ATTACK_MULT, attacker, &about, m));
        let damage = match sneak {
            Some(mult) => {
                log::info!("{attacker} sneak attacks {target}: {mult}x {damage:.0}");
                if attacker == PLAYER_REF {
                    let text = format!(
                        "{}{mult:.1}{}",
                        self.gmst_string("sSuccessfulSneakAttackMain")
                            .unwrap_or_else(|| "Sneak attack for ".into()),
                        self.gmst_string("sSuccessfulSneakAttackEnd")
                            .unwrap_or_else(|| "X damage!".into()),
                    );
                    self.scripts.notify(text);
                }
                damage * mult
            }
            None => damage,
        };
        if attacker == PLAYER_REF {
            self.player_blow_landed(projectile.is_some(), sneak.is_some());
        }
        let raw = damage;
        // Armor penetration (Bone Breaker...): the attacker's perks scale what
        // the target's armor takes away.
        let penetration =
            self.perk_entry_point(ep::MOD_TARGET_DAMAGE_RESISTANCE, attacker, &about, 1.0);
        let mut damage = self.after_armor(target, damage, penetration);
        // The target's perks against blows.
        damage = self.perk_entry_point(
            ep::MOD_INCOMING_DAMAGE,
            target,
            &[Some(attacker), weapon],
            damage,
        );
        // Attacks that can stagger do so only some of the time (iStaggerAttackChance).
        let staggers = (self.rand() % 100) < self.combat_settings().stagger_chance;
        let mut stagger = if staggers { stagger } else { 0.0 };
        let share = self.block_share(target, attacker, power);
        self.send_hit_event(
            target,
            attacker,
            projectile,
            power,
            sneak.is_some(),
            false,
            share.is_some(),
        );
        // Arrows land where they strike ([`Engine::arrow_impact`]).
        if projectile.is_none() {
            self.melee_impact(target, attacker, share.is_some(), false);
        }
        if target == PLAYER_REF {
            // Blocking trains by the damage stopped, the armor by the rest.
            let stopped = share.unwrap_or(0.0);
            self.use_skill(esp::actor_value::BLOCK, raw * stopped);
            self.armor_struck(raw * (1.0 - stopped));
        }
        if let Some(share) = share {
            log::info!(
                "{target} blocks {attacker}{}: {:.0}% of {damage:.0} stopped",
                if power { "'s power attack" } else { "" },
                share * 100.0
            );
            // Holding the blow back takes stamina.
            let set = self.combat_settings();
            self.spend_stamina(
                target,
                set.stamina_block_base + set.stamina_block_mult * damage * share,
            );
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
        // An enchanted weapon's (a bow's, for a shot) enchantment lands with
        // the blow.
        if let Some(ench) = weapon.and_then(|w| self.item_enchantment(w)) {
            self.apply_item(ench, Some(attacker), target);
        }
    }

    /// A blow or shot is a sneak attack when the attacker is sneaking and the
    /// struck actor doesn't detect it: the damage multiplier for the attacker's
    /// weapon (a shot's for bows). Not against the player, who detects no one
    /// (`crate::detection`), nor with a staff.
    fn sneak_attack_mult(&self, target: FormId, attacker: FormId, shot: bool) -> Option<f32> {
        if target == PLAYER_REF || !self.is_sneaking(attacker) || self.detects(target, attacker) {
            return None;
        }
        if shot {
            return Some(SNEAK_BOW_MULT);
        }
        let weapon = if attacker == PLAYER_REF {
            self.player_weapon()
        } else {
            self.inventories
                .get(&attacker)
                .and_then(|i| i.weapon(&self.lo))
        };
        let anim = weapon
            .and_then(|w| self.lo.get(w))
            .and_then(|r| r.get(b"DNAM").and_then(|d| d.first().copied()))
            .unwrap_or(0);
        self.combat_settings()
            .sneak_mult
            .get(anim as usize)
            .copied()
    }

    /// A bash landing: armor takes its share, but no guard stops it; it breaks
    /// an actor's guard, staggers the target (harder for a power bash) and cuts short
    /// the swing it was making.
    pub(crate) fn bash_hit(
        &mut self,
        target: FormId,
        attacker: FormId,
        damage: f32,
        power: bool,
        stagger: f32,
    ) {
        // Bashing perks (Power Bash, Deadly Bash...): with the shield, else the
        // weapon.
        let with = self
            .equipped_shield(attacker)
            .or_else(|| self.weapon_of(attacker));
        let damage = self.perk_entry_point(ep::MOD_BASHING_DAMAGE, attacker, &[with], damage);
        let raw = damage;
        let damage = self.after_armor(target, damage, 1.0);
        log::info!(
            "{attacker} {} {target} for {damage:.1}",
            if power { "power bashes" } else { "bashes" }
        );
        self.send_hit_event(target, attacker, None, power, false, true, false);
        if attacker == PLAYER_REF {
            self.use_skill(esp::actor_value::BLOCK, PLAYER_BASH_XP);
        } else if target == PLAYER_REF {
            self.armor_struck(raw);
        }
        self.melee_impact(target, attacker, false, true);
        if let Some(a) = self.actor_mut(target) {
            a.set_guard(0.0);
            if let Some(c) = a.combat.as_mut().filter(|c| c.swinging()) {
                log::debug!("{target}'s swing is cut short");
                c.swing = 0.0;
                c.struck = true;
                c.cost = 0.0;
            }
        }
        self.damage(target, damage, Some(attacker), stagger.max(0.25));
    }

    /// What a bash by `actor` (or the player) strikes for, before the attack's
    /// multiplier: its shield's rating, else its weapon's damage, times a share
    /// growing with block skill from `fShieldBashMin` / `fWeaponBashMin` to the
    /// matching max (the game settings' names suggest the rule; the game's own
    /// formula isn't documented). Nothing without either.
    pub(crate) fn bash_damage(&self, actor: FormId, block_skill: f32) -> f32 {
        let set = self.combat_settings();
        let t = (block_skill / 100.0).clamp(0.0, 1.0);
        if let Some(rating) = self.shield_rating(actor) {
            let max = if actor == PLAYER_REF {
                set.shield_bash_pc_max
            } else {
                set.shield_bash_max
            };
            return rating * (set.shield_bash_min + (max - set.shield_bash_min) * t);
        }
        self.weapon_base(actor)
            * (set.weapon_bash_min + (set.weapon_bash_max - set.weapon_bash_min) * t)
    }

    /// Armor and block game settings.
    pub(crate) fn combat_settings(&self) -> CombatSettings {
        *self
            .combat_settings
            .get_or_init(|| CombatSettings::load(&self.lo))
    }

    pub(crate) fn actor_mut(&mut self, actor: FormId) -> Option<&mut ActorRuntime> {
        let key = self.actor_cells.get(&actor).copied()?;
        self.cells
            .get_mut(&key)?
            .actors
            .iter_mut()
            .find(|a| a.ref_id == actor)
    }

    pub(crate) fn actor_ref(&self, actor: FormId) -> Option<&ActorRuntime> {
        self.actor_cells
            .get(&actor)
            .and_then(|k| self.cells.get(k))
            .and_then(|rt| rt.actors.iter().find(|a| a.ref_id == actor))
    }

    /// The player's weapon: the one they have equipped, else the best melee one
    /// they carry.
    pub(crate) fn player_weapon(&self) -> Option<FormId> {
        let inv = self.inventories.get(&PLAYER_REF)?;
        inv.weapon(&self.lo)
            .filter(|w| inv.count(*w) > 0)
            .or_else(|| {
                inv.items
                    .iter()
                    .filter(|(_, n)| *n > 0)
                    .map(|(f, _)| *f)
                    .filter(|f| {
                        self.lo.tag_of(*f).map(|t| t.0) == Some(*b"WEAP")
                            && !super::archery::is_bow(&self.lo, *f)
                    })
                    .max_by_key(|f| weapon_damage(&self.lo, *f) as u32)
            })
    }

    /// Base damage of what an actor (or the player) strikes with; 0 for creatures
    /// and bare hands.
    fn weapon_base(&self, actor: FormId) -> f32 {
        self.weapon_of(actor)
            .map_or(0.0, |w| weapon_damage(&self.lo, w))
    }

    /// The weapon an actor (or the player) strikes with, if any.
    pub(crate) fn weapon_of(&self, actor: FormId) -> Option<FormId> {
        if actor == PLAYER_REF {
            self.player_weapon()
        } else {
            self.inventories
                .get(&actor)
                .and_then(|i| i.weapon(&self.lo))
        }
    }

    /// The equipped shield's base armor rating, if any.
    fn shield_rating(&self, actor: FormId) -> Option<f32> {
        let r = self.lo.get(self.equipped_shield(actor)?)?;
        Some(
            r.get(b"DNAM")
                .filter(|d| d.len() >= 4)
                .map_or(0, |d| i32::from_le_bytes(d[0..4].try_into().unwrap())) as f32
                / 100.0,
        )
    }

    /// The shield an actor (or the player) has equipped.
    pub(crate) fn equipped_shield(&self, actor: FormId) -> Option<FormId> {
        let inv = self.inventories.get(&actor)?;
        inv.equipped.iter().copied().find(|&f| {
            self.lo.get(f).is_some_and(|r| {
                r.tag().0 == *b"ARMO" && crate::world::inventory::armor_slots(&r) & SHIELD_SLOT != 0
            })
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
            let skill = self.actor_value(PLAYER_REF, esp::actor_value::BLOCK);
            (
                self.player.position,
                glam::Vec2::new(f.x, f.y).normalize_or_zero(),
                skill,
            )
        } else {
            let a = self.actor_ref(target).filter(|a| a.guarding())?;
            (
                a.pos,
                glam::Vec2::new(a.heading.sin(), a.heading.cos()),
                a.stats.block_skill,
            )
        };
        let from = if attacker == PLAYER_REF {
            self.player.position
        } else {
            self.actor_ref(attacker)?.pos
        };
        let angle = facing
            .angle_to((from - pos).truncate().normalize_or_zero())
            .abs()
            .to_degrees();
        if angle > 35.0 {
            log::debug!("{target}'s guard faces away from {attacker} ({angle:.0} deg)");
            return None;
        }
        let skill = 1.0 + 1.5 * skill / 100.0;
        let share = match self.shield_rating(target) {
            Some(rating) => set.shield_base + set.shield_scaling * rating * skill / 100.0,
            None => {
                set.weapon_base + set.weapon_scaling * self.weapon_base(attacker) * skill / 100.0
            }
        };
        // Blocking perks (Shield Wall...).
        let share = self.perk_entry_point(ep::MOD_PERCENT_BLOCKED, target, &[], share);
        Some((share * if power { set.power_mult } else { 1.0 }).min(0.85))
    }

    /// Guards: actors raise theirs against a swing started at them (`started`:
    /// attacker, target), or now and then while waiting to strike, as their
    /// combat style's defensive side has them (less often without a shield);
    /// they lower it after a while.
    pub(crate) fn update_guards(&mut self, dt: f32, started: &[(FormId, FormId)]) {
        let set = self.combat_settings();
        // Who can bash, and what it costs them.
        let bashers: Vec<(FormId, Option<Bash>)> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| a.combat.is_some())
            .map(|a| {
                (
                    a.ref_id,
                    self.can_bash(a).then_some(Bash {
                        cost: [set.bash_stamina, set.power_bash_stamina],
                        reach: set.bash_reach,
                    }),
                )
            })
            .collect();
        for (actor, bash) in bashers {
            if let Some(c) = self.actor_mut(actor).and_then(|a| a.combat.as_mut()) {
                c.bash = bash;
            }
        }
        // A swing started at a raised guard: bash it now and then (as the combat
        // style's bash attack / bash power attack multipliers have it).
        for &(attacker, target) in started {
            let Some(swing) = self
                .actor_ref(attacker)
                .and_then(|a| {
                    a.combat
                        .as_ref()?
                        .attack
                        .and_then(|i| a.stats.attacks.get(i))
                })
                .map(|x| x.flags)
            else {
                continue;
            };
            let Some(a) = self
                .actor_ref(target)
                .filter(|a| a.guarding() && a.combat.as_ref().is_some_and(|c| c.bash.is_some()))
            else {
                continue;
            };
            if swing & ATK_BASH != 0 {
                continue;
            }
            let chance = if swing & ATK_POWER != 0 {
                a.stats.bash_vs_power
            } else {
                a.stats.bash_vs_attack
            };
            let roll = (self.rand() % 10_000) as f32 / 10_000.0;
            if roll < chance
                && let Some(c) = self.actor_mut(target).and_then(|a| a.combat.as_mut())
            {
                c.counter = true;
            }
        }
        let mut want: Vec<(FormId, f32)> = Vec::new();
        for &(attacker, target) in started {
            let Some(a) = self.actor_ref(target).filter(|a| {
                a.combat
                    .as_ref()
                    .is_some_and(|c| c.guard <= 0.0 && !c.swinging())
            }) else {
                continue;
            };
            let Some(chance) = self.block_chance(a, &set) else {
                continue;
            };
            let Some(from) = self
                .actor_ref(attacker)
                .map(|x| x.pos)
                .or((attacker == PLAYER_REF).then_some(self.player.position))
            else {
                continue;
            };
            let facing = glam::Vec2::new(a.heading.sin(), a.heading.cos());
            if facing
                .angle_to((from - a.pos).truncate().normalize_or_zero())
                .abs()
                > 1.0
            {
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
                let Some(c) = a
                    .combat
                    .as_ref()
                    .filter(|c| c.guard <= 0.0 && !c.swinging() && c.cooldown > 0.4)
                else {
                    return false;
                };
                let target = if c.target == PLAYER_REF {
                    Some(self.player.position)
                } else {
                    self.actor_ref(c.target).map(|t| t.pos)
                };
                target.is_some_and(|t| {
                    let to = (t - a.pos).truncate();
                    let facing = glam::Vec2::new(a.heading.sin(), a.heading.cos());
                    to.length() < a.reach() * 1.5
                        && facing.angle_to(to.normalize_or_zero()).abs() < 0.6
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
        let guarding: Vec<FormId> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| a.guarding())
            .map(|a| a.ref_id)
            .chain(self.player_blocking.then_some(PLAYER_REF))
            .collect();
        for rt in self.cells.values_mut() {
            for a in rt.actors.iter_mut() {
                if let Some(c) = a.combat.as_mut() {
                    c.target_guarding = guarding.contains(&c.target);
                }
                let Some(c) = a.combat.as_mut().filter(|c| c.guard > 0.0) else {
                    continue;
                };
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

    /// Whether an actor can bash: a humanoid with a shield (not a torch) or a
    /// melee weapon out.
    fn can_bash(&self, a: &ActorRuntime) -> bool {
        if a.dead
            || a.bow
            || a.bleeding.is_some()
            || !a.graph.as_ref().is_some_and(|g| g.project().humanoid())
        {
            return false;
        }
        (self.shield_rating(a.ref_id).is_some() && a.torch.is_none())
            || (a.weapon_out && a.weapon_reach > 0.0)
    }

    /// How likely an actor is to block a blow: humanoids with a shield, or a
    /// weapon drawn, as defensive as their combat style.
    fn block_chance(&self, a: &ActorRuntime, set: &CombatSettings) -> Option<f32> {
        if a.dead
            || a.bleeding.is_some()
            || !a.graph.as_ref().is_some_and(|g| g.project().humanoid())
        {
            return None;
        }
        if a.bow {
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
            log::info!(
                "player takes {amount:.0} damage ({:.0} left)",
                self.player_health
            );
            if self.player_health <= 0.0 {
                self.player_health = 0.0;
                self.scripts.notify("You have died.");
                self.player_died_at = Some(self.scripts.real_time);
            }
            return;
        }
        let Some(key) = self.actor_cells.get(&target).copied() else {
            return;
        };
        let Some(a) = self
            .cells
            .get_mut(&key)
            .and_then(|rt| rt.actors.iter_mut().find(|a| a.ref_id == target))
        else {
            return;
        };
        if a.dead {
            return;
        }
        // Down and bleeding out: only the player finishes off a protected actor.
        if a.bleeding.is_some() {
            if !a.stats.essential && attacker == Some(PLAYER_REF) {
                a.bleeding = None;
                self.kill_actor_by(target, attacker);
            }
            return;
        }
        // A first blow from someone it wasn't fighting is an assault.
        let assault = attacker
            .filter(|&by| by != target && a.combat.as_ref().is_none_or(|c| c.target != by))
            .map(|by| (by, a.combat.is_none(), a.stats.factions.clone()));
        a.health -= amount;
        log::info!(
            "{target} takes {amount:.0} damage ({:.0} / {:.0})",
            a.health,
            a.stats.max_health
        );
        // Hurt while fleeing, it thinks again.
        if let Some(c) = a.combat.as_mut().filter(|c| c.fleeing) {
            c.recheck = true;
        }
        if a.health <= 0.0 {
            // Essential actors (and protected ones, but to the player) bleed out.
            let bleeds = a.stats.essential || (a.stats.protected && attacker != Some(PLAYER_REF));
            if let Some((by, calm, factions)) = assault {
                self.send_assault(target, by, calm && self.law_abiding(&factions));
            }
            if bleeds {
                self.start_bleedout(target);
                return;
            }
            self.kill_actor_by(target, attacker);
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
        if let Some((by, calm, factions)) = assault {
            self.send_assault(target, by, calm && self.law_abiding(&factions));
        }
    }

    /// Bring an actor down to bleed out: it drops, stops fighting, and whoever
    /// fought it looks for someone else.
    fn start_bleedout(&mut self, actor: FormId) {
        let Some(a) = self
            .actor_cells
            .get(&actor)
            .and_then(|k| self.cells.get_mut(k))
            .and_then(|rt| rt.actors.iter_mut().find(|a| a.ref_id == actor))
        else {
            return;
        };
        a.health = 0.0;
        a.bleeding = Some(BLEEDOUT_TIME);
        a.combat = None;
        log::info!("{actor} bleeds out (was {})", a.state_name());
        a.fall_out_of_furniture(&mut self.furniture, &self.nav);
        a.halt(BLEEDOUT_TIME);
        if let Some(g) = a.graph.as_mut() {
            g.send_event("bleedOutStart");
        }
        self.send_script_event(actor, "OnEnterBleedout", Vec::new());
        let fighting: Vec<FormId> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| a.combat.as_ref().is_some_and(|c| c.target == actor))
            .map(|a| a.ref_id)
            .collect();
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
                let Some(t) = a.bleeding.as_mut() else {
                    continue;
                };
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
        let worn = self
            .inventories
            .get(&actor)
            .map_or(&[][..], |i| &i.equipped[..]);
        let skills = if actor == PLAYER_REF {
            [
                self.actor_value(PLAYER_REF, esp::actor_value::LIGHT_ARMOR),
                self.actor_value(PLAYER_REF, esp::actor_value::HEAVY_ARMOR),
            ]
        } else {
            match self
                .actor_cells
                .get(&actor)
                .and_then(|k| self.cells.get(k))
                .and_then(|rt| rt.actors.iter().find(|a| a.ref_id == actor))
            {
                Some(a) => a.stats.armor_skills,
                None => return Protection::default(),
            }
        };
        // Armor perks (Juggernaut, Agile Defender, Well Fitted...) on each piece.
        let perk = |piece: FormId, rating: f32| {
            self.perk_entry_point(ep::MOD_ARMOR_RATING, actor, &[Some(piece)], rating)
        };
        let mut p = protection(&self.lo, &set, worn, skills, actor == PLAYER_REF, &perk);
        // Magic armor (Oakflesh..., the full set bonuses) adds to the rating.
        let magic = self.actor_value(actor, esp::actor_value::DAMAGE_RESIST);
        if magic != 0.0 {
            p.rating += magic;
            p.reduction = ((p.rating * set.scaling + p.pieces as f32 * set.per_piece).min(set.max)
                / 100.0)
                .clamp(0.0, 1.0);
        }
        p
    }

    /// Physical damage after the target's armor, `penetration` of what it
    /// takes away still taken.
    fn after_armor(&self, target: FormId, damage: f32, penetration: f32) -> f32 {
        let mut p = self.protection(target);
        p.reduction *= penetration.max(0.0);
        if p.reduction > 0.0 {
            log::debug!(
                "{target}'s armor ({:.0}, {} pieces) takes {:.0}% of {damage:.0}",
                p.rating,
                p.pieces,
                p.reduction * 100.0
            );
        }
        damage * (1.0 - p.reduction)
    }

    /// `OnHit` (aggressor, source, projectile, power attack, sneak attack,
    /// bash, blocked) to what was struck: the source is the attacker's weapon.
    pub(crate) fn send_hit_event(
        &mut self,
        target: FormId,
        attacker: FormId,
        projectile: Option<FormId>,
        power: bool,
        sneak: bool,
        bash: bool,
        blocked: bool,
    ) {
        let weapon = if attacker == PLAYER_REF {
            self.player_weapon()
        } else {
            self.inventories
                .get(&attacker)
                .and_then(|i| i.weapon(&self.lo))
        };
        let form =
            |e: &Self, f: Option<FormId>| f.map_or(papyrus::Value::None, |f| e.object_value(f));
        let args = vec![
            self.object_value(attacker),
            form(self, weapon),
            form(self, projectile),
            papyrus::Value::Bool(power),
            papyrus::Value::Bool(sneak),
            papyrus::Value::Bool(bash),
            papyrus::Value::Bool(blocked),
        ];
        self.send_script_event(target, "OnHit", args);
    }

    /// An actor's combat state (Papyrus `GetCombatState`): fighting, or
    /// searching for a target it lost. The player is in combat while anyone
    /// fights them, searching while anyone searches for them.
    pub fn combat_state(&self, actor: FormId) -> CombatState {
        if actor == PLAYER_REF {
            let mut state = CombatState::None;
            for a in self
                .cells
                .values()
                .flat_map(|rt| &rt.actors)
                .filter(|a| !a.dead)
            {
                match a.combat_state() {
                    (CombatState::Fighting, Some(PLAYER_REF)) => return CombatState::Fighting,
                    (CombatState::Searching, Some(PLAYER_REF)) => state = CombatState::Searching,
                    _ => {}
                }
            }
            return state;
        }
        self.actor_ref(actor)
            .map_or(CombatState::None, |a| a.combat_state().0)
    }

    /// Whom an actor fights (or searches for, having lost them).
    pub fn combat_target(&self, actor: FormId) -> Option<FormId> {
        self.actor_ref(actor).and_then(|a| a.combat_state().1)
    }

    /// Tell actors' scripts of changes to their combat state
    /// (`OnCombatStateChanged`: target, state).
    pub(crate) fn report_combat_states(&mut self) {
        let mut changed: Vec<(FormId, Option<FormId>, CombatState)> = Vec::new();
        for a in self.cells.values_mut().flat_map(|rt| rt.actors.iter_mut()) {
            let (state, target) = if a.dead {
                (CombatState::None, None)
            } else {
                a.combat_state()
            };
            if state != a.combat_reported {
                a.combat_reported = state;
                changed.push((a.ref_id, target, state));
            }
        }
        for (actor, target, state) in changed {
            log::debug!("{actor} combat state {state:?} ({target:?})");
            let target = target.map_or(papyrus::Value::None, |t| self.object_value(t));
            self.send_script_event(
                actor,
                "OnCombatStateChanged",
                vec![target, papyrus::Value::Int(state as i32)],
            );
        }
    }

    pub fn is_bleeding_out(&self, actor: FormId) -> bool {
        self.actor_cells
            .get(&actor)
            .and_then(|k| self.cells.get(k))
            .and_then(|rt| rt.actors.iter().find(|a| a.ref_id == actor))
            .is_some_and(|a| a.bleeding.is_some())
    }

    pub fn player_dead(&self) -> bool {
        self.player_died_at.is_some()
    }

    /// The attack button went down: a power attack if it is held long enough
    /// (`update_player_attack`), else a basic one when it comes up.
    pub fn player_attack_press(&mut self) {
        if self.disabled_controls.fighting {
            return;
        }
        if self.player_blocking {
            self.player_bash();
            return;
        }
        // A weapon put away comes out first (fists too).
        if self.player_sheathed() {
            self.player_draw_weapon(true);
            return;
        }
        if self.player_has_bow() {
            self.player_graph_event("bowAttackStart");
        }
        self.player_attack_held = Some(0.0);
    }

    /// Attacking with the guard up: a bash at whatever actor is in front within
    /// bash reach, with a shield or a weapon and the stamina for it (power bashes
    /// need a perk the player doesn't have yet).
    pub fn player_bash(&mut self) {
        if self.player_dead() || self.conversation.is_some() || self.disabled_controls.fighting {
            return;
        }
        let set = self.combat_settings();
        if self.shield_rating(PLAYER_REF).is_none() && self.player_weapon().is_none() {
            return;
        }
        let stats = self.player_stats();
        let Some(attack) = stats
            .attacks
            .iter()
            .find(|a| a.flags & ATK_BASH != 0 && a.flags & ATK_POWER == 0)
            .cloned()
        else {
            return;
        };
        let cost = set.bash_stamina * attack.stamina_mult;
        if self.player_stamina < cost {
            log::debug!(
                "player hasn't the stamina to bash ({:.0} / {cost:.0})",
                self.player_stamina
            );
            return;
        }
        self.spend_stamina(PLAYER_REF, cost);
        self.player_graph_event(&attack.event);
        let skill = self.actor_value(PLAYER_REF, esp::actor_value::BLOCK);
        let damage = self.bash_damage(PLAYER_REF, skill) * attack.damage_mult;
        let hit = self.physics.raycast(
            self.camera.position,
            self.camera.forward(),
            set.bash_reach + 40.0,
        );
        if let Some((_, Some(r))) = hit
            && self.lo.tag_of(r).map(|t| t.0) == Some(*b"ACHR")
            && !self.is_dead(r)
        {
            self.bash_hit(r, PLAYER_REF, damage, false, attack.stagger);
        }
    }

    /// The attack button came up: a basic swing, or with a bow, the arrow loosed.
    pub fn player_attack_release(&mut self) {
        if let Some(held) = self.player_attack_held.take() {
            if self.player_has_bow() {
                self.player_graph_event("attackRelease");
                self.player_loose(held);
            } else {
                self.player_attack(false);
            }
        }
    }

    fn player_has_bow(&self) -> bool {
        self.player_weapon()
            .is_some_and(|w| super::archery::is_bow(&self.lo, w))
    }

    /// Holding the attack button: a power attack once held long enough (a bow
    /// just draws on).
    pub(crate) fn update_player_attack(&mut self, dt: f32) {
        let Some(t) = self.player_attack_held.as_mut() else {
            return;
        };
        *t += dt;
        if *t >= POWER_ATTACK_HOLD && !self.player_has_bow() {
            self.player_attack_held = None;
            self.player_attack(true);
        }
    }

    /// The player swings at whatever actor is in front within reach. A power
    /// attack takes the race's standing power attack's damage multiplier and
    /// stagger, and stamina; without enough left it is a basic swing.
    pub fn player_attack(&mut self, power: bool) {
        if self.disabled_controls.fighting {
            return;
        }
        // One swing at a time.
        if self.player_dead()
            || self.conversation.is_some()
            || self.player_blocking
            || self.player_swing.is_some()
        {
            return;
        }
        let weapon = self.player_weapon();
        let stats = self.player_stats();
        // The power attack for how they move, else the standing one, else any.
        let wanted = self.player_power_attack_event();
        let power_attacks: Vec<&Attack> = stats
            .attacks
            .iter()
            .filter(|a| {
                a.flags & ATK_POWER != 0
                    && a.event.to_ascii_lowercase().starts_with("attackpowerstart")
            })
            .collect();
        let find = |name: &str| {
            power_attacks
                .iter()
                .find(|a| a.event.eq_ignore_ascii_case(name))
                .copied()
        };
        let power_attack = find(wanted)
            .or_else(|| find("attackPowerStartInPlace"))
            .or(power_attacks.first().copied());
        let cost = self.power_attack_cost(PLAYER_REF, weapon)
            * power_attack.map_or(1.0, |a| a.stamina_mult);
        let power = power && self.player_stamina >= cost;
        let (mult, stagger) = match power_attack.filter(|_| power) {
            Some(a) => (a.damage_mult.max(1.0) * 1.5, a.stagger),
            None => (1.0, 0.0),
        };
        // With the weapon out, the body's graph plays the swing and lands it at
        // its HitFrame, and won't swing again mid-swing; otherwise (no graph,
        // or the console's swing with it put away) it lands at once.
        let event = match power_attack.filter(|_| power) {
            Some(a) => a.event.clone(),
            None => "attackStart".to_owned(),
        };
        // Still drawing: not yet.
        let played = self.player_weapon_drawn();
        if played && (!self.player_weapon_ready() || !self.player_graph_event(&event)) {
            return;
        }
        if power {
            self.spend_stamina(PLAYER_REF, cost);
        }
        let (damage, reach) = match weapon.and_then(|w| self.lo.get(w)) {
            Some(rec) => (
                rec.get(b"DATA")
                    .filter(|d| d.len() >= 10)
                    .map_or(4.0, |d| u16::from_le_bytes([d[8], d[9]]) as f32),
                COMBAT_DISTANCE * rec.get(b"DNAM").map_or(1.0, |d| f32_at(d, 8)).max(0.5) + 40.0,
            ),
            None => (4.0, 120.0),
        };
        let damage = damage * mult;
        if played {
            self.player_swing = Some(crate::player_body::PlayerSwing {
                power,
                damage,
                reach,
                stagger,
                waited: 0.0,
            });
            return;
        }
        self.player_strike(power, damage, reach, stagger);
    }

    /// The player's swing lands on whatever is in front of them within reach.
    pub(crate) fn player_strike(&mut self, power: bool, damage: f32, reach: f32, stagger: f32) {
        let dir = self.camera.forward();
        let ray = self.physics.raycast(self.camera.position, dir, reach);
        log::debug!("player attack: {ray:?} within {reach:.0}");
        let Some((_, Some(r))) = ray else {
            return;
        };
        let actor = self.created(r).map_or_else(
            || self.lo.tag_of(r).is_some_and(|t| t.0 == *b"ACHR"),
            |c| c.actor,
        );
        if actor && !self.is_dead(r) {
            log::info!(
                "player {} {r} for {damage:.0}",
                if power { "power attacks" } else { "strikes" }
            );
            self.hit(r, PLAYER_REF, damage, power, stagger, None);
        } else {
            // Anything else (a body, clutter, a wall): pushed if it moves.
            let push = if power { BLOW_PUSH * 2.0 } else { BLOW_PUSH };
            log::info!("player strikes {r}");
            self.strike(
                r,
                PLAYER_REF,
                None,
                dir * push * crate::physics::METRE,
                power,
            );
        }
    }

    /// Kill an actor: it stops whatever it was doing and falls as a ragdoll (or
    /// just stops, without one). False if it isn't loaded or is already dead.
    pub fn kill_actor(&mut self, actor: FormId) -> bool {
        self.kill_actor_by(actor, None)
    }

    /// Kill an actor, `killer` (if known) having dealt the blow: its scripts
    /// hear of it (`OnDying`, `OnDeath`) and the Story Manager gets a kill event.
    pub fn kill_actor_by(&mut self, actor: FormId, killer: Option<FormId>) -> bool {
        let rank = killer.map_or(0, |k| self.relationship_rank(actor, k));
        let killed = self.kill_actor_quietly(actor);
        if killed {
            let k = killer.map_or(papyrus::Value::None, |k| self.object_value(k));
            self.send_script_event(actor, "OnDying", vec![k.clone()]);
            self.send_script_event(actor, "OnDeath", vec![k]);
            let crime = if killer == Some(PLAYER_REF) {
                self.player_kill(actor)
            } else {
                0
            };
            self.send_kill_event(actor, killer, crime, rank);
        }
        killed
    }

    /// Kill an actor without telling its scripts (one placed dead).
    pub(crate) fn kill_actor_quietly(&mut self, actor: FormId) -> bool {
        self.kill_actor_lying(actor, None)
    }

    /// Kill an actor quietly, its ragdoll bodies placed as given (a body as it
    /// lay before its cell unloaded) or from its pose.
    pub(crate) fn kill_actor_lying(&mut self, actor: FormId, lying: Option<Vec<Mat4>>) -> bool {
        let Some(key) = self.actor_cells.get(&actor).copied() else {
            return false;
        };
        let Some(index) = self
            .cells
            .get(&key)
            .and_then(|rt| rt.actors.iter().position(|a| a.ref_id == actor))
        else {
            return false;
        };
        let (pose, transform) = match self
            .scene
            .cells
            .get(&key)
            .and_then(|rc| rc.actors.get(index))
        {
            Some(inst) => (inst.pose.clone(), inst.transform),
            None => return false,
        };
        let Some(a) = self
            .cells
            .get_mut(&key)
            .and_then(|rt| rt.actors.get_mut(index))
        else {
            return false;
        };
        if a.dead {
            return false;
        }
        a.dead = true;
        a.health = 0.0;
        a.combat = None;
        self.remember_dead(actor);
        let Some(a) = self
            .cells
            .get_mut(&key)
            .and_then(|rt| rt.actors.get_mut(index))
        else {
            return false;
        };
        a.objects_changed |= !a.objects.is_empty();
        a.objects.clear();
        a.carrying = false;
        let velocity = Vec3::new(a.heading.sin(), a.heading.cos(), 0.0) * a.speed;
        let skeleton = a.skeleton.clone();
        let capsule = a.capsule;
        if let Some(c) = capsule {
            self.physics.set_enabled(&[c], false);
        }
        if let Some(desc) = skeleton.ragdoll.clone() {
            let lying = lying.filter(|b| b.len() == desc.bodies.len());
            let velocity = if lying.is_some() {
                Vec3::ZERO
            } else {
                velocity
            };
            let rd = self.physics.spawn_ragdoll(
                &desc,
                |i| match &lying {
                    Some(b) => b[i],
                    None => {
                        transform
                            * pose.get(desc.bodies[i].bone).copied().unwrap_or_default()
                            * desc.bodies[i].offset
                    }
                },
                velocity,
                actor,
            );
            let mapping = RagdollPose::new(&desc, &skeleton, &pose, transform);
            if let Some(a) = self
                .cells
                .get_mut(&key)
                .and_then(|rt| rt.actors.get_mut(index))
            {
                a.ragdoll = Some((rd, mapping));
            }
            log::info!(
                "{actor} dies ({} ragdoll bodies, {} joints)",
                desc.bodies.len(),
                desc.joints.len()
            );
        } else {
            log::info!("{actor} dies (no ragdoll)");
        }
        self.furniture.release(actor);
        if self
            .barks
            .current
            .as_ref()
            .is_some_and(|b| b.speaker == actor)
        {
            self.barks.current = None;
        }
        // Whoever fought it looks for someone else.
        let fighting: Vec<FormId> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| a.combat.as_ref().is_some_and(|c| c.target == actor))
            .map(|a| a.ref_id)
            .collect();
        for f in fighting {
            self.end_combat(f);
        }
        true
    }

    /// Dead, whether loaded or not.
    pub fn is_dead(&self, actor: FormId) -> bool {
        match self.actor_ref(actor) {
            Some(a) => a.dead,
            None => self.world_state.dead.contains_key(&actor),
        }
    }

    /// A summary of a loaded actor's combat stats (console).
    pub fn combat_summary(&mut self, actor: FormId) -> Vec<String> {
        let Some(a) = self
            .actor_cells
            .get(&actor)
            .and_then(|k| self.cells.get(k))
            .and_then(|rt| rt.actors.iter().find(|a| a.ref_id == actor))
        else {
            return vec![format!("{actor} isn't a loaded actor")];
        };
        let stats = a.stats.clone();
        let mut out = vec![
            format!(
                "{actor} at {:.0}: health {:.0} / {:.0} (+{}%/s), stamina {:.0} / {:.0} (power attack {:.0}), aggression {}, assistance {}, confidence {} ({:.3}{:+.2}), strength {:.0}, reach {:.0}, {} {:?}{}{}",
                a.pos,
                a.health,
                stats.max_health,
                stats.health_regen,
                a.stamina,
                stats.max_stamina,
                a.power_cost,
                stats.aggression,
                stats.assistance,
                stats.confidence,
                stats.confidence_value,
                a.combat.as_ref().map_or(0.0, |c| c.confidence_mod),
                a.strength,
                a.reach(),
                if a.combat.as_ref().is_some_and(|c| c.fleeing) {
                    "fleeing"
                } else {
                    "fighting"
                },
                a.combat.as_ref().map(|c| c.target),
                if stats.essential { ", essential" } else { "" },
                if stats.protected { ", protected" } else { "" }
            ),
            format!(
                "factions {:?}",
                stats
                    .factions
                    .iter()
                    .map(|f| format!(
                        "{f} {}",
                        self.lo
                            .get(*f)
                            .and_then(|r| r.editor_id().map(|e| e.to_string()))
                            .unwrap_or_default()
                    ))
                    .collect::<Vec<_>>()
            ),
            format!(
                "attacks {:?}",
                stats
                    .attacks
                    .iter()
                    .map(|x| x.event.as_str())
                    .collect::<Vec<_>>()
            ),
        ];
        out.push(format!(
            "offensive {:.2} (power vs guard x{:.2}), block skill {:.0}, defensive {:.2}, guard up {}",
            stats.offensive,
            stats.power_vs_guard,
            stats.block_skill,
            stats.defensive,
            self.actor_ref(actor).is_some_and(|a| a.guarding())
        ));
        if let Some(a) = self.actor_ref(actor).filter(|a| a.bow) {
            let arrow = self.arrows_of(actor);
            out.push(format!(
                "archer (draw speed {:.2}): shoots {:?} for {:.0} + bow, shot {:?}, clear {}",
                a.bow_speed,
                arrow.as_ref().map(|x| x.ammo),
                arrow.as_ref().map_or(0.0, |x| x.damage),
                a.combat.as_ref().map(|c| c.draw),
                a.combat.as_ref().is_some_and(|c| c.clear_shot)
            ));
        }
        let bash = self.actor_ref(actor).is_some_and(|a| self.can_bash(a));
        out.push(format!(
            "bash x{:.2} (vs attack {:.2}, vs power attack {:.2}), can bash {bash} for {:.1}",
            stats.bash,
            stats.bash_vs_attack,
            stats.bash_vs_power,
            self.bash_damage(actor, stats.block_skill)
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
        out.push(format!(
            "towards the player: reaction {:?}, hostile {}",
            self.faction_reaction(&stats.factions, &pf),
            self.hostile_to(&stats, &pf, true)
        ));
        out
    }

    /// What a power attack with `weapon` (or bare hands) costs, before the
    /// attack's own multiplier.
    pub(crate) fn power_attack_cost(&self, actor: FormId, weapon: Option<FormId>) -> f32 {
        let set = self.combat_settings();
        let cost = set.stamina_attack_base
            + set.stamina_attack_mult * weapon.map_or(0.0, |w| weapon_weight(&self.lo, w));
        self.perk_entry_point(ep::MOD_POWER_ATTACK_STAMINA, actor, &[weapon], cost)
    }

    /// The player's stats from their NPC record and race (stamina, attacks).
    pub(crate) fn player_stats(&self) -> Arc<CombatStats> {
        self.player_stats
            .get_or_init(|| {
                let race = self.npc_race(FormId(0x7)).unwrap_or(FormId(0x13746));
                let mut s = CombatStats::of(self, &Sources::of_npc(&self.lo, FormId(0x7), 0), race);
                self.apply_actor_values(PLAYER_REF, &mut s);
                Arc::new(s)
            })
            .clone()
    }

    /// Take stamina from an actor or the player; it waits a moment to come back.
    pub(crate) fn spend_stamina(&mut self, who: FormId, amount: f32) {
        if amount <= 0.0 {
            return;
        }
        if who == PLAYER_REF {
            self.player_stamina = (self.player_stamina - amount).max(0.0);
            self.player_stamina_wait = self.combat_settings().regen_delay;
        } else if let Some(a) = self.actor_mut(who) {
            a.spend_stamina(amount);
            return;
        }
        log::debug!(
            "{who} spends {amount:.0} stamina ({:.0} left)",
            self.player_stamina
        );
    }

    /// Sprinting drains the player's stamina, the more the heavier their armor;
    /// out of it, they can't. Returns whether they may sprint.
    pub(crate) fn player_sprint(&mut self, dt: f32) -> bool {
        if self.player_stamina <= 0.0 {
            return false;
        }
        let set = self.combat_settings();
        let weight: f32 = self.inventories.get(&PLAYER_REF).map_or(0.0, |i| {
            i.equipped
                .iter()
                .filter_map(|&f| {
                    self.lo
                        .get(f)
                        .filter(|r| r.tag().0 == *b"ARMO")?
                        .get(b"DATA")
                        .filter(|d| d.len() >= 8)
                        .map(|d| f32_at(d, 4))
                })
                .sum()
        });
        self.player_stamina = (self.player_stamina
            - set.sprint * (set.sprint_base + set.sprint_weight * weight) * dt)
            .max(0.0);
        self.player_stamina_wait = set.regen_delay;
        true
    }

    /// Stamina comes back by the race's rate (percent of the most a second),
    /// slower in combat, once a moment has passed since it was spent.
    pub(crate) fn update_stamina(&mut self, dt: f32) {
        let set = self.combat_settings();
        let mut player_fought = false;
        for rt in self.cells.values_mut() {
            for a in rt.actors.iter_mut().filter(|a| !a.dead) {
                player_fought |= a.combat.as_ref().is_some_and(|c| c.target == PLAYER_REF);
                if std::mem::take(&mut a.stamina_spent) {
                    a.stamina_wait = set.regen_delay;
                }
                if a.stamina_wait > 0.0 {
                    a.stamina_wait -= dt;
                    continue;
                }
                let rate = a.stats.stamina_regen / 100.0
                    * if a.combat.is_some() {
                        set.combat_regen
                    } else {
                        1.0
                    };
                a.stamina = (a.stamina + a.stats.max_stamina * rate * dt).min(a.stats.max_stamina);
            }
            // Health comes back out of combat (`fCombatHealthRegenRateMult` in it).
            for a in rt
                .actors
                .iter_mut()
                .filter(|a| !a.dead && a.bleeding.is_none())
            {
                let rate = a.stats.health_regen / 100.0
                    * if a.combat.is_some() {
                        set.combat_health_regen
                    } else {
                        1.0
                    };
                a.health = (a.health + a.stats.max_health * rate * dt).min(a.stats.max_health);
            }
        }
        if self.player_stamina_wait > 0.0 {
            self.player_stamina_wait -= dt;
        } else {
            let stats = self.player_stats();
            let rate =
                stats.stamina_regen / 100.0 * if player_fought { set.combat_regen } else { 1.0 };
            self.player_stamina =
                (self.player_stamina + stats.max_stamina * rate * dt).min(stats.max_stamina);
        }
    }

    /// Stamina of a loaded actor or the player (current, max).
    pub fn stamina(&self, actor: FormId) -> Option<(f32, f32)> {
        if actor == PLAYER_REF {
            return Some((self.player_stamina, self.player_stats().max_stamina));
        }
        let a = self.actor_ref(actor)?;
        Some((a.stamina, a.stats.max_stamina))
    }

    /// Set a loaded actor's (or the player's) stamina (console, Papyrus).
    pub fn set_stamina(&mut self, actor: FormId, value: f32) -> bool {
        if actor == PLAYER_REF {
            self.player_stamina = value.clamp(0.0, self.player_stats().max_stamina);
            return true;
        }
        match self.actor_mut(actor) {
            Some(a) => {
                a.stamina = value.clamp(0.0, a.stats.max_stamina);
                true
            }
            None => false,
        }
    }

    /// Health of a loaded actor (current, max).
    /// Health and the most an actor has: a loaded one's, or a hurt one's that
    /// unloaded (healing meanwhile).
    pub fn actor_health(&self, actor: FormId) -> Option<(f32, f32)> {
        match self.actor_ref(actor) {
            Some(a) => Some((a.health, a.stats.max_health)),
            None => self
                .world_state
                .wounds
                .get(&actor)
                .map(|w| (w.health_at(self.scripts.real_time), w.max)),
        }
    }
}

/// Faction relations cache type.
pub type FactionRelations = HashMap<FormId, Arc<Vec<(FormId, u32)>>>;
