//! Skill advancement and the player's level.
//!
//! Using a skill earns skill XP: `use mult x base XP + use offset`, the
//! skill's `AVIF` `AVSK` values, the base XP depending on the use (a blow's
//! base weapon damage, a lock's difficulty...). A skill goes up a level once
//! it has `improve mult x level ^ fSkillUseCurve + improve offset`, the extra
//! carrying over. Each skill level reached gives the player that many
//! character XP (`fXPPerSkillRank`); `fXPLevelUpBase + fXPLevelUpMult x level`
//! of them make a level up available, taken by choosing health, magicka or
//! stamina to raise (`iAVDhmsLevelUp`; stamina also carry weight,
//! `fLevelUpCarryWeightMod`), with a perk point. Only the player advances.
//! Sources: UESP's Skyrim:Leveling and skill pages, the game settings named
//! here; open questions in `known_gaps/skills.md`.

use std::collections::HashSet;
use std::sync::atomic::{AtomicU16, Ordering};

use esp::FormId;
use esp::actor_value as av;

use crate::ai::combat::{gmst_f32, gmst_i32};
use crate::engine::{Engine, PLAYER_REF};
use crate::story::StoryEvent;

/// The number of skills (`OneHanded` .. `Enchanting`).
pub const SKILL_COUNT: usize = (av::LAST_SKILL - av::FIRST_SKILL + 1) as usize;

/// Skills top out here.
const SKILL_MAX: f32 = 100.0;

/// The player's level, for what reads it without the engine (leveled lists).
static PLAYER_LEVEL: AtomicU16 = AtomicU16::new(0);

/// The player's level as leveled lists see it: the engine's, else
/// `VRM_PC_LEVEL`, else 1.
pub fn player_level() -> u16 {
    match PLAYER_LEVEL.load(Ordering::Relaxed) {
        0 => std::env::var("VRM_PC_LEVEL")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1),
        l => l,
    }
}

/// The player's progress.
#[derive(Debug, Clone)]
pub struct Skills {
    /// Skill XP towards each skill's next level (by index from `FIRST_SKILL`).
    pub xp: [f32; SKILL_COUNT],
    pub level: u16,
    /// Character XP towards the next level up.
    pub level_xp: f32,
    /// Level ups earned and not yet taken.
    pub pending: u32,
    pub perk_points: u32,
    /// Skill books already read: each teaches once.
    pub read_books: HashSet<FormId>,
}

impl Default for Skills {
    fn default() -> Self {
        Skills {
            xp: [0.0; SKILL_COUNT],
            level: player_level(),
            level_xp: 0.0,
            pending: 0,
            perk_points: 0,
            read_books: HashSet::new(),
        }
    }
}

/// A skill's `AVSK`: how uses turn into XP and how much XP each level takes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkillCurve {
    pub use_mult: f32,
    pub use_offset: f32,
    pub improve_mult: f32,
    pub improve_offset: f32,
}

impl SkillCurve {
    /// Skill XP for a use worth `base` XP.
    pub fn use_xp(&self, base: f32) -> f32 {
        self.use_mult * base + self.use_offset
    }

    /// Skill XP to go up from `level`.
    pub fn to_advance(&self, level: f32, curve: f32) -> f32 {
        self.improve_mult * level.max(0.0).powf(curve) + self.improve_offset
    }
}

/// Character XP to level up from `level`.
pub fn level_up_xp(base: f32, mult: f32, level: u16) -> f32 {
    base + mult * level as f32
}

/// A skill's index from its Papyrus / console name (`OneHanded`, `Marksman`,
/// `Speechcraft`...), case-insensitive.
pub fn skill_index(name: &str) -> Option<u32> {
    (av::FIRST_SKILL..=av::LAST_SKILL).find(|&i| av::NAMES[i as usize].eq_ignore_ascii_case(name))
}

/// `%s` and `%i` / `%d` in a game setting's text, in order.
fn printf(fmt: &str, s: &str, i: i64) -> String {
    let mut out = fmt.replacen("%s", s, 1);
    for spec in ["%i", "%d"] {
        out = out.replacen(spec, &i.to_string(), 1);
    }
    out
}

impl Engine {
    /// A skill's `AVSK` (the skill's `AVIF` is `0x446 + index`). Without one:
    /// a use is its base XP and a level `2 x level ^ curve`, as most skills.
    pub fn skill_curve(&self, skill: u32) -> SkillCurve {
        let f = |d: &[u8], o: usize| f32::from_le_bytes(d[o..o + 4].try_into().unwrap());
        self.lo
            .get(FormId(0x446 + skill))
            .and_then(|r| r.get(b"AVSK").filter(|d| d.len() >= 16).map(<[u8]>::to_vec))
            .map_or(
                SkillCurve {
                    use_mult: 1.0,
                    use_offset: 0.0,
                    improve_mult: 2.0,
                    improve_offset: 0.0,
                },
                |d| SkillCurve {
                    use_mult: f(&d, 0),
                    use_offset: f(&d, 4),
                    improve_mult: f(&d, 8),
                    improve_offset: f(&d, 12),
                },
            )
    }

    /// A skill's name as the game shows it (its `AVIF` `FULL`).
    pub fn skill_name(&self, skill: u32) -> String {
        self.lo
            .get(FormId(0x446 + skill))
            .and_then(|r| r.get(b"FULL").map(|d| self.lo.lstring(&r, d)))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| av::NAMES[skill as usize].to_string())
    }

    /// Skill XP the player's skill needs to go up from where it is.
    pub fn skill_xp_needed(&self, skill: u32) -> f32 {
        let curve = gmst_f32(&self.lo, "fSkillUseCurve", 1.95);
        self.skill_curve(skill)
            .to_advance(self.av_base(PLAYER_REF, skill), curve)
    }

    /// The player used a skill, worth `base` XP (`Game.AdvanceSkill`).
    pub fn use_skill(&mut self, skill: u32, base: f32) {
        if !(av::FIRST_SKILL..=av::LAST_SKILL).contains(&skill) || base <= 0.0 {
            return;
        }
        let xp = self.skill_curve(skill).use_xp(base);
        log::debug!(
            "{} skill use: {base:.2} -> {xp:.1} XP",
            av::NAMES[skill as usize]
        );
        self.add_skill_xp(skill, xp);
    }

    /// Skill XP into a skill: it goes up each time it has enough.
    pub fn add_skill_xp(&mut self, skill: u32, xp: f32) {
        let i = (skill - av::FIRST_SKILL) as usize;
        self.skills.xp[i] += xp;
        loop {
            if self.av_base(PLAYER_REF, skill) >= SKILL_MAX {
                self.skills.xp[i] = 0.0;
                return;
            }
            let need = self.skill_xp_needed(skill);
            if self.skills.xp[i] < need {
                return;
            }
            self.skills.xp[i] -= need;
            self.increment_skill(skill);
        }
    }

    /// The player's skill goes up one (`Game.IncrementSkill`): "<skill>
    /// increased to <n>", character XP, the Story Manager's skill increase
    /// event. False if it is already at its most.
    pub fn increment_skill(&mut self, skill: u32) -> bool {
        let level = self.av_base(PLAYER_REF, skill);
        if level >= SKILL_MAX {
            return false;
        }
        let new = level + 1.0;
        self.set_actor_value(PLAYER_REF, skill, new);
        let fmt = self
            .gmst_string("sSkillIncreased")
            .unwrap_or_else(|| "%s Increased to %i".into());
        let name = self.skill_name(skill);
        self.scripts.notify(printf(&fmt, &name, new as i64));
        let mut e = StoryEvent::new(b"SKIL");
        e.values[0] = skill as f32;
        self.send_story_event(e);
        self.add_level_xp(new * gmst_f32(&self.lo, "fXPPerSkillRank", 1.0));
        true
    }

    /// Character XP: a level up becomes available each time there are enough
    /// for the level the player will be at ("Level up available.").
    pub fn add_level_xp(&mut self, xp: f32) {
        self.skills.level_xp += xp;
        loop {
            let need = level_up_xp(
                gmst_f32(&self.lo, "fXPLevelUpBase", 75.0),
                gmst_f32(&self.lo, "fXPLevelUpMult", 25.0),
                self.skills.level + self.skills.pending as u16,
            );
            if self.skills.level_xp < need {
                return;
            }
            self.skills.level_xp -= need;
            self.skills.pending += 1;
            log::info!(
                "level up available (level {}, {} pending)",
                self.skills.level,
                self.skills.pending
            );
            let msg = self
                .gmst_string("sLevelUpAvailable")
                .unwrap_or_else(|| "Level up available.".into());
            self.scripts.notify(msg);
        }
    }

    /// Take a level up, raising `attribute` (health, magicka or stamina): the
    /// level, a perk point, health, magicka and stamina restored, and the Story
    /// Manager's level increase event. False with none to take.
    pub fn take_level_up(&mut self, attribute: u32) -> bool {
        if self.skills.pending == 0 || !(av::HEALTH..=av::STAMINA).contains(&attribute) {
            return false;
        }
        self.skills.pending -= 1;
        self.skills.level += 1;
        self.skills.perk_points += 1;
        PLAYER_LEVEL.store(self.skills.level, Ordering::Relaxed);
        let gain = gmst_i32(&self.lo, "iAVDhmsLevelUp", 10) as f32;
        let base = self.av_base(PLAYER_REF, attribute);
        self.set_actor_value(PLAYER_REF, attribute, base + gain);
        if attribute == av::STAMINA {
            let carry = self.av_base(PLAYER_REF, av::CARRY_WEIGHT);
            let more = gmst_f32(&self.lo, "fLevelUpCarryWeightMod", 5.0);
            self.set_actor_value(PLAYER_REF, av::CARRY_WEIGHT, carry + more);
        }
        for a in [av::HEALTH, av::MAGICKA, av::STAMINA] {
            let max = self.av_max(PLAYER_REF, a);
            self.damage_actor_value(PLAYER_REF, a, -max);
        }
        log::info!(
            "player is level {} ({} +{gain})",
            self.skills.level,
            av::NAMES[attribute as usize]
        );
        let mut e = StoryEvent::new(b"LEVL");
        e.values[0] = self.skills.level as f32;
        self.send_story_event(e);
        true
    }

    /// An actor's level (`GetLevel`): the player's, or the NPC's (`ACBS`): a
    /// fixed level, or with "PC level mult" the player's times the
    /// multiplier, between its calc min and max (0: none).
    pub fn actor_level(&self, actor: FormId) -> i32 {
        if actor == PLAYER_REF {
            return self.skills.level as i32;
        }
        let Some(acbs) = self
            .templates_of(actor)
            .and_then(|t| t.field(&self.lo, crate::world::template::STATS, b"ACBS"))
            .filter(|d| d.len() >= 14)
        else {
            return 1;
        };
        let u16_at = |o: usize| u16::from_le_bytes([acbs[o], acbs[o + 1]]) as i32;
        let flags = u32::from_le_bytes(acbs[0..4].try_into().unwrap());
        if flags & 0x80 == 0 {
            return u16_at(8).max(1);
        }
        let (min, max) = (u16_at(10), u16_at(12));
        let level = (self.skills.level as f32 * u16_at(8) as f32 / 1000.0).round() as i32;
        let level = if max > 0 { level.min(max) } else { level };
        level.max(min).max(1)
    }

    /// The skill the player's armor trains when struck: the kind (light or
    /// heavy, `BOD2`) most of the worn armor pieces are, heavy on a tie.
    fn worn_armor_skill(&self) -> Option<u32> {
        let worn = self.inventories.get(&PLAYER_REF)?;
        let (mut light, mut heavy) = (0, 0);
        for &f in &worn.equipped {
            let Some(rec) = self.lo.get(f).filter(|r| r.tag().0 == *b"ARMO") else {
                continue;
            };
            match rec.get(b"BOD2").and_then(|d| d.get(4).copied()) {
                Some(0) => light += 1,
                Some(1) => heavy += 1,
                _ => {}
            }
        }
        match (light, heavy) {
            (0, 0) => None,
            (l, h) if l > h => Some(av::LIGHT_ARMOR),
            _ => Some(av::HEAVY_ARMOR),
        }
    }

    /// The player took a blow of `raw` damage (before armor): the armor
    /// worn trains, a point of XP for each point of damage.
    pub(crate) fn armor_struck(&mut self, raw: f32) {
        if let Some(skill) = self.worn_armor_skill() {
            self.use_skill(skill, raw);
        }
    }

    /// The player's blow or shot landed on an actor: the weapon's skill
    /// trains by its base damage (one-handed, two-handed, archery: no skill
    /// for fists), and Sneak for a sneak attack (`fSneakAttackSkillUsageMelee`,
    /// 2.5 for a shot).
    pub(crate) fn player_blow_landed(&mut self, shot: bool, sneak: bool) {
        let weapon = self.player_weapon();
        let rec = weapon.and_then(|w| self.lo.get(w));
        let anim = rec
            .as_ref()
            .and_then(|r| r.get(b"DNAM").and_then(|d| d.first().copied()))
            .unwrap_or(0);
        let damage = rec
            .as_ref()
            .and_then(|r| r.get(b"DATA").filter(|d| d.len() >= 10))
            .map_or(0.0, |d| u16::from_le_bytes([d[8], d[9]]) as f32);
        let skill = match anim {
            7 | 9 if shot => Some(av::MARKSMAN),
            1..=4 if !shot => Some(av::ONE_HANDED),
            5 | 6 if !shot => Some(av::TWO_HANDED),
            _ => None,
        };
        if let Some(skill) = skill {
            self.use_skill(skill, damage);
        }
        if sneak {
            let base = if shot {
                SNEAK_SHOT_XP
            } else {
                gmst_f32(&self.lo, "fSneakAttackSkillUsageMelee", 30.0)
            };
            self.use_skill(av::SNEAK, base);
        }
    }

    /// Sneak trains while the player sneaks hidden from someone close enough
    /// to notice them (`fSkillUsageSneakPerSecond`).
    pub(crate) fn update_skills(&mut self, dt: f32) {
        if !self.player.sneaking || self.player_dead() {
            return;
        }
        let near = &self.detection.of_player;
        if !near.is_empty() && near.values().all(|&v| v <= 0.0) {
            let rate = gmst_f32(&self.lo, "fSkillUsageSneakPerSecond", 0.625);
            self.use_skill(av::SNEAK, rate * dt);
        }
    }

    /// A book read for the first time: a skill book (`DATA` flag 1) raises
    /// its skill one level.
    pub(crate) fn read_book(&mut self, book: FormId) {
        let Some(data) = self
            .lo
            .get(book)
            .and_then(|r| r.get(b"DATA").filter(|d| d.len() >= 8).map(<[u8]>::to_vec))
        else {
            return;
        };
        if data[0] & 1 == 0 || !self.skills.read_books.insert(book) {
            return;
        }
        let skill = u32::from_le_bytes(data[4..8].try_into().unwrap());
        if (av::FIRST_SKILL..=av::LAST_SKILL).contains(&skill) {
            log::info!("{book} teaches {}", av::NAMES[skill as usize]);
            self.increment_skill(skill);
        }
    }

    /// Console `skills`: the player's level and each skill's progress.
    pub fn describe_skills(&self) -> Vec<String> {
        let s = &self.skills;
        let need = level_up_xp(
            gmst_f32(&self.lo, "fXPLevelUpBase", 75.0),
            gmst_f32(&self.lo, "fXPLevelUpMult", 25.0),
            s.level + s.pending as u16,
        );
        let mut out = vec![format!(
            "level {} ({:.0} / {need:.0} XP), {} level ups to take, {} perk points",
            s.level, s.level_xp, s.pending, s.perk_points
        )];
        for skill in av::FIRST_SKILL..=av::LAST_SKILL {
            out.push(format!(
                "  {:12} {:>3.0}  {:>7.1} / {:.1}",
                av::NAMES[skill as usize],
                self.av_base(PLAYER_REF, skill),
                s.xp[(skill - av::FIRST_SKILL) as usize],
                self.skill_xp_needed(skill)
            ));
        }
        out
    }
}

/// Sneak XP for a sneak attack shot: UESP's, no game setting holds it.
const SNEAK_SHOT_XP: f32 = 2.5;

#[cfg(test)]
mod tests {
    use super::*;

    const LOCKPICKING: SkillCurve = SkillCurve {
        use_mult: 45.0,
        use_offset: 10.0,
        improve_mult: 0.25,
        improve_offset: 300.0,
    };

    #[test]
    fn uesp_examples() {
        // Lockpicking 15 -> 16: 0.25 x 15^1.95 + 300.
        assert!((LOCKPICKING.to_advance(15.0, 1.95) - 349.13).abs() < 0.05);
        // A use worth 50: 45 x 50 + 10.
        assert_eq!(LOCKPICKING.use_xp(50.0), 2260.0);
        // Level 1 -> 2 takes 100 character XP, 2 -> 3 125.
        assert_eq!(level_up_xp(75.0, 25.0, 1), 100.0);
        assert_eq!(level_up_xp(75.0, 25.0, 2), 125.0);
    }

    #[test]
    fn names() {
        assert_eq!(skill_index("marksman"), Some(av::MARKSMAN));
        assert_eq!(skill_index("Health"), None);
        assert_eq!(
            printf("%s Increased to %i", "Sneak", 16),
            "Sneak Increased to 16"
        );
    }
}
