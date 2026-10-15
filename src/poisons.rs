//! Poisons on weapons. Used from the inventory, a poison goes on the weapon
//! held (the right hand's, else the left's) once the player says yes to
//! `sPoisonConfirmMessage` ("Do you want to poison the <weapon>?"; for a bow
//! `sPoisonBowConfirmMessage`, "...your next shot with the <poison>?"). No
//! weapon (`sPoisonNoWeaponMessage`), a staff (`sPoisonUnableToPoison`) or a
//! weapon already poisoned (`sPoisonAlreadyPoisonedMessage`) refuse it. The
//! weapon keeps it for one dose, or as many as the wielder's Mod Poison Dose
//! Count perks make it (Concentrated Poison: 2): each blow it lands uses one
//! and the poison's effects land on the one struck; a bow's shot takes a dose
//! with it as it is loosed (the game's `ArrowProjectile::poison`), hit or
//! miss. Using one counts towards Poisons Used. The poisoned weapon is one of
//! those carried (`Inventory::poisoned`). Open questions:
//! `known_ambiguities/poisons.md`.

use esp::FormId;

use crate::engine::{Engine, PLAYER_REF};
use crate::messages::BoxAction;
use crate::perks::ep;
use crate::world::inventory::{HandType, LEFT, RIGHT};

impl Engine {
    /// The weapon a poison would go on: the right hand's, else the left's.
    fn weapon_to_poison(&self, actor: FormId) -> Option<(FormId, HandType)> {
        let inv = self.inventories.get(&actor)?;
        [RIGHT, LEFT].into_iter().find_map(|h| {
            let w = inv.weapon_in(&self.lo, h)?;
            Some((w, inv.hand(&self.lo, h == LEFT)))
        })
    }

    /// The player uses a poison they carry: asked to put it on their weapon,
    /// or told why it can't go on.
    pub fn offer_poison(&mut self, poison: FormId) -> Result<(), String> {
        let refuse = |e: &Engine, name: &str, english: &str| {
            Err(e.gmst_string(name).unwrap_or_else(|| english.into()))
        };
        let Some((weapon, hand)) = self.weapon_to_poison(PLAYER_REF) else {
            return refuse(
                self,
                "sPoisonNoWeaponMessage",
                "You must first equip a weapon to poison it.",
            );
        };
        if hand == HandType::Staff {
            return refuse(
                self,
                "sPoisonUnableToPoison",
                "Current weapon cannot be poisoned.",
            );
        }
        if self
            .inventories
            .get(&PLAYER_REF)
            .is_some_and(|i| i.poison_on(weapon).is_some())
        {
            return refuse(
                self,
                "sPoisonAlreadyPoisonedMessage",
                "The current weapon is already poisoned.",
            );
        }
        let name = |f: FormId| {
            crate::world::inventory::item_info(&self.lo, f)
                .map(|i| i.name)
                .unwrap_or_default()
        };
        let text = if matches!(hand, HandType::Bow | HandType::Crossbow) {
            format!(
                "{}{}?",
                self.gmst_string("sPoisonBowConfirmMessage")
                    .unwrap_or_else(|| "Do you want to poison your next shot with the ".into()),
                name(poison)
            )
        } else {
            format!(
                "{}{}?",
                self.gmst_string("sPoisonConfirmMessage")
                    .unwrap_or_else(|| "Do you want to poison the ".into()),
                name(weapon)
            )
        };
        let yes = self.gmst_string("sYes").unwrap_or_else(|| "Yes".into());
        let no = self.gmst_string("sNo").unwrap_or_else(|| "No".into());
        self.ask(text, vec![yes, no], BoxAction::Poison { weapon, poison });
        Ok(())
    }

    /// `actor` puts a dose of `poison` from their inventory on the carried
    /// `weapon`.
    pub fn apply_poison(&mut self, actor: FormId, weapon: FormId, poison: FormId) {
        if self.item_count(actor, poison) <= 0 || self.item_count(actor, weapon) <= 0 {
            return;
        }
        self.remove_item(actor, poison, 1, None);
        let doses = self
            .perk_entry_point(
                ep::MOD_POISON_DOSE_COUNT,
                actor,
                &[Some(weapon), Some(poison)],
                1.0,
            )
            .round()
            .max(1.0) as i32;
        log::info!("{actor} poisons {weapon} with {poison} ({doses} doses)");
        if let Some(inv) = self.inventories.get_mut(&actor) {
            inv.poison(weapon, poison, doses);
        }
        if actor == PLAYER_REF {
            self.increment_stat("Poisons Used", 1);
        }
    }

    /// A dose of the poison on the `weapon` `actor` strikes or shoots with,
    /// used up: the poison, if any.
    pub(crate) fn take_poison_dose(&mut self, actor: FormId, weapon: FormId) -> Option<FormId> {
        let p = self.inventories.get_mut(&actor)?.take_poison_dose(weapon)?;
        log::info!("{actor}'s {weapon} gives up a dose of {p}");
        Some(p)
    }
}
