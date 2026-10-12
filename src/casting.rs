//! Casting by hand: the player's spells from either hand and their powers,
//! paid for in magicka, and magicka coming back.
//!
//! A spell in a hand is cast with that hand's attack button (the right hand's
//! is the left mouse button). Fire-and-forget spells charge while it is held,
//! for their charge time (`SPIT`), and are cast when it comes up; let go
//! sooner, nothing is cast. Concentration spells cast as soon as it is down
//! and go on while it is held, their cost paid by the second. Without the
//! magicka, nothing is cast and the meter blinks.
//!
//! Where a cast goes is its delivery: on the caster (self), the actor in
//! front within reach (contact) or within the spell's range (target actor),
//! or as its effect's projectile (aimed): missiles and lobbers fly as arrows
//! do, flames and beams reach at once to their range. Target location spells
//! land where the view meets the world. Where a spell lands with an area,
//! the actors within it take it too.
//!
//! Powers (the voice slot, Z) are cast at once, greater powers once a day.
//!
//! Sources: UESP's Magic Overview, Magicka and Spells pages, the records
//! (`SPEL` `SPIT`, `MGEF` `DATA`, `PROJ` `DATA`, `EXPL` radius) and the game
//! settings. Choices: `known_gaps/casting.md`.

use std::collections::HashMap;

use esp::FormId;
use esp::actor_value as av;
use glam::Vec3;

use crate::ai::casters::CombatSpell;
use crate::ai::combat::gmst_f32;
use crate::engine::{Engine, PLAYER_REF};
use crate::magic::{MagicItem, flags, spell_type};
use crate::spells::{CONCENTRATION, LEFT, RIGHT, VOICE};

/// Deliveries (`SPIT` / `MGEF`).
#[allow(dead_code)]
pub mod delivery {
    pub const SELF: u32 = 0;
    pub const CONTACT: u32 = 1;
    pub const AIMED: u32 = 2;
    pub const TARGET_ACTOR: u32 = 3;
    pub const TARGET_LOCATION: u32 = 4;
}

/// `PROJ` `DATA` types that reach at once rather than fly.
const PROJ_BEAM: u16 = 0x4;
const PROJ_FLAME: u16 = 0x8;
const PROJ_CONE: u16 = 0x10;

/// Units in a foot of an effect's area: Fireball's 15 feet are its
/// explosion's 320 units.
const UNITS_PER_FOOT: f32 = 320.0 / 15.0;
/// How often a concentration spell lands on what it reaches (seconds).
const CONCENTRATION_TICK: f32 = 0.25;
/// Contact spells' reach, and target actor spells' without a range.
const CONTACT_REACH: f32 = 160.0;
const TARGET_RANGE: f32 = 4096.0;
/// How far target location spells reach without a projectile.
const LOCATION_RANGE: f32 = 4096.0;
/// The middle of an actor's body at scale 1 (area hits).
const BODY_CENTRE: f32 = 60.0;
/// Game days between a greater power's uses.
const POWER_RECHARGE_DAYS: f32 = 1.0;

/// A spell held ready in a hand.
#[derive(Debug, Clone)]
struct HandCast {
    spell: FormId,
    /// Seconds the button has been down.
    held: f32,
    /// Concentration: cast and going on; seconds to its next landing.
    flowing: bool,
    tick: f32,
}

/// The player's casting: what each hand holds ready, magicka's wait to come
/// back, and when greater powers were last used.
#[derive(Debug, Default)]
pub struct Casting {
    hands: [Option<HandCast>; 2],
    /// Seconds before the player's magicka comes back after being spent.
    magicka_wait: f32,
    /// Greater powers' last use (game days).
    powers_used: HashMap<FormId, f32>,
    /// The magicka meter should blink (counted; the HUD blinks for each).
    pub magicka_blinks: u32,
    /// Actors' offensive spells, read once.
    offensive: HashMap<FormId, std::sync::Arc<Vec<CombatSpell>>>,
}

impl Engine {
    /// What a hand (`RIGHT`, `LEFT`) holds, if a spell.
    fn hand_spell(&self, hand: usize) -> Option<FormId> {
        self.equipped_spell(PLAYER_REF, hand)
    }

    /// A hand's attack button went down: casting starts if it holds a spell
    /// (true), else it does what it does with a weapon (false).
    pub fn player_cast_press(&mut self, hand: usize) -> bool {
        let Some(spell) = self.hand_spell(hand) else {
            return false;
        };
        if self.disabled_controls.fighting
            || self.player_dead()
            || self.conversation.is_some()
            || self.casting.hands[hand].is_some()
        {
            return true;
        }
        // Hands put away come out first.
        if self.player_sheathed() {
            self.player_draw_weapon(true);
            return true;
        }
        let Some(item) = self.magic_item(spell) else {
            return true;
        };
        if !self.can_pay(spell, &item) {
            return true;
        }
        log::debug!("player readies {} in hand {hand}", item.name);
        self.casting.hands[hand] = Some(HandCast {
            spell,
            held: 0.0,
            flowing: false,
            tick: 0.0,
        });
        if item.cast_type == CONCENTRATION {
            self.start_concentration(hand);
        }
        true
    }

    /// A hand's attack button came up: a charged spell is cast, a
    /// concentration spell stops. True if the hand was casting.
    pub fn player_cast_release(&mut self, hand: usize) -> bool {
        let Some(h) = self.casting.hands[hand].take() else {
            return false;
        };
        if h.flowing {
            return true;
        }
        let Some(item) = self.magic_item(h.spell) else {
            return true;
        };
        if h.held + 1e-3 < item.charge_time {
            log::debug!("{} let go before it charged", item.name);
            return true;
        }
        if self.can_pay(h.spell, &item) {
            let cost = self.spell_cost(PLAYER_REF, h.spell);
            self.spend_magicka(cost);
            self.cast(PLAYER_REF, h.spell, 1.0);
        }
        true
    }

    /// Whether an actor is casting by hand (only the player does yet).
    pub fn is_casting(&self, actor: FormId) -> bool {
        actor == PLAYER_REF && self.casting.hands.iter().any(Option::is_some)
    }

    /// Whether the player has the magicka for a spell (a second of a
    /// concentration spell); if not, the meter blinks and they are told.
    fn can_pay(&mut self, spell: FormId, item: &MagicItem) -> bool {
        if item.spell_type != spell_type::SPELL {
            return true;
        }
        let cost = self.spell_cost(PLAYER_REF, spell);
        let have = self.actor_value(PLAYER_REF, av::MAGICKA);
        // A concentration spell starts with any magicka at all.
        let need = if item.cast_type == CONCENTRATION {
            cost.min(1.0)
        } else {
            cost
        };
        if have + 1e-3 >= need {
            return true;
        }
        self.casting.magicka_blinks += 1;
        let text = self
            .gmst_string("sMagicCastInsufficientMagicka")
            .unwrap_or_else(|| "You don't have enough Magicka".into());
        self.scripts.notify(text);
        false
    }

    /// Take magicka from the player; it waits a moment to come back.
    fn spend_magicka(&mut self, amount: f32) {
        if amount <= 0.0 {
            return;
        }
        self.damage_actor_value(PLAYER_REF, av::MAGICKA, amount);
        self.casting.magicka_wait = gmst_f32(&self.lo, "fDamagedMagickaRegenDelay", 0.5);
    }

    fn start_concentration(&mut self, hand: usize) {
        if let Some(h) = self.casting.hands[hand].as_mut() {
            h.flowing = true;
            h.tick = 0.0;
        }
    }

    /// The voice key (Z): the power readied is used. Shouts aren't built.
    pub fn player_use_power(&mut self) {
        if self.disabled_controls.fighting || self.player_dead() || self.conversation.is_some() {
            return;
        }
        let Some(power) = self.equipped_spell(PLAYER_REF, VOICE) else {
            return;
        };
        let Some(item) = self.magic_item(power) else {
            return;
        };
        match item.spell_type {
            spell_type::POWER => {
                let now = self.game_days();
                if self
                    .casting
                    .powers_used
                    .get(&power)
                    .is_some_and(|&t| now - t < POWER_RECHARGE_DAYS)
                {
                    let text = self
                        .gmst_string("sMagicCastPowerUsed")
                        .unwrap_or_else(|| "Powers can only be used once a day.".into());
                    self.scripts.notify(text);
                    return;
                }
                self.casting.powers_used.insert(power, now);
            }
            spell_type::LESSER_POWER => {}
            _ => return,
        }
        self.cast(PLAYER_REF, power, 1.0);
    }

    /// Holding spells: charging, concentration landing and paying by the
    /// second; magicka coming back.
    pub(crate) fn update_casting(&mut self, dt: f32) {
        let mut casting = false;
        for hand in [RIGHT, LEFT] {
            let Some(mut h) = self.casting.hands[hand].clone() else {
                continue;
            };
            casting = true;
            // The spell left the hand (unequipped, a weapon took it).
            if self.hand_spell(hand) != Some(h.spell) {
                self.casting.hands[hand] = None;
                continue;
            }
            h.held += dt;
            if h.flowing {
                let cost = self.spell_cost(PLAYER_REF, h.spell) * dt;
                if self.actor_value(PLAYER_REF, av::MAGICKA) < cost {
                    log::debug!("player's magicka ran out");
                    self.casting.magicka_blinks += 1;
                    self.casting.hands[hand] = None;
                    continue;
                }
                self.spend_magicka(cost);
                h.tick -= dt;
                if h.tick <= 0.0 {
                    h.tick += CONCENTRATION_TICK;
                    self.cast(PLAYER_REF, h.spell, CONCENTRATION_TICK);
                }
            }
            self.casting.hands[hand] = Some(h);
        }
        if !casting {
            self.regen_magicka(dt);
        }
        self.regen_npc_magicka(dt);
    }

    /// Magicka comes back by the actor's rate (percent of the most a second,
    /// times `MagickaRateMult`), a third as fast in combat; the player's once
    /// a moment has passed since it was spent.
    fn magicka_regen(&self, actor: FormId, fighting: bool, dt: f32) -> f32 {
        let max = self.av_max(actor, av::MAGICKA);
        let now = self.actor_value(actor, av::MAGICKA);
        if now >= max {
            return 0.0;
        }
        let mult = if fighting {
            gmst_f32(&self.lo, "fCombatMagickaRegenRateMult", 0.33)
        } else {
            1.0
        };
        let rate = self.actor_value(actor, av::MAGICKA_RATE) / 100.0
            * (self.actor_value(actor, av::MAGICKA_RATE_MULT) / 100.0)
            * mult;
        (max * rate * dt).clamp(0.0, max - now)
    }

    pub(crate) fn regen_magicka(&mut self, dt: f32) {
        if self.casting.magicka_wait > 0.0 {
            self.casting.magicka_wait -= dt;
            return;
        }
        let fighting = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .any(|a| !a.dead && a.combat.as_ref().is_some_and(|c| c.target == PLAYER_REF));
        let gain = self.magicka_regen(PLAYER_REF, fighting, dt);
        if gain > 0.0 {
            self.damage_actor_value(PLAYER_REF, av::MAGICKA, -gain);
        }
    }

    /// Loaded actors' spent magicka comes back, but not while they channel a
    /// spell (`bRegenNPCMagickaDuringCast`).
    pub(crate) fn regen_npc_magicka(&mut self, dt: f32) {
        let spent: Vec<FormId> = self
            .scripts
            .actor_values
            .iter()
            .filter(|((a, i), m)| *i == av::MAGICKA && m.damage < 0.0 && *a != PLAYER_REF)
            .map(|((a, _), _)| *a)
            .collect();
        for actor in spent {
            let Some(a) = self.actor_ref(actor).filter(|a| !a.dead) else {
                continue;
            };
            let channeling = a.combat.as_ref().is_some_and(|c| c.channel > 0.0);
            if channeling {
                continue;
            }
            let fighting = a.combat.is_some();
            let gain = self.magicka_regen(actor, fighting, dt);
            if gain > 0.0 {
                self.damage_actor_value(actor, av::MAGICKA, -gain);
            }
        }
    }

    /// Each actor in combat whose style favours magic fights with the
    /// strongest offensive spell it has the magicka for (none: by hand).
    pub(crate) fn update_combat_spells(&mut self) {
        let fighters: Vec<(FormId, bool)> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| !a.dead && a.combat.as_ref().is_some_and(|c| !c.fleeing))
            .map(|a| (a.ref_id, a.stats.magic_mult > a.stats.melee_mult))
            .collect();
        for (actor, caster) in fighters {
            let pick = if caster {
                let magicka = self.actor_value(actor, av::MAGICKA);
                let spells = self.offensive_spells(actor);
                spells
                    .iter()
                    .map(|s| (*s, self.spell_cost(actor, s.spell)))
                    // A concentration spell for a second or more.
                    .filter(|(_, cost)| *cost <= magicka)
                    .max_by(|a, b| a.1.total_cmp(&b.1))
                    .map(|(s, _)| s)
            } else {
                None
            };
            if let Some(a) = self.actor_mut(actor)
                && a.combat_spell != pick
            {
                if pick.is_none() && a.combat_spell.is_some() {
                    log::debug!("{actor} has no spell to fight with: by hand");
                }
                // A spell being channelled stops with it.
                if let Some(c) = a.combat.as_mut()
                    && a.combat_spell.is_some_and(|s| Some(s) != pick)
                {
                    c.channel = 0.0;
                }
                a.combat_spell = pick;
            }
        }
    }

    /// The hostile spells an actor knows that it can throw at a foe (aimed or
    /// at a target actor), read once.
    fn offensive_spells(&mut self, actor: FormId) -> std::sync::Arc<Vec<CombatSpell>> {
        if let Some(s) = self.casting.offensive.get(&actor) {
            return s.clone();
        }
        let mut out = Vec::new();
        for spell in self.actor_spells(actor) {
            let Some(item) = self.magic_item(spell) else {
                continue;
            };
            if item.spell_type != spell_type::SPELL
                || !matches!(item.delivery, delivery::AIMED | delivery::TARGET_ACTOR)
                || !self.item_hostile(&item)
            {
                continue;
            }
            let range = if item.delivery == delivery::TARGET_ACTOR {
                if item.range > 0.0 {
                    item.range
                } else {
                    TARGET_RANGE
                }
            } else {
                self.item_projectile(&item)
                    .and_then(|p| crate::ai::archery::spell_projectile(&self.lo, p))
                    .map_or(LOCATION_RANGE, |s| s.range)
            };
            out.push(CombatSpell {
                spell,
                range: range.min(crate::ai::archery::BOW_RANGE),
                concentration: item.cast_type == CONCENTRATION,
            });
        }
        let out = std::sync::Arc::new(out);
        self.casting.offensive.insert(actor, out.clone());
        out
    }

    /// Casters' casts this frame go at their targets' bodies, paid in
    /// magicka (a landing's share of a second for concentration spells).
    pub(crate) fn npc_casts(&mut self) {
        let mut casts: Vec<(FormId, CombatSpell, u32, FormId, Vec3)> = Vec::new();
        for rt in self.cells.values_mut() {
            for a in rt.actors.iter_mut() {
                let n = std::mem::take(&mut a.casts);
                if n == 0 {
                    continue;
                }
                if let (Some(s), Some(c)) = (a.combat_spell, a.combat.as_ref()) {
                    casts.push((a.ref_id, s, n, c.target, a.pos + Vec3::Z * 110.0 * a.scale));
                }
            }
        }
        for (actor, spell, n, target, eye) in casts {
            let aim = if target == PLAYER_REF {
                Some(self.player.position)
            } else {
                self.actor_ref(target)
                    .map(|t| t.pos + Vec3::Z * BODY_CENTRE * t.scale)
            };
            let Some(aim) = aim else { continue };
            let share = if spell.concentration {
                crate::ai::casters::CHANNEL_TICK
            } else {
                1.0
            };
            for _ in 0..n {
                let cost = self.spell_cost(actor, spell.spell) * share;
                if self.actor_value(actor, av::MAGICKA) < cost {
                    break;
                }
                self.damage_actor_value(actor, av::MAGICKA, cost);
                let dir = (aim - eye).normalize_or_zero();
                self.cast_from(actor, spell.spell, share, eye, dir);
            }
        }
    }

    /// A spell is cast by `caster` (the player, from the view; an actor, the
    /// way it faces), already paid for; `share` is the part of a second a
    /// concentration spell's landing stands for (1 for others).
    pub(crate) fn cast(&mut self, caster: FormId, spell: FormId, share: f32) {
        let (eye, dir) = if caster == PLAYER_REF {
            (self.camera.position, self.camera.forward())
        } else {
            match self.actor_ref(caster) {
                Some(a) => (
                    a.pos + Vec3::Z * 110.0 * a.scale,
                    Vec3::new(a.heading.sin(), a.heading.cos(), 0.0),
                ),
                None => return,
            }
        };
        self.cast_from(caster, spell, share, eye, dir);
    }

    /// A spell cast by `caster` from `eye` along `dir`.
    pub(crate) fn cast_from(
        &mut self,
        caster: FormId,
        spell: FormId,
        share: f32,
        eye: Vec3,
        dir: Vec3,
    ) {
        let Some(item) = self.magic_item(spell) else {
            return;
        };
        if item.cast_type != CONCENTRATION || share >= 1.0 {
            log::info!("{caster} casts {} ({spell})", item.name);
        }
        let hostile = self.item_hostile(&item);
        // Non-hostile spells teach by being cast; hostile ones by landing.
        if caster == PLAYER_REF && !hostile {
            self.magic_skill_use(&item, share);
        }
        if caster == PLAYER_REF && share >= 1.0 && item.spell_type == spell_type::SPELL {
            let mut e = crate::story::StoryEvent::new(b"CAST");
            e.refs[0] = caster;
            e.form = spell;
            self.send_story_event(e);
        }
        match item.delivery {
            // A hostile spell around the caster (Battle Cry) spares them.
            delivery::SELF if hostile => {
                let at = self.feet_of(caster).unwrap_or(eye);
                self.spell_lands(caster, spell, None, at, dir)
            }
            delivery::SELF => self.spell_lands(caster, spell, Some(caster), eye, dir),
            delivery::CONTACT | delivery::TARGET_ACTOR => {
                let reach = if item.delivery == delivery::CONTACT {
                    CONTACT_REACH
                } else if item.range > 0.0 {
                    item.range
                } else {
                    TARGET_RANGE
                };
                if let Some((t, Some(actor))) = self.spell_ray(eye, dir, reach, caster)
                    && self.is_actor_ref(actor)
                {
                    self.spell_lands(caster, spell, Some(actor), eye + dir * t, dir);
                }
            }
            _ => {
                let shot = self.item_projectile(&item).and_then(|p| {
                    crate::ai::archery::spell_projectile(&self.lo, p).map(|s| (p, s))
                });
                match shot {
                    Some((proj, shot)) if projectile_type(&self.lo, proj) & PROJ_CONE != 0 => {
                        let spread = cone_spread(&self.lo, proj);
                        for (actor, at) in self.actors_in_cone(caster, eye, dir, shot.range, spread)
                        {
                            self.spell_lands(caster, spell, Some(actor), at, dir);
                        }
                    }
                    Some((proj, shot)) if reaches_at_once(&self.lo, proj) => {
                        match self.spell_ray(eye, dir, shot.range, caster) {
                            Some((t, owner)) => {
                                let actor = owner.filter(|&o| self.is_actor_ref(o));
                                self.spell_lands(caster, spell, actor, eye + dir * t, dir);
                            }
                            None => {}
                        }
                    }
                    Some((_, shot)) => {
                        let from = eye + dir * 20.0 - Vec3::Z * 6.0;
                        self.spawn_spell_projectile(caster, spell, shot, from, dir);
                    }
                    None => {
                        if let Some((t, owner)) = self.spell_ray(eye, dir, LOCATION_RANGE, caster) {
                            let actor = owner.filter(|&o| self.is_actor_ref(o));
                            self.spell_lands(caster, spell, actor, eye + dir * t, dir);
                        }
                    }
                }
            }
        }
    }

    /// The live actors (the player too, for others) whose body's middle is
    /// within `range` of `eye` and `spread` of `dir`, with nothing solid in
    /// between: (actor, where it is struck).
    fn actors_in_cone(
        &self,
        caster: FormId,
        eye: Vec3,
        dir: Vec3,
        range: f32,
        spread: f32,
    ) -> Vec<(FormId, Vec3)> {
        let mut bodies: Vec<(FormId, Vec3)> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| !a.dead && a.ref_id != caster)
            .map(|a| (a.ref_id, a.pos + Vec3::Z * BODY_CENTRE * a.scale))
            .collect();
        if caster != PLAYER_REF {
            bodies.push((PLAYER_REF, self.player.position));
        }
        bodies
            .into_iter()
            .filter(|&(actor, at)| {
                let to = at - eye;
                let d = to.length();
                if d > range || d < 1.0 || to.normalize().angle_between(dir) > spread {
                    return false;
                }
                // Walls shelter: the first thing along the way is the actor.
                match self.physics.raycast_excluding(eye, to / d, d, caster) {
                    Some((_, owner)) => owner.is_none_or(|o| o == actor),
                    None => true,
                }
            })
            .collect()
    }

    /// What a spell reaching along `dir` from `eye` meets first within
    /// `range`: the world, an actor, or the player (who isn't in the physics
    /// world: their capsule) for anyone else's.
    fn spell_ray(
        &self,
        eye: Vec3,
        dir: Vec3,
        range: f32,
        caster: FormId,
    ) -> Option<(f32, Option<FormId>)> {
        let world = self.physics.raycast_excluding(eye, dir, range, caster);
        if caster == PLAYER_REF {
            return world;
        }
        let (r, h) = (self.physics.player_radius, self.physics.player_half_height);
        let p = self.player.position;
        let (s, d) = crate::ai::archery::segment_distance(
            eye,
            eye + dir * range,
            p - Vec3::Z * h,
            p + Vec3::Z * h,
        );
        match world {
            _ if d >= r => world,
            Some((t, _)) if t < s * range => world,
            _ => Some((s * range, Some(PLAYER_REF))),
        }
    }

    /// Where an actor (or the player) stands.
    fn feet_of(&self, actor: FormId) -> Option<Vec3> {
        if actor == PLAYER_REF {
            return Some(self.player.position);
        }
        self.actor_ref(actor).map(|a| a.pos)
    }

    /// Whether a reference is an actor (placed or made while playing).
    fn is_actor_ref(&self, r: FormId) -> bool {
        r == PLAYER_REF
            || self.created(r).map_or_else(
                || self.lo.tag_of(r).is_some_and(|t| t.0 == *b"ACHR"),
                |c| c.actor,
            )
    }

    /// Whether any of a spell's effects is hostile.
    fn item_hostile(&self, item: &MagicItem) -> bool {
        item.effects.iter().any(|e| {
            self.magic_effect(e.effect)
                .is_some_and(|m| m.has(flags::HOSTILE))
        })
    }

    /// The projectile a spell flies as: its first effect's that has one.
    fn item_projectile(&self, item: &MagicItem) -> Option<FormId> {
        item.effects.iter().find_map(|e| {
            self.magic_effect(e.effect)
                .map(|m| m.projectile)
                .filter(|p| !p.is_null())
        })
    }

    /// A spell lands at `at` (flying along `dir`): on the actor struck, and on
    /// every actor within its area; its impact there.
    pub(crate) fn spell_lands(
        &mut self,
        caster: FormId,
        spell: FormId,
        target: Option<FormId>,
        at: Vec3,
        dir: Vec3,
    ) {
        let Some(item) = self.magic_item(spell) else {
            return;
        };
        if target != Some(caster)
            && let Some(ipds) = item
                .effects
                .iter()
                .find_map(|e| self.magic_effect(e.effect).map(|m| m.impacts))
                .filter(|i| !i.is_null())
        {
            self.impact_at(ipds, target, at, dir);
        }
        let mut struck: Vec<FormId> = target.into_iter().collect();
        let area = item.effects.iter().map(|e| e.area).max().unwrap_or(0) as f32;
        if area > 0.0 {
            let radius = area * UNITS_PER_FOOT;
            let near = self
                .cells
                .values()
                .flat_map(|rt| &rt.actors)
                .filter(|a| !a.dead && a.ref_id != caster)
                .filter(|a| (a.pos + Vec3::Z * BODY_CENTRE * a.scale).distance(at) <= radius)
                .map(|a| a.ref_id);
            let player = (caster != PLAYER_REF && self.player.position.distance(at) <= radius)
                .then_some(PLAYER_REF);
            for a in near.chain(player) {
                if !struck.contains(&a) {
                    struck.push(a);
                }
            }
        }
        let hostile = self.item_hostile(&item);
        let share = if item.cast_type == CONCENTRATION {
            CONCENTRATION_TICK
        } else {
            1.0
        };
        for t in struck {
            if self.apply_item(spell, Some(caster), t)
                && hostile
                && caster == PLAYER_REF
                && t != PLAYER_REF
            {
                self.magic_skill_use(&item, share);
            }
        }
    }

    /// The player cast a spell (or a hostile one landed): its school's skill
    /// gets the spell's base cost as XP (`share` of it for a concentration
    /// spell's landing), times its costliest effect's skill usage multiplier.
    fn magic_skill_use(&mut self, item: &MagicItem, share: f32) {
        if item.spell_type != spell_type::SPELL {
            return;
        }
        let Some(school) = self.spell_school(item.id) else {
            return;
        };
        let base = self
            .lo
            .get(item.id)
            .and_then(|r| {
                r.get(b"SPIT")
                    .filter(|d| d.len() >= 4)
                    .map(|d| u32::from_le_bytes(d[0..4].try_into().unwrap()) as f32)
            })
            .unwrap_or(0.0);
        let mult = item
            .effects
            .iter()
            .filter_map(|e| self.magic_effect(e.effect))
            .find(|m| m.skill == Some(school))
            .map_or(1.0, |m| m.skill_usage_mult);
        self.use_skill(school, base * mult * share);
    }
}

/// A projectile's type flags (`PROJ` `DATA`).
fn projectile_type(lo: &esp::LoadOrder, proj: FormId) -> u16 {
    lo.get(proj)
        .and_then(|r| {
            r.get(b"DATA")
                .filter(|d| d.len() >= 4)
                .map(|d| u16::from_le_bytes([d[2], d[3]]))
        })
        .unwrap_or(0)
}

/// Whether a projectile reaches its range at once (flames, beams, cones)
/// rather than flying.
fn reaches_at_once(lo: &esp::LoadOrder, proj: FormId) -> bool {
    projectile_type(lo, proj) & (PROJ_BEAM | PROJ_FLAME | PROJ_CONE) != 0
}

/// A cone projectile's spread (`PROJ` `DATA` 0x44, degrees), as radians
/// either side of its middle.
fn cone_spread(lo: &esp::LoadOrder, proj: FormId) -> f32 {
    lo.get(proj)
        .and_then(|r| {
            r.get(b"DATA")
                .filter(|d| d.len() >= 0x48)
                .map(|d| f32::from_le_bytes(d[0x44..0x48].try_into().unwrap()))
        })
        .filter(|s| *s > 0.0)
        .unwrap_or(15.0)
        .to_radians()
}
