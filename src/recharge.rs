//! Recharging enchanted weapons (and staffs) with soul gems: a soul gives
//! its size's charge (petty 250, lesser 500, common 1000, greater 2000,
//! grand and black 3000), up to the weapon's most (`EAMT`), the rest lost.
//! The gem is used up, except reusable ones (the `ReusableSoulGem` keyword:
//! Azura's Star, the Black Star), which are left empty. The player gains
//! Enchanting by the soul's size (its charge times
//! `fSkillUsageRechargeMult`).
//!
//! Sources: UESP's Skyrim:Soul Gems (the sizes' charges, gems consumed on
//! recharging, Azura's Star and the Black Star not), Skyrim:Leveling (0.05
//! .. 0.6 base XP, petty to grand), CommonLibSSE's
//! `kKeywordReusableSoulGem`. Choices: `known_ambiguities/staffs.md`.

use esp::FormId;
use esp::actor_value as av;

use crate::engine::{Engine, PLAYER_REF};

/// The charge a soul gives (1 petty .. 5 grand).
pub(crate) fn soul_charge(soul: u8) -> f32 {
    match soul {
        1 => 250.0,
        2 => 500.0,
        3 => 1000.0,
        4 => 2000.0,
        5 => 3000.0,
        _ => 0.0,
    }
}

/// What a soul of `soul` adds to a weapon of `left` charge out of `full`.
fn charge_added(left: f32, full: f32, soul: u8) -> f32 {
    soul_charge(soul).min((full - left).max(0.0))
}

impl Engine {
    fn recharge_amount(&self, actor: FormId, left: f32, full: f32, soul: u8) -> f32 {
        self.perk_entry_point(
            crate::perks::ep::MOD_SOUL_GEM_RECHARGE,
            actor,
            &[None],
            soul_charge(soul),
        )
        .max(0.)
        .min((full - left).max(0.))
    }
    /// The souls `actor` carries to charge with: (gem, soul), one for each
    /// kind, smallest soul first.
    pub(crate) fn charging_souls(&self, actor: FormId) -> Vec<(FormId, u8)> {
        let Some(inv) = self.inventories.get(&actor) else {
            return Vec::new();
        };
        let mut out: Vec<(FormId, u8)> = Vec::new();
        for &(form, n) in &inv.items {
            let Some(gem) = self.soul_gem(form).filter(|_| n > 0) else {
                continue;
            };
            if gem.soul > 0 && inv.without_soul(form) > 0 {
                out.push((form, gem.soul));
            }
            for &(_, soul, _) in inv.souls.iter().filter(|s| s.0 == form && s.2 > 0) {
                if !out.contains(&(form, soul)) {
                    out.push((form, soul));
                }
            }
            for copy in inv.copies.iter().filter(|c| c.item == form && c.count > 0) {
                if let Some(soul) = copy.state.soul.filter(|s| *s > 0) {
                    if !out.contains(&(form, soul)) {
                        out.push((form, soul));
                    }
                }
            }
        }
        out.retain(|&(_, s)| soul_charge(s) > 0.0);
        out.sort_by_key(|&(f, s)| (s, f.0));
        out
    }

    /// One of `actor`'s weapons' charge as a percent of its most, if it
    /// holds one.
    pub(crate) fn charge_percent(&self, actor: FormId, weapon: FormId) -> Option<f32> {
        let full = self.weapon_capacity_for(actor, weapon);
        self.item_enchantment_for(actor, weapon)?;
        (full > 0.0).then(|| self.weapon_charge(actor, weapon) / full * 100.0)
    }

    /// What charging `weapon` with a soul of `soul` would add, as a percent
    pub(crate) fn charge_added_percent_for_copy(
        &self,
        actor: FormId,
        weapon: FormId,
        copy: &crate::item_copies::CopyState,
        soul: u8,
    ) -> f32 {
        let full = if copy.capacity > 0. {
            copy.capacity
        } else {
            self.weapon_capacity(weapon)
        };
        let left = if copy.capacity > 0. {
            copy.charge
        } else {
            full
        };
        if full <= 0. {
            return 0.;
        }
        self.recharge_amount(actor, left, full, soul) / full * 100.
    }

    /// What charging `weapon` with a soul of `soul` would add, as a percent
    /// of its most.
    pub(crate) fn charge_added_percent(&self, actor: FormId, weapon: FormId, soul: u8) -> f32 {
        let full = self.weapon_capacity_for(actor, weapon);
        if full <= 0.0 {
            return 0.0;
        }
        self.recharge_amount(actor, self.weapon_charge(actor, weapon), full, soul) / full * 100.0
    }

    /// `actor` charges one of its `weapon`s with a `soul` from one of its
    /// `gem`s. Whether it did (a weapon not full, and that soul there).
    pub(crate) fn recharge(
        &mut self,
        actor: FormId,
        weapon: FormId,
        gem: FormId,
        soul: u8,
    ) -> bool {
        self.recharge_copy(actor, weapon, gem, soul, None)
    }

    pub(crate) fn recharge_copy(
        &mut self,
        actor: FormId,
        weapon: FormId,
        gem: FormId,
        soul: u8,
        copy: Option<(Option<FormId>, &crate::item_copies::CopyState)>,
    ) -> bool {
        let full = copy.map_or_else(
            || self.weapon_capacity_for(actor, weapon),
            |(_, c)| {
                if c.capacity > 0. {
                    c.capacity
                } else {
                    self.weapon_capacity(weapon)
                }
            },
        );
        let left = copy.map_or_else(
            || self.weapon_charge(actor, weapon),
            |(_, c)| if c.capacity > 0. { c.charge } else { full },
        );
        let enchantment = copy
            .and_then(|(_, c)| c.enchantment)
            .or_else(|| self.item_enchantment_for(actor, weapon));
        if full <= 0.0 || left >= full || enchantment.is_none() {
            return false;
        }
        if let Some((owner, state)) = copy {
            let inv = self.inventory_mut(actor);
            if !inv
                .copies
                .iter()
                .any(|c| c.item == weapon && c.owner == owner && c.state == *state && c.count > 0)
            {
                return false;
            }
        }
        let Some(record) = self.soul_gem(gem).map(|g| g.soul) else {
            return false;
        };
        let keep = self
            .lo
            .find_editor_id("ReusableSoulGem")
            .is_some_and(|k| self.has_keyword(gem, k));
        if !self
            .inventory_mut(actor)
            .spend_soul(gem, soul, record, keep)
        {
            return false;
        }
        let now = left + self.recharge_amount(actor, left, full, soul);
        if let Some(inv) = self.inventories.get_mut(&actor) {
            if let Some((owner, state)) = copy {
                let mut new = state.clone();
                new.capacity = full;
                new.charge = now;
                inv.replace_copy_state(weapon, owner, state, new);
            } else {
                inv.set_charge(weapon, now, full);
            }
        }
        log::info!(
            "{actor} charges {} with a soul of {soul} from {}: {left:.0} -> {now:.0} / {full:.0}",
            self.form_name(weapon),
            self.form_name(gem)
        );
        if actor == PLAYER_REF {
            let mult = crate::ai::combat::gmst_f32(&self.lo, "fSkillUsageRechargeMult", 0.0002);
            self.use_skill(av::ENCHANTING, soul_charge(soul) * mult);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_soul_fills_up_to_the_most_and_the_rest_is_lost() {
        assert_eq!(charge_added(0.0, 1000.0, 1), 250.0);
        assert_eq!(charge_added(900.0, 1000.0, 5), 100.0);
        assert_eq!(charge_added(1000.0, 1000.0, 3), 0.0);
        // Grand souls' base XP is UESP's 0.6 at the game's multiplier.
        assert!((soul_charge(5) * 0.0002 - 0.6).abs() < 1e-6);
        assert!((soul_charge(1) * 0.0002 - 0.05).abs() < 1e-6);
    }
}
