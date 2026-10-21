//! Casting by hand: the player's spells from either hand and their powers,
//! paid for in magicka, and magicka coming back.
//!
//! A spell in a hand is cast with that hand's attack button (the right hand's
//! is the left mouse button). Fire-and-forget spells charge while it is held,
//! for their charge time (`SPIT`), and are cast when it comes up; let go
//! sooner, nothing is cast. Concentration spells cast as soon as it is down
//! and go on while it is held, their cost paid by the second. Without the
//! magicka, nothing is cast and the meter blinks.
//!
//! Where a cast goes is its delivery: on the caster (self), the actor in
//! front within reach (contact) or within the spell's range (target actor),
//! or as its effect's projectile (aimed): missiles and lobbers fly as arrows
//! do, flames and beams reach at once to their range. Target location spells
//! land where the view meets the world. Where a spell lands with an area,
//! the actors within it take it too.
//!
//! Powers (the voice slot, Z) are cast at once, greater powers once a day.
//!
//! With a behaviour graph, the body casts with its hands: the graph enters
//! its casting state (`BeginCastRight` / `BeginCastLeft`), the hand starts
//! the spell's kind of cast (`MRh_SpellAimedStart`...), is told when it is
//! charged (`..._SpellReady_Event`) or flowing and when it is let go
//! (`..._SpellRelease_Event`); a charged spell is cast at its release clip's
//! `..._SpellFire_Event`. When neither hand casts any more, `CastStop`.
//!
//! The same spell in both hands, both buttons pressed together and the
//! school's dual casting perk (`CanDualCastSpell`) make a dual cast: one
//! spell from both hands, costing `fMagicDualCastingCostMult` times as much,
//! its effects' duration (those flagged so) or else magnitude multiplied by
//! `fMagicDualCastingEffectivenessBase` (`test-mod dualcast`). The
//! graph casts it on the left hand's state machine (`DualMagic_...Start`,
//! then the left hand's events).
//!
//! Sources: UESP's Magic Overview, Magicka and Spells pages, the records
//! (`SPEL` `SPIT`, `MGEF` `DATA`, `PROJ` `DATA`, `EXPL` radius) and the game
//! settings. Choices: `known_ambiguities/casting.md`.

use std::collections::HashMap;

use esp::FormId;
use esp::actor_value as av;
use glam::Vec3;

use crate::ai::casters::CombatSpell;
use crate::ai::combat::gmst_f32;
use crate::engine::{Engine, PLAYER_REF};
use crate::magic::{EffectItem, MagicEffect, MagicItem, archetype, flags, spell_type};
use crate::spells::{CONCENTRATION, LEFT, RIGHT, VOICE};

/// Deliveries (`SPIT` / `MGEF`).
#[allow(dead_code)]
pub mod delivery {
    pub const SELF: u32 = 0;
    pub const CONTACT: u32 = 1;
    pub const AIMED: u32 = 2;
    pub const TARGET_ACTOR: u32 = 3;
    pub const TARGET_LOCATION: u32 = 4;
}

/// `PROJ` `DATA` types that reach at once rather than fly.
const PROJ_BEAM: u16 = 0x4;
const PROJ_FLAME: u16 = 0x8;
const PROJ_CONE: u16 = 0x10;

/// Units in a foot of an effect's area: Fireball's 15 feet are its
/// explosion's 320 units.
pub(crate) const UNITS_PER_FOOT: f32 = 320.0 / 15.0;
/// How often a concentration spell lands on what it reaches (seconds).
pub(crate) const CONCENTRATION_TICK: f32 = 0.25;
/// Contact spells' reach, and target actor spells' without a range.
const CONTACT_REACH: f32 = 160.0;
const TARGET_RANGE: f32 = 4096.0;
/// How far target location spells reach without a projectile.
const LOCATION_RANGE: f32 = 4096.0;
/// The middle of an actor's body at scale 1 (area hits).
const BODY_CENTRE: f32 = 60.0;
/// Game days between a greater power's uses.
const POWER_RECHARGE_DAYS: f32 = 1.0;
/// How long a spell let go may wait for its release clip's `SpellFire`,
/// and then for the clip to end; a choice (`known_ambiguities/casting.md`).
const FIRE_TIMEOUT: f32 = 1.5;
/// How soon after one hand the other must start the same spell for the two
/// to be a dual cast (seconds); a choice (`known_ambiguities/casting.md`).
const DUAL_WINDOW: f32 = 0.2;

/// A spell held ready in a hand.
#[derive(Debug, Clone)]
struct HandCast {
    spell: FormId,
    /// Seconds the button has been down.
    held: f32,
    /// Concentration: cast and going on; seconds to its next landing.
    flowing: bool,
    tick: f32,
    /// A concentration channel counts once when it actually starts landing.
    counted: bool,
    /// The hand casts it in the body's graph, and the graph was told it is
    /// charged (fire-and-forget) or flowing (concentration).
    graph: bool,
    told_ready: bool,
    /// Cast from both hands (held as the left's).
    dual: bool,
}

/// A charged spell let go in the graph: cast at its hand's `SpellFire`, then
/// the hand's release clip plays out (`Equipped`).
#[derive(Debug, Clone, Copy)]
struct Firing {
    spell: FormId,
    dual: bool,
    fired: bool,
    /// Seconds waiting for the next of those.
    waited: f32,
}

/// The hands' names in the graphs' events.
pub(crate) fn hand_prefix(hand: usize) -> &'static str {
    if hand == LEFT { "MLh" } else { "MRh" }
}

/// The event starting a hand's cast in the magic behaviour
/// (`magicbehavior.hkx`'s `MRh_ReadyCast_Behavior`): its kind from the
/// spell's casting type and delivery, wards (accumulating `WardPower`) and
/// telekinesis by their first effect's archetype. Contact, target actor and
/// target location spells are cast as aimed ones; a choice
/// (`known_ambiguities/casting.md`).
fn cast_start_event(item: &MagicItem, archetype: Option<u32>) -> &'static str {
    match archetype {
        Some(archetype::ACCUMULATE_MAGNITUDE) => return "WardStart",
        Some(archetype::TELEKINESIS) => return "SpellTelekinesisStart",
        _ => {}
    }
    match (
        item.delivery == delivery::SELF,
        item.cast_type == CONCENTRATION,
    ) {
        (true, false) => "SpellSelfStart",
        (true, true) => "SpellSelfConcentrationStart",
        (false, false) => "SpellAimedStart",
        (false, true) => "SpellAimedConcentrationStart",
    }
}

/// The player's casting: what each hand holds ready, magicka's wait to come
/// back, and when greater powers were last used.
#[derive(Debug, Default)]
pub struct Casting {
    hands: [Option<HandCast>; 2],
    firing: [Option<Firing>; 2],
    /// Seconds before the player's magicka comes back after being spent.
    magicka_wait: f32,
    /// Greater powers' last use (game days).
    powers_used: HashMap<FormId, f32>,
    /// The magicka meter should blink (counted; the HUD blinks for each).
    pub magicka_blinks: u32,
    /// Candidates from the actor's current known spells. Conditions and costs
    /// are checked when choosing; AddSpell / RemoveSpell rebuild this cache.
    npc_spells: HashMap<FormId, std::sync::Arc<NpcSpells>>,
}

#[derive(Debug, Default)]
struct NpcSpells {
    known: Vec<FormId>,
    offensive: Vec<CombatSpell>,
    healing: Vec<CombatSpell>,
    wards: Vec<CombatSpell>,
}

/// Provisional combat policy, not the native scoring formula; see
/// `known_ambiguities/npc-healing.md` for the evidence and unresolved settings.
fn needs_combat_healing(health: f32, max_health: f32, continuing: bool) -> bool {
    health > 0.0 && health < max_health * if continuing { 1.0 } else { 0.5 }
}

/// Whether an effect takes health from what it lands on, as a hostile
/// Value Modifier on Health does to an actor (fire, frost and shock damage).
/// Whether it does so to objects is `known_ambiguities/destruction.md`.
fn damages_health(effect: &MagicEffect, item: &EffectItem) -> bool {
    effect.has(flags::HOSTILE)
        && effect.has(flags::DETRIMENTAL)
        && item.magnitude > 0.0
        && matches!(
            effect.archetype,
            archetype::VALUE_MODIFIER | archetype::DUAL_VALUE_MODIFIER
        )
        && effect.primary == Some(av::HEALTH)
}

/// Restore current Health, as opposed to fortifying its maximum or absorbing
/// it from someone else. A dual value modifier can restore Health second.
fn restores_health(effect: &MagicEffect, item: &EffectItem) -> bool {
    if effect.has(flags::DETRIMENTAL | flags::RECOVER | flags::HOSTILE) || item.magnitude <= 0.0 {
        return false;
    }
    match effect.archetype {
        archetype::VALUE_MODIFIER => effect.primary == Some(av::HEALTH),
        archetype::DUAL_VALUE_MODIFIER => {
            effect.primary == Some(av::HEALTH)
                || (effect.secondary == Some(av::HEALTH) && effect.second_weight > 0.0)
        }
        _ => false,
    }
}

impl Engine {
    /// What a hand (`RIGHT`, `LEFT`) casts: its spell, or its staff's
    /// enchantment.
    fn hand_spell(&self, hand: usize) -> Option<FormId> {
        self.equipped_spell(PLAYER_REF, hand)
            .or_else(|| self.staff_in(PLAYER_REF, hand).map(|(_, ench)| ench))
    }

    /// The staff in the player's hands that casts `ench`, if it is a staff's.
    fn player_staff_of(&self, ench: FormId) -> Option<FormId> {
        [RIGHT, LEFT]
            .into_iter()
            .filter_map(|h| self.staff_in(PLAYER_REF, h))
            .find(|&(_, e)| e == ench)
            .map(|(staff, _)| staff)
    }

    /// What the player has to pay for `spell` with: magicka, or its staff's
    /// charge.
    fn player_reserve(&self, spell: FormId) -> f32 {
        match self.player_staff_of(spell) {
            Some(staff) => self.weapon_charge(PLAYER_REF, staff),
            None => self.actor_value(PLAYER_REF, av::MAGICKA),
        }
    }

    /// Pay for `spell`: from its staff's charge, else magicka.
    fn player_pay(&mut self, spell: FormId, amount: f32) {
        match self.player_staff_of(spell) {
            Some(staff) => self.spend_weapon_charge(PLAYER_REF, staff, amount),
            None => self.spend_magicka(amount),
        }
    }

    /// A scroll the player cast is used up: one of them leaves the inventory,
    /// and the hand puts the scroll away when it was the last. A scroll cut
    /// short before the cast is not used (UESP's Skyrim:Scrolls).
    fn use_up_scroll(&mut self, spell: FormId) {
        if self.lo.tag_of(spell).is_none_or(|t| t.0 != *b"SCRL") {
            return;
        }
        self.remove_item(PLAYER_REF, spell, 1, None);
        self.unequip_if_gone(PLAYER_REF, spell);
    }

    /// A hand's attack button went down: casting starts if it holds a spell
    /// (true), else it does what it does with a weapon (false).
    pub fn player_cast_press(&mut self, hand: usize) -> bool {
        let Some(spell) = self.hand_spell(hand) else {
            return false;
        };
        self.player_breaks_ethereal();
        if self.disabled_controls.fighting
            || self.player_dead()
            || self.conversation.is_some()
            || self.casting.hands[hand].is_some()
            || self.casting.firing[hand].is_some_and(|f| !f.fired)
            || self.player_dual_casting()
        {
            return true;
        }
        // Hands put away come out first.
        if self.player_sheathed() {
            self.player_draw_weapon(true);
            return true;
        }
        let Some(item) = self.magic_item(spell) else {
            return true;
        };
        if self.player_has_graph() && !self.player_weapon_ready() {
            return true;
        }
        if self.player_dual_start(hand, spell, &item) {
            return true;
        }
        if !self.can_pay(spell, &item, false) {
            return true;
        }
        // Where the graph has no casting for the hands (a spell beside a
        // shield), the spell is cast without it.
        let graph = self.player_has_graph() && self.player_hand_start(hand, &item);
        log::debug!("player readies {} in hand {hand}", item.name);
        self.casting.hands[hand] = Some(HandCast {
            spell,
            held: 0.0,
            flowing: false,
            tick: 0.0,
            counted: false,
            graph,
            told_ready: false,
            dual: false,
        });
        if item.cast_type == CONCENTRATION {
            self.start_concentration(hand);
        }
        true
    }

    /// A hand's attack button came up: a charged spell is cast, a
    /// concentration spell stops. True if the hand was casting.
    pub fn player_cast_release(&mut self, hand: usize) -> bool {
        // Either button lets a dual cast go.
        let hand = if self.player_dual_casting() {
            LEFT
        } else {
            hand
        };
        let Some(h) = self.casting.hands[hand].take() else {
            return false;
        };
        if h.flowing {
            if h.graph {
                self.player_hand_idle(hand);
            }
            return true;
        }
        let Some(item) = self.magic_item(h.spell) else {
            return true;
        };
        if h.held + 1e-3 < self.charge_time(&item, h.dual) {
            log::debug!("{} let go before it charged", item.name);
            if h.graph {
                self.player_hand_idle(hand);
            }
            return true;
        }
        if h.graph {
            // Charged in the very update it was let go.
            let p = hand_prefix(hand);
            if !h.told_ready {
                self.player_graph_event(&format!("{p}_SpellReady_Event"));
            }
            if self.player_graph_event(&format!("{p}_SpellRelease_Event")) {
                self.casting.firing[hand] = Some(Firing {
                    spell: h.spell,
                    dual: h.dual,
                    fired: false,
                    waited: 0.0,
                });
            } else {
                log::debug!("the graph won't release {}", item.name);
                self.player_hand_idle(hand);
            }
            return true;
        }
        if self.can_pay(h.spell, &item, h.dual) {
            let cost = self.player_spell_cost(h.spell, h.dual);
            self.player_pay(h.spell, cost);
            self.count_spell_cast(PLAYER_REF, h.spell);
            self.cast_with(PLAYER_REF, h.spell, 1.0, h.dual);
            self.use_up_scroll(h.spell);
        }
        true
    }

    /// Whether a hand of the player's casts `spell` as a concentration spell
    /// (its channelled effects last while it does).
    pub(crate) fn player_holds_spell(&self, spell: FormId) -> bool {
        self.casting
            .hands
            .iter()
            .flatten()
            .any(|h| h.flowing && h.spell == spell)
    }

    /// The hands casting `spell` stop (a ward broken).
    pub(crate) fn player_stop_holding(&mut self, spell: FormId) {
        for hand in [RIGHT, LEFT] {
            if let Some(h) = self.casting.hands[hand].take_if(|h| h.spell == spell)
                && h.graph
            {
                self.player_hand_idle(hand);
            }
        }
    }

    /// Whether the player is dual casting: holding a spell from both hands,
    /// or letting one go that hasn't left yet.
    fn player_dual_casting(&self) -> bool {
        self.casting.hands[LEFT].as_ref().is_some_and(|h| h.dual)
            || self.casting.firing[LEFT].is_some_and(|f| f.dual && !f.fired)
    }

    /// Whether an actor is dual casting (`IsDualCasting`).
    pub fn is_dual_casting(&self, actor: FormId) -> bool {
        if actor == PLAYER_REF {
            return self.player_dual_casting();
        }
        self.actor_ref(actor)
            .and_then(|a| a.combat.as_ref()?.hand_cast)
            .is_some_and(|h| h.cast.dual)
    }

    /// `hand` starts `spell` while the other hand started the same spell
    /// within `DUAL_WINDOW`: with the school's dual casting perk and the
    /// magicka for it, the two become one dual cast (true), the graph casting
    /// it with the left hand (`DualMagic_...Start`, the right hand back to
    /// ready). Else each hand casts its own.
    fn player_dual_start(&mut self, hand: usize, spell: FormId, item: &MagicItem) -> bool {
        let other = if hand == LEFT { RIGHT } else { LEFT };
        let Some(o) = self.casting.hands[other].clone() else {
            return false;
        };
        if o.spell != spell
            || o.dual
            || o.held > DUAL_WINDOW
            || item.spell_type != spell_type::SPELL
            || !self.perk_flag(
                crate::perks::ep::CAN_DUAL_CAST_SPELL,
                PLAYER_REF,
                &[Some(spell)],
            )
            || !self.affords(spell, item, true)
        {
            return false;
        }
        if o.graph {
            let start = format!("DualMagic_{}", self.cast_start(item));
            // Ignored in the casting state, where the other hand's cast is.
            self.player_graph_event("BeginCastLeft");
            if !self.player_graph_event(&start) {
                log::debug!("the graph won't dual cast {}", item.name);
                return false;
            }
            if other == RIGHT {
                self.player_graph_event("MRh_Equipped_Event");
            }
            // The left hand's last release, if still playing out, gives way.
            self.casting.firing[LEFT] = None;
        }
        log::debug!("player dual casts {}", item.name);
        self.casting.hands[other] = None;
        self.casting.hands[LEFT] = Some(HandCast {
            told_ready: false,
            dual: true,
            ..o
        });
        true
    }

    /// A spell's charge time, dual cast or not.
    fn charge_time(&self, item: &MagicItem, dual: bool) -> f32 {
        if dual {
            item.charge_time * gmst_f32(&self.lo, "fMagicDualCastingTimeMult", 1.0)
        } else {
            item.charge_time
        }
    }

    /// What a spell costs the player, dual cast or not (a staff's
    /// enchantment, in charge).
    fn player_spell_cost(&self, spell: FormId, dual: bool) -> f32 {
        // A scroll costs no magicka (UESP's Skyrim:Scrolls): it is used up.
        if self.lo.tag_of(spell).is_some_and(|t| t.0 == *b"SCRL") {
            return 0.0;
        }
        if self.player_staff_of(spell).is_some() {
            return self.staff_cost(PLAYER_REF, spell);
        }
        let cost = self.spell_cost(PLAYER_REF, spell);
        if dual {
            cost * gmst_f32(&self.lo, "fMagicDualCastingCostMult", 2.8)
        } else {
            cost
        }
    }

    /// Whether an actor is casting by hand (only the player does yet).
    /// The spells the player's hands are charging, holding or letting go.
    pub(crate) fn player_casting_spells(&self) -> Vec<FormId> {
        let held = self.casting.hands.iter().flatten().map(|h| h.spell);
        let firing = self.casting.firing.iter().flatten().filter(|f| !f.fired);
        held.chain(firing.map(|f| f.spell)).collect()
    }

    pub fn is_casting(&self, actor: FormId) -> bool {
        actor == PLAYER_REF
            && (self.casting.hands.iter().any(Option::is_some)
                || self.casting.firing.iter().flatten().any(|f| !f.fired))
    }

    /// The spell `actor` casts from a hand (`LEFT` or `RIGHT`) now: the
    /// player's held spell or one let go and not yet fired, or an actor's
    /// cast in combat. None for the voice (no hand's state) and when nothing
    /// is cast.
    pub(crate) fn hand_casting(&self, actor: FormId, hand: usize) -> Option<FormId> {
        if actor == PLAYER_REF {
            self.casting
                .hands
                .get(hand)?
                .as_ref()
                .map(|h| h.spell)
                .or_else(|| {
                    self.casting
                        .firing
                        .get(hand)?
                        .as_ref()
                        .filter(|f| !f.fired)
                        .map(|f| f.spell)
                })
        } else {
            self.actor_ref(actor)?
                .combat
                .as_ref()?
                .hand_cast
                .as_ref()
                .filter(|h| h.hand == hand)
                .map(|h| h.cast.spell.spell)
        }
    }

    /// Start a hand's cast in the body's graph: into the casting state (the
    /// graph ignores it there already), then the hand's kind of cast. False
    /// if the graph won't.
    fn player_hand_start(&mut self, hand: usize, item: &MagicItem) -> bool {
        let start = format!("{}_{}", hand_prefix(hand), self.cast_start(item));
        let began = self.player_graph_event(if hand == LEFT {
            "BeginCastLeft"
        } else {
            "BeginCastRight"
        });
        if self.player_graph_event(&start) {
            // The hand's last release, if still playing out, gives way.
            self.casting.firing[hand] = None;
            return true;
        }
        if began {
            self.player_graph_event("CastStop");
        }
        false
    }

    /// A hand stopped casting in the graph: back to holding its spell ready,
    /// and the graph out of its casting state once neither hand casts.
    fn player_hand_idle(&mut self, hand: usize) {
        self.player_graph_event(&format!("{}_Equipped_Event", hand_prefix(hand)));
        self.player_cast_stop();
    }

    fn player_cast_stop(&mut self) {
        if self.casting.hands.iter().all(Option::is_none)
            && self.casting.firing.iter().all(Option::is_none)
        {
            self.player_graph_event("CastStop");
        }
    }

    /// The body's graph raised its hands' `SpellFire` (`fire`) and
    /// `Equipped` (`done`, the release clip over) this update: spells let go
    /// are cast, paid for then, and hands done with their release free.
    pub(crate) fn player_spell_events(&mut self, fire: [bool; 2], done: [bool; 2], dt: f32) {
        for hand in [RIGHT, LEFT] {
            let Some(mut f) = self.casting.firing[hand] else {
                continue;
            };
            if f.fired && done[hand] {
                self.casting.firing[hand] = None;
                self.player_cast_stop();
                continue;
            }
            if !f.fired && fire[hand] {
                f.fired = true;
                f.waited = 0.0;
                self.casting.firing[hand] = Some(f);
                if let Some(item) = self.magic_item(f.spell)
                    && !self.player_dead()
                    && self.can_pay(f.spell, &item, f.dual)
                {
                    let cost = self.player_spell_cost(f.spell, f.dual);
                    self.player_pay(f.spell, cost);
                    self.count_spell_cast(PLAYER_REF, f.spell);
                    self.cast_with(PLAYER_REF, f.spell, 1.0, f.dual);
                    self.use_up_scroll(f.spell);
                }
                continue;
            }
            f.waited += dt;
            self.casting.firing[hand] = Some(f);
            if f.waited >= FIRE_TIMEOUT {
                if !f.fired {
                    log::debug!("hand {hand}'s release never reached its SpellFire");
                }
                self.casting.firing[hand] = None;
                self.player_hand_idle(hand);
            }
        }
    }

    /// Whether the player has the magicka for a spell (a second of a
    /// concentration spell), dual cast or not.
    fn affords(&self, spell: FormId, item: &MagicItem, dual: bool) -> bool {
        if item.spell_type != spell_type::SPELL && item.spell_type != spell_type::STAFF_ENCHANTMENT
        {
            return true;
        }
        let cost = self.player_spell_cost(spell, dual);
        let have = self.player_reserve(spell);
        // A concentration spell starts with any magicka at all.
        let need = if item.cast_type == CONCENTRATION {
            cost.min(1.0)
        } else {
            cost
        };
        have + 1e-3 >= need
    }

    /// As `affords`; if not, the meter blinks and the player is told.
    fn can_pay(&mut self, spell: FormId, item: &MagicItem, dual: bool) -> bool {
        if self.affords(spell, item, dual) {
            return true;
        }
        if self.player_staff_of(spell).is_some() {
            self.notify_no_charge();
            return false;
        }
        self.casting.magicka_blinks += 1;
        let text = self
            .gmst_string("sMagicCastInsufficientMagicka")
            .unwrap_or_else(|| "You don't have enough Magicka".into());
        self.scripts.notify(text);
        false
    }

    /// "Your item has insufficient charge".
    fn notify_no_charge(&mut self) {
        let text = self
            .gmst_string("sMagicCastInsufficientItemCharge")
            .unwrap_or_else(|| "Your item has insufficient charge".into());
        self.scripts.notify(text);
    }

    /// Take magicka from the player; it waits a moment to come back.
    fn spend_magicka(&mut self, amount: f32) {
        if amount <= 0.0 {
            return;
        }
        self.damage_actor_value(PLAYER_REF, av::MAGICKA, amount);
        self.casting.magicka_wait = gmst_f32(&self.lo, "fDamagedMagickaRegenDelay", 0.5);
    }

    fn start_concentration(&mut self, hand: usize) {
        if let Some(h) = self.casting.hands[hand].as_mut() {
            h.flowing = true;
            h.tick = 0.0;
        }
    }

    /// The voice key (Z): the power readied is used. Shouts aren't built.
    pub fn player_use_power(&mut self) {
        if self.disabled_controls.fighting || self.player_dead() || self.conversation.is_some() {
            return;
        }
        let Some(power) = self.equipped_spell(PLAYER_REF, VOICE) else {
            return;
        };
        let Some(item) = self.magic_item(power) else {
            return;
        };
        match item.spell_type {
            spell_type::POWER => {
                let now = self.game_days();
                if self
                    .casting
                    .powers_used
                    .get(&power)
                    .is_some_and(|&t| now - t < POWER_RECHARGE_DAYS)
                {
                    let text = self
                        .gmst_string("sMagicCastPowerUsed")
                        .unwrap_or_else(|| "Powers can only be used once a day.".into());
                    self.scripts.notify(text);
                    return;
                }
                self.casting.powers_used.insert(power, now);
            }
            spell_type::LESSER_POWER => {}
            _ => return,
        }
        self.cast(PLAYER_REF, power, 1.0);
    }

    /// Holding spells: charging, concentration landing and paying by the
    /// second; magicka coming back.
    pub(crate) fn update_casting(&mut self, dt: f32) {
        let mut casting = false;
        for hand in [RIGHT, LEFT] {
            let Some(mut h) = self.casting.hands[hand].clone() else {
                continue;
            };
            // Staffs leave magicka coming back.
            casting |= self.player_staff_of(h.spell).is_none();
            // The spell left the hand (unequipped, a weapon took it), or the
            // hands were put away.
            if self.hand_spell(hand) != Some(h.spell)
                || (h.dual && self.hand_spell(RIGHT) != Some(h.spell))
                || (h.graph && !self.player_weapon_drawn())
            {
                self.casting.hands[hand] = None;
                if h.graph {
                    self.player_hand_idle(hand);
                }
                continue;
            }
            h.held += dt;
            if h.graph && !h.told_ready {
                // Told the update after it started, when the hand's own state
                // machine is running.
                let ready = if h.flowing {
                    Some("SpellRelease_Event")
                } else {
                    self.magic_item(h.spell)
                        .filter(|item| h.held + 1e-3 >= self.charge_time(item, h.dual))
                        .map(|_| "SpellReady_Event")
                };
                if let Some(ev) = ready {
                    self.player_graph_event(&format!("{}_{ev}", hand_prefix(hand)));
                    h.told_ready = true;
                }
            }
            if h.flowing {
                let cost = self.player_spell_cost(h.spell, h.dual) * dt;
                if self.player_reserve(h.spell) < cost {
                    let staff = self.player_staff_of(h.spell).is_some();
                    log::debug!(
                        "player's {} ran out",
                        if staff { "staff" } else { "magicka" }
                    );
                    if staff {
                        self.notify_no_charge();
                    } else {
                        self.casting.magicka_blinks += 1;
                    }
                    self.casting.hands[hand] = None;
                    if h.graph {
                        self.player_hand_idle(hand);
                    }
                    continue;
                }
                self.player_pay(h.spell, cost);
                h.tick -= dt;
                if h.tick <= 0.0 {
                    h.tick += CONCENTRATION_TICK;
                    if !h.counted {
                        self.count_spell_cast(PLAYER_REF, h.spell);
                        h.counted = true;
                    }
                    self.cast_with(PLAYER_REF, h.spell, CONCENTRATION_TICK, h.dual);
                }
            }
            self.casting.hands[hand] = Some(h);
        }
        if !casting {
            self.regen_magicka(dt);
        }
        self.regen_npc_magicka(dt);
    }

    /// Magicka comes back by the actor's rate (percent of the most a second,
    /// times `MagickaRateMult`), a third as fast in combat; the player's once
    /// a moment has passed since it was spent.
    fn magicka_regen(&self, actor: FormId, fighting: bool, dt: f32) -> f32 {
        let max = self.av_max(actor, av::MAGICKA);
        let now = self.actor_value(actor, av::MAGICKA);
        if now >= max {
            return 0.0;
        }
        let mult = if fighting {
            gmst_f32(&self.lo, "fCombatMagickaRegenRateMult", 0.33)
        } else {
            1.0
        };
        let rate = self.actor_value(actor, av::MAGICKA_RATE) / 100.0
            * (self.actor_value(actor, av::MAGICKA_RATE_MULT) / 100.0)
            * mult;
        (max * rate * dt).clamp(0.0, max - now)
    }

    pub(crate) fn regen_magicka(&mut self, dt: f32) {
        if self.casting.magicka_wait > 0.0 {
            self.casting.magicka_wait -= dt;
            return;
        }
        let fighting = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .any(|a| !a.dead && a.combat.as_ref().is_some_and(|c| c.target == PLAYER_REF));
        let gain = self.magicka_regen(PLAYER_REF, fighting, dt);
        if gain > 0.0 {
            self.damage_actor_value(PLAYER_REF, av::MAGICKA, -gain);
        }
    }

    /// Loaded actors' spent magicka comes back, but not while they channel a
    /// spell (`bRegenNPCMagickaDuringCast`).
    pub(crate) fn regen_npc_magicka(&mut self, dt: f32) {
        let spent: Vec<FormId> = self
            .scripts
            .actor_values
            .iter()
            .filter(|((a, i), m)| *i == av::MAGICKA && m.damage < 0.0 && *a != PLAYER_REF)
            .map(|((a, _), _)| *a)
            .collect();
        for actor in spent {
            let Some(a) = self.actor_ref(actor).filter(|a| !a.dead) else {
                continue;
            };
            let channeling = a.combat.as_ref().is_some_and(|c| c.channel > 0.0)
                || a.magic_practice.is_some_and(|m| m.channelling());
            if channeling {
                continue;
            }
            let fighting = a.combat.is_some();
            let gain = self.magicka_regen(actor, fighting, dt);
            if gain > 0.0 {
                self.damage_actor_value(actor, av::MAGICKA, -gain);
            }
        }
    }

    /// Injured fighters first choose an affordable self heal; then anyone a
    /// spell is coming at raises a ward it knows (`crate::wards`). Otherwise
    /// those whose style favours magic choose an offensive spell (none: by
    /// hand).
    pub(crate) fn update_combat_spells(&mut self, dt: f32) {
        self.update_ward_threats(dt);
        // The tuple's last member: the spell of a casting practice (UseMagic) at
        // its place, out of combat, and whether the package dual casts it.
        type Practice = Option<(FormId, bool)>;
        let fighters: Vec<(FormId, bool, bool, bool, bool, Practice)> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .map(|a| {
                // Equal scores: those with no weapon to hand (atronachs) cast.
                let unarmed = || {
                    self.inventories
                        .get(&a.ref_id)
                        .and_then(|i| i.weapon(&self.lo))
                        .is_none()
                };
                let caster = a.stats.magic_mult > a.stats.melee_mult
                    || (a.stats.magic_mult == a.stats.melee_mult && unarmed());
                let active = !a.dead
                    && a.bleeding.is_none()
                    && a.combat.as_ref().is_some_and(|c| !c.fleeing);
                (
                    a.ref_id,
                    active,
                    caster,
                    a.combat_spell.is_some_and(|s| s.self_target),
                    a.stats.staff_mult >= a.stats.magic_mult,
                    (a.magic_practising() && !a.dead && a.bleeding.is_none())
                        .then(|| a.goal.and_then(|g| g.magic).map(|m| (m.spell, m.dual)))
                        .flatten(),
                )
            })
            .collect();
        for (actor, active, caster, continuing, staff_first, practice) in fighters {
            let pick = if active {
                let magicka = self.actor_value(actor, av::MAGICKA);
                let spells = self.npc_spells(actor);
                let heal = needs_combat_healing(
                    self.actor_value(actor, av::HEALTH),
                    self.av_max(actor, av::HEALTH),
                    continuing,
                );
                let healing = spells
                    .healing
                    .iter()
                    .filter(|s| heal && self.healing_spell_works(actor, s.spell))
                    .map(|s| (*s, self.spell_cost(actor, s.spell)))
                    // A concentration spell for a second or more.
                    .filter(|(_, cost)| *cost <= magicka)
                    .max_by(|a, b| a.1.total_cmp(&b.1))
                    .map(|(s, _)| s);
                // The strongest ward it can hold a second of.
                let ward = spells
                    .wards
                    .iter()
                    .filter(|_| self.wants_ward(actor))
                    .map(|s| (*s, self.spell_cost(actor, s.spell)))
                    .filter(|(_, cost)| *cost <= magicka)
                    .max_by(|a, b| a.1.total_cmp(&b.1))
                    .map(|(s, _)| s);
                // A staff in hand is fought with by anyone holding it: first,
                // if the style weighs staffs as much as magic, else when
                // there is no spell to cast.
                let staff = self.npc_staff_spell(actor);
                let spell = spells
                    .offensive
                    .iter()
                    .filter(|_| caster)
                    .map(|s| (*s, self.spell_cost(actor, s.spell)))
                    .filter(|(_, cost)| *cost <= magicka)
                    .max_by(|a, b| a.1.total_cmp(&b.1))
                    .map(|(s, _)| s);
                healing.or(ward).or(if staff_first {
                    staff.or(spell)
                } else {
                    spell.or(staff)
                })
            } else {
                practice.and_then(|(spell, _)| self.practice_spell(spell))
            };
            // Both hands free (no one-handed weapon or staff in the right),
            // the caster may dual cast what its perks let it (a package that
            // dual casts, what it can).
            let one_handed = self.inventories.get(&actor).is_some_and(|i| {
                let right = i.hand(&self.lo, false);
                right.one_handed() || right == crate::world::inventory::HandType::Staff
            });
            let pick = pick.map(|s| CombatSpell {
                dual: !s.staff
                    && !one_handed
                    && (practice.is_some_and(|(_, dual)| dual)
                        || self.perk_flag(
                            crate::perks::ep::CAN_DUAL_CAST_SPELL,
                            actor,
                            &[Some(s.spell)],
                        )),
                ..s
            });
            if self.actor_ref(actor).is_none_or(|a| a.combat_spell == pick) {
                continue;
            }
            // Fighting with a spell, it is in hand in the graph: the left
            // beside a one-handed weapon, else the right.
            let speeds = self.weapon_speed_mults(actor);
            let inv = self.inventories.get(&actor).cloned().unwrap_or_default();
            let vars = crate::ai::combat::hand_graph_vars(&self.lo, &inv, speeds);
            let hand = pick.map(|s| if one_handed && !s.staff { LEFT } else { RIGHT });
            let was_ward = self
                .actor_ref(actor)
                .is_some_and(|a| a.combat_spell.is_some_and(|s| s.ward));
            let ward_wait = self.ward_wait_roll();
            if let Some(a) = self.actor_mut(actor) {
                if let Some(c) = a.combat.as_mut() {
                    if pick.is_some_and(|s| s.ward) {
                        // Raised at once, whatever its last throw.
                        c.cooldown = c.cooldown.min(0.0);
                    } else if was_ward {
                        c.ward_wait = c.ward_wait.max(ward_wait);
                        c.ward_keep = 0.0;
                    }
                }
                // A spell being channelled stops with it; one thrown plays
                // out, unless there is none to fight with now.
                if let Some(c) = a.combat.as_mut()
                    && c.channel > 0.0
                {
                    c.channel = 0.0;
                    a.stop_hand_cast();
                }
                if pick.is_none() {
                    a.stop_hand_cast();
                }
                a.combat_spell = pick;
                if a.cast_hand != hand {
                    a.cast_hand = hand;
                    a.set_hand_vars(vars);
                }
            }
        }
    }

    /// A spell's kind of cast for the hands' graph events.
    fn cast_start(&self, item: &MagicItem) -> &'static str {
        let archetype = item
            .effects
            .first()
            .and_then(|e| self.magic_effect(e.effect))
            .map(|e| e.archetype);
        cast_start_event(item, archetype)
    }

    /// Read candidates again when the actor's known spell list changes. The
    /// effects' live target conditions are deliberately not cached here.
    fn npc_spells(&mut self, actor: FormId) -> std::sync::Arc<NpcSpells> {
        let known = self.actor_spells(actor);
        if let Some(s) = self
            .casting
            .npc_spells
            .get(&actor)
            .filter(|s| s.known == known)
        {
            return s.clone();
        }
        let mut out = NpcSpells {
            known,
            ..Default::default()
        };
        for &spell in &out.known {
            let Some(item) = self.magic_item(spell) else {
                continue;
            };
            if item.spell_type != spell_type::SPELL || !matches!(item.cast_type, 1 | CONCENTRATION)
            {
                continue;
            }
            if item.delivery == delivery::SELF && !self.item_hostile(&item) {
                if item.effects.iter().any(|ei| {
                    self.magic_effect(ei.effect)
                        .is_some_and(|m| restores_health(&m, ei))
                }) {
                    out.healing.push(CombatSpell {
                        spell,
                        range: 0.0,
                        concentration: item.cast_type == CONCENTRATION,
                        self_target: true,
                        start: self.cast_start(&item),
                        charge: item.charge_time,
                        dual: false,
                        staff: false,
                        ward: false,
                    });
                }
                if item.cast_type == CONCENTRATION
                    && item.effects.iter().any(|ei| {
                        self.magic_effect(ei.effect)
                            .is_some_and(|m| m.archetype == archetype::ACCUMULATE_MAGNITUDE)
                    })
                {
                    out.wards.push(CombatSpell {
                        spell,
                        range: 0.0,
                        concentration: true,
                        self_target: true,
                        start: self.cast_start(&item),
                        charge: item.charge_time,
                        dual: false,
                        staff: false,
                        ward: true,
                    });
                }
                continue;
            }
            out.offensive.extend(self.offensive_spell(&item));
        }
        let out = std::sync::Arc::new(out);
        self.casting.npc_spells.insert(actor, out.clone());
        out
    }

    /// A spell a casting practice (UseMagic) casts: a spell on the caster
    /// itself, or an aimed one or one at an actor, hostile or not, at the
    /// target it is given. Others (touch, at a location) aren't cast.
    pub(crate) fn practice_spell(&self, spell: FormId) -> Option<CombatSpell> {
        let item = self.magic_item(spell)?;
        if !matches!(item.cast_type, 1 | CONCENTRATION) {
            return None;
        }
        if item.delivery == delivery::SELF {
            let ward = item.cast_type == CONCENTRATION
                && item.effects.iter().any(|ei| {
                    self.magic_effect(ei.effect)
                        .is_some_and(|m| m.archetype == archetype::ACCUMULATE_MAGNITUDE)
                });
            return Some(CombatSpell {
                spell,
                range: 0.0,
                concentration: item.cast_type == CONCENTRATION,
                self_target: true,
                start: self.cast_start(&item),
                charge: item.charge_time,
                dual: false,
                staff: false,
                ward,
            });
        }
        self.aimed_spell(&item)
    }

    /// A hostile aimed or target actor spell (or staff enchantment) to
    /// fight with.
    fn offensive_spell(&self, item: &MagicItem) -> Option<CombatSpell> {
        self.item_hostile(item)
            .then(|| self.aimed_spell(item))
            .flatten()
    }

    /// An aimed or target actor spell (or staff enchantment) to cast.
    fn aimed_spell(&self, item: &MagicItem) -> Option<CombatSpell> {
        if !matches!(item.delivery, delivery::AIMED | delivery::TARGET_ACTOR) {
            return None;
        }
        let range = if item.delivery == delivery::TARGET_ACTOR {
            if item.range > 0.0 {
                item.range
            } else {
                TARGET_RANGE
            }
        } else {
            self.item_projectile(item)
                .and_then(|p| crate::ai::archery::spell_projectile(&self.lo, p))
                .map_or(LOCATION_RANGE, |s| s.range)
        };
        Some(CombatSpell {
            spell: item.id,
            range: range.min(crate::ai::archery::BOW_RANGE),
            concentration: item.cast_type == CONCENTRATION,
            self_target: false,
            start: self.cast_start(item),
            charge: item.charge_time,
            dual: false,
            staff: item.spell_type == spell_type::STAFF_ENCHANTMENT,
            ward: false,
        })
    }

    /// The staff in an NPC's right hand as a spell to fight with, while it
    /// has the charge for a cast (a second of a concentration one).
    fn npc_staff_spell(&self, actor: FormId) -> Option<CombatSpell> {
        let (staff, ench) = self.staff_in(actor, RIGHT)?;
        let spell = self.offensive_spell(&*self.magic_item(ench)?)?;
        (self.staff_cost(actor, ench) <= self.weapon_charge(actor, staff)).then_some(spell)
    }

    /// At least one restorative effect can currently reach this actor. The
    /// ordinary effect pipeline still evaluates conditions again on landing.
    fn healing_spell_works(&self, actor: FormId, spell: FormId) -> bool {
        let Some(item) = self.magic_item(spell) else {
            return false;
        };
        let ctx = crate::condition::Context {
            subject: Some(actor),
            target: Some(actor),
            ..Default::default()
        };
        item.effects.iter().any(|ei| {
            self.magic_effect(ei.effect).is_some_and(|m| {
                restores_health(&m, ei)
                    && crate::condition::evaluate(self, &ei.conditions, ctx)
                    && crate::condition::evaluate(self, &m.conditions, ctx)
            })
        })
    }

    /// Casters' casts this frame go at their targets' bodies, paid in
    /// magicka (a landing's share of a second for concentration spells).
    pub(crate) fn npc_casts(&mut self) {
        // (caster, spell, landings, target, eye, dual, practice aim, fighting)
        let mut casts: Vec<(
            FormId,
            CombatSpell,
            u32,
            FormId,
            Vec3,
            bool,
            Option<Vec3>,
            bool,
        )> = Vec::new();
        for rt in self.cells.values_mut() {
            for a in rt.actors.iter_mut() {
                let Some(cast) = a.casts.take() else { continue };
                // Fighting (not fleeing), or practising a spell out of combat.
                let casting = match a.combat.as_ref() {
                    Some(c) => !c.fleeing,
                    None => a.magic_practising(),
                };
                if a.dead || a.bleeding.is_some() || !casting {
                    continue;
                }
                // A practice target may be a thing, not an actor.
                let practice_aim = a
                    .combat
                    .is_none()
                    .then(|| a.magic_practice.and_then(|m| m.aim))
                    .flatten();
                casts.push((
                    a.ref_id,
                    cast.spell,
                    cast.landings,
                    cast.target,
                    a.pos + Vec3::Z * 110.0 * a.scale,
                    cast.dual,
                    practice_aim,
                    a.combat.is_some(),
                ));
            }
        }
        let dual_mult = gmst_f32(&self.lo, "fMagicDualCastingCostMult", 2.8);
        for (actor, spell, n, target, eye, dual, practice_aim, fighting) in casts {
            let aim = if spell.self_target {
                Some(eye)
            } else if practice_aim.is_some() {
                practice_aim
            } else if target == PLAYER_REF {
                Some(self.player.position)
            } else {
                self.actor_ref(target)
                    .map(|t| t.pos + Vec3::Z * BODY_CENTRE * t.scale)
            };
            let Some(aim) = aim else { continue };
            let share = if spell.concentration {
                crate::ai::casters::CHANNEL_TICK
            } else {
                1.0
            };
            for _ in 0..n {
                // Several landings can be queued by a long frame; do not spend
                // another dose once an earlier one restored all its health.
                if fighting
                    && spell.self_target
                    && !spell.ward
                    && self.actor_value(actor, av::HEALTH) >= self.av_max(actor, av::HEALTH)
                {
                    break;
                }
                if spell.staff {
                    let Some((staff, ench)) = self
                        .staff_in(actor, RIGHT)
                        .filter(|&(_, e)| e == spell.spell)
                    else {
                        break;
                    };
                    let cost = self.staff_cost(actor, ench) * share;
                    if self.weapon_charge(actor, staff) < cost {
                        break;
                    }
                    self.spend_weapon_charge(actor, staff, cost);
                    let dir = (aim - eye).normalize_or_zero();
                    self.cast_from(actor, spell.spell, share, eye, dir, false);
                    continue;
                }
                let cost = self.spell_cost(actor, spell.spell) * share;
                let magicka = self.actor_value(actor, av::MAGICKA);
                // A dual cast it can't pay for goes as a single one.
                let dual = dual && magicka >= cost * dual_mult;
                let cost = if dual { cost * dual_mult } else { cost };
                if magicka < cost {
                    break;
                }
                self.damage_actor_value(actor, av::MAGICKA, cost);
                let dir = (aim - eye).normalize_or_zero();
                self.cast_from(actor, spell.spell, share, eye, dir, dual);
            }
        }
    }

    /// A spell is cast by `caster` (the player, from the view; an actor, the
    /// way it faces), already paid for; `share` is the part of a second a
    /// concentration spell's landing stands for (1 for others).
    pub(crate) fn cast(&mut self, caster: FormId, spell: FormId, share: f32) {
        self.cast_with(caster, spell, share, false);
    }

    /// A spell cast as `cast`, `dual`: dual cast.
    fn cast_with(&mut self, caster: FormId, spell: FormId, share: f32, dual: bool) {
        let (eye, dir) = if caster == PLAYER_REF {
            self.player_aim()
        } else {
            match self.actor_ref(caster) {
                Some(a) => (
                    a.pos + Vec3::Z * 110.0 * a.scale,
                    Vec3::new(a.heading.sin(), a.heading.cos(), 0.0),
                ),
                None => return,
            }
        };
        self.cast_from(caster, spell, share, eye, dir, dual);
    }

    /// A spell cast by `caster` from `eye` along `dir`.
    pub(crate) fn cast_from(
        &mut self,
        caster: FormId,
        spell: FormId,
        share: f32,
        eye: Vec3,
        dir: Vec3,
        dual: bool,
    ) {
        let Some(item) = self.magic_item(spell) else {
            return;
        };
        self.casting_breaks_invisibility(caster, &item);
        if item.cast_type != CONCENTRATION || share >= 1.0 {
            let how = if dual { " dual cast" } else { "" };
            log::info!("{caster} casts {} ({spell}){how}", item.name);
        }
        let hostile = self.item_hostile(&item);
        // Non-hostile spells teach by being cast; hostile ones by landing.
        if caster == PLAYER_REF && !hostile {
            self.magic_skill_use(&item, share);
        }
        if caster == PLAYER_REF && share >= 1.0 && item.spell_type == spell_type::SPELL {
            let mut e = crate::story::StoryEvent::new(b"CAST");
            e.refs[0] = caster;
            e.locs[0] = self.ref_current_location(caster).unwrap_or_default();
            e.form = spell;
            self.send_story_event(e);
        }
        match item.delivery {
            // A hostile spell around the caster (Battle Cry) spares them.
            delivery::SELF if hostile => {
                let at = self.feet_of(caster).unwrap_or(eye);
                self.spell_lands(caster, spell, None, at, dir, dual)
            }
            delivery::SELF => self.spell_lands(caster, spell, Some(caster), eye, dir, dual),
            delivery::CONTACT | delivery::TARGET_ACTOR => {
                let reach = if item.delivery == delivery::CONTACT {
                    CONTACT_REACH
                } else if item.range > 0.0 {
                    item.range
                } else {
                    TARGET_RANGE
                };
                // A touch reaches destructible objects too; a target actor
                // spell only actors.
                if let Some((t, Some(hit))) = self.spell_ray(eye, dir, reach, caster)
                    && (self.is_actor_ref(hit)
                        || item.delivery == delivery::CONTACT
                            && self.destructible_data(hit).is_some())
                {
                    self.spell_lands(caster, spell, Some(hit), eye + dir * t, dir, dual);
                }
            }
            _ => {
                let shot = self.item_projectile(&item).and_then(|p| {
                    crate::ai::archery::spell_projectile(&self.lo, p).map(|s| (p, s))
                });
                match shot {
                    Some((proj, shot)) if projectile_type(&self.lo, proj) & PROJ_CONE != 0 => {
                        let spread = cone_spread(&self.lo, proj);
                        for (actor, at) in self.actors_in_cone(caster, eye, dir, shot.range, spread)
                        {
                            self.spell_lands(caster, spell, Some(actor), at, dir, dual);
                        }
                    }
                    Some((proj, shot)) if reaches_at_once(&self.lo, proj) => {
                        if let Some((t, owner)) = self.spell_ray(eye, dir, shot.range, caster) {
                            self.spell_lands(caster, spell, owner, eye + dir * t, dir, dual);
                        }
                    }
                    Some((_, shot)) => {
                        let from = eye + dir * 20.0 - Vec3::Z * 6.0;
                        self.spawn_spell_projectile(caster, spell, shot, from, dir, dual);
                    }
                    None => {
                        if let Some((t, owner)) = self.spell_ray(eye, dir, LOCATION_RANGE, caster) {
                            self.spell_lands(caster, spell, owner, eye + dir * t, dir, dual);
                        }
                    }
                }
            }
        }
    }

    /// The live actors (the player too, for others) whose body's middle is
    /// within `range` of `eye` and `spread` of `dir`, with nothing solid in
    /// between: (actor, where it is struck).
    fn actors_in_cone(
        &self,
        caster: FormId,
        eye: Vec3,
        dir: Vec3,
        range: f32,
        spread: f32,
    ) -> Vec<(FormId, Vec3)> {
        let mut bodies: Vec<(FormId, Vec3)> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| !a.dead && a.ref_id != caster)
            .map(|a| (a.ref_id, a.pos + Vec3::Z * BODY_CENTRE * a.scale))
            .collect();
        if caster != PLAYER_REF {
            bodies.push((PLAYER_REF, self.player.position));
        }
        bodies
            .into_iter()
            .filter(|&(actor, at)| {
                let to = at - eye;
                let d = to.length();
                if d > range || d < 1.0 || to.normalize().angle_between(dir) > spread {
                    return false;
                }
                // Walls shelter: the first thing along the way is the actor.
                match self.physics.raycast_excluding(eye, to / d, d, caster) {
                    Some((_, owner)) => owner.is_none_or(|o| o == actor),
                    None => true,
                }
            })
            .collect()
    }

    /// What a spell reaching along `dir` from `eye` meets first within
    /// `range`: the world, an actor, or the player (who isn't in the physics
    /// world: their capsule) for anyone else's.
    fn spell_ray(
        &self,
        eye: Vec3,
        dir: Vec3,
        range: f32,
        caster: FormId,
    ) -> Option<(f32, Option<FormId>)> {
        let world = self.physics.raycast_excluding(eye, dir, range, caster);
        if caster == PLAYER_REF {
            return world;
        }
        let (r, h) = (self.physics.player_radius, self.physics.player_half_height);
        let p = self.player.position;
        let (s, d) = crate::ai::archery::segment_distance(
            eye,
            eye + dir * range,
            p - Vec3::Z * h,
            p + Vec3::Z * h,
        );
        match world {
            _ if d >= r => world,
            Some((t, _)) if t < s * range => world,
            _ => Some((s * range, Some(PLAYER_REF))),
        }
    }

    /// Where an actor (or the player) stands.
    fn feet_of(&self, actor: FormId) -> Option<Vec3> {
        if actor == PLAYER_REF {
            return Some(self.player.position);
        }
        self.actor_ref(actor).map(|a| a.pos)
    }

    /// Whether a reference is an actor (placed or made while playing).
    pub(crate) fn is_actor_ref(&self, r: FormId) -> bool {
        r == PLAYER_REF
            || self.created(r).map_or_else(
                || self.lo.tag_of(r).is_some_and(|t| t.0 == *b"ACHR"),
                |c| c.actor,
            )
    }

    /// Whether any of a spell's effects is hostile.
    pub(crate) fn item_hostile(&self, item: &MagicItem) -> bool {
        item.effects.iter().any(|e| {
            self.magic_effect(e.effect)
                .is_some_and(|m| m.has(flags::HOSTILE))
        })
    }

    /// Health a spell's damaging effects in `mask` (bit `i` for its `i`th
    /// effect; `u64::MAX` for all) deal to an object they reach. Objects have
    /// no resistances, so the effects' magnitudes are taken as they are.
    fn object_damage(&self, item: &MagicItem, mask: u64) -> f32 {
        item.effects
            .iter()
            .enumerate()
            .filter(|&(i, _)| i >= 64 || mask & (1u64 << i) != 0)
            .filter(|&(_, e)| {
                self.magic_effect(e.effect)
                    .is_some_and(|m| damages_health(&m, e))
            })
            .map(|(_, e)| e.magnitude)
            .sum()
    }

    /// The projectile a spell flies as: its first effect's that has one.
    pub(crate) fn item_projectile(&self, item: &MagicItem) -> Option<FormId> {
        item.effects.iter().find_map(|e| {
            self.magic_effect(e.effect)
                .map(|m| m.projectile)
                .filter(|p| !p.is_null())
        })
    }

    /// A spell lands at `at` (flying along `dir`): on the reference struck, and
    /// on every actor within its area; its impact there. An object struck or
    /// in the area takes the health damage of the hostile effects, and nothing
    /// else. `dual`: it was dual cast.
    pub(crate) fn spell_lands(
        &mut self,
        caster: FormId,
        spell: FormId,
        target: Option<FormId>,
        at: Vec3,
        dir: Vec3,
        dual: bool,
    ) {
        let Some(item) = self.magic_item(spell) else {
            return;
        };
        let object = target.filter(|&t| !self.is_actor_ref(t));
        let target = target.filter(|_| object.is_none());
        if target != Some(caster)
            && let Some(ipds) = item
                .effects
                .iter()
                .find_map(|e| self.magic_effect(e.effect).map(|m| m.impacts))
                .filter(|i| !i.is_null())
        {
            self.impact_at(ipds, target, at, dir);
        }
        // Summons are the caster's effects, bringing their actor where the
        // spell lands.
        if item.effects.iter().any(|e| {
            self.magic_effect(e.effect)
                .is_some_and(|m| m.archetype == crate::magic::archetype::SUMMON_CREATURE)
        }) {
            self.apply_cast(spell, caster, caster, Some(at), dual);
            return;
        }
        // The one struck takes every effect; the others within an effect's
        // area that effect (bit `i` of the mask for the `i`th effect).
        let mut struck: Vec<(FormId, Option<u64>)> =
            target.map(|t| (t, None)).into_iter().collect();
        // Objects damaged: the one struck takes all its effects' damage.
        let mut damaged: Vec<(FormId, f32)> = object
            .map(|o| (o, self.object_damage(&item, u64::MAX)))
            .into_iter()
            .collect();
        let radii: Vec<f32> = item
            .effects
            .iter()
            .map(|e| e.area as f32 * UNITS_PER_FOOT)
            .collect();
        let radius = radii.iter().copied().fold(0.0, f32::max);
        if radius > 0.0 {
            // The blast reaches what it can see from where it went off:
            // walls and bodies (the struck one's too) shelter those behind.
            let in_sight = |to: Vec3, actor: FormId| {
                let d = to.distance(at);
                d < 1.0
                    || match self.physics.raycast_excluding(at, (to - at) / d, d, caster) {
                        Some((_, owner)) => owner == Some(actor),
                        None => true,
                    }
            };
            // The effects (bit `i` for the `i`th) a point `d` from the blast reaches.
            let mask_at = |d: f32| {
                radii
                    .iter()
                    .enumerate()
                    .filter(|&(i, &r)| i < 64 && r > 0.0 && d <= r)
                    .fold(0u64, |m, (i, _)| m | 1 << i)
            };
            let near = self
                .cells
                .values()
                .flat_map(|rt| &rt.actors)
                .filter(|a| !a.dead && a.ref_id != caster)
                .map(|a| (a.ref_id, a.pos + Vec3::Z * BODY_CENTRE * a.scale));
            let player = (caster != PLAYER_REF).then_some((PLAYER_REF, self.player.position));
            let reached: Vec<(FormId, u64)> = near
                .chain(player)
                .filter(|&(id, _)| target != Some(id))
                .filter_map(|(id, mid)| {
                    let mask = mask_at(mid.distance(at));
                    (mask != 0 && in_sight(mid, id)).then_some((id, mask))
                })
                .collect();
            struck.extend(reached.into_iter().map(|(id, mask)| (id, Some(mask))));
            // Loaded destructible objects in the area, by their placed position.
            let objects: Vec<(FormId, u64)> = self
                .cells
                .values()
                .flat_map(|rt| &rt.refs)
                .copied()
                .filter(|&r| Some(r) != object && self.destructible_data(r).is_some())
                .filter_map(|r| {
                    let p = self.ref_position(r)?;
                    let mask = mask_at(p.distance(at));
                    (mask != 0 && in_sight(p, r)).then_some((r, mask))
                })
                .collect();
            damaged.extend(
                objects
                    .into_iter()
                    .map(|(r, mask)| (r, self.object_damage(&item, mask))),
            );
        }
        for (reference, damage) in damaged {
            self.damage_object_by_blow(reference, damage);
        }
        let hostile = self.item_hostile(&item);
        let share = if item.cast_type == CONCENTRATION {
            CONCENTRATION_TICK
        } else {
            1.0
        };
        for (t, effects) in struck {
            let landed = match effects {
                None => self.apply_cast(spell, caster, t, None, dual),
                Some(mask) => self.apply_cast_effects(spell, caster, t, dual, mask),
            };
            if landed && hostile && caster == PLAYER_REF && t != PLAYER_REF {
                self.magic_skill_use(&item, share);
            }
        }
    }

    /// The player cast a spell (or a hostile one landed): its school's skill
    /// gets the spell's base cost as XP (`share` of it for a concentration
    /// spell's landing), times its costliest effect's skill usage multiplier.
    fn magic_skill_use(&mut self, item: &MagicItem, share: f32) {
        if item.spell_type != spell_type::SPELL {
            return;
        }
        let Some(school) = self.spell_school(item.id) else {
            return;
        };
        let base = self
            .lo
            .get(item.id)
            .and_then(|r| {
                r.get(b"SPIT")
                    .filter(|d| d.len() >= 4)
                    .map(|d| u32::from_le_bytes(d[0..4].try_into().unwrap()) as f32)
            })
            .unwrap_or(0.0);
        let mult = item
            .effects
            .iter()
            .filter_map(|e| self.magic_effect(e.effect))
            .find(|m| m.skill == Some(school))
            .map_or(1.0, |m| m.skill_usage_mult);
        self.use_skill(school, base * mult * share);
    }
}

/// A projectile's type flags (`PROJ` `DATA`).
fn projectile_type(lo: &esp::LoadOrder, proj: FormId) -> u16 {
    lo.get(proj)
        .and_then(|r| {
            r.get(b"DATA")
                .filter(|d| d.len() >= 4)
                .map(|d| u16::from_le_bytes([d[2], d[3]]))
        })
        .unwrap_or(0)
}

/// Whether a projectile reaches its range at once (flames, beams, cones)
/// rather than flying.
fn reaches_at_once(lo: &esp::LoadOrder, proj: FormId) -> bool {
    projectile_type(lo, proj) & (PROJ_BEAM | PROJ_FLAME | PROJ_CONE) != 0
}

/// A cone projectile's spread (`PROJ` `DATA` 0x44, degrees), as radians
/// either side of its middle.
fn cone_spread(lo: &esp::LoadOrder, proj: FormId) -> f32 {
    lo.get(proj)
        .and_then(|r| {
            r.get(b"DATA")
                .filter(|d| d.len() >= 0x48)
                .map(|d| f32::from_le_bytes(d[0x44..0x48].try_into().unwrap()))
        })
        .filter(|s| *s > 0.0)
        .unwrap_or(15.0)
        .to_radians()
}

#[cfg(test)]
mod npc_healing_tests {
    use super::*;

    #[test]
    fn emergency_healing_starts_below_half_and_ends_at_full_health() {
        assert!(!needs_combat_healing(100.0, 100.0, false));
        assert!(!needs_combat_healing(50.0, 100.0, false));
        assert!(needs_combat_healing(49.0, 100.0, false));
        assert!(needs_combat_healing(75.0, 100.0, true));
        assert!(!needs_combat_healing(100.0, 100.0, true));
        assert!(!needs_combat_healing(0.0, 100.0, true));
        assert!(!needs_combat_healing(-1.0, 100.0, false));
        // The current maximum includes fortification and actor-value changes.
        assert!(needs_combat_healing(75.0, 200.0, false));
    }

    #[test]
    fn restorative_candidates_exclude_fortification_damage_and_absorption() {
        let item = EffectItem {
            effect: FormId(1),
            magnitude: 10.0,
            area: 0,
            duration: 1,
            conditions: Vec::new(),
        };
        let effect = MagicEffect {
            archetype: archetype::VALUE_MODIFIER,
            primary: Some(av::HEALTH),
            ..Default::default()
        };
        assert!(restores_health(&effect, &item));
        for flags in [flags::RECOVER, flags::DETRIMENTAL, flags::HOSTILE] {
            assert!(!restores_health(
                &MagicEffect {
                    flags,
                    ..effect.clone()
                },
                &item
            ));
        }
        assert!(!restores_health(
            &MagicEffect {
                archetype: archetype::ABSORB,
                ..effect.clone()
            },
            &item
        ));
        assert!(!restores_health(
            &MagicEffect {
                primary: Some(av::STAMINA),
                ..effect
            },
            &item
        ));
        let dual = MagicEffect {
            archetype: archetype::DUAL_VALUE_MODIFIER,
            primary: Some(av::STAMINA),
            secondary: Some(av::HEALTH),
            second_weight: 0.5,
            ..Default::default()
        };
        assert!(restores_health(&dual, &item));
        assert!(!restores_health(
            &MagicEffect {
                second_weight: 0.0,
                ..dual
            },
            &item
        ));
        assert!(!restores_health(
            &MagicEffect {
                primary: Some(av::HEALTH),
                ..Default::default()
            },
            &EffectItem {
                magnitude: 0.0,
                ..item
            }
        ));
    }
}

#[cfg(test)]
mod cast_start_tests {
    use super::*;

    fn spell(cast_type: u32, delivery: u32) -> MagicItem {
        MagicItem {
            cast_type,
            delivery,
            ..Default::default()
        }
    }

    #[test]
    fn hands_start_the_spells_kind_of_cast() {
        let ff = 1; // fire and forget
        // Firebolt, Flames, Fast Healing, Healing, Healing Hands.
        assert_eq!(
            cast_start_event(&spell(ff, delivery::AIMED), Some(0)),
            "SpellAimedStart"
        );
        assert_eq!(
            cast_start_event(&spell(CONCENTRATION, delivery::AIMED), Some(0)),
            "SpellAimedConcentrationStart"
        );
        assert_eq!(
            cast_start_event(&spell(ff, delivery::SELF), Some(0)),
            "SpellSelfStart"
        );
        assert_eq!(
            cast_start_event(&spell(CONCENTRATION, delivery::SELF), Some(0)),
            "SpellSelfConcentrationStart"
        );
        assert_eq!(
            cast_start_event(&spell(CONCENTRATION, delivery::TARGET_ACTOR), Some(0)),
            "SpellAimedConcentrationStart"
        );
        // Lesser Ward and Telekinesis are self-delivered concentration spells.
        let ward = Some(archetype::ACCUMULATE_MAGNITUDE);
        assert_eq!(
            cast_start_event(&spell(CONCENTRATION, delivery::SELF), ward),
            "WardStart"
        );
        let tk = Some(archetype::TELEKINESIS);
        assert_eq!(
            cast_start_event(&spell(CONCENTRATION, delivery::SELF), tk),
            "SpellTelekinesisStart"
        );
    }
}

#[cfg(test)]
mod story_cast_tests {
    use super::*;
    use esp::writer::{Node, RecordBuilder, write_plugin};

    #[test]
    #[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
    fn cast_location_reaches_story_aliases_and_papyrus_and_is_not_stale() {
        const SPELL: FormId = FormId(0x800);
        const QUEST: FormId = FormId(0x810);
        const CELL: FormId = FormId(0x820);
        const LOCATION: FormId = FormId(0x821);

        struct Fixture(std::path::PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = Fixture(
            std::env::temp_dir().join(format!("vrm-story-cast-{}-{stamp}", std::process::id())),
        );
        std::fs::create_dir(&dir.0).unwrap();
        let mut spit = [0; 36];
        spit[16..20].copy_from_slice(&1u32.to_le_bytes()); // fire and forget
        let records = [
            Node::top(
                b"SPEL",
                vec![RecordBuilder::new(b"SPEL", SPELL).field(b"SPIT", &spit)],
            ),
            Node::top(
                b"SMEN",
                vec![RecordBuilder::new(b"SMEN", FormId(0x830)).field(b"ENAM", b"CAST")],
            ),
            Node::top(
                b"SMQN",
                vec![
                    RecordBuilder::new(b"SMQN", FormId(0x831))
                        .u32(b"PNAM", 0x830)
                        .u32(b"NNAM", QUEST.0),
                ],
            ),
            Node::top(
                b"QUST",
                vec![
                    RecordBuilder::new(b"QUST", QUEST)
                        .u32(b"ALLS", 0)
                        .u32(b"FNAM", crate::aliases::flags::OPTIONAL)
                        .field(b"ALFE", b"CAST")
                        .field(b"ALFD", b"L1")
                        .field(b"ALED", &[]),
                ],
            ),
            Node::top(
                b"CELL",
                vec![RecordBuilder::new(b"CELL", CELL).u32(b"XLCN", LOCATION.0)],
            ),
            Node::top(b"LCTN", vec![RecordBuilder::new(b"LCTN", LOCATION)]),
        ];
        std::fs::write(
            dir.0.join("StoryCast.esp"),
            write_plugin(
                0,
                &[],
                "test",
                "synthetic cast story event",
                0x900,
                &records,
            ),
        )
        .unwrap();
        let mut engine = crate::stats::action_tests::headless_engine(&dir.0, "StoryCast.esp");
        engine.location = crate::engine::Location::Interior(CELL);

        // Exercise actual casting, Story Manager traversal, a from-event
        // location alias, and the argument list delivered to quest scripts.
        engine.cast(PLAYER_REF, SPELL, 1.0);
        let state = &engine.scripts.quests[&QUEST];
        assert!(state.running);
        assert_eq!(state.aliases.get(&0), Some(&LOCATION));
        let event = state.event.as_ref().unwrap();
        assert_eq!(event.refs, [PLAYER_REF, FormId::NULL]);
        assert_eq!(event.locs, [LOCATION, FormId::NULL]);
        assert_eq!(event.form, SPELL);
        let (name, args) = engine.story_papyrus_event(event).unwrap();
        assert_eq!(name, "OnStoryCastMagic");
        assert_eq!(args.len(), 4);
        assert!(matches!(args[1], papyrus::Value::None));
        assert_eq!(args[2].as_form(), Some(LOCATION.0));
        assert_eq!(args[3].as_form(), Some(SPELL.0));

        // A new cast with no location carries None, never the old location.
        engine.stop_quest(QUEST);
        engine.location = crate::engine::Location::Nowhere;
        engine.cast(PLAYER_REF, SPELL, 1.0);
        let state = &engine.scripts.quests[&QUEST];
        assert!(state.running);
        assert!(!state.aliases.contains_key(&0));
        let event = state.event.as_ref().unwrap();
        assert_eq!(event.locs, [FormId::NULL; 2]);
        let (_, args) = engine.story_papyrus_event(event).unwrap();
        assert!(matches!(args[2], papyrus::Value::None));
    }
}

#[cfg(test)]
#[path = "casting_scroll_tests.rs"]
mod scroll_tests;

#[cfg(test)]
#[path = "casting_condition_tests.rs"]
mod condition_tests;
