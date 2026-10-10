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
//! records themselves (`vrm-tool magic`). Open questions: `known_gaps/magic.md`.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use esp::FormId;
use esp::actor_value as av;
use papyrus::{ObjectId, Value};

use crate::condition::{self, Condition, Context};
use crate::engine::{Engine, PLAYER_REF};

/// Magic effect archetypes (`DATA` 0x40; CommonLibSSE `EffectArchetype`).
#[allow(dead_code)]
pub mod archetype {
    pub const VALUE_MODIFIER: u32 = 0;
    pub const SCRIPT: u32 = 1;
    pub const CURE_DISEASE: u32 = 3;
    pub const ABSORB: u32 = 4;
    pub const DUAL_VALUE_MODIFIER: u32 = 5;
    pub const INVISIBILITY: u32 = 11;
    pub const PARALYSIS: u32 = 21;
    pub const CURE_PARALYSIS: u32 = 27;
    pub const CURE_POISON: u32 = 29;
    pub const STAGGER: u32 = 33;
    pub const PEAK_VALUE_MODIFIER: u32 = 34;
}

/// Magic effect flags (`DATA` 0x00).
#[allow(dead_code)]
pub mod flags {
    pub const HOSTILE: u32 = 0x1;
    pub const RECOVER: u32 = 0x2;
    pub const DETRIMENTAL: u32 = 0x4;
    pub const NO_DURATION: u32 = 0x200;
    pub const HIDE_IN_UI: u32 = 0x8000;
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
    pub const SCROLL: u32 = 13;
}

/// The constant effect casting type.
pub const CONSTANT_EFFECT: u32 = 0;

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
    pub keywords: Vec<FormId>,
    pub conditions: Vec<Condition>,
    pub scripts: Vec<crate::script::vmad::ScriptRef>,
}

impl MagicEffect {
    pub fn load(e: &Engine, id: FormId) -> Option<MagicEffect> {
        let rec = e.lo.get(id).filter(|r| r.tag().0 == *b"MGEF")?;
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
                .map(|x| e.lo.lstring(&rec, x))
                .unwrap_or_default(),
            flags: u(0),
            related: fid(u(8)),
            skill: av(0x0C),
            resist: av(0x10),
            archetype: u(0x40),
            primary: av(0x44),
            secondary: av(0x58),
            second_weight: f(0x3C),
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
#[derive(Debug, Clone)]
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
    /// Per-second change not yet applied (applied in whole points).
    carry: f32,
    /// Its magic effect has scripts (running on `ObjectId::Effect(id)`).
    pub scripted: bool,
    pub done: bool,
}

/// Magic on actors: the active effects, spells added and removed, and what was
/// read.
#[derive(Debug, Default)]
pub struct Magic {
    pub effects: Vec<ActiveEffect>,
    next_id: u64,
    /// Spells added (`AddSpell`) and record spells removed (`RemoveSpell`).
    pub added: HashMap<FormId, Vec<FormId>>,
    pub removed: HashMap<FormId, HashSet<FormId>>,
    sync: f32,
    /// Finished effects' script objects and when to let them go.
    linger: Vec<(u64, f64)>,
    effect_cache: std::cell::RefCell<HashMap<FormId, Option<Arc<MagicEffect>>>>,
    item_cache: std::cell::RefCell<HashMap<FormId, Option<Arc<MagicItem>>>>,
}

impl Engine {
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
    /// giving the spell list), its race's, its perks' abilities and those
    /// added, less those removed.
    pub fn actor_spells(&self, actor: FormId) -> Vec<FormId> {
        let mut out: Vec<FormId> = Vec::new();
        let splo = |rec: &esp::LoadedRecord<'_>, out: &mut Vec<FormId>| {
            for s in rec
                .subrecords()
                .filter(|s| s.tag.0 == *b"SPLO" && s.data.len() >= 4)
            {
                out.push(rec.fid(FormId(s.u32(0))));
            }
        };
        if let Some(t) = self.templates_of(actor) {
            if let Some(rec) = t.record(&self.lo, crate::world::template::SPELLS, b"SPLO") {
                splo(&rec, &mut out);
            }
            if let Some(race) = t
                .form(&self.lo, crate::world::template::TRAITS, b"RNAM")
                .and_then(|r| self.lo.get(r))
            {
                splo(&race, &mut out);
            }
        }
        for &(perk, _) in self.actor_perks(actor).iter() {
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

    /// Apply an item's effects to `target` (no aiming or projectile: it lands).
    /// The same item's effects already on the target are replaced.
    pub fn apply_item(&mut self, item: FormId, caster: Option<FormId>, target: FormId) -> bool {
        let Some(m) = self.magic_item(item) else {
            return false;
        };
        if self.is_dead(target) && target != PLAYER_REF {
            return false;
        }
        self.dispel(target, |x| x.item == item);
        let constant = m.is_constant();
        let mut any = false;
        let mut hostile = false;
        for (index, ei) in m.effects.iter().enumerate() {
            let Some(mgef) = self.magic_effect(ei.effect) else {
                continue;
            };
            let ctx = Context {
                subject: Some(target),
                target: caster,
                ..Default::default()
            };
            // Fire-and-forget effects whose conditions fail don't land; constant
            // and lasting ones wait for them.
            let passes = condition::evaluate(self, &ei.conditions, ctx)
                && condition::evaluate(self, &mgef.conditions, ctx);
            if !passes && !constant && ei.duration == 0 {
                continue;
            }
            let magnitude = self.effect_magnitude(&m, &mgef, ei.magnitude, caster, target);
            let duration = if constant {
                f32::INFINITY
            } else if mgef.has(flags::NO_DURATION) {
                0.0
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
                carry: 0.0,
                scripted: !mgef.scripts.is_empty(),
                done: false,
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
        if hostile && let Some(c) = caster.filter(|&c| c != target) {
            self.hostile_magic_landed(target, c, item);
        }
        any
    }

    /// A hostile spell landed: `OnHit` (the spell as its source) to the
    /// target, and like a first blow it starts a fight (and an assault).
    fn hostile_magic_landed(&mut self, target: FormId, caster: FormId, item: FormId) {
        let args = vec![
            self.object_value(caster),
            self.object_value(item),
            Value::None,
            Value::Bool(false),
            Value::Bool(false),
            Value::Bool(false),
            Value::Bool(false),
        ];
        self.send_script_event(target, "OnHit", args);
        if target == PLAYER_REF {
            return;
        }
        let first = self.actor_ref(target).filter(|a| !a.dead).map(|a| {
            (
                a.combat.as_ref().is_none_or(|c| c.target != caster),
                a.combat.is_none(),
                a.stats.factions.clone(),
            )
        });
        if let Some((true, calm, factions)) = first {
            self.start_combat(target, caster);
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

    fn effect_index(&self, id: u64) -> Option<usize> {
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
            }
            archetype::PEAK_VALUE_MODIFIER => self.recompute_peaks(x.target, &mgef),
            archetype::INVISIBILITY => self.hold(i, &mgef, [1.0, 0.0]),
            archetype::PARALYSIS => self.hold(i, &mgef, [1.0, 0.0]),
            archetype::CURE_DISEASE => self.cure(x.target, |m| m.spell_type == spell_type::DISEASE),
            archetype::CURE_POISON => self.cure(x.target, |m| m.spell_type == spell_type::POISON),
            archetype::CURE_PARALYSIS => self.dispel(x.target, |e| {
                e.id != id && e.effect_archetype == archetype::PARALYSIS
            }),
            archetype::STAGGER => {
                if x.target != PLAYER_REF
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
    fn hold(&mut self, i: usize, mgef: &MagicEffect, by: [f32; 2]) {
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
    /// through the combat damage path (a kill is the caster's), absorbing
    /// gives the caster what the target lost.
    fn change_value(&mut self, x: &ActiveEffect, mgef: &MagicEffect, amount: f32, instant: bool) {
        let Some(a) = mgef.primary else { return };
        if mgef.archetype == archetype::ABSORB {
            let taken = amount.abs().min(self.actor_value(x.target, a).max(0.0));
            self.value_change(x.target, a, -taken, x.caster, instant);
            if let Some(c) = x.caster {
                self.value_change(c, a, taken, None, false);
            }
            return;
        }
        self.value_change(x.target, a, amount, x.caster, instant);
        if let Some(b) = mgef.secondary
            && mgef.archetype == archetype::DUAL_VALUE_MODIFIER
        {
            self.value_change(x.target, b, amount * mgef.second_weight, x.caster, instant);
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
        // What per-second change is left over lands now.
        if x.carry != 0.0 {
            self.magic.effects[i].carry = 0.0;
            self.change_value(&x, &mgef, x.carry, false);
        }
        if mgef.archetype == archetype::PEAK_VALUE_MODIFIER {
            self.release(i, &mgef);
            self.recompute_peaks(x.target, &mgef);
        } else {
            self.release(i, &mgef);
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
            if gone || dead {
                self.finish_magic_effect(id);
                continue;
            }
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
            if per_second {
                let sign = if mgef.has(flags::DETRIMENTAL) {
                    -1.0
                } else {
                    1.0
                };
                x.carry += sign * x.magnitude * step;
                if x.carry.abs() >= 1.0 || ends {
                    let amount = std::mem::take(&mut x.carry);
                    apply = Some((x.clone(), amount));
                }
            }
            if let Some((snapshot, amount)) = apply {
                self.change_value(&snapshot, &mgef, amount, false);
            }
            if ends {
                self.finish_magic_effect(id);
            }
        }
        self.magic.effects.retain(|x| !x.done);
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
        inv.equipped
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

    /// `HasMagicEffect`.
    pub fn has_magic_effect(&self, actor: FormId, effect: FormId) -> bool {
        self.magic
            .effects
            .iter()
            .any(|x| x.target == actor && x.active && !x.done && x.effect == effect)
    }

    /// `IsSpellTarget`: an effect of `item` is on the actor.
    pub fn is_spell_target(&self, actor: FormId, item: FormId) -> bool {
        self.magic
            .effects
            .iter()
            .any(|x| x.target == actor && !x.done && x.item == item)
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
            return Err("poisons go on weapons (not done yet)".into());
        }
        if self.item_count(actor, item) <= 0 {
            return Err(format!("{actor} doesn't carry {item}"));
        }
        self.remove_item(actor, item, 1, None);
        log::info!("{actor} uses {}", m.name);
        self.apply_item(item, Some(actor), actor);
        Ok(())
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
    pub effect_archetype: u32,
}
