//! Keyframe-animated objects: doors that open and close, and statics that loop
//! an "Idle" sequence (water wheels, flags, machinery).

use std::sync::Arc;

use esp::FormId;
use glam::{Mat4, Vec3};
use rapier3d::prelude::ColliderHandle;

use crate::engine::{Engine, PLAYER_REF};
use crate::render::{CellKey, Instance};
use crate::world::loader::ModelAnim;

/// Walking actors this close to a closed door open it.
const OPEN_DISTANCE: f32 = 110.0;
/// An auto-opened door closes once nobody has been this close for `CLOSE_DELAY`.
const CLEAR_DISTANCE: f32 = 160.0;
const CLOSE_DELAY: f32 = 2.5;

pub struct AnimatedObject {
    pub ref_id: FormId,
    pub transform: Mat4,
    pub position: Vec3,
    pub anim: Arc<ModelAnim>,
    /// Render instance index (in the cell) of each part, in `anim.parts` order.
    pub instances: Vec<usize>,
    /// Colliders that move with an animated node (door leaves).
    pub colliders: Vec<ColliderHandle>,
    pub door: bool,
    pub open: bool,
    /// Opened for a passing actor (closes again by itself).
    pub auto: bool,
    pub clear_for: f32,
    /// Sequence playing and its time.
    pub playing: Option<(usize, f32)>,
}

impl AnimatedObject {
    fn play(&mut self, name: &str) -> bool {
        let Some(i) = self.anim.sequences.iter().position(|s| s.name.eq_ignore_ascii_case(name)) else { return false };
        self.playing = Some((i, self.anim.sequences[i].start));
        true
    }

    /// Advance the playing sequence and pose the parts.
    fn update(&mut self, dt: f32, instances: &mut [Instance]) {
        let Some((si, t)) = &mut self.playing else { return };
        let seq = &self.anim.sequences[*si];
        *t += dt;
        let len = (seq.stop - seq.start).max(1e-3);
        if *t > seq.stop {
            if seq.cycle == 0 {
                *t = seq.start + (*t - seq.start) % len;
            } else {
                *t = seq.stop;
            }
        }
        let t = *t;
        for (part, &ii) in self.anim.parts.iter().zip(&self.instances) {
            let Some(ch) = seq.channels.iter().find(|c| c.node.eq_ignore_ascii_case(&part.node)) else { continue };
            if let Some(inst) = instances.get_mut(ii) {
                set_transform(inst, self.transform * part.parent * ch.sample(t, part.rest));
            }
        }
        if seq.cycle != 0 && t >= seq.stop {
            self.playing = None;
        }
    }
}

fn set_transform(inst: &mut Instance, m: Mat4) {
    inst.transform = m;
    inst.world_center = m.transform_point3(inst.model.bound_center);
}

impl Engine {
    /// Add render instances for an object's animated parts (at rest) and track it.
    /// Returns the object if the model has animated parts.
    pub(crate) fn animated_object(
        &self,
        ref_id: FormId,
        model: &str,
        transform: Mat4,
        instances: &mut Vec<Instance>,
        colliders: &[(ColliderHandle, Option<String>)],
    ) -> Option<AnimatedObject> {
        let anim = self.models.anim(model)?;
        let mut idx = Vec::new();
        for part in &anim.parts {
            let mut inst = Instance::new(part.model.clone(), transform * part.parent * part.rest);
            inst.ref_id = ref_id.0;
            inst.hidden = self.is_disabled(ref_id);
            idx.push(instances.len());
            instances.push(inst);
        }
        let door = self.base_of(ref_id).and_then(|b| self.lo.tag_of(b)).is_some_and(|t| t.0 == *b"DOOR");
        let mut obj = AnimatedObject {
            ref_id,
            transform,
            position: transform.w_axis.truncate(),
            anim,
            instances: idx,
            colliders: colliders.iter().filter(|(_, n)| n.is_some()).map(|(h, _)| *h).collect(),
            door,
            open: false,
            auto: false,
            clear_for: 0.0,
            playing: None,
        };
        // Machinery and the like loop their idle animation.
        if !door {
            obj.play("Idle");
        } else if self.world_state.open_doors.contains(&ref_id) && obj.play("Open") {
            // Left open: at the end of its opening (posed on the first update).
            if let Some((si, t)) = &mut obj.playing {
                *t = obj.anim.sequences[*si].stop;
            }
            obj.open = true;
        }
        Some(obj)
    }

    /// Open or close a (non-load) door. Returns false if it isn't an animated door.
    pub fn toggle_door(&mut self, door: FormId, auto: bool) -> bool {
        let Some((key, i)) = self.find_animated(door) else { return false };
        let Some(rt) = self.cells.get_mut(&key) else { return false };
        let obj = &mut rt.animated[i];
        if !obj.door {
            return false;
        }
        let open = !obj.open;
        if !obj.play(if open { "Open" } else { "Close" }) {
            log::debug!("door {door}: no {} sequence in {:?}", if open { "Open" } else { "Close" }, obj.anim.sequences.iter().map(|s| &s.name).collect::<Vec<_>>());
            return false;
        }
        obj.open = open;
        obj.auto = auto && open;
        // Doors the player opens stay open across loads (actors' close behind them).
        if open && !auto {
            self.world_state.open_doors.insert(door);
        } else if !open {
            self.world_state.open_doors.remove(&door);
        }
        obj.clear_for = 0.0;
        let colliders = obj.colliders.clone();
        // The leaves stop blocking as soon as the door starts to open.
        self.physics.set_enabled(&colliders, !open);
        log::debug!("door {door} {}", if open { "opens" } else { "closes" });
        true
    }

    fn find_animated(&self, r: FormId) -> Option<(CellKey, usize)> {
        self.cells.iter().find_map(|(k, rt)| rt.animated.iter().position(|a| a.ref_id == r).map(|i| (*k, i)))
    }

    /// Animate objects; open doors for actors walking up to them and close them after.
    pub(crate) fn update_animated(&mut self, dt: f32) {
        let player = self.ref_position(PLAYER_REF).unwrap_or_default();
        let walkers: Vec<(FormId, Vec3)> =
            self.cells.values().flat_map(|rt| &rt.actors).filter(|a| a.is_walking()).map(|a| (a.ref_id, a.pos)).collect();
        let near = |p: Vec3, d: f32, with_player: bool| {
            walkers.iter().any(|w| w.1.distance(p) < d) || (with_player && player.distance(p) < d)
        };
        // Closed doors someone walks up to, if they get past its lock (so a bandit
        // walking by doesn't open a locked gate).
        let mut opening = Vec::new();
        for obj in self.cells.values().flat_map(|rt| &rt.animated) {
            if obj.door && !obj.open && obj.playing.is_none() {
                let at: Vec<FormId> = walkers.iter().filter(|w| w.1.distance(obj.position) < OPEN_DISTANCE).map(|w| w.0).collect();
                if at.is_empty() {
                    continue;
                }
                if !self.is_locked(obj.ref_id) || at.iter().any(|&w| self.npc_may_open(w, obj.ref_id)) {
                    opening.push(obj.ref_id);
                } else {
                    log::trace!("locked door {} stays shut for {at:?}", obj.ref_id);
                }
            }
        }
        if log::log_enabled!(log::Level::Trace) {
            for rt in self.cells.values() {
                for obj in rt.animated.iter().filter(|o| o.door && !o.open) {
                    let d = walkers.iter().map(|w| w.1.distance(obj.position)).fold(f32::MAX, f32::min);
                    if d < 400.0 {
                        log::trace!("closed door {} nearest walker {d:.0}", obj.ref_id);
                    }
                }
            }
        }
        let mut toggle = Vec::new();
        for (key, rt) in self.cells.iter_mut() {
            for obj in &mut rt.animated {
                if !obj.door || obj.playing.is_some() {
                    continue;
                }
                if !obj.open && opening.contains(&obj.ref_id) {
                    toggle.push((obj.ref_id, true));
                } else if obj.open && obj.auto {
                    if near(obj.position, CLEAR_DISTANCE, true) {
                        obj.clear_for = 0.0;
                    } else {
                        obj.clear_for += dt;
                        if obj.clear_for > CLOSE_DELAY {
                            toggle.push((obj.ref_id, false));
                        }
                    }
                }
            }
            if let Some(rc) = self.scene.cells.get_mut(key) {
                for obj in &mut rt.animated {
                    obj.update(dt, &mut rc.instances);
                }
            }
        }
        for (door, auto) in toggle {
            self.toggle_door(door, auto);
        }
    }
}
