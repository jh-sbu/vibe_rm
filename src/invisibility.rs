//! Timed invisibility ends when its actor attacks, casts other magic or
//! activates something. Action boundaries and unsourced cases are recorded in
//! `known_ambiguities/invisibility.md`.

use esp::FormId;

use crate::casting::delivery;
use crate::engine::Engine;
use crate::magic::{MagicItem, archetype};

impl Engine {
    /// End only timed invisibility, leaving other effects of its source item
    /// alone. Constant abilities/enchantments have their own reconciliation
    /// lifecycle, whose interruption rules are not established.
    pub(crate) fn break_invisibility(&mut self, actor: FormId) {
        let ids: Vec<u64> = self
            .magic
            .effects
            .iter()
            .filter(|x| x.target == actor && !x.done && x.duration.is_finite())
            .filter(|x| {
                self.magic_effect(x.effect)
                    .is_some_and(|m| m.archetype == archetype::INVISIBILITY)
            })
            .map(|x| x.id)
            .collect();
        for id in ids {
            self.dispel_effect(id);
        }
    }

    /// Charging is harmless; an actual delivery of non-invisibility magic
    /// interrupts it. A self invisibility recast can renew the effect without
    /// a gap, including when cast while an earlier source is still active.
    pub(crate) fn casting_breaks_invisibility(&mut self, caster: FormId, item: &MagicItem) {
        let renews = item.delivery == delivery::SELF
            && item.effects.iter().any(|e| {
                self.magic_effect(e.effect)
                    .is_some_and(|m| m.archetype == archetype::INVISIBILITY)
            });
        if !renews {
            self.break_invisibility(caster);
        }
    }
}

#[cfg(test)]
#[path = "invisibility_tests.rs"]
mod tests;
