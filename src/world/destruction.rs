//! Destructible base-object definitions and per-reference stage queries.

use esp::{FormId, destructible::DestructibleData};

use crate::engine::Engine;

#[cfg(test)]
#[path = "../destruction_tests.rs"]
mod tests;

/// Mutable destruction state belongs to a placed reference, not its shared base.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DestructionState {
    /// Remaining object health; the authored `DEST` value is the maximum.
    pub health: f64,
    /// Authored `DSTD.index`, or -1 before the first stage is entered.
    pub stage: i32,
}

/// Apply the bounded health/stage portion of one scripted damage call.
/// `None` means invalid damage, no `DEST` health, or no actual state change.
fn apply_damage(
    data: &DestructibleData,
    current: Option<DestructionState>,
    damage: f32,
) -> Option<DestructionState> {
    if data.health == 0 || !damage.is_finite() || damage <= 0.0 {
        return None;
    }
    let before = current.unwrap_or(DestructionState {
        health: data.health as f64,
        stage: -1,
    });
    if !before.health.is_finite() || before.health <= 0.0 {
        return None;
    }
    let health = (before.health - damage as f64).max(0.0);
    let max_health = data.health as f64;
    if health >= before.health {
        return None;
    }
    let health_percent = health * 100.0 / max_health;
    let next_stage = if before.stage < 0 {
        0
    } else if let Some(position) = data
        .stages
        .iter()
        .position(|s| s.index as i32 == before.stage)
    {
        position + 1
    } else {
        // An unknown persisted index cannot safely be mapped back to array order.
        data.stages.len()
    };
    let crossed: Vec<_> = data
        .stages
        .iter()
        .enumerate()
        .skip(next_stage)
        .filter(|(_, stage)| health_percent <= stage.health_percent as f64)
        .collect();

    let capped = crossed.iter().find(|(_, stage)| stage.flags & 0x01 != 0);
    let (stage, health) = if let Some((_, capped_stage)) = capped {
        // Cap at the first crossed CAP_DAMAGE stage. This explicit policy
        // prevents one hit from entering any later stage in the same call.
        let threshold = max_health * capped_stage.health_percent as f64 / 100.0;
        (
            capped_stage.index as i32,
            health.max(threshold.min(before.health)),
        )
    } else if let Some((_, last)) = crossed.last() {
        (last.index as i32, health)
    } else {
        (before.stage, health)
    };

    Some(DestructionState { health, stage })
}

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
            .destruction
            .get(&reference)
            .map(|state| state.stage)
            .unwrap_or(-1)
    }

    /// Papyrus `ObjectReference.DamageObject`. Only references present in a
    /// loaded cell's reference list are processed, matching the public CK
    /// caveat that this native may not work on unloaded objects.
    pub fn damage_object(&mut self, reference: FormId, damage: f32) {
        if !self
            .cells
            .values()
            .any(|cell| cell.refs.contains(&reference))
        {
            return;
        }
        let Some(data) = self.destructible_data(reference) else {
            return;
        };
        let current = self.world_state.destruction.get(&reference).copied();
        if let Some(next) = apply_damage(&data, current, damage) {
            let old_stage = current.map_or(-1, |state| state.stage);
            let stage_changed = next.stage != old_stage;
            self.world_state.destruction.insert(reference, next);
            if stage_changed {
                self.send_script_event(
                    reference,
                    "OnDestructionStageChanged",
                    vec![
                        papyrus::Value::Int(old_stage),
                        papyrus::Value::Int(next.stage),
                    ],
                );
            }
        }
    }

    /// Health damage from a weapon, bash or arrow blow. Like `damage_object`,
    /// but a stage with `Ignore External Damage` (DSTD flag 0x08) takes none
    /// from blows: UESP's flag list, with the choice in
    /// `known_ambiguities/destruction.md`.
    pub(crate) fn damage_object_by_blow(&mut self, reference: FormId, damage: f32) {
        let Some(data) = self.destructible_data(reference) else {
            return;
        };
        let stage = self
            .world_state
            .destruction
            .get(&reference)
            .map_or(-1, |state| state.stage);
        if data
            .stages
            .iter()
            .any(|s| s.index as i32 == stage && s.flags & 0x08 != 0)
        {
            return;
        }
        self.damage_object(reference, damage);
    }

    /// Papyrus `ObjectReference.ClearDestruction`. Clear the stored per-reference
    /// health and stage without requiring the reference to be loaded.
    pub fn clear_destruction(&mut self, reference: FormId) {
        self.world_state.destruction.remove(&reference);
    }
}
