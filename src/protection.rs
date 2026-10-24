//! Whether actors can be hurt or killed. Essential and protected actors (the
//! base's `ACBS` flags, or filling a quest alias with the flag while its
//! quest runs) bleed out instead of dying, protected ones but to the player.
//! Invulnerable actors' health can't be lowered, and blows pass ghosts by.
//! Scripts change the base's flags (`ActorBase.SetEssential`, `SetProtected`,
//! `SetInvulnerable`; saved: `BFLG`) and make actors ghosts (`Actor.SetGhost`;
//! saved: `GHST`). Choices:
//! `known_ambiguities/protection.md`.

use std::collections::HashMap;

use esp::FormId;

use crate::engine::Engine;

/// `ACBS` flags scripts change.
pub mod acbs {
    pub const ESSENTIAL: u32 = 0x2;
    pub const PROTECTED: u32 = 0x800;
    pub const GHOST: u32 = 0x2000_0000;
    pub const INVULNERABLE: u32 = 0x8000_0000;
    /// The flags `BFLG` may hold.
    pub const CHANGEABLE: u32 = ESSENTIAL | PROTECTED | INVULNERABLE;
}

#[derive(Default)]
pub struct Protection {
    /// `ACBS` flags scripts set (true) or cleared on actor bases, by
    /// (base, flag).
    pub(crate) base_flags: HashMap<(FormId, u32), bool>,
    /// References `Actor.SetGhost` made ghosts (true) or not, over their
    /// base's flag.
    pub(crate) ghosts: HashMap<FormId, bool>,
}

impl Engine {
    /// Whether an actor base has an `ACBS` flag: as a script left it, else
    /// as its record says.
    pub fn base_has_flag(&self, base: FormId, flag: u32) -> bool {
        if let Some(&on) = self.protection.base_flags.get(&(base, flag)) {
            return on;
        }
        self.lo
            .get(base)
            .and_then(|r| {
                r.get(b"ACBS")
                    .filter(|d| d.len() >= 4)
                    .map(|d| u32::from_le_bytes(d[0..4].try_into().unwrap()))
            })
            .is_some_and(|f| f & flag != 0)
    }

    /// A flag a script set or cleared on `actor`'s base, if one did.
    pub(crate) fn changed_base_flag(&self, actor: FormId, flag: u32) -> Option<bool> {
        let base = self.base_of(actor)?;
        self.protection.base_flags.get(&(base, flag)).copied()
    }

    /// Invulnerable (`ACBS` 0x80000000, or `SetInvulnerable`): the actor's
    /// health can't be lowered.
    pub fn is_invulnerable(&self, actor: FormId) -> bool {
        self.base_of(actor)
            .is_some_and(|b| self.base_has_flag(b, acbs::INVULNERABLE))
    }

    /// A ghost (`IsGhost`, `GetIsGhost`): as `SetGhost` left the reference,
    /// else its base's `ACBS` 0x20000000. Blows, spells and falls pass it by.
    pub fn is_ghost(&self, actor: FormId) -> bool {
        match self.protection.ghosts.get(&actor) {
            Some(&on) => on,
            None => self
                .base_of(actor)
                .is_some_and(|b| self.base_has_flag(b, acbs::GHOST)),
        }
    }

    /// `Actor.SetGhost`.
    pub fn set_ghost(&mut self, actor: FormId, on: bool) {
        if !actor.is_null() {
            self.protection.ghosts.insert(actor, on);
        }
    }

    /// Restore the ghosts a save kept, in place of this session's.
    pub(crate) fn restore_ghosts(&mut self, ghosts: HashMap<FormId, bool>) {
        self.protection.ghosts = ghosts;
    }

    /// `ActorBase.SetEssential`, `SetProtected` and `SetInvulnerable`.
    pub fn set_base_flag(&mut self, base: FormId, flag: u32, on: bool) {
        if base.is_null() || flag & acbs::CHANGEABLE == 0 {
            return;
        }
        self.protection.base_flags.insert((base, flag), on);
    }

    /// Restore the base flags a save kept, in place of this session's.
    pub(crate) fn restore_base_flags(&mut self, flags: HashMap<(FormId, u32), bool>) {
        self.protection.base_flags = flags;
    }
}

#[cfg(test)]
#[path = "protection_tests.rs"]
mod tests;
