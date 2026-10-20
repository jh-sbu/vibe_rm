//! Destructible base-object definitions and per-reference stage queries.

use esp::{FormId, destructible::DestructibleData};

use crate::engine::Engine;

#[cfg(test)]
#[path = "../destruction_tests.rs"]
mod tests;

impl Engine {
    /// The `DEST` definition of a reference's base object.
    pub fn destructible_data(&self, reference: FormId) -> Option<DestructibleData> {
        let base = self.base_of(reference)?;
        self.lo.get(base)?.destructible_data()
    }

    /// Current stage of a reference. Untouched references start at -1, as
    /// documented for Skyrim's `GetCurrentDestructionStage` Papyrus native.
    pub fn current_destruction_stage(&self, reference: FormId) -> i32 {
        self.world_state
            .destruction_stages
            .get(&reference)
            .copied()
            .unwrap_or(-1)
    }
}
