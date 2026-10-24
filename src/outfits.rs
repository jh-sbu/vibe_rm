//! Outfits scripts give actors (`Actor.SetOutfit`): the old outfit's armor
//! goes and the new one's is added and worn. The outfit is the reference's
//! (saved: `OUTF`) over its NPC's default (`DOFT`). Choices:
//! `known_ambiguities/outfits.md`.

use std::collections::HashMap;

use esp::FormId;

use crate::engine::Engine;

#[derive(Default)]
pub struct Outfits {
    /// The default outfit scripts gave each actor.
    pub(crate) set: HashMap<FormId, FormId>,
}

impl Engine {
    /// An actor's default outfit: as a script set it, else its NPC's
    /// (`DOFT`, through its templates' inventory).
    pub fn default_outfit(&self, actor: FormId) -> Option<FormId> {
        if let Some(&o) = self.outfits.set.get(&actor) {
            return Some(o);
        }
        self.templates_of(actor)?
            .form(&self.lo, crate::world::template::INVENTORY, b"DOFT")
    }

    /// `Actor.SetOutfit(akOutfit, abSleepOutfit)`: the old outfit's armor is
    /// taken away and the new outfit's added and worn. Sleep outfits aren't
    /// worn, so setting one does nothing.
    pub fn set_outfit(&mut self, actor: FormId, outfit: FormId, sleep: bool) {
        if sleep || actor.is_null() || actor == crate::engine::PLAYER_REF {
            return;
        }
        if self.lo.tag_of(outfit).map(|t| t.0) != Some(*b"OTFT") {
            return;
        }
        let old = self.default_outfit(actor);
        if old == Some(outfit) {
            return;
        }
        let seed = actor.0 as u64;
        let armors = |e: &Self, o: FormId| {
            crate::world::actor::outfit_armors(&e.lo, o, seed, &e.scripts.globals)
        };
        let gone = old.map(|o| armors(self, o)).unwrap_or_default();
        let worn = armors(self, outfit);
        // Make sure the inventory exists (with the old outfit) before changing it.
        self.inventory_mut(actor);
        for &a in gone.iter().filter(|a| !worn.contains(a)) {
            self.remove_item(actor, a, 1, None);
        }
        for &a in &worn {
            if self.inventory_mut(actor).count(a) == 0 {
                self.add_item(actor, a, 1);
            }
            if let Err(err) = self.equip_item(actor, a, true) {
                log::debug!("{actor} outfit {outfit}: {a}: {err}");
            }
        }
        self.outfits.set.insert(actor, outfit);
    }

    /// Restore the outfits a save kept, in place of this session's.
    pub(crate) fn restore_outfits(&mut self, set: HashMap<FormId, FormId>) {
        self.outfits.set = set;
    }
}
