//! Spells in hand: what an actor has equipped in each hand and as its power
//! or shout (the magic menu, `Actor.EquipSpell` / `GetEquippedSpell`), and
//! what the magic menu says of a spell: its school, level and magicka cost.
//!
//! The cost is shared by the magic menu and casting. Sources and remaining
//! questions: `known_ambiguities/magic-menu.md`.

use esp::FormId;
use esp::actor_value as av;

use crate::engine::Engine;
use crate::magic::{MagicItem, spell_type};
use crate::world::inventory::equip_slot;

/// `Actor.GetEquippedSpell`'s slots: the hands, and the voice (a power,
/// lesser power or shout).
pub use crate::world::inventory::{LEFT, RIGHT, VOICE};

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

/// What skill in a school leaves of a cost: `1 - (fMagicCasterPCSkillCostBase
/// x skill) ^ fMagicPCSkillCostScale` (0.59 at 100).
pub(crate) fn skill_cost_factor(lo: &esp::LoadOrder, skill: f32) -> f32 {
    let g = |name, default| crate::ai::combat::gmst_f32(lo, name, default);
    let k = g("fMagicCasterPCSkillCostBase", 0.0025);
    let scale = g("fMagicPCSkillCostScale", 0.65);
    1.0 - (k * skill).powf(scale)
}

impl Engine {
    /// The spell, power or shout `actor` holds in a slot (`RIGHT`, `LEFT`,
    /// `VOICE`).
    pub fn equipped_spell(&self, actor: FormId, slot: usize) -> Option<FormId> {
        self.inventories
            .get(&actor)?
            .held(slot)
            .filter(|&f| matches!(self.lo.tag_of(f).map(|t| t.0), Some(t) if t == *b"SPEL" || t == *b"SHOU"))
    }

    /// Readies a spell the actor knows in a hand (powers and shouts in the
    /// voice slot whatever is asked), in place of what the hand holds: the
    /// hand asked for if its equip slot (`ETYP`) allows it, else its own
    /// (`IncinerateLeftHand`); both for spells cast with both hands.
    pub fn equip_spell(&mut self, actor: FormId, spell: FormId, slot: usize) {
        let Some(item) = self.magic_item(spell) else {
            return;
        };
        let power = matches!(
            item.spell_type,
            spell_type::POWER | spell_type::LESSER_POWER | spell_type::VOICE
        );
        let hands = match equip_slot(&self.lo, spell) {
            _ if power => vec![VOICE],
            Some(s) if s.voice => vec![VOICE],
            Some(s) => s.hands(Some(slot), false),
            None => vec![if slot == LEFT { LEFT } else { RIGHT }],
        };
        self.hold_magic(actor, spell, &hands);
        log::info!("{actor} readies {} ({spell}) in {hands:?}", item.name);
    }

    /// Holds a spell or shout in `slots`, in place of what they hold.
    pub(crate) fn hold_magic(&mut self, actor: FormId, form: FormId, slots: &[usize]) {
        if slots.is_empty() {
            return;
        }
        self.inventory_mut(actor);
        let lo = &self.lo;
        if let Some(inv) = self.inventories.get_mut(&actor) {
            inv.hold(lo, form, slots);
        }
    }

    /// Readies a spell in a hand (`RIGHT` or `LEFT`), a power or shout as the
    /// voice, or puts it away when it already is (the magic and favorites
    /// menus).
    pub fn toggle_spell(&mut self, actor: FormId, spell: FormId, slot: usize) {
        if self.is_shout(spell) {
            if self.equipped_spell(actor, VOICE) == Some(spell) {
                self.unequip_spell(actor, spell, Some(VOICE));
            } else {
                self.equip_shout(actor, spell);
            }
            return;
        }
        let power = self
            .magic_item(spell)
            .is_some_and(|i| i.spell_type != spell_type::SPELL);
        let slot = if power { VOICE } else { slot };
        if self.equipped_spell(actor, slot) == Some(spell) {
            self.unequip_spell(actor, spell, Some(slot));
        } else {
            self.equip_spell(actor, spell, slot);
        }
    }

    /// Takes a spell out of a slot (any slot when `None`; both hands when it
    /// fills both).
    pub fn unequip_spell(&mut self, actor: FormId, spell: FormId, slot: Option<usize>) {
        let lo = &self.lo;
        if let Some(inv) = self.inventories.get_mut(&actor) {
            for s in [RIGHT, LEFT, VOICE] {
                if inv.held(s) == Some(spell) && slot.is_none_or(|x| x == s) {
                    inv.release(lo, s);
                }
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

    /// The spell's authored casting perk (`SPIT` byte 32), used by
    /// `SpellHasCastingPerk` in the Novice .. Master cost perks. This is
    /// a property of the spell, independent of who owns the perk.
    pub fn spell_casting_perk(&self, spell: FormId) -> Option<FormId> {
        let rec = self.lo.get(spell).filter(|r| r.tag().0 == *b"SPEL")?;
        let bytes = rec.get(b"SPIT")?.get(32..36)?;
        let perk = rec.fid(FormId(u32::from_le_bytes(bytes.try_into().unwrap())));
        (!perk.is_null()).then_some(perk)
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
    /// caster's `ModSpellCost` perks (including `PerkSkillBoosts`, which
    /// applies the school's Fortify cost reduction from the game records).
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
            let skill = self.actor_value(caster, school).clamp(0.0, 100.0);
            cost *= skill_cost_factor(&self.lo, skill);
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
