//! Trigger volumes: references with a box or sphere primitive (`XPRM`), or
//! whose model has phantoms (`bhkSimpleShapePhantom`: pressure plates, trip
//! wires), and a script. Actors (and the player) stepping into one send its scripts
//! `OnTriggerEnter`, stepping out `OnTriggerLeave`, with themselves as
//! `akActionRef`; `GetTriggerObjectCount` counts who is inside.

use esp::FormId;
use glam::{Mat4, Quat, Vec3};
use rapier3d::prelude::{Pose, SharedShape};

use crate::engine::{Engine, PLAYER_REF};
use crate::render::CellKey;

/// Heights above the feet tested against a volume: an actor is in it when any is.
const BODY_POINTS: [f32; 3] = [10.0, 60.0, 110.0];
/// A body's reach beyond those points (about a capsule's radius).
const BODY_RADIUS: f32 = 20.0;

/// The space a trigger takes up.
enum Volume {
    /// A primitive: its centre, world to its frame, its half extents (a
    /// sphere's radius in x).
    Primitive {
        centre: Vec3,
        inv_rotation: Quat,
        half: Vec3,
        sphere: bool,
    },
    /// The model's phantoms, placed in the world.
    Phantoms(Vec<(Pose, SharedShape)>),
}

pub struct Trigger {
    pub ref_id: FormId,
    volume: Volume,
    /// Who is inside.
    pub inside: Vec<FormId>,
}

impl Trigger {
    fn contains(&self, p: Vec3) -> bool {
        match &self.volume {
            Volume::Primitive {
                centre,
                inv_rotation,
                half,
                sphere,
            } => {
                let l = *inv_rotation * (p - *centre);
                if *sphere {
                    l.length() <= half.x + BODY_RADIUS
                } else {
                    l.abs().cmple(*half + Vec3::splat(BODY_RADIUS)).all()
                }
            }
            Volume::Phantoms(shapes) => shapes
                .iter()
                .any(|(pose, s)| s.distance_to_point(pose, p, true) <= BODY_RADIUS),
        }
    }
}

/// Whether a reference or its base has scripts.
fn scripted(lo: &esp::LoadOrder, rec: &esp::LoadedRecord<'_>, base: FormId) -> bool {
    rec.get(b"VMAD").is_some() || lo.get(base).is_some_and(|b| b.get(b"VMAD").is_some())
}

/// The trigger of a scripted reference whose model has phantoms, placed at
/// `transform` (scale included).
pub fn phantom_trigger(
    lo: &esp::LoadOrder,
    r: FormId,
    model: &crate::physics::shapes::CollisionModel,
    transform: Mat4,
) -> Option<Trigger> {
    if model.phantoms.is_empty() {
        return None;
    }
    let rec = lo.get(r)?;
    if !scripted(lo, &rec, crate::world::records::reference(&rec).base) {
        return None;
    }
    let shapes = model
        .phantoms
        .iter()
        .filter_map(|p| {
            let (pose, scale) = crate::physics::shapes::decompose(transform * p.transform);
            Some((pose, p.shape.build_solid(scale)?))
        })
        .collect();
    Some(Trigger {
        ref_id: r,
        volume: Volume::Phantoms(shapes),
        inside: Vec::new(),
    })
}

/// The trigger volume of a reference, if it is one: a box or sphere primitive
/// (`XPRM`: half extents, colour, a float, type 1 box / 2 sphere) with scripts
/// on it or its base.
pub fn trigger_of(lo: &esp::LoadOrder, r: FormId) -> Option<Trigger> {
    let rec = lo.get(r)?;
    let p = rec.get(b"XPRM").filter(|p| p.len() >= 32)?;
    let f = |o: usize| f32::from_le_bytes(p[o..o + 4].try_into().unwrap());
    let shape = u32::from_le_bytes(p[28..32].try_into().unwrap());
    if !matches!(shape, 1 | 2) {
        return None;
    }
    let rf = crate::world::records::reference(&rec);
    if !scripted(lo, &rec, rf.base) {
        return None;
    }
    Some(Trigger {
        ref_id: r,
        volume: Volume::Primitive {
            centre: rf.position,
            inv_rotation: rf.rotation_quat().inverse(),
            half: Vec3::new(f(0), f(4), f(8)) * rf.scale,
            sphere: shape == 2,
        },
        inside: Vec::new(),
    })
}

impl Engine {
    /// Set up the trigger volumes among a loaded cell's references.
    pub(crate) fn load_triggers(&mut self, key: CellKey, refs: &[FormId]) {
        let triggers: Vec<Trigger> = refs
            .iter()
            .filter_map(|&r| {
                trigger_of(&self.lo, r).or_else(|| {
                    let rec = self.lo.get(r)?;
                    let rf = crate::world::records::reference(&rec);
                    let model = crate::world::records::model_path(&self.lo.get(rf.base)?)?;
                    let c = self.models.collision(&model)?;
                    let transform = Mat4::from_scale_rotation_translation(
                        Vec3::splat(rf.scale),
                        rf.rotation_quat(),
                        rf.position,
                    );
                    phantom_trigger(&self.lo, r, &c, transform)
                })
            })
            .collect();
        if !triggers.is_empty() {
            log::debug!("{key:?}: {} trigger volumes", triggers.len());
        }
        if let Some(rt) = self.cells.get_mut(&key) {
            rt.triggers = triggers;
        }
    }

    /// Send enter / leave events for who stepped in or out (called every frame).
    pub(crate) fn update_triggers(&mut self) {
        // Who can set them off: the player and the loaded, living actors.
        let mut bodies: Vec<(FormId, Vec3)> = self
            .cells
            .values()
            .flat_map(|rt| &rt.actors)
            .filter(|a| !a.dead)
            .map(|a| (a.ref_id, a.pos))
            .collect();
        if self.player_died_at.is_none()
            && let Some(p) = self.ref_position(PLAYER_REF)
        {
            bodies.push((PLAYER_REF, p));
        }
        let mut events: Vec<(FormId, FormId, bool)> = Vec::new();
        let keys: Vec<CellKey> = self.cells.keys().copied().collect();
        for key in keys {
            let n = self.cells[&key].triggers.len();
            for i in 0..n {
                let r = self.cells[&key].triggers[i].ref_id;
                let disabled = self.is_disabled(r);
                let t = &mut self.cells.get_mut(&key).unwrap().triggers[i];
                if disabled {
                    t.inside.clear();
                    continue;
                }
                let now: Vec<FormId> = bodies
                    .iter()
                    .filter(|(_, p)| BODY_POINTS.iter().any(|h| t.contains(*p + Vec3::Z * h)))
                    .map(|(b, _)| *b)
                    .collect();
                for &b in now.iter().filter(|b| !t.inside.contains(b)) {
                    events.push((r, b, true));
                }
                for &b in t.inside.iter().filter(|b| !now.contains(b)) {
                    events.push((r, b, false));
                }
                t.inside = now;
            }
        }
        for (r, who, enter) in events {
            log::debug!(
                "{who} {} trigger {r} {:?}",
                if enter { "enters" } else { "leaves" },
                self.vm.attached_scripts(papyrus::ObjectId::Form(r.0))
            );
            let action = self.object_value(who);
            let event = if enter {
                "OnTriggerEnter"
            } else {
                "OnTriggerLeave"
            };
            for obj in self.objects_of_ref(r) {
                self.scripts
                    .pending_events
                    .push((obj, event.into(), vec![action.clone()]));
            }
        }
    }

    /// How many are inside trigger `r` (`GetTriggerObjectCount`).
    pub fn trigger_object_count(&self, r: FormId) -> usize {
        self.cells
            .values()
            .flat_map(|rt| &rt.triggers)
            .find(|t| t.ref_id == r)
            .map_or(0, |t| t.inside.len())
    }
}
