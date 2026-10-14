//! Carry weight: what an actor carries weighs its items' weights (`DATA`)
//! times their counts, worn armor through the perks; the player carrying more than their `CarryWeight`
//! is over-encumbered and can only walk (UESP's Skyrim:Carry Weight), told
//! so (`sOverEncumbered`) as they become it. `known_ambiguities/encumbrance.md`.

use esp::FormId;
use esp::actor_value as av;

use crate::engine::{Engine, PLAYER_REF};
use crate::player::MoveInput;

impl Engine {
    /// The weight of everything an actor carries, its worn armor's through
    /// the "Mod Armor Weight" perk entry point (Conditioning and Unhindered
    /// make worn heavy / light armor weigh nothing).
    pub fn carried_weight(&self, actor: FormId) -> f32 {
        let Some(inv) = self.inventories.get(&actor) else {
            return 0.0;
        };
        let weight = |f: FormId| crate::world::inventory::item_info(&self.lo, f).map(|i| i.weight);
        let all: f32 = inv
            .items
            .iter()
            .filter_map(|&(f, n)| weight(f).map(|w| w * n as f32))
            .sum();
        let worn: f32 = inv
            .equipped()
            .iter()
            .filter(|&&f| self.lo.get(f).is_some_and(|r| r.tag().0 == *b"ARMO"))
            .filter_map(|&f| {
                let w = weight(f)?;
                let point = crate::perks::ep::MOD_ARMOR_WEIGHT;
                Some(self.perk_entry_point(point, actor, &[Some(f)], w) - w)
            })
            .sum();
        all + worn
    }

    /// Whether the player carries more than they can (and still run).
    pub fn player_overencumbered(&self) -> bool {
        self.carried_weight(PLAYER_REF) > self.actor_value(PLAYER_REF, av::CARRY_WEIGHT)
    }

    /// Over-encumbered, the player walks: running and sprinting are dropped
    /// from their input. Becoming so, they're told.
    pub(crate) fn encumber_player(&mut self, input: &mut MoveInput) {
        let over = self.player_overencumbered();
        if over && !self.player.overencumbered {
            let msg = self
                .gmst_string("sOverEncumbered")
                .unwrap_or_else(|| "You are carrying too much to be able to run.".into());
            self.scripts.notify(msg);
        }
        self.player.overencumbered = over;
        if over {
            input.run = false;
            input.sprint = false;
        }
    }
}
