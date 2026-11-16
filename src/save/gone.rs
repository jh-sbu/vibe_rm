//! Loading without some of a save's plugins (`plans/saves-remaining.md`,
//! step 7). Their forms were moved where they resolve to nothing
//! (`super::plugin_map`) and are otherwise kept as they were, inert, as a
//! record a plugin update removed would be. What `Snapshot::apply` checks
//! against the records, and would refuse a save over, is dropped here when
//! it names a gone plugin's form; anything else that doesn't resolve is
//! still refused as damage. The choices are in `known_ambiguities/saves.md`.

use std::collections::HashSet;

use esp::FormId;

use super::state::{Snapshot, Spot};
use crate::engine::Location;

impl Snapshot {
    /// Drop what can't stand without the gone plugins' forms, before
    /// `apply` validates the rest. A save standing in a gone plugin's cell
    /// or worldspace is refused, naming the plugins.
    pub(super) fn drop_gone(&mut self) -> anyhow::Result<()> {
        if self.gone.is_none() {
            return Ok(());
        }
        let gone = |f: FormId| self.is_gone(f);
        let gone_opt = |f: Option<FormId>| f.is_some_and(gone);
        if let Some(Spot { location, .. }) = &self.spot {
            let place = match *location {
                Location::Interior(cell) => cell,
                Location::Exterior { world, .. } => world,
                Location::Nowhere => FormId::NULL,
            };
            anyhow::ensure!(
                !gone(place),
                "the save stands in a place from a plugin that isn't loaded ({})",
                self.missing_plugins().join(", ")
            );
        }

        let weapon_usage: Vec<_> = self
            .weapon_usage
            .iter()
            .copied()
            .filter(|&(w, _)| !gone(w))
            .collect();
        // A gone favorite gives way to the most used weapon left (the
        // lowest id of those tied).
        let favorite = match self.favorite_weapon {
            Some(w) if gone(w) => weapon_usage
                .iter()
                .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
                .map(|&(w, _)| w),
            f => f,
        };
        let power_uses: Vec<_> = self
            .power_uses
            .keys()
            .copied()
            .filter(|&p| gone(p))
            .collect();

        let w = &self.weather;
        let weather_gone = [w.current, w.outgoing, w.before_override]
            .into_iter()
            .any(gone_opt);

        // Scenes go with their records; a gone actor's part in one goes
        // with it.
        let scene_gone: HashSet<FormId> = self
            .scenes
            .iter()
            .map(|s| s.scene)
            .chain(self.scene_lifecycle.iter().map(|s| s.scene))
            .chain(self.scene_bindings.iter().map(|s| s.scene))
            .chain(self.scene_repeats.iter().map(|s| s.scene))
            .chain(self.scene_dialogue.iter().map(|s| s.scene))
            .chain(self.scene_restarts.iter().copied())
            .filter(|&s| gone(s))
            .chain(
                self.scene_dialogue
                    .iter()
                    .filter(|d| gone(d.topic) || gone(d.info))
                    .map(|d| d.scene),
            )
            .chain(
                self.scene_bindings
                    .iter()
                    .filter(|b| gone_opt(b.actor))
                    .map(|b| b.scene),
            )
            .collect();

        let rolled_gone: Vec<bool> = self
            .rolled_spells
            .iter()
            .map(|r| gone(r.actor) || gone(r.list) || r.spells.iter().any(|&s| gone(s)))
            .collect();
        let speech_gone: Vec<bool> = self
            .continuity
            .package_speech
            .iter()
            .map(|s| {
                gone(s.actor)
                    || gone(s.owner.package)
                    || gone(s.topic)
                    || gone(s.info)
                    || gone_opt(s.listener)
            })
            .collect();
        let effect_gone = |x: &crate::magic::ActiveEffect| {
            gone(x.target)
                || gone_opt(x.caster)
                || gone(x.item)
                || gone(x.effect)
                || gone_opt(x.equipment)
                || gone_opt(x.summoned)
        };
        let effects_gone: Vec<bool> = self
            .continuity
            .active_effects
            .0
            .iter()
            .map(effect_gone)
            .collect();
        let tapers_gone: Vec<bool> = self
            .continuity
            .tapers
            .iter()
            .map(|t| effect_gone(&t.0))
            .collect();
        let package_gone: Vec<FormId> = self
            .continuity
            .executions
            .iter()
            .filter(|(_, x)| {
                [x.run.as_ref(), x.suspended.as_ref()]
                    .into_iter()
                    .flatten()
                    .any(|r| gone(r.id))
                    || gone_opt(x.done_once)
            })
            .map(|(&actor, _)| actor)
            .collect();
        let dropped_effects: HashSet<u64> = self
            .continuity
            .active_effects
            .0
            .iter()
            .zip(&effects_gone)
            .filter(|(_, g)| **g)
            .map(|(x, _)| x.id)
            .collect();

        self.weapon_usage = weapon_usage;
        self.favorite_weapon = favorite;
        for p in power_uses {
            self.power_uses.remove(&p);
        }
        if weather_gone {
            // The place picks a weather as it does for a save without one.
            self.weather = Default::default();
        }
        self.scenes.retain(|s| !scene_gone.contains(&s.scene));
        self.scene_lifecycle
            .retain(|s| !scene_gone.contains(&s.scene));
        self.scene_bindings
            .retain(|s| !scene_gone.contains(&s.scene));
        self.scene_dialogue
            .retain(|s| !scene_gone.contains(&s.scene));
        self.scene_repeats
            .retain(|s| !scene_gone.contains(&s.scene));
        self.scene_restarts.retain(|s| !scene_gone.contains(s));
        let mut i = 0;
        self.rolled_spells.retain(|_| {
            i += 1;
            !rolled_gone[i - 1]
        });
        let mut i = 0;
        self.continuity.package_speech.retain(|_| {
            i += 1;
            !speech_gone[i - 1]
        });
        let mut i = 0;
        self.continuity.active_effects.0.retain(|_| {
            i += 1;
            !effects_gone[i - 1]
        });
        let mut i = 0;
        self.continuity.tapers.retain(|_| {
            i += 1;
            !tapers_gone[i - 1]
        });
        // Their scripts can't stand without them.
        self.vm.drop_effect_objects(&dropped_effects);
        // An actor whose package is gone picks one afresh.
        for actor in package_gone {
            if let Some(x) = self.continuity.executions.get_mut(&actor) {
                if x.run
                    .as_ref()
                    .is_some_and(|r| self.gone.as_ref().is_some_and(|m| m.is_gone(r.id)))
                {
                    x.run = None;
                    x.current = None;
                }
                if x.suspended
                    .as_ref()
                    .is_some_and(|r| self.gone.as_ref().is_some_and(|m| m.is_gone(r.id)))
                {
                    x.suspended = None;
                }
                if x.done_once
                    .is_some_and(|p| self.gone.as_ref().is_some_and(|m| m.is_gone(p)))
                {
                    x.done_once = None;
                }
            }
        }
        Ok(())
    }
}
