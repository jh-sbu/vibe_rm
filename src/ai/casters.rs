//! Casters in combat: an actor whose combat style favours magic and that has
//! a spell to throw (and the magicka for it) keeps its distance, faces its
//! target and casts at it; the engine casts the spell (`crate::casting`).
//! Out of magicka it fights as the others do.

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
        let in_range = dist < spell.range * 0.85 && c.clear_shot;
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
        let off = self.turn_towards(to, dt).abs();
        let c = self.combat.as_mut()?;
        if c.channel > 0.0 {
            // A landing each tick while it lasts.
            let before = (c.channel / CHANNEL_TICK).ceil();
            c.channel -= dt;
            let after = (c.channel.max(0.0) / CHANNEL_TICK).ceil();
            self.casts += (before - after) as u32;
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
            self.casts += 1;
        }
        Some("BeginCastRight".into())
    }
}
