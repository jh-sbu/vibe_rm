//! Casters in combat: an actor whose combat style favours magic and that has
//! a spell to throw (and the magicka for it) keeps its distance, faces its
//! target and casts at it. Injured fighters can instead heal themselves,
//! without needing a clear line to their enemy. The engine casts the spell
//! (`crate::casting`). Out of magicka it fights as the others do.

use esp::FormId;
use glam::Vec3;

use super::{ActorRuntime, State, World, uniform};

/// A spell an actor fights with: how far it reaches, and whether it is
/// channelled (concentration) rather than thrown.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CombatSpell {
    pub spell: FormId,
    pub range: f32,
    pub concentration: bool,
    /// A restorative spell delivered to the caster, rather than its enemy.
    pub self_target: bool,
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
            assert_eq!(
                actor
                    .caster_step(1.0, world, Vec3::ZERO, 0.0, spell)
                    .as_deref(),
                Some("CastStop")
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

impl ActorRuntime {
    /// A caster's turn: close in until within the spell's range with a clear
    /// line, then stand facing the target and cast: a throw now and then, or
    /// a concentration spell channelled a while. Returns the graph event to
    /// send.
    pub(crate) fn caster_step(
        &mut self,
        dt: f32,
        w: &mut World,
        target: Vec3,
        dist: f32,
        spell: CombatSpell,
    ) -> Option<String> {
        let run = self.run_speed();
        let c = self.combat.as_mut()?;
        let in_range = spell.self_target || (dist < spell.range * 0.85 && c.clear_shot);
        if !in_range {
            if c.channel > 0.0 {
                c.channel = 0.0;
                return Some("CastStop".into());
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
        if c.channel > 0.0 {
            // A landing each tick while it lasts.
            let before = (c.channel / CHANNEL_TICK).ceil();
            c.channel -= dt;
            let after = (c.channel.max(0.0) / CHANNEL_TICK).ceil();
            let landings = (before - after) as u32;
            let target = if spell.self_target {
                self.ref_id
            } else {
                c.target
            };
            self.casts = Some(CombatCast {
                spell,
                target,
                landings,
            });
            if c.channel <= 0.0 {
                c.channel = 0.0;
                c.cooldown = uniform(w.rand, THROW_COOLDOWN.0, THROW_COOLDOWN.1);
                return Some("CastStop".into());
            }
            return None;
        }
        if c.cooldown > 0.0 || off > 0.2 {
            return None;
        }
        if spell.concentration {
            c.channel = uniform(w.rand, CHANNEL_TIME.0, CHANNEL_TIME.1);
        } else {
            c.cooldown = uniform(w.rand, THROW_COOLDOWN.0, THROW_COOLDOWN.1);
            self.casts = Some(CombatCast {
                spell,
                target: if spell.self_target {
                    self.ref_id
                } else {
                    c.target
                },
                landings: 1,
            });
        }
        Some("BeginCastRight".into())
    }
}
