//! What actors wear and hold beyond their animation: rigid equipment (weapons,
//! shields) and torches carried after dark, with their light.

use esp::FormId;
use glam::{Mat4, Vec3};

use crate::engine::{Engine, Location};
use crate::render::{CellKey, HeldLight};
use crate::world::inventory::{HandType, LEFT, RIGHT};

/// `LIGH` flag: can be carried.
const LIGHT_CARRIED: u32 = 0x2;

impl Engine {
    /// Outdoors between dusk and dawn (the climate's sunset end to sunrise start).
    fn dark_outside(&self) -> bool {
        if !matches!(self.location, Location::Exterior { .. }) {
            return false;
        }
        let (sunrise, sunset) = self
            .climate()
            .map_or((5.5, 20.5), |c| (c.sunrise.0, c.sunset.1));
        self.hour < sunrise + 0.5 || self.hour > sunset - 0.5
    }

    /// A light it can carry, from an inventory (`Torch01`).
    fn carried_light(&self, actor: FormId) -> Option<FormId> {
        let inv = self.inventories.get(&actor)?;
        inv.items.iter().map(|(f, _)| *f).find(|&f| {
            self.lo.get(f).is_some_and(|r| {
                r.tag().0 == *b"LIGH"
                    && crate::world::records::light_data(&r)
                        .is_some_and(|l| l.flags & LIGHT_CARRIED != 0)
            })
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
                // Not with both hands on a weapon (a two-handed one, a bow).
                let both = self
                    .inventories
                    .get(&a.ref_id)
                    .is_some_and(|i| i.both_hands() && i.held(LEFT) != a.torch);
                let want = dark
                    && humanoid
                    && !both
                    && !a.in_furniture()
                    && a.seat.is_none()
                    && a.objects.is_empty();
                let torch = if want {
                    self.carried_light(a.ref_id)
                } else {
                    None
                };
                if torch != a.torch {
                    changes.push((*key, i, a.ref_id, torch));
                }
            }
        }
        for (key, index, actor, torch) in changes {
            self.hold_torch(key, index, actor, torch);
        }
    }

    /// Light (`Some`) or put away (`None`) a torch: it takes the place of the
    /// shield or second weapon in the left hand (`Inventory::hold`) and the
    /// graph raises it.
    fn hold_torch(&mut self, key: CellKey, index: usize, actor: FormId, torch: Option<FormId>) {
        let speeds = self.weapon_speed_mults(actor);
        let lo = &self.lo;
        let Some(a) = self
            .cells
            .get_mut(&key)
            .and_then(|rt| rt.actors.get_mut(index))
        else {
            return;
        };
        let old = std::mem::replace(&mut a.torch, torch);
        let inv = self.inventories.entry(actor).or_default();
        if let Some(t) = old {
            inv.take_off(t);
            // What it set aside, unless the hand was taken since.
            if let Some(s) = a.stowed_left.take()
                && inv.held(LEFT).is_none()
            {
                inv.hold(lo, s, &[LEFT]);
            }
        }
        if let Some(t) = torch {
            // The shield, or a second weapon.
            a.stowed_left = inv.held(LEFT).filter(
                |&f| matches!(lo.tag_of(f).map(|t| t.0), Some(t) if t == *b"ARMO" || t == *b"WEAP"),
            );
            inv.hold(lo, t, &[LEFT]);
        }
        a.hands = crate::ai::combat::fighting_hands(lo, inv);
        if let Some(g) = &mut a.graph {
            for (var, value) in crate::ai::combat::hand_graph_vars(lo, inv, speeds) {
                g.set_variable(var, value);
            }
        }
        log::debug!(
            "{actor} {}",
            if torch.is_some() {
                "lights a torch"
            } else {
                "puts its torch away"
            }
        );
        self.refresh_equipment(key, index, actor);
    }

    /// Put a weapon an NPC carries in its hand (a bow displaces the shield): its
    /// model, the graph's hand types and how it fights follow.
    pub(crate) fn wield(&mut self, actor: FormId, weapon: FormId) -> bool {
        let Some(key) = self.actor_cells.get(&actor).copied() else {
            return false;
        };
        if self.equip_item(actor, weapon, true).is_err() {
            return false;
        }
        if let Some(m) = crate::world::inventory::weapon_model(&self.lo, weapon) {
            let models = self.rigid_models.entry(actor).or_default();
            if !models.iter().any(|(f, _)| *f == weapon) {
                models.push((weapon, m.clone()));
                self.models.load_all(&mut self.renderer, &self.vfs, &[m]);
            }
        }
        let lo = &self.lo;
        let inv = self.inventories.get(&actor).cloned().unwrap_or_default();
        let bow = crate::ai::archery::is_bow(lo, weapon);
        let reach = lo
            .get(weapon)
            .and_then(|r| {
                r.get(b"DNAM")
                    .filter(|x| x.len() >= 12)
                    .map(|x| f32::from_le_bytes(x[8..12].try_into().unwrap()))
            })
            .unwrap_or(0.0);
        let cost = self.power_attack_cost(actor, Some(weapon));
        let speeds = self.weapon_speed_mults(actor);
        let Some(index) = self
            .cells
            .get(&key)
            .and_then(|rt| rt.actors.iter().position(|a| a.ref_id == actor))
        else {
            return false;
        };
        let a = &mut self.cells.get_mut(&key).unwrap().actors[index];
        a.bow = bow;
        a.bow_speed = if bow {
            crate::ai::archery::bow_speed(lo, weapon)
        } else {
            a.bow_speed
        };
        a.weapon_reach = reach;
        a.power_cost = cost;
        a.hands = crate::ai::combat::fighting_hands(lo, &inv);
        if let Some(g) = &mut a.graph {
            for (var, value) in crate::ai::combat::hand_graph_vars(lo, &inv, speeds) {
                g.set_variable(var, value);
            }
        }
        self.refresh_equipment(key, index, actor);
        true
    }

    /// Draw (or sheathe) an actor's weapon: the graph plays the draw and raises
    /// `weaponDraw` as the hand takes it. False when the graph won't.
    pub fn draw_weapon(&mut self, actor: FormId, draw: bool) -> bool {
        if actor == crate::engine::PLAYER_REF {
            return self.player_draw_weapon(draw);
        }
        let Some(key) = self.actor_cells.get(&actor).copied() else {
            return false;
        };
        let Some(a) = self
            .cells
            .get_mut(&key)
            .and_then(|rt| rt.actors.iter_mut().find(|a| a.ref_id == actor))
        else {
            return false;
        };
        if a.drawn == draw {
            return true;
        }
        let Some(g) = a.graph.as_mut() else {
            return false;
        };
        let took = g.send_event(if draw { "WeapEquip" } else { "Unequip" });
        if took {
            a.drawn = draw;
        }
        took
    }

    pub fn weapon_drawn(&self, actor: FormId) -> bool {
        if actor == crate::engine::PLAYER_REF {
            return self.player_weapon_drawn();
        }
        let Some(key) = self.actor_cells.get(&actor) else {
            return false;
        };
        self.cells
            .get(key)
            .and_then(|rt| rt.actors.iter().find(|a| a.ref_id == actor))
            .is_some_and(|a| a.drawn)
    }

    /// Rebuild an actor's rigid equipment (and carried light) from what it has
    /// equipped.
    pub(crate) fn refresh_equipment(&mut self, key: CellKey, index: usize, actor: FormId) {
        let Some((skel, drawn)) = self
            .cells
            .get(&key)
            .and_then(|rt| rt.actors.get(index))
            .map(|a| (a.skeleton.clone(), a.weapon_out))
        else {
            return;
        };
        let (equipment, light) = self.rigid_equipment(actor, &skel, drawn);
        if let Some(inst) = self
            .scene
            .cells
            .get_mut(&key)
            .and_then(|rc| rc.actors.get_mut(index))
        {
            inst.equipment = equipment;
            inst.held_light = light;
        }
    }

    /// An actor's rigid equipment hung from its skeleton's bones, and the light
    /// it carries: its equipped weapons, shields and lights among its rigid
    /// models, a weapon `drawn` in hand (its scabbard left where it hangs).
    #[allow(clippy::type_complexity)]
    pub(crate) fn rigid_equipment(
        &mut self,
        actor: FormId,
        skel: &crate::world::skeleton::Skeleton,
        drawn: bool,
    ) -> (
        Vec<(std::sync::Arc<crate::render::GpuModel>, usize, Mat4)>,
        Option<HeldLight>,
    ) {
        let inv = self.inventories.get(&actor).cloned().unwrap_or_default();
        let equipped = inv.equipped().to_vec();
        // (model, the bone to hang it from if not its own, light it gives)
        let mut models: Vec<(String, Option<&'static str>, Option<FormId>)> = Vec::new();
        for (item, m) in self
            .rigid_models
            .get(&actor)
            .into_iter()
            .flatten()
            .filter(|(item, _)| equipped.contains(item))
        {
            let weapon = self.lo.tag_of(*item).map(|t| t.0) == Some(*b"WEAP");
            // A weapon in the left hand only (not a two-handed one's).
            let left = weapon && inv.held(LEFT) == Some(*item) && !inv.both_hands();
            let right = !left || inv.held(RIGHT) == Some(*item);
            if weapon && drawn {
                // In hand (bows in the left), the scabbard left where it hangs.
                let bow = self
                    .lo
                    .get(*item)
                    .and_then(|r| r.get(b"DNAM").and_then(|d| d.first().copied()))
                    .is_some_and(|t| t == 7 || t == 9);
                if right {
                    models.push((
                        format!("{m}#blade"),
                        Some(if bow { "SHIELD" } else { "WEAPON" }),
                        None,
                    ));
                    models.push((format!("{m}#scb"), None, None));
                }
                if left {
                    models.push((format!("{m}#blade"), Some("SHIELD"), None));
                }
            } else if right {
                // Sheathed, a left-hand weapon isn't shown: the skeleton
                // has no place on the body for a second one.
                models.push((m.clone(), None, None));
            }
        }
        for &f in &equipped {
            let Some(rec) = self.lo.get(f).filter(|r| r.tag().0 == *b"LIGH") else {
                continue;
            };
            if let Some(m) = crate::world::records::model_path(&rec) {
                models.push((m, None, Some(f)));
            }
        }
        let paths: Vec<String> = models.iter().map(|(m, _, _)| m.clone()).collect();
        self.models.load_all(&mut self.renderer, &self.vfs, &paths);
        let mut equipment = Vec::new();
        let mut light = None;
        for (m, hand, light_form) in models {
            let file = m.split('#').next().unwrap_or(&m).to_owned();
            let bone = match hand {
                Some(h) => skel.find(h),
                None => self.parent_bone(&file).and_then(|b| skel.find(b.as_str())),
            };
            let (Some(model), Some(bone)) = (self.models.get(&m), bone) else {
                continue;
            };
            equipment.push((model, bone, Mat4::IDENTITY));
            if let Some(l) = light_form
                .and_then(|f| self.lo.get(f))
                .and_then(|r| crate::world::records::light_data(&r))
            {
                let offset = self.light_attach_point(&file);
                light = Some(HeldLight {
                    bone,
                    offset,
                    radius: l.radius.max(1.0),
                    color: l.color * l.fade,
                });
            }
        }
        (equipment, light)
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
            .and_then(|n| {
                crate::render::model::node_transforms(&n)
                    .get("AttachLight")
                    .map(|m| m.w_axis.truncate())
            })
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
                Some(crate::render::GpuLight {
                    pos_radius: [p.x, p.y, p.z, l.radius],
                    color: [l.color.x, l.color.y, l.color.z, 1.0],
                })
            })
            .collect();
        if held.is_empty() && !had {
            return;
        }
        self.scene.lights.extend(held);
        self.scene.assign_moving_lights(first);
    }
}

impl Engine {
    /// The armor and weapons of an inventory's equipped items: what an
    /// actor's models show (lights are its torch's); then the weapon in its
    /// left hand again, so that one moving between hands is a change.
    pub(crate) fn worn_items(&self, inv: &crate::world::inventory::Inventory) -> Vec<FormId> {
        let mut worn: Vec<FormId> = inv
            .equipped()
            .iter()
            .copied()
            .filter(|&f| {
                matches!(self.lo.tag_of(f).map(|t| t.0), Some(t) if t == *b"ARMO" || t == *b"WEAP")
            })
            .collect();
        worn.extend(inv.weapon_in(&self.lo, LEFT));
        worn
    }

    /// Actors whose armor or weapons changed (put on by scripts or for
    /// themselves, taken off their bodies) are dressed again.
    pub(crate) fn update_worn(&mut self) {
        let mut changed = Vec::new();
        for rt in self.cells.values() {
            for a in &rt.actors {
                let worn = self
                    .inventories
                    .get(&a.ref_id)
                    .map(|i| self.worn_items(i))
                    .unwrap_or_default();
                if worn != a.worn {
                    changed.push(a.ref_id);
                }
            }
        }
        for actor in changed {
            self.redress(actor);
        }
    }

    /// Rebuild what an actor shows from what it has equipped: the skinned
    /// armor and the body beneath, the head unless a helmet hides it, its
    /// weapon and shield; its footsteps, the graph's hand types and how it
    /// fights follow.
    pub(crate) fn redress(&mut self, actor: FormId) {
        let Some(key) = self.actor_cells.get(&actor).copied() else {
            return;
        };
        let Some(index) = self
            .cells
            .get(&key)
            .and_then(|rt| rt.actors.iter().position(|a| a.ref_id == actor))
        else {
            return;
        };
        let inv = self.inventories.get(&actor).cloned().unwrap_or_default();
        let worn = self.worn_items(&inv);
        let d = self.reference_of(actor).and_then(|rf| {
            crate::world::actor::describe_reference(
                &self.lo,
                &rf,
                &self.scripts.globals,
                Some(&inv),
            )
        });
        let Some(d) = d else {
            // Nothing to build it from: don't try again until it changes.
            if let Some(a) = self.cells.get_mut(&key).map(|rt| &mut rt.actors[index]) {
                a.worn = worn;
            }
            return;
        };
        let paths: Vec<String> = d.models.iter().map(|(m, _)| m.clone()).collect();
        self.models.load_all(&mut self.renderer, &self.vfs, &paths);
        let skel = self.cells[&key].actors[index].skeleton.clone();
        let (meshes, _, rigid) = self.actor_meshes(&d, &skel);
        log::debug!(
            "{actor} dressed again: {} meshes, wearing {worn:?}",
            meshes.len()
        );
        self.rigid_models.insert(actor, rigid);
        if let Some(inst) = self
            .scene
            .cells
            .get_mut(&key)
            .and_then(|rc| rc.actors.get_mut(index))
        {
            inst.meshes = meshes;
        }
        let weapon = inv.weapon(&self.lo);
        let reach = weapon
            .and_then(|w| self.lo.get(w))
            .and_then(|r| {
                r.get(b"DNAM")
                    .filter(|x| x.len() >= 12)
                    .map(|x| f32::from_le_bytes(x[8..12].try_into().unwrap()))
            })
            .unwrap_or(0.0);
        let bow = weapon.filter(|&w| crate::ai::archery::is_bow(&self.lo, w));
        let cost = self.power_attack_cost(actor, weapon);
        let footsteps = d.footsteps.and_then(|f| self.footstep_set(f));
        let speeds = self.weapon_speed_mults(actor);
        let lo = &self.lo;
        let a = &mut self.cells.get_mut(&key).unwrap().actors[index];
        a.worn = worn;
        a.footstep_set = footsteps;
        a.weapon_reach = reach;
        a.power_cost = cost;
        a.hands = crate::ai::combat::fighting_hands(lo, &inv);
        a.bow = bow.is_some();
        if let Some(b) = bow {
            a.bow_speed = crate::ai::archery::bow_speed(lo, b);
        }
        if let Some(g) = &mut a.graph {
            for (var, value) in crate::ai::combat::hand_graph_vars(lo, &inv, speeds) {
                g.set_variable(var, value);
            }
        }
        self.refresh_equipment(key, index, actor);
    }

    /// An actor leaving the loaded world puts its torch away, and takes up
    /// what it set aside for it, so it comes back as it was.
    pub(crate) fn put_torch_away_for_now(&mut self, a: &crate::ai::ActorRuntime) {
        let Some(torch) = a.torch else {
            return;
        };
        let lo = &self.lo;
        if let Some(inv) = self.inventories.get_mut(&a.ref_id) {
            inv.take_off(torch);
            if let Some(s) = a.stowed_left
                && inv.held(LEFT).is_none()
            {
                inv.hold(lo, s, &[LEFT]);
            }
        }
    }

    /// An NPC given an item (placed in its pockets, or a teammate's
    /// inventory) wears or wields it if it is better than what it has: armor
    /// that protects it more than what it covers (clothing: worth more), a
    /// weapon it would pick over its own.
    pub(crate) fn take_up_if_better(&mut self, actor: FormId, item: FormId) {
        if actor == crate::engine::PLAYER_REF
            || self.actor_ref(actor).is_none()
            || self.is_dead(actor)
            || self
                .inventories
                .get(&actor)
                .is_none_or(|i| i.count(item) <= 0 || i.is_equipped(item))
        {
            return;
        }
        let Some(rf) = self.reference_of(actor) else {
            return;
        };
        match self.lo.tag_of(item).map(|t| t.0) {
            Some(t) if t == *b"WEAP" => {
                let inv = self.inventories.get(&actor).cloned().unwrap_or_default();
                let best =
                    crate::world::actor::best_weapon(&self.lo, &rf, &self.scripts.globals, &inv);
                if best == Some(item) && inv.weapon(&self.lo) != Some(item) {
                    log::info!("{actor} wields the {item} it was given");
                    self.wield(actor, item);
                }
            }
            Some(t) if t == *b"ARMO" => {
                if !crate::world::actor::can_wear(&self.lo, &rf, &self.scripts.globals, item) {
                    return;
                }
                let lo = &self.lo;
                let inv = self.inventories.get(&actor).cloned().unwrap_or_default();
                let shield = lo
                    .get(item)
                    .is_some_and(|r| crate::world::inventory::armor_slots(&r) & (1 << 9) != 0);
                let hand = inv.hand(lo, false);
                if shield
                    && !(matches!(hand, HandType::Empty | HandType::Spell) || hand.one_handed())
                {
                    return;
                }
                let now = self.protection(actor).rating;
                let with = self.worn_with(actor, item);
                let then = self.protection_of(actor, &with).rating;
                let value =
                    |f: FormId| crate::world::inventory::item_info(lo, f).map_or(0, |i| i.value);
                let covered: i32 = inv
                    .equipped()
                    .iter()
                    .filter(|f| !with.contains(f))
                    .map(|&f| value(f))
                    .sum();
                let better = then > now || (then == now && value(item) > covered);
                log::info!(
                    "{actor} given {item}: armor {now:.0} -> {then:.0}, worth {} for {covered}{}",
                    value(item),
                    if better { ": wears it" } else { "" }
                );
                if better {
                    let _ = self.equip_armor(actor, item, true);
                }
            }
            _ => {}
        }
    }
}
