//! Ingredient queries and created potions.
use crate::engine::{Engine, PLAYER_REF};
use crate::items::ListedItem;
use crate::world::inventory::ItemKind;

impl Engine {
    pub(crate) fn alchemy_ingredients(&mut self) -> Vec<ListedItem> {
        self.listed_inventory(PLAYER_REF)
            .into_iter()
            .filter(|r| r.info.kind == ItemKind::Ingredient)
            .collect()
    }
}

use crate::magic::{EffectItem, flags};
use esp::FormId;

#[derive(Clone)]
pub(crate) struct PotionResult {
    pub name: String,
    pub value: i32,
    pub effects: Vec<EffectItem>,
    pub poison: bool,
}

/// Each distinct ingredient contributes at most once to an effect. Retain all
/// fields from its highest-priority source, rather than combining magnitude
/// from one ingredient with duration from another.
fn shared_effects(
    ingredients: &[Vec<EffectItem>],
    priority: impl Fn(&EffectItem) -> f32,
) -> Vec<EffectItem> {
    let mut shared = std::collections::BTreeMap::<FormId, (usize, EffectItem, f32)>::new();
    for effects in ingredients {
        let mut seen = std::collections::HashSet::new();
        for effect in effects {
            if !seen.insert(effect.effect) {
                continue;
            }
            let cost = priority(effect);
            let entry = shared
                .entry(effect.effect)
                .or_insert_with(|| (0, effect.clone(), cost));
            entry.0 += 1;
            if cost > entry.2 {
                entry.1 = effect.clone();
                entry.2 = cost;
            }
        }
    }
    shared
        .into_values()
        .filter(|(n, _, _)| *n >= 2)
        .map(|(_, effect, _)| effect)
        .collect()
}

impl Engine {
    pub(crate) fn alchemy_effect_cost(&self, effect: &EffectItem) -> f32 {
        let base = self
            .lo
            .get(effect.effect)
            .and_then(|r| {
                r.get(b"DATA")
                    .and_then(|d| d.get(4..8))
                    .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
            })
            .unwrap_or(0.);
        let duration = if effect.duration == 0 {
            1.
        } else {
            effect.duration as f32 / 10.
        };
        base * (effect.magnitude.max(1.) * duration).powf(1.1)
    }
    pub(crate) fn unscaled_potion(&self, ingredients: &[FormId]) -> Option<PotionResult> {
        if !(2..=3).contains(&ingredients.len())
            || ingredients
                .iter()
                .copied()
                .collect::<std::collections::HashSet<_>>()
                .len()
                != ingredients.len()
        {
            return None;
        }
        let source = ingredients
            .iter()
            .map(|&f| {
                self.magic_item(f)
                    .filter(|m| m.spell_type == crate::magic::spell_type::INGREDIENT)
                    .map(|m| m.effects.clone())
            })
            .collect::<Option<Vec<_>>>()?;
        let mut effects = shared_effects(&source, |e| self.alchemy_effect_cost(e));
        let dominant = effects.iter().max_by(|a, b| {
            self.alchemy_effect_cost(a)
                .total_cmp(&self.alchemy_effect_cost(b))
        })?;
        let poison = self
            .magic_effect(dominant.effect)
            .is_some_and(|m| m.has(flags::HOSTILE));
        if self.perk_flag(
            crate::perks::ep::PURIFY_ALCHEMY_INGREDIENTS,
            PLAYER_REF,
            &[],
        ) {
            effects.retain(|e| {
                self.magic_effect(e.effect)
                    .is_some_and(|m| (m.has(flags::HOSTILE) || m.has(flags::DETRIMENTAL)) == poison)
            });
        }
        (!effects.is_empty()).then_some(PotionResult {
            effects,
            poison,
            name: String::new(),
            value: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn effect(id: u32, magnitude: f32) -> EffectItem {
        EffectItem {
            effect: FormId(id),
            magnitude,
            area: 0,
            duration: 10,
            conditions: Vec::new(),
        }
    }
    #[test]
    fn crafting_shared_effects_require_two_distinct_ingredients() {
        let a = vec![effect(1, 1.), effect(2, 2.), effect(1, 3.)];
        let b = vec![effect(2, 4.), effect(3, 3.)];
        let c = vec![effect(1, 5.), effect(3, 6.)];
        let two = shared_effects(&[a.clone(), b.clone()], |e| e.magnitude);
        assert_eq!(two.len(), 1);
        assert_eq!(two[0].effect, FormId(2));
        assert_eq!(two[0].magnitude, 4.);
        let three = shared_effects(&[a, b, c], |e| e.magnitude);
        assert_eq!(
            three.iter().map(|e| e.effect).collect::<Vec<_>>(),
            vec![FormId(1), FormId(2), FormId(3)]
        );
        assert_eq!(three[0].magnitude, 5.);
        assert_eq!(three[2].magnitude, 6.);
        assert!(shared_effects(&[vec![effect(1, 1.), effect(1, 1.)]], |e| e.magnitude).is_empty());
    }
}

#[derive(Clone, Copy)]
pub(crate) struct AlchemyQuery {
    pub effect: FormId,
    pub poison: bool,
}

fn ingredient_power(skill: f32, initial: f32, skill_factor: f32) -> f32 {
    initial * (1. + (skill_factor - 1.) * skill / 100.)
}

impl Engine {
    pub(crate) fn potion_result(&self, ingredients: &[FormId]) -> Option<PotionResult> {
        let mut result = self.unscaled_potion(ingredients)?;
        let initial = crate::ai::combat::gmst_f32(&self.lo, "fAlchemyIngredientInitMult", 4.);
        let skill_factor = crate::ai::combat::gmst_f32(&self.lo, "fAlchemySkillFactor", 1.5);
        let power = ingredient_power(
            self.av_base(PLAYER_REF, esp::actor_value::ALCHEMY),
            initial,
            skill_factor,
        );
        for effect in &mut result.effects {
            let flags = self.magic_effect(effect.effect)?.flags;
            let previous = self.crafting.alchemy_query.replace(Some(AlchemyQuery {
                effect: effect.effect,
                poison: result.poison,
            }));
            let factor = self.perk_entry_point(
                crate::perks::ep::MOD_ALCHEMY_EFFECTIVENESS,
                PLAYER_REF,
                &[],
                power,
            );
            self.crafting.alchemy_query.set(previous);
            let factor = if factor < 0. || !factor.is_finite() {
                1.
            } else {
                factor
            };
            effect.magnitude = if flags & crate::magic::flags::NO_MAGNITUDE != 0 {
                0.
            } else {
                (effect.magnitude
                    * if flags & crate::magic::flags::POWER_AFFECTS_MAGNITUDE != 0 {
                        factor
                    } else {
                        1.
                    })
                .round()
            };
            effect.duration = if flags & crate::magic::flags::NO_DURATION != 0 {
                0
            } else {
                (effect.duration as f32
                    * if flags & crate::magic::flags::POWER_AFFECTS_DURATION != 0 {
                        factor
                    } else {
                        1.
                    })
                .round()
                .max(0.) as u32
            };
        }
        let dominant = result.effects.iter().max_by(|a, b| {
            self.alchemy_effect_cost(a)
                .total_cmp(&self.alchemy_effect_cost(b))
        })?;
        result.poison = self
            .magic_effect(dominant.effect)
            .is_some_and(|m| m.has(flags::HOSTILE));
        let prefix = self
            .gmst_string(if result.poison {
                "sCreatedPoisonNamePrefix"
            } else {
                "sCreatedPotionNamePrefix"
            })
            .unwrap_or_else(|| {
                if result.poison {
                    "Poison of ".into()
                } else {
                    "Potion of ".into()
                }
            });
        result.name = format!("{prefix}{}", self.form_name(dominant.effect));
        result.value = result
            .effects
            .iter()
            .map(|e| self.alchemy_effect_cost(e))
            .sum::<f32>()
            .floor()
            .max(0.) as i32;
        Some(result)
    }
}

#[cfg(test)]
#[path = "alchemy_tests.rs"]
mod integration_tests;

impl Engine {
    fn potion_record(&self, result: &PotionResult) -> esp::writer::RecordBuilder {
        let mut record = esp::writer::RecordBuilder::new(b"ALCH", FormId::NULL)
            .windows1252_zstring(b"FULL", &result.name)
            .f32(b"DATA", 0.5);
        let mut enit = [0u8; 20];
        enit[..4].copy_from_slice(&(result.value.max(0) as u32).to_le_bytes());
        enit[4..8].copy_from_slice(&(1u32 | if result.poison { 0x20000 } else { 0 }).to_le_bytes());
        if let Some(template) = self
            .lo
            .find_editor_id(if result.poison {
                "DefaultPoison"
            } else {
                "DefaultPotion"
            })
            .and_then(|f| self.lo.get(f))
        {
            for tag in [b"MODL", b"OBND"] {
                if let Some(bytes) = template.get(tag) {
                    record = record.field(tag, bytes);
                }
            }
            for tag in [b"YNAM", b"ZNAM"] {
                if let Some(bytes) = template.get(tag).and_then(|d| d.get(..4)) {
                    record = record.form(
                        tag,
                        template.fid(FormId(u32::from_le_bytes(bytes.try_into().unwrap()))),
                    );
                }
            }
            if let Some(bytes) = template.get(b"KWDA") {
                let keywords: Vec<_> = bytes
                    .chunks_exact(4)
                    .flat_map(|b| {
                        template
                            .fid(FormId(u32::from_le_bytes(b.try_into().unwrap())))
                            .0
                            .to_le_bytes()
                    })
                    .collect();
                record = record
                    .u32(b"KSIZ", (keywords.len() / 4) as u32)
                    .field(b"KWDA", &keywords);
            }
            if let Some(sound) = template.get(b"ENIT").and_then(|d| d.get(16..20)) {
                enit[16..20].copy_from_slice(
                    &template
                        .fid(FormId(u32::from_le_bytes(sound.try_into().unwrap())))
                        .0
                        .to_le_bytes(),
                );
            }
        }
        record = record.field(b"ENIT", &enit);
        for effect in &result.effects {
            record = record.form(b"EFID", effect.effect).field(
                b"EFIT",
                &[
                    effect.magnitude.to_le_bytes(),
                    effect.area.to_le_bytes(),
                    effect.duration.to_le_bytes(),
                ]
                .concat(),
            );
        }
        record
    }

    pub(crate) fn mix_ingredients(
        &mut self,
        station: FormId,
        ingredients: &[FormId],
    ) -> Result<Option<FormId>, String> {
        if self.menu != Some(crate::items::Menu::Crafting { station, kind: 5 })
            || !(2..=3).contains(&ingredients.len())
            || ingredients
                .iter()
                .copied()
                .collect::<std::collections::HashSet<_>>()
                .len()
                != ingredients.len()
            || ingredients.iter().any(|&f| {
                self.lo.tag_of(f).map(|t| t.0) != Some(*b"INGR")
                    || self
                        .inventories
                        .get(&PLAYER_REF)
                        .is_none_or(|i| i.count(f) <= 0)
            })
        {
            return Err(String::new());
        }
        let result = self.potion_result(ingredients);
        let potion = match &result {
            Some(result) => {
                let id = self
                    .intern_base_form(self.potion_record(result))
                    .map_err(|e| e.to_string())?;
                if self.inventory_mut(PLAYER_REF).count(id) == i32::MAX {
                    return Err(String::new());
                }
                Some(id)
            }
            None => None,
        };
        for &ingredient in ingredients {
            self.remove_stack(PLAYER_REF, ingredient, 1, None, None, None);
        }
        if let (Some(result), Some(potion)) = (result, potion) {
            self.inventory_mut(PLAYER_REF).add(potion, 1);
            self.inventory_event(PLAYER_REF, true, potion, 1, None);
            for &ingredient in ingredients {
                if let Some(item) = self.magic_item(ingredient) {
                    for (i, effect) in item.effects.iter().enumerate() {
                        if result.effects.iter().any(|e| e.effect == effect.effect) {
                            self.learn_effect(ingredient, i as i32);
                        }
                    }
                }
            }
            self.use_skill(esp::actor_value::ALCHEMY, result.value.max(0) as f32);
            self.increment_stat(
                if result.poison {
                    "Poisons Mixed"
                } else {
                    "Potions Mixed"
                },
                1,
            );
            Ok(Some(potion))
        } else {
            for (i, &a) in ingredients.iter().enumerate() {
                for &b in &ingredients[i + 1..] {
                    self.knowledge.failed_alchemy.insert((a.min(b), a.max(b)));
                }
            }
            self.use_skill(esp::actor_value::ALCHEMY, 1.);
            self.scripts.notify(
                self.gmst_string("sPotionCreationFailed")
                    .unwrap_or_else(|| "Potion creation failed".into()),
            );
            Ok(None)
        }
    }
}
