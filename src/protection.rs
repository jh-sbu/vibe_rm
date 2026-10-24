//! Whether actors can be hurt or killed. Essential and protected actors (the
//! base's `ACBS` flags, or filling a quest alias with the flag while its
//! quest runs) bleed out instead of dying, protected ones but to the player.
//! Invulnerable actors' health can't be lowered. Scripts change the base's
//! flags (`ActorBase.SetEssential`, `SetProtected`, `SetInvulnerable`; saved:
//! `BFLG`). Choices:
//! `known_ambiguities/protection.md`.

use std::collections::HashMap;

use esp::FormId;

use crate::engine::Engine;

/// `ACBS` flags scripts change.
pub mod acbs {
    pub const ESSENTIAL: u32 = 0x2;
    pub const PROTECTED: u32 = 0x800;
    pub const INVULNERABLE: u32 = 0x8000_0000;
    /// The flags `BFLG` may hold.
    pub const CHANGEABLE: u32 = ESSENTIAL | PROTECTED | INVULNERABLE;
}

#[derive(Default)]
pub struct Protection {
    /// `ACBS` flags scripts set (true) or cleared on actor bases, by
    /// (base, flag).
    pub(crate) base_flags: HashMap<(FormId, u32), bool>,
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
