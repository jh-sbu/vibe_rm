//! Learned enchantments, disenchanting and player enchantments.
use crate::engine::{Engine, PLAYER_REF};
use crate::items::ListedItem;
use crate::world::inventory::ItemKind;
use esp::FormId;

impl Engine {
    pub(crate) fn base_enchantment(&self, mut enchantment: FormId) -> Option<FormId> {
        let mut seen = std::collections::HashSet::new();
        while seen.insert(enchantment) {
            let magic = self
                .magic_item(enchantment)
                .filter(|m| m.spell_type == crate::magic::spell_type::ENCHANTMENT)?;
            let base = magic.enchantment?.base;
            if base.is_null() || base == enchantment {
                return Some(enchantment);
            }
            enchantment = base;
        }
        None
    }
    pub(crate) fn enchantment_cost(&self, enchantment: FormId) -> f32 {
        self.magic_item(enchantment).map_or(0., |m| {
            if m.flags & 1 != 0 {
                m.enchantment.map_or(0., |e| e.cost as f32)
            } else {
                m.effects.iter().map(|e| self.alchemy_effect_cost(e)).sum()
            }
        })
    }
    pub(crate) fn enchanting_allowed(&self, item: FormId) -> bool {
        matches!(self.lo.tag_of(item).map(|t|t.0),Some(t) if t==*b"WEAP" || t==*b"ARMO")
            && !self
                .lo
                .find_editor_id("MagicDisallowEnchanting")
                .is_some_and(|k| self.has_keyword(item, k))
    }
    pub(crate) fn disenchantable(&mut self) -> Vec<ListedItem> {
        self.listed_inventory(PLAYER_REF)
            .into_iter()
            .filter(|r| {
                matches!(r.info.kind, ItemKind::Armor | ItemKind::Weapon)
                    && r.copy.enchantment.is_none()
                    && self.enchanting_allowed(r.item)
            })
            .filter(|r| {
                self.item_enchantment(r.item)
                    .and_then(|e| self.base_enchantment(e))
                    .is_some_and(|e| !self.knowledge.enchantments.contains(&e))
            })
            .collect()
    }
    pub(crate) fn disenchant_item(
        &mut self,
        station: FormId,
        item: FormId,
        owner: Option<FormId>,
        key: u64,
    ) -> Result<(), String> {
        if self.menu != Some(crate::items::Menu::Crafting { station, kind: 3 }) {
            return Err(String::new());
        }
        let Some(row) = self
            .disenchantable()
            .into_iter()
            .find(|r| r.item == item && r.owner == owner && r.copy.fingerprint() == key)
        else {
            return Err(self
                .gmst_string("sEnchantmentKnown")
                .unwrap_or_else(|| "The enchantment is already known.".into()));
        };
        let enchantment = self.item_enchantment(item).unwrap();
        let base = self.base_enchantment(enchantment).unwrap();
        let cost = self.enchantment_cost(enchantment);
        let removed = self.remove_stack_with_copy(
            PLAYER_REF,
            item,
            1,
            Some(owner),
            None,
            None,
            Some(&row.copy),
        );
        if removed.parts.iter().map(|(_, n)| n).sum::<i32>() != 1 {
            return Err(String::new());
        }
        self.unequip_if_gone(PLAYER_REF, item);
        self.knowledge.enchantments.insert(base);
        let xp = crate::ai::combat::gmst_f32(&self.lo, "fDisenchantSkillUse", 0.0025) * cost;
        self.use_skill(esp::actor_value::ENCHANTING, xp);
        let message = self
            .gmst_string("sEnchantmentsLearned")
            .unwrap_or_else(|| "You learned the %s enchantment.".into())
            .replacen("%s", &self.form_name(base), 1);
        self.scripts.notify(message);
        Ok(())
    }
}

#[cfg(test)]
#[path = "enchanting_tests.rs"]
mod tests;
