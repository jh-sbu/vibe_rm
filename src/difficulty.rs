//! UESP Damage table, with data GMSTs overriding documented fallback values.
use crate::{
    ai::combat::gmst_f32,
    engine::{Engine, PLAYER_REF},
};
use esp::{FormId, LoadOrder};
pub const NPC: [f32; 6] = [2., 1.5, 1., 0.75, 0.5, 0.25];
pub const PLAYER: [f32; 6] = [0.5, 0.75, 1., 1.5, 2., 3.];
const SUFFIX: [&str; 6] = ["VE", "E", "N", "H", "VH", "L"];
pub fn multiplier(lo: &LoadOrder, level: usize, player: bool, fall: bool) -> f32 {
    let level = level.min(5);
    let fallback = if player { PLAYER[level] } else { NPC[level] };
    let name = format!(
        "fDiffMultHP{}PC{}",
        if player { "To" } else { "By" },
        SUFFIX[level]
    );
    let value = gmst_f32(lo, &name, fallback);
    let value = if value.is_finite() && value >= 0. {
        value
    } else {
        fallback
    };
    if fall { value.min(1.) } else { value }
}
impl Engine {
    /// Target-based even for NPC → NPC damage; this attacker policy remains
    /// provisional. See known_ambiguities/system-page.md and the SE observations.
    pub fn difficulty_multiplier(&self, target: FormId, fall: bool) -> f32 {
        multiplier(
            &self.lo,
            self.settings.get("iDifficulty") as usize,
            target == PLAYER_REF,
            fall,
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn documented_damage_and_fall_tables() {
        let lo = LoadOrder::load(".", &[]).unwrap();
        for i in 0..6 {
            assert_eq!(multiplier(&lo, i, true, false), PLAYER[i]);
            assert_eq!(multiplier(&lo, i, false, false), NPC[i]);
        }
        assert_eq!(multiplier(&lo, 0, true, true), 0.5);
        assert_eq!(multiplier(&lo, 5, true, true), 1.);
    }
}
