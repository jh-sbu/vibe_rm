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

#[derive(Clone)]
pub(crate) struct EnchantSelection {
    pub item: Option<ListedItem>,
    pub effects: Vec<FormId>,
    pub gem: Option<(FormId, u8)>,
    pub name: Option<String>,
    pub slider: f32,
}
impl Default for EnchantSelection {
    fn default() -> Self {
        Self {
            item: None,
            effects: Vec::new(),
            gem: None,
            name: None,
            slider: 1.,
        }
    }
}

impl Engine {
    pub(crate) fn enchantable_items(&mut self) -> Vec<ListedItem> {
        self.listed_inventory(PLAYER_REF)
            .into_iter()
            .filter(|r| {
                self.enchanting_allowed(r.item)
                    && r.copy.enchantment.is_none()
                    && self.item_enchantment(r.item).is_none()
                    && self.lo.get(r.item).is_none_or(|rec| {
                        rec.tag().0 != *b"WEAP"
                            || crate::world::inventory::weapon_anim_type(&rec) != 8
                    })
            })
            .collect()
    }
    pub(crate) fn enchantment_compatible(&self, enchantment: FormId, item: FormId) -> bool {
        let Some(magic) = self
            .magic_item(enchantment)
            .filter(|m| m.spell_type == crate::magic::spell_type::ENCHANTMENT)
        else {
            return false;
        };
        let armor = self.lo.tag_of(item).map(|t| t.0) == Some(*b"ARMO");
        if !self.enchanting_allowed(item)
            || armor != (magic.cast_type == crate::magic::CONSTANT_EFFECT)
        {
            return false;
        }
        let restrictions = magic.enchantment.map_or(FormId::NULL, |e| e.restrictions);
        restrictions.is_null()
            || self
                .formlist(restrictions)
                .iter()
                .any(|&keyword| self.has_keyword(item, keyword))
    }
    pub(crate) fn known_enchantments_for(&self, item: Option<FormId>) -> Vec<FormId> {
        let mut forms: Vec<_> = self
            .knowledge
            .enchantments
            .iter()
            .copied()
            .filter(|&f| {
                self.base_enchantment(f) == Some(f)
                    && item.is_none_or(|item| self.enchantment_compatible(f, item))
            })
            .collect();
        forms.sort_by_key(|&f| (self.form_name(f), f));
        forms
    }
    pub(crate) fn enchantment_slots(&self) -> usize {
        self.perk_entry_point(
            crate::perks::ep::MOD_NUMBER_APPLIED_ENCHANTMENTS_ALLOWED,
            PLAYER_REF,
            &[],
            1.,
        )
        .round()
        .clamp(1., 2.) as usize
    }
    pub(crate) fn enchanting_souls(&mut self) -> Vec<(ListedItem, u8)> {
        let souls = self.charging_souls(PLAYER_REF);
        let items = self.listed_inventory(PLAYER_REF);
        souls
            .into_iter()
            .filter_map(|(gem, soul)| {
                items
                    .iter()
                    .find(|r| {
                        r.item == gem
                            && r.copy
                                .soul
                                .unwrap_or_else(|| self.soul_gem(gem).map_or(0, |g| g.soul))
                                == soul
                    })
                    .cloned()
                    .map(|row| (row, soul))
            })
            .collect()
    }
}

pub(crate) struct EnchantPreview {
    pub effects: Vec<crate::magic::EffectItem>,
    pub maximum: f32,
    pub capacity: f32,
    pub cost: f32,
    pub uses: u32,
    pub value: i32,
    pub weapon: bool,
}

pub(crate) fn skill_strength(skill: f32) -> f32 {
    let x = skill / 100.;
    1. + x * (x - 0.14) / 3.4
}
pub(crate) fn weapon_charge_cost(base_cost: f32, skill: f32, fraction: f32) -> f32 {
    3. * (base_cost.max(0.) * fraction.max(0.)).powf(1.1)
        * (1. - (skill.max(0.) / 200.).sqrt()).max(0.)
}

impl Engine {
    pub(crate) fn enchant_preview(&self, selection: &EnchantSelection) -> Option<EnchantPreview> {
        let item = selection.item.as_ref()?;
        let (gem, soul) = selection.gem?;
        if selection.effects.is_empty()
            || selection.effects.len() > self.enchantment_slots()
            || selection.effects.iter().any(|f| {
                !self.knowledge.enchantments.contains(f)
                    || !self.enchantment_compatible(*f, item.item)
            })
        {
            return None;
        }
        let weapon = item.info.kind == ItemKind::Weapon;
        let capacity = self.perk_entry_point(
            crate::perks::ep::MOD_SOUL_GEM_ENCHANTING,
            PLAYER_REF,
            &[Some(gem)],
            crate::recharge::soul_charge(soul),
        );
        if !capacity.is_finite() || capacity <= 0. {
            return None;
        }
        let power = skill_strength(self.actor_value(PLAYER_REF, esp::actor_value::ENCHANTING));
        let skill = self.av_base(PLAYER_REF, esp::actor_value::ENCHANTING);
        let full_cost = selection
            .effects
            .iter()
            .map(|&f| weapon_charge_cost(self.enchantment_cost(f), skill, 1.))
            .sum::<f32>();
        let eligible = if weapon && full_cost > capacity {
            (capacity / full_cost).powf(1. / 1.1)
        } else {
            1.
        };
        let fraction = if weapon {
            selection.slider.clamp(0.001, 1.) * eligible
        } else {
            capacity / crate::recharge::soul_charge(5)
        };
        let mut effects = Vec::new();
        let mut maximum = 0.;
        for &enchantment in &selection.effects {
            let source = self.magic_item(enchantment)?;
            for e in &source.effects {
                let m = self.magic_effect(e.effect)?;
                let mut effect = e.clone();
                // Custom Muffle and Waterbreathing apparel always has full
                // strength, including when the learned Muffle base is 0.5.
                if !weapon
                    && matches!(
                        m.primary,
                        Some(esp::actor_value::MOVEMENT_NOISE_MULT)
                            | Some(esp::actor_value::WATER_BREATHING)
                    )
                {
                    effect.magnitude = 1.;
                    effect.duration = 0;
                    effects.push(effect);
                    continue;
                }
                let factor = self.perk_entry_point(
                    crate::perks::ep::MOD_ENCHANTMENT_POWER,
                    PLAYER_REF,
                    &[Some(e.effect), Some(item.item)],
                    power,
                );
                let max_mag = (e.magnitude * factor * if weapon { eligible } else { fraction })
                    .floor()
                    .max(0.);
                let max_duration = if m.has(crate::magic::flags::POWER_AFFECTS_DURATION) {
                    (e.duration as f32 * factor * if weapon { eligible } else { fraction })
                        .floor()
                        .max(0.) as u32
                } else {
                    e.duration
                };
                if maximum == 0. {
                    maximum = if m.has(crate::magic::flags::POWER_AFFECTS_DURATION)
                        && m.has(crate::magic::flags::NO_MAGNITUDE)
                    {
                        max_duration as f32
                    } else {
                        max_mag
                    };
                }
                effect.magnitude = if m.has(crate::magic::flags::NO_MAGNITUDE) {
                    0.
                } else {
                    (if weapon {
                        max_mag * selection.slider.clamp(0.001, 1.)
                    } else {
                        max_mag
                    })
                    .floor()
                    .max(if max_mag > 0. { 1. } else { 0. })
                };
                effect.duration = if m.has(crate::magic::flags::NO_DURATION) {
                    0
                } else if weapon && m.has(crate::magic::flags::POWER_AFFECTS_DURATION) {
                    (max_duration as f32 * selection.slider.clamp(0.001, 1.))
                        .floor()
                        .max(1.) as u32
                } else {
                    max_duration
                };
                effects.push(effect);
            }
        }
        let cost = if weapon {
            full_cost * fraction.powf(1.1)
        } else {
            0.
        };
        let uses = if cost > 0. {
            (capacity / cost).floor().max(1.) as u32
        } else {
            u32::MAX
        };
        let value_cost = effects
            .iter()
            .map(|e| self.alchemy_effect_cost(e))
            .sum::<f32>();
        let base = self.copy_item_info(item.item, &item.copy)?.value;
        let value = base.saturating_add(if weapon {
            (crate::ai::combat::gmst_f32(&self.lo, "fEnchantmentPointsMult", 0.12) * capacity
                + crate::ai::combat::gmst_f32(&self.lo, "fEnchantmentEffectPointsMult", 8.)
                    * value_cost)
                .floor() as i32
        } else {
            value_cost.floor() as i32
        });
        Some(EnchantPreview {
            effects,
            maximum: maximum.max(1.),
            capacity,
            cost,
            uses,
            value,
            weapon,
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct EnchantRequest {
    pub station: FormId,
    pub item: FormId,
    pub owner: Option<FormId>,
    pub copy: crate::item_copies::CopyState,
    pub effects: Vec<FormId>,
    pub gem: FormId,
    pub soul: u8,
    pub name: Option<String>,
    pub slider: f32,
}

impl Engine {
    pub(crate) fn enchant_item(&mut self, request: EnchantRequest) -> Result<(), String> {
        if self.menu
            != Some(crate::items::Menu::Crafting {
                station: request.station,
                kind: 3,
            })
        {
            return Err(String::new());
        }
        let missing = || {
            self.gmst_string("sEnchantMustChooseItems")
                .unwrap_or_else(|| "Choose an item, enchantment, and soul gem.".into())
        };
        if !request.slider.is_finite()
            || request.slider <= 0.
            || request.slider > 1.
            || request
                .effects
                .iter()
                .copied()
                .collect::<std::collections::HashSet<_>>()
                .len()
                != request.effects.len()
            || request
                .name
                .as_ref()
                .is_some_and(|n| n.chars().count() > 32 || n.chars().any(char::is_control))
        {
            return Err(missing());
        }
        let Some(item) = self
            .enchantable_items()
            .into_iter()
            .find(|r| r.item == request.item && r.owner == request.owner && r.copy == request.copy)
        else {
            return Err(self
                .gmst_string("sEnchantMustChooseItems")
                .unwrap_or_default());
        };
        if !self
            .charging_souls(PLAYER_REF)
            .contains(&(request.gem, request.soul))
        {
            return Err(self
                .gmst_string("sEnchantInsufficientCharge")
                .unwrap_or_default());
        }
        let selection = EnchantSelection {
            item: Some(item),
            effects: request.effects.clone(),
            gem: Some((request.gem, request.soul)),
            name: request.name.clone(),
            slider: request.slider,
        };
        let Some(preview) = self.enchant_preview(&selection) else {
            return Err(self
                .gmst_string("sEnchantArmorIncompatible")
                .unwrap_or_default());
        };
        let mut enit = [0u8; 36];
        let effect_cost = preview
            .effects
            .iter()
            .map(|e| self.alchemy_effect_cost(e))
            .sum::<f32>()
            .floor()
            .max(0.) as u32;
        enit[..4].copy_from_slice(&effect_cost.to_le_bytes());
        enit[8..12].copy_from_slice(&if preview.weapon { 1u32 } else { 0 }.to_le_bytes());
        enit[12..16].copy_from_slice(&effect_cost.to_le_bytes());
        enit[16..20].copy_from_slice(&if preview.weapon { 1u32 } else { 0 }.to_le_bytes());
        enit[20..24].copy_from_slice(&6u32.to_le_bytes());
        enit[28..32].copy_from_slice(&request.effects.iter().min().unwrap().0.to_le_bytes());
        let mut record = esp::writer::RecordBuilder::new(b"ENCH", FormId::NULL)
            .zstring(b"FULL", "Player Enchantment")
            .field(b"ENIT", &enit);
        for e in &preview.effects {
            record = record.form(b"EFID", e.effect).field(
                b"EFIT",
                &[
                    e.magnitude.to_le_bytes(),
                    e.area.to_le_bytes(),
                    e.duration.to_le_bytes(),
                ]
                .concat(),
            );
        }
        let enchantment = self.intern_base_form(record).map_err(|e| e.to_string())?;
        let reusable = self
            .lo
            .find_editor_id("ReusableSoulGem")
            .is_some_and(|k| self.has_keyword(request.gem, k));
        let record_soul = self.soul_gem(request.gem).map_or(0, |g| g.soul);
        if !self.inventory_mut(PLAYER_REF).spend_soul(
            request.gem,
            request.soul,
            record_soul,
            reusable,
        ) {
            return Err(String::new());
        }
        let mut state = request.copy.clone();
        state.enchantment = Some(enchantment);
        state.capacity = if preview.weapon { preview.capacity } else { 0. };
        state.charge = state.capacity;
        state.use_cost = preview.weapon.then_some(preview.cost);
        state.name = request.name.or(state.name);
        let changed = self.inventory_mut(PLAYER_REF).replace_copy_state(
            request.item,
            request.owner,
            &request.copy,
            state,
        );
        debug_assert!(changed, "validated enchanting copy disappeared");
        if !reusable {
            self.inventory_event(PLAYER_REF, false, request.gem, 1, None);
        }
        self.inventory_event(PLAYER_REF, false, request.item, 1, None);
        self.inventory_event(PLAYER_REF, true, request.item, 1, None);
        self.refresh_crafting_effects();
        self.use_skill(esp::actor_value::ENCHANTING, 1.);
        self.increment_stat("Magic Items Made", 1);
        self.increment_stat("Soul Gems Used", 1);
        self.crafting.revision = self.crafting.revision.wrapping_add(1);
        Ok(())
    }
}
