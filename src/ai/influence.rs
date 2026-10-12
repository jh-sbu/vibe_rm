//! Illusion's influence on fighting: calm, fear (demoralize, turn undead),
//! frenzy and rally effects hold their target's aggression or confidence at
//! a value of their own (`crate::magic::archetype::target_value`), and what
//! the actor is doing changes with it. Calmed actors stop fighting and look
//! for no fights until attacked; the frightened, cowards while it lasts,
//! flee at once; the frenzied fall on whoever is nearest, friend or not, and
//! give up the fights they no longer want when it ends; the rallied stop
//! fleeing. Open questions: `known_gaps/magic.md`.

use std::collections::HashSet;

use esp::FormId;

use crate::engine::{Engine, PLAYER_REF};
use crate::magic::archetype;

use super::combat::ENGAGE_DISTANCE;

impl Engine {
    /// The actors with an active effect of this archetype on them.
    pub(crate) fn under_archetype(&self, archetype: u32) -> HashSet<FormId> {
        self.magic
            .effects
            .iter()
            .filter(|x| x.active && !x.done)
            .filter(|x| {
                self.magic_effect(x.effect)
                    .is_some_and(|m| m.archetype == archetype)
            })
            .map(|x| x.target)
            .collect()
    }

    fn under(&self, actor: FormId, archetype: u32) -> bool {
        self.magic.effects.iter().any(|x| {
            x.target == actor
                && x.active
                && !x.done
                && self
                    .magic_effect(x.effect)
                    .is_some_and(|m| m.archetype == archetype)
        })
    }

    pub(crate) fn is_frenzied(&self, actor: FormId) -> bool {
        self.under(actor, archetype::FRENZY)
    }

    /// Attacked, a calmed actor is calm no longer.
    pub(crate) fn break_calm(&mut self, actor: FormId) {
        if self.under(actor, archetype::CALM) {
            log::info!("{actor}: attacked, no longer calm");
            self.dispel(actor, |x| x.effect_archetype == archetype::CALM);
        }
    }

    /// A target value modifier began holding its value.
    pub(crate) fn target_value_started(&mut self, actor: FormId, archetype: u32) {
        if actor == PLAYER_REF {
            return;
        }
        match archetype {
            archetype::CALM => {
                if self.actor_ref(actor).is_some_and(|a| a.combat.is_some()) {
                    log::info!("{actor} is calmed");
                    self.end_combat(actor);
                }
            }
            archetype::FRENZY => {
                if let Some(t) = self.frenzy_target(actor) {
                    log::info!("{actor} is frenzied: attacks {t}");
                    self.start_combat(actor, t);
                }
            }
            _ => self.recheck_flee(actor),
        }
    }

    /// A target value modifier let its value go.
    pub(crate) fn target_value_stopped(&mut self, actor: FormId, archetype: u32) {
        if actor == PLAYER_REF {
            return;
        }
        match archetype {
            archetype::FRENZY => self.frenzy_over(actor),
            archetype::CALM => {}
            _ => self.recheck_flee(actor),
        }
    }

    /// Its confidence changed: whether to flee is looked at again now.
    fn recheck_flee(&mut self, actor: FormId) {
        if let Some(c) = self.actor_mut(actor).and_then(|a| a.combat.as_mut()) {
            c.recheck = true;
        }
    }

    /// The nearest living actor (or the player) a frenzied actor finds.
    fn frenzy_target(&self, actor: FormId) -> Option<FormId> {
        let pos = self.actor_ref(actor)?.pos;
        let mut best: Option<(f32, FormId)> = None;
        let d = pos.distance(self.player.position);
        if d < ENGAGE_DISTANCE && !self.player_dead() && self.finds_player(actor) {
            best = Some((d, PLAYER_REF));
        }
        for a in self.cells.values().flat_map(|rt| &rt.actors) {
            let d = pos.distance(a.pos);
            if a.ref_id == actor
                || a.dead
                || a.bleeding.is_some()
                || d > ENGAGE_DISTANCE
                || best.is_some_and(|b| b.0 <= d)
            {
                continue;
            }
            if self.finds(actor, a.ref_id) {
                best = Some((d, a.ref_id));
            }
        }
        best.map(|b| b.1)
    }

    /// The frenzy over, the fights it started that neither side would have
    /// picked end: its own, and those against it.
    fn frenzy_over(&mut self, actor: FormId) {
        let Some(me) = self
            .actor_ref(actor)
            .map(|a| (a.stats.clone(), a.combat.as_ref().map(|c| c.target)))
        else {
            return;
        };
        let (stats, target) = me;
        let player_factions = self.player_factions();
        let factions_of = |e: &Engine, r: FormId| {
            if r == PLAYER_REF {
                player_factions.clone()
            } else {
                e.actor_ref(r)
                    .map(|a| a.stats.factions.clone())
                    .unwrap_or_default()
            }
        };
        if let Some(t) = target {
            let theirs = factions_of(self, t);
            if !self.hostile_to(&stats, &theirs, t == PLAYER_REF) {
                log::info!("{actor}: frenzy over, stops fighting {t}");
                self.end_combat(actor);
            }
        }
        let against: Vec<(FormId, std::sync::Arc<super::combat::CombatStats>)> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| !a.dead && a.combat.as_ref().is_some_and(|c| c.target == actor))
            .map(|a| (a.ref_id, a.stats.clone()))
            .collect();
        let mine = stats.factions.clone();
        for (r, theirs) in against {
            if !self.hostile_to(&theirs, &mine, false) {
                log::info!("{r}: {actor}'s frenzy over, stops fighting it");
                self.end_combat(r);
            }
        }
    }
}
