//! Impacts: what a blow or an arrow leaves where it lands. The weapon's
//! impact data set (IPDS: `INAM`, a bash's `BIDS`, bare hands' and
//! creatures' their race's `NAM5`) pairs materials with impacts (IPCT); the
//! material struck is the target's race's (`NAM4`: skin, draugr skin,
//! canine...), the blocking shield's or weapon's (`BAMT`), or the surface's
//! for an arrow in the world. The impact plays its sound, runs its effect
//! model (blood sprays: addon-node particle systems emitting for the
//! impact's duration) and leaves its decal on the surface it lands on: for a
//! wound, the blood thrown onto the floor or wall behind. See
//! `known_gaps/impacts.md`.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use esp::{FormId, LoadOrder};
use glam::{Mat4, Quat, Vec3};

use crate::engine::{Engine, PLAYER_REF};
use crate::render::{CellKey, Instance};
use crate::world::decal::DecalData;
use crate::world::records;

/// Runtime decals kept at once (the oldest go first).
const MAX_DECALS: usize = 64;
/// How far blood thrown from a wound reaches for a surface.
const BLOOD_REACH: f32 = 320.0;
/// The longest an effect runs, whatever its particles.
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

/// An impact's effect running.
struct Effect {
    inst: Instance,
    age: f32,
    duration: f32,
}

#[derive(Default)]
pub struct ImpactState {
    loaded: HashMap<FormId, Option<Arc<Impact>>>,
    effects: Vec<Effect>,
    /// The cells holding the runtime decals, oldest first.
    decals: VecDeque<CellKey>,
}

/// Where an impact lands: the point, the way the blow or arrow travelled
/// and the struck surface's normal.
#[derive(Debug, Clone, Copy)]
pub struct Landing {
    pub at: Vec3,
    pub dir: Vec3,
    pub normal: Vec3,
    /// It struck an actor: its decal goes on what lies behind.
    pub on_actor: bool,
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
            on_actor: !blocked,
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
            on_actor: target.is_some(),
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
                on_actor: false,
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
    }

    fn start_effect(&mut self, impact: &Impact, l: Landing) {
        let Some(path) = impact.model.clone() else {
            return;
        };
        self.models
            .load_all(&mut self.renderer, &self.vfs, std::slice::from_ref(&path));
        let Some(model) = self.models.get(&path) else {
            log::debug!("impact effect {path}: no model");
            return;
        };
        if model.particles.is_empty() {
            log::debug!("impact effect {path}: no particle systems");
            return;
        }
        let n = l.normal.normalize_or(-l.dir);
        let facing = match impact.orientation {
            Orientation::SurfaceNormal => n,
            Orientation::ProjectileVector => l.dir,
            Orientation::ProjectileReflection => l.dir - 2.0 * l.dir.dot(n) * n,
        }
        .normalize_or(Vec3::Z);
        // The effects' emitters point along their +Z.
        let spin = Quat::from_rotation_z(self.frand() * std::f32::consts::TAU);
        let rot = Quat::from_rotation_arc(Vec3::Z, facing) * spin;
        log::debug!(
            "impact effect {path}: {} particle systems, {:.2}s, facing {facing:.2}",
            model.particles.len(),
            impact.duration
        );
        let mut inst = Instance::new(model, Mat4::from_rotation_translation(rot, l.at));
        inst.lights =
            crate::render::pick_lights(&self.scene.lights, inst.world_center, inst.world_radius);
        let seed = self.rand() as u32;
        inst.particles = (0..inst.model.particles.len())
            .map(|i| {
                crate::render::particles::ParticleState::new(
                    seed ^ (i as u32).wrapping_mul(0x9E37_79B9),
                )
            })
            .collect();
        self.impacts.effects.push(Effect {
            inst,
            age: 0.0,
            duration: impact.duration.max(0.05),
        });
    }

    /// Run the impact effects' particles; those done are dropped.
    pub(crate) fn update_impact_effects(
        &mut self,
        dt: f32,
        right: Vec3,
        up: Vec3,
        batches: &mut Vec<crate::render::particles::ParticleBatch>,
    ) {
        self.impacts.effects.retain_mut(|e| {
            e.age += dt;
            let model_from_world = glam::Mat3::from_mat4(e.inst.transform).inverse();
            let mut alive = false;
            for (state, sys) in e.inst.particles.iter_mut().zip(&e.inst.model.particles) {
                state.stopped = e.age > e.duration;
                state.step(&sys.desc, dt, model_from_world);
                if state.count() == 0 {
                    continue;
                }
                alive = true;
                let mut vertices = Vec::new();
                state.quads(&sys.desc, e.inst.transform, right, up, &mut vertices);
                if !vertices.is_empty() {
                    batches.push(crate::render::particles::ParticleBatch {
                        material: sys.material.clone(),
                        vertices,
                        center: e.inst.transform.transform_point3(sys.desc.bound_center),
                    });
                }
            }
            log::trace!(
                "effect {} at {:.2}s: {} particles",
                e.inst.model.path,
                e.age,
                e.inst.particles.iter().map(|p| p.count()).sum::<usize>()
            );
            (alive || e.age <= e.duration) && e.age < MAX_EFFECT_TIME
        });
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
        let (origin, dir, reach) = if l.on_actor {
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

    /// A random value in [0, 1).
    fn frand(&mut self) -> f32 {
        (self.rand() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Forget the runtime decals of a cell unloaded.
    pub(crate) fn forget_impact_decals(&mut self, key: CellKey) {
        self.impacts.decals.retain(|k| *k != key);
    }
}
