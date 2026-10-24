//! Factions' standing changed at runtime. A faction's reactions towards
//! others start as its plugin's `XNAM` entries (another faction, a reaction
//! modifier and a group combat reaction: 0 neutral, 1 enemy, 2 ally,
//! 3 friend); Papyrus `SetEnemy` and `SetAlly` change the combat reactions of
//! both factions towards each other, and `SetReaction` / `ModReaction` the
//! modifier of one towards the other. Changes are kept by faction pair, over
//! the plugin's entry, and saved (`FRCT`). The player's expelled and enemy
//! flags start as the faction's `DATA` bits and change by `SetPlayerExpelled`
//! / `SetPlayerEnemy` (saved: `FPLR`). Actors `SetAttackActorOnSight`
//! marks are attacked by those who would fight their enemies (saved:
//! `AOSG`). Choices:
//! `known_ambiguities/factions.md`.

use std::collections::{HashMap, HashSet};

use esp::FormId;

use crate::ai::combat::CombatStats;
use crate::engine::Engine;

/// One faction's reaction towards another (`XNAM`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reaction {
    pub modifier: i32,
    /// 0 neutral, 1 enemy, 2 ally, 3 friend.
    pub combat: u32,
}

/// The highest group combat reaction (friend).
pub const MAX_COMBAT_REACTION: u32 = 3;

/// The faction's `DATA` flags for the player: expelled, and enemy.
pub const PLAYER_EXPELLED: u8 = 1 << 2;
pub const PLAYER_ENEMY: u8 = 1 << 3;
pub const PLAYER_FLAGS: u8 = PLAYER_EXPELLED | PLAYER_ENEMY;

#[derive(Default)]
pub struct Factions {
    /// Reactions changed since the plugins, by (faction, towards).
    pub(crate) reactions: HashMap<(FormId, FormId), Reaction>,
    /// The player flags (`PLAYER_FLAGS` bits) of factions scripts changed.
    pub(crate) player_flags: HashMap<FormId, u8>,
    /// Actors others attack on sight (`SetAttackActorOnSight`).
    pub(crate) attacked_on_sight: HashSet<FormId>,
}

impl Engine {
    /// The plugin's reaction of `from` towards `to`: its first `XNAM`
    /// naming `to`.
    fn authored_reaction(&self, from: FormId, to: FormId) -> Option<Reaction> {
        let rec = self.lo.get(from)?;
        rec.subrecords()
            .filter(|s| s.tag.0 == *b"XNAM" && s.data.len() >= 12)
            .find(|s| rec.fid(s.form_id(0)) == to)
            .map(|s| Reaction {
                modifier: i32::from_le_bytes(s.data[4..8].try_into().unwrap()),
                combat: u32::from_le_bytes(s.data[8..12].try_into().unwrap()),
            })
    }

    /// How `from` regards `to` now: a change made since, else the plugin's
    /// entry, else neutral with no modifier.
    pub(crate) fn reaction_towards(&self, from: FormId, to: FormId) -> Reaction {
        self.factions
            .reactions
            .get(&(from, to))
            .copied()
            .or_else(|| self.authored_reaction(from, to))
            .unwrap_or_default()
    }

    /// Change how `from` regards `to` in combat, keeping its modifier.
    pub(crate) fn set_combat_reaction(&mut self, from: FormId, to: FormId, combat: u32) {
        if from.is_null() || to.is_null() {
            return;
        }
        let r = Reaction {
            combat: combat.min(MAX_COMBAT_REACTION),
            ..self.reaction_towards(from, to)
        };
        self.factions.reactions.insert((from, to), r);
        self.faction_relations.remove(&from);
    }

    /// Papyrus `SetReaction` / `ModReaction`: the reaction modifier of `from`
    /// towards `to`, set or added to, keeping its combat reaction.
    pub fn set_reaction_modifier(&mut self, from: FormId, to: FormId, value: i32, add: bool) {
        if from.is_null() || to.is_null() {
            return;
        }
        let old = self.reaction_towards(from, to);
        let modifier = if add {
            old.modifier.saturating_add(value)
        } else {
            value
        };
        self.factions
            .reactions
            .insert((from, to), Reaction { modifier, ..old });
        self.faction_relations.remove(&from);
    }

    /// Papyrus `SetEnemy`: the two factions become enemies of each other,
    /// or neutral on the sides asked (`abSelfIsNeutralToOther`,
    /// `abOtherIsNeutralToSelf`).
    pub fn set_factions_enemies(&mut self, a: FormId, b: FormId, a_neutral: bool, b_neutral: bool) {
        self.set_combat_reaction(a, b, if a_neutral { 0 } else { 1 });
        self.set_combat_reaction(b, a, if b_neutral { 0 } else { 1 });
    }

    /// Papyrus `SetAlly`: the two factions become allies of each other, or
    /// friends on the sides asked (`abSelfIsFriendToOther`,
    /// `abOtherIsFriendToSelf`).
    pub fn set_factions_allies(&mut self, a: FormId, b: FormId, a_friend: bool, b_friend: bool) {
        self.set_combat_reaction(a, b, if a_friend { 3 } else { 2 });
        self.set_combat_reaction(b, a, if b_friend { 3 } else { 2 });
    }

    /// The reactions as each faction lists them for combat: the plugin's
    /// `XNAM` entries with the changes made since in their place.
    pub(crate) fn combat_reactions_of(&self, from: FormId) -> Vec<(FormId, u32)> {
        let mut list: Vec<(FormId, u32)> = self.lo.get(from).map_or_else(Vec::new, |rec| {
            rec.subrecords()
                .filter(|s| s.tag.0 == *b"XNAM" && s.data.len() >= 12)
                .map(|s| {
                    (
                        rec.fid(s.form_id(0)),
                        u32::from_le_bytes(s.data[8..12].try_into().unwrap()),
                    )
                })
                .filter(|(to, _)| !self.factions.reactions.contains_key(&(from, *to)))
                .collect()
        });
        let mut changed: Vec<_> = self
            .factions
            .reactions
            .iter()
            .filter(|((f, _), _)| *f == from)
            .map(|(&(_, to), r)| (to, r.combat))
            .collect();
        changed.sort();
        list.extend(changed);
        list
    }

    /// The faction's player flags now (`PLAYER_FLAGS` bits): changed by a
    /// script, else its `DATA`.
    pub fn faction_player_flags(&self, faction: FormId) -> u8 {
        if let Some(&f) = self.factions.player_flags.get(&faction) {
            return f;
        }
        self.lo
            .get(faction)
            .filter(|r| r.header.tag.0 == *b"FACT")
            .and_then(|r| r.get(b"DATA").and_then(|d| d.first().copied()))
            .map_or(0, |d| d & PLAYER_FLAGS)
    }

    /// Papyrus `SetPlayerExpelled` / `SetPlayerEnemy`: set or clear one of
    /// a faction's player flags.
    pub fn set_faction_player_flag(&mut self, faction: FormId, flag: u8, on: bool) {
        let Some(rec) = self.lo.get(faction) else {
            return;
        };
        if rec.header.tag.0 != *b"FACT" {
            return;
        }
        let old = self.faction_player_flags(faction);
        let new = if on { old | flag } else { old & !flag };
        self.factions
            .player_flags
            .insert(faction, new & PLAYER_FLAGS);
    }

    pub fn player_expelled_from(&self, faction: FormId) -> bool {
        self.faction_player_flags(faction) & PLAYER_EXPELLED != 0
    }

    pub fn player_enemy_of(&self, faction: FormId) -> bool {
        self.faction_player_flags(faction) & PLAYER_ENEMY != 0
    }

    /// Whether `actor` is in a faction the player is an enemy of: one of
    /// `factions` (its memberships) or its crime faction.
    pub(crate) fn in_player_enemy_faction(&self, actor: FormId, factions: &[FormId]) -> bool {
        factions.iter().any(|&f| self.player_enemy_of(f)) || self.crime_player_enemy(actor)
    }

    /// `GetActorCrimePlayerEnemy`: the actor's crime faction has the player
    /// as an enemy.
    pub fn crime_player_enemy(&self, actor: FormId) -> bool {
        self.crime_faction(actor)
            .is_some_and(|f| self.player_enemy_of(f))
    }

    /// Papyrus `SetAttackActorOnSight`.
    pub fn set_attacked_on_sight(&mut self, actor: FormId, on: bool) {
        if actor.is_null() {
            return;
        }
        if on {
            self.factions.attacked_on_sight.insert(actor);
        } else {
            self.factions.attacked_on_sight.remove(&actor);
        }
    }

    /// Whether an actor with `stats` would attack `target`, another actor
    /// with `target_factions`, on sight: hostile to it (`hostile_to`), or
    /// `target` is attacked on sight and the attacker would fight its
    /// enemies and doesn't like it.
    pub(crate) fn hostile_to_actor(
        &mut self,
        stats: &CombatStats,
        target: FormId,
        target_factions: &[FormId],
    ) -> bool {
        if self.hostile_to(stats, target_factions, false) {
            return true;
        }
        stats.aggression >= 1
            && self.factions.attacked_on_sight.contains(&target)
            && !matches!(
                self.faction_reaction(&stats.factions, target_factions),
                Some(2 | 3)
            )
    }

    /// Restore the actors attacked on sight a save kept.
    pub(crate) fn restore_attacked_on_sight(&mut self, actors: HashSet<FormId>) {
        self.factions.attacked_on_sight = actors;
    }

    /// Restore the player flags a save kept, in place of this session's.
    pub(crate) fn restore_faction_player_flags(&mut self, flags: HashMap<FormId, u8>) {
        self.factions.player_flags = flags;
    }

    /// Restore the reactions a save kept, in place of this session's.
    pub(crate) fn restore_faction_reactions(
        &mut self,
        reactions: HashMap<(FormId, FormId), Reaction>,
    ) {
        self.factions.reactions = reactions;
        self.faction_relations.clear();
    }
}

#[cfg(test)]
#[path = "factions_tests.rs"]
mod tests;
