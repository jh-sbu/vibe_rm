//! Animated objects: doors that open and close, statics that loop an "Idle"
//! keyframe sequence (water wheels, flags, machinery), objects with
//! standalone NIF transform / visibility controllers, and objects run by a
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

/// Papyrus `GetOpenState` / condition `GetOpenState` values (CommonLibSSE's
/// `BGSOpenCloseForm::OPEN_STATE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenState {
    None = 0,
    Open = 1,
    Opening = 2,
    Closed = 3,
    Closing = 4,
}

impl OpenState {
    /// The state an object's graph is in after it takes (`sent`) or raises
    /// event `ev`, if the event is one of opening or closing: `Open` / `Close`
    /// start the movement, the graph raises `Opening` / `Closing` once it is
    /// done (a portcullis's), `Opened` / `Closed` snap to the end.
    fn after_event(ev: &str, sent: bool) -> Option<Self> {
        let is = |s: &str| ev.eq_ignore_ascii_case(s);
        Some(match sent {
            true if is("Open") => Self::Opening,
            true if is("Close") => Self::Closing,
            true if is("Opened") => Self::Open,
            true if is("Closed") => Self::Closed,
            false if is("Opening") => Self::Open,
            false if is("Closing") => Self::Closed,
            _ => return None,
        })
    }
}

/// The `Vm::signal_with` key a `GetOpenState` on `r` waits for.
pub(crate) fn open_state_key(r: FormId) -> u64 {
    0x4f50_454e_0000_0000 | r.0 as u64
}

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
    /// Sequence playing and its phase (reverse cycles include the return leg).
    pub playing: Option<(usize, f32)>,
    /// Time since this loaded object started its APP_INIT controllers.
    pub age: f32,
    /// Last local pose of every part, retaining a completed sequence's pose.
    locals: Vec<Mat4>,
    /// The behaviour graph running it, if its model has one.
    pub graph: Option<ObjectGraph>,
}

#[cfg(test)]
pub(crate) fn test_graph_object(
    ref_id: FormId,
    graph: crate::world::behavior::GraphAnim,
) -> AnimatedObject {
    AnimatedObject {
        ref_id,
        transform: Mat4::IDENTITY,
        position: Vec3::ZERO,
        anim: Arc::new(ModelAnim {
            parts: Vec::new(),
            sequences: Vec::new(),
            controllers: Vec::new(),
            start: 0.0,
            graph: None,
        }),
        locals: Vec::new(),
        instances: Vec::new(),
        colliders: Vec::new(),
        door: false,
        open: false,
        auto: false,
        clear_for: 0.0,
        playing: None,
        age: 0.0,
        graph: Some(ObjectGraph {
            anim: graph,
            skeleton: Arc::new(crate::world::skeleton::Skeleton::default()),
            bones: Vec::new(),
            colliders: Vec::new(),
            moved: Vec::new(),
            dt: 0.0,
        }),
    }
}

/// An object's running behaviour graph.
pub struct ObjectGraph {
    anim: crate::world::behavior::GraphAnim,
    skeleton: Arc<crate::world::skeleton::Skeleton>,
    /// The skeleton bone of each part (in `anim.parts` order).
    bones: Vec<Option<usize>>,
    /// Colliders on the moving nodes: their bone and pose relative to it.
    colliders: Vec<(ColliderHandle, usize, Mat4)>,
    /// Each collider's world pose before and after the last update, and that
    /// update's time step (for their velocity: trap hits).
    moved: Vec<(Mat4, Mat4)>,
    dt: f32,
}

/// A collider moved by an object's behaviour graph in the last update.
pub(crate) struct GraphPart {
    pub ref_id: FormId,
    pub collider: ColliderHandle,
    /// World poses before and after, and the time between.
    pub from: Mat4,
    pub to: Mat4,
    pub dt: f32,
}

impl GraphPart {
    /// The part's velocity at world point `p` (game units / s).
    pub fn velocity_at(&self, p: Vec3) -> Vec3 {
        let before = self
            .from
            .transform_point3(self.to.inverse().transform_point3(p));
        (p - before) / self.dt.max(1e-4)
    }
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

    /// Advance a sequence and the object's own controllers. A graph, if any,
    /// overwrites these transforms afterward (visibility still applies).
    fn update(&mut self, dt: f32, clock: f32, instances: &mut [Instance]) {
        self.age += dt;
        let mut completed = false;
        let sequence_sample = self.playing.as_mut().map(|(si, phase)| {
            let seq = &self.anim.sequences[*si];
            let (time, done) = seq.advance(phase, dt);
            completed = done;
            (*si, time)
        });
        let mut poses = Vec::with_capacity(self.anim.parts.len());
        for (i, (part, &ii)) in self.anim.parts.iter().zip(&self.instances).enumerate() {
            // Keep completed sequences' terminal pose until another is played.
            let mut local = self.locals[i];
            let mut shown = true;
            for c in self
                .anim
                .controllers
                .iter()
                .filter(|c| c.block == part.block)
            {
                let t = controller_time(c.timing, clock, self.age);
                if let Some(m) = c.transform(t, part.rest) {
                    local = m;
                }
                if let Some(v) = c.visible(t) {
                    shown &= v;
                }
            }
            // A sequence owns its keyed transforms while playing, without
            // stopping unrelated controllers or visibility channels.
            if let Some((si, t)) = sequence_sample {
                if let Some(ch) = self.anim.sequences[si]
                    .channels
                    .iter()
                    .find(|c| c.node.eq_ignore_ascii_case(&part.node))
                {
                    local = ch.sample(t, part.rest);
                }
            }
            let (parent, parent_shown) = part_parent(part.parent, part.within, &poses);
            shown &= parent_shown;
            self.locals[i] = local;
            let pose = parent * local;
            poses.push((pose, shown));
            if let Some(inst) = instances.get_mut(ii) {
                if let Some((si, t)) = sequence_sample {
                    inst.model
                        .set_material_sequence(&self.anim.sequences[si].name, t);
                }
                set_transform(inst, self.transform * pose);
                // Preserve the reference's hidden flag (Disable, moved away,
                // taken items). Visibility animation only controls its opacity.
                inst.tint.w = if shown { 1.0 } else { 0.0 };
            }
        }
        if completed {
            self.playing = None;
        }
    }
}

fn set_transform(inst: &mut Instance, m: Mat4) {
    inst.transform = m;
    inst.world_center = m.transform_point3(inst.model.bound_center);
    inst.world_radius = inst.model.bound_radius
        * m.x_axis
            .truncate()
            .length()
            .max(m.y_axis.truncate().length())
            .max(m.z_axis.truncate().length());
}

fn controller_time(timing: nif::anim::Timing, clock: f32, age: f32) -> f32 {
    if timing.app_init() { age } else { clock }
}

/// A flattened part follows its earlier parent, including that parent's
/// visibility, instead of the parent's model-space rest transform.
fn part_parent(rest: Mat4, within: Option<(usize, Mat4)>, poses: &[(Mat4, bool)]) -> (Mat4, bool) {
    within
        .and_then(|(i, relative)| poses.get(i).map(|&(pose, shown)| (pose * relative, shown)))
        .unwrap_or((rest, true))
}

#[cfg(test)]
mod node_controller_tests {
    use super::*;
    use crate::render::{GpuModel, model};
    use crate::world::loader::AnimPart;
    use nif::anim::{Timing, TransformData, TransformInterpolator, ValueKeys};

    fn timing(flags: u16) -> Timing {
        Timing {
            flags,
            frequency: 1.0,
            phase: 0.0,
            start: 0.0,
            stop: 2.0,
        }
    }

    fn channel(node: &str, end: Vec3) -> model::Channel {
        model::Channel {
            node: node.into(),
            interpolator: TransformInterpolator {
                translation: Vec3::NAN,
                rotation: glam::Quat::NAN,
                scale: f32::NAN,
                data: nif::Ref(-1),
            },
            data: Some(Arc::new(TransformData {
                translations: vec![(0.0, Vec3::ZERO), (2.0, end)],
                scales: vec![(0.0, 1.0), (2.0, 3.0)],
                ..Default::default()
            })),
        }
    }

    fn fixture() -> (AnimatedObject, Vec<Instance>) {
        // Empty GPU models need no device; exercising object posing, not rendering.
        let model = Arc::new(GpuModel {
            path: String::new(),
            parts: Vec::new(),
            skinned: Vec::new(),
            particles: Vec::new(),
            bound_center: Vec3::X,
            bound_radius: 2.0,
        });
        let anim = Arc::new(ModelAnim {
            parts: vec![
                AnimPart {
                    node: "Parent".into(),
                    block: 1,
                    parent: Mat4::IDENTITY,
                    rest: Mat4::IDENTITY,
                    model: model.clone(),
                    within: None,
                },
                AnimPart {
                    node: "Child".into(),
                    block: 2,
                    parent: Mat4::IDENTITY,
                    rest: Mat4::from_translation(Vec3::Y),
                    model: model.clone(),
                    within: Some((0, Mat4::from_translation(Vec3::X))),
                },
            ],
            sequences: Vec::new(),
            controllers: vec![
                model::NodeChannel {
                    node: "Parent".into(),
                    block: 1,
                    timing: timing(8),
                    drive: model::NodeDrive::Transform(channel("Parent", Vec3::X * 10.0)),
                },
                model::NodeChannel {
                    node: "Parent".into(),
                    block: 1,
                    timing: timing(8),
                    drive: model::NodeDrive::Visibility {
                        value: true,
                        keys: Some(ValueKeys {
                            interpolation: 1,
                            tangents: Vec::new(),
                            keys: vec![(0.0, Vec3::ZERO), (1.0, Vec3::ONE)],
                        }),
                    },
                },
            ],
            start: 0.0,
            graph: None,
        });
        let obj = AnimatedObject {
            ref_id: FormId(1),
            transform: Mat4::IDENTITY,
            position: Vec3::ZERO,
            instances: vec![0, 1],
            colliders: Vec::new(),
            door: false,
            open: false,
            auto: false,
            clear_for: 0.0,
            playing: None,
            age: 0.0,
            locals: anim.parts.iter().map(|p| p.rest).collect(),
            anim,
            graph: None,
        };
        (
            obj,
            vec![
                Instance::new(model.clone(), Mat4::IDENTITY),
                Instance::new(model, Mat4::IDENTITY),
            ],
        )
    }

    #[test]
    fn standalone_controllers_propagate_pose_visibility_and_scaled_bounds() {
        let (mut obj, mut instances) = fixture();
        instances[1].hidden = true; // A scripted Disable remains authoritative.
        obj.update(0.5, 0.5, &mut instances);
        assert_eq!(instances[0].tint.w, 0.0);
        assert_eq!(instances[1].tint.w, 0.0);
        assert!(instances[1].hidden);
        assert!(
            (instances[1].transform.w_axis.truncate() - Vec3::new(4.0, 1.5, 0.0)).length() < 1e-5
        );
        assert!((instances[1].world_radius - 3.0).abs() < 1e-5);
        obj.update(0.5, 1.0, &mut instances);
        assert_eq!(instances[1].tint.w, 1.0);
        assert!(instances[1].hidden);
        assert!(
            (instances[1].transform.w_axis.truncate() - Vec3::new(7.0, 2.0, 0.0)).length() < 1e-5
        );
        assert!((instances[1].world_radius - 4.0).abs() < 1e-5);
        assert!((instances[1].world_center - Vec3::new(9.0, 2.0, 0.0)).length() < 1e-5);
    }

    #[test]
    fn app_init_uses_object_age_and_sequence_only_owns_its_keyed_transform() {
        let (mut obj, mut instances) = fixture();
        let anim = Arc::get_mut(&mut obj.anim).unwrap();
        anim.controllers[0].timing.flags |= 1;
        anim.sequences.push(model::Sequence {
            name: "Open".into(),
            cycle: 2,
            start: 0.0,
            stop: 2.0,
            frequency: 1.0,
            channels: vec![channel("Child", Vec3::Z * 8.0)],
        });
        assert!(obj.play("Open"));
        obj.update(0.5, 20.0, &mut instances);
        // Parent uses age 0.5 (x=2.5, scale=1.5), child sequence z=2.
        assert!(
            (instances[1].transform.w_axis.truncate() - Vec3::new(4.0, 0.0, 3.0)).length() < 1e-5
        );
        obj.update(1.5, 21.5, &mut instances);
        assert!(obj.playing.is_none());
        let terminal = instances[1].transform;
        obj.update(0.0, 21.5, &mut instances);
        assert_eq!(instances[1].transform, terminal);
    }

    #[test]
    fn standalone_reverse_pose_and_visibility_use_nonzero_start() {
        for app_init in [false, true] {
            let (mut obj, mut instances) = fixture();
            let anim = Arc::get_mut(&mut obj.anim).unwrap();
            for c in &mut anim.controllers {
                c.timing = Timing {
                    flags: 8 | 2 | u16::from(app_init),
                    frequency: 2.0,
                    phase: 1.0,
                    start: 3.0,
                    stop: 5.0,
                };
            }
            let model::NodeDrive::Transform(c) = &mut anim.controllers[0].drive else {
                panic!("fixture transform missing");
            };
            c.data = Some(Arc::new(TransformData {
                translations: vec![(3.0, Vec3::ZERO), (5.0, Vec3::X * 8.0)],
                ..Default::default()
            }));
            anim.controllers[1].drive = model::NodeDrive::Visibility {
                value: true,
                keys: Some(ValueKeys {
                    interpolation: 1,
                    tangents: Vec::new(),
                    keys: vec![(3.0, Vec3::ZERO), (4.0, Vec3::ONE)],
                }),
            };
            let mut previous = 0.0;
            for (time, x, shown) in [
                (1.0, 0.0, 0.0),
                (1.5, 4.0, 1.0),
                (2.0, 8.0, 1.0),
                (2.5, 4.0, 1.0),
                (3.0, 0.0, 0.0),
                (3.5, 4.0, 1.0),
                (9.5, 4.0, 1.0),
            ] {
                // APP_INIT follows object age despite an unrelated global clock.
                let clock = if app_init { 100.0 } else { time };
                obj.update(time - previous, clock, &mut instances);
                previous = time;
                assert_eq!(instances[0].transform.w_axis.x, x, "time={time}");
                assert_eq!(instances[1].transform.w_axis.x, x + 1.0);
                assert_eq!(instances[0].tint.w, shown);
                assert_eq!(instances[1].tint.w, shown);
            }
        }
    }

    #[test]
    fn material_only_sequence_uses_its_frequency_and_restarts_on_selection() {
        let (mut obj, mut instances) = fixture();
        Arc::get_mut(&mut obj.anim)
            .unwrap()
            .sequences
            .push(model::Sequence {
                name: "Fade".into(),
                cycle: 2,
                start: 0.0,
                stop: 2.0,
                frequency: 2.0,
                channels: Vec::new(),
            });
        assert!(obj.play("fade"));
        obj.update(0.5, 100.0, &mut instances);
        assert_eq!(obj.playing, Some((0, 1.0)));
        obj.update(0.75, 100.75, &mut instances);
        assert_eq!(obj.playing, None);
        assert!(obj.play("Fade"));
        assert_eq!(obj.playing, Some((0, 0.0)));
    }

    #[test]
    fn reverse_sequence_turns_at_both_endpoints_and_reselection_restarts() {
        let (mut obj, mut instances) = fixture();
        let anim = Arc::get_mut(&mut obj.anim).unwrap();
        anim.controllers.clear();
        let mut transform = channel("Parent", Vec3::X * 8.0);
        transform.data = Some(Arc::new(TransformData {
            translations: vec![(3.0, Vec3::ZERO), (5.0, Vec3::X * 8.0)],
            ..Default::default()
        }));
        anim.sequences.push(model::Sequence {
            name: "Idle".into(),
            cycle: 1,
            start: 3.0,
            stop: 5.0,
            frequency: 2.0,
            channels: vec![transform],
        });
        assert!(obj.play("Idle"));
        for x in [4.0, 8.0, 4.0, 0.0, 4.0] {
            obj.update(0.5, 100.0, &mut instances);
            assert_eq!(instances[0].transform.w_axis.x, x);
            assert!(obj.playing.is_some());
        }
        assert!(obj.play("idle"));
        obj.update(0.0, 100.0, &mut instances);
        assert_eq!(instances[0].transform.w_axis.x, 0.0);
        // One update can skip several complete out-and-back cycles.
        obj.update(8.25, 108.25, &mut instances);
        assert_eq!(instances[0].transform.w_axis.x, 2.0);
        assert_eq!(obj.playing, Some((0, 3.5)));
    }

    #[test]
    fn sequence_cycle_endpoints_and_degenerate_ranges_stay_finite() {
        let mut seq = model::Sequence {
            name: "Idle".into(),
            cycle: 0,
            start: 3.0,
            stop: 5.0,
            frequency: 2.0,
            channels: Vec::new(),
        };
        let mut phase = seq.start;
        assert_eq!(seq.advance(&mut phase, 1.0), (5.0, false));
        assert_eq!(seq.advance(&mut phase, 0.25), (3.5, false));
        seq.cycle = 2;
        phase = seq.start;
        assert_eq!(seq.advance(&mut phase, 1.0), (5.0, true));
        assert_eq!(seq.advance(&mut phase, 10.0), (5.0, true));
        for stop in [3.0, 2.0] {
            seq.stop = stop;
            for cycle in [0, 1, 2] {
                seq.cycle = cycle;
                phase = seq.start;
                assert_eq!(seq.advance(&mut phase, 10.0), (3.0, cycle == 2));
                assert_eq!(phase, 3.0);
            }
        }
        // A short but nonzero reverse range must not use an artificial length.
        seq.cycle = 1;
        seq.start = 0.0;
        seq.stop = 0.0005;
        seq.frequency = 1.0;
        phase = 0.0;
        let (time, done) = seq.advance(&mut phase, 0.00075);
        assert!((time - 0.00025).abs() < 1e-8);
        assert!(!done);
    }

    #[test]
    fn graph_open_close_events_give_open_states() {
        use super::OpenState as S;
        // A portcullis: `open` sent, `Opening` raised once up; `close`, `Closing`.
        assert_eq!(S::after_event("open", true), Some(S::Opening));
        assert_eq!(S::after_event("Opening", false), Some(S::Open));
        assert_eq!(S::after_event("close", true), Some(S::Closing));
        assert_eq!(S::after_event("Closing", false), Some(S::Closed));
        assert_eq!(S::after_event("opened", true), Some(S::Open));
        // Raised `Open` or sent `Opening` say nothing; nor do other events.
        assert_eq!(S::after_event("Open", false), None);
        assert_eq!(S::after_event("Opening", true), None);
        assert_eq!(S::after_event("Trigger01", true), None);
    }
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
            let mut inst = Instance::new(
                self.renderer.sequence_model(&part.model),
                transform * part.parent * part.rest,
            );
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
            locals: anim.parts.iter().map(|p| p.rest).collect(),
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
            age: 0.0,
            graph: None,
        };
        // Machinery and the like loop their idle animation.
        if !door {
            obj.play("Idle");
        } else if let Some(auto) = self.world_state.door_left_open(ref_id)
            && obj.play("Open")
        {
            // Left open: at the end of its opening (posed on the first update).
            if let Some((si, t)) = &mut obj.playing {
                *t = obj.anim.sequences[*si].stop;
            }
            obj.open = true;
            obj.auto = auto;
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
        // Doors the player opens stay open across loads; doors actors open stay
        // open too while they are open at unloading, and close behind them once
        // clear (`update_animated`).
        let ws = &mut self.world_state;
        if open && !auto {
            ws.open_doors.insert(door);
            ws.actor_open_doors.remove(&door);
        } else if open {
            ws.actor_open_doors.insert(door);
        } else {
            ws.open_doors.remove(&door);
            ws.actor_open_doors.remove(&door);
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
        let Some((path, skeleton, false)) = obj.anim.graph.clone() else {
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
        // Run on from where it was when its cell unloaded.
        let mut anim = match self.world_state.object_graphs.remove(&obj.ref_id) {
            Some(g) if Arc::ptr_eq(g.project(), &project) => {
                log::debug!("{}: graph resumes in {:?}", obj.ref_id, g.active_states());
                g
            }
            _ => crate::world::behavior::GraphAnim::new(
                project,
                &rig,
                false,
                &skeleton,
                self.rng ^ obj.ref_id.0 as u64,
            ),
        };
        anim.label = obj.ref_id.to_string();
        let bones: Vec<Option<usize>> = obj
            .anim
            .parts
            .iter()
            .map(|p| skeleton.find(&p.node))
            .collect();
        let rest = skeleton.model_space(&skeleton.bind_locals());
        let colliders: Vec<(ColliderHandle, usize, Mat4)> = tagged
            .iter()
            .filter_map(|(h, node)| {
                let b = skeleton.find(node.as_deref()?)?;
                let at = self.physics.world.colliders.get(*h)?.position().to_mat4();
                Some((*h, b, (obj.transform * rest[b]).inverse() * at))
            })
            .collect();
        let moved = colliders
            .iter()
            .map(|&(_, b, local)| {
                let at = obj.transform * rest[b] * local;
                (at, at)
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
            moved,
            dt: 0.0,
        });
    }

    /// Keep the behaviour graphs of `key`'s objects as they are, for when it
    /// loads again.
    pub(crate) fn park_object_graphs(&mut self, key: CellKey) {
        let Some(rt) = self.cells.get_mut(&key) else {
            return;
        };
        for obj in &mut rt.animated {
            if let Some(g) = obj.graph.take() {
                self.world_state.object_graphs.insert(obj.ref_id, g.anim);
            }
        }
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
        if !g.anim.send_event(event) {
            return false;
        }
        if let Some(s) = OpenState::after_event(event, true) {
            self.world_state.graph_open.insert(r, s);
        }
        true
    }

    /// Papyrus / condition `GetOpenState`: animated doors by their sequences
    /// (or whether they were left open), objects with a graph by the opening
    /// and closing events it took and raised; other doors, activators and
    /// containers are closed, everything else has no open state.
    pub fn open_state(&self, r: FormId) -> OpenState {
        if let Some((k, i)) = self.find_animated(r) {
            let obj = &self.cells[&k].animated[i];
            if obj.door {
                return match (obj.open, obj.playing.is_some()) {
                    (true, true) => OpenState::Opening,
                    (true, false) => OpenState::Open,
                    (false, true) => OpenState::Closing,
                    (false, false) => OpenState::Closed,
                };
            }
        }
        if let Some(&s) = self.world_state.graph_open.get(&r) {
            return s;
        }
        match self.base_of(r).and_then(|b| self.lo.tag_of(b)) {
            Some(t) if t.0 == *b"DOOR" && self.world_state.open_doors.contains(&r) => {
                OpenState::Open
            }
            Some(t) if matches!(&t.0, b"DOOR" | b"ACTI" | b"CONT") => OpenState::Closed,
            _ => OpenState::None,
        }
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

    /// The colliders objects' behaviour graphs moved in their last update.
    pub(crate) fn moving_graph_parts(&self) -> Vec<GraphPart> {
        let mut out = Vec::new();
        for obj in self.cells.values().flat_map(|rt| &rt.animated) {
            let Some(g) = &obj.graph else {
                continue;
            };
            for (&(collider, ..), &(from, to)) in g.colliders.iter().zip(&g.moved) {
                if from != to {
                    out.push(GraphPart {
                        ref_id: obj.ref_id,
                        collider,
                        from,
                        to,
                        dt: g.dt,
                    });
                }
            }
        }
        out
    }

    /// The behaviour graph object `r` runs, if any.
    pub(crate) fn object_graph_mut(
        &mut self,
        r: FormId,
    ) -> Option<&mut crate::world::behavior::GraphAnim> {
        let (key, i) = self.find_animated(r)?;
        Some(&mut self.cells.get_mut(&key)?.animated[i].graph.as_mut()?.anim)
    }

    /// The behaviour graph object `r` runs, if any (read-only).
    pub(crate) fn object_graph(&self, r: FormId) -> Option<&crate::world::behavior::GraphAnim> {
        let (key, i) = self.find_animated(r)?;
        Some(&self.cells.get(&key)?.animated[i].graph.as_ref()?.anim)
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
                    obj.update(dt, self.renderer.time, &mut rc.instances);
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
                    // The first update jumps from the bind pose to the graph's:
                    // no motion.
                    let first = g.dt == 0.0;
                    g.dt = dt;
                    for (&(h, b, local), moved) in g.colliders.iter().zip(&mut g.moved) {
                        let (Some(m), Some(c)) =
                            (frame.pose.get(b), self.physics.world.colliders.get_mut(h))
                        else {
                            continue;
                        };
                        let at = obj.transform * *m * local;
                        *moved = (if first { at } else { moved.1 }, at);
                        let (pose, _) = crate::physics::shapes::decompose(at);
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
            for ev in &events {
                if let Some(s) = OpenState::after_event(ev, false) {
                    self.world_state.graph_open.insert(r, s);
                }
            }
            self.raise_anim_events(r, &events);
        }
    }
}

#[cfg(test)]
mod door_tests {
    use super::*;
    use crate::render::model;
    use crate::world::loader::ModelAnim;

    const DOOR: FormId = FormId(0x810);
    const ACTOR_DOOR: FormId = FormId(0x811);
    const CELL: FormId = FormId(0x850);

    /// A door with its own Open and Close sequences and no parts.
    fn door(id: FormId) -> AnimatedObject {
        let sequence = |name: &str| model::Sequence {
            name: name.into(),
            cycle: 2,
            start: 0.0,
            stop: 1.0,
            frequency: 1.0,
            channels: Vec::new(),
        };
        AnimatedObject {
            ref_id: id,
            transform: Mat4::IDENTITY,
            position: Vec3::ZERO,
            anim: Arc::new(ModelAnim {
                parts: Vec::new(),
                sequences: vec![sequence("Open"), sequence("Close")],
                controllers: Vec::new(),
                start: 0.0,
                graph: None,
            }),
            locals: Vec::new(),
            instances: Vec::new(),
            colliders: Vec::new(),
            door: true,
            open: false,
            auto: false,
            clear_for: 0.0,
            playing: None,
            age: 0.0,
            graph: None,
        }
    }

    struct Dir(std::path::PathBuf);
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    #[ignore = "needs a headless GPU adapter; synthetic records only"]
    fn doors_actors_leave_open_are_remembered_until_they_close() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            Dir(std::env::temp_dir().join(format!("vrm-doors-{}-{stamp}", std::process::id())));
        std::fs::create_dir(&dir.0).unwrap();
        let records: Vec<esp::writer::Node> = Vec::new();
        std::fs::write(
            dir.0.join("Doors.esp"),
            esp::writer::write_plugin(0, &[], "test", "synthetic doors", 0x900, &records),
        )
        .unwrap();
        let mut e = crate::stats::action_tests::headless_engine(&dir.0, "Doors.esp");
        let rt = e.cells.entry(CellKey::Interior(CELL)).or_default();
        rt.animated.push(door(DOOR));
        rt.animated.push(door(ACTOR_DOOR));

        // The player's door stays remembered as open until the player shuts it.
        assert!(e.toggle_door(DOOR, false));
        assert_eq!(e.world_state.door_left_open(DOOR), Some(false));
        assert!(e.toggle_door(DOOR, false));
        assert_eq!(e.world_state.door_left_open(DOOR), None);

        // An actor's door is remembered while open, so it reloads open and
        // closes once clear; shutting it (the auto close) forgets it.
        assert!(e.toggle_door(ACTOR_DOOR, true));
        assert_eq!(e.world_state.door_left_open(ACTOR_DOOR), Some(true));
        assert_eq!(e.open_state(ACTOR_DOOR), OpenState::Opening);
        assert!(e.toggle_door(ACTOR_DOOR, false));
        assert_eq!(e.world_state.door_left_open(ACTOR_DOOR), None);
        assert_eq!(e.open_state(ACTOR_DOOR), OpenState::Closing);
    }
}
