//! Animated objects: doors that open and close, statics that loop an "Idle"
//! keyframe sequence (water wheels, flags, machinery), and objects run by a
//! behaviour graph of their own (traps, pressure plates, levers), whose clips
//! move the model's parts and the collision on them as `PlayAnimation`'s
//! events drive it.

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
    /// The behaviour graph running it, if its model has one.
    pub graph: Option<ObjectGraph>,
}

/// An object's running behaviour graph.
pub struct ObjectGraph {
    anim: crate::world::behavior::GraphAnim,
    skeleton: Arc<crate::world::skeleton::Skeleton>,
    /// The skeleton bone of each part (in `anim.parts` order).
    bones: Vec<Option<usize>>,
    /// Colliders on the moving nodes: their bone and pose relative to it.
    colliders: Vec<(ColliderHandle, usize, Mat4)>,
}

impl AnimatedObject {
    fn play(&mut self, name: &str) -> bool {
        let Some(i) = self
            .anim
            .sequences
            .iter()
            .position(|s| s.name.eq_ignore_ascii_case(name))
        else {
            return false;
        };
        self.playing = Some((i, self.anim.sequences[i].start));
        true
    }

    /// Advance the playing sequence and pose the parts.
    fn update(&mut self, dt: f32, instances: &mut [Instance]) {
        let Some((si, t)) = &mut self.playing else {
            return;
        };
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
            let Some(ch) = seq
                .channels
                .iter()
                .find(|c| c.node.eq_ignore_ascii_case(&part.node))
            else {
                continue;
            };
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
        let door = self
            .base_of(ref_id)
            .and_then(|b| self.lo.tag_of(b))
            .is_some_and(|t| t.0 == *b"DOOR");
        let mut obj = AnimatedObject {
            ref_id,
            transform,
            position: transform.w_axis.truncate(),
            anim,
            instances: idx,
            colliders: colliders
                .iter()
                .filter(|(_, n)| n.is_some())
                .map(|(h, _)| *h)
                .collect(),
            door,
            open: false,
            auto: false,
            clear_for: 0.0,
            playing: None,
            graph: None,
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
        let Some((key, i)) = self.find_animated(door) else {
            return false;
        };
        let Some(rt) = self.cells.get_mut(&key) else {
            return false;
        };
        let obj = &mut rt.animated[i];
        if !obj.door {
            return false;
        }
        let open = !obj.open;
        if !obj.play(if open { "Open" } else { "Close" }) {
            log::debug!(
                "door {door}: no {} sequence in {:?}",
                if open { "Open" } else { "Close" },
                obj.anim
                    .sequences
                    .iter()
                    .map(|s| &s.name)
                    .collect::<Vec<_>>()
            );
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

    /// Start an object's behaviour graph, if its model has one: its parts and
    /// the colliders on them follow the graph's pose from now on.
    pub(crate) fn start_object_graph(
        &mut self,
        obj: &mut AnimatedObject,
        tagged: &[(ColliderHandle, Option<String>)],
    ) {
        let Some((path, skeleton)) = obj.anim.graph.clone() else {
            return;
        };
        let Some(project) = self.graphs.project(&self.vfs, &path) else {
            log::debug!("{}: no behaviour project {path}", obj.ref_id);
            return;
        };
        let Some(rig) = project.shared.project.character.as_ref().map(|c| {
            let rig = c.rig.to_ascii_lowercase().replace('\\', "/");
            let rig = rig.strip_suffix(".hkx").unwrap_or(&rig).to_owned();
            format!("{}/{rig}.nif", project.dir)
        }) else {
            return;
        };
        let mut anim = crate::world::behavior::GraphAnim::new(
            project,
            &rig,
            false,
            &skeleton,
            self.rng ^ obj.ref_id.0 as u64,
        );
        anim.label = obj.ref_id.to_string();
        let bones: Vec<Option<usize>> = obj
            .anim
            .parts
            .iter()
            .map(|p| skeleton.find(&p.node))
            .collect();
        let rest = skeleton.model_space(&skeleton.bind_locals());
        let colliders = tagged
            .iter()
            .filter_map(|(h, node)| {
                let b = skeleton.find(node.as_deref()?)?;
                let at = self.physics.world.colliders.get(*h)?.position().to_mat4();
                Some((*h, b, (obj.transform * rest[b]).inverse() * at))
            })
            .collect();
        log::debug!(
            "{}: behaviour graph {path} ({} parts, {} colliders)",
            obj.ref_id,
            bones.iter().flatten().count(),
            tagged.iter().filter(|t| t.1.is_some()).count()
        );
        obj.graph = Some(ObjectGraph {
            anim,
            skeleton,
            bones,
            colliders,
        });
    }

    /// Papyrus `PlayAnimation(event)`: the event to the object's behaviour
    /// graph. False if it has none.
    pub fn play_object_animation(&mut self, r: FormId, event: &str) -> bool {
        let Some((key, i)) = self.find_animated(r) else {
            return false;
        };
        let Some(g) = self
            .cells
            .get_mut(&key)
            .and_then(|rt| rt.animated[i].graph.as_mut())
        else {
            return false;
        };
        log::debug!("{r}: animation event {event}");
        g.anim.send_event(event)
    }

    /// Papyrus `PlayGamebryoAnimation(name)`: one of the model's keyframe
    /// sequences.
    pub fn play_gamebryo_animation(&mut self, r: FormId, name: &str) -> bool {
        let Some((key, i)) = self.find_animated(r) else {
            return false;
        };
        self.cells
            .get_mut(&key)
            .is_some_and(|rt| rt.animated[i].play(name))
    }

    /// Whether `r` is an object running a behaviour graph.
    pub(crate) fn object_has_graph(&self, r: FormId) -> bool {
        self.find_animated(r)
            .is_some_and(|(k, i)| self.cells[&k].animated[i].graph.is_some())
    }

    fn find_animated(&self, r: FormId) -> Option<(CellKey, usize)> {
        self.cells.iter().find_map(|(k, rt)| {
            rt.animated
                .iter()
                .position(|a| a.ref_id == r)
                .map(|i| (*k, i))
        })
    }

    /// Animate objects; open doors for actors walking up to them and close them after.
    pub(crate) fn update_animated(&mut self, dt: f32) {
        let player = self.ref_position(PLAYER_REF).unwrap_or_default();
        let walkers: Vec<(FormId, Vec3)> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| a.is_walking())
            .map(|a| (a.ref_id, a.pos))
            .collect();
        let near = |p: Vec3, d: f32, with_player: bool| {
            walkers.iter().any(|w| w.1.distance(p) < d) || (with_player && player.distance(p) < d)
        };
        // Closed doors someone walks up to, if they get past its lock (so a bandit
        // walking by doesn't open a locked gate).
        let mut opening = Vec::new();
        for obj in self.cells.values().flat_map(|rt| &rt.animated) {
            if obj.door && !obj.open && obj.playing.is_none() {
                let at: Vec<FormId> = walkers
                    .iter()
                    .filter(|w| w.1.distance(obj.position) < OPEN_DISTANCE)
                    .map(|w| w.0)
                    .collect();
                if at.is_empty() {
                    continue;
                }
                if !self.is_locked(obj.ref_id)
                    || at.iter().any(|&w| self.npc_may_open(w, obj.ref_id))
                {
                    opening.push(obj.ref_id);
                } else {
                    log::trace!("locked door {} stays shut for {at:?}", obj.ref_id);
                }
            }
        }
        if log::log_enabled!(log::Level::Trace) {
            for rt in self.cells.values() {
                for obj in rt.animated.iter().filter(|o| o.door && !o.open) {
                    let d = walkers
                        .iter()
                        .map(|w| w.1.distance(obj.position))
                        .fold(f32::MAX, f32::min);
                    if d < 400.0 {
                        log::trace!("closed door {} nearest walker {d:.0}", obj.ref_id);
                    }
                }
            }
        }
        let mut toggle = Vec::new();
        let mut raised: Vec<(FormId, Vec<String>)> = Vec::new();
        let mut sounds: Vec<(String, Vec3)> = Vec::new();
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
                    // Behaviour graphs pose the parts and carry their collision.
                    let Some(g) = obj.graph.as_mut() else {
                        continue;
                    };
                    let frame = g.anim.update(dt, &self.vfs, &mut self.anims, &g.skeleton);
                    if !frame.raised.is_empty() {
                        let mut events = Vec::new();
                        for r in frame.raised {
                            log::trace!("{} raised {}", obj.ref_id, r.event);
                            // `SoundPlay` with the sound as payload, or in the
                            // event's name (`SoundPlay.TRPBladeSwingSwing`).
                            let sound = match r.event.split_once('.') {
                                Some((k, s)) if k.eq_ignore_ascii_case("SoundPlay") => {
                                    Some(s.to_owned())
                                }
                                _ if r.event.eq_ignore_ascii_case("SoundPlay") => r.payload,
                                _ => None,
                            };
                            if let Some(s) = sound {
                                sounds.push((s, obj.position));
                            }
                            events.push(r.event);
                        }
                        raised.push((obj.ref_id, events));
                    }
                    for (&ii, b) in obj.instances.iter().zip(&g.bones) {
                        if let (Some(inst), Some(m)) =
                            (rc.instances.get_mut(ii), b.and_then(|b| frame.pose.get(b)))
                        {
                            set_transform(inst, obj.transform * *m);
                        }
                    }
                    for &(h, b, local) in &g.colliders {
                        let (Some(m), Some(c)) =
                            (frame.pose.get(b), self.physics.world.colliders.get_mut(h))
                        else {
                            continue;
                        };
                        let (pose, _) =
                            crate::physics::shapes::decompose(obj.transform * *m * local);
                        c.set_position(pose);
                    }
                }
            }
        }
        for (door, auto) in toggle {
            self.toggle_door(door, auto);
        }
        for (sound, at) in sounds {
            self.play_sound_at(&sound, at);
        }
        for (r, events) in raised {
            self.raise_anim_events(r, &events);
        }
    }
}
