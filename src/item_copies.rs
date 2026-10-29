//! Per-copy item state, kept sparse within an inventory's base-form counts.
use crate::engine::Engine;
use crate::world::inventory::Inventory;
use esp::FormId;

#[derive(Debug, Clone, PartialEq)]
pub struct CopyState {
    /// Internal health/value multiplier. One is untouched; each quality adds 1/6.
    pub quality: f32,
}
impl Default for CopyState {
    fn default() -> Self {
        Self { quality: 1.0 }
    }
}
impl CopyState {
    pub fn level(&self) -> u32 {
        ((self.quality - 1.0).max(0.0) * 6.0).round() as u32
    }
    pub fn at_level(level: u32) -> Self {
        Self {
            quality: 1.0 + level as f32 / 6.0,
        }
    }
    pub fn rating_bonus(&self, chest: bool) -> f32 {
        if self.level() == 0 {
            0.0
        } else {
            (3.6 * self.level() as f32 - 1.6) * if chest { 1.0 } else { 0.5 }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ItemCopy {
    pub item: FormId,
    pub owner: Option<FormId>,
    pub state: CopyState,
    pub count: i32,
}

impl Inventory {
    pub fn copy_state(&self, item: FormId) -> CopyState {
        self.equipped_copies.get(&item).cloned().unwrap_or_default()
    }
    pub fn add_copy_state(&mut self, copy: ItemCopy) {
        if copy.count <= 0 || copy.state == CopyState::default() {
            return;
        }
        if let Some(c) = self
            .copies
            .iter_mut()
            .find(|c| c.item == copy.item && c.owner == copy.owner && c.state == copy.state)
        {
            c.count += copy.count;
        } else {
            self.copies.push(copy);
        }
    }
    pub fn plain_copies(&self, item: FormId, owner: Option<FormId>) -> i32 {
        self.count_owned(item, owner)
            - self
                .copies
                .iter()
                .filter(|c| c.item == item && c.owner == owner)
                .map(|c| c.count)
                .sum::<i32>()
    }
    /// Removing an unspecified copy uses plain copies before modified copies,
    /// within the ownership stack requested by the caller.
    pub(crate) fn detach_copies(
        &mut self,
        item: FormId,
        parts: &[(Option<FormId>, i32)],
        plain: &[(Option<FormId>, i32)],
    ) -> Vec<ItemCopy> {
        let mut out = Vec::new();
        for &(owner, n) in parts {
            let mut left = (n - plain
                .iter()
                .find(|(o, _)| *o == owner)
                .map_or(0, |(_, n)| *n))
            .max(0);
            for c in self
                .copies
                .iter_mut()
                .rev()
                .filter(|c| c.item == item && c.owner == owner)
            {
                let take = left.min(c.count);
                if take > 0 {
                    let mut copy = c.clone();
                    copy.count = take;
                    out.push(copy);
                    c.count -= take;
                    left -= take;
                }
            }
        }
        self.copies.retain(|c| c.count > 0);
        if self.count(item) == 0 {
            self.equipped_copies.remove(&item);
        }
        out
    }
}

impl Engine {
    pub(crate) fn equipped_copy(&self, actor: FormId, item: FormId) -> CopyState {
        self.inventories
            .get(&actor)
            .map_or_else(CopyState::default, |i| i.copy_state(item))
    }
    pub fn item_health_percent(&self, reference: FormId) -> f32 {
        self.ref_state
            .item_copies
            .get(&reference)
            .and_then(|c| c.first())
            .map_or(1.0, |c| c.state.quality)
    }
    pub(crate) fn tempering_bonus(&self, actor: FormId, item: FormId) -> f32 {
        let chest = self
            .lo
            .get(item)
            .is_some_and(|r| crate::world::inventory::armor_slots(&r) & (1 << 2) != 0);
        self.equipped_copy(actor, item).rating_bonus(chest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quality_uses_exact_rating_and_value_increments() {
        for (level, chest, other) in [
            (1, 2., 1.),
            (2, 5.6, 2.8),
            (3, 9.2, 4.6),
            (4, 12.8, 6.4),
            (5, 16.4, 8.2),
            (6, 20., 10.),
            (7, 23.6, 11.8),
        ] {
            let c = CopyState::at_level(level);
            assert_eq!(c.level(), level);
            assert!((c.rating_bonus(true) - chest).abs() < 0.0001);
            assert!((c.rating_bonus(false) - other).abs() < 0.0001);
            assert!((c.quality - (1. + level as f32 / 6.)).abs() < 0.0001);
        }
    }
    #[test]
    fn removing_copies_respects_ownership_and_preserves_quality() {
        let f = FormId(1);
        let owner = Some(FormId(2));
        let mut inv = Inventory::default();
        inv.add(f, 2);
        inv.add_owned(f, owner, 1);
        inv.add_copy_state(ItemCopy {
            item: f,
            owner,
            state: CopyState::at_level(2),
            count: 1,
        });
        assert!(inv.remove_with_extras(f, 1, Some(None)).copies.is_empty());
        let removed = inv.remove_with_extras(f, 1, Some(owner));
        assert_eq!(removed.copies[0].state.level(), 2);
        assert_eq!(removed.copies[0].owner, owner);
        assert!(inv.copies.is_empty());
    }
}
