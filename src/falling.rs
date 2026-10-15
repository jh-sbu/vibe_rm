//! Falling damage: the player coming down further than `fJumpFallHeightMin`
//! (600 units) takes `((height - min) x fJumpFallHeightMult)^fJumpFallHeightExponent`
//! (UESP's Skyrim:Damage), through the "Mod Falling Damage" perk entry point
//! (Cushioned halves it in heavy armor). Landing in deep water spares them,
//! as does being ethereal or paralyzed.
//! The NPC settings (`...NPC`: 450, 0.1, 1.65) wait for NPCs that fall.
//! `known_ambiguities/falling.md`.

use esp::LoadOrder;

use crate::ai::combat::gmst_f32;
use crate::engine::{Engine, PLAYER_REF};

/// The damage a fall of `height` units does (0 below the least that hurts).
pub fn fall_damage(lo: &LoadOrder, height: f32, npc: bool) -> f32 {
    let setting = |name: &str, player: f32, npc_default: f32| {
        if npc {
            gmst_f32(lo, &format!("{name}NPC"), npc_default)
        } else {
            gmst_f32(lo, name, player)
        }
    };
    let min = setting("fJumpFallHeightMin", 600.0, 450.0);
    let mult = setting("fJumpFallHeightMult", 0.1, 0.1);
    let exponent = setting("fJumpFallHeightExponent", 1.45, 1.65);
    if height <= min {
        return 0.0;
    }
    ((height - min) * mult).powf(exponent)
}

impl Engine {
    /// The player came down `height` units: hurt by the fall, unless into
    /// water deep enough to cover them to the middle.
    pub(crate) fn player_landed(&mut self, height: f32) {
        let damage = fall_damage(&self.lo, height, false);
        if damage <= 0.0 || self.player_dead() {
            return;
        }
        // Ethereal (UESP's Become Ethereal) and paralyzed (UESP's Damage)
        // players are spared.
        if self.is_ethereal(PLAYER_REF)
            || self.actor_value(PLAYER_REF, esp::actor_value::PARALYSIS) > 0.0
        {
            log::info!("player falls {height:.0} units unharmed");
            return;
        }
        let middle = self.player.position;
        if self.water_level(middle).is_some_and(|w| w >= middle.z) {
            log::info!("player falls {height:.0} units into water");
            return;
        }
        let damage = self.perk_entry_point(
            crate::perks::ep::MOD_FALLING_DAMAGE,
            PLAYER_REF,
            &[],
            damage,
        );
        log::info!("player falls {height:.0} units: {damage:.1} damage");
        if damage > 0.0 {
            self.damage(PLAYER_REF, damage, None, 0.0);
        }
    }
}
