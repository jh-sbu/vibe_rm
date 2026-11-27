//! Illusion's influence on fighting: calm, fear (demoralize, turn undead),
//! frenzy and rally effects hold their target's aggression or confidence at
//! a value of their own (`crate::magic::archetype::target_value`), and what
//! the actor is doing changes with it. Calmed actors stop fighting and look
//! for no fights until attacked; the frightened, cowards while it lasts,
//! flee at once; the frenzied fall on the player who cast it and is seen
//! (else whoever is nearest, friend or not), and give up the fights they no
//! longer want when it ends; the rallied stop
//! fleeing. Open questions: `known_ambiguities/magic.md`.

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

    /// A target value modifier from `caster` began holding its value.
    pub(crate) fn target_value_started(
        &mut self,
        actor: FormId,
        archetype: u32,
        caster: Option<FormId>,
    ) {
        if actor == PLAYER_REF {
            return;
        }
        match archetype {
            archetype::CALM => {
                if self
                    .actor_ref(actor)
                    .is_some_and(|a| a.exec.combat.is_some())
                {
                    log::info!("{actor} is calmed");
                    self.end_combat(actor);
                }
            }
            archetype::FRENZY => {
                // Skyrim SE: cast by the player it finds, it goes for the
                // player, even with another actor nearer (autotest27,
                // autotest28); cast by an NPC, or by the player unseen, for
                // the nearest it finds, and with nobody it finds, nobody
                // (autotest30).
                let caster = caster.filter(|&c| {
                    c == PLAYER_REF && self.frenzy_can_attack(c) && self.finds_player(actor)
                });
                if let Some(t) = caster.or_else(|| self.frenzy_target(actor)) {
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
        if let Some(c) = self.actor_mut(actor).and_then(|a| a.exec.combat.as_mut()) {
            c.recheck = true;
        }
    }

    /// Whether a frenzied actor can go for `r`: living, standing, loaded.
    fn frenzy_can_attack(&self, r: FormId) -> bool {
        if r == PLAYER_REF {
            return !self.player_dead();
        }
        self.actor_ref(r)
            .is_some_and(|a| !a.dead && a.exec.bleeding.is_none())
    }

    /// The nearest living actor (or the player) a frenzied actor finds.
    fn frenzy_target(&self, actor: FormId) -> Option<FormId> {
        let pos = self.actor_ref(actor)?.exec.pos;
        let mut best: Option<(f32, FormId)> = None;
        let d = pos.distance(self.player.position);
        if d < ENGAGE_DISTANCE && !self.player_dead() && self.finds_player(actor) {
            best = Some((d, PLAYER_REF));
        }
        for a in self.cells.values().flat_map(|rt| &rt.actors) {
            let d = pos.distance(a.exec.pos);
            if a.ref_id == actor
                || a.dead
                || a.exec.bleeding.is_some()
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
            .map(|a| (a.stats.clone(), a.exec.combat.as_ref().map(|c| c.target)))
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
            .filter(|a| !a.dead && a.exec.combat.as_ref().is_some_and(|c| c.target == actor))
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

#[cfg(test)]
mod tests {
    use crate::engine::PLAYER_REF;
    use crate::magic::archetype;
    use esp::FormId;
    use glam::{Mat4, Vec3};

    #[test]
    fn a_fury_sends_its_target_at_the_player_who_cast_it_only_if_it_finds_them() {
        use crate::world::skeleton::Skeleton;
        use esp::writer::write_plugin;
        use std::sync::Arc;

        let dir = std::env::temp_dir().join(format!("vrm-fury-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("Empty.esp"),
            write_plugin(0, &[], "test", "fury", 0x900, &[]),
        )
        .unwrap();
        let mut e = crate::stats::action_tests::headless_engine(&dir, "Empty.esp");
        let key = crate::render::CellKey::Interior(FormId(0x801));
        let skeleton = Arc::new(Skeleton::default());
        for (id, x) in [
            (FormId(0x800), 0.0),
            (FormId(0x803), 150.0),
            (FormId(0x804), -400.0),
        ] {
            let mut a = crate::ai::ActorRuntime::new(
                id,
                FormId(0x802),
                skeleton.clone(),
                Mat4::IDENTITY,
                vec![],
                0.0,
            );
            a.exec.pos = Vec3::X * x;
            e.actor_cells.insert(id, key);
            e.cells.entry(key).or_default().actors.push(a);
        }
        e.player.position = Vec3::Y * -450.0;
        let target = |e: &crate::engine::Engine| {
            e.actor_ref(FormId(0x800))
                .unwrap()
                .exec
                .combat
                .as_ref()
                .map(|c| c.target)
        };
        // Cast by the player it doesn't find (autotest30's booth): not at them.
        e.target_value_started(FormId(0x800), archetype::FRENZY, Some(PLAYER_REF));
        assert_ne!(target(&e), Some(PLAYER_REF));
        // Cast by an NPC: not at its caster.
        e.target_value_started(FormId(0x800), archetype::FRENZY, Some(FormId(0x804)));
        assert_ne!(target(&e), Some(FormId(0x804)));
        // Cast by the player it finds: at the player (autotest27, autotest28).
        e.detection.of_player.insert(FormId(0x800), 100.0);
        e.target_value_started(FormId(0x800), archetype::FRENZY, Some(PLAYER_REF));
        assert_eq!(target(&e), Some(PLAYER_REF));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
