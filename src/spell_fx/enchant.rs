//! Drawn weapon enchantment art and shaders (armour uses active hit effects).
use super::{
    ArtKey, records,
    shaders::{ShaderEffect, WeaponTarget},
};
use crate::engine::{Engine, PLAYER_REF};
use esp::FormId;
use std::collections::HashSet;

impl Engine {
    pub(super) fn update_enchant_fx(&mut self) -> HashSet<(FormId, ArtKey)> {
        let mut requests = Vec::new();
        for (&actor, inventory) in &self.inventories {
            let drawn = if actor == PLAYER_REF {
                self.player_weapon_drawn()
            } else {
                self.actor_ref(actor).is_some_and(|a| a.drawn)
            };
            if !drawn || self.is_disabled(actor) {
                continue;
            }
            for hand in [crate::spells::LEFT, crate::spells::RIGHT] {
                let Some(weapon) = inventory
                    .held(hand)
                    .filter(|&w| self.lo.tag_of(w).is_some_and(|t| t.0 == *b"WEAP"))
                else {
                    continue;
                };
                let Some(enchantment) = self
                    .item_enchantment(weapon)
                    .and_then(|e| self.magic_item(e))
                else {
                    continue;
                };
                for item in &enchantment.effects {
                    if let Some(effect) = self.magic_effect(item.effect) {
                        requests.push((
                            actor,
                            hand,
                            weapon,
                            effect.enchant_art,
                            effect.enchant_shader,
                        ));
                    }
                }
            }
        }
        let mut arts = HashSet::new();
        let mut shaders = HashSet::new();
        for (actor, hand, weapon, art, shader) in requests {
            if !art.is_null() {
                let key = (actor, ArtKey::Enchant(hand, weapon, art));
                arts.insert(key);
                if !self.spell_fx.art.contains_key(&key) {
                    if let Some(mut effect) = self.load_art(
                        art,
                        if hand == crate::spells::RIGHT {
                            "Weapon"
                        } else {
                            "SHIELD"
                        },
                    ) {
                        effect.persistent = true;
                        self.spell_fx.art.insert(key, effect);
                    }
                }
            }
            if !shader.is_null() {
                let key = (actor, shader, u64::MAX - hand as u64);
                let Some(path) = self
                    .rigid_models
                    .get(&actor)
                    .and_then(|models| models.iter().find(|(form, _)| *form == weapon))
                    .map(|(_, path)| format!("{path}#blade"))
                else {
                    continue;
                };
                let bow = self
                    .lo
                    .get(weapon)
                    .and_then(|r| r.get(b"DNAM").and_then(|d| d.first().copied()))
                    .is_some_and(|kind| kind == 7 || kind == 9);
                let node = if bow || hand == crate::spells::LEFT {
                    "SHIELD"
                } else {
                    "WEAPON"
                };
                let skeleton = if actor == PLAYER_REF {
                    self.player_body.as_ref().map(|body| &body.skeleton)
                } else {
                    self.actor_ref(actor).map(|actor| &actor.skeleton)
                };
                let Some(bone) = skeleton.and_then(|s| s.find(node)) else {
                    continue;
                };
                let target = WeaponTarget {
                    form: weapon,
                    hand,
                    path,
                    bone,
                };
                shaders.insert(key);
                if self
                    .spell_fx
                    .shaders
                    .get(&key)
                    .is_none_or(|fx| fx.weapon.as_ref() != Some(&target) || fx.stopped.is_some())
                {
                    if let Some(record) = records::EffectShader::load(&self.lo, shader) {
                        crate::world::loader::load_textures(
                            &mut self.renderer,
                            &self.vfs,
                            record.textures.iter().flatten().cloned().collect(),
                        );
                        let mut effect = ShaderEffect::new(record, f32::INFINITY, false);
                        effect.weapon = Some(target);
                        self.spell_fx.shaders.insert(key, effect);
                    }
                }
            }
        }
        for (key, effect) in &mut self.spell_fx.shaders {
            if effect.weapon.is_some() && !shaders.contains(key) {
                effect.stop();
            }
        }
        arts
    }
}
