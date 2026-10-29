//! Loose objects: clutter, weapons, food and the like whose models' rigid
//! bodies are simulated (`MO_SYS_DYNAMIC`). Each of a model's bodies is a body
//! in the physics world (a cart's frame and wheels, joined by hinges), asleep
//! until disturbed; the parts of the model drawn on each body's node follow it,
//! and where the object comes to rest is remembered like a scripted move.

use esp::FormId;
use glam::{EulerRot, Mat4, Quat, Vec3};
use rapier3d::prelude::RigidBodyHandle;

use crate::engine::Engine;
use crate::physics::Motion;
use crate::render::CellKey;

/// How far below where it started a loose object may fall before it is put
/// back (through a gap in the world's collision).
const LOST_DEPTH: f32 = 4096.0;

/// Where dropped items appear: ahead of the one dropping them, at about waist
/// height (game units; no source).
const DROP_AHEAD: f32 = 50.0;
const DROP_HEIGHT: f32 = 70.0;
/// Apart, side by side, the references of one drop from different owners
/// (game units; no source).
const DROP_SPREAD: f32 = 16.0;

/// Steps after loading during which loose objects are held where they were
/// placed: the physics engine wakes bodies as their colliders go in and it
/// first finds their contacts, but placed objects stay put (asleep) until
/// something disturbs them.
const SETTLE_STEPS: u32 = 3;

/// One of a loose object's bodies.
pub(crate) struct LooseBody {
    pub handle: RigidBodyHandle,
    /// Its frame in the model (`BodyDesc::frame`), and where it is in the world
    /// now (the object's scale included).
    pub frame: Mat4,
    pub world: Mat4,
    /// The render instances (of its cell) drawn with it.
    pub instances: Vec<usize>,
}

/// A loose object in a loaded cell.
pub(crate) struct Loose {
    pub ref_id: FormId,
    pub bodies: Vec<LooseBody>,
    /// The body whose place is the reference's (the model root's).
    pub root: usize,
    /// Where the reference is now (scale included; from the root body), and
    /// where it was placed.
    pub transform: Mat4,
    pub start: Mat4,
    /// It has moved since it last came to rest; times it fell out of the world.
    pub moved: bool,
    pub lost: u32,
    /// Steps since it was added (`SETTLE_STEPS`), and whether it was just made
    /// (it falls rather than lying still).
    pub age: u32,
    pub fresh: bool,
}

/// Euler angles (radians, as references' `DATA`) of a rotation.
pub(crate) fn euler_of(q: Quat) -> Vec3 {
    let (x, y, z) = q.to_euler(EulerRot::XYZ);
    -Vec3::new(x, y, z)
}

impl Engine {
    /// Make a loose object's bodies if its model has them; false for anything
    /// else. `instances`: its render instances in the cell, by the node they
    /// are drawn on (none for the model's root). One just made (`fresh`) falls
    /// at once; others lie still until disturbed.
    pub(crate) fn add_loose(
        &mut self,
        key: CellKey,
        r: FormId,
        model: &str,
        transform: Mat4,
        fresh: bool,
        instances: Vec<(Option<String>, usize)>,
    ) -> bool {
        let Some(c) = self.models.collision(model).filter(|c| c.is_loose()) else {
            return false;
        };
        let broken = self
            .world_state
            .broken_joints
            .get(&r)
            .cloned()
            .unwrap_or_default();
        let handles = self.physics.add_loose(&c, transform, r, &broken);
        let mut bodies: Vec<LooseBody> = Vec::new();
        let mut index = vec![None; handles.len()];
        for (i, h) in handles.iter().enumerate() {
            if let Some(h) = *h {
                index[i] = Some(bodies.len());
                let frame = c.bodies[i].frame;
                bodies.push(LooseBody {
                    handle: h,
                    frame,
                    world: transform * frame,
                    instances: Vec::new(),
                });
            }
        }
        if bodies.is_empty() {
            return false;
        }
        let root = c
            .bodies
            .iter()
            .position(|b| b.root)
            .and_then(|i| index[i])
            .unwrap_or(0);
        for (node, i) in instances {
            let body = node
                .and_then(|n| c.bodies.iter().position(|b| b.node == n))
                .and_then(|b| index[b])
                .unwrap_or(root);
            bodies[body].instances.push(i);
        }
        if self.is_disabled(r) {
            self.physics.set_owner_enabled(r, false);
        }
        if let Some(rt) = self.cells.get_mut(&key) {
            rt.loose.push(Loose {
                ref_id: r,
                bodies,
                root,
                transform,
                start: transform,
                moved: false,
                lost: 0,
                age: 0,
                fresh,
            });
        }
        true
    }

    /// Draw loose objects where their bodies are; remember where those that
    /// moved come to rest.
    pub(crate) fn update_loose(&mut self) {
        let mut settled: Vec<(FormId, Vec3, Vec3)> = Vec::new();
        let mut lost: Vec<(RigidBodyHandle, Mat4, bool)> = Vec::new();
        let mut release: Vec<(RigidBodyHandle, bool)> = Vec::new();
        let mut moved_refs: Vec<(FormId, Vec3)> = Vec::new();
        for (key, rt) in self.cells.iter_mut() {
            let mut rc = self.scene.cells.get_mut(key);
            for l in rt.loose.iter_mut() {
                if l.age < SETTLE_STEPS {
                    l.age += 1;
                    if l.age == SETTLE_STEPS {
                        release.extend(l.bodies.iter().map(|b| (b.handle, !l.fresh)));
                    }
                    continue;
                }
                let scale =
                    Mat4::from_scale(Vec3::splat(l.transform.to_scale_rotation_translation().0.x));
                let mut asleep = true;
                let mut changed = false;
                for b in l.bodies.iter_mut() {
                    let Some((pose, sleeping)) = self.physics.body_pose(b.handle) else {
                        continue;
                    };
                    asleep &= sleeping;
                    let now = pose * scale;
                    if now.abs_diff_eq(b.world, 1e-3) {
                        continue;
                    }
                    let delta = now * b.world.inverse();
                    b.world = now;
                    changed = true;
                    if let Some(rc) = rc.as_deref_mut() {
                        for &i in &b.instances {
                            if let Some(inst) = rc.instances.get_mut(i) {
                                inst.transform = delta * inst.transform;
                                inst.world_center =
                                    inst.transform.transform_point3(inst.model.bound_center);
                            }
                        }
                    }
                }
                let root = &l.bodies[l.root];
                let now = root.world * root.frame.inverse();
                let (pos, start) = (now.w_axis.truncate(), l.start.w_axis.truncate());
                if pos.z < start.z - LOST_DEPTH {
                    l.lost += 1;
                    // Again: it is caught in the world's collision; it stays put.
                    let hold = l.lost > 1;
                    log::info!(
                        "{} fell out of the world; put back{}",
                        l.ref_id,
                        if hold { " and held" } else { "" }
                    );
                    lost.extend(l.bodies.iter().map(|b| (b.handle, l.start * b.frame, hold)));
                    continue;
                }
                if changed {
                    l.transform = now;
                    l.moved = true;
                    moved_refs.push((l.ref_id, pos));
                }
                if asleep && l.moved {
                    l.moved = false;
                    let (_, rot, _) = now.to_scale_rotation_translation();
                    let turned = rot.angle_between(l.start.to_scale_rotation_translation().1);
                    log::debug!(
                        "{} comes to rest {:.1} units, {:.0} deg from its place",
                        l.ref_id,
                        pos.distance(start),
                        turned.to_degrees()
                    );
                    settled.push((l.ref_id, pos, euler_of(rot)));
                }
            }
        }
        for (h, asleep) in release {
            self.physics.release(h, asleep);
        }
        for (r, i) in std::mem::take(&mut self.physics.broken) {
            self.world_state.broken_joints.entry(r).or_default().push(i);
        }
        for (h, m, hold) in lost {
            self.physics.set_body_pose(h, m);
            if let Some(b) = self.physics.world.bodies.get_mut(h) {
                if hold {
                    b.set_body_type(rapier3d::prelude::RigidBodyType::Fixed, false);
                }
                b.sleep();
            }
        }
        self.moved_refs.extend(moved_refs);
        for (r, pos, rot) in settled {
            self.remember_rest(r, pos, rot);
        }
    }

    /// Keep where a loose object lies across loads.
    fn remember_rest(&mut self, r: FormId, pos: Vec3, rot: Vec3) {
        let Some((like, _)) = self.current_place_of(r) else {
            return;
        };
        let place = Self::place_at(like, pos);
        self.world_state
            .moved
            .insert(r, crate::world_state::Moved { place, pos, rot });
        if let Some(c) = self.created_refs.refs.get_mut(&r) {
            (c.position, c.rotation, c.place) = (pos, rot, Some(place));
        }
    }

    /// A cell unloading: remember where its loose objects still moving are, and
    /// take their bodies out.
    pub(crate) fn unload_loose(&mut self, key: CellKey) {
        let Some(rt) = self.cells.get_mut(&key) else {
            return;
        };
        let loose = std::mem::take(&mut rt.loose);
        for l in loose {
            if l.moved {
                let (_, rot, pos) = l.transform.to_scale_rotation_translation();
                self.remember_rest(l.ref_id, pos, euler_of(rot));
            }
            for b in &l.bodies {
                self.physics.remove_body(b.handle);
            }
        }
    }

    /// A loose object moved by a script (`MoveTo`, `SetPosition`): drawn there
    /// already, so the body's move isn't drawn again.
    pub(crate) fn loose_moved(&mut self, r: FormId, delta: Mat4) {
        for l in self
            .cells
            .values_mut()
            .flat_map(|rt| rt.loose.iter_mut())
            .filter(|l| l.ref_id == r)
        {
            l.transform = delta * l.transform;
            for b in l.bodies.iter_mut() {
                b.world = delta * b.world;
            }
        }
    }

    /// Something moves a loose object while it is still being held in place
    /// after loading: let it go.
    pub(crate) fn disturb(&mut self, r: FormId) {
        for l in self
            .cells
            .values_mut()
            .flat_map(|rt| rt.loose.iter_mut())
            .filter(|l| l.ref_id == r && l.age < SETTLE_STEPS)
        {
            l.age = SETTLE_STEPS;
            for b in &l.bodies {
                self.physics.release(b.handle, false);
            }
        }
    }

    /// Drop `count` of `item` from `holder` (the player or an actor): out of
    /// its inventory and into the world in front of it as one reference,
    /// falling. Papyrus `DropObject`, the console's `drop`, the inventory's
    /// right click.
    pub fn drop_item(&mut self, holder: FormId, item: FormId, count: i32) -> Option<FormId> {
        self.drop_item_from_stack(holder, item, count, None)
    }

    /// As [`drop_item`], taking only the selected ownership row when supplied.
    pub fn drop_item_from_stack(
        &mut self,
        holder: FormId,
        item: FormId,
        count: i32,
        stack: Option<Option<FormId>>,
    ) -> Option<FormId> {
        if !crate::world::inventory::item_droppable(&self.lo, item) {
            return None;
        }
        let (place, feet) = self.current_place_of(holder)?;
        let heading = if holder == crate::engine::PLAYER_REF {
            self.camera.yaw
        } else {
            self.ref_rotation(holder).z
        };
        let removed = self.remove_stack_with_extras(holder, item, count.max(1), stack, None, None);
        let n: i32 = removed.parts.iter().map(|(_, n)| n).sum();
        if n <= 0 {
            return None;
        }
        self.unequip_if_gone(holder, item);
        let ahead = Vec3::new(heading.sin(), heading.cos(), 0.0);
        let side = Vec3::new(ahead.y, -ahead.x, 0.0);
        let at = feet + ahead * DROP_AHEAD + Vec3::Z * DROP_HEIGHT;
        let from = self.object_value(holder);
        // One reference per owner, so what the player stole stays stolen
        // (UESP *Skyrim:Crime*: stolen items "tracked separately when dropped").
        // Which copy had which charge isn't kept, so the charges are shared out
        // in order.
        let mut charges = removed.charges.into_iter();
        let mut first = None;
        for (i, (owner, k)) in removed.parts.into_iter().enumerate() {
            let spot = at + side * (i as f32 * DROP_SPREAD);
            let r = self.create_object_at(item, place, spot, Vec3::new(0.0, 0.0, heading), k);
            if let Some(c) = self.created_refs.refs.get_mut(&r) {
                c.owner = owner;
            }
            let copies: Vec<_> = removed
                .copies
                .iter()
                .filter(|c| c.owner == owner)
                .cloned()
                .collect();
            if !copies.is_empty() {
                self.ref_state.item_copies.insert(r, copies);
            }
            let mine: Vec<f32> = charges.by_ref().take(k as usize).collect();
            if !mine.is_empty() {
                self.world_state.item_charges.insert(r, mine);
            }
            log::info!("{holder} drops {k} {item} (owner {owner:?}) as {r}");
            self.send_script_event(
                r,
                "OnContainerChanged",
                vec![papyrus::Value::None, from.clone()],
            );
            if holder == crate::engine::PLAYER_REF {
                self.send_player_remove_item(item, r, 4);
            }
            first.get_or_insert(r);
        }
        first
    }

    /// Console `loose [n]`: the loose objects nearest the player.
    pub fn describe_loose(&self, n: usize) -> Vec<String> {
        let player = self
            .ref_position(crate::engine::PLAYER_REF)
            .unwrap_or_default();
        let mut all: Vec<&Loose> = self.cells.values().flat_map(|rt| &rt.loose).collect();
        let at = |l: &Loose| l.transform.w_axis.truncate();
        all.sort_by(|a, b| at(a).distance(player).total_cmp(&at(b).distance(player)));
        let mut out = vec![format!("{} loose objects loaded", all.len())];
        for l in all.into_iter().take(n) {
            let p = at(l);
            let asleep = l
                .bodies
                .iter()
                .all(|b| self.physics.body_pose(b.handle).is_some_and(|(_, s)| s));
            let mass: f32 = l
                .bodies
                .iter()
                .map(|b| self.physics.body_mass_speed(b.handle).0)
                .sum();
            let speed = self.physics.body_mass_speed(l.bodies[l.root].handle).1;
            let name = self
                .base_of(l.ref_id)
                .and_then(|b| self.lo.get(b))
                .and_then(|r| r.editor_id().map(|e| e.to_string()))
                .unwrap_or_default();
            out.push(format!(
                "{} {name} at {:.0} {:.0} {:.0} ({:.0} away, {:.0} from its place), {} bodies, {:.1} kg, {:.1} u/s, {:?}{}",
                l.ref_id,
                p.x,
                p.y,
                p.z,
                p.distance(player),
                p.distance(l.start.w_axis.truncate()),
                l.bodies.len(),
                mass,
                speed,
                self.physics.motion(l.ref_id),
                if asleep { ", asleep" } else { "" }
            ));
        }
        out
    }

    /// Something struck by `attacker`'s blow or projectile (none for a blow):
    /// a destructible one loses `damage` health, what moves of it is pushed by
    /// `impulse` (kg game units / s), and its scripts hear of it (`OnHit`).
    pub(crate) fn strike(
        &mut self,
        what: FormId,
        attacker: FormId,
        impulse: Vec3,
        power: bool,
        damage: f32,
    ) {
        self.disturb(what);
        self.damage_object_by_blow(what, damage);
        self.physics.apply_impulse(what, impulse);
        self.send_hit_event(what, attacker, power, false, false, false);
    }

    /// Papyrus `ApplyHavokImpulse(x, y, z, magnitude)`: a push along the
    /// direction, its magnitude in Havok units (kg m/s).
    pub fn apply_havok_impulse(&mut self, r: FormId, dir: Vec3, magnitude: f32) -> bool {
        let impulse = dir.normalize_or_zero() * magnitude * crate::physics::METRE;
        self.disturb(r);
        self.physics.apply_impulse(r, impulse)
    }

    /// Papyrus `SetMotionType`: 1, 2, 3, 6 dynamic, 4 keyframed, 5 fixed.
    pub fn set_motion_type(&mut self, r: FormId, motion: i32) -> bool {
        let m = match motion {
            4 => Motion::Keyframed,
            5 => Motion::Fixed,
            1..=3 | 6 => Motion::Dynamic,
            _ => return false,
        };
        log::debug!("{r} motion {m:?}");
        self.disturb(r);
        self.physics.set_motion(r, m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Location, PLAYER_REF};
    use esp::writer::{Node, RecordBuilder, write_plugin};

    const ITEM: FormId = FormId(0x800);
    const OWNER: FormId = FormId(0x801);
    const ACTOR: FormId = FormId(0x802);
    const HOUSE: FormId = FormId(0x803);
    const WORLD_ITEM: FormId = FormId(0x804);

    /// An interior owned by `OWNER`, holding one placed `ITEM` (which is theirs
    /// too), and an actor of `OWNER` to name it in the inventory.
    fn house() -> (std::path::PathBuf, Engine) {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("vrm-loose-{}-{stamp}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let cell = RecordBuilder::new(b"CELL", HOUSE)
            .field(b"DATA", &[0x21, 0])
            .form(b"XOWN", OWNER);
        let placed = RecordBuilder::new(b"REFR", WORLD_ITEM).form(b"NAME", ITEM);
        let records = [
            Node::top(
                b"MISC",
                vec![
                    RecordBuilder::new(b"MISC", ITEM)
                        .zstring(b"FULL", "Loose Test Item")
                        .field(
                            b"DATA",
                            &[&100i32.to_le_bytes()[..], &1.0f32.to_le_bytes()].concat(),
                        ),
                ],
            ),
            Node::top(b"NPC_", vec![RecordBuilder::new(b"NPC_", OWNER)]),
            Node::top(
                b"ACHR",
                vec![RecordBuilder::new(b"ACHR", ACTOR).form(b"NAME", OWNER)],
            ),
            Node::Group {
                label: *b"CELL",
                group_type: 0,
                children: vec![
                    Node::Record(cell),
                    Node::Group {
                        label: HOUSE.0.to_le_bytes(),
                        group_type: esp::group_type::CELL_CHILDREN,
                        children: vec![Node::Group {
                            label: HOUSE.0.to_le_bytes(),
                            group_type: esp::group_type::CELL_TEMPORARY,
                            children: vec![Node::Record(placed)],
                        }],
                    },
                ],
            },
        ];
        std::fs::write(
            dir.join("Loose.esp"),
            write_plugin(0, &[], "test", "loose items", 0x900, &records),
        )
        .unwrap();
        let mut e = crate::stats::action_tests::headless_engine(&dir, "Loose.esp");
        e.location = Location::Interior(HOUSE);
        (dir, e)
    }

    #[test]
    fn euler_round_trip() {
        let rot = Vec3::new(0.3, -0.2, 2.5);
        let q = crate::world::records::rotation_from_euler(rot);
        let back = crate::world::records::rotation_from_euler(euler_of(q));
        assert!(q.angle_between(back) < 1e-4);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
    fn a_dropped_stolen_item_stays_stolen_when_picked_up_again() {
        let (dir, mut e) = house();
        e.inventory_mut(PLAYER_REF).add_owned(ITEM, Some(OWNER), 2);
        let r = e.drop_item(PLAYER_REF, ITEM, 2).unwrap();
        assert_eq!(e.created(r).unwrap().owner, Some(OWNER));
        assert_eq!(e.ref_owner(r), Some(OWNER));
        assert_eq!(e.item_count(PLAYER_REF, ITEM), 0);

        assert!(e.take_item(r));
        assert_eq!(
            e.inventory_mut(PLAYER_REF).count_owned(ITEM, Some(OWNER)),
            2
        );
        assert_eq!(e.inventory_mut(PLAYER_REF).count_owned(ITEM, None), 0);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
    fn a_dropped_own_item_is_not_theft_even_in_the_owners_house() {
        let (dir, mut e) = house();
        e.add_item(PLAYER_REF, ITEM, 2);
        let r = e.drop_item(PLAYER_REF, ITEM, 2).unwrap();
        assert_eq!(e.created(r).unwrap().owner, None);
        // The house's owner doesn't reach a reference the player made.
        assert_eq!(e.ref_owner(r), None);
        assert!(e.take_item(r));
        assert_eq!(e.inventory_mut(PLAYER_REF).count_owned(ITEM, None), 2);
        assert_eq!(
            e.inventory_mut(PLAYER_REF).count_owned(ITEM, Some(OWNER)),
            0
        );

        // What was placed there is the owner's: taking it is theft.
        assert!(e.take_item(WORLD_ITEM));
        assert_eq!(
            e.inventory_mut(PLAYER_REF).count_owned(ITEM, Some(OWNER)),
            1
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    #[ignore = "needs a headless GPU adapter to construct Engine; synthetic records only"]
    fn dropping_a_mixed_stack_makes_one_reference_per_owner() {
        let (dir, mut e) = house();
        e.add_item(PLAYER_REF, ITEM, 1);
        e.inventory_mut(PLAYER_REF).add_owned(ITEM, Some(OWNER), 2);
        let first = e.drop_item(PLAYER_REF, ITEM, 3).unwrap();
        assert_eq!(e.created(first).unwrap().count, 1);
        assert_eq!(e.created(first).unwrap().owner, None);
        let stolen: Vec<FormId> = e
            .created_refs
            .refs
            .iter()
            .filter(|(_, c)| c.owner == Some(OWNER))
            .map(|(&id, _)| id)
            .collect();
        assert_eq!(stolen.len(), 1);
        assert_eq!(e.created(stolen[0]).unwrap().count, 2);
        assert_eq!(e.item_count(PLAYER_REF, ITEM), 0);

        assert!(e.take_item(first));
        assert!(e.take_item(stolen[0]));
        assert_eq!(
            e.inventory_mut(PLAYER_REF).count_owned(ITEM, Some(OWNER)),
            2
        );
        assert_eq!(e.inventory_mut(PLAYER_REF).count_owned(ITEM, None), 1);
        let _ = std::fs::remove_dir_all(dir);
    }
}
