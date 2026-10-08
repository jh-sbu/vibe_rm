//! What happens to references stays when their cells unload and load again (in
//! memory; what saves will write out): dead actors stay dead, lying where they
//! fell, in the pose they lay in; the hurt come back as hurt as they left, less
//! what they would have healed meanwhile; doors the player opened stay open.
//! Taken items, locks, enable state and inventories are kept elsewhere
//! (`ScriptState`, `Engine::inventories`).

use std::collections::HashMap;

use esp::FormId;
use glam::{Mat4, Vec3};

use crate::ai::schedule::Place;
use crate::engine::Engine;

#[derive(Default)]
pub struct WorldState {
    /// Dead actors and where their bodies lie.
    pub dead: HashMap<FormId, (Place, Vec3)>,
    /// Animated doors the player left open.
    pub open_doors: std::collections::HashSet<FormId>,
    /// Dead actors' ragdoll bodies as they lay (world transforms).
    pub body_poses: HashMap<FormId, Vec<Mat4>>,
    /// Hurt actors that unloaded.
    pub wounds: HashMap<FormId, Wounds>,
    /// References scripts moved (`MoveTo`, `SetPosition`, `SetAngle`): where to.
    pub moved: HashMap<FormId, Moved>,
    /// Loose objects' breakable joints that broke (by index in the model).
    pub broken_joints: HashMap<FormId, Vec<usize>>,
}

/// Where a moved reference is now.
#[derive(Debug, Clone, Copy)]
pub struct Moved {
    pub place: Place,
    pub pos: Vec3,
    /// Euler angles (radians), as references' `DATA`.
    pub rot: Vec3,
}

/// An actor's health as it unloaded, and how fast it comes back.
#[derive(Debug, Clone, Copy)]
pub struct Wounds {
    pub health: f32,
    pub max: f32,
    /// Share of the most a second (out of combat).
    pub rate: f32,
    /// `ScriptState::real_time` it unloaded at.
    pub at: f64,
}

impl Wounds {
    /// Health now, having healed since.
    pub fn health_at(&self, now: f64) -> f32 {
        (self.health + self.max * self.rate * (now - self.at).max(0.0) as f32).min(self.max)
    }
}

impl Engine {
    /// An actor died: remember it, and where (until its body settles elsewhere).
    pub(crate) fn remember_dead(&mut self, actor: FormId) {
        if let Some(at) = self.current_place_of(actor) {
            self.world_state.dead.insert(actor, at);
        }
    }

    /// Before a cell unloads: where its dead actors' bodies came to rest (and
    /// how), and how hurt the living are.
    pub(crate) fn remember_actors(&mut self, key: crate::render::CellKey) {
        let Some(place) = self.place_of_key(key) else {
            return;
        };
        let Some(rt) = self.cells.get(&key) else {
            return;
        };
        let now = self.scripts.real_time;
        let mut rest = Vec::new();
        let mut hurt = Vec::new();
        for a in &rt.actors {
            if !a.dead {
                let w = (a.health < a.stats.max_health - 0.5).then(|| Wounds {
                    health: a.health,
                    max: a.stats.max_health,
                    rate: a.stats.health_regen / 100.0,
                    at: now,
                });
                hurt.push((a.ref_id, w));
                continue;
            }
            let bodies = a
                .ragdoll
                .as_ref()
                .map(|(rd, _)| self.physics.ragdoll_bodies(rd));
            let mut pos = bodies
                .as_ref()
                .and_then(|b| b.first())
                .map_or(a.pos, |m| m.w_axis.truncate());
            if let Some(z) = self.nav.height_at(pos) {
                pos.z = z;
            }
            rest.push((a.ref_id, pos, bodies));
        }
        for (r, pos, bodies) in rest {
            self.world_state.dead.insert(r, (place, pos));
            if let Some(b) = bodies {
                self.world_state.body_poses.insert(r, b);
            }
        }
        for (r, w) in hurt {
            match w {
                Some(w) => self.world_state.wounds.insert(r, w),
                None => self.world_state.wounds.remove(&r),
            };
        }
    }

    /// The health a loading actor comes back with (its most, unless it left hurt).
    pub(crate) fn returning_health(&mut self, actor: FormId, max: f32) -> f32 {
        let now = self.scripts.real_time;
        self.world_state
            .wounds
            .remove(&actor)
            .map_or(max, |w| w.health_at(now).min(max))
    }

    /// Where a dead actor's body lies.
    pub fn body_of(&self, actor: FormId) -> Option<(Place, Vec3)> {
        self.world_state.dead.get(&actor).copied()
    }
}

impl Engine {
    /// A reference's rotation now (Euler angles, radians).
    pub fn ref_rotation(&self, r: FormId) -> Vec3 {
        if r == crate::engine::PLAYER_REF {
            return Vec3::new(0.0, 0.0, self.camera.yaw);
        }
        if let Some(a) = self.actor_ref(r) {
            return Vec3::new(0.0, 0.0, a.heading);
        }
        if let Some(m) = self.world_state.moved.get(&r) {
            return m.rot;
        }
        self.reference_of(r)
            .map(|rf| rf.rotation)
            .unwrap_or_default()
    }

    /// The place a position is in, in the worldspace or interior of `like`.
    pub(crate) fn place_at(like: Place, pos: Vec3) -> Place {
        match like {
            Place::Interior(c) => Place::Interior(c),
            Place::Exterior(w, _) => Place::Exterior(w, crate::engine::grid_of(pos.truncate())),
        }
    }

    /// Move a reference (scripts' `MoveTo`, `SetPosition`, `SetAngle`): to `pos`
    /// with rotation `rot`, in the place of `near` (the reference moved to, or
    /// itself). Remembered across cell loads.
    pub fn move_ref(&mut self, r: FormId, near: FormId, pos: Vec3, rot: Vec3) -> bool {
        if r == crate::engine::PLAYER_REF {
            self.queue_player_moveto(near);
            return true;
        }
        let Some((like, _)) = self
            .current_place_of(near)
            .or_else(|| self.current_place_of(r))
        else {
            return false;
        };
        self.move_ref_to(r, Self::place_at(like, pos), pos, rot)
    }

    /// Back where it was placed in the editor (`MoveToMyEditorLocation`).
    pub fn move_to_editor_location(&mut self, r: FormId) -> bool {
        let Some(rf) = self
            .lo
            .get(r)
            .map(|rec| crate::world::records::reference(&rec))
        else {
            return false;
        };
        let Some(place) = self.editor_place_of(r, rf.position) else {
            return false;
        };
        let done = self.move_ref_to(r, place, rf.position, rf.rotation);
        self.world_state.moved.remove(&r);
        done
    }

    fn move_ref_to(&mut self, r: FormId, place: Place, pos: Vec3, rot: Vec3) -> bool {
        let actor = self.created(r).map_or_else(
            || self.lo.tag_of(r).is_some_and(|t| t.0 == *b"ACHR"),
            |c| c.actor,
        );
        let old = self.ref_transform(r);
        self.world_state.moved.insert(r, Moved { place, pos, rot });
        if let Some(c) = self.created_refs.refs.get_mut(&r) {
            (c.position, c.rotation, c.place) = (pos, rot, Some(place));
        }
        log::debug!("{r} moved to {pos:?} in {place:?}");
        if actor {
            self.move_actor(r, place, pos, rot);
        } else if let Some(old) = old {
            self.move_object(r, place, old, pos, rot);
        }
        true
    }

    /// Where a reference is and how it is turned (scale included), now.
    fn ref_transform(&self, r: FormId) -> Option<Mat4> {
        let rf = self.reference_of(r)?;
        let pos = self.ref_position(r)?;
        let rot = self.ref_rotation(r);
        Some(Mat4::from_scale_rotation_translation(
            Vec3::splat(rf.scale),
            crate::world::records::rotation_from_euler(rot),
            pos,
        ))
    }

    fn move_actor(&mut self, r: FormId, place: Place, pos: Vec3, rot: Vec3) {
        self.whereabouts.add_actor(r, place, pos);
        let here = self.actor_cells.get(&r).copied();
        let there = self.key_of_place(place);
        match (here, there) {
            (Some(h), Some(t)) if h == t => {
                if let Some(a) = self.actor_mut(r) {
                    a.pos = pos;
                    a.heading = rot.z;
                    a.halt(0.5);
                }
                self.moved_refs.insert(r, pos);
            }
            _ => {
                if here.is_some() {
                    self.despawn_actor(r);
                }
                if let Some(t) = there {
                    self.spawn_actors(t, &[(r, Some(pos))]);
                }
            }
        }
    }

    fn move_object(&mut self, r: FormId, place: Place, old: Mat4, pos: Vec3, rot: Vec3) {
        let scale = old.to_scale_rotation_translation().0.x;
        let new = Mat4::from_scale_rotation_translation(
            Vec3::splat(scale),
            crate::world::records::rotation_from_euler(rot),
            pos,
        );
        let delta = new * old.inverse();
        let loaded = self.cells.values().any(|rt| rt.refs.contains(&r));
        let shown = self.key_of_place(place).is_some();
        if loaded && shown {
            for rc in self.scene.cells.values_mut() {
                for inst in rc.instances.iter_mut().filter(|i| i.ref_id == r.0) {
                    inst.transform = delta * inst.transform;
                    inst.world_center = inst.transform.transform_point3(inst.model.bound_center);
                }
            }
            self.physics.transform_owner(r, delta);
            self.loose_moved(r, delta);
            let mut lights = false;
            for rt in self.cells.values_mut() {
                for l in rt.lights.iter_mut().filter(|l| l.ref_id == r) {
                    l.position = pos;
                    lights = true;
                }
                for d in rt.doors.iter_mut().filter(|d| d.ref_id == r) {
                    d.position = pos;
                }
                for o in rt.animated.iter_mut().filter(|o| o.ref_id == r) {
                    o.transform = delta * o.transform;
                    o.position = o.transform.w_axis.truncate();
                }
            }
            if lights {
                self.rebuild_lights();
            }
        } else if loaded {
            // Moved somewhere not loaded: gone from here.
            for rc in self.scene.cells.values_mut() {
                for inst in rc.instances.iter_mut().filter(|i| i.ref_id == r.0) {
                    inst.hidden = true;
                }
            }
            self.physics.set_owner_enabled(r, false);
        }
        // Moved into a loaded cell from one that isn't: shown when a cell holding
        // it loads (`apply_moves`).
    }

    /// While building a cell's objects: leave out references moved elsewhere, put
    /// those moved here where they were moved to.
    pub(crate) fn apply_moves(
        &self,
        place: Place,
        objects: &mut Vec<crate::world::cell::PlacedObject>,
        lights: &mut Vec<crate::world::cell::PointLight>,
        doors: &mut Vec<crate::world::cell::Door>,
    ) {
        if self.world_state.moved.is_empty() {
            return;
        }
        let away = |r: &FormId| {
            self.world_state
                .moved
                .get(r)
                .is_some_and(|m| m.place != place)
        };
        objects.retain(|o| !away(&o.ref_id));
        lights.retain(|l| !away(&l.ref_id));
        doors.retain(|d| !away(&d.ref_id));
        for (&r, m) in self
            .world_state
            .moved
            .iter()
            .filter(|(_, m)| m.place == place)
        {
            if self.created(r).is_some() || self.lo.tag_of(r).is_some_and(|t| t.0 == *b"ACHR") {
                continue;
            }
            objects.retain(|o| o.ref_id != r);
            lights.retain(|l| l.ref_id != r);
            doors.retain(|d| d.ref_id != r);
            let (mut o, mut l, mut d) = (Vec::new(), Vec::new(), Vec::new());
            crate::world::cell::add_reference(&self.lo, r, &mut o, &mut l, &mut d);
            for mut x in o {
                let scale = x.transform.to_scale_rotation_translation().0.x;
                x.transform = Mat4::from_scale_rotation_translation(
                    Vec3::splat(scale),
                    crate::world::records::rotation_from_euler(m.rot),
                    m.pos,
                );
                objects.push(x);
            }
            lights.extend(l.into_iter().map(|x| crate::world::cell::PointLight {
                position: m.pos,
                ..x
            }));
            doors.extend(d.into_iter().map(|x| crate::world::cell::Door {
                position: m.pos,
                ..x
            }));
        }
    }
}
