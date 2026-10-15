//! Carry weight: what an actor carries weighs its items' weights (`DATA`)
//! times their counts, worn armor through the perks; the player carrying more than their `CarryWeight`
//! is over-encumbered and can only walk (UESP's Skyrim:Carry Weight), told
//! so (`sOverEncumbered`) as they become it. Worn armor slows the player:
//! speed x (1 - effect x worn weight / `fMoveWeightMax`), the effect
//! `fMoveEncumEffect` with a weapon drawn, else `fMoveEncumEffectNoWeapon`
//! (the Oblivion CS wiki's movement formula, whose settings Skyrim keeps);
//! Conditioning and Unhindered make their armor weigh nothing there too.
//! `known_ambiguities/encumbrance.md`.

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

    /// What an actor's worn armor weighs, through "Mod Armor Weight".
    pub fn worn_armor_weight(&self, actor: FormId) -> f32 {
        let Some(inv) = self.inventories.get(&actor) else {
            return 0.0;
        };
        inv.equipped()
            .iter()
            .filter(|&&f| self.lo.get(f).is_some_and(|r| r.tag().0 == *b"ARMO"))
            .filter_map(|&f| {
                let w = crate::world::inventory::item_info(&self.lo, f)?.weight;
                let point = crate::perks::ep::MOD_ARMOR_WEIGHT;
                Some(self.perk_entry_point(point, actor, &[Some(f)], w))
            })
            .sum()
    }

    /// The share of its speed an actor's worn armor leaves it.
    pub fn armor_speed_mult(&self, actor: FormId) -> f32 {
        let gmst = |n: &str, d: f32| crate::ai::combat::gmst_f32(&self.lo, n, d);
        let max = gmst("fMoveWeightMax", 115.0).max(1.0);
        let drawn = actor == PLAYER_REF && self.player_weapon_drawn();
        let effect = if drawn {
            gmst("fMoveEncumEffect", 0.3)
        } else {
            gmst("fMoveEncumEffectNoWeapon", 0.15)
        };
        armor_speed(self.worn_armor_weight(actor), max, effect)
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

/// Speed left by `worn` weight of armor: 1 - effect x worn / max, the
/// weight counted up to `max`.
pub fn armor_speed(worn: f32, max: f32, effect: f32) -> f32 {
    1.0 - effect * worn.clamp(0.0, max) / max
}

#[cfg(test)]
mod tests {
    use super::armor_speed;

    #[test]
    fn armor_slows_up_to_the_max_weight() {
        assert_eq!(armor_speed(0.0, 115.0, 0.3), 1.0);
        assert!((armor_speed(57.5, 115.0, 0.3) - 0.85).abs() < 1e-6);
        assert!((armor_speed(500.0, 115.0, 0.15) - 0.85).abs() < 1e-6);
    }
}
