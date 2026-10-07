//! Actor values: one store per actor that conditions, Papyrus and the console
//! share.
//!
//! An actor value is its base (from the records, unless a script set it) plus a
//! permanent modifier (`ModActorValue`, `ForceActorValue`) and the damage it has
//! taken (`DamageActorValue`, healed by `RestoreActorValue`). Loaded actors'
//! health and stamina live with their AI (combat spends and regenerates them), so
//! for those the current value is the live one and the store keeps their maximum;
//! AI data values (aggression, confidence, assistance) and the skills combat uses
//! are pushed into the actor's combat stats when they change.

use std::sync::Arc;

use esp::FormId;
use esp::actor_value as av;

use crate::ai::combat::CombatStats;
use crate::engine::{Engine, PLAYER_HEALTH, PLAYER_REF};
use crate::world::template;

/// What scripts did to one actor value over its base.
#[derive(Debug, Clone, Copy, Default)]
pub struct Modifiers {
    /// Set by `SetActorValue`, in place of the records' base.
    pub base: Option<f32>,
    pub permanent: f32,
    /// Damage taken (never above 0).
    pub damage: f32,
}

fn f32_at(d: &[u8], o: usize) -> f32 {
    d.get(o..o + 4).map_or(0.0, |b| f32::from_le_bytes(b.try_into().unwrap()))
}

impl Engine {
    fn av_mods(&self, actor: FormId, index: u32) -> Modifiers {
        self.scripts.actor_values.get(&(actor, index)).copied().unwrap_or_default()
    }

    /// An actor value's base as the records give it: AI data (`AIDT`), skills
    /// and health / magicka / stamina offsets (`DNAM`) from the templates that give
    /// them, starting values and rates from the race (`DATA`).
    pub fn av_record_base(&self, actor: FormId, index: u32) -> f32 {
        let Some(src) = self.templates_of(actor) else { return if index == av::SPEED_MULT { 100.0 } else { 0.0 } };
        let lo = &self.lo;
        let stats = || src.field(lo, template::STATS, b"DNAM").unwrap_or_default();
        let race = || src.form(lo, template::TRAITS, b"RNAM").and_then(|r| lo.get(r)).and_then(|r| r.get(b"DATA").map(<[u8]>::to_vec)).unwrap_or_default();
        let offset = |o: usize| stats().get(o..o + 2).map_or(0.0, |d| u16::from_le_bytes([d[0], d[1]]) as f32);
        match index {
            av::AGGRESSION..=av::ASSISTANCE => src.field(lo, template::AI_DATA, b"AIDT").and_then(|d| d.get(index as usize).copied()).unwrap_or(0) as f32,
            av::FIRST_SKILL..=av::LAST_SKILL => stats().get((index - av::FIRST_SKILL) as usize).copied().unwrap_or(15) as f32,
            av::HEALTH if actor == PLAYER_REF => PLAYER_HEALTH,
            // As combat works them out (`CombatStats::of`).
            av::HEALTH => (f32_at(&race(), 36) + offset(36)).max(5.0),
            av::MAGICKA => f32_at(&race(), 40) + offset(38),
            av::STAMINA => (f32_at(&race(), 44) + offset(40)).max(10.0),
            av::HEAL_RATE => f32_at(&race(), 84),
            av::MAGICKA_RATE => f32_at(&race(), 88),
            av::STAMINA_RATE => f32_at(&race(), 92),
            av::CARRY_WEIGHT => f32_at(&race(), 48),
            av::MASS => f32_at(&race(), 52),
            av::SPEED_MULT => 100.0,
            _ => 0.0,
        }
    }

    /// `GetBaseActorValue`.
    pub fn av_base(&self, actor: FormId, index: u32) -> f32 {
        self.av_mods(actor, index).base.unwrap_or_else(|| self.av_record_base(actor, index))
    }

    /// The most it can be (`GetPermanentActorValue`): the base and the permanent
    /// modifier.
    pub fn av_max(&self, actor: FormId, index: u32) -> f32 {
        self.av_base(actor, index) + self.av_mods(actor, index).permanent
    }

    /// Health and stamina of a loaded actor or the player (current, max).
    fn av_live(&self, actor: FormId, index: u32) -> Option<(f32, f32)> {
        match index {
            av::HEALTH if actor == PLAYER_REF => Some((self.player_health, self.player_max_health())),
            av::HEALTH => self.actor_health(actor),
            // The player's stamina is unset (infinite) until first worked out.
            av::STAMINA => self.stamina(actor).map(|(cur, max)| (cur.min(max), max)),
            _ => None,
        }
    }

    /// `GetActorValue`: what it is now.
    pub fn actor_value(&self, actor: FormId, index: u32) -> f32 {
        match self.av_live(actor, index) {
            Some((current, _)) => current,
            None => self.av_max(actor, index) + self.av_mods(actor, index).damage,
        }
    }

    /// `GetActorValuePercent(age)`: what it is now over the most it can be (0..1).
    pub fn actor_value_fraction(&self, actor: FormId, index: u32) -> f32 {
        let (current, max) = self.av_live(actor, index).unwrap_or_else(|| (self.actor_value(actor, index), self.av_max(actor, index)));
        if max > 0.0 { (current / max).clamp(0.0, 1.0) } else { 0.0 }
    }

    pub fn player_max_health(&self) -> f32 {
        self.av_max(PLAYER_REF, av::HEALTH)
    }

    /// `SetActorValue`: a new base. Health and stamina rise or fall with the most
    /// they can be.
    pub fn set_actor_value(&mut self, actor: FormId, index: u32, value: f32) {
        let before = self.av_max(actor, index);
        self.scripts.actor_values.entry((actor, index)).or_default().base = Some(value);
        self.actor_value_changed(actor, index, before);
    }

    /// `ModActorValue`: raise (or lower) the most it can be, and what it is now.
    pub fn mod_actor_value(&mut self, actor: FormId, index: u32, delta: f32) {
        let before = self.av_max(actor, index);
        self.scripts.actor_values.entry((actor, index)).or_default().permanent += delta;
        self.actor_value_changed(actor, index, before);
    }

    /// `ForceActorValue`: make it this now, through the permanent modifier.
    pub fn force_actor_value(&mut self, actor: FormId, index: u32, value: f32) {
        let delta = value - self.actor_value(actor, index);
        self.mod_actor_value(actor, index, delta);
    }

    /// `DamageActorValue` (positive `amount`) and `RestoreActorValue` (negative):
    /// restoring heals damage but never raises it past its most. Health taken
    /// to nothing kills (essential actors bleed out).
    pub fn damage_actor_value(&mut self, actor: FormId, index: u32, amount: f32) {
        if let Some((current, max)) = self.av_live(actor, index) {
            let value = (current - amount).min(max);
            match index {
                av::HEALTH if actor == PLAYER_REF => {
                    if amount > 0.0 {
                        self.damage(actor, amount, None, 0.0);
                    } else {
                        self.player_health = value;
                    }
                }
                av::HEALTH => {
                    if let Some(a) = self.actor_mut(actor).filter(|a| !a.dead && a.bleeding.is_none()) {
                        a.health = value.max(0.0);
                        if value <= 0.0 {
                            self.kill(actor, false);
                        }
                    }
                }
                _ => {
                    if amount > 0.0 {
                        self.spend_stamina(actor, amount);
                    } else {
                        self.set_stamina(actor, value);
                    }
                }
            }
            return;
        }
        let m = self.scripts.actor_values.entry((actor, index)).or_default();
        m.damage = (m.damage - amount).min(0.0);
    }

    /// After a change to its most, a loaded actor's (or the player's) combat
    /// stats follow, and its live health / stamina move by as much.
    fn actor_value_changed(&mut self, actor: FormId, index: u32, before: f32) {
        let delta = self.av_max(actor, index) - before;
        let mut stats = if actor == PLAYER_REF {
            self.player_stats.get().map(|s| CombatStats::clone(s))
        } else {
            self.actor_ref(actor).map(|a| CombatStats::clone(&a.stats))
        };
        let Some(s) = stats.as_mut() else { return };
        self.apply_actor_values(actor, s);
        let s = Arc::new(stats.unwrap());
        if actor == PLAYER_REF {
            if index == av::HEALTH {
                self.player_health = (self.player_health + delta).clamp(0.0, self.player_max_health());
            }
            if index == av::STAMINA {
                self.player_stamina = (self.player_stamina + delta).clamp(0.0, s.max_stamina);
            }
            if let Some(p) = self.player_stats.get_mut() {
                *p = s;
            }
        } else if let Some(a) = self.actor_mut(actor) {
            match index {
                av::HEALTH => a.health = (a.health + delta).clamp(1.0, s.max_health),
                av::STAMINA => a.stamina = (a.stamina + delta).clamp(0.0, s.max_stamina),
                _ => {}
            }
            a.stats = s;
        }
    }

    /// Put what scripts did to an actor's values into the stats combat runs on
    /// (as it loads, and when they change).
    pub(crate) fn apply_actor_values(&self, actor: FormId, s: &mut CombatStats) {
        let changed = |i: u32| self.scripts.actor_values.contains_key(&(actor, i));
        if changed(av::HEALTH) && actor != PLAYER_REF {
            s.max_health = self.av_max(actor, av::HEALTH).max(1.0);
        }
        if changed(av::STAMINA) {
            s.max_stamina = self.av_max(actor, av::STAMINA).max(0.0);
        }
        if changed(av::AGGRESSION) {
            s.aggression = self.actor_value(actor, av::AGGRESSION).clamp(0.0, 3.0) as u8;
        }
        if changed(av::ASSISTANCE) {
            s.assistance = self.actor_value(actor, av::ASSISTANCE).clamp(0.0, 2.0) as u8;
        }
        if changed(av::CONFIDENCE) {
            s.confidence = self.actor_value(actor, av::CONFIDENCE).clamp(0.0, 4.0) as u8;
            s.confidence_value = crate::ai::threat::confidence_value(&self.lo, s.confidence);
        }
        if changed(av::LIGHT_ARMOR) {
            s.armor_skills[0] = self.actor_value(actor, av::LIGHT_ARMOR);
        }
        if changed(av::HEAVY_ARMOR) {
            s.armor_skills[1] = self.actor_value(actor, av::HEAVY_ARMOR);
        }
        if changed(av::BLOCK) {
            s.block_skill = self.actor_value(actor, av::BLOCK);
        }
    }

    /// An actor value by its name, for scripts and the console: the store for the
    /// known ones, a plain number per name for the rest.
    pub fn actor_value_named(&self, actor: FormId, name: &str) -> f32 {
        match av::index(name) {
            Some(i) => self.actor_value(actor, i),
            None => self.scripts.other_actor_values.get(&(actor, name.to_ascii_lowercase())).copied().unwrap_or(0.0),
        }
    }

    /// Console `getav` / `cstats`: an actor value's parts.
    pub fn describe_actor_value(&self, actor: FormId, index: u32) -> String {
        let m = self.av_mods(actor, index);
        format!(
            "{} {:.2} (base {:.2}{}, max {:.2}, damage {:.2})",
            av::name(index).unwrap_or("?"),
            self.actor_value(actor, index),
            self.av_base(actor, index),
            if m.base.is_some() { " set" } else { "" },
            self.av_max(actor, index),
            m.damage
        )
    }
}
