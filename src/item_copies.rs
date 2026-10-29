//! Per-copy item state, kept sparse within an inventory's base-form counts.
use crate::engine::Engine;
use crate::world::inventory::Inventory;
use esp::FormId;

#[derive(Debug, Clone, PartialEq)]
pub struct CopyState {
    /// Internal health/value multiplier. One is untouched; each quality adds 1/6.
    pub quality: f32,
    pub enchantment: Option<FormId>,
    pub capacity: f32,
    pub charge: f32,
    pub name: Option<String>,
}
impl Default for CopyState {
    fn default() -> Self {
        Self {
            quality: 1.0,
            enchantment: None,
            capacity: 0.0,
            charge: 0.0,
            name: None,
        }
    }
}
impl CopyState {
    pub fn level(&self) -> u32 {
        ((self.quality - 1.0).max(0.0) * 6.0).round() as u32
    }
    pub fn at_level(level: u32) -> Self {
        Self {
            quality: 1.0 + level as f32 / 6.0,
            ..Default::default()
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
        self.equipped_copies.get(&item).cloned().unwrap_or_else(|| {
            let modified = self
                .copies
                .iter()
                .filter(|c| c.item == item)
                .map(|c| c.count)
                .sum::<i32>();
            if modified >= self.count(item) {
                self.copies
                    .iter()
                    .find(|c| c.item == item)
                    .map(|c| c.state.clone())
                    .unwrap_or_default()
            } else {
                CopyState::default()
            }
        })
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
    /// Changes one selected equipped copy, splitting a repeated state when needed.
    pub fn change_equipped_copy(
        &mut self,
        item: FormId,
        change: impl FnOnce(&mut CopyState),
    ) -> bool {
        let old = self.copy_state(item);
        let Some(i) = self
            .copies
            .iter()
            .position(|c| c.item == item && c.state == old && c.count > 0)
        else {
            return false;
        };
        let mut c = self.copies[i].clone();
        c.count = 1;
        self.copies[i].count -= 1;
        self.copies.retain(|c| c.count > 0);
        change(&mut c.state);
        self.equipped_copies.insert(item, c.state.clone());
        self.add_copy_state(c);
        true
    }
    /// Exact metadata selection, used by rows that distinguish copies.
    pub fn remove_copy(
        &mut self,
        item: FormId,
        n: i32,
        owner: Option<FormId>,
        state: &CopyState,
    ) -> crate::world::inventory::RemovedItems {
        let available = if *state == CopyState::default() {
            self.plain_copies(item, owner)
        } else {
            self.copies
                .iter()
                .filter(|c| c.item == item && c.owner == owner && c.state == *state)
                .map(|c| c.count)
                .sum()
        };
        let n = n.max(0).min(available);
        let mut extra = Vec::new();
        if *state != CopyState::default() {
            let mut left = n;
            for c in self
                .copies
                .iter_mut()
                .filter(|c| c.item == item && c.owner == owner && c.state == *state)
            {
                let take = left.min(c.count);
                if take > 0 {
                    let mut part = c.clone();
                    part.count = take;
                    extra.push(part);
                    c.count -= take;
                    left -= take;
                }
            }
            self.copies.retain(|c| c.count > 0);
        }
        let mut removed = self.remove_with_extras(item, n, Some(owner));
        removed.copies.extend(extra);
        removed
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
    pub(crate) fn copy_item_info(
        &self,
        item: FormId,
        state: &CopyState,
    ) -> Option<crate::world::inventory::ItemInfo> {
        let mut info = crate::world::inventory::item_info(&self.lo, item)?;
        if let Some(name) = &state.name {
            info.name = name.clone();
        }
        let level = state.level();
        if level > 0 {
            let stem = if info.kind == crate::world::inventory::ItemKind::Weapon {
                "Weap"
            } else {
                "Armo"
            };
            let fallback = [
                "Fine",
                "Superior",
                "Exquisite",
                "Flawless",
                "Epic",
                "Legendary",
            ][(level.min(6) - 1) as usize];
            let quality = self
                .gmst_string(&format!("sHealthDataPrefix{stem}{}", level.min(6)))
                .unwrap_or_else(|| fallback.into());
            info.name = format!("{} ({quality})", info.name);
            info.value = (info.value as f32 * state.quality).floor() as i32;
        }
        if let Some(enchantment) = state.enchantment {
            let cost = self
                .magic_item(enchantment)
                .and_then(|m| m.enchantment)
                .map_or(0, |e| e.cost);
            info.value = info.value.saturating_add(cost as i32);
        }
        Some(info)
    }

    pub(crate) fn equipped_copy(&self, actor: FormId, item: FormId) -> CopyState {
        self.inventories
            .get(&actor)
            .map_or_else(CopyState::default, |i| i.copy_state(item))
    }
    pub(crate) fn item_enchantment_for(&self, actor: FormId, item: FormId) -> Option<FormId> {
        self.equipped_copy(actor, item)
            .enchantment
            .or_else(|| self.item_enchantment(item))
    }
    pub(crate) fn weapon_capacity_for(&self, actor: FormId, item: FormId) -> f32 {
        let copy = self.equipped_copy(actor, item);
        if copy.enchantment.is_some() {
            copy.capacity
        } else {
            self.weapon_capacity(item)
        }
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
    fn player_enchantment_name_and_charge_stay_with_the_selected_copy() {
        let f = FormId(1);
        let state = CopyState {
            enchantment: Some(FormId(0xFF000800)),
            capacity: 250.,
            charge: 250.,
            name: Some("Sparks".into()),
            ..CopyState::at_level(2)
        };
        let mut inv = Inventory::default();
        inv.add(f, 2);
        inv.add_copy_state(ItemCopy {
            item: f,
            owner: None,
            state: state.clone(),
            count: 2,
        });
        inv.equipped_copies.insert(f, state);
        inv.set_charge(f, 100., 250.);
        assert_eq!(inv.copy_state(f).charge, 100.);
        assert_eq!(inv.copies.len(), 2);
        let removed = inv.remove_with_extras(f, 2, None);
        let mut destination = Inventory::default();
        destination.add(f, 2);
        for copy in removed.copies {
            destination.add_copy_state(copy);
        }
        assert_eq!(destination.copies.len(), 2);
        assert!(
            destination
                .copies
                .iter()
                .all(|c| c.state.name.as_deref() == Some("Sparks") && c.state.level() == 2)
        );
        assert_eq!(
            destination
                .copies
                .iter()
                .map(|c| c.state.charge)
                .sum::<f32>(),
            350.
        );
    }
    #[test]
    fn explicit_row_removal_never_substitutes_a_plain_or_other_modified_copy() {
        let f = FormId(1);
        let mut inv = Inventory::default();
        inv.add(f, 3);
        for level in [1, 3] {
            inv.add_copy_state(ItemCopy {
                item: f,
                owner: None,
                state: CopyState::at_level(level),
                count: 1,
            });
        }
        let removed = inv.remove_copy(f, 5, None, &CopyState::at_level(3));
        assert_eq!(removed.parts, vec![(None, 1)]);
        assert_eq!(removed.copies[0].state.level(), 3);
        assert_eq!(inv.count(f), 2);
        assert_eq!(inv.plain_copies(f, None), 1);
        assert_eq!(inv.copies[0].state.level(), 1);
    }
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
