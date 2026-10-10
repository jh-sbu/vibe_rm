//! Pickpocketing: a sneaking player activating someone looks through what
//! they carry and tries to take it, item by item, with a chance from the
//! game settings (UESP *Skyrim:Pickpocket*). Caught, it is a crime the
//! victim knows of, and the victim won't let them try again.

use esp::FormId;

use crate::crime::CrimeType;
use crate::engine::{Engine, PLAYER_REF};

/// Taken off the chance when the victim detects the player (UESP: "Detected
/// = 25"; no game setting found).
const DETECTED_PENALTY: f32 = 25.0;

impl Engine {
    /// Whether the player may pick `r`'s pocket: sneaking, and `r` a living
    /// actor not fighting them who hasn't caught them at it.
    pub fn can_pickpocket(&self, r: FormId) -> bool {
        self.player.sneaking
            && !self.is_dead(r)
            && !self.crime.pickpocket_caught.contains(&r)
            && self
                .actor_ref(r)
                .is_some_and(|a| a.combat.as_ref().is_none_or(|c| c.target != PLAYER_REF))
    }

    /// The chance (percent) of taking `count` of `item` from `victim`:
    /// `fPickPocketActorSkillBase + fPickPocketActorSkillMult x the player's
    /// Pickpocket + fPickPocketTargetSkillMult x (fPickPocketTargetSkillBase +
    /// the victim's) + fPickPocketWeightMult x weight + fPickPocketAmountMult x
    /// gold`, less 25 when the victim detects the player, between
    /// `fPickPocketMinChance` and `fPickPocketMaxChance`. With Skyrim.esm's
    /// settings that is UESP's `15 + skill - target skill / 4 - 4 x weight -
    /// gold / 10`.
    pub fn pickpocket_chance(&self, victim: FormId, item: FormId, count: i32) -> f32 {
        let g = |name: &str, default: f32| crate::ai::combat::gmst_f32(&self.lo, name, default);
        let info = crate::world::inventory::item_info(&self.lo, item);
        let (weight, gold) = if item == crate::crime::GOLD {
            (0.0, count as f32)
        } else {
            (info.map_or(0.0, |i| i.weight) * count as f32, 0.0)
        };
        let mut chance = g("fPickPocketActorSkillBase", 20.0)
            + g("fPickPocketActorSkillMult", 1.0)
                * self.actor_value(PLAYER_REF, esp::actor_value::PICKPOCKET)
            + g("fPickPocketTargetSkillMult", -0.25)
                * (g("fPickPocketTargetSkillBase", 20.0)
                    + self.actor_value(victim, esp::actor_value::PICKPOCKET))
            + g("fPickPocketWeightMult", -4.0) * weight
            + g("fPickPocketAmountMult", -0.1) * gold;
        if self.detects(victim, PLAYER_REF) {
            chance -= DETECTED_PENALTY;
        }
        // Pickpocket perks add to it (Light Fingers, Night Thief, Cutpurse; UESP)
        // or set it (Keymaster), before the limits.
        let chance = self.perk_entry_point(
            crate::perks::ep::MOD_PICKPOCKET_CHANCE,
            PLAYER_REF,
            &[Some(victim), Some(item)],
            chance,
        );
        chance.clamp(
            g("fPickPocketMinChance", 0.0),
            g("fPickPocketMaxChance", 90.0),
        )
    }

    /// Try to take `count` of `item` (of the stack belonging to `stack`'s owner,
    /// if given) from `victim`. Taken, it is the victim's, stolen. Caught,
    /// the menu closes and the pickpocketing is reported, the victim knowing
    /// of it (`sPickpocketFail`), and the victim says so (`PICN`). True if
    /// taken.
    pub fn try_pickpocket(
        &mut self,
        victim: FormId,
        item: FormId,
        count: i32,
        stack: Option<Option<FormId>>,
    ) -> bool {
        let chance = self.pickpocket_chance(victim, item, count);
        let roll = (self.rand() % 100) as f32;
        log::info!(
            "pickpocketing {count} x {item} from {victim}: chance {chance:.0}, rolled {roll:.0}"
        );
        if roll < chance {
            let owner = self.base_of(victim);
            let moved: i32 = self
                .remove_stack(victim, item, count, stack, Some(PLAYER_REF), owner)
                .iter()
                .map(|(_, n)| n)
                .sum();
            if moved > 0 {
                self.send_player_add_item_as(item, Some(victim), victim, 3);
                let info = crate::world::inventory::item_info(&self.lo, item);
                // Pickpocket trains by the value taken.
                let value = if item == crate::crime::GOLD {
                    1.0
                } else {
                    info.as_ref().map_or(0.0, |i| i.value as f32)
                };
                self.use_skill(esp::actor_value::PICKPOCKET, value * moved as f32);
                let name = info.map(|i| i.name).unwrap_or_default();
                self.scripts.notify(if moved > 1 {
                    format!("{name} ({moved}) added")
                } else {
                    format!("{name} added")
                });
            }
            return moved > 0;
        }
        self.menu = None;
        self.crime.pickpocket_caught.insert(victim);
        let msg = self
            .gmst_string("sPickpocketFail")
            .unwrap_or_else(|| "You've been caught pickpocketing.".into());
        self.scripts.notify(msg);
        self.commit_crime(CrimeType::Pickpocket, Some(victim), None, 0);
        self.crime.reacting_victim = Some(victim);
        self.bark(victim, b"PICN");
        self.crime.reacting_victim = None;
        false
    }

    /// The player activating someone while sneaking: their pockets, unless
    /// they caught the player before (`sNoPickPocketAgain`).
    pub(crate) fn start_pickpocket(&mut self, victim: FormId, name: &str) -> bool {
        if !self.player.sneaking || self.is_dead(victim) {
            return false;
        }
        if self.crime.pickpocket_caught.contains(&victim) {
            let msg = self
                .gmst_string("sNoPickPocketAgain")
                .unwrap_or_else(|| " has already caught you.".into());
            self.scripts.notify(format!("{name}{msg}"));
            return true;
        }
        if !self.can_pickpocket(victim) {
            return false;
        }
        self.menu = Some(crate::items::Menu::Pickpocket(victim));
        true
    }
}
