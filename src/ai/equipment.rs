//! What actors wear and hold beyond their animation: rigid equipment (weapons,
//! shields) and torches carried after dark, with their light.

use esp::FormId;
use glam::{Mat4, Vec3};

use crate::engine::{Engine, Location};
use crate::render::{CellKey, HeldLight};
use crate::world::inventory::HandType;

/// `LIGH` flag: can be carried.
const LIGHT_CARRIED: u32 = 0x2;

impl Engine {
    /// Outdoors between dusk and dawn (the climate's sunset end to sunrise start).
    fn dark_outside(&self) -> bool {
        if !matches!(self.location, Location::Exterior { .. }) {
            return false;
        }
        let (sunrise, sunset) = self.sky.as_ref().map_or((5.5, 20.5), |(_, c)| (c.sunrise.0, c.sunset.1));
        self.hour < sunrise + 0.5 || self.hour > sunset - 0.5
    }

    /// A light it can carry, from an inventory (`Torch01`).
    fn carried_light(&self, actor: FormId) -> Option<FormId> {
        let inv = self.inventories.get(&actor)?;
        inv.items.iter().map(|(f, _)| *f).find(|&f| {
            self.lo.get(f).is_some_and(|r| r.tag().0 == *b"LIGH" && crate::world::records::light_data(&r).is_some_and(|l| l.flags & LIGHT_CARRIED != 0))
        })
    }

    /// Humanoids walking about outdoors after dark light a torch if they carry one,
    /// and put it away to sit down, use furniture or come back to daylight.
    pub(crate) fn update_torches(&mut self) {
        let dark = self.dark_outside();
        let mut changes: Vec<(CellKey, usize, FormId, Option<FormId>)> = Vec::new();
        for (key, rt) in &self.cells {
            for (i, a) in rt.actors.iter().enumerate() {
                let humanoid = a.graph.as_ref().is_some_and(|g| g.project().humanoid());
                let want = dark && humanoid && !a.in_furniture() && a.seat.is_none() && a.objects.is_empty();
                let torch = if want { self.carried_light(a.ref_id) } else { None };
                if torch != a.torch {
                    changes.push((*key, i, a.ref_id, torch));
                }
            }
        }
        for (key, index, actor, torch) in changes {
            self.hold_torch(key, index, actor, torch);
        }
    }

    /// Light (`Some`) or put away (`None`) a torch: it takes the shield's place in
    /// the left hand and the graph raises it.
    fn hold_torch(&mut self, key: CellKey, index: usize, actor: FormId, torch: Option<FormId>) {
        let lo = &self.lo;
        let Some(a) = self.cells.get_mut(&key).and_then(|rt| rt.actors.get_mut(index)) else { return };
        let old = std::mem::replace(&mut a.torch, torch);
        let inv = self.inventories.entry(actor).or_default();
        if let Some(t) = old {
            inv.equipped.retain(|&f| f != t);
            // The shield it set aside.
            if let Some(s) = a.stowed_shield.take() {
                inv.equipped.push(s);
            }
        }
        if let Some(t) = torch {
            if let Some(s) = inv.equipped.iter().position(|&f| lo.get(f).is_some_and(|r| r.tag().0 == *b"ARMO" && crate::world::inventory::armor_slots(&r) & (1 << 9) != 0)) {
                a.stowed_shield = Some(inv.equipped.remove(s));
            }
            inv.equipped.push(t);
        }
        let left = if torch.is_some() { HandType::Torch } else { inv.hand(lo, true) };
        if let Some(g) = &mut a.graph {
            g.set_variable("iLeftHandType", left as i32 as f32);
        }
        log::debug!("{actor} {}", if torch.is_some() { "lights a torch" } else { "puts its torch away" });
        self.refresh_equipment(key, index, actor);
    }

    /// Rebuild an actor's rigid equipment (and carried light) from what it has
    /// equipped.
    pub(crate) fn refresh_equipment(&mut self, key: CellKey, index: usize, actor: FormId) {
        let equipped = self.inventories.get(&actor).map(|i| i.equipped.clone()).unwrap_or_default();
        let mut models: Vec<(String, Option<FormId>)> = self
            .rigid_models
            .get(&actor)
            .into_iter()
            .flatten()
            .filter(|(item, _)| equipped.contains(item))
            .map(|(_, m)| (m.clone(), None))
            .collect();
        for &f in &equipped {
            let Some(rec) = self.lo.get(f).filter(|r| r.tag().0 == *b"LIGH") else { continue };
            if let Some(m) = crate::world::records::model_path(&rec) {
                models.push((m, Some(f)));
            }
        }
        let paths: Vec<String> = models.iter().map(|(m, _)| m.clone()).collect();
        self.models.load_all(&mut self.renderer, &self.vfs, &paths);
        let Some(skel) = self.cells.get(&key).and_then(|rt| rt.actors.get(index)).map(|a| a.skeleton.clone()) else { return };
        let mut equipment = Vec::new();
        let mut light = None;
        for (m, light_form) in models {
            let (Some(model), Some(bone)) = (self.models.get(&m), self.parent_bone(&m).and_then(|b| skel.find(b.as_str()))) else { continue };
            equipment.push((model, bone, Mat4::IDENTITY));
            if let Some(l) = light_form.and_then(|f| self.lo.get(f)).and_then(|r| crate::world::records::light_data(&r)) {
                let offset = self.light_attach_point(&m);
                light = Some(HeldLight { bone, offset, radius: l.radius.max(1.0), color: l.color * l.fade });
            }
        }
        if let Some(inst) = self.scene.cells.get_mut(&key).and_then(|rc| rc.actors.get_mut(index)) {
            inst.equipment = equipment;
            inst.held_light = light;
        }
    }

    /// Where a carried light's model gives off its light: its `AttachLight` node.
    fn light_attach_point(&mut self, model: &str) -> Vec3 {
        if let Some(p) = self.light_attach.get(model) {
            return *p;
        }
        let p = self
            .vfs
            .read(model)
            .and_then(|b| nif::Nif::parse(&b).ok())
            .and_then(|n| crate::render::model::node_transforms(&n).get("AttachLight").map(|m| m.w_axis.truncate()))
            .unwrap_or(Vec3::ZERO);
        self.light_attach.insert(model.to_owned(), p);
        p
    }

    /// Put the lights actors carry after the cells' own and light what is near them.
    pub(crate) fn update_held_lights(&mut self) {
        let first = self.static_lights.min(self.scene.lights.len());
        let had = self.scene.lights.len() > first;
        self.scene.lights.truncate(first);
        let held: Vec<crate::render::GpuLight> = self
            .scene
            .cells
            .values()
            .flat_map(|c| c.actors.iter())
            .filter_map(|a| {
                let l = a.held_light?;
                let p = a.held_light_pos()?;
                Some(crate::render::GpuLight { pos_radius: [p.x, p.y, p.z, l.radius], color: [l.color.x, l.color.y, l.color.z, 1.0] })
            })
            .collect();
        if held.is_empty() && !had {
            return;
        }
        self.scene.lights.extend(held);
        self.scene.assign_moving_lights(first);
    }
}
