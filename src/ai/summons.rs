//! Summons: a summon creature effect (Conjuration's atronachs, familiars...)
//! brings its actor (the effect's related NPC) where the spell landed, or
//! beside its caster, commanded by the caster for as long as the effect
//! lasts. The commanded follow their commander, fight whoever fights it (or
//! whoever it fights) and pick no fights of their own; at the end of the
//! effect, or when killed, they go. A commander has one at a time, more with
//! the Mod Commanded Actor Limit perk entry (Twin Souls); a new one sends the
//! oldest away. Open questions: `known_gaps/magic.md`.

use esp::FormId;
use glam::Vec3;

use crate::engine::{Engine, PLAYER_REF};
use crate::magic::MagicEffect;

use super::package::Gait;

/// How far from their commander the commanded keep: closer than the first
/// they stop, farther than the second they come (chosen by eye).
const ATTEND_RADIUS: (f32, f32) = (160.0, 320.0);
/// How far a commanded actor looks for its commander's enemies.
const DEFEND_RADIUS: f32 = 3000.0;
/// Farther than this from its commander (a cell's width), out of a fight,
/// it is moved beside it.
const CATCH_UP: f32 = 4096.0;

impl Engine {
    /// A summon effect starts: its actor appears, commanded by the caster.
    pub(crate) fn summon(&mut self, id: u64, mgef: &MagicEffect) {
        let Some(x) = self.active_effect(id).cloned() else {
            return;
        };
        let commander = x.caster.unwrap_or(x.target);
        if mgef.related.is_null() || self.is_dead(commander) {
            return;
        }
        // The commander's limit: the oldest summons go to make room.
        let limit = self
            .perk_entry_point(
                crate::perks::ep::MOD_COMMANDED_ACTOR_LIMIT,
                commander,
                &[Some(x.item)],
                1.0,
            )
            .max(1.0) as usize;
        let mut theirs: Vec<u64> = self
            .magic
            .effects
            .iter()
            .filter(|e| !e.done && e.id != id && e.summoned.is_some())
            .filter(|e| e.caster.unwrap_or(e.target) == commander)
            .map(|e| e.id)
            .collect();
        while theirs.len() >= limit {
            let oldest = theirs.remove(0);
            self.dispel_effect(oldest);
        }
        let Some(from) = self.ref_position(commander) else {
            return;
        };
        let at = x.at.unwrap_or(from + Vec3::X * ATTEND_RADIUS.0);
        let Some(r) = self.create_ref(mgef.related, commander, false) else {
            return;
        };
        let facing = (from - at).truncate();
        let yaw = if facing.length_squared() > 1.0 {
            facing.x.atan2(facing.y)
        } else {
            0.0
        };
        self.move_ref(r, commander, at, Vec3::new(0.0, 0.0, yaw));
        log::info!("{commander} summons {r} ({}) at {at:.0?}", mgef.related);
        self.magic.commanded.insert(r, commander);
        if let Some(e) = self.magic.effects.iter_mut().find(|e| e.id == id) {
            e.summoned = Some(r);
        }
    }

    /// A summon effect ends: its actor goes.
    pub(crate) fn unsummon(&mut self, r: FormId) {
        log::info!("{r} unsummoned");
        self.magic.commanded.remove(&r);
        self.delete_created(r);
    }

    /// The player's summons with them now, about to leave through a load
    /// door (or be moved) with them.
    pub(crate) fn player_party(&self) -> Vec<FormId> {
        let mut out: Vec<FormId> = self
            .magic
            .commanded
            .iter()
            .filter(|&(&r, &c)| c == PLAYER_REF && self.actor_ref(r).is_some_and(|a| !a.dead))
            .map(|(&r, _)| r)
            .collect();
        out.sort();
        out
    }

    /// The player arrived (through a load door, or moved): their summons
    /// come too, behind them.
    pub(crate) fn party_arrives(&mut self, party: Vec<FormId>, yaw: f32) {
        let feet = self.player.position
            - Vec3::Z * (self.physics.player_half_height + self.physics.player_radius);
        // Behind the player, side by side.
        let back = -Vec3::new(yaw.sin(), yaw.cos(), 0.0);
        let side = Vec3::new(back.y, -back.x, 0.0);
        let n = party.len() as f32;
        for (i, r) in party.into_iter().enumerate() {
            let offset = (i as f32 - (n - 1.0) / 2.0) * ATTEND_RADIUS.0;
            let mut to = feet + back * ATTEND_RADIUS.0 + side * offset;
            if let Some(z) = self.nav.height_at(to) {
                to.z = z;
            }
            log::info!("{r} follows {PLAYER_REF} to {to:.0?}");
            self.move_ref(r, PLAYER_REF, to, Vec3::new(0.0, 0.0, yaw));
        }
    }

    /// Whom `r` is commanded by (`IsCommandedActor`).
    pub fn commander_of(&self, r: FormId) -> Option<FormId> {
        self.magic.commanded.get(&r).copied()
    }

    /// The commanded fight their commander's fights, else keep near it.
    pub(crate) fn update_commanded(&mut self) {
        let commanded: Vec<(FormId, FormId)> =
            self.magic.commanded.iter().map(|(&r, &c)| (r, c)).collect();
        for (r, commander) in commanded {
            let Some(a) = self.actor_ref(r).filter(|a| !a.dead) else {
                continue;
            };
            let (pos, heading) = (a.pos, a.heading);
            let idle = a.combat.is_none();
            // Left far behind, it catches up at once, beside its commander.
            if let Some(c) = self.ref_position(commander)
                && idle
                && c.distance(pos) > CATCH_UP
            {
                let back = (pos - c).truncate().normalize_or_zero().extend(0.0);
                let to = self
                    .nav
                    .height_at(c + back * ATTEND_RADIUS.0)
                    .map_or(c, |z| {
                        let p = c + back * ATTEND_RADIUS.0;
                        Vec3::new(p.x, p.y, z)
                    });
                log::debug!("{r} catches up with {commander}");
                self.move_ref(r, commander, to, Vec3::new(0.0, 0.0, heading));
                continue;
            }
            // Following its commander about, it belongs to the cell it is in
            // now (actors otherwise stay with the cell they loaded in, going
            // when it unloads).
            if let Some((place, key)) = self
                .actor_cells
                .get(&r)
                .and_then(|&k| Some((self.place_of_key(k)?, k)))
                && idle
            {
                let now = Self::place_at(place, pos);
                if now != place && self.key_of_place(now).is_some_and(|k| k != key) {
                    log::debug!("{r} moves into {now:?}");
                    self.move_ref(r, r, pos, Vec3::new(0.0, 0.0, heading));
                    continue;
                }
            }
            // Never against its commander; a fight over (its target dead)
            // ends.
            let fighting = a.combat.as_ref().map(|c| c.target);
            if let Some(t) = fighting {
                if t == commander || self.is_dead(t) {
                    self.end_combat(r);
                } else {
                    continue;
                }
            }
            if let Some(enemy) = self.commanders_enemy(commander, pos) {
                log::info!("{r} defends {commander} against {enemy}");
                self.start_combat(r, enemy);
                continue;
            }
            let Some(key) = self.actor_cells.get(&r).copied() else {
                continue;
            };
            if let Some(a) = self
                .cells
                .get_mut(&key)
                .and_then(|rt| rt.actors.iter_mut().find(|a| a.ref_id == r))
            {
                a.attend(commander, ATTEND_RADIUS, Gait::Run, &mut self.furniture);
            }
        }
    }

    /// The nearest living actor near `pos` fighting `commander`, or that
    /// `commander` fights.
    fn commanders_enemy(&self, commander: FormId, pos: Vec3) -> Option<FormId> {
        let theirs = if commander == PLAYER_REF {
            None
        } else {
            self.actor_ref(commander)
                .and_then(|a| a.combat.as_ref())
                .map(|c| c.target)
        };
        let mut best: Option<(f32, FormId)> = theirs
            .filter(|&t| !self.is_dead(t) && !self.magic.commanded.contains_key(&t))
            .and_then(|t| Some((self.ref_position(t)?.distance(pos), t)));
        for a in self.cells.values().flat_map(|rt| &rt.actors) {
            if a.dead
                || a.bleeding.is_some()
                || self.magic.commanded.get(&a.ref_id) == Some(&commander)
                || a.combat.as_ref().is_none_or(|c| c.target != commander)
            {
                continue;
            }
            let d = a.pos.distance(pos);
            if d < DEFEND_RADIUS && best.is_none_or(|b| d < b.0) {
                best = Some((d, a.ref_id));
            }
        }
        best.map(|b| b.1)
    }
}
