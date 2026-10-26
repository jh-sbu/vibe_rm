//! Magic: magic effects, the items carrying them (spells, enchantments,
//! potions, ingredients, scrolls) and the effects active on actors.
//!
//! An item lists effects (`EFID` + `EFIT`: magnitude, area, duration, with
//! their own conditions). Applied to an actor, each becomes an active effect
//! for its duration (abilities and diseases for as long as the actor has
//! them), doing what its archetype does: value modifiers change an actor
//! value; abilities and effects with the Recover flag hold the change and
//! give it back at the end, effects with a duration otherwise apply their
//! magnitude every second, and those without one apply it once. Detrimental
//! effects take away. Magic effects' scripts run on the active effect
//! (`ActiveMagicEffect`: `OnEffectStart` / `OnEffectFinish`), which also hears
//! its target's events. Effects whose conditions stop passing go quiet
//! (finishing) until they pass again.
//!
//! Sources: UESP's MGEF / SPEL / ALCH / ENCH record pages, CommonLibSSE's
//! `EffectSetting`, `SpellItem`, `MagicSystem` and `ActiveEffect` headers, the
//! records themselves (`vrm-tool magic`). Open questions: `known_ambiguities/magic.md`.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use esp::FormId;
use esp::actor_value as av;
use papyrus::{ObjectId, Value};

use crate::condition::{self, Condition, Context};
use crate::engine::{Engine, PLAYER_REF};
use crate::spell_fx::records::{optional_float, optional_form};

/// Magic effect archetypes (`DATA` 0x40; CommonLibSSE `EffectArchetype`).
#[allow(dead_code)]
/// Seconds between a cloak's touches.
const CLOAK_TICK: f32 = 1.0;

pub mod archetype {
    pub const VALUE_MODIFIER: u32 = 0;
    pub const SCRIPT: u32 = 1;
    pub const CURE_DISEASE: u32 = 3;
    pub const ABSORB: u32 = 4;
    pub const DUAL_VALUE_MODIFIER: u32 = 5;
    pub const CALM: u32 = 6;
    pub const DEMORALIZE: u32 = 7;
    pub const FRENZY: u32 = 8;
    pub const DISARM: u32 = 9;
    pub const INVISIBILITY: u32 = 11;
    pub const BOUND_WEAPON: u32 = 17;
    pub const SUMMON_CREATURE: u32 = 18;
    pub const TELEKINESIS: u32 = 20;
    pub const PARALYSIS: u32 = 21;
    pub const SOUL_TRAP: u32 = 23;
    pub const CLOAK: u32 = 35;
    pub const TURN_UNDEAD: u32 = 24;
    pub const CURE_PARALYSIS: u32 = 27;
    pub const CURE_POISON: u32 = 29;
    /// Wards (`WardPower`).
    pub const ACCUMULATE_MAGNITUDE: u32 = 32;
    pub const STAGGER: u32 = 33;
    pub const PEAK_VALUE_MODIFIER: u32 = 34;
    /// Slow Time (`SlowTimeEffect`): the world's speed, as its magnitude.
    pub const SLOW_TIME: u32 = 37;
    pub const RALLY: u32 = 38;
    pub const ETHEREALIZE: u32 = 41;

    /// Target value modifiers (CommonLibSSE `TargetValueModifierEffect`)
    /// hold their actor value at a value of their own while they last, and
    /// the value they hold it at (`GetTargetValue`: calm -1, demoralize 0,
    /// frenzy 3; rally's isn't given: foolhardy, as it keeps the target from
    /// fleeing).
    pub fn target_value(archetype: u32) -> Option<f32> {
        match archetype {
            CALM => Some(-1.0),
            DEMORALIZE | TURN_UNDEAD => Some(0.0),
            FRENZY => Some(3.0),
            RALLY => Some(4.0),
            _ => None,
        }
    }

    /// Those whose magnitude is the highest level they affect (UESP's
    /// Illusion: "Target Level does not apply to Rally effects").
    pub fn level_capped(archetype: u32) -> bool {
        matches!(archetype, CALM | DEMORALIZE | FRENZY | TURN_UNDEAD | DISARM)
    }
}

/// Magic effect flags (`DATA` 0x00).
#[allow(dead_code)]
pub mod flags {
    pub const HOSTILE: u32 = 0x1;
    pub const RECOVER: u32 = 0x2;
    pub const DETRIMENTAL: u32 = 0x4;
    pub const NO_DURATION: u32 = 0x200;
    pub const HIDE_IN_UI: u32 = 0x8000;
    pub const FX_PERSIST: u32 = 1 << 12;
    pub const NO_HIT_EFFECT: u32 = 1 << 27;
    /// A spell's power (dual casting) scales its duration rather than its
    /// magnitude (`test-mod dualcast`).
    pub const POWER_AFFECTS_DURATION: u32 = 0x40_0000;
    pub const NO_DEATH_DISPEL: u32 = 0x1000_0000;
}

/// Spell types (`SPIT` 0x08; CommonLibSSE `MagicSystem::SpellType`).
#[allow(dead_code)]
pub mod spell_type {
    pub const SPELL: u32 = 0;
    pub const DISEASE: u32 = 1;
    pub const POWER: u32 = 2;
    pub const LESSER_POWER: u32 = 3;
    pub const ABILITY: u32 = 4;
    pub const POISON: u32 = 5;
    pub const ENCHANTMENT: u32 = 6;
    pub const POTION: u32 = 7;
    pub const INGREDIENT: u32 = 8;
    pub const ADDICTION: u32 = 10;
    pub const VOICE: u32 = 11;
    pub const STAFF_ENCHANTMENT: u32 = 12;
    pub const SCROLL: u32 = 13;
}

/// The constant effect casting type.
pub const CONSTANT_EFFECT: u32 = 0;

/// Every effect of an item (`apply_item_as`'s mask).
const ALL_EFFECTS: u64 = u64::MAX;

/// `SPIT` flag: a dual cast lands as two casts' worth, without dual
/// casting's bonus (`test-mod dualcast`).
const NO_DUAL_CAST_MODS: u32 = 0x80_0000;

/// Seconds a channelled effect lasts after its spell's last landing, when
/// nothing says the spell is still cast (the player's hands do).
const CHANNEL_LINGER: f32 = 0.5;

/// How often abilities and conditions are looked at again (seconds).
const SYNC_INTERVAL: f32 = 1.0;
/// Real seconds a finished effect's scripts are kept, so `OnEffectFinish`
/// (and what it waits on) can run.
const SCRIPT_LINGER: f64 = 30.0;

/// A `MGEF` record as read.
#[derive(Debug, Clone, Default)]
pub struct MagicEffect {
    pub name: String,
    pub flags: u32,
    /// The archetype's form (a keyword for peak value modifiers, the NPC
    /// summoned, the light...).
    pub related: FormId,
    pub skill: Option<u32>,
    pub resist: Option<u32>,
    pub archetype: u32,
    pub primary: Option<u32>,
    pub secondary: Option<u32>,
    pub second_weight: f32,
    /// What it flies as when aimed (`PROJ`), and its impact data set.
    pub projectile: FormId,
    pub impacts: FormId,
    pub casting_light: FormId,
    pub hit_shader: FormId,
    pub enchant_shader: FormId,
    pub casting_art: FormId,
    pub hit_art: FormId,
    pub dual_cast: (FormId, f32),
    pub enchant_art: FormId,
    pub hit_visuals: FormId,
    pub enchant_visuals: FormId,
    /// Skill XP for a use is scaled by this.
    pub skill_usage_mult: f32,
    /// Player-only perk held while this effect is active (`DATA` 0x88).
    pub perk: FormId,
    /// Taper (`DATA` 0x1C weight, 0x34 curve, 0x38 duration): what lingers
    /// after the effect (fire's burn).
    pub taper: [f32; 3],
    pub keywords: Vec<FormId>,
    pub conditions: Vec<Condition>,
    pub scripts: Vec<crate::script::vmad::ScriptRef>,
}

impl MagicEffect {
    pub fn load(e: &Engine, id: FormId) -> Option<MagicEffect> {
        Self::load_from(&e.lo, id)
    }

    pub fn load_from(lo: &esp::LoadOrder, id: FormId) -> Option<MagicEffect> {
        let rec = lo.get(id).filter(|r| r.tag().0 == *b"MGEF")?;
        let d = rec.get(b"DATA").filter(|d| d.len() >= 0x6C)?;
        let u = |o: usize| u32::from_le_bytes(d[o..o + 4].try_into().unwrap());
        let f = |o: usize| f32::from_bits(u(o));
        let av = |o: usize| Some(u(o) as i32).filter(|&v| v >= 0).map(|v| v as u32);
        let fid = |v: u32| {
            if v == 0 {
                FormId::NULL
            } else {
                rec.fid(FormId(v))
            }
        };
        Some(MagicEffect {
            name: rec
                .get(b"FULL")
                .map(|x| lo.lstring(&rec, x))
                .unwrap_or_default(),
            flags: u(0),
            related: fid(u(8)),
            skill: av(0x0C),
            resist: av(0x10),
            archetype: u(0x40),
            primary: av(0x44),
            secondary: av(0x58),
            second_weight: f(0x3C),
            projectile: fid(u(0x48)),
            impacts: fid(u(0x64)),
            casting_light: fid(u(0x18)),
            hit_shader: fid(u(0x20)),
            enchant_shader: fid(u(0x24)),
            casting_art: fid(u(0x5C)),
            hit_art: fid(u(0x60)),
            dual_cast: (optional_form(&rec, d, 0x6C), optional_float(d, 0x70)),
            enchant_art: optional_form(&rec, d, 0x74),
            hit_visuals: optional_form(&rec, d, 0x78),
            enchant_visuals: optional_form(&rec, d, 0x7C),
            skill_usage_mult: f(0x68),
            perk: d.get(0x88..0x8C).map_or(FormId::NULL, |b| {
                fid(u32::from_le_bytes(b.try_into().unwrap()))
            }),
            taper: [f(0x1C), f(0x34), f(0x38)],
            keywords: rec
                .get(b"KWDA")
                .map(|k| {
                    k.chunks_exact(4)
                        .map(|c| fid(u32::from_le_bytes(c.try_into().unwrap())))
                        .collect()
                })
                .unwrap_or_default(),
            conditions: condition::parse_all(&rec),
            scripts: crate::script::vmad::parse(&rec)
                .map(|v| v.scripts)
                .unwrap_or_default(),
        })
    }

    pub fn has(&self, flag: u32) -> bool {
        self.flags & flag != 0
    }
}

/// One effect of an item (`EFID` / `EFIT` and the conditions after them).
#[derive(Debug, Clone)]
pub struct EffectItem {
    pub effect: FormId,
    pub magnitude: f32,
    pub area: u32,
    pub duration: u32,
    pub conditions: Vec<Condition>,
}

/// A spell, enchantment, potion, ingredient or scroll.
#[derive(Debug, Clone, Default)]
pub struct MagicItem {
    pub id: FormId,
    pub name: String,
    /// `MagicSystem::SpellType` (potions 7, poisons 5, ingredients 8...).
    pub spell_type: u32,
    pub cast_type: u32,
    pub delivery: u32,
    pub flags: u32,
    pub cost: Option<f32>,
    pub charge_time: f32,
    /// How far a target actor spell reaches (`SPIT` range).
    pub range: f32,
    /// The item's own keywords (`KWDA`).
    pub keywords: Vec<FormId>,
    pub effects: Vec<EffectItem>,
}

impl MagicItem {
    pub fn load(e: &Engine, id: FormId) -> Option<MagicItem> {
        let rec = e.lo.get(id)?;
        let tag = rec.tag().0;
        let mut m = MagicItem {
            id,
            name: rec
                .get(b"FULL")
                .map(|x| e.lo.lstring(&rec, x))
                .unwrap_or_default(),
            ..Default::default()
        };
        let u = |d: &[u8], o: usize| {
            d.get(o..o + 4)
                .map_or(0, |b| u32::from_le_bytes(b.try_into().unwrap()))
        };
        match &tag {
            b"SPEL" | b"SCRL" => {
                let d = rec.get(b"SPIT").unwrap_or_default();
                m.flags = u(d, 4);
                m.cost = (m.flags & 1 != 0).then(|| u(d, 0) as f32);
                m.spell_type = if tag == *b"SCRL" {
                    spell_type::SCROLL
                } else {
                    u(d, 8)
                };
                m.charge_time = f32::from_bits(u(d, 0x0C));
                m.cast_type = u(d, 0x10);
                m.delivery = u(d, 0x14);
                m.range = f32::from_bits(u(d, 0x1C));
            }
            b"ENCH" => {
                let d = rec.get(b"ENIT").unwrap_or_default();
                m.flags = u(d, 4);
                m.cast_type = u(d, 8);
                m.delivery = u(d, 0x10);
                m.spell_type = u(d, 0x14);
                m.charge_time = f32::from_bits(u(d, 0x18));
            }
            b"ALCH" => {
                let d = rec.get(b"ENIT").unwrap_or_default();
                m.flags = u(d, 4);
                m.spell_type = if m.flags & 0x20000 != 0 {
                    spell_type::POISON
                } else {
                    spell_type::POTION
                };
                m.cast_type = 1;
            }
            b"INGR" => {
                m.spell_type = spell_type::INGREDIENT;
                m.cast_type = 1;
            }
            _ => return None,
        }
        m.keywords = rec
            .get(b"KWDA")
            .map(|k| {
                k.chunks_exact(4)
                    .map(|c| rec.fid(FormId(u32::from_le_bytes(c.try_into().unwrap()))))
                    .collect()
            })
            .unwrap_or_default();
        // Each EFID starts an effect; the EFIT and conditions after it are its.
        for s in rec.subrecords() {
            match &s.tag.0 {
                b"EFID" if s.data.len() >= 4 => m.effects.push(EffectItem {
                    effect: rec.fid(FormId(s.u32(0))),
                    magnitude: 0.0,
                    area: 0,
                    duration: 0,
                    conditions: Vec::new(),
                }),
                b"EFIT" if s.data.len() >= 12 => {
                    if let Some(x) = m.effects.last_mut() {
                        x.magnitude = s.f32(0);
                        x.area = s.u32(4);
                        x.duration = s.u32(8);
                    }
                }
                b"CTDA" => {
                    if let (Some(x), Some(c)) =
                        (m.effects.last_mut(), condition::parse(&rec, s.data))
                    {
                        x.conditions.push(c);
                    }
                }
                b"CIS1" | b"CIS2" => {
                    if let Some(c) = m.effects.last_mut().and_then(|x| x.conditions.last_mut()) {
                        let text = Some(s.zstring().into());
                        if s.tag.0 == *b"CIS1" {
                            c.string_p1 = text;
                        } else {
                            c.string_p2 = text;
                        }
                    }
                }
                _ => {}
            }
        }
        Some(m)
    }

    /// Abilities, diseases and addictions are on for as long as the actor has
    /// them.
    pub fn is_constant(&self) -> bool {
        matches!(
            self.spell_type,
            spell_type::ABILITY | spell_type::DISEASE | spell_type::ADDICTION
        ) || self.cast_type == CONSTANT_EFFECT
    }
}

/// An effect on an actor.
#[derive(Debug, Clone, PartialEq)]
pub struct ActiveEffect {
    pub id: u64,
    pub target: FormId,
    pub caster: Option<FormId>,
    pub item: FormId,
    /// Which of the item's effects.
    pub index: usize,
    pub effect: FormId,
    pub magnitude: f32,
    /// Seconds (`f32::INFINITY` for constant effects; 0 applies once).
    pub duration: f32,
    pub elapsed: f32,
    /// Its conditions passed when last looked at, and it is doing its work.
    pub active: bool,
    /// What it holds on its actor values (primary, secondary) to give back.
    pub held: [f32; 2],
    /// Its magic effect has scripts (running on `ObjectId::Effect(id)`).
    pub scripted: bool,
    pub done: bool,
    /// Landed unnoticed (`apply_item_quietly`): what it does is no blow of
    /// the caster's.
    pub quiet: bool,
    /// Where the spell landed (target location spells).
    pub at: Option<glam::Vec3>,
    /// Summons: the actor it brought.
    pub summoned: Option<FormId>,
    /// Its spell was dual cast (`EffectWasDualCast`).
    pub dual: bool,
    /// A concentration spell's effect without a duration (wards): it lasts
    /// as long as the spell is cast, each landing keeping it on.
    pub channelled: bool,
}

/// What leveled spell lists gave actors, rolled when first asked for and kept.
#[derive(Debug, Default)]
struct RolledSpells(std::cell::RefCell<HashMap<(FormId, FormId), Vec<FormId>>>);

/// One leveled spell-list result retained in an optional native save section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SavedRolledSpell {
    pub actor: FormId,
    pub list: FormId,
    pub spells: Vec<FormId>,
}

impl RolledSpells {
    /// What `list` gave `actor`, rolling it with `roll` the first time only.
    fn get_or_roll(
        &self,
        actor: FormId,
        list: FormId,
        roll: impl FnOnce() -> Vec<FormId>,
    ) -> Vec<FormId> {
        if let Some(v) = self.0.borrow().get(&(actor, list)) {
            return v.clone();
        }
        let v = roll();
        self.0.borrow_mut().insert((actor, list), v.clone());
        v
    }

    fn save(&self) -> Vec<SavedRolledSpell> {
        let mut saved: Vec<_> = self
            .0
            .borrow()
            .iter()
            .map(|(&(actor, list), spells)| SavedRolledSpell {
                actor,
                list,
                spells: spells.clone(),
            })
            .collect();
        saved.sort_by_key(|entry| (entry.actor, entry.list));
        saved
    }

    fn restore(&mut self, saved: Vec<SavedRolledSpell>) {
        self.0 = std::cell::RefCell::new(
            saved
                .into_iter()
                .map(|entry| ((entry.actor, entry.list), entry.spells))
                .collect(),
        );
    }
}

/// Magic on actors: the active effects, spells added and removed, and what was
/// read.
#[derive(Debug, Default)]
pub struct Magic {
    pub effects: Vec<ActiveEffect>,
    next_id: u64,
    /// Summoned actors and who commands them.
    pub commanded: HashMap<FormId, FormId>,
    /// Spells added (`AddSpell`) and record spells removed (`RemoveSpell`).
    pub added: HashMap<FormId, Vec<FormId>>,
    pub removed: HashMap<FormId, HashSet<FormId>>,
    sync: f32,
    /// Finished effects' script objects and when to let them go.
    linger: Vec<(u64, f64)>,
    /// What lingers after instant value changes (the effect as it landed,
    /// the rate it starts at, the seconds since).
    tapers: Vec<(ActiveEffect, Arc<MagicEffect>, f32, f32)>,
    effect_cache: std::cell::RefCell<HashMap<FormId, Option<Arc<MagicEffect>>>>,
    item_cache: std::cell::RefCell<HashMap<FormId, Option<Arc<MagicItem>>>>,
    rolled: RolledSpells,
    /// Bodies whose soul has been taken (`crate::soul_trap`).
    pub souls_taken: HashSet<FormId>,
    /// Actors' last ward hit (`IsWardState`: absorbed 1, broken 2) and the
    /// seconds it still holds (`crate::wards`).
    pub ward_states: HashMap<FormId, (i32, f32)>,
}

impl Engine {
    pub(crate) fn saved_rolled_spells(&self) -> Vec<SavedRolledSpell> {
        self.magic.rolled.save()
    }

    pub(crate) fn validate_saved_rolled_spells(
        &self,
        saved: &[SavedRolledSpell],
    ) -> anyhow::Result<()> {
        let mut previous = None;
        for entry in saved {
            let key = (entry.actor, entry.list);
            anyhow::ensure!(
                !entry.actor.is_null()
                    && !entry.list.is_null()
                    && entry.spells.iter().all(|spell| !spell.is_null())
                    && previous.is_none_or(|previous| previous < key),
                "save has invalid, duplicate or unsorted rolled spell-list result"
            );
            anyhow::ensure!(
                self.lo
                    .tag_of(entry.list)
                    .is_some_and(|tag| tag.0 == *b"LVSP"),
                "save has non-leveled spell-list key {}",
                entry.list
            );
            previous = Some(key);
        }
        Ok(())
    }

    pub(crate) fn restore_rolled_spells(&mut self, saved: Vec<SavedRolledSpell>) {
        self.magic.rolled.restore(saved);
    }

    /// The effects a native save keeps (`AEFF`), in id order, and the next
    /// id. Finished effects and telekinesis (its hold on an object isn't
    /// saved) are left out.
    pub(crate) fn saved_active_effects(&self) -> (Vec<ActiveEffect>, u64) {
        let mut saved: Vec<ActiveEffect> = self
            .magic
            .effects
            .iter()
            .filter(|x| !x.done)
            .filter(|x| {
                self.magic_effect(x.effect)
                    .is_none_or(|m| m.archetype != archetype::TELEKINESIS)
            })
            .cloned()
            .collect();
        saved.sort_by_key(|x| x.id);
        (saved, self.magic.next_id)
    }

    /// Saved effects must name this load order's items and their effects.
    pub(crate) fn validate_saved_active_effects(
        &self,
        saved: &[ActiveEffect],
    ) -> anyhow::Result<()> {
        for x in saved {
            let item = self.magic_item(x.item);
            anyhow::ensure!(
                item.as_ref()
                    .and_then(|m| m.effects.get(x.index))
                    .is_some_and(|e| e.effect == x.effect)
                    && self.magic_effect(x.effect).is_some(),
                "save has active effect {} that isn't effect {} of item {}",
                x.id,
                x.index,
                x.item
            );
        }
        Ok(())
    }

    /// The taper runs a native save keeps (`TAPR`): the effect as it
    /// landed, its starting rate and the seconds since.
    pub(crate) fn saved_tapers(&self) -> Vec<(ActiveEffect, f32, f32)> {
        self.magic
            .tapers
            .iter()
            .map(|(x, _, rate, elapsed)| (x.clone(), *rate, *elapsed))
            .collect()
    }

    /// Saved tapers whose magic effect still loads go on; they replace any
    /// running (after `restore_active_effects`, which clears them).
    pub(crate) fn restore_tapers(&mut self, saved: Vec<(ActiveEffect, f32, f32)>) {
        self.magic.tapers = saved
            .into_iter()
            .filter_map(|(x, rate, elapsed)| {
                let mgef = self.magic_effect(x.effect)?;
                Some((x, mgef, rate, elapsed))
            })
            .collect();
    }

    /// Replace the active effects with saved ones. What they hold on actor
    /// values is already in the saved modifiers, so nothing is started or
    /// given back: the old game's effects go without releasing into the new
    /// values, and the restored ones carry on. Summons command again.
    pub(crate) fn restore_active_effects(&mut self, saved: Vec<ActiveEffect>, next_id: u64) {
        let m = &mut self.magic;
        m.next_id = saved.iter().map(|x| x.id).fold(next_id, u64::max);
        m.commanded = saved
            .iter()
            .filter_map(|x| Some((x.summoned?, x.caster.unwrap_or(x.target))))
            .collect();
        m.effects = saved;
        m.tapers.clear();
        m.linger.clear();
        m.sync = 0.0;
        // What the old game held (with telekinesis or by hand) isn't saved.
        self.grab = crate::grab::Grab::default();
    }

    pub fn magic_effect(&self, id: FormId) -> Option<Arc<MagicEffect>> {
        if let Some(m) = self.magic.effect_cache.borrow().get(&id) {
            return m.clone();
        }
        let m = MagicEffect::load(self, id).map(Arc::new);
        self.magic.effect_cache.borrow_mut().insert(id, m.clone());
        m
    }

    pub fn magic_item(&self, id: FormId) -> Option<Arc<MagicItem>> {
        if let Some(m) = self.magic.item_cache.borrow().get(&id) {
            return m.clone();
        }
        let m = MagicItem::load(self, id).map(Arc::new);
        self.magic.item_cache.borrow_mut().insert(id, m.clone());
        m
    }

    /// The spells an actor knows: its record's (`SPLO`, from the template
    /// giving the spell list; leveled spell lists rolled once, for the actor's
    /// level then), its race's, its perks' abilities and those added,
    /// less those removed.
    pub fn actor_spells(&self, actor: FormId) -> Vec<FormId> {
        let listed = self
            .templates_of(actor)
            .map(|t| t.spells(&self.lo))
            .unwrap_or_default();
        let mut out = Vec::with_capacity(listed.len());
        let mut level = None;
        for s in listed {
            if self.lo.tag_of(s).is_some_and(|t| t.0 == *b"LVSP") {
                out.extend(self.magic.rolled.get_or_roll(actor, s, || {
                    let level = *level.get_or_insert_with(|| self.actor_level(actor).max(1) as u16);
                    crate::world::actor::resolve_spells(
                        &self.lo,
                        s,
                        level,
                        actor.0 as u64,
                        &self.scripts.globals,
                        0,
                    )
                }));
            } else {
                out.push(s);
            }
        }
        // Temporary effect perks supply entry points, not ability sections:
        // otherwise an ability could keep its own granting effect alive.
        // Unknown vanilla lifecycle cases: known_ambiguities/effect-perks.md.
        for &(perk, _) in self.permanent_perks(actor).iter() {
            if let Some(p) = self.perk(perk) {
                for en in &p.entries {
                    if let crate::perks::Section::Ability(s) = en.section
                        && !s.is_null()
                    {
                        out.push(s);
                    }
                }
            }
        }
        out.extend(self.magic.added.get(&actor).into_iter().flatten().copied());
        if let Some(r) = self.magic.removed.get(&actor) {
            out.retain(|s| !r.contains(s));
        }
        let mut seen = HashSet::new();
        out.retain(|s| seen.insert(*s));
        out
    }

    pub fn has_spell(&self, actor: FormId, spell: FormId) -> bool {
        self.actor_spells(actor).contains(&spell)
    }

    /// `AddSpell`: abilities take effect at once.
    pub fn add_spell(&mut self, actor: FormId, spell: FormId) -> bool {
        let Some(item) = self.magic_item(spell) else {
            return false;
        };
        if self.has_spell(actor, spell) {
            return false;
        }
        if let Some(r) = self.magic.removed.get_mut(&actor) {
            r.remove(&spell);
        }
        if !self.has_spell(actor, spell) {
            self.magic.added.entry(actor).or_default().push(spell);
        }
        log::info!("{actor} learns {} ({spell})", item.name);
        if actor == crate::engine::PLAYER_REF && item.spell_type == spell_type::SPELL {
            self.increment_stat("Spells Learned", 1);
        }
        if item.is_constant() {
            self.apply_item(spell, Some(actor), actor);
        }
        true
    }

    /// `RemoveSpell`: its effects on the actor end.
    pub fn remove_spell(&mut self, actor: FormId, spell: FormId) -> bool {
        let had = self.has_spell(actor, spell);
        if let Some(v) = self.magic.added.get_mut(&actor) {
            v.retain(|&s| s != spell);
        }
        if self.has_spell(actor, spell) {
            self.magic.removed.entry(actor).or_default().insert(spell);
        }
        self.dispel(actor, |x| x.item == spell);
        had
    }

    /// An explicit script/console cast, separate from effects landing through
    /// projectiles, areas, concentration ticks or passive abilities.
    pub fn cast_item(&mut self, item: FormId, caster: Option<FormId>, target: FormId) -> bool {
        let Some(magic) = self.magic_item(item) else {
            return false;
        };
        if let Some(caster) = caster {
            self.casting_breaks_invisibility(caster, &magic);
        }
        let landed = self.apply_item(item, caster, target);
        if landed && let Some(caster) = caster {
            self.count_spell_cast(caster, item);
        }
        landed
    }

    /// Apply an item's effects to `target` (no aiming or projectile: it lands).
    /// The same item's effects already on the target are replaced.
    pub fn apply_item(&mut self, item: FormId, caster: Option<FormId>, target: FormId) -> bool {
        self.apply_item_as(item, caster, target, false, None, false, ALL_EFFECTS)
    }

    /// Apply a spell cast by `caster` as `apply_item`, landing at `at` (a
    /// target location spell: where a summon appears); `dual`: dual cast,
    /// its effects stronger or longer by their power.
    pub fn apply_cast(
        &mut self,
        item: FormId,
        caster: FormId,
        target: FormId,
        at: Option<glam::Vec3>,
        dual: bool,
    ) -> bool {
        self.apply_item_as(item, Some(caster), target, false, at, dual, ALL_EFFECTS)
    }

    /// Apply some of a spell's effects (bit `i` of `effects` for its `i`th)
    /// as `apply_cast`: those whose area reaches an actor other than the one
    /// struck. The same effects of it already on the target are replaced.
    pub fn apply_cast_effects(
        &mut self,
        item: FormId,
        caster: FormId,
        target: FormId,
        dual: bool,
        effects: u64,
    ) -> bool {
        self.apply_item_as(item, Some(caster), target, false, None, dual, effects)
    }

    /// Apply an item's effects as `apply_item`, but unnoticed: hostile effects
    /// landing aren't a blow that starts a fight (a poison slipped into
    /// someone's pockets).
    pub fn apply_item_quietly(&mut self, item: FormId, caster: FormId, target: FormId) -> bool {
        self.apply_item_as(item, Some(caster), target, true, None, false, ALL_EFFECTS)
    }

    fn apply_item_as(
        &mut self,
        item: FormId,
        caster: Option<FormId>,
        target: FormId,
        quiet: bool,
        at: Option<glam::Vec3>,
        dual: bool,
        effects: u64,
    ) -> bool {
        let Some(m) = self.magic_item(item) else {
            return false;
        };
        let applies = |index: usize| index >= 64 || effects & (1 << index) != 0;
        // Dual cast: `fMagicDualCastingEffectivenessBase` whatever the
        // effect's dual casting scale (`DATA` 0x70), on its duration if it
        // says so, else its magnitude (`test-mod dualcast`).
        let power = match dual {
            false => 1.0,
            true if m.flags & NO_DUAL_CAST_MODS != 0 => 2.0,
            true => {
                crate::ai::combat::gmst_f32(&self.lo, "fMagicDualCastingEffectivenessBase", 2.2)
            }
        };
        if self.is_dead(target) && target != PLAYER_REF {
            return false;
        }
        // A ward in the way stops it; a hostile spell stopped is still a blow.
        if !quiet
            && let Some(c) = caster.filter(|&c| c != target)
            && self.ward_stops(target, c, &m, power)
        {
            if self.item_hostile(&m) {
                self.hostile_magic_landed(target, c, item);
            }
            return false;
        }
        // A concentration spell landing again keeps its channelled effects
        // on (cast the same way); the rest are replaced.
        let mut kept: u64 = 0;
        for x in self.magic.effects.iter_mut().filter(|x| {
            x.target == target
                && !x.done
                && x.channelled
                && x.item == item
                && x.caster == caster
                && x.dual == dual
                && x.index < 64
                && applies(x.index)
        }) {
            x.elapsed = 0.0;
            kept |= 1 << x.index;
        }
        let applies = |index: usize| applies(index) && (index >= 64 || kept & (1 << index) == 0);
        self.dispel(target, |x| x.item == item && applies(x.index));
        // One cloak at a time: a new one ends the last (UESP).
        let cloak = m.effects.iter().enumerate().any(|(i, e)| {
            applies(i)
                && self
                    .magic_effect(e.effect)
                    .is_some_and(|g| g.archetype == archetype::CLOAK)
        });
        if cloak {
            self.dispel(target, |x| x.effect_archetype == archetype::CLOAK);
        }
        let constant = m.is_constant();
        let mut any = false;
        let mut hostile = false;
        for (index, ei) in m.effects.iter().enumerate() {
            if !applies(index) {
                continue;
            }
            let Some(mgef) = self.magic_effect(ei.effect) else {
                continue;
            };
            let ctx = Context {
                subject: Some(target),
                target: caster,
                dual_cast: dual,
                ..Default::default()
            };
            // Fire-and-forget effects whose conditions fail don't land; constant
            // and lasting ones wait for them.
            let passes = condition::evaluate(self, &ei.conditions, ctx)
                && condition::evaluate(self, &mgef.conditions, ctx);
            if !passes && !constant && ei.duration == 0 {
                continue;
            }
            let mut base = ei.magnitude;
            if !mgef.has(flags::POWER_AFFECTS_DURATION) {
                base *= power;
            }
            let magnitude = self.effect_magnitude(&m, &mgef, base, caster, target);
            // Calm, fear, frenzy and disarm reach only those up to their magnitude's
            // level; one too strong for it is still struck by a hostile spell.
            if archetype::level_capped(mgef.archetype)
                && self.actor_level(target) as f32 > magnitude
            {
                log::debug!(
                    "{target}: {} from {} reaches level {magnitude:.0}, not {}",
                    mgef.name,
                    m.name,
                    self.actor_level(target)
                );
                hostile |= mgef.has(flags::HOSTILE) && passes;
                continue;
            }
            let channelled = m.cast_type == crate::spells::CONCENTRATION && ei.duration == 0;
            let duration = if constant {
                f32::INFINITY
            } else if channelled {
                CHANNEL_LINGER
            } else if mgef.has(flags::NO_DURATION) {
                0.0
            } else if mgef.has(flags::POWER_AFFECTS_DURATION) {
                ei.duration as f32 * power
            } else {
                ei.duration as f32
            };
            self.magic.next_id += 1;
            let id = self.magic.next_id;
            self.magic.effects.push(ActiveEffect {
                id,
                target,
                caster,
                item,
                index,
                effect: ei.effect,
                magnitude,
                duration,
                elapsed: 0.0,
                active: false,
                held: [0.0; 2],
                scripted: !mgef.scripts.is_empty(),
                done: false,
                quiet,
                at,
                summoned: None,
                dual,
                channelled,
            });
            log::debug!(
                "{target}: {} from {} ({item}): magnitude {magnitude:.1}, {duration}s",
                mgef.name,
                m.name
            );
            any = true;
            hostile |= mgef.has(flags::HOSTILE) && passes;
            if passes {
                self.start_magic_effect(id);
            }
            // `OnMagicEffectApply` (caster, effect) to the target.
            let args = vec![
                caster.map_or(Value::None, |c| self.object_value(c)),
                self.object_value(ei.effect),
            ];
            self.send_script_event(target, "OnMagicEffectApply", args);
        }
        if hostile
            && !quiet
            && let Some(c) = caster.filter(|&c| c != target)
        {
            self.hostile_magic_landed(target, c, item);
        }
        any
    }

    /// A hostile spell landed: `OnHit` (the spell as its source, its
    /// projectile: Firebolt's `FireboltProjectile01` in the game, `test-mod
    /// autotest2`) to the target, and like a first blow it starts a fight
    /// (and an assault).
    pub(crate) fn hostile_magic_landed(&mut self, target: FormId, caster: FormId, item: FormId) {
        self.break_calm(target);
        let projectile = self.magic_item(item).and_then(|m| self.item_projectile(&m));
        let proj_value = projectile.map_or(Value::None, |p| self.object_value(p));
        let args = vec![
            self.object_value(caster),
            self.object_value(item),
            proj_value,
            Value::Bool(false),
            Value::Bool(false),
            Value::Bool(false),
            Value::Bool(false),
        ];
        // A projectile with an explosion strikes twice: Firebolt's (and a
        // spell of ours with only its fire damage) sent two alike 0.1 s apart
        // in the game (`test-mod autotest4`), its projectile exploding
        // (`PROJ` `DATA` 0x24); the second taken as the explosion's.
        if projectile.is_some_and(|p| projectile_explodes(&self.lo, p)) {
            self.send_script_event(target, "OnHit", args.clone());
        }
        self.send_script_event(target, "OnHit", args);
        if target == PLAYER_REF {
            self.assault_on_player(caster);
            return;
        }
        let first = self.actor_ref(target).filter(|a| !a.dead).map(|a| {
            (
                a.exec.combat.as_ref().is_none_or(|c| c.target != caster),
                a.exec.combat.is_none(),
                a.stats.factions.clone(),
            )
        });
        if let Some((true, calm, factions)) = first {
            // The frenzied pick their own fights; it is an assault all the same.
            if !self.is_frenzied(target) {
                self.start_combat(target, caster);
            }
            let crime = calm && self.law_abiding(&factions);
            self.send_assault(target, caster, crime);
        }
    }

    /// The magnitude an effect lands with: the caster's spell perks (Mod Spell
    /// Magnitude), then for hostile effects the target's perks (Mod Incoming
    /// Spell Magnitude), magic resistance and the effect's resistance.
    fn effect_magnitude(
        &self,
        item: &MagicItem,
        mgef: &MagicEffect,
        base: f32,
        caster: Option<FormId>,
        target: FormId,
    ) -> f32 {
        use crate::perks::ep;
        let mut m = base;
        if item.is_constant() {
            return m;
        }
        if let Some(c) = caster
            && item.spell_type != spell_type::POTION
        {
            m = self.perk_entry_point(
                ep::MOD_SPELL_MAGNITUDE,
                c,
                &[Some(item.id), Some(target)],
                m,
            );
        }
        if mgef.has(flags::HOSTILE) || mgef.has(flags::DETRIMENTAL) {
            m = self.perk_entry_point(
                ep::MOD_INCOMING_SPELL_MAGNITUDE,
                target,
                &[Some(item.id)],
                m,
            );
            // Spells that ignore resistance (`SPIT` flag 0x100000) skip it.
            if item.flags & 0x10_0000 == 0 {
                let cap = if target == PLAYER_REF {
                    crate::ai::combat::gmst_f32(&self.lo, "fPlayerMaxResistance", 85.0)
                } else {
                    100.0
                };
                let resist =
                    |a: u32| (self.actor_value(target, a).min(cap) / 100.0).clamp(-10.0, 1.0);
                if mgef.has(flags::HOSTILE) {
                    m *= 1.0 - resist(av::MAGIC_RESIST);
                }
                if let Some(r) = mgef.resist {
                    m *= 1.0 - resist(r);
                }
            }
        }
        m
    }

    pub(crate) fn effect_index(&self, id: u64) -> Option<usize> {
        self.magic.effects.iter().position(|x| x.id == id)
    }

    /// An effect begins its work: what it holds goes on, what it does at once
    /// happens, its scripts get `OnEffectStart`.
    fn start_magic_effect(&mut self, id: u64) {
        let Some(i) = self.effect_index(id) else {
            return;
        };
        let x = self.magic.effects[i].clone();
        let Some(mgef) = self.magic_effect(x.effect) else {
            return;
        };
        self.magic.effects[i].active = true;
        let sign = if mgef.has(flags::DETRIMENTAL) {
            -1.0
        } else {
            1.0
        };
        let holds = x.duration.is_infinite() || mgef.has(flags::RECOVER);
        match mgef.archetype {
            archetype::VALUE_MODIFIER | archetype::DUAL_VALUE_MODIFIER | archetype::ABSORB
                if holds && mgef.archetype != archetype::ABSORB =>
            {
                let second = if mgef.archetype == archetype::DUAL_VALUE_MODIFIER {
                    mgef.second_weight
                } else {
                    0.0
                };
                self.hold(i, &mgef, [sign * x.magnitude, sign * x.magnitude * second]);
            }
            archetype::VALUE_MODIFIER | archetype::DUAL_VALUE_MODIFIER | archetype::ABSORB
                if x.duration == 0.0 =>
            {
                self.change_value(&x, &mgef, sign * x.magnitude, true);
                let [weight, _, seconds] = mgef.taper;
                if weight > 0.0 && seconds > 0.0 {
                    let rate = sign * x.magnitude * weight;
                    self.magic.tapers.push((x.clone(), mgef.clone(), rate, 0.0));
                }
            }
            archetype::PEAK_VALUE_MODIFIER => self.recompute_peaks(x.target, &mgef),
            // A ward builds from nothing, or is whole at once dual cast.
            archetype::ACCUMULATE_MAGNITUDE => {
                let from = if x.dual {
                    self.ward_most(x.magnitude)
                } else {
                    0.0
                };
                self.hold(i, &mgef, [from, 0.0]);
            }
            a if archetype::target_value(a).is_some() && mgef.primary.is_some() => {
                let to = archetype::target_value(a).unwrap_or_default();
                let now = self.actor_value(x.target, mgef.primary.unwrap_or_default());
                self.hold(i, &mgef, [to - now, 0.0]);
                self.target_value_started(x.target, a);
            }
            archetype::INVISIBILITY => self.hold(i, &mgef, [1.0, 0.0]),
            archetype::SUMMON_CREATURE => self.summon(id, &mgef),
            // A bound weapon: in hand while it lasts.
            archetype::BOUND_WEAPON if !mgef.related.is_null() => {
                self.add_item(x.target, mgef.related, 1);
                if let Err(e) = self.equip_item(x.target, mgef.related, true) {
                    log::debug!("{}: {} not equipped: {e}", x.target, mgef.related);
                }
            }
            archetype::PARALYSIS => self.hold(i, &mgef, [1.0, 0.0]),
            archetype::TELEKINESIS if x.target == PLAYER_REF && x.caster == Some(PLAYER_REF) => {
                self.telekinesis_started(id, x.magnitude);
            }
            archetype::CURE_DISEASE => self.cure(x.target, |m| m.spell_type == spell_type::DISEASE),
            archetype::CURE_POISON => self.cure(x.target, |m| m.spell_type == spell_type::POISON),
            archetype::CURE_PARALYSIS => self.dispel(x.target, |e| {
                e.id != id && e.effect_archetype == archetype::PARALYSIS
            }),
            archetype::DISARM => {
                self.disarm(x.target, x.caster);
            }
            archetype::STAGGER => {
                if x.target != PLAYER_REF
                    && !self.is_ethereal(x.target)
                    && let Some(g) = self.actor_mut(x.target).and_then(|a| a.graph.as_mut())
                {
                    g.set_variable("staggerMagnitude", x.magnitude.clamp(0.0, 1.0));
                    g.send_event("staggerStart");
                }
            }
            _ => {}
        }
        if x.scripted {
            self.attach_effect_scripts(&x, &mgef);
        }
    }

    /// Hold a change on the effect's actor values (primary, secondary).
    pub(crate) fn hold(&mut self, i: usize, mgef: &MagicEffect, by: [f32; 2]) {
        let target = self.magic.effects[i].target;
        let avs = [
            mgef.primary.or(match mgef.archetype {
                archetype::INVISIBILITY => Some(av::INVISIBILITY),
                archetype::PARALYSIS => Some(av::PARALYSIS),
                _ => None,
            }),
            mgef.secondary,
        ];
        for k in 0..2 {
            let Some(a) = avs[k] else { continue };
            let delta = by[k] - self.magic.effects[i].held[k];
            if delta != 0.0 {
                self.magic.effects[i].held[k] = by[k];
                self.mod_temporary_av(target, a, delta);
            }
        }
    }

    /// Give back what an effect holds.
    fn release(&mut self, i: usize, mgef: &MagicEffect) {
        self.hold(i, mgef, [0.0; 2]);
    }

    /// Peak value modifiers sharing a keyword and actor value don't add up:
    /// only the strongest active one holds its magnitude.
    fn recompute_peaks(&mut self, target: FormId, of: &MagicEffect) {
        let group: Vec<(usize, f32, Arc<MagicEffect>)> = self
            .magic
            .effects
            .iter()
            .enumerate()
            .filter(|(_, x)| x.target == target && !x.done)
            .filter_map(|(i, x)| Some((i, x, self.magic_effect(x.effect)?)))
            .filter(|(_, _, m)| {
                m.archetype == archetype::PEAK_VALUE_MODIFIER
                    && m.related == of.related
                    && m.primary == of.primary
            })
            .map(|(i, x, m)| {
                let sign = if m.has(flags::DETRIMENTAL) { -1.0 } else { 1.0 };
                (i, if x.active { sign * x.magnitude } else { 0.0 }, m)
            })
            .collect();
        let best = group
            .iter()
            .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
            .map(|g| g.0);
        for (i, mag, m) in group {
            let keep = if Some(i) == best { mag } else { 0.0 };
            self.hold(i, &m, [keep, 0.0]);
        }
    }

    /// Change an actor value by `amount` now (damage when negative): health
    /// through the combat damage path (a kill is the caster's, unless it
    /// landed quietly), absorbing gives the caster what the target lost.
    /// Tapers run down: after an instant value change, or a lasting one's
    /// end, the value goes on changing at `taper weight x magnitude` a
    /// second, falling as `(1 - t / duration) ^ curve` to nothing at the
    /// taper duration: `magnitude x weight x duration / (curve + 1)` in all,
    /// a little less as it is applied each frame at the frame's end rate.
    /// In the game (`test-mod taper`) 50 points of fire with (0.3, 2, 1 s)
    /// took 4.875 more, with 2 s 9.875 and 4 s 19.875; poison with (0.2, 2,
    /// 1 s) 3.25; Storm Call's bolt (1, 1, 1 s) 24.583; a heal (1, 0, 0.3 s)
    /// 15: those sums at 60 frames a second. Frost has none.
    fn update_tapers(&mut self, dt: f32) {
        if self.magic.tapers.is_empty() || dt <= 0.0 {
            return;
        }
        let tapers = std::mem::take(&mut self.magic.tapers);
        let mut keep = Vec::with_capacity(tapers.len());
        for (x, mgef, rate, elapsed) in tapers {
            let [_, curve, seconds] = mgef.taper;
            if self.is_dead(x.target)
                || (x.target != PLAYER_REF && !self.actor_cells.contains_key(&x.target))
            {
                continue;
            }
            let end = (elapsed + dt).min(seconds);
            let amount = taper_step(rate, curve, seconds, elapsed, end);
            self.change_value(&x, &mgef, amount, false);
            if end < seconds {
                keep.push((x, mgef, rate, end));
            }
        }
        keep.append(&mut self.magic.tapers);
        self.magic.tapers = keep;
    }

    fn change_value(&mut self, x: &ActiveEffect, mgef: &MagicEffect, amount: f32, instant: bool) {
        let Some(a) = mgef.primary else { return };
        let by = x.caster.filter(|_| !x.quiet);
        if mgef.archetype == archetype::ABSORB {
            let taken = amount.abs().min(self.actor_value(x.target, a).max(0.0));
            self.value_change(x.target, a, -taken, by, instant);
            if let Some(c) = x.caster {
                self.value_change(c, a, taken, None, false);
            }
            return;
        }
        self.value_change(x.target, a, amount, by, instant);
        if let Some(b) = mgef.secondary
            && mgef.archetype == archetype::DUAL_VALUE_MODIFIER
        {
            self.value_change(x.target, b, amount * mgef.second_weight, by, instant);
        }
    }

    fn value_change(
        &mut self,
        target: FormId,
        a: u32,
        amount: f32,
        by: Option<FormId>,
        instant: bool,
    ) {
        if amount == 0.0 {
            return;
        }
        if a == av::HEALTH && amount < 0.0 {
            if self.is_ethereal(target) {
                log::debug!("{target} is ethereal: no harm from magic");
                return;
            }
            // A blow at once, or enough to kill, goes through the damage path
            // (a flinch, death, bleeding out); a little at a time just wears
            // health down.
            if instant
                || self.actor_value(target, av::HEALTH) + amount <= 0.0
                || target == PLAYER_REF
            {
                self.damage(target, -amount, by, 0.0);
            } else {
                self.damage_actor_value(target, a, -amount);
            }
        } else {
            self.damage_actor_value(target, a, -amount);
        }
    }

    /// Effects from items matching `pick` on `target` end (curing diseases,
    /// poisons).
    fn cure(&mut self, target: FormId, pick: impl Fn(&MagicItem) -> bool) {
        let items: HashSet<FormId> = self
            .magic
            .effects
            .iter()
            .filter(|x| x.target == target)
            .filter(|x| self.magic_item(x.item).is_some_and(|m| pick(&m)))
            .map(|x| x.item)
            .collect();
        for item in items {
            if self
                .magic_item(item)
                .is_some_and(|m| m.spell_type == spell_type::DISEASE)
            {
                self.remove_spell(target, item);
            } else {
                self.dispel(target, |x| x.item == item);
            }
        }
    }

    /// End effects on `target` matching `pick` (`DispelSpell`, `RemoveSpell`).
    pub fn dispel(&mut self, target: FormId, pick: impl Fn(&EffectView) -> bool) {
        let ids: Vec<u64> = self
            .magic
            .effects
            .iter()
            .filter(|x| x.target == target && !x.done)
            .filter(|x| {
                pick(&EffectView {
                    id: x.id,
                    item: x.item,
                    index: x.index,
                    effect_archetype: self
                        .magic_effect(x.effect)
                        .map_or(u32::MAX, |m| m.archetype),
                })
            })
            .map(|x| x.id)
            .collect();
        for id in ids {
            self.finish_magic_effect(id);
        }
    }

    /// An effect stops working (its conditions failed, or it is ending): what
    /// it holds is given back, its scripts get `OnEffectFinish`.
    fn stop_magic_effect(&mut self, id: u64) {
        let Some(i) = self.effect_index(id) else {
            return;
        };
        if !self.magic.effects[i].active {
            return;
        }
        self.magic.effects[i].active = false;
        let x = self.magic.effects[i].clone();
        let Some(mgef) = self.magic_effect(x.effect) else {
            return;
        };
        if mgef.archetype == archetype::PEAK_VALUE_MODIFIER {
            self.release(i, &mgef);
            self.recompute_peaks(x.target, &mgef);
        } else {
            self.release(i, &mgef);
        }
        if archetype::target_value(mgef.archetype).is_some() {
            self.target_value_stopped(x.target, mgef.archetype);
        }
        if mgef.archetype == archetype::TELEKINESIS {
            self.telekinesis_stopped(id);
        }
        if mgef.archetype == archetype::BOUND_WEAPON && !mgef.related.is_null() {
            let _ = self.equip_item(x.target, mgef.related, false);
            let n = self.item_count(x.target, mgef.related);
            self.remove_item(x.target, mgef.related, n, None);
        }
        if let Some(r) = x.summoned {
            self.magic.effects[i].summoned = None;
            self.unsummon(r);
        }
        if x.scripted {
            let obj = ObjectId::Effect(x.id);
            let args = vec![
                self.object_value(x.target),
                x.caster.map_or(Value::None, |c| self.object_value(c)),
            ];
            self.scripts
                .pending_events
                .push((obj, "OnEffectFinish".into(), args));
        }
    }

    /// An effect ends for good.
    fn finish_magic_effect(&mut self, id: u64) {
        self.stop_magic_effect(id);
        if let Some(i) = self.effect_index(id) {
            self.magic.effects[i].done = true;
            if self.magic.effects[i].scripted {
                self.magic
                    .linger
                    .push((id, self.scripts.real_time + SCRIPT_LINGER));
            }
        }
    }

    fn attach_effect_scripts(&mut self, x: &ActiveEffect, mgef: &MagicEffect) {
        let obj = ObjectId::Effect(x.id);
        let mut vm = std::mem::take(&mut self.vm);
        {
            let mut host = crate::script::EngineHost { engine: self };
            if vm.attached_scripts(obj).is_empty() {
                for s in &mgef.scripts {
                    let props: Vec<(String, Value)> = s
                        .properties
                        .iter()
                        .map(|(n, pv)| {
                            (
                                n.clone(),
                                crate::script::vmad::to_value(pv, &|f| host.engine.native_class(f)),
                            )
                        })
                        .collect();
                    vm.attach(&mut host, obj, &s.name, &props);
                }
                if host.engine.scripts.initialized.insert(obj) {
                    vm.send_event(&mut host, obj, "OnInit", vec![]);
                }
            }
            let args = vec![
                host.engine.object_value(x.target),
                x.caster
                    .map_or(Value::None, |c| host.engine.object_value(c)),
            ];
            vm.send_event(&mut host, obj, "OnEffectStart", args);
        }
        self.vm = vm;
    }

    /// Run active effects: per-second changes, durations running out,
    /// conditions and abilities looked at again now and then, effects on the
    /// dead (but those that outlast death) and the unloaded ended.
    pub(crate) fn update_magic(&mut self, dt: f32) {
        self.magic.sync -= dt;
        let sync = self.magic.sync <= 0.0;
        if sync {
            self.magic.sync = SYNC_INTERVAL;
            self.sync_abilities();
        }
        let ids: Vec<u64> = self
            .magic
            .effects
            .iter()
            .filter(|x| !x.done)
            .map(|x| x.id)
            .collect();
        for id in ids {
            let Some(i) = self.effect_index(id) else {
                continue;
            };
            let x = self.magic.effects[i].clone();
            let Some(mgef) = self.magic_effect(x.effect) else {
                self.finish_magic_effect(id);
                continue;
            };
            let gone = x.target != PLAYER_REF && !self.actor_cells.contains_key(&x.target);
            let dead = self.is_dead(x.target) && !mgef.has(flags::NO_DEATH_DISPEL);
            // A summon ends with its actor (killed, or left behind unloaded).
            let summon_gone = x
                .summoned
                .is_some_and(|r| self.is_dead(r) || !self.actor_cells.contains_key(&r));
            if summon_gone {
                log::debug!("{id}: its summon is dead or gone");
            }
            // Dying under a soul trap gives up the soul.
            if dead
                && mgef.archetype == archetype::SOUL_TRAP
                && let Some(caster) = x.caster
            {
                self.trap_soul(caster, x.target);
            }
            if gone || dead || summon_gone {
                self.finish_magic_effect(id);
                continue;
            }
            // A bound weapon put away (another taken up) leaves the effect
            // on: in the game it was still on 5 s after an iron sword took
            // the bound sword's hand (`test-mod autotest4`).
            if sync && (x.duration > 0.0) {
                let pass = self.effect_conditions_pass(&x, &mgef);
                if pass && !x.active {
                    self.start_magic_effect(id);
                } else if !pass && x.active {
                    self.stop_magic_effect(id);
                }
            }
            let Some(i) = self.effect_index(id) else {
                continue;
            };
            // The player's channelled effects last while a hand casts the spell.
            if x.channelled && x.caster == Some(PLAYER_REF) {
                if self.player_holds_spell(x.item) {
                    self.magic.effects[i].elapsed = 0.0;
                } else {
                    self.finish_magic_effect(id);
                    continue;
                }
            }
            let x = &mut self.magic.effects[i];
            let step = dt.min((x.duration - x.elapsed).max(0.0));
            x.elapsed += dt;
            let ends = x.elapsed >= x.duration;
            // Lasting value changes without Recover: magnitude per second.
            let per_second = x.active
                && x.duration.is_finite()
                && x.duration > 0.0
                && !mgef.has(flags::RECOVER)
                && matches!(
                    mgef.archetype,
                    archetype::VALUE_MODIFIER | archetype::DUAL_VALUE_MODIFIER | archetype::ABSORB
                );
            let mut apply = None;
            let mut taper = None;
            // A cloak touches those near once a second, from the start.
            let cloak_tick = x.active
                && mgef.archetype == archetype::CLOAK
                && (x.elapsed <= dt
                    || (x.elapsed / CLOAK_TICK).floor() > ((x.elapsed - dt) / CLOAK_TICK).floor());
            let cloak = cloak_tick.then(|| x.clone());
            if per_second {
                let sign = if mgef.has(flags::DETRIMENTAL) {
                    -1.0
                } else {
                    1.0
                };
                // Every frame, fractions too (the game's fire over time
                // wore health down 3.0 a quarter second at 10 a second:
                // `test-mod taper`).
                apply = Some((x.clone(), sign * x.magnitude * step));
                // Its taper follows it (it went on after: about the 1 that
                // 10 a second's makes).
                let [weight, _, seconds] = mgef.taper;
                if ends && weight > 0.0 && seconds > 0.0 {
                    taper = Some((x.clone(), sign * x.magnitude * weight));
                }
            }
            if let Some((snapshot, amount)) = apply {
                self.change_value(&snapshot, &mgef, amount, false);
            }
            if let Some(snapshot) = cloak {
                self.cloak_touch(&snapshot, &mgef);
            }
            if let Some((snapshot, rate)) = taper {
                self.magic.tapers.push((snapshot, mgef.clone(), rate, 0.0));
            }
            if ends {
                self.finish_magic_effect(id);
            }
        }
        self.magic.effects.retain(|x| !x.done);
        self.update_wards(dt);
        self.update_tapers(dt);
        // Let finished effects' scripts go once they have had their time.
        let now = self.scripts.real_time;
        let (old, keep): (Vec<_>, Vec<_>) =
            self.magic.linger.drain(..).partition(|&(_, t)| t <= now);
        self.magic.linger = keep;
        for (id, _) in old {
            self.vm.detach_all(ObjectId::Effect(id));
        }
    }

    fn effect_conditions_pass(&self, x: &ActiveEffect, mgef: &MagicEffect) -> bool {
        let ctx = Context {
            subject: Some(x.target),
            target: x.caster,
            dual_cast: x.dual,
            ..Default::default()
        };
        let item_conds = self
            .magic_item(x.item)
            .and_then(|m| m.effects.get(x.index).map(|e| e.conditions.clone()))
            .unwrap_or_default();
        condition::evaluate(self, &item_conds, ctx)
            && condition::evaluate(self, &mgef.conditions, ctx)
    }

    /// The player and loaded actors have their abilities' effects on, and
    /// none of those they no longer have.
    fn sync_abilities(&mut self) {
        let mut actors: Vec<FormId> = vec![PLAYER_REF];
        actors.extend(
            self.cells
                .values()
                .flat_map(|rt| rt.actors.iter())
                .filter(|a| !a.dead)
                .map(|a| a.ref_id),
        );
        for actor in actors {
            let mut wanted: Vec<FormId> = self
                .actor_spells(actor)
                .into_iter()
                .filter(|&s| self.magic_item(s).is_some_and(|m| m.is_constant()))
                .collect();
            // Worn things' constant enchantments (rings, armor).
            wanted.extend(
                self.worn_enchantments(actor)
                    .into_iter()
                    .filter(|&s| self.magic_item(s).is_some_and(|m| m.is_constant())),
            );
            let on: HashSet<FormId> = self
                .magic
                .effects
                .iter()
                .filter(|x| x.target == actor && x.duration.is_infinite() && !x.done)
                .map(|x| x.item)
                .collect();
            for s in &wanted {
                if !on.contains(s) {
                    self.apply_item(*s, Some(actor), actor);
                }
            }
            for s in on {
                if !wanted.contains(&s) {
                    self.dispel(actor, |x| x.item == s);
                }
            }
        }
    }

    /// The enchantments (`EITM`) of what an actor wears and wields.
    pub fn worn_enchantments(&self, actor: FormId) -> Vec<FormId> {
        let Some(inv) = self.inventories.get(&actor) else {
            return Vec::new();
        };
        inv.equipped()
            .iter()
            .filter_map(|&f| self.item_enchantment(f))
            .collect()
    }

    /// An item's enchantment (`EITM`).
    pub fn item_enchantment(&self, item: FormId) -> Option<FormId> {
        let rec = self.lo.get(item)?;
        let d = rec.get(b"EITM").filter(|d| d.len() >= 4)?;
        Some(rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap()))))
            .filter(|f| !f.is_null())
    }

    /// The active effects' script objects on an actor (they hear its events).
    pub(crate) fn effect_objects(&self, target: FormId) -> impl Iterator<Item = ObjectId> + '_ {
        self.magic
            .effects
            .iter()
            .filter(move |x| x.target == target && x.scripted && x.active && !x.done)
            .map(|x| ObjectId::Effect(x.id))
    }

    pub fn active_effect(&self, id: u64) -> Option<&ActiveEffect> {
        self.magic.effects.iter().find(|x| x.id == id)
    }

    /// How fast the world runs: Slow Time's magnitude is the share of its
    /// speed (0.3, 0.2, 0.1 for the shout's words), the slowest of any
    /// lasting taking it. The player, and the effects themselves, keep real
    /// time (`known_ambiguities/magic.md`).
    pub fn world_speed(&self) -> f32 {
        slow_time_speed(
            self.magic
                .effects
                .iter()
                .filter(|x| x.active && !x.done)
                .filter(|x| {
                    self.magic_effect(x.effect)
                        .is_some_and(|m| m.archetype == archetype::SLOW_TIME)
                })
                .map(|x| x.magnitude),
        )
    }

    /// `HasMagicEffect`.
    pub fn has_magic_effect(&self, actor: FormId, effect: FormId) -> bool {
        self.magic
            .effects
            .iter()
            .any(|x| x.target == actor && x.active && !x.done && x.effect == effect)
    }

    /// A disease's effect is on the actor.
    pub fn has_disease(&self, actor: FormId) -> bool {
        self.magic.effects.iter().any(|x| {
            x.target == actor
                && x.active
                && !x.done
                && self
                    .magic_item(x.item)
                    .is_some_and(|m| m.spell_type == spell_type::DISEASE)
        })
    }

    /// `IsSpellTarget`: an effect of `item` is on the actor.
    pub fn is_spell_target(&self, actor: FormId, item: FormId) -> bool {
        self.magic
            .effects
            .iter()
            .any(|x| x.target == actor && !x.done && x.item == item)
    }

    /// `SpellHasKeyword`: the spell's own keywords, or its effects'. (The
    /// flames spells carry none of their own; their effects carry
    /// `MagicDamageFire`, which the vanilla dialogue checks for.)
    pub fn spell_has_keyword(&self, spell: FormId, kw: FormId) -> bool {
        self.magic_item(spell).is_some_and(|m| {
            m.keywords.contains(&kw)
                || m.effects.iter().any(|x| {
                    self.magic_effect(x.effect)
                        .is_some_and(|f| f.keywords.contains(&kw))
                })
        })
    }

    /// `HasMagicEffectWithKeyword`.
    pub fn has_magic_effect_keyword(&self, actor: FormId, kw: FormId) -> bool {
        self.magic
            .effects
            .iter()
            .filter(|x| x.target == actor && x.active && !x.done)
            .any(|x| {
                self.magic_effect(x.effect)
                    .is_some_and(|m| m.keywords.contains(&kw))
            })
    }

    /// `ActiveMagicEffect.Dispel`.
    /// A cloak on `x.target` gives its related spell to everyone (the
    /// player too) within its magnitude in feet whom its conditions pass,
    /// run on them (Flame Cloak's: in combat, not the player).
    fn cloak_touch(&mut self, x: &ActiveEffect, mgef: &MagicEffect) {
        let wearer = x.target;
        let Some(at) = self.ref_position(wearer) else {
            return;
        };
        let reach = x.magnitude * crate::casting::UNITS_PER_FOOT;
        let mut near: Vec<FormId> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| !a.dead && a.ref_id != wearer && a.exec.pos.distance(at) <= reach)
            .map(|a| a.ref_id)
            .collect();
        if wearer != PLAYER_REF && !self.player_dead() && self.player.position.distance(at) <= reach
        {
            near.push(PLAYER_REF);
        }
        let item_conds = self
            .magic_item(x.item)
            .and_then(|m| m.effects.get(x.index).map(|e| e.conditions.clone()))
            .unwrap_or_default();
        for who in near {
            let ctx = Context {
                subject: Some(who),
                target: Some(wearer),
                ..Default::default()
            };
            if condition::evaluate(self, &item_conds, ctx)
                && condition::evaluate(self, &mgef.conditions, ctx)
            {
                self.apply_item(mgef.related, Some(wearer), who);
            }
        }
    }

    pub fn dispel_effect(&mut self, id: u64) {
        self.finish_magic_effect(id);
    }

    /// The player (or an actor) drinks a potion or eats food or an ingredient
    /// they carry.
    pub fn consume(&mut self, actor: FormId, item: FormId) -> Result<(), String> {
        let Some(m) = self.magic_item(item) else {
            return Err(format!("{item} isn't a potion or ingredient"));
        };
        if m.spell_type == spell_type::POISON {
            // Used from the inventory, a poison goes on the weapon
            // (`crate::poisons`); what `EquipItem` does with one is open.
            return Err("poisons go on weapons from the inventory".into());
        }
        if self.item_count(actor, item) <= 0 {
            return Err(format!("{actor} doesn't carry {item}"));
        }
        self.remove_item(actor, item, 1, None);
        log::info!("{actor} uses {}", m.name);
        if actor == crate::engine::PLAYER_REF {
            let stat = if self.lo.tag_of(item).map(|t| t.0) == Some(*b"INGR") {
                "Ingredients Eaten"
            } else if self.is_food(item) {
                "Food Eaten"
            } else {
                "Potions Used"
            };
            self.increment_stat(stat, 1);
        }
        // The sound of drinking or eating (`ALCH` `ENIT`'s consume sound), where
        // they stand.
        if let Some(sound) = self
            .lo
            .get(item)
            .filter(|r| r.tag().0 == *b"ALCH")
            .and_then(|r| r.get(b"ENIT").and_then(|d| consume_sound(&d)))
            && let Some(at) = self.ref_position(actor)
        {
            self.play_sound(sound, at);
        }
        if actor == crate::engine::PLAYER_REF {
            self.learn_ingredient_effects(item);
        }
        self.apply_item(item, Some(actor), actor);
        Ok(())
    }

    /// Eating an ingredient teaches its first effect, or as many as the
    /// player's perks reveal (Experimenter: two, three, all four).
    fn learn_ingredient_effects(&mut self, item: FormId) {
        if self.lo.tag_of(item).map(|t| t.0) != Some(*b"INGR") {
            return;
        }
        let n = self.perk_entry_point(
            crate::perks::ep::MOD_INITIAL_INGREDIENT_EFFECTS_LEARNED,
            crate::engine::PLAYER_REF,
            &[Some(item)],
            1.0,
        );
        for i in 0..n.max(0.0) as usize {
            self.world_state.learned_effects.insert((item, i));
        }
    }

    /// Whether the player knows an ingredient's effect (its index in the list).
    pub fn effect_learned(&self, item: FormId, index: usize) -> bool {
        self.world_state.learned_effects.contains(&(item, index))
    }

    /// Whether an item is a poison (`IsPoison`).
    pub fn is_poison(&self, item: FormId) -> bool {
        self.lo.tag_of(item).is_some_and(|t| t.0 == *b"ALCH")
            && self
                .magic_item(item)
                .is_some_and(|m| m.spell_type == spell_type::POISON)
    }

    /// Console `effects`: what is on an actor.
    pub fn describe_effects(&self, actor: FormId) -> Vec<String> {
        let mut out = vec![format!(
            "{actor}: {} spells known",
            self.actor_spells(actor).len()
        )];
        for x in self.magic.effects.iter().filter(|x| x.target == actor) {
            let m = self.magic_effect(x.effect);
            let item = self.magic_item(x.item);
            out.push(format!(
                "{} {} ({}) from {} ({}): magnitude {:.1}, {}{}{}",
                x.id,
                m.as_ref().map_or("?", |m| m.name.as_str()),
                x.effect,
                item.as_ref().map_or("?", |m| m.name.as_str()),
                x.item,
                x.magnitude,
                if x.duration.is_infinite() {
                    "constant".to_string()
                } else {
                    format!("{:.0} / {:.0}s", x.elapsed, x.duration)
                },
                if x.active { "" } else { ", inactive" },
                if x.scripted { ", scripted" } else { "" },
            ));
        }
        out
    }
}

/// What `Engine::dispel` filters on.
pub struct EffectView {
    pub id: u64,
    pub item: FormId,
    /// Which of the item's effects.
    pub index: usize,
    pub effect_archetype: u32,
}

/// A taper's change over a frame from `from` to `to` seconds after it
/// began: `rate x (1 - t / seconds) ^ curve` at the frame's end, for its
/// length.
fn taper_step(rate: f32, curve: f32, seconds: f32, from: f32, to: f32) -> f32 {
    rate * (1.0 - (to / seconds).min(1.0)).powf(curve) * (to - from)
}

/// The sound drinking or eating a potion (`ALCH` `ENIT`: value, flags,
/// addiction, addiction chance, consume sound) makes, if it names one.
fn consume_sound(enit: &[u8]) -> Option<FormId> {
    let id = u32::from_le_bytes(enit.get(16..20)?.try_into().ok()?);
    (id != 0).then_some(FormId(id))
}

/// The world's speed under Slow Time effects of these magnitudes: the
/// slowest, from the least a tick can step to normal speed.
fn slow_time_speed(magnitudes: impl Iterator<Item = f32>) -> f32 {
    magnitudes.fold(1.0, |speed, m| speed.min(m.clamp(0.01, 1.0)))
}

/// Whether a projectile (`PROJ`) has an explosion (`DATA` 0x24).
fn projectile_explodes(lo: &esp::LoadOrder, proj: FormId) -> bool {
    lo.get(proj)
        .and_then(|r| {
            r.get(b"DATA")
                .and_then(|d| d.get(0x24..0x28))
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()) != 0)
        })
        .unwrap_or(false)
}

#[cfg(test)]
#[path = "effect_perk_tests.rs"]
mod effect_perk_tests;

#[cfg(test)]
mod taper_tests {
    use super::{consume_sound, slow_time_speed, taper_step};
    use esp::FormId;

    #[test]
    fn potions_name_their_consume_sound() {
        // The Sweet Roll's `ENIT`: value 2, flags 3, no addiction, 0xCAF94.
        let mut enit: Vec<u8> = vec![2, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(consume_sound(&enit), None);
        enit.extend([0x94, 0xaf, 0x0c, 0]);
        assert_eq!(consume_sound(&enit), Some(FormId(0xcaf94)));
        enit[16..].fill(0);
        assert_eq!(consume_sound(&enit), None);
    }

    #[test]
    fn slow_time_takes_the_slowest() {
        assert_eq!(slow_time_speed([].into_iter()), 1.0);
        assert_eq!(slow_time_speed([0.3, 0.1, 0.2].into_iter()), 0.1);
        assert_eq!(slow_time_speed([2.0].into_iter()), 1.0);
        assert_eq!(slow_time_speed([0.0].into_iter()), 0.01);
    }

    /// At 60 frames a second, what the game added (`test-mod taper`).
    #[test]
    fn tapers_add_the_measured_amounts() {
        let total = |m: f32, w: f32, c: f32, d: f32| {
            let frames = (d * 60.0).round() as usize;
            (0..frames)
                .map(|i| taper_step(m * w, c, d, i as f32 / 60.0, (i + 1) as f32 / 60.0))
                .sum::<f32>()
        };
        for (m, w, c, d, game) in [
            (50.0, 0.3, 2.0, 1.0, 4.875),
            (50.0, 0.3, 2.0, 2.0, 9.875),
            (50.0, 0.3, 2.0, 4.0, 19.875),
            (50.0, 0.2, 2.0, 1.0, 3.25),
            (50.0, 1.0, 1.0, 1.0, 24.583),
            (50.0, 1.0, 0.0, 0.3, 15.0),
        ] {
            let t = total(m, w, c, d);
            assert!(
                (t - game).abs() < 0.01,
                "{m} {w} {c} {d}: {t} against {game}"
            );
        }
    }
}

#[cfg(test)]
mod rolled_spells_tests {
    use super::*;

    #[test]
    fn a_leveled_list_is_rolled_once_per_actor() {
        let rolled = RolledSpells::default();
        let (actor, other, list) = (FormId(0x10), FormId(0x11), FormId(0x20));
        let a = rolled.get_or_roll(actor, list, || vec![FormId(0x30)]);
        // Later asks (at another level, say) give the first roll.
        let b = rolled.get_or_roll(actor, list, || vec![FormId(0x31)]);
        assert_eq!(a, b);
        // Another actor rolls its own.
        assert_eq!(
            rolled.get_or_roll(other, list, || vec![FormId(0x31)]),
            vec![FormId(0x31)]
        );
    }
}
