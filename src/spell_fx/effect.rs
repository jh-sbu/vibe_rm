//! Shared independently clocked NIF effect models, including impact models.
use crate::render::{GpuModel, Instance, Renderer};
use crate::world::loader::ModelAnim;
use glam::{Mat4, Vec3};
use std::sync::Arc;

/// An impact's effect running: its model's meshes and particle systems,
/// the nodes with controllers of their own (parts) moved, scaled and shown
/// by them, and its materials' controllers, all from when it started.
pub struct EffectInstance {
    /// Where it was placed.
    pub transform: Mat4,
    /// The model's still meshes and particle systems, if any.
    pub root: Option<Instance>,
    pub anim: Option<Arc<ModelAnim>>,
    /// Each part (in `anim.parts` order) and whether it shows.
    pub parts: Vec<(Instance, bool)>,
    pub age: f32,
    pub duration: f32,
    playing: Option<(usize, f32)>,
    pub graph: Option<havok::behavior::runtime::Instance>,
    samples: Vec<(usize, f32, f32)>,
    pub stopped: bool,
    pub finished: bool,
    pub tags: nif::AttachTags,
    sequence_nodes: std::collections::HashMap<String, Vec<crate::render::model::NodeChannel>>,
    text_keys: std::collections::HashMap<String, Vec<(f32, String)>>,
    /// World frames replacing the parent transform of attached subtrees.
    pub anchors: std::collections::HashMap<i32, Mat4>,
}

impl EffectInstance {
    /// Pose the parts at the effect's age (each after the part it lies in);
    /// world-space particles stay where they were emitted.
    pub fn pose(&mut self) {
        if let Some(root) = &mut self.root {
            let delta = self.transform.inverse() * root.transform;
            for (state, sys) in root.particles.iter_mut().zip(&root.model.particles) {
                if sys.desc.world_space {
                    state.carry(delta);
                }
            }
            root.transform = self.transform;
            root.world_center = self.transform.transform_point3(root.model.bound_center);
            if let Some(anim) = &self.anim {
                if let Some(&(i, t, _)) = self.samples.iter().max_by(|a, b| a.2.total_cmp(&b.2)) {
                    root.model.set_material_sequence(&anim.sequences[i].name, t);
                }
            }
        }
        let Some(anim) = &self.anim else { return };
        let t = anim.start + self.age;
        let mut poses: Vec<Mat4> = Vec::with_capacity(anim.parts.len());
        for (i, part) in anim.parts.iter().enumerate() {
            let own = anim.controllers.iter().filter(|c| c.block == part.block);
            let mut local = part.rest;
            let mut shown = true;
            for c in own {
                if let Some(m) = c.transform(t, part.rest) {
                    local = m;
                }
                if let Some(v) = c.visible(t) {
                    shown &= v;
                }
            }
            let mut total = 0.0f32;
            let mut blended: Option<(Vec3, glam::Quat, Vec3)> = None;
            for &(si, time, weight) in &self.samples {
                let seq = &anim.sequences[si];
                if let Some(channel) = seq.channels.iter().find(|c| c.node == part.node) {
                    let (scale, rot, pos) = channel
                        .sample(time, part.rest)
                        .to_scale_rotation_translation();
                    let f = weight / (total + weight).max(1e-6);
                    blended = Some(match blended {
                        Some((s, r, p)) => (s.lerp(scale, f), r.slerp(rot, f), p.lerp(pos, f)),
                        None => (scale, rot, pos),
                    });
                    total += weight;
                }
                if let Some(channels) = self.sequence_nodes.get(&seq.name.to_ascii_lowercase()) {
                    for c in channels.iter().filter(|c| c.block == part.block) {
                        if let Some(v) = c.visible(time) {
                            shown &= v;
                        }
                    }
                }
            }
            if let Some((s, r, p)) = blended {
                local = Mat4::from_scale_rotation_translation(s, r, p);
            }
            let parent = match part.within {
                Some((o, rel)) if o < i => {
                    shown &= self.parts[o].1;
                    poses[o] * rel
                }
                _ => part.parent,
            };
            let pose = if let Some(anchor) = self.anchors.get(&part.block) {
                self.transform.inverse() * *anchor * local
            } else {
                parent * local
            };
            poses.push(pose);
            let (inst, visible) = &mut self.parts[i];
            *visible = shown;
            if let Some(&(index, time, _)) = self.samples.iter().max_by(|a, b| a.2.total_cmp(&b.2))
            {
                inst.model
                    .set_material_sequence(&anim.sequences[index].name, time);
            }
            let to = self.transform * pose;
            let delta = to.inverse() * inst.transform;
            for (state, sys) in inst.particles.iter_mut().zip(&inst.model.particles) {
                if sys.desc.world_space {
                    state.carry(delta);
                }
            }
            inst.transform = to;
            inst.world_center = to.transform_point3(inst.model.bound_center);
        }
    }

    /// Its instances and whether each shows.
    pub fn instances_mut(&mut self) -> impl Iterator<Item = (&mut Instance, bool)> {
        self.root
            .iter_mut()
            .map(|i| (i, true))
            .chain(self.parts.iter_mut().map(|(i, v)| (i, *v)))
    }
}

impl EffectInstance {
    pub fn from_parts(
        transform: Mat4,
        root: Option<Instance>,
        anim: Option<Arc<ModelAnim>>,
        parts: Vec<(Instance, bool)>,
        duration: f32,
    ) -> Self {
        Self {
            transform,
            root,
            anim,
            parts,
            age: 0.0,
            duration,
            playing: None,
            graph: None,
            samples: Vec::new(),
            stopped: false,
            finished: false,
            tags: Default::default(),
            sequence_nodes: Default::default(),
            text_keys: Default::default(),
            anchors: Default::default(),
        }
    }

    pub fn new(
        renderer: &Renderer,
        root: Option<Arc<GpuModel>>,
        anim: Option<Arc<ModelAnim>>,
        transform: Mat4,
        seed: u32,
    ) -> Self {
        let start = anim.as_ref().map_or(0.0, |a| a.start);
        let mut systems = 0u32;
        let mut instance = |model: &GpuModel, at| {
            let mut inst = Instance::new(Arc::new(renderer.started_model(model, start)), at);
            inst.particles = (0..inst.model.particles.len())
                .map(|_| {
                    systems += 1;
                    let mut state = crate::render::particles::ParticleState::new(
                        seed ^ systems.wrapping_mul(0x9E3779B9),
                    );
                    state.time = start;
                    state
                })
                .collect();
            inst
        };
        let root = root.map(|m| instance(&m, transform));
        let parts = anim
            .iter()
            .flat_map(|a| &a.parts)
            .map(|p| (instance(&p.model, transform * p.parent * p.rest), true))
            .collect();
        Self::from_parts(transform, root, anim, parts, f32::INFINITY)
    }

    pub fn play(&mut self, name: &str) -> bool {
        let Some((index, seq)) = self.anim.as_ref().and_then(|a| {
            a.sequences
                .iter()
                .enumerate()
                .find(|(_, s)| s.name.eq_ignore_ascii_case(name))
        }) else {
            return false;
        };
        self.playing = Some((index, seq.start));
        self.samples = vec![(index, seq.start, 1.0)];
        self.finished = false;
        true
    }
    pub fn send_event(&mut self, event: &str) {
        if let Some(graph) = &mut self.graph {
            graph.send_event(event);
        }
    }
    pub fn set_variable(&mut self, name: &str, value: f32) {
        if let Some(graph) = &mut self.graph {
            graph.set_variable(name, value);
        }
    }
    pub fn stop(&mut self) {
        if !self.stopped {
            self.send_event("StopEffect");
            self.stopped = true;
            self.duration = self.age;
        }
    }
    pub fn configure(&mut self, nif: &nif::Nif) {
        self.tags = nif::attach_tags(nif);
        for block in &nif.blocks {
            let nif::Block::ControllerSequence(seq) = block else {
                continue;
            };
            if let Some(nif::Block::TextKeys(keys)) = nif.get(seq.text_keys) {
                self.text_keys
                    .insert(seq.name.to_ascii_lowercase(), keys.clone());
            }
            let mut channels = Vec::new();
            for cb in &seq.blocks {
                if !cb.controller_type.contains("VisController") {
                    continue;
                }
                let Some(nif::Block::BoolInterpolator(interp)) = nif.get(cb.interpolator) else {
                    continue;
                };
                for (index, block) in nif.blocks.iter().enumerate() {
                    let Some(av) = block.av().filter(|av| av.net.name == cb.node) else {
                        continue;
                    };
                    if !cb.controller.is_none() && av.net.controller != cb.controller {
                        continue;
                    }
                    channels.push(crate::render::model::NodeChannel {
                        node: cb.node.clone(),
                        block: index as i32,
                        timing: nif::anim::Timing {
                            flags: 8 | 4,
                            frequency: 1.0,
                            phase: 0.0,
                            start: seq.start,
                            stop: seq.stop,
                        },
                        drive: crate::render::model::NodeDrive::Visibility {
                            value: interp.value,
                            keys: match nif.get(interp.data) {
                                Some(nif::Block::ValueKeys(k)) => Some(k.clone()),
                                _ => None,
                            },
                        },
                    });
                }
            }
            self.sequence_nodes
                .insert(seq.name.to_ascii_lowercase(), channels);
        }
    }

    pub fn advance(&mut self, dt: f32) {
        self.age += dt;
        if let Some(anim) = &self.anim {
            if let Some(graph) = &mut self.graph {
                struct Source<'a>(&'a ModelAnim);
                impl havok::behavior::runtime::ClipSource for Source<'_> {
                    fn duration(&mut self, name: &str) -> Option<f32> {
                        self.0
                            .sequences
                            .iter()
                            .find(|s| s.name.eq_ignore_ascii_case(name))
                            .map(|s| (s.stop - s.start) / s.frequency.abs().max(1e-6))
                    }
                    fn sequence_looping(&mut self, name: &str) -> bool {
                        self.0
                            .sequences
                            .iter()
                            .find(|s| s.name.eq_ignore_ascii_case(name))
                            .is_some_and(|s| s.cycle < 2)
                    }
                }
                graph.update(dt, &mut Source(anim));
                let leaves = graph.sequence_samples();
                self.samples.clear();
                let mut events = Vec::new();
                self.finished = !leaves.is_empty();
                for leaf in leaves {
                    let Some((index, seq)) = anim
                        .sequences
                        .iter()
                        .enumerate()
                        .find(|(_, s)| s.name.eq_ignore_ascii_case(&leaf.animation))
                    else {
                        continue;
                    };
                    let t = seq.start + leaf.time * seq.frequency;
                    let prev = seq.start + leaf.prev_time * seq.frequency;
                    self.samples.push((index, t, leaf.weight));
                    self.finished &= seq.cycle >= 2 && t >= seq.stop;
                    if let Some(keys) = self.text_keys.get(&seq.name.to_ascii_lowercase()) {
                        for (time, event) in keys {
                            if (*time > prev && *time <= t)
                                || (leaf.wrapped && (*time > prev || *time <= t))
                            {
                                events.extend(
                                    event
                                        .split('\n')
                                        .map(str::trim)
                                        .filter(|e| !e.is_empty())
                                        .map(str::to_owned),
                                );
                            }
                        }
                    }
                }
                for event in events {
                    graph.send_event(&event);
                }
                graph.take_raised();
            } else if let Some((index, phase)) = &mut self.playing {
                let (t, done) = anim.sequences[*index].advance(phase, dt);
                self.samples = vec![(*index, t, 1.0)];
                self.finished = done;
            }
        }
        self.pose();
    }

    /// Simulate and collect quads. Ended effects keep their existing particles.
    pub fn step(
        &mut self,
        dt: f32,
        right: Vec3,
        up: Vec3,
        batches: &mut Vec<crate::render::particles::ParticleBatch>,
    ) -> bool {
        let selected = self
            .anim
            .as_ref()
            .map(|a| {
                self.samples
                    .iter()
                    .map(|&(i, t, w)| (a.sequences[i].name.to_ascii_lowercase(), t, w))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let stopped = self.stopped || self.age > self.duration;
        let mut alive = false;
        for (inst, shown) in self.instances_mut() {
            for (state, sys) in inst.particles.iter_mut().zip(&inst.model.particles) {
                state.stopped = stopped || !shown;
                let variant =
                    selected
                        .iter()
                        .max_by(|a, b| a.2.total_cmp(&b.2))
                        .and_then(|(name, t, _)| {
                            sys.desc.sequences.get(name).map(|d| (d.as_ref(), *t))
                        });
                let desc = match variant {
                    Some((d, t)) => {
                        state.time = t;
                        d
                    }
                    None => sys.desc.as_ref(),
                };
                state.step(desc, dt, glam::Mat3::from_mat4(inst.transform).inverse());
                alive |= state.count() > 0;
                let mut vertices = Vec::new();
                state.quads(desc, inst.transform, right, up, &mut vertices);
                if !vertices.is_empty() {
                    batches.push(crate::render::particles::ParticleBatch {
                        material: sys.material.clone(),
                        vertices,
                        center: inst.world_center,
                    });
                }
            }
        }
        alive
    }
    pub fn draw(&mut self, scene: &mut crate::render::Scene) {
        for (inst, shown) in self.instances_mut() {
            if !shown || inst.model.parts.is_empty() {
                continue;
            }
            let mut drawn = Instance::new(inst.model.clone(), inst.transform);
            drawn.lights =
                crate::render::pick_lights(&scene.lights, drawn.world_center, drawn.world_radius);
            scene.dynamic.push(drawn);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::model::{NodeChannel, NodeDrive, Sequence};
    fn fixture() -> EffectInstance {
        let model = Arc::new(GpuModel {
            path: "synthetic".into(),
            parts: Vec::new(),
            skinned: Vec::new(),
            particles: Vec::new(),
            bound_center: Vec3::ZERO,
            bound_radius: 1.0,
        });
        let anim = Arc::new(ModelAnim {
            parts: vec![crate::world::loader::AnimPart {
                node: "part".into(),
                block: 1,
                parent: Mat4::IDENTITY,
                rest: Mat4::IDENTITY,
                model: model.clone(),
                within: None,
            }],
            sequences: ["On", "Off"]
                .map(|name| Sequence {
                    name: name.into(),
                    cycle: 2,
                    start: 0.0,
                    stop: 2.0,
                    frequency: 1.0,
                    channels: Vec::new(),
                })
                .into(),
            controllers: Vec::new(),
            start: 0.0,
            graph: None,
        });
        let mut fx = EffectInstance::from_parts(
            Mat4::IDENTITY,
            None,
            Some(anim),
            vec![(Instance::new(model, Mat4::IDENTITY), true)],
            10.0,
        );
        for (name, value) in [("on", true), ("off", false)] {
            fx.sequence_nodes.insert(
                name.into(),
                vec![NodeChannel {
                    node: "part".into(),
                    block: 1,
                    timing: nif::anim::Timing {
                        flags: 8 | 4,
                        frequency: 1.0,
                        phase: 0.0,
                        start: 0.0,
                        stop: 2.0,
                    },
                    drive: NodeDrive::Visibility { value, keys: None },
                }],
            );
        }
        fx
    }
    #[test]
    fn selection_restarts_visibility_and_attachment_follows_bone() {
        let mut fx = fixture();
        assert!(fx.play("On"));
        fx.advance(1.0);
        assert!(fx.parts[0].1);
        assert!(fx.play("Off"));
        fx.advance(0.1);
        assert!(!fx.parts[0].1);
        assert!(fx.play("On"));
        fx.advance(0.0);
        assert!(fx.parts[0].1);
        assert_eq!(fx.samples[0].1, 0.0);
        fx.anchors
            .insert(1, Mat4::from_translation(Vec3::new(4.0, 5.0, 6.0)));
        fx.pose();
        assert_eq!(
            fx.parts[0].0.transform.w_axis.truncate(),
            Vec3::new(4.0, 5.0, 6.0)
        );
        fx.anchors.insert(1, Mat4::from_translation(Vec3::X));
        fx.pose();
        assert_eq!(fx.parts[0].0.transform.w_axis.truncate(), Vec3::X);
    }
    #[test]
    fn standalone_impact_clock_still_starts_at_authored_start() {
        let mut fx = fixture();
        let a = Arc::get_mut(fx.anim.as_mut().unwrap()).unwrap();
        a.start = 3.0;
        a.controllers.push(NodeChannel {
            node: "part".into(),
            block: 1,
            timing: nif::anim::Timing {
                flags: 8 | 4,
                frequency: 1.0,
                phase: 0.0,
                start: 3.0,
                stop: 5.0,
            },
            drive: NodeDrive::Visibility {
                value: true,
                keys: Some(nif::anim::ValueKeys {
                    interpolation: 1,
                    tangents: Vec::new(),
                    keys: vec![(3.0, Vec3::ONE), (4.0, Vec3::ZERO)],
                }),
            },
        });
        fx.age = 0.5;
        fx.pose();
        assert!(fx.parts[0].1);
        fx.age = 1.0;
        fx.pose();
        assert!(!fx.parts[0].1);
    }
}
