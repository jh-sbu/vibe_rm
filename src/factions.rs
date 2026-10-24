//! Factions' standing changed at runtime. A faction's reactions towards
//! others start as its plugin's `XNAM` entries (another faction, a reaction
//! modifier and a group combat reaction: 0 neutral, 1 enemy, 2 ally,
//! 3 friend); Papyrus `SetEnemy` and `SetAlly` change the combat reactions of
//! both factions towards each other. Changes are kept by faction pair, over
//! the plugin's entry, and saved (`FRCT`). Choices:
//! `known_ambiguities/factions.md`.

use std::collections::HashMap;

use esp::FormId;

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

#[derive(Default)]
pub struct Factions {
    /// Reactions changed since the plugins, by (faction, towards).
    pub(crate) reactions: HashMap<(FormId, FormId), Reaction>,
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
