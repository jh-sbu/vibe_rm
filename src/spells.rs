//! Spells in hand: what an actor has equipped in each hand and as its power
//! or shout (the magic menu, `Actor.EquipSpell` / `GetEquippedSpell`), and
//! what the magic menu says of a spell: its school, level and magicka cost.
//!
//! Casting them isn't built yet (see the roadmap). The cost formula is
//! UESP's reading of the game settings; see `known_gaps/magic-menu.md`.

use esp::FormId;
use esp::actor_value as av;

use crate::engine::Engine;
use crate::magic::{MagicItem, spell_type};

/// `Actor.GetEquippedSpell`'s slots.
pub const RIGHT: usize = 0;
pub const LEFT: usize = 1;
/// A power, lesser power or shout.
pub const VOICE: usize = 2;

/// The magic schools' skills, in the magic menu's order.
pub const SCHOOLS: [u32; 5] = [18, 21, 20, 19, 22];
/// Alteration, Conjuration, Destruction, Illusion, Restoration.
const FIRST_SCHOOL: u32 = 18;
const LAST_SCHOOL: u32 = 22;

/// `MGEF` `DATA` offsets: base cost, minimum skill level.
const MGEF_BASE_COST: usize = 0x04;
const MGEF_MIN_SKILL: usize = 0x28;

/// Concentration (`SPIT` cast type).
pub const CONCENTRATION: u32 = 2;

/// `ModSpellCost` (`BGSEntryPoint`).
const MOD_SPELL_COST: u8 = 38;

/// Whether a weapon needs both hands (two-handed blades and axes, bows,
/// staves aside, crossbows: `DNAM` animation types 5, 6, 7 and 9).
pub fn two_handed(engine: &Engine, weapon: FormId) -> bool {
    engine
        .lo
        .get(weapon)
        .filter(|r| r.tag().0 == *b"WEAP")
        .and_then(|r| r.get(b"DNAM").and_then(|d| d.first().copied()))
        .is_some_and(|t| matches!(t, 5 | 6 | 7 | 9))
}

impl Engine {
    /// What `actor` holds in a slot (`RIGHT`, `LEFT`, `VOICE`).
    pub fn equipped_spell(&self, actor: FormId, slot: usize) -> Option<FormId> {
        self.equipped_spells
            .get(&actor)
            .and_then(|s| s.get(slot).copied().flatten())
    }

    /// Readies a spell the actor knows in a hand (powers and shouts in the
    /// voice slot whatever is asked). A spell in the right hand puts the
    /// weapon away; in the left, the shield, and a two-handed weapon either
    /// way.
    pub fn equip_spell(&mut self, actor: FormId, spell: FormId, slot: usize) {
        let Some(item) = self.magic_item(spell) else {
            return;
        };
        let slot = match item.spell_type {
            spell_type::POWER | spell_type::LESSER_POWER | spell_type::VOICE => VOICE,
            _ if slot == LEFT => LEFT,
            _ => RIGHT,
        };
        if slot != VOICE {
            let weapon = self
                .inventories
                .get(&actor)
                .and_then(|i| i.weapon(&self.lo));
            if let Some(w) = weapon
                && (slot == RIGHT || two_handed(self, w))
            {
                let _ = self.equip_item(actor, w, false);
            }
            if slot == LEFT
                && let Some(s) = self.equipped_shield(actor)
            {
                let _ = self.equip_armor(actor, s, false);
            }
        }
        self.equipped_spells.entry(actor).or_default()[slot] = Some(spell);
        log::info!("{actor} readies {} ({spell}) in slot {slot}", item.name);
    }

    /// Takes a spell out of a slot (any slot when `None`).
    pub fn unequip_spell(&mut self, actor: FormId, spell: FormId, slot: Option<usize>) {
        if let Some(slots) = self.equipped_spells.get_mut(&actor) {
            for (i, s) in slots.iter_mut().enumerate() {
                if *s == Some(spell) && slot.is_none_or(|x| x == i) {
                    *s = None;
                }
            }
        }
    }

    /// A hand taken by a weapon or shield: the spell in it goes (a two-handed
    /// weapon takes both).
    pub fn hand_taken(&mut self, actor: FormId, item: FormId) {
        let tag = self.lo.tag_of(item).map(|t| t.0);
        let hands: &[usize] = match tag {
            Some(t) if t == *b"WEAP" && two_handed(self, item) => &[RIGHT, LEFT],
            Some(t) if t == *b"WEAP" => &[RIGHT],
            Some(t) if t == *b"ARMO" && self.equipped_shield(actor) == Some(item) => &[LEFT],
            _ => &[],
        };
        if let Some(slots) = self.equipped_spells.get_mut(&actor) {
            for &h in hands {
                slots[h] = None;
            }
        }
    }

    /// The effect that decides a spell's school, level and cost: its costliest
    /// (`EffectItem`, base cost).
    fn costliest_effect(&self, item: &MagicItem) -> Option<(usize, f32)> {
        item.effects
            .iter()
            .enumerate()
            .map(|(i, e)| (i, self.effect_cost(e)))
            .max_by(|a, b| a.1.total_cmp(&b.1))
    }

    /// One effect's weight in the spell: base cost × magnitude^1.1 × (duration
    /// / 10)^1.1, magnitude at least 1 and duration at least 10 (the
    /// costliest effect decides the school and level).
    fn effect_cost(&self, e: &crate::magic::EffectItem) -> f32 {
        let base = self
            .lo
            .get(e.effect)
            .and_then(|r| {
                r.get(b"DATA")
                    .filter(|d| d.len() >= MGEF_BASE_COST + 4)
                    .map(|d| {
                        f32::from_le_bytes(
                            d[MGEF_BASE_COST..MGEF_BASE_COST + 4].try_into().unwrap(),
                        )
                    })
            })
            .unwrap_or(0.0);
        let mag = e.magnitude.max(1.0);
        let dur = (e.duration as f32).max(10.0) / 10.0;
        base * mag.powf(1.1) * dur.powf(1.1)
    }

    /// A spell's school: its costliest effect's skill, when one of the five.
    pub fn spell_school(&self, spell: FormId) -> Option<u32> {
        let item = self.magic_item(spell)?;
        let (i, _) = self.costliest_effect(&item)?;
        self.magic_effect(item.effects[i].effect)?
            .skill
            .filter(|s| (FIRST_SCHOOL..=LAST_SCHOOL).contains(s))
    }

    /// The level of a spell (Novice to Master) by its costliest effect's
    /// minimum skill.
    pub fn spell_level_name(&self, spell: FormId) -> String {
        let item = self.magic_item(spell);
        let min = item
            .as_ref()
            .and_then(|i| Some((i, self.costliest_effect(i)?.0)))
            .and_then(|(i, k)| self.lo.get(i.effects[k].effect))
            .and_then(|r| {
                r.get(b"DATA")
                    .filter(|d| d.len() >= MGEF_MIN_SKILL + 4)
                    .map(|d| {
                        u32::from_le_bytes(
                            d[MGEF_MIN_SKILL..MGEF_MIN_SKILL + 4].try_into().unwrap(),
                        )
                    })
            })
            .unwrap_or(0);
        let (setting, english) = match min {
            0..25 => ("sNoviceDisplayName", "Novice"),
            25..50 => ("sApprenticeDisplayName", "Apprentice"),
            50..75 => ("sJourneymanDisplayName", "Adept"),
            75..100 => ("sExpertDisplayName", "Expert"),
            _ => ("sMasterDisplayName", "Master"),
        };
        self.gmst_string(setting)
            .unwrap_or_else(|| english.to_string())
    }

    /// What casting a spell costs `caster` in magicka (a second of it for
    /// concentration spells): the record's cost (`SPIT`, which the Creation
    /// Kit works out when it isn't set by hand), less for skill in the school
    /// (`fMagicCasterPCSkillCostBase`, `fMagicPCSkillCostScale`), through the
    /// caster's `ModSpellCost` perks.
    pub fn spell_cost(&self, caster: FormId, spell: FormId) -> f32 {
        let base = self
            .lo
            .get(spell)
            .and_then(|r| {
                r.get(b"SPIT")
                    .filter(|d| d.len() >= 4)
                    .map(|d| u32::from_le_bytes(d[0..4].try_into().unwrap()) as f32)
            })
            .unwrap_or(0.0);
        let mut cost = base;
        if let Some(school) = self.spell_school(spell) {
            let g = |name, default| crate::ai::combat::gmst_f32(&self.lo, name, default);
            let skill = self.actor_value(caster, school).clamp(0.0, 100.0);
            let k = g("fMagicCasterPCSkillCostBase", 0.0025);
            let scale = g("fMagicPCSkillCostScale", 0.65);
            cost *= 1.0 - (k * skill).powf(scale);
        }
        cost = self.perk_entry_point(MOD_SPELL_COST, caster, &[Some(spell)], cost);
        cost.max(0.0)
    }

    /// The skill's progress to its next level, 0 to 1 (the magic menu's
    /// school meter).
    pub fn skill_progress(&self, skill: u32) -> f32 {
        if !(av::FIRST_SKILL..=av::LAST_SKILL).contains(&skill) {
            return 0.0;
        }
        let need = self.skill_xp_needed(skill);
        let xp = self.skills.xp[(skill - av::FIRST_SKILL) as usize];
        if need > 0.0 {
            (xp / need).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}
