//! The player's items: taking things from the world and from containers, and the
//! menus that list them.

use esp::FormId;

use crate::engine::{Engine, PLAYER_REF};
use crate::world::inventory::{item_info, ItemInfo};

/// A menu taking the mouse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    /// The player's inventory.
    Inventory,
    /// A container (or another inventory) being searched.
    Container(FormId),
}

impl Engine {
    /// Whether a reference is an item lying about that can be picked up.
    pub fn is_item_ref(&self, r: FormId) -> bool {
        self.base_of(r).is_some_and(|b| item_info(&self.lo, b).is_some()) && self.lo.tag_of(r).map(|t| t.0) == Some(*b"REFR")
    }

    /// Pick up an item reference: into the player's inventory (as many as the
    /// reference stands for, `XCNT`) and gone from the world.
    pub fn take_item(&mut self, r: FormId) -> bool {
        let Some(base) = self.base_of(r) else { return false };
        let Some(info) = item_info(&self.lo, base) else { return false };
        let count = self.lo.get(r).and_then(|rec| rec.get(b"XCNT").filter(|d| d.len() >= 4).map(|d| i32::from_le_bytes(d[0..4].try_into().unwrap()))).unwrap_or(1).max(1);
        self.inventory_mut(PLAYER_REF).add(base, count);
        self.set_disabled(r, true);
        self.scripts.notify(if count > 1 { format!("{} ({count}) added", info.name) } else { format!("{} added", info.name) });
        log::info!("took {r} ({base} x{count})");
        let player = self.object_value(PLAYER_REF);
        self.send_script_event(r, "OnContainerChanged", vec![player, papyrus::Value::None]);
        true
    }

    /// Move up to `n` of `item` from one inventory to another; returns how many moved.
    pub fn transfer_item(&mut self, from: FormId, to: FormId, item: FormId, n: i32) -> i32 {
        let moved = self.remove_item(from, item, n, Some(to));
        if moved > 0 && to == PLAYER_REF {
            let name = item_info(&self.lo, item).map(|i| i.name).unwrap_or_default();
            self.scripts.notify(if moved > 1 { format!("{name} ({moved}) added") } else { format!("{name} added") });
        }
        moved
    }

    /// An inventory's items with what they are, by kind then name.
    pub fn listed_inventory(&mut self, r: FormId) -> Vec<(FormId, i32, ItemInfo)> {
        let items = self.inventory_mut(r).items.clone();
        let mut out: Vec<(FormId, i32, ItemInfo)> = items.into_iter().filter_map(|(f, n)| Some((f, n, item_info(&self.lo, f)?))).filter(|x| x.1 > 0).collect();
        out.sort_by(|a, b| (a.2.kind, &a.2.name).cmp(&(b.2.kind, &b.2.name)));
        out
    }

    /// Send a Papyrus event to the scripts on a reference.
    pub fn send_script_event(&mut self, r: FormId, event: &str, args: Vec<papyrus::Value>) {
        let mut vm = std::mem::take(&mut self.vm);
        let mut host = crate::script::EngineHost { engine: self };
        vm.send_event(&mut host, papyrus::ObjectId::Form(r.0), event, args);
        self.vm = vm;
    }
}
