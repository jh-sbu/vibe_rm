//! Casters in combat: an actor whose combat style favours magic and that has
//! a spell to throw (and the magicka for it) keeps its distance, faces its
//! target and casts at it. Injured fighters can instead heal themselves,
//! without needing a clear line to their enemy. The engine casts the spell
//! (`crate::casting`). Out of magicka it fights as the others do.

use esp::FormId;
use glam::Vec3;

use super::{ActorRuntime, State, World, uniform};
use crate::casting::hand_prefix;
use crate::world::inventory::{LEFT, RIGHT};

/// A spell an actor fights with: how far it reaches, and whether it is
/// channelled (concentration) rather than thrown.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CombatSpell {
    pub spell: FormId,
    pub range: f32,
    pub concentration: bool,
    /// A restorative spell delivered to the caster, rather than its enemy.
    pub self_target: bool,
    /// The hand's kind of cast in the graph (`SpellAimedStart`...:
    /// `crate::casting::cast_start_event`), and its charge time (`SPIT`).
    pub start: &'static str,
    pub charge: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::{Clips, combat::Combat, furniture::FurnitureWorld, nav::NavWorld};
    use crate::world::animation::{AnimationLibrary, BehaviorLibrary};
    use crate::world::skeleton::Skeleton;
    use glam::Mat4;
    use std::collections::HashMap;
    use std::sync::Arc;

    fn actor() -> ActorRuntime {
        let mut actor = ActorRuntime::new(
            FormId(1),
            FormId(2),
            Arc::new(Skeleton::default()),
            Mat4::IDENTITY,
            Vec::new(),
            0.0,
        );
        let mut combat = Combat::new(FormId(3));
        combat.cooldown = 0.0;
        actor.combat = Some(combat);
        actor
    }

    fn with_world(test: impl FnOnce(&mut World<'_>)) {
        let nav = NavWorld::default();
        let mut furniture = FurnitureWorld::default();
        let vfs = vfs::Vfs::new("/nonexistent-vrm-npc-healing-fixture", &[]);
        let mut anims = AnimationLibrary::default();
        let mut behaviors = BehaviorLibrary::default();
        let mut world = World {
            nav: &nav,
            furniture: &mut furniture,
            clips: Clips {
                vfs: &vfs,
                anims: &mut anims,
                behaviors: &mut behaviors,
            },
            may_use: &|_, _, _| false,
            claimed: &HashMap::new(),
            patrols: &HashMap::new(),
            targets: &HashMap::new(),
            holds: &|_, _| false,
            rand: &mut || 1,
        };
        test(&mut world);
    }

    fn healing(concentration: bool) -> CombatSpell {
        CombatSpell {
            spell: FormId(4),
            range: 0.0,
            concentration,
            self_target: true,
            start: "SpellSelfStart",
            charge: 0.0,
        }
    }

    #[test]
    fn self_heal_ignores_enemy_distance_sight_and_facing_and_precedes_bow() {
        with_world(|world| {
            let mut actor = actor();
            actor.bow = true;
            let spell = healing(false);
            actor.combat_spell = Some(spell);
            let target = Vec3::new(0.0, -50_000.0, 0.0);
            assert_eq!(
                actor.combat_step(0.01, world, target).as_deref(),
                Some("BeginCastRight")
            );
            assert_eq!(actor.heading, 0.0);
            assert_eq!(actor.speed, 0.0);
            let cast = actor.casts.unwrap();
            assert_eq!(cast.spell, spell);
            assert_eq!(cast.target, actor.ref_id);
            assert_eq!(cast.landings, 1);
        });
    }

    #[test]
    fn concentration_progresses_in_quarter_second_landings_then_stops() {
        with_world(|world| {
            let mut actor = actor();
            let spell = healing(true);
            assert_eq!(
                actor
                    .caster_step(0.1, world, Vec3::ZERO, 0.0, spell)
                    .as_deref(),
                Some("BeginCastRight")
            );
            assert!(actor.casts.is_none());
            actor.combat.as_mut().unwrap().channel = 1.0;
            assert!(
                actor
                    .caster_step(0.5, world, Vec3::ZERO, 0.0, spell)
                    .is_none()
            );
            assert_eq!(actor.casts.take().unwrap().landings, 2);
            assert_eq!(actor.combat.as_ref().unwrap().channel, 0.5);
            // Its end is sent to the graph by the caster (none here).
            assert!(
                actor
                    .caster_step(1.0, world, Vec3::ZERO, 0.0, spell)
                    .is_none()
            );
            assert_eq!(actor.casts.take().unwrap().landings, 2);
            assert_eq!(actor.combat.as_ref().unwrap().channel, 0.0);
            assert!(actor.combat.as_ref().unwrap().cooldown > 0.0);
        });
    }

    #[test]
    fn queued_cast_keeps_original_spell_and_recipient_after_selection_changes() {
        with_world(|world| {
            let mut actor = actor();
            let spell = healing(false);
            actor.caster_step(0.1, world, Vec3::ZERO, 0.0, spell);
            actor.combat_spell = None;
            actor.combat.as_mut().unwrap().target = FormId(99);
            let queued = actor.casts.take().unwrap();
            assert_eq!(queued.spell, spell);
            assert_eq!(queued.target, actor.ref_id);
        });
    }

    #[test]
    fn a_thrown_spell_is_cast_at_its_spell_fire_and_ends_with_its_clip() {
        with_world(|world| {
            let mut actor = actor();
            let spell = healing(false);
            let hand_cast = |phase| HandCast {
                hand: LEFT,
                phase,
                t: 0.0,
                cast: CombatCast {
                    spell,
                    target: FormId(1),
                    landings: 1,
                },
            };
            actor.combat.as_mut().unwrap().hand_cast = Some(hand_cast(HandPhase::Released));
            assert!(
                actor
                    .caster_step(0.1, world, Vec3::ZERO, 0.0, spell)
                    .is_none()
            );
            assert!(actor.casts.is_none());
            // The other hand's annotation isn't this one's.
            actor.spell_fire[RIGHT] = true;
            actor.caster_step(0.1, world, Vec3::ZERO, 0.0, spell);
            assert!(actor.casts.is_none());
            actor.spell_fire[LEFT] = true;
            actor.caster_step(0.1, world, Vec3::ZERO, 0.0, spell);
            assert_eq!(actor.casts.take().unwrap().target, FormId(1));
            let c = actor.combat.as_ref().unwrap();
            assert_eq!(c.hand_cast.unwrap().phase, HandPhase::Fired);
            actor.hand_equipped[LEFT] = true;
            actor.caster_step(0.1, world, Vec3::ZERO, 0.0, spell);
            let c = actor.combat.as_ref().unwrap();
            assert!(c.hand_cast.is_none());
            assert!(c.cooldown > 0.0);

            // No SpellFire at all: nothing is cast.
            actor.combat.as_mut().unwrap().hand_cast = Some(hand_cast(HandPhase::Released));
            for _ in 0..20 {
                actor.caster_step(0.1, world, Vec3::ZERO, 0.0, spell);
            }
            assert!(actor.casts.is_none());
            assert!(actor.combat.as_ref().unwrap().hand_cast.is_none());
        });
    }

    #[test]
    fn offensive_spell_still_requires_a_clear_shot() {
        with_world(|world| {
            let mut actor = actor();
            let spell = CombatSpell {
                range: 1000.0,
                self_target: false,
                ..healing(false)
            };
            assert!(
                actor
                    .caster_step(0.1, world, Vec3::Y * 100.0, 100.0, spell)
                    .is_none()
            );
            assert!(actor.casts.is_none());
            actor.combat.as_mut().unwrap().clear_shot = true;
            assert_eq!(
                actor
                    .caster_step(0.1, world, Vec3::Y * 100.0, 100.0, spell)
                    .as_deref(),
                Some("BeginCastRight")
            );
            assert_eq!(actor.casts.take().unwrap().target, FormId(3));
        });
    }

    #[test]
    fn healing_cancels_a_pending_bow_draw_and_archery_starts_fresh() {
        use crate::ai::archery::Draw;
        for draw in [Draw::Drawing(1.0, 1.1), Draw::Loosed(0.2, false)] {
            with_world(|world| {
                let mut actor = actor();
                actor.bow = true;
                actor.combat_spell = Some(healing(false));
                actor.combat.as_mut().unwrap().draw = draw;
                actor.arrow_release = true;
                actor.loose = true;
                let target = Vec3::Y * 1000.0;
                assert_eq!(
                    actor.combat_step(0.01, world, target).as_deref(),
                    Some("attackStop")
                );
                assert_eq!(actor.combat.as_ref().unwrap().draw, Draw::Idle);
                assert!(!actor.arrow_release);
                assert!(!actor.loose);
                assert!(actor.casts.is_none());
                assert_eq!(
                    actor.combat_step(0.01, world, target).as_deref(),
                    Some("BeginCastRight")
                );
                actor.casts.take();
                actor.combat_spell = None;
                let combat = actor.combat.as_mut().unwrap();
                combat.cooldown = 0.0;
                combat.clear_shot = true;
                assert_eq!(
                    actor.combat_step(0.01, world, target).as_deref(),
                    Some("bowAttackStart")
                );
                assert!(matches!(
                    actor.combat.as_ref().unwrap().draw,
                    Draw::Drawing(0.0, _)
                ));
                assert!(!actor.loose);
            });
        }
    }
}

/// Landings already requested by the AI keep the spell and recipient selected
/// at that moment, even if a blow or spell changes the fight before dispatch.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CombatCast {
    pub spell: CombatSpell,
    pub target: FormId,
    pub landings: u32,
}

/// Seconds between a caster's throws, and how long it channels a
/// concentration spell (chosen by eye; no source).
const THROW_COOLDOWN: (f32, f32) = (1.5, 3.0);
const CHANNEL_TIME: (f32, f32) = (1.5, 3.0);
/// Seconds between a channelled spell's landings (as the player's).
pub(crate) const CHANNEL_TICK: f32 = 0.25;

/// How long a spell let go may wait for its release clip's `SpellFire`,
/// and then for the clip to end (as the player's).
const FIRE_TIMEOUT: f32 = 1.5;
/// Seconds before a caster whose graph won't start a cast tries again.
const REFUSED_RETRY: f32 = 0.5;

/// Where a hand casting through the graph is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum HandPhase {
    /// Started: a fire-and-forget spell charging, a concentration spell
    /// winding up.
    Charging,
    /// Charged (`SpellReady`), to be let go next.
    Ready,
    /// Let go (`SpellRelease`), waiting for its `SpellFire`.
    Released,
    /// Cast, its release clip playing out.
    Fired,
    /// A concentration spell flowing (`SpellRelease`).
    Flowing,
}

/// A spell cast with a hand in the caster's graph; a thrown one keeps the
/// recipient it was started at.
#[derive(Debug, Clone, Copy)]
pub(crate) struct HandCast {
    pub hand: usize,
    pub phase: HandPhase,
    /// Seconds in this phase.
    pub t: f32,
    pub cast: CombatCast,
}

impl ActorRuntime {
    /// A caster's turn: close in until within the spell's range with a clear
    /// line, then stand facing the target and cast: a throw now and then, or
    /// a concentration spell channelled a while. Humanoids cast with a hand
    /// in their graph (`crate::casting` has the events), once their hands
    /// are drawn; a throw is cast at its release clip's `SpellFire`. Returns
    /// `BeginCastRight` / `BeginCastLeft` when a cast starts (already sent).
    pub(crate) fn caster_step(
        &mut self,
        dt: f32,
        w: &mut World,
        target: Vec3,
        dist: f32,
        spell: CombatSpell,
    ) -> Option<String> {
        if let Some(h) = self.combat.as_ref()?.hand_cast
            && !h.cast.spell.concentration
        {
            self.throw_step(dt, w, target, h);
            return None;
        }
        let run = self.run_speed();
        let c = self.combat.as_mut()?;
        let in_range = spell.self_target || (dist < spell.range * 0.85 && c.clear_shot);
        if !in_range {
            if c.channel > 0.0 {
                c.channel = 0.0;
                self.stop_hand_cast();
            }
            self.chase(dt, w, target, run);
            return None;
        }
        self.speed = 0.0;
        self.state = State::Idle(1.0);
        let to = (target - self.pos).normalize_or_zero();
        let off = if spell.self_target {
            0.0
        } else {
            self.turn_towards(to, dt).abs()
        };
        let c = self.combat.as_mut()?;
        let recipient = if spell.self_target {
            self.ref_id
        } else {
            c.target
        };
        if c.channel > 0.0 {
            // Flowing in the hand once its state machine runs.
            if let Some(h) = c.hand_cast.as_mut()
                && h.phase == HandPhase::Charging
            {
                h.phase = HandPhase::Flowing;
                let ev = format!("{}_SpellRelease_Event", hand_prefix(h.hand));
                self.graph_event(&ev, &mut w.clips);
            }
            let c = self.combat.as_mut()?;
            // A landing each tick while it lasts.
            let before = (c.channel / CHANNEL_TICK).ceil();
            c.channel -= dt;
            let after = (c.channel.max(0.0) / CHANNEL_TICK).ceil();
            let landings = (before - after) as u32;
            self.casts = Some(CombatCast {
                spell,
                target: recipient,
                landings,
            });
            if c.channel <= 0.0 {
                c.channel = 0.0;
                c.cooldown = uniform(w.rand, THROW_COOLDOWN.0, THROW_COOLDOWN.1);
                self.stop_hand_cast();
            }
            return None;
        }
        if c.cooldown > 0.0 || off > 0.2 {
            return None;
        }
        let cast = CombatCast {
            spell,
            target: recipient,
            landings: 1,
        };
        let hand = self.cast_hand.unwrap_or(RIGHT);
        let humanoid = self.graph.as_ref().is_some_and(|g| g.project().humanoid());
        if humanoid && !self.start_hand_cast(hand, cast, w) {
            // Not drawn yet, or busy (a stagger): again shortly.
            if let Some(c) = self.combat.as_mut() {
                c.cooldown = REFUSED_RETRY;
            }
            return None;
        }
        let c = self.combat.as_mut()?;
        if spell.concentration {
            c.channel = uniform(w.rand, CHANNEL_TIME.0, CHANNEL_TIME.1);
        } else if !humanoid {
            c.cooldown = uniform(w.rand, THROW_COOLDOWN.0, THROW_COOLDOWN.1);
            self.casts = Some(cast);
        }
        Some(
            if hand == LEFT {
                "BeginCastLeft"
            } else {
                "BeginCastRight"
            }
            .into(),
        )
    }

    /// Into the graph's casting state (ignored there already), then the
    /// hand's kind of cast. False if the graph won't.
    fn start_hand_cast(&mut self, hand: usize, cast: CombatCast, w: &mut World) -> bool {
        if !self.drawn || !self.weapon_out {
            return false;
        }
        let begin = if hand == LEFT {
            "BeginCastLeft"
        } else {
            "BeginCastRight"
        };
        let began = self.graph_event(begin, &mut w.clips);
        let start = format!("{}_{}", hand_prefix(hand), cast.spell.start);
        if !self.graph_event(&start, &mut w.clips) {
            log::debug!("{}: the graph won't take {start}", self.ref_id);
            if began {
                self.graph_event("CastStop", &mut w.clips);
            }
            return false;
        }
        self.spell_fire = [false; 2];
        self.hand_equipped = [false; 2];
        if let Some(c) = self.combat.as_mut() {
            c.hand_cast = Some(HandCast {
                hand,
                phase: HandPhase::Charging,
                t: 0.0,
                cast,
            });
        }
        true
    }

    /// A thrown spell through the graph: charged for its charge time, let
    /// go, cast at its hand's `SpellFire`, done when its release clip ends.
    fn throw_step(&mut self, dt: f32, w: &mut World, target: Vec3, mut h: HandCast) {
        self.speed = 0.0;
        self.state = State::Idle(1.0);
        if !h.cast.spell.self_target {
            self.turn_towards((target - self.pos).normalize_or_zero(), dt);
        }
        h.t += dt;
        let p = hand_prefix(h.hand);
        let fired = std::mem::take(&mut self.spell_fire[h.hand]);
        let ended = std::mem::take(&mut self.hand_equipped[h.hand]);
        let next = match h.phase {
            HandPhase::Charging if h.t >= h.cast.spell.charge => {
                self.graph_event(&format!("{p}_SpellReady_Event"), &mut w.clips);
                Some(HandPhase::Ready)
            }
            HandPhase::Ready => {
                if self.graph_event(&format!("{p}_SpellRelease_Event"), &mut w.clips) {
                    Some(HandPhase::Released)
                } else {
                    return self.end_throw(w);
                }
            }
            HandPhase::Released if fired => {
                self.casts = Some(h.cast);
                Some(HandPhase::Fired)
            }
            HandPhase::Released if h.t >= FIRE_TIMEOUT => {
                log::debug!("{}: its release never reached its SpellFire", self.ref_id);
                return self.end_throw(w);
            }
            HandPhase::Fired if ended || h.t >= FIRE_TIMEOUT => return self.end_throw(w),
            _ => None,
        };
        if let Some(phase) = next {
            h.phase = phase;
            h.t = 0.0;
        }
        if let Some(c) = self.combat.as_mut() {
            c.hand_cast = Some(h);
        }
    }

    fn end_throw(&mut self, w: &mut World) {
        self.stop_hand_cast();
        if let Some(c) = self.combat.as_mut() {
            c.cooldown = uniform(w.rand, THROW_COOLDOWN.0, THROW_COOLDOWN.1);
        }
    }

    /// The hand stops casting in the graph: back to ready (its own
    /// `Equipped` event, as for the player), and out of the casting state.
    pub(crate) fn stop_hand_cast(&mut self) {
        let Some(h) = self.combat.as_mut().and_then(|c| c.hand_cast.take()) else {
            return;
        };
        if let Some(g) = self.graph.as_mut() {
            g.send_event(&format!("{}_Equipped_Event", hand_prefix(h.hand)));
            g.send_event("CastStop");
        }
    }

    /// The graph's hand types, with the hand an actor casts with (set by the
    /// engine while it fights with a spell) holding a spell: the left beside
    /// a one-handed weapon, else both (only the left hand's spell state
    /// reaches the magic behaviour).
    pub(crate) fn set_hand_vars(&mut self, vars: [(&'static str, f32); 4]) {
        let Some(g) = self.graph.as_mut() else {
            return;
        };
        for (var, value) in vars {
            g.set_variable(var, value);
        }
        let spell = crate::world::inventory::HandType::Spell.for_graph();
        match self.cast_hand {
            Some(LEFT) => {
                g.set_variable("iLeftHandType", spell);
            }
            Some(_) => {
                g.set_variable("iRightHandType", spell);
                g.set_variable("iLeftHandType", spell);
            }
            None => {}
        }
    }
}
