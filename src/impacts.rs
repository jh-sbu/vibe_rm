//! Impacts: what a blow or an arrow leaves where it lands. The weapon's
//! impact data set (IPDS: `INAM`, a bash's `BIDS`, bare hands' and
//! creatures' their race's `NAM5`) pairs materials with impacts (IPCT); the
//! material struck is the target's race's (`NAM4`: skin, draugr skin,
//! canine...), the blocking shield's or weapon's (`BAMT`), or the surface's
//! for an arrow in the world. The impact plays its sound, runs its effect
//! model (blood sprays: addon-node particle systems emitting for the
//! impact's duration; spells' meshes), its nodes' own controllers and its
//! shaders' on a clock of its own, and leaves its decal on the surface it
//! lands on: for a wound, the blood thrown onto the floor or wall behind.
//! See `known_gaps/impacts.md`.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use esp::{FormId, LoadOrder};
use glam::{Mat4, Quat, Vec3};

use crate::engine::{Engine, PLAYER_REF};
use crate::render::{CellKey, Instance};
use crate::world::decal::DecalData;
use crate::world::loader::ModelAnim;
use crate::world::records;

/// Runtime decals kept at once (the oldest go first).
const MAX_DECALS: usize = 64;
/// How far blood thrown from a wound reaches for a surface.
const BLOOD_REACH: f32 = 320.0;
/// Decals kept on one actor's body (the oldest go first).
const MAX_SKIN_DECALS_PER_ACTOR: usize = 8;
/// Decals kept on all actors' bodies.
const MAX_SKIN_DECALS: usize = 48;
/// The longest an effect runs, whatever its particles and meshes.
const MAX_EFFECT_TIME: f32 = 8.0;

/// Which way an impact's effect points (`DATA` orientation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    SurfaceNormal,
    ProjectileVector,
    ProjectileReflection,
}

/// An impact (IPCT).
#[derive(Debug, Clone)]
pub struct Impact {
    pub id: FormId,
    /// Its effect model (`meshes/...`).
    pub model: Option<String>,
    /// How long the effect emits, seconds.
    pub duration: f32,
    pub orientation: Orientation,
    /// Surfaces met at more than this many degrees from head on take no decal.
    pub angle_threshold: f32,
    /// How far from the impact point its decal may land.
    pub placement_radius: f32,
    pub decal: Option<DecalData>,
    pub sound: Option<FormId>,
}

impl Impact {
    pub fn load(lo: &LoadOrder, id: FormId) -> Option<Impact> {
        let rec = lo.get(id)?;
        if rec.tag().0 != *b"IPCT" {
            return None;
        }
        let d = rec.get(b"DATA").filter(|d| d.len() >= 21)?;
        let f = |i: usize| f32::from_le_bytes(d[i..i + 4].try_into().unwrap());
        let form = |tag: &[u8; 4]| {
            rec.get(tag)
                .and_then(|d| d.get(..4))
                .map(|d| rec.fid(FormId(u32::from_le_bytes(d.try_into().unwrap()))))
                .filter(|f| !f.is_null())
        };
        let no_decal = d[20] & 0x01 != 0;
        let decal = if no_decal {
            None
        } else {
            let txst = form(b"DNAM")?;
            match rec.get(b"DODT") {
                Some(dodt) => DecalData::with_dodt(lo, txst, dodt),
                None => DecalData::load(lo, txst),
            }
        };
        Some(Impact {
            id,
            model: records::model_path(&rec),
            duration: f(0),
            orientation: match u32::from_le_bytes(d[4..8].try_into().unwrap()) {
                1 => Orientation::ProjectileVector,
                2 => Orientation::ProjectileReflection,
                _ => Orientation::SurfaceNormal,
            },
            angle_threshold: f(8),
            placement_radius: f(12),
            decal,
            sound: form(b"SNAM"),
        })
    }
}

/// An impact's effect running: its model's meshes and particle systems,
/// the nodes with controllers of their own (parts) moved, scaled and shown
/// by them, and its materials' controllers, all from when it started.
struct Effect {
    /// Where it was placed.
    transform: Mat4,
    /// The model's still meshes and particle systems, if any.
    root: Option<Instance>,
    anim: Option<Arc<ModelAnim>>,
    /// Each part (in `anim.parts` order) and whether it shows.
    parts: Vec<(Instance, bool)>,
    age: f32,
    duration: f32,
}

impl Effect {
    /// Pose the parts at the effect's age (each after the part it lies in);
    /// world-space particles stay where they were emitted.
    fn pose(&mut self) {
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
            let parent = match part.within {
                Some((o, rel)) if o < i => {
                    shown &= self.parts[o].1;
                    poses[o] * rel
                }
                _ => part.parent,
            };
            let pose = parent * local;
            poses.push(pose);
            let (inst, visible) = &mut self.parts[i];
            *visible = shown;
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
    fn instances_mut(&mut self) -> impl Iterator<Item = (&mut Instance, bool)> {
        self.root
            .iter_mut()
            .map(|i| (i, true))
            .chain(self.parts.iter_mut().map(|(i, v)| (i, *v)))
    }
}

#[derive(Default)]
pub struct ImpactState {
    loaded: HashMap<FormId, Option<Arc<Impact>>>,
    effects: Vec<Effect>,
    /// The cells holding the runtime decals, oldest first.
    decals: VecDeque<CellKey>,
    /// Decals on actors' bodies, by actor, oldest first.
    skin: HashMap<FormId, VecDeque<SkinDecal>>,
    /// The actors given each body decal, oldest first.
    skin_order: VecDeque<FormId>,
}

/// Bones a wound is held by: not those of equipment, cameras and the like,
/// nor fingers and toes (a hand raised before the chest would carry it off).
fn holds_wounds(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    ![
        "weapon",
        "shield",
        "quiver",
        "camera",
        "magic",
        "animobject",
        "sheath",
        "scb",
        "look",
        "pauldron",
        "finger",
        "toe",
    ]
    .iter()
    .any(|k| n.contains(k))
}

/// An actor's drawn instance (its pose this frame) and skeleton.
fn actor_pose<'a>(
    actor_cells: &HashMap<FormId, CellKey>,
    cells: &'a HashMap<CellKey, crate::engine::CellRuntime>,
    scene: &'a crate::render::Scene,
    actor: FormId,
) -> Option<(
    &'a crate::render::ActorInstance,
    &'a crate::world::skeleton::Skeleton,
)> {
    let key = actor_cells.get(&actor)?;
    let rt = cells.get(key)?;
    let i = rt.actors.iter().position(|a| a.ref_id == actor)?;
    let inst = scene.cells.get(key)?.actors.get(i)?;
    Some((inst, &rt.actors[i].skeleton))
}

/// A decal on an actor's body: its box held in a bone's space, moving with
/// the pose.
struct SkinDecal {
    bone: usize,
    /// The box in the bone's model-space frame.
    local: Mat4,
    decal: crate::render::decal::GpuDecal,
}

/// Where an impact lands: the point, the way the blow or arrow travelled
/// and the struck surface's normal.
#[derive(Debug, Clone, Copy)]
pub struct Landing {
    pub at: Vec3,
    pub dir: Vec3,
    pub normal: Vec3,
    /// The actor it struck (unguarded): blood goes on its body and on what
    /// lies behind it.
    pub actor: Option<FormId>,
}

/// A form held by a record's subrecord.
fn form_of(lo: &LoadOrder, rec: FormId, tag: &[u8; 4]) -> Option<FormId> {
    let r = lo.get(rec)?;
    let d = r.get(tag)?.get(..4)?;
    Some(r.fid(FormId(u32::from_le_bytes(d.try_into().unwrap())))).filter(|f| !f.is_null())
}

impl Engine {
    fn impact_data(&mut self, id: FormId) -> Option<Arc<Impact>> {
        self.impacts
            .loaded
            .entry(id)
            .or_insert_with(|| Impact::load(&self.lo, id).map(Arc::new))
            .clone()
    }

    /// An actor's (or the player's) race.
    fn race_of(&self, actor: FormId) -> Option<FormId> {
        self.templates_of(actor)?
            .form(&self.lo, crate::world::template::TRAITS, b"RNAM")
    }

    /// The impact data set of what `attacker` strikes with: a bash's shield or
    /// weapon (`BIDS`), else its weapon (`INAM`), else its race's (`NAM5`:
    /// bare hands, claws and teeth).
    fn strike_impacts(&self, attacker: FormId, bash: bool) -> Option<FormId> {
        let weapon = self.weapon_of(attacker);
        if bash {
            return self
                .equipped_shield(attacker)
                .or(weapon)
                .and_then(|f| form_of(&self.lo, f, b"BIDS"));
        }
        weapon
            .and_then(|w| form_of(&self.lo, w, b"INAM"))
            .or_else(|| form_of(&self.lo, self.race_of(attacker)?, b"NAM5"))
    }

    /// The material a blow on `target` meets: its guard's (the blocking
    /// shield's or weapon's `BAMT`) when it blocked, else its race's (`NAM4`).
    fn struck_material(&self, target: FormId, blocked: bool) -> Option<FormId> {
        if blocked
            && let Some(m) = self
                .equipped_shield(target)
                .or_else(|| self.weapon_of(target))
                .and_then(|f| form_of(&self.lo, f, b"BAMT"))
        {
            return Some(m);
        }
        form_of(&self.lo, self.race_of(target)?, b"NAM4")
    }

    /// Where an actor (or the player) stands and how tall it is.
    fn stance_of(&self, actor: FormId) -> Option<(Vec3, f32, f32)> {
        if actor == PLAYER_REF {
            let p = &self.physics;
            let feet = self.player.position - Vec3::Z * p.player_half_height;
            return Some((feet, p.player_half_height * 2.0, p.player_radius));
        }
        let a = self.actor_ref(actor)?;
        Some((a.pos, 120.0 * a.scale, 20.0 * a.scale))
    }

    /// A melee blow (or bash) by `attacker` landing on `target`.
    pub(crate) fn melee_impact(
        &mut self,
        target: FormId,
        attacker: FormId,
        blocked: bool,
        bash: bool,
    ) {
        let (Some((tp, height, radius)), Some((ap, _, _))) =
            (self.stance_of(target), self.stance_of(attacker))
        else {
            return;
        };
        let mut dir = (tp - ap).truncate().extend(0.0).normalize_or(Vec3::Y);
        // Blows land about the chest, at the side facing the attacker.
        let at = tp + Vec3::Z * height * 0.7 - dir * radius;
        dir = (dir + Vec3::Z * (self.frand() * 0.4 - 0.3)).normalize();
        let Some(ipds) = self.strike_impacts(attacker, bash) else {
            return;
        };
        let Some(material) = self.struck_material(target, blocked) else {
            return;
        };
        let landing = Landing {
            at,
            dir,
            normal: -dir,
            actor: (!blocked).then_some(target),
        };
        self.impact(ipds, material, landing);
    }

    /// An arrow from `shooter` striking `target` (an actor), or the world
    /// when none, at `at` flying along `dir`.
    pub(crate) fn arrow_impact(
        &mut self,
        shooter: FormId,
        target: Option<FormId>,
        at: Vec3,
        dir: Vec3,
    ) {
        let Some(ipds) = self
            .weapon_of(shooter)
            .and_then(|w| form_of(&self.lo, w, b"INAM"))
        else {
            return;
        };
        let (material, normal) = match target {
            Some(t) => (self.struck_material(t, false), -dir),
            None => match self.physics.surface_ray(at - dir * 16.0, dir, 48.0) {
                Some((_, n, s)) => (Some(self.surface_material(s, at)), n),
                None => (self.ground_material(at), -dir),
            },
        };
        let Some(material) = material else { return };
        let landing = Landing {
            at,
            dir,
            normal,
            actor: target,
        };
        self.impact(ipds, material, landing);
    }

    /// The impact `ipds` has for `material`, landing: its sound, effect and decal.
    pub(crate) fn impact(&mut self, ipds: FormId, material: FormId, landing: Landing) {
        let Some(ipct) = self.footsteps.impacts.impact(&self.lo, ipds, material) else {
            log::debug!("{ipds}: no impact on {material}");
            return;
        };
        self.land_impact(ipct, landing);
    }

    /// Console `impact`: an impact on what lies ahead of the camera, as an
    /// arrow flying along the view would leave it.
    pub(crate) fn test_impact(&mut self, ipct: FormId, reach: f32) -> String {
        let (from, dir) = (self.camera.position, self.camera.forward());
        let Some((t, normal)) = self.physics.ground_ray(from, dir, reach) else {
            return format!("nothing within {reach} units ahead");
        };
        let at = from + dir * t;
        self.land_impact(
            ipct,
            Landing {
                at,
                dir,
                normal,
                actor: None,
            },
        );
        format!("{ipct} at {at:.0}")
    }

    fn land_impact(&mut self, ipct: FormId, landing: Landing) {
        let Some(impact) = self.impact_data(ipct) else {
            return;
        };
        log::debug!(
            "impact {} at {:.0}",
            self.lo
                .get(ipct)
                .and_then(|r| r.editor_id())
                .unwrap_or_default(),
            landing.at
        );
        if let Some(s) = impact.sound {
            self.play_sound(s, landing.at);
        }
        if std::env::var_os("VRM_NO_IMPACTS").is_some() {
            return;
        }
        self.start_effect(&impact, landing);
        self.impact_decal(&impact, landing);
        if let Some(actor) = landing.actor {
            self.skin_decal(&impact, actor, landing);
        }
    }

    fn start_effect(&mut self, impact: &Impact, l: Landing) {
        let Some(model) = impact.model.as_ref() else {
            return;
        };
        let path = format!("{model}#effect");
        self.models
            .load_all(&mut self.renderer, &self.vfs, std::slice::from_ref(&path));
        let root = self.models.get(&path);
        let anim = self.models.anim(&path);
        if root.is_none() && anim.is_none() {
            log::debug!("impact effect {model}: nothing to show");
            return;
        }
        let n = l.normal.normalize_or(-l.dir);
        let facing = match impact.orientation {
            Orientation::SurfaceNormal => n,
            Orientation::ProjectileVector => l.dir,
            Orientation::ProjectileReflection => l.dir - 2.0 * l.dir.dot(n) * n,
        }
        .normalize_or(Vec3::Z);
        // The effects point along their +Z.
        let spin = Quat::from_rotation_z(self.frand() * std::f32::consts::TAU);
        let rot = Quat::from_rotation_arc(Vec3::Z, facing) * spin;
        let transform = Mat4::from_rotation_translation(rot, l.at);
        let seed = self.rand() as u32;
        let mut systems = 0u32;
        // Its animation may start later than 0 (its text keys' `start`).
        let start = anim.as_ref().map_or(0.0, |a| a.start);
        // Each instance on the effect's clock, its particle systems fresh.
        let mut instance = |model: &crate::render::GpuModel, at: Mat4| {
            let model = self.renderer.started_model(model, start);
            let mut inst = Instance::new(Arc::new(model), at);
            inst.particles = (0..inst.model.particles.len())
                .map(|_| {
                    systems += 1;
                    let mut s = crate::render::particles::ParticleState::new(
                        seed ^ systems.wrapping_mul(0x9E37_79B9),
                    );
                    s.time = start;
                    s
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
        log::debug!(
            "impact effect {model}: {systems} particle systems, {} parts, {} controllers, from {start:.2}s for {:.2}s, facing {facing:.2}",
            anim.as_ref().map_or(0, |a| a.parts.len()),
            anim.as_ref().map_or(0, |a| a.controllers.len()),
            impact.duration
        );
        let mut effect = Effect {
            transform,
            root,
            anim,
            parts,
            age: 0.0,
            duration: impact.duration.max(0.05),
        };
        effect.pose();
        self.impacts.effects.push(effect);
    }

    /// Run the impact effects (their parts' controllers and particles); those
    /// done are dropped.
    pub(crate) fn update_impact_effects(
        &mut self,
        dt: f32,
        right: Vec3,
        up: Vec3,
        batches: &mut Vec<crate::render::particles::ParticleBatch>,
    ) {
        self.impacts.effects.retain_mut(|e| {
            e.age += dt;
            e.pose();
            let (age, duration) = (e.age, e.duration);
            let mut alive = false;
            for (inst, shown) in e.instances_mut() {
                let model_from_world = glam::Mat3::from_mat4(inst.transform).inverse();
                for (state, sys) in inst.particles.iter_mut().zip(&inst.model.particles) {
                    state.stopped = age > duration || !shown;
                    state.step(&sys.desc, dt, model_from_world);
                    if state.count() == 0 {
                        continue;
                    }
                    alive = true;
                    let mut vertices = Vec::new();
                    state.quads(&sys.desc, inst.transform, right, up, &mut vertices);
                    if !vertices.is_empty() {
                        batches.push(crate::render::particles::ParticleBatch {
                            material: sys.material.clone(),
                            vertices,
                            center: inst.transform.transform_point3(sys.desc.bound_center),
                        });
                    }
                }
            }
            let age = e.age;
            log::trace!(
                "effect at {age:.2}s: {} particles",
                e.instances_mut()
                    .flat_map(|(i, _)| i.particles.iter().map(|p| p.count()))
                    .sum::<usize>()
            );
            (alive || e.age <= e.duration) && e.age < MAX_EFFECT_TIME
        });
    }

    /// Draw the impact effects' meshes (for their duration, the parts their
    /// controllers show) among the frame's moving instances.
    pub(crate) fn draw_impact_effects(&mut self) {
        let lights = &self.scene.lights;
        for e in &mut self.impacts.effects {
            if e.age > e.duration {
                continue;
            }
            for (inst, shown) in e.instances_mut() {
                if !shown || inst.model.parts.is_empty() {
                    continue;
                }
                let mut i = Instance::new(inst.model.clone(), inst.transform);
                i.lights = crate::render::pick_lights(lights, i.world_center, i.world_radius);
                self.scene.dynamic.push(i);
            }
        }
    }

    /// The loaded cell a point lies in.
    fn cell_at(&self, at: Vec3) -> Option<CellKey> {
        let interior = self
            .scene
            .cells
            .keys()
            .find(|k| matches!(k, CellKey::Interior(_)))
            .copied();
        interior.or_else(|| {
            let size = crate::world::terrain::CELL_SIZE;
            let key = CellKey::Exterior((at.x / size).floor() as i32, (at.y / size).floor() as i32);
            self.scene.cells.contains_key(&key).then_some(key)
        })
    }

    /// Leave the impact's decal: where the blow or arrow met the world, or for
    /// a wound, where the blood thrown on along the blow lands.
    fn impact_decal(&mut self, impact: &Impact, l: Landing) {
        let Some(data) = impact.decal.as_ref() else {
            return;
        };
        let (origin, dir, reach) = if l.actor.is_some() {
            // Thrown on and down, landing a little way behind.
            let down = 0.6 + self.frand() * 1.4;
            let side = l.dir.cross(Vec3::Z).normalize_or_zero() * (self.frand() - 0.5) * 0.6;
            (
                l.at,
                (l.dir + side - Vec3::Z * down).normalize(),
                BLOOD_REACH,
            )
        } else {
            (l.at - l.dir * 16.0, l.dir, 48.0)
        };
        let Some((t, normal)) = self.physics.ground_ray(origin, dir, reach) else {
            log::debug!("impact decal: nothing within {reach} along {dir:.2}");
            return;
        };
        // Met too obliquely: no mark.
        let angle = (-dir).dot(normal).clamp(-1.0, 1.0).acos().to_degrees();
        if angle > impact.angle_threshold {
            log::debug!("impact decal: met at {angle:.0} degrees");
            return;
        }
        let Some(key) = self.cell_at(origin + dir * t) else {
            return;
        };
        // Projected along the surface, turned at random about it.
        let into = -normal;
        let (u, v) = into.any_orthonormal_pair();
        let r = self.frand() * impact.placement_radius;
        let a = self.frand() * std::f32::consts::TAU;
        let hit = origin + dir * t + (u * a.cos() + v * a.sin()) * r;
        let rot = Quat::from_rotation_arc(Vec3::Y, into)
            * Quat::from_rotation_y(self.frand() * std::f32::consts::TAU);
        let w = data.min_width + (data.max_width - data.min_width) * self.frand();
        let h = data.min_height + (data.max_height - data.min_height) * self.frand();
        let size = Vec3::new(w, data.depth.max(1.0), h);
        let transform = Mat4::from_scale_rotation_translation(size, rot, hit);
        let sub = (self.rand() % 4) as u32;
        let decal = self.gpu_decal(impact.id, data, transform, sub);
        let Some(rc) = self.scene.cells.get_mut(&key) else {
            return;
        };
        log::debug!("impact decal at {hit:.0} ({w:.0} x {h:.0})");
        rc.decals.push(decal);
        self.impacts.decals.push_back(key);
        while self.impacts.decals.len() > MAX_DECALS {
            let Some(old) = self.impacts.decals.pop_front() else {
                break;
            };
            // Runtime decals are the cell's last, oldest first.
            if let Some(rc) = self.scene.cells.get_mut(&old)
                && let Some(i) = rc.decals.iter().position(|d| d.ref_id == 0)
            {
                rc.decals.remove(i);
            }
        }
    }

    /// Put the impact's decal on `actor`'s body where the blow or arrow
    /// went in, held by the bone nearest the wound.
    fn skin_decal(&mut self, impact: &Impact, actor: FormId, l: Landing) {
        let Some(data) = impact.decal.as_ref() else {
            return;
        };
        // The box reaches from outside the body to its middle along the blow.
        let center = l.at + l.dir * 10.0;
        let (bone, bone_world, bone_name) = {
            let Some((inst, skeleton)) =
                actor_pose(&self.actor_cells, &self.cells, &self.scene, actor)
            else {
                return;
            };
            let bone_at = |i: usize| inst.transform * inst.pose[i];
            let at = |i: usize| bone_at(i).w_axis.truncate();
            // The bone whose length (to its children) passes nearest the
            // wound's middle: a spine bone for a blow to the chest, not an arm
            // hanging beside it.
            let n = inst.pose.len().min(skeleton.bones.len());
            let mut reach = vec![f32::MAX; n];
            for i in 0..n {
                reach[i] = reach[i].min(at(i).distance(center));
                if let Some(p) = skeleton.bones[i].parent.filter(|&p| p < n) {
                    let (a, b) = (at(p), at(i));
                    let ab = b - a;
                    let t = ((center - a).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
                    reach[p] = reach[p].min((a + ab * t).distance(center));
                }
            }
            let Some(bone) = (0..n)
                // Not the model's root: it stays at the actor's origin.
                .filter(|&i| {
                    skeleton.bones[i].parent.is_some() && holds_wounds(&skeleton.bones[i].name)
                })
                .min_by(|&a, &b| reach[a].total_cmp(&reach[b]))
            else {
                return;
            };
            (bone, bone_at(bone), skeleton.bones[bone].name.clone())
        };
        let roll = Quat::from_rotation_y(self.frand() * std::f32::consts::TAU);
        let rot = Quat::from_rotation_arc(Vec3::Y, l.dir.normalize_or(Vec3::Y)) * roll;
        let w = data.min_width + (data.max_width - data.min_width) * self.frand();
        let h = data.min_height + (data.max_height - data.min_height) * self.frand();
        let size = Vec3::new(w, data.depth.max(24.0), h);
        let world = Mat4::from_scale_rotation_translation(size, rot, center);
        let sub = (self.rand() % 4) as u32;
        let decal = self.gpu_decal(impact.id, data, world, sub);
        log::debug!("skin decal on {actor} at {center:.0}, bone {bone_name}");
        let skin = &mut self.impacts.skin;
        let mine = skin.entry(actor).or_default();
        mine.push_back(SkinDecal {
            bone,
            local: bone_world.inverse() * world,
            decal,
        });
        if mine.len() > MAX_SKIN_DECALS_PER_ACTOR {
            mine.pop_front();
        }
        self.impacts.skin_order.push_back(actor);
        while skin.values().map(VecDeque::len).sum::<usize>() > MAX_SKIN_DECALS {
            let Some(oldest) = self.impacts.skin_order.pop_front() else {
                break;
            };
            if let Some(d) = skin.get_mut(&oldest) {
                d.pop_front();
            }
        }
        skin.retain(|_, d| !d.is_empty());
    }

    /// Place the decals on actors' bodies where their bones are this frame;
    /// those of actors no longer loaded are dropped.
    pub(crate) fn update_skin_decals(&mut self) {
        let mut out = Vec::new();
        let (cells, actor_cells, scene) = (&self.cells, &self.actor_cells, &self.scene);
        self.impacts.skin.retain(|&actor, decals| {
            let Some((inst, _)) = actor_pose(actor_cells, cells, scene, actor) else {
                return false;
            };
            for d in decals.iter() {
                let Some(pose) = inst.pose.get(d.bone) else {
                    continue;
                };
                let transform = inst.transform * *pose * d.local;
                let (scale, _, center) = transform.to_scale_rotation_translation();
                let radius = scale.length() * 0.5;
                let mut g = d.decal.clone();
                g.transform = transform;
                g.center = center;
                g.radius = radius;
                g.lights = crate::render::pick_lights(&scene.lights, center, radius);
                out.push(g);
            }
            true
        });
        let skin = &self.impacts.skin;
        self.impacts.skin_order.retain(|a| skin.contains_key(a));
        self.scene.actor_decals = out;
    }

    /// A random value in [0, 1).
    fn frand(&mut self) -> f32 {
        (self.rand() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Forget the runtime decals of a cell unloaded.
    pub(crate) fn forget_impact_decals(&mut self, key: CellKey) {
        self.impacts.decals.retain(|k| *k != key);
    }
}
