//! Particle systems (`NiParticleSystem`): fire, smoke, embers, dust. Each
//! system in a model becomes a [`ParticleDesc`] (its emitters, forces,
//! colour, size and atlas animation, with the controllers that drive them
//! over time); each drawn instance of the model runs a [`ParticleState`] per
//! system on the CPU, in model space, and every frame its live particles
//! become camera-facing quads ([`ParticleBatch`]) drawn with the system's
//! effect material. Where the Gamebryo semantics aren't public the choices
//! are written down in `known_ambiguities/particles.md`.

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use glam::{Mat3, Mat4, Vec3, Vec4};
use nif::psys::{ControllerKind, EmitterShape, FloatTarget, ModifierKind};
use nif::{Block, Nif, Ref};

use super::GpuMaterial;
use super::model::{MaterialDesc, Vertex};

#[derive(Debug, Clone, Copy)]
enum CurveInterpolation {
    Linear,
    /// Float CONST_KEY selects the nearest endpoint.
    Nearest,
    /// Boolean keys hold their value until the next key.
    Step,
}

/// A float driven by a controller: its timing and keys, or a constant.
#[derive(Debug, Clone)]
pub struct Curve {
    timing: Option<nif::anim::Timing>,
    keys: Vec<(f32, f32)>,
    value: f32,
    interpolation: CurveInterpolation,
}

impl Curve {
    fn constant(value: f32) -> Curve {
        Curve {
            timing: None,
            keys: Vec::new(),
            value,
            interpolation: CurveInterpolation::Linear,
        }
    }

    pub fn sample(&self, t: f32) -> f32 {
        let (Some(timing), Some(first)) = (self.timing, self.keys.first()) else {
            return self.value;
        };
        let k = timing.key_time(t);
        if k <= first.0 {
            return first.1;
        }
        let i = self.keys.partition_point(|key| key.0 <= k);
        let (t0, a) = self.keys[i - 1];
        match self.keys.get(i) {
            Some(&(t1, b)) if t1 > t0 => match self.interpolation {
                CurveInterpolation::Linear => a + (b - a) * (k - t0) / (t1 - t0),
                CurveInterpolation::Nearest if (k - t0) / (t1 - t0) >= 0.5 => b,
                _ => a,
            },
            _ => a,
        }
    }
}

/// A controller's interpolator as a curve (`fallback` when it has neither
/// keys nor a set value).
fn curve(nif: &Nif, timing: nif::anim::Timing, interpolator: Ref, fallback: f32) -> Curve {
    match nif.get(interpolator) {
        Some(Block::ValueInterpolator(i)) => {
            let (keys, interpolation) = match nif.get(i.data) {
                Some(Block::ValueKeys(k)) => (
                    k.keys.iter().map(|(t, v)| (*t, v.x)).collect(),
                    if k.interpolation == 5 {
                        CurveInterpolation::Nearest
                    } else {
                        CurveInterpolation::Linear
                    },
                ),
                _ => (Vec::new(), CurveInterpolation::Linear),
            };
            Curve {
                timing: Some(timing),
                keys,
                value: if i.value.x > -1.0e30 {
                    i.value.x
                } else {
                    fallback
                },
                interpolation,
            }
        }
        Some(Block::BoolInterpolator(i)) => {
            let keys = match nif.get(i.data) {
                Some(Block::ValueKeys(k)) => k.keys.iter().map(|(t, v)| (*t, v.x)).collect(),
                _ => Vec::new(),
            };
            Curve {
                timing: Some(timing),
                keys,
                value: i.value as u8 as f32,
                interpolation: CurveInterpolation::Step,
            }
        }
        _ => Curve::constant(fallback),
    }
}

#[derive(Debug, Clone)]
pub struct EmitterDesc {
    pub params: nif::psys::Emitter,
    /// Model-space transform of the emitter's object (its volume's centre
    /// and axes).
    pub object: Mat4,
    /// Particles a second.
    pub birth_rate: Curve,
    /// Emitting at all (0 / 1).
    pub visible: Curve,
    pub active: Curve,
    pub speed: Option<Curve>,
    pub radius: Option<Curve>,
    pub life_span: Option<Curve>,
    /// Rigid mesh samples captured at the mesh's rest transform.
    meshes: Vec<MeshEmission>,
}

#[derive(Debug, Clone)]
struct MeshPrimitive {
    positions: [Vec3; 3],
    normals: [Vec3; 3],
    /// One point, an edge, or a triangle.
    count: usize,
    cumulative_weight: f32,
}

#[derive(Debug, Clone)]
struct MeshEmission {
    object: Mat4,
    primitives: Vec<MeshPrimitive>,
}

impl MeshEmission {
    fn new(geometry: &nif::Geometry, object: Mat4, mode: u32) -> Option<Self> {
        if !object.is_finite() || !object.determinant().is_finite() || object.determinant() == 0.0 {
            return None;
        }
        let positions = &geometry.positions;
        let mut triangles = Vec::new();
        let mut edges = BTreeSet::new();
        let mut normals = vec![Vec3::ZERO; positions.len()];
        for &[a, b, c] in &geometry.triangles {
            let indices = [a as usize, b as usize, c as usize];
            if indices
                .iter()
                .any(|&i| i >= positions.len() || !positions[i].is_finite())
            {
                continue;
            }
            let [pa, pb, pc] = indices.map(|i| positions[i]);
            let normal = (pb - pa).cross(pc - pa);
            if !normal.is_finite() || normal.length_squared() <= 1e-12 {
                continue;
            }
            triangles.push(indices);
            for i in indices {
                normals[i] += normal;
            }
            for (i, j) in [(a, b), (b, c), (c, a)] {
                edges.insert((i.min(j) as usize, i.max(j) as usize));
            }
        }
        for (i, normal) in normals.iter_mut().enumerate() {
            if let Some(&authored) = geometry.normals.get(i)
                && authored.is_finite()
                && authored.length_squared() > 1e-12
            {
                *normal = authored;
            }
            *normal = normal.normalize_or_zero();
        }
        let mut primitives = Vec::new();
        let mut weight_sum = 0.0;
        let mut add = |indices: &[usize], weight: f32, centre: bool| {
            if !weight.is_finite() || weight <= 0.0 || !(weight_sum + weight).is_finite() {
                return;
            }
            let mut p = [Vec3::ZERO; 3];
            let mut n = [Vec3::ZERO; 3];
            for (j, &i) in indices.iter().enumerate() {
                p[j] = positions[i];
                n[j] = normals[i];
            }
            if centre {
                p[0] = p.iter().sum::<Vec3>() / indices.len() as f32;
                n[0] = n.iter().sum::<Vec3>().normalize_or_zero();
            }
            weight_sum += weight;
            primitives.push(MeshPrimitive {
                positions: p,
                normals: n,
                count: if centre { 1 } else { indices.len() },
                cumulative_weight: weight_sum,
            });
        };
        match mode {
            0 => {
                for (i, position) in positions.iter().enumerate() {
                    if position.is_finite() {
                        add(&[i], 1.0, false);
                    }
                }
            }
            1 | 3 => {
                for indices in triangles {
                    let [a, b, c] = indices.map(|i| positions[i]);
                    let area = (b - a).cross(c - a).length() * 0.5;
                    add(&indices, if mode == 3 { area } else { 1.0 }, mode == 1);
                }
            }
            2 | 4 => {
                for (a, b) in edges {
                    let length = positions[a].distance(positions[b]);
                    add(&[a, b], if mode == 4 { length } else { 1.0 }, mode == 2);
                }
            }
            _ => {}
        }
        (!primitives.is_empty()).then_some(Self { object, primitives })
    }
}

/// Mesh emission currently supports decoded rigid geometry only. Deformation
/// and animated mesh transforms need a live geometry source, not a rest sample.
fn mesh_geometry(nif: &Nif, mesh: Ref) -> Option<&nif::Geometry> {
    match nif.get(mesh)? {
        Block::TriShape(shape) if shape.skin.0 < 0 && !shape.dynamic => Some(&shape.geometry),
        Block::NiTriShape(shape) | Block::NiTriStrips(shape) if shape.skin.0 < 0 => {
            match nif.get(shape.data)? {
                Block::TriShapeData(data) => Some(&data.geometry),
                _ => None,
            }
        }
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct GravityDesc {
    /// Model-space axis (planar) and position (spherical).
    pub axis: Vec3,
    pub position: Vec3,
    pub spherical: bool,
    /// The axis is in world space (turned into the model's for each instance).
    pub world_aligned: bool,
    pub strength: Curve,
    pub active: Curve,
}

#[derive(Debug, Clone)]
pub struct DragDesc {
    /// Model-space direction and centre of the drag object.
    pub axis: Vec3,
    pub position: Vec3,
    pub percentage: f32,
    /// Full-strength radius, followed by a linear falloff distance.
    pub range: f32,
    pub falloff: f32,
    pub active: Curve,
}

impl DragDesc {
    fn from_object(
        object: Mat4,
        axis: Vec3,
        percentage: f32,
        range: f32,
        falloff: f32,
        active: Curve,
    ) -> Self {
        let scale = object.x_axis.truncate().length();
        Self {
            axis: object.transform_vector3(axis).normalize_or_zero(),
            position: object.w_axis.truncate(),
            percentage,
            range: range.max(0.0) * scale,
            falloff: falloff.max(0.0) * scale,
            active,
        }
    }

    fn apply(&self, position: Vec3, velocity: &mut Vec3, dt: f32) {
        if !self.percentage.is_finite() || self.percentage <= 0.0 {
            return;
        }
        let distance = position.distance(self.position);
        let influence = if distance <= self.range {
            1.0
        } else if self.falloff > 0.0 {
            (1.0 - (distance - self.range) / self.falloff).clamp(0.0, 1.0)
        } else {
            0.0
        };
        // Only the component parallel to the object's axis is damped.
        // Temporal attenuation and linear falloff are explicit runtime
        // choices; see known_ambiguities/particles.md.
        let loss = (self.percentage * influence * dt).clamp(0.0, 1.0);
        *velocity -= self.axis * velocity.dot(self.axis) * loss;
    }
}

#[derive(Debug, Clone)]
pub struct SpawnDesc {
    pub percentage: f32,
    pub count: (u16, u16),
    pub speed_variation: f32,
    pub direction_variation: f32,
    pub life_span: f32,
    pub life_span_variation: f32,
    pub generations: u16,
}

#[derive(Debug, Clone)]
pub struct ParticleDesc {
    pub material: MaterialDesc,
    pub max: usize,
    /// Atlas cells (u, width, v, height).
    pub subtextures: Vec<Vec4>,
    /// Quads' width over height.
    pub aspect: f32,
    /// Quads stand along the particle's motion (aspect flag 1, "velocity
    /// orientation") instead of turning by their rotation.
    pub velocity_oriented: bool,
    pub emitters: Vec<EmitterDesc>,
    pub gravity: Vec<GravityDesc>,
    pub drag: Vec<DragDesc>,
    /// Rotation: speed and variation, start angle and variation, random sign.
    pub rotation: Option<(f32, f32, f32, f32, bool)>,
    /// Size over the particle's life.
    pub scales: Vec<f32>,
    pub color: Option<SimpleColor>,
    /// Atlas animation: start frame and its random extra, end frame.
    pub subtex_anim: Option<(f32, f32, f32)>,
    pub spawn_on_death: Option<SpawnDesc>,
    /// The system node's scale, sizing its particles.
    pub size_scale: f32,
    /// Its particles stay where they were emitted when the system moves
    /// (else they move with it).
    pub world_space: bool,
    /// The system's bounds in model space.
    pub bound_center: Vec3,
    pub bound_radius: f32,
}

#[derive(Debug, Clone)]
pub struct SimpleColor {
    pub fade_in: f32,
    pub fade_out: f32,
    /// Keyframe times: colour 0 held until `times[0]`, blended to colour 1
    /// by `times[1]`, held until `times[2]`, blended to colour 2 by `times[3]`.
    pub times: [f32; 4],
    pub colors: [Vec4; 3],
}

impl SimpleColor {
    fn at(&self, f: f32) -> Vec4 {
        let [t0, t1, t2, t3] = self.times;
        let mix = |a: Vec4, b: Vec4, s: f32, e: f32| {
            if e > s {
                a.lerp(b, ((f - s) / (e - s)).clamp(0.0, 1.0))
            } else {
                b
            }
        };
        let mut c = if f <= t0 {
            self.colors[0]
        } else if f <= t1 {
            mix(self.colors[0], self.colors[1], t0, t1)
        } else if f <= t2 {
            self.colors[1]
        } else {
            mix(self.colors[1], self.colors[2], t2, t3)
        };
        // Both fades at 0: no fading.
        if self.fade_in > 0.0 && f < self.fade_in {
            c.w *= f / self.fade_in;
        }
        if self.fade_out > 0.0 && f > self.fade_out {
            c.w *= ((1.0 - f) / (1.0 - self.fade_out).max(1.0e-4)).clamp(0.0, 1.0);
        }
        c
    }
}

/// Model-space transforms of the NIF's objects, by block index, its root
/// placed at `root` (an addon node's model in its host's).
fn object_transforms(nif: &Nif, root: Mat4) -> HashMap<i32, Mat4> {
    fn visit(nif: &Nif, r: Ref, parent: Mat4, out: &mut HashMap<i32, Mat4>, depth: u32) {
        let Some(block) = nif.get(r) else { return };
        let Some(av) = block.av() else { return };
        let world = parent * super::model::local_transform(av, depth);
        out.entry(r.0).or_insert(world);
        if let (Block::Node(n), true) = (block, depth < 64) {
            for &c in &n.children {
                visit(nif, c, world, out, depth + 1);
            }
        }
    }
    let mut out = HashMap::new();
    for &r in &nif.roots {
        visit(nif, Ref(r as i32), root, &mut out, 0);
    }
    out
}

/// The description of the particle system at `r`, whose node sits at
/// `world` in the model (whose root is at `root`).
pub fn describe(
    nif: &Nif,
    sys: &nif::psys::ParticleSystem,
    world: Mat4,
    root: Mat4,
) -> Option<ParticleDesc> {
    if sys.strip {
        return None;
    }
    let Some(Block::ParticleData(data)) = nif.get(sys.data) else {
        return None;
    };
    let objects = object_transforms(nif, root);
    let object_at = |r: Ref| objects.get(&r.0).copied().unwrap_or(world);
    // The controllers, by the modifier they drive.
    let mut ctlrs: Vec<&nif::psys::ParticleController> = Vec::new();
    let mut c = sys.av.net.controller;
    while let Some(Block::ParticleController(pc)) = nif.get(c) {
        if ctlrs.len() > 64 {
            break;
        }
        ctlrs.push(pc);
        c = pc.next;
    }
    let float_ctlr = |name: &str, target: FloatTarget| {
        ctlrs
            .iter()
            .find(|c| {
                c.modifier == name && matches!(c.kind, ControllerKind::Float(t) if t == target)
            })
            .map(|c| curve(nif, c.timing, c.interpolator, 0.0))
    };
    let active = |m: &nif::psys::Modifier| {
        ctlrs
            .iter()
            .find(|c| c.modifier == m.name && matches!(c.kind, ControllerKind::ModifierActive))
            .map(|c| curve(nif, c.timing, c.interpolator, m.active as u8 as f32))
            .unwrap_or_else(|| Curve::constant(m.active as u8 as f32))
    };

    let mut material = super::model::material(nif, sys.shader, sys.alpha);
    // Particles carry their colour and fade in their vertices (the fade picks
    // the palette's row instead when a palette gives the alpha).
    if material.flags1 & nif::sf1::GREYSCALE_TO_PALETTE_ALPHA == 0 || material.glow.is_none() {
        material.flags1 |= nif::sf1::VERTEX_ALPHA;
    }
    material.flags2 |= nif::sf2::VERTEX_COLORS;
    material.double_sided = true;
    material.z_write = false;
    material.anim = None;
    let mut d = ParticleDesc {
        material,
        max: data.max_particles.max(1) as usize,
        subtextures: data.subtexture_offsets.clone(),
        aspect: if data.aspect_ratio > 0.0 {
            data.aspect_ratio
        } else {
            1.0
        },
        velocity_oriented: data.aspect_flags & 1 != 0,
        emitters: Vec::new(),
        gravity: Vec::new(),
        drag: Vec::new(),
        rotation: None,
        scales: Vec::new(),
        color: None,
        subtex_anim: None,
        spawn_on_death: None,
        size_scale: world.x_axis.truncate().length(),
        world_space: sys.world_space,
        bound_center: world.transform_point3(sys.bound_center),
        bound_radius: sys.bound_radius * world.x_axis.truncate().length(),
    };
    for &mr in &sys.modifiers {
        let Some(Block::ParticleModifier(m)) = nif.get(mr) else {
            continue;
        };
        match &m.kind {
            ModifierKind::Emitter(e) => {
                let obj = match &e.shape {
                    EmitterShape::Box { object, .. }
                    | EmitterShape::Cylinder { object, .. }
                    | EmitterShape::Sphere { object, .. } => object_at(*object),
                    EmitterShape::Mesh { meshes, .. } => {
                        meshes.first().map_or(world, |&r| object_at(r))
                    }
                };
                let ctl = ctlrs.iter().find(|c| {
                    c.modifier == m.name && matches!(c.kind, ControllerKind::Emitter { .. })
                });
                let (birth_rate, visible) = match ctl {
                    Some(c) => {
                        let ControllerKind::Emitter { visibility } = c.kind else {
                            unreachable!()
                        };
                        (
                            curve(nif, c.timing, c.interpolator, 0.0),
                            curve(nif, c.timing, visibility, 1.0),
                        )
                    }
                    None => (Curve::constant(0.0), Curve::constant(1.0)),
                };
                d.emitters.push(EmitterDesc {
                    params: e.clone(),
                    object: obj,
                    birth_rate,
                    visible,
                    active: active(m),
                    speed: float_ctlr(&m.name, FloatTarget::EmitterSpeed),
                    radius: float_ctlr(&m.name, FloatTarget::EmitterInitialRadius),
                    life_span: float_ctlr(&m.name, FloatTarget::EmitterLifeSpan),
                    meshes: match &e.shape {
                        EmitterShape::Mesh {
                            meshes, emit_from, ..
                        } => meshes
                            .iter()
                            .filter_map(|&r| {
                                MeshEmission::new(mesh_geometry(nif, r)?, object_at(r), *emit_from)
                            })
                            .collect(),
                        _ => Vec::new(),
                    },
                });
            }
            ModifierKind::Gravity {
                object: o,
                axis,
                strength,
                force,
                world_aligned,
                ..
            } => {
                let xf = object_at(*o);
                d.gravity.push(GravityDesc {
                    axis: if *world_aligned {
                        axis.normalize_or_zero()
                    } else {
                        xf.transform_vector3(*axis).normalize_or_zero()
                    },
                    position: xf.w_axis.truncate(),
                    spherical: *force == 1,
                    world_aligned: *world_aligned,
                    strength: float_ctlr(&m.name, FloatTarget::GravityStrength)
                        .unwrap_or_else(|| Curve::constant(*strength)),
                    active: active(m),
                });
            }
            ModifierKind::Drag {
                object,
                axis,
                percentage,
                range,
                falloff,
            } => d.drag.push(DragDesc::from_object(
                object_at(*object),
                *axis,
                *percentage,
                *range,
                *falloff,
                active(m),
            )),
            ModifierKind::Rotation {
                speed,
                speed_variation,
                angle,
                angle_variation,
                random_sign,
                ..
            } => {
                d.rotation = Some((
                    *speed,
                    *speed_variation,
                    *angle,
                    *angle_variation,
                    *random_sign,
                ))
            }
            ModifierKind::Scale(s) => d.scales = s.clone(),
            ModifierKind::SimpleColor {
                fade_in,
                fade_out,
                color1_end,
                color1_start,
                color2_end,
                color2_start,
                colors,
            } => {
                d.color = Some(SimpleColor {
                    fade_in: *fade_in,
                    fade_out: *fade_out,
                    times: [*color1_end, *color1_start, *color2_end, *color2_start],
                    colors: *colors,
                })
            }
            ModifierKind::SubTex {
                start,
                start_fudge,
                end,
                ..
            } => d.subtex_anim = Some((*start, *start_fudge, *end)),
            ModifierKind::AgeDeath {
                spawn_on_death: true,
                spawn,
            } => {
                if let Some(Block::ParticleModifier(s)) = nif.get(*spawn)
                    && let ModifierKind::Spawn {
                        generations,
                        percentage,
                        min,
                        max,
                        speed_variation,
                        direction_variation,
                        life_span,
                        life_span_variation,
                    } = s.kind
                {
                    d.spawn_on_death = Some(SpawnDesc {
                        percentage,
                        count: (min, max),
                        speed_variation,
                        direction_variation,
                        life_span,
                        life_span_variation,
                        generations,
                    });
                }
            }
            _ => {}
        }
    }
    (!d.emitters.is_empty()).then_some(d)
}

#[derive(Debug, Clone, Copy)]
struct Particle {
    position: Vec3,
    velocity: Vec3,
    age: f32,
    life: f32,
    radius: f32,
    color: Vec4,
    angle: f32,
    spin: f32,
    frame: f32,
    generation: u16,
}

/// One instance's run of one particle system.
#[derive(Debug, Clone, Default)]
pub struct ParticleState {
    particles: Vec<Particle>,
    /// Particles owed by each emitter (fractions carried over).
    owed: Vec<f32>,
    rng: u32,
    /// Seconds run.
    pub time: f32,
    /// No more particles are emitted (an impact's effect, past its duration).
    pub stopped: bool,
}

impl ParticleState {
    pub fn new(seed: u32) -> ParticleState {
        ParticleState {
            rng: seed | 1,
            ..Default::default()
        }
    }

    pub fn count(&self) -> usize {
        self.particles.len()
    }

    /// The system moved by `delta` (its old space in its new one): its
    /// particles are kept where they were.
    pub fn carry(&mut self, delta: Mat4) {
        for p in &mut self.particles {
            p.position = delta.transform_point3(p.position);
            p.velocity = delta.transform_vector3(p.velocity);
        }
    }

    fn rand(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }

    /// -1..1.
    fn rand_signed(&mut self) -> f32 {
        self.rand() * 2.0 - 1.0
    }

    /// Run `dt` seconds. `world_rot` turns world-aligned axes into the model's.
    pub fn step(&mut self, d: &ParticleDesc, dt: f32, model_from_world: Mat3) {
        let t = self.time;
        self.time += dt;
        self.owed.resize(d.emitters.len(), 0.0);
        // Ageing and dying.
        let mut i = 0;
        while i < self.particles.len() {
            let p = &mut self.particles[i];
            p.age += dt;
            if p.age >= p.life {
                let dead = self.particles.swap_remove(i);
                if let Some(s) = &d.spawn_on_death {
                    self.spawn_from(d, s, &dead);
                }
                continue;
            }
            i += 1;
        }
        // Forces.
        for g in &d.gravity {
            if g.active.sample(t) < 0.5 {
                continue;
            }
            let strength = g.strength.sample(t);
            let axis = if g.world_aligned {
                (model_from_world * g.axis).normalize_or_zero()
            } else {
                g.axis
            };
            for p in &mut self.particles {
                let dir = if g.spherical {
                    (g.position - p.position).normalize_or_zero()
                } else {
                    axis
                };
                p.velocity += dir * strength * dt;
            }
        }
        for drag in &d.drag {
            if drag.active.sample(t) < 0.5 {
                continue;
            }
            for p in &mut self.particles {
                drag.apply(p.position, &mut p.velocity, dt);
            }
        }
        for p in &mut self.particles {
            p.position += p.velocity * dt;
            p.angle += p.spin * dt;
        }
        // Emitting.
        for (ei, e) in d.emitters.iter().enumerate() {
            if self.stopped || e.active.sample(t) < 0.5 || e.visible.sample(t) < 0.5 {
                self.owed[ei] = 0.0;
                continue;
            }
            self.owed[ei] += e.birth_rate.sample(t).max(0.0) * dt;
            while self.owed[ei] >= 1.0 {
                self.owed[ei] -= 1.0;
                if self.particles.len() >= d.max {
                    continue;
                }
                if let Some(p) = self.emit(d, e, t) {
                    self.particles.push(p);
                }
            }
        }
    }

    fn emit(&mut self, d: &ParticleDesc, e: &EmitterDesc, t: f32) -> Option<Particle> {
        let ep = &e.params;
        let local = match ep.shape {
            EmitterShape::Box {
                width,
                height,
                depth,
                ..
            } => Vec3::new(
                self.rand_signed() * 0.5 * width,
                self.rand_signed() * 0.5 * height,
                self.rand_signed() * 0.5 * depth,
            ),
            EmitterShape::Cylinder { radius, height, .. } => {
                let a = self.rand() * std::f32::consts::TAU;
                let r = radius * self.rand().sqrt();
                Vec3::new(a.cos() * r, a.sin() * r, self.rand_signed() * 0.5 * height)
            }
            EmitterShape::Sphere { radius, .. } => {
                let dir = Vec3::new(self.rand_signed(), self.rand_signed(), self.rand_signed())
                    .normalize_or_zero();
                dir * radius * self.rand().cbrt()
            }
            EmitterShape::Mesh { .. } => Vec3::ZERO,
        };
        let decl = ep.declination + ep.declination_variation * self.rand_signed();
        let planar = ep.planar_angle + ep.planar_angle_variation * self.rand_signed();
        let dir_local = Vec3::new(
            decl.sin() * planar.cos(),
            decl.sin() * planar.sin(),
            decl.cos(),
        );
        let (position, direction) = match ep.shape {
            EmitterShape::Mesh {
                velocity_type,
                axis,
                ..
            } => self.mesh_sample(e, velocity_type, axis)?,
            _ => (
                e.object.transform_point3(local),
                e.object.transform_vector3(dir_local).normalize_or_zero(),
            ),
        };
        let base_speed = e.speed.as_ref().map_or(ep.speed, |c| c.sample(t));
        let speed = base_speed + ep.speed_variation * 0.5 * self.rand_signed();
        let base_radius = e.radius.as_ref().map_or(ep.radius, |c| c.sample(t));
        let radius = (base_radius + ep.radius_variation * 0.5 * self.rand_signed()).max(0.0);
        let base_life = e.life_span.as_ref().map_or(ep.life_span, |c| c.sample(t));
        let life = (base_life + ep.life_span_variation * 0.5 * self.rand_signed()).max(0.05);
        let (spin, angle) = match d.rotation {
            Some((s, sv, a, av, random_sign)) => {
                let mut spin = s + sv * self.rand_signed();
                if random_sign && self.rand() < 0.5 {
                    spin = -spin;
                }
                (spin, a + av * self.rand_signed())
            }
            None => (0.0, 0.0),
        };
        let frame = self.start_frame(d);
        Some(Particle {
            position,
            velocity: direction * speed,
            age: 0.0,
            life,
            radius,
            color: ep.color,
            angle,
            spin,
            frame,
            generation: 0,
        })
    }

    fn mesh_sample(
        &mut self,
        e: &EmitterDesc,
        velocity_type: u32,
        axis: Vec3,
    ) -> Option<(Vec3, Vec3)> {
        if e.meshes.is_empty() || velocity_type > 2 {
            return None;
        }
        // Uniform mesh selection; within a mesh surfaces are weighted by area
        // or length. Exact engine weighting remains an ambiguity.
        let mesh_index = ((self.rand() * e.meshes.len() as f32) as usize).min(e.meshes.len() - 1);
        let mesh = &e.meshes[mesh_index];
        let total = mesh.primitives.last()?.cumulative_weight;
        let pick = self.rand() * total;
        let index = mesh
            .primitives
            .partition_point(|p| p.cumulative_weight <= pick)
            .min(mesh.primitives.len() - 1);
        let primitive = &mesh.primitives[index];
        let weights = match primitive.count {
            2 => {
                let t = self.rand();
                [1.0 - t, t, 0.0]
            }
            3 => {
                let u = self.rand().sqrt();
                let v = self.rand();
                [1.0 - u, u * (1.0 - v), u * v]
            }
            _ => [1.0, 0.0, 0.0],
        };
        let point: Vec3 = primitive
            .positions
            .iter()
            .zip(weights)
            .map(|(&p, w)| p * w)
            .sum();
        let direction = match velocity_type {
            0 => {
                let normal: Vec3 = primitive
                    .normals
                    .iter()
                    .zip(weights)
                    .map(|(&n, w)| n * w)
                    .sum();
                (Mat3::from_mat4(mesh.object).inverse().transpose() * normal).normalize_or_zero()
            }
            1 => {
                let z = self.rand_signed();
                let angle = self.rand() * std::f32::consts::TAU;
                let radial = (1.0 - z * z).max(0.0).sqrt();
                mesh.object
                    .transform_vector3(Vec3::new(radial * angle.cos(), radial * angle.sin(), z))
                    .normalize_or_zero()
            }
            _ => mesh.object.transform_vector3(axis).normalize_or_zero(),
        };
        let position = mesh.object.transform_point3(point);
        (position.is_finite() && direction.is_finite()).then_some((position, direction))
    }

    fn start_frame(&mut self, d: &ParticleDesc) -> f32 {
        match d.subtex_anim {
            Some((start, fudge, ..)) => start + (fudge * self.rand()).floor(),
            None => (self.rand() * d.subtextures.len().max(1) as f32).floor(),
        }
    }

    /// Particles spawned where `parent` died.
    fn spawn_from(&mut self, d: &ParticleDesc, s: &SpawnDesc, parent: &Particle) {
        if parent.generation >= s.generations || self.rand() > s.percentage {
            return;
        }
        let (lo, hi) = (s.count.0.min(s.count.1), s.count.0.max(s.count.1));
        let n = lo + ((hi - lo + 1) as f32 * self.rand()).floor() as u16;
        for _ in 0..n {
            if self.particles.len() >= d.max {
                return;
            }
            let jitter = Vec3::new(self.rand_signed(), self.rand_signed(), self.rand_signed())
                * s.direction_variation;
            let speed = parent.velocity.length() * (1.0 + s.speed_variation * self.rand_signed());
            let dir = (parent.velocity.normalize_or_zero() + jitter).normalize_or_zero();
            let frame = self.start_frame(d);
            let life = (s.life_span + s.life_span_variation * 0.5 * self.rand_signed()).max(0.05);
            self.particles.push(Particle {
                velocity: dir * speed,
                age: 0.0,
                life,
                frame,
                generation: parent.generation + 1,
                ..*parent
            });
        }
    }

    /// The live particles as camera-facing quads in world space, through
    /// `model` (the instance's transform).
    pub fn quads(
        &self,
        d: &ParticleDesc,
        model: Mat4,
        camera_right: Vec3,
        camera_up: Vec3,
        out: &mut Vec<Vertex>,
    ) {
        let scale = model.x_axis.truncate().length() * d.size_scale;
        let cells = d.subtextures.len();
        let forward = camera_up.cross(camera_right);
        for p in &self.particles {
            let f = (p.age / p.life).clamp(0.0, 1.0);
            let size = p.radius * scale * sample_scale(&d.scales, f);
            if size <= 0.0 {
                continue;
            }
            let color = d.color.as_ref().map_or(p.color, |c| c.at(f) * p.color);
            if color.w <= 0.0 {
                continue;
            }
            let (mut right, mut up) = {
                let (sin, cos) = p.angle.sin_cos();
                (
                    camera_right * cos + camera_up * sin,
                    camera_up * cos - camera_right * sin,
                )
            };
            if d.velocity_oriented {
                let v = model.transform_vector3(p.velocity);
                let along = v - forward * v.dot(forward);
                if along.length_squared() > 1.0e-6 {
                    up = along.normalize();
                    right = up.cross(forward);
                }
            }
            let (right, up) = (right * size * d.aspect, up * size);
            let centre = model.transform_point3(p.position);
            let cell = if cells == 0 {
                Vec4::new(0.0, 1.0, 0.0, 1.0)
            } else {
                // Flipping from its start frame on to the end frame over its life.
                let frame = match d.subtex_anim {
                    Some((start, _, end, ..)) => p.frame + f * (end - start),
                    None => p.frame,
                };
                d.subtextures[(frame.max(0.0) as usize) % cells]
            };
            let corner = |sx: f32, sy: f32| Vertex {
                position: (centre + right * sx + up * sy).to_array(),
                normal: (camera_right.cross(camera_up)).to_array(),
                tangent: camera_right.to_array(),
                bitangent: camera_up.to_array(),
                uv: [
                    cell.x + cell.y * (sx * 0.5 + 0.5),
                    cell.z + cell.w * (0.5 - sy * 0.5),
                ],
                color: color.to_array(),
            };
            let (a, b, c, e) = (
                corner(-1.0, -1.0),
                corner(1.0, -1.0),
                corner(1.0, 1.0),
                corner(-1.0, 1.0),
            );
            out.extend([a, b, c, a, c, e]);
        }
    }
}

/// The size factor at `f` (0..1) of the particle's life: the scale curve's
/// values spread evenly over it.
fn sample_scale(scales: &[f32], f: f32) -> f32 {
    match scales.len() {
        0 => 1.0,
        1 => scales[0],
        n => {
            let x = f * (n - 1) as f32;
            let i = (x.floor() as usize).min(n - 2);
            scales[i] + (scales[i + 1] - scales[i]) * (x - i as f32)
        }
    }
}

/// A model's particle system, ready to draw.
pub struct GpuParticles {
    pub desc: Arc<ParticleDesc>,
    pub material: Arc<GpuMaterial>,
}

/// One system's quads this frame.
pub struct ParticleBatch {
    pub material: Arc<GpuMaterial>,
    pub vertices: Vec<Vertex>,
    /// Where it is, for sorting it among the blended draws.
    pub center: Vec3,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imported_float_constant_curve_differs_from_boolean_steps() {
        let mut nif = Nif {
            header: nif::Header {
                version: 0x14020007,
                user_version: 12,
                bs_version: 100,
                block_types: Vec::new(),
                block_type_index: Vec::new(),
                block_sizes: Vec::new(),
                block_offsets: Vec::new(),
                strings: Vec::new(),
            },
            blocks: vec![
                Block::ValueInterpolator(nif::anim::ValueInterpolator {
                    value: Vec3::splat(-f32::MAX),
                    data: Ref(1),
                }),
                Block::ValueKeys(nif::anim::ValueKeys {
                    keys: vec![(2.0, Vec3::ZERO), (6.0, Vec3::ONE)],
                    interpolation: 5,
                }),
            ],
            roots: Vec::new(),
        };
        let timing = nif::anim::Timing {
            flags: 8 | 4,
            frequency: 2.0,
            phase: 0.0,
            start: 2.0,
            stop: 6.0,
        };
        let nearest = curve(&nif, timing, Ref(0), -1.0);
        for (clock, expected) in [
            (-1.0, 0.0),
            (1.99, 0.0),
            (2.0, 1.0),
            (2.01, 1.0),
            (3.0, 1.0),
            (10.0, 1.0),
        ] {
            assert_eq!(nearest.sample(clock), expected);
        }
        nif.blocks[0] = Block::BoolInterpolator(nif::psys::BoolInterpolator {
            value: true,
            data: Ref(1),
        });
        let stepped = curve(&nif, timing, Ref(0), -1.0);
        assert_eq!(stepped.sample(2.0), 0.0);
        assert_eq!(stepped.sample(2.99), 0.0);
        assert_eq!(stepped.sample(3.0), 1.0);
    }

    #[test]
    fn reverse_particle_curves_respect_nonzero_start_and_step_keys() {
        let linear = Curve {
            timing: Some(nif::anim::Timing {
                flags: 8 | 2,
                frequency: 2.0,
                phase: 1.0,
                start: 3.0,
                stop: 5.0,
            }),
            keys: vec![(3.0, 0.0), (4.0, 10.0), (5.0, 20.0)],
            value: -1.0,
            interpolation: CurveInterpolation::Linear,
        };
        let stepped = Curve {
            interpolation: CurveInterpolation::Step,
            ..linear.clone()
        };
        for (time, expected, held) in [
            (1.0, 0.0, 0.0),
            (1.25, 5.0, 0.0),
            (1.5, 10.0, 10.0),
            (2.0, 20.0, 20.0),
            (2.25, 15.0, 10.0),
            (2.5, 10.0, 10.0),
            (2.75, 5.0, 0.0),
            (3.0, 0.0, 0.0),
            (9.25, 5.0, 0.0),
        ] {
            assert_eq!(linear.sample(time), expected, "linear at {time}");
            assert_eq!(stepped.sample(time), held, "stepped at {time}");
        }
    }

    fn drag_fixture(object: nif::Transform, axis: Vec3, range: f32, falloff: f32) -> ParticleDesc {
        let node = |transform, children| {
            Block::Node(nif::Node {
                av: nif::AvObject {
                    transform,
                    ..Default::default()
                },
                kind: nif::NodeKind::Plain,
                children,
                effects: Vec::new(),
            })
        };
        let nif = Nif {
            header: nif::Header {
                version: 0x14020007,
                user_version: 12,
                bs_version: 100,
                block_types: Vec::new(),
                block_type_index: Vec::new(),
                block_sizes: Vec::new(),
                block_offsets: Vec::new(),
                strings: Vec::new(),
            },
            roots: vec![0],
            blocks: vec![
                node(nif::Transform::default(), vec![Ref(1)]),
                node(object, Vec::new()),
                Block::ParticleData(Box::new(nif::psys::ParticleData {
                    max_particles: 10,
                    subtexture_offsets: Vec::new(),
                    aspect_ratio: 1.0,
                    aspect_flags: 0,
                    speed_to_aspect: (0.0, 0.0, 0.0),
                })),
                Block::ParticleModifier(Box::new(nif::psys::Modifier {
                    name: "drag".into(),
                    order: 4000,
                    active: true,
                    kind: ModifierKind::Drag {
                        object: Ref(1),
                        axis,
                        percentage: 0.5,
                        range,
                        falloff,
                    },
                })),
                Block::ParticleModifier(Box::new(nif::psys::Modifier {
                    name: "emitter".into(),
                    order: 1000,
                    active: false,
                    kind: ModifierKind::Emitter(nif::psys::Emitter {
                        speed: 0.0,
                        speed_variation: 0.0,
                        declination: 0.0,
                        declination_variation: 0.0,
                        planar_angle: 0.0,
                        planar_angle_variation: 0.0,
                        color: Vec4::ONE,
                        radius: 1.0,
                        radius_variation: 0.0,
                        life_span: 10.0,
                        life_span_variation: 0.0,
                        shape: EmitterShape::Sphere {
                            object: Ref(1),
                            radius: 0.0,
                        },
                    }),
                })),
            ],
        };
        let sys = nif::psys::ParticleSystem {
            av: nif::AvObject {
                net: nif::ObjectNet {
                    controller: Ref(-1),
                    ..Default::default()
                },
                ..Default::default()
            },
            bound_center: Vec3::ZERO,
            bound_radius: 100.0,
            shader: Ref(-1),
            alpha: Ref(-1),
            data: Ref(2),
            world_space: false,
            modifiers: vec![Ref(3), Ref(4)],
            strip: false,
        };
        describe(&nif, &sys, Mat4::IDENTITY, Mat4::IDENTITY).unwrap()
    }

    fn particle(position: Vec3, velocity: Vec3) -> Particle {
        Particle {
            position,
            velocity,
            age: 0.0,
            life: 10.0,
            radius: 1.0,
            color: Vec4::ONE,
            angle: 0.0,
            spin: 0.0,
            frame: 0.0,
            generation: 0,
        }
    }

    fn triangle_geometry() -> nif::Geometry {
        nif::Geometry {
            positions: vec![Vec3::ZERO, Vec3::X * 2.0, Vec3::Y * 2.0],
            normals: vec![Vec3::Z; 3],
            triangles: vec![[0, 1, 2]],
            ..Default::default()
        }
    }

    fn mesh_fixture(mode: u32, velocity_type: u32) -> ParticleDesc {
        let mut d = drag_fixture(nif::Transform::default(), Vec3::X, 0.0, 0.0);
        let e = &mut d.emitters[0];
        e.params.shape = EmitterShape::Mesh {
            meshes: vec![Ref(0)],
            velocity_type,
            emit_from: mode,
            axis: Vec3::X,
        };
        e.params.speed = 5.0;
        e.active = Curve::constant(1.0);
        e.birth_rate = Curve::constant(10.0);
        e.meshes = vec![MeshEmission::new(&triangle_geometry(), Mat4::IDENTITY, mode).unwrap()];
        d
    }

    #[test]
    fn mesh_description_resolves_rigid_legacy_and_packed_geometry() {
        let template = mesh_fixture(1, 0);
        let mut nif = Nif {
            header: nif::Header {
                version: 0x14020007,
                user_version: 12,
                bs_version: 100,
                block_types: Vec::new(),
                block_type_index: Vec::new(),
                block_sizes: Vec::new(),
                block_offsets: Vec::new(),
                strings: Vec::new(),
            },
            roots: vec![0],
            blocks: vec![
                Block::Node(nif::Node {
                    av: nif::AvObject::default(),
                    kind: nif::NodeKind::Plain,
                    children: vec![Ref(1)],
                    effects: Vec::new(),
                }),
                Block::NiTriShape(nif::NiGeometry {
                    av: nif::AvObject {
                        transform: nif::Transform {
                            translation: Vec3::X * 10.0,
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                    data: Ref(2),
                    skin: Ref(-1),
                    shader: Ref(-1),
                    alpha: Ref(-1),
                }),
                Block::TriShapeData(nif::TriShapeData {
                    geometry: triangle_geometry(),
                    center: Vec3::ZERO,
                    radius: 2.0,
                }),
                Block::ParticleData(Box::new(nif::psys::ParticleData {
                    max_particles: 10,
                    subtexture_offsets: Vec::new(),
                    aspect_ratio: 1.0,
                    aspect_flags: 0,
                    speed_to_aspect: (0.0, 0.0, 0.0),
                })),
                Block::ParticleModifier(Box::new(nif::psys::Modifier {
                    name: "mesh emitter".into(),
                    order: 1000,
                    active: true,
                    kind: ModifierKind::Emitter(nif::psys::Emitter {
                        shape: EmitterShape::Mesh {
                            meshes: vec![Ref(-1), Ref(1), Ref(99)],
                            velocity_type: 0,
                            emit_from: 1,
                            axis: Vec3::X,
                        },
                        ..template.emitters[0].params.clone()
                    }),
                })),
            ],
        };
        let sys = nif::psys::ParticleSystem {
            av: nif::AvObject::default(),
            bound_center: Vec3::ZERO,
            bound_radius: 100.0,
            shader: Ref(-1),
            alpha: Ref(-1),
            data: Ref(3),
            world_space: false,
            modifiers: vec![Ref(4)],
            strip: false,
        };
        let check = |nif: &Nif| {
            let d = describe(nif, &sys, Mat4::IDENTITY, Mat4::IDENTITY).unwrap();
            assert_eq!(d.emitters[0].meshes.len(), 1);
            let mut state = ParticleState::new(1);
            let p = state.emit(&d, &d.emitters[0], 0.0).unwrap();
            assert!(
                p.position
                    .abs_diff_eq(Vec3::new(10.0 + 2.0 / 3.0, 2.0 / 3.0, 0.0), 1e-6)
            );
        };
        check(&nif);
        let av = nif.blocks[1].av().unwrap().clone();
        nif.blocks[1] = Block::TriShape(nif::TriShape {
            av,
            bound_center: Vec3::ZERO,
            bound_radius: 2.0,
            skin: Ref(-1),
            shader: Ref(-1),
            alpha: Ref(-1),
            vertex_desc: 0,
            geometry: triangle_geometry(),
            dynamic: false,
        });
        check(&nif);
        if let Block::TriShape(shape) = &mut nif.blocks[1] {
            shape.dynamic = true;
        }
        assert!(mesh_geometry(&nif, Ref(1)).is_none());
        if let Block::TriShape(shape) = &mut nif.blocks[1] {
            shape.dynamic = false;
            shape.skin = Ref(0);
        }
        assert!(mesh_geometry(&nif, Ref(1)).is_none());
    }

    #[test]
    fn mesh_emission_samples_vertices_centres_and_surfaces() {
        for mode in 0..5 {
            let d = mesh_fixture(mode, 0);
            let mut state = ParticleState::new(13);
            let vertices = [Vec3::ZERO, Vec3::X * 2.0, Vec3::Y * 2.0];
            let edges = [Vec3::X, Vec3::Y, Vec3::X + Vec3::Y];
            let mut sum = Vec3::ZERO;
            for _ in 0..4096 {
                let p = state.emit(&d, &d.emitters[0], 0.0).unwrap();
                assert!(p.velocity.abs_diff_eq(Vec3::Z * 5.0, 1e-6));
                assert_eq!(p.position.z, 0.0);
                assert!(p.position.x >= 0.0 && p.position.y >= 0.0);
                assert!(p.position.x + p.position.y <= 2.0 + 1e-6);
                match mode {
                    0 => assert!(vertices.contains(&p.position)),
                    1 => assert!(
                        p.position
                            .abs_diff_eq(Vec3::new(2.0 / 3.0, 2.0 / 3.0, 0.0), 1e-6)
                    ),
                    2 => assert!(edges.contains(&p.position)),
                    4 => assert!(
                        p.position.x == 0.0
                            || p.position.y == 0.0
                            || (p.position.x + p.position.y - 2.0).abs() < 1e-6
                    ),
                    _ => {}
                }
                sum += p.position;
            }
            if mode == 3 {
                assert!((sum / 4096.0).abs_diff_eq(Vec3::new(2.0 / 3.0, 2.0 / 3.0, 0.0), 0.025));
            }
        }
    }

    #[test]
    fn mesh_emission_uses_each_mesh_transform_and_velocity_mode() {
        let mut d = mesh_fixture(1, 0);
        let transform = Mat4::from_scale_rotation_translation(
            Vec3::splat(2.0),
            glam::Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            Vec3::new(10.0, 20.0, 30.0),
        );
        d.emitters[0]
            .meshes
            .push(MeshEmission::new(&triangle_geometry(), transform, 1).unwrap());
        let mut state = ParticleState::new(37);
        let centre = Vec3::new(2.0 / 3.0, 2.0 / 3.0, 0.0);
        let mut counts = [0; 2];
        for _ in 0..256 {
            let p = state.emit(&d, &d.emitters[0], 0.0).unwrap();
            if p.position.abs_diff_eq(centre, 1e-6) {
                counts[0] += 1;
                assert!(p.velocity.abs_diff_eq(Vec3::Z * 5.0, 1e-5));
            } else {
                counts[1] += 1;
                assert!(
                    p.position
                        .abs_diff_eq(transform.transform_point3(centre), 1e-5)
                );
                assert!(p.velocity.abs_diff_eq(Vec3::X * 5.0, 1e-5));
            }
        }
        assert!(counts.iter().all(|&count| count > 80));
        d.emitters[0].meshes.remove(0);
        if let EmitterShape::Mesh { velocity_type, .. } = &mut d.emitters[0].params.shape {
            *velocity_type = 2;
        }
        let p = state.emit(&d, &d.emitters[0], 0.0).unwrap();
        assert!(p.velocity.abs_diff_eq(-Vec3::Z * 5.0, 1e-5));
        if let EmitterShape::Mesh { velocity_type, .. } = &mut d.emitters[0].params.shape {
            *velocity_type = 1;
        }
        let mut sum = Vec3::ZERO;
        for _ in 0..4096 {
            let p = state.emit(&d, &d.emitters[0], 0.0).unwrap();
            assert!((p.velocity.length() - 5.0).abs() < 1e-5);
            sum += p.velocity;
        }
        assert!((sum / 4096.0).length() < 0.15);
    }

    #[test]
    fn mesh_primitives_deduplicate_edges_weight_surfaces_and_reject_invalid_topology() {
        let mut geometry = triangle_geometry();
        geometry.positions.push(Vec3::new(2.0, 2.0, 0.0));
        geometry
            .triangles
            .extend([[1, 3, 2], [0, 0, 0], [0, 1, 99]]);
        geometry.normals.clear();
        let edges = MeshEmission::new(&geometry, Mat4::IDENTITY, 2).unwrap();
        assert_eq!(edges.primitives.len(), 5);
        let faces = MeshEmission::new(&geometry, Mat4::IDENTITY, 3).unwrap();
        assert_eq!(faces.primitives.len(), 2);
        assert_eq!(faces.primitives[1].cumulative_weight, 4.0);
        assert_eq!(faces.primitives[0].normals, [Vec3::Z; 3]);
        let edges = MeshEmission::new(&geometry, Mat4::IDENTITY, 4).unwrap();
        assert!(
            (edges.primitives.last().unwrap().cumulative_weight - (8.0 + 8.0_f32.sqrt())).abs()
                < 1e-6
        );
        geometry.triangles = vec![[0, 0, 0], [0, 1, 99]];
        assert!(MeshEmission::new(&geometry, Mat4::IDENTITY, 3).is_none());
        assert!(MeshEmission::new(&geometry, Mat4::ZERO, 0).is_none());
        assert!(MeshEmission::new(&geometry, Mat4::IDENTITY, 99).is_none());
        let mut d = mesh_fixture(0, 0);
        d.emitters[0].meshes.clear();
        let mut state = ParticleState::new(1);
        state.step(&d, 0.5, Mat3::IDENTITY);
        assert_eq!(state.count(), 0);
    }

    #[test]
    fn drag_uses_object_axis_position_and_scale() {
        let d = drag_fixture(
            nif::Transform {
                translation: Vec3::new(10.0, 20.0, 30.0),
                rotation: Mat3::from_rotation_z(std::f32::consts::FRAC_PI_2),
                scale: 2.0,
            },
            Vec3::X,
            2.0,
            3.0,
        );
        assert!(d.drag[0].axis.abs_diff_eq(Vec3::Y, 1e-6));
        assert_eq!(d.drag[0].position, Vec3::new(10.0, 20.0, 30.0));
        assert_eq!((d.drag[0].range, d.drag[0].falloff), (4.0, 6.0));
        let mut s = ParticleState::new(1);
        s.particles
            .push(particle(d.drag[0].position, Vec3::new(2.0, -4.0, 6.0)));
        s.step(&d, 0.5, Mat3::IDENTITY);
        assert!(
            s.particles[0]
                .velocity
                .abs_diff_eq(Vec3::new(2.0, -3.0, 6.0), 1e-6)
        );
        assert!(
            s.particles[0]
                .position
                .abs_diff_eq(Vec3::new(11.0, 18.5, 33.0), 1e-6)
        );
    }

    #[test]
    fn drag_range_and_falloff_limit_parallel_damping() {
        let d = drag_fixture(nif::Transform::default(), Vec3::X, 2.0, 4.0);
        let mut s = ParticleState::new(1);
        for distance in [0.0, 2.0, 4.0, 6.0, 8.0] {
            s.particles
                .push(particle(Vec3::Y * distance, Vec3::new(4.0, 2.0, -3.0)));
        }
        s.step(&d, 0.5, Mat3::IDENTITY);
        for (p, x) in s.particles.iter().zip([3.0, 3.0, 3.5, 4.0, 4.0]) {
            assert_eq!(p.velocity, Vec3::new(x, 2.0, -3.0));
        }
    }

    #[test]
    fn drag_respects_active_curve_and_zero_parameters() {
        let mut d = drag_fixture(nif::Transform::default(), Vec3::X, 0.0, 0.0);
        d.drag[0].active = Curve::constant(0.0);
        let mut s = ParticleState::new(1);
        s.particles.push(particle(Vec3::ZERO, Vec3::ONE));
        s.step(&d, 0.5, Mat3::IDENTITY);
        assert_eq!(s.particles[0].velocity, Vec3::ONE);

        d.drag[0].active = Curve::constant(1.0);
        s.particles[0].position = Vec3::ZERO;
        s.step(&d, 0.5, Mat3::IDENTITY);
        assert_eq!(s.particles[0].velocity, Vec3::new(0.75, 1.0, 1.0));
        s.particles[0].position = Vec3::Y;
        s.step(&d, 0.5, Mat3::IDENTITY);
        assert_eq!(s.particles[0].velocity, Vec3::new(0.75, 1.0, 1.0));
    }

    #[test]
    fn drag_degenerate_axis_and_large_percentage_do_not_reverse_velocity() {
        let mut d = drag_fixture(nif::Transform::default(), Vec3::ZERO, f32::MAX, f32::MAX);
        let mut velocity = Vec3::new(-2.0, 3.0, 4.0);
        d.drag[0].apply(Vec3::ZERO, &mut velocity, 1.0);
        assert_eq!(velocity, Vec3::new(-2.0, 3.0, 4.0));
        d.drag[0].axis = Vec3::X;
        d.drag[0].percentage = 10.0;
        d.drag[0].apply(Vec3::ZERO, &mut velocity, 1.0);
        assert_eq!(velocity, Vec3::new(0.0, 3.0, 4.0));
        for percentage in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            d.drag[0].percentage = percentage;
            let mut velocity = Vec3::ONE;
            d.drag[0].apply(Vec3::ZERO, &mut velocity, 1.0);
            assert_eq!(velocity, Vec3::ONE);
        }
    }

    #[test]
    fn simple_color_fades_and_blends() {
        let c = SimpleColor {
            fade_in: 0.1,
            fade_out: 0.9,
            times: [0.0, 0.5, 0.5, 1.0],
            colors: [
                Vec4::new(1.0, 0.0, 0.0, 1.0),
                Vec4::ONE,
                Vec4::new(0.0, 0.0, 1.0, 1.0),
            ],
        };
        assert!((c.at(0.05).w - 0.5).abs() < 1e-5);
        assert!((c.at(0.25).y - 0.5).abs() < 1e-5);
        assert!((c.at(0.95).w - 0.5).abs() < 1e-5);
        assert_eq!(sample_scale(&[1.0, 3.0], 0.5), 2.0);
    }
}
