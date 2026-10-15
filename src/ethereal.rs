//! Ethereal (Become Ethereal: `VoiceMakeEthereal`, archetype 41; its script
//! only fades the actor's alpha). UESP's Skyrim:Become Ethereal: an
//! ethereal actor "cannot deal damage or take damage (including damage from
//! falls or poison)", isn't staggered, and fighting or interacting with the
//! world ends it. Here blows and bashes to or from an ethereal actor do
//! nothing, magic takes none of its health and doesn't stagger it, falls
//! spare it, and the player attacking, casting or activating ends it.
//! Scripts' `DamageActorValue` still hurts (UESP: the Twilight Sepulcher's
//! light, Apocrypha's darkness). `known_ambiguities/magic.md`.

use esp::FormId;

use crate::engine::{Engine, PLAYER_REF};
use crate::magic::archetype;

impl Engine {
    /// Whether an actor (or the player) is ethereal.
    pub fn is_ethereal(&self, actor: FormId) -> bool {
        self.magic.effects.iter().any(|x| {
            x.target == actor
                && !x.done
                && self
                    .magic_effect(x.effect)
                    .is_some_and(|m| m.archetype == archetype::ETHEREALIZE)
        })
    }

    /// The player does something that ends being ethereal: attacking,
    /// casting, activating.
    pub(crate) fn player_breaks_ethereal(&mut self) {
        if self.is_ethereal(PLAYER_REF) {
            log::info!("player is no longer ethereal");
            self.dispel(PLAYER_REF, |x| x.effect_archetype == archetype::ETHEREALIZE);
        }
    }
}
