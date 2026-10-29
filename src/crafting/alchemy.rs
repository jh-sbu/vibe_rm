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

pub(crate) struct PotionResult {
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
        (!effects.is_empty()).then_some(PotionResult { effects, poison })
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
