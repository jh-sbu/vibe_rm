//! Where persistent actors are: package-driven placement across cells, and
//! actors leaving / arriving through load doors while the player watches.

use std::collections::HashMap;

use esp::FormId;
use glam::Vec3;

use super::package::{LocationKind, Package};
use crate::engine::{Engine, Location};
use crate::render::CellKey;
use crate::world::records;

/// Real seconds between whereabouts refreshes.
const REFRESH_INTERVAL: f32 = 20.0;
/// Actors re-evaluated per frame during a refresh.
const REFRESH_BUDGET: usize = 250;
/// Actors farther than this from the player may appear / vanish without walking through a door.
const UNSEEN_DISTANCE: f32 = 2500.0;

/// A cell an actor can be in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Place {
    Interior(FormId),
    Exterior(FormId, (i32, i32)),
}

#[derive(Default)]
pub struct Whereabouts {
    /// Persistent actor references (built once).
    actors: Vec<FormId>,
    packages: HashMap<FormId, std::sync::Arc<Vec<Package>>>,
    /// Scheduled place and position for each persistent actor.
    pub of: HashMap<FormId, (Place, Vec3)>,
    /// `of` before the last refresh.
    prev: HashMap<FormId, (Place, Vec3)>,
    pub by_place: HashMap<Place, Vec<FormId>>,
    pub built: bool,
    pub next_refresh: f32,
    /// Incremental refresh in progress: next actor index and results so far.
    cursor: usize,
    pending: HashMap<FormId, (Place, Vec3)>,
}

impl Engine {
    /// The place containing a reference, from its cell (and position for worldspace-persistent refs).
    pub fn place_of_ref(&self, r: FormId, pos: Vec3) -> Option<Place> {
        let cell = self.lo.cell_of_ref(r)?;
        let idx = self.lo.cell(cell)?;
        match idx.world {
            None => Some(Place::Interior(cell)),
            Some(w) => Some(Place::Exterior(w, idx.grid.unwrap_or_else(|| crate::engine::grid_of(pos.truncate())))),
        }
    }

    pub fn place_of_key(&self, key: CellKey) -> Option<Place> {
        match (key, self.location) {
            (CellKey::Interior(c), _) => Some(Place::Interior(c)),
            (CellKey::Exterior(x, y), Location::Exterior { world, .. }) => Some(Place::Exterior(world, (x, y))),
            _ => None,
        }
    }

    pub fn key_of_place(&self, p: Place) -> Option<CellKey> {
        match (p, self.location) {
            (Place::Interior(c), Location::Interior(cur)) if c == cur => Some(CellKey::Interior(c)),
            (Place::Exterior(w, (x, y)), Location::Exterior { world, .. }) if w == world => {
                let k = CellKey::Exterior(x, y);
                self.cells.contains_key(&k).then_some(k)
            }
            _ => None,
        }
    }

    fn npc_packages_cached(&mut self, npc: FormId) -> std::sync::Arc<Vec<Package>> {
        if let Some(p) = self.whereabouts.packages.get(&npc) {
            return p.clone();
        }
        let p = std::sync::Arc::new(super::package::npc_packages(&self.lo, npc));
        self.whereabouts.packages.insert(npc, p.clone());
        p
    }

    /// Where the current package of a persistent actor puts it.
    fn scheduled_place(&self, achr: FormId, editor: (Place, Vec3), packages: &[Package]) -> (Place, Vec3) {
        let ctx = crate::condition::Context { subject: Some(achr), target: None, quest: None };
        let Some(p) = packages
            .iter()
            .find(|p| p.schedule.matches(self.hour, self.day) && crate::condition::evaluate(self, &p.conditions, ctx))
        else {
            return editor;
        };
        let target = match p.location.map(|l| l.kind) {
            Some(LocationKind::NearReference(r)) => Some(r),
            Some(LocationKind::NearLinkedRef(kw)) => self.linked_ref(achr, (!kw.is_null()).then_some(kw)),
            Some(LocationKind::InCell(c)) => {
                return match self.lo.cell(c) {
                    Some(idx) if idx.world.is_none() => (Place::Interior(c), Vec3::NAN),
                    _ => editor,
                };
            }
            _ => None,
        };
        let Some(t) = target else { return editor };
        let Some(rec) = self.lo.get(t) else { return editor };
        let pos = records::reference(&rec).position;
        match self.place_of_ref(t, pos) {
            Some(place) => (place, pos),
            None => editor,
        }
    }

    /// Recompute where every persistent actor should be, all at once.
    pub fn refresh_whereabouts(&mut self) {
        self.whereabouts.cursor = 0;
        while !self.refresh_whereabouts_step(usize::MAX) {}
    }

    /// Recompute up to `budget` actors; returns true when a full pass completed
    /// and the new table took effect.
    fn refresh_whereabouts_step(&mut self, budget: usize) -> bool {
        if !self.whereabouts.built {
            let mut actors = Vec::new();
            for &id in self.lo.ids_of_type(b"ACHR") {
                if let Some(rec) = self.lo.get(id)
                    && rec.flags() & esp::record_flags::PERSISTENT != 0
                {
                    actors.push(id);
                }
            }
            self.whereabouts.actors = actors;
            self.whereabouts.built = true;
        }
        let actors = std::mem::take(&mut self.whereabouts.actors);
        let start = self.whereabouts.cursor;
        let end = start.saturating_add(budget).min(actors.len());
        let mut pending = std::mem::take(&mut self.whereabouts.pending);
        for &a in &actors[start..end] {
            if self.is_disabled(a) {
                continue;
            }
            let Some(rec) = self.lo.get(a) else { continue };
            let rf = records::reference(&rec);
            if rf.deleted() {
                continue;
            }
            let Some(editor_place) = self.place_of_ref(a, rf.position) else { continue };
            let Some(npc) = self.base_npc(rf.base) else { continue };
            let packages = self.npc_packages_cached(npc);
            let (place, pos) = self.scheduled_place(a, (editor_place, rf.position), &packages);
            if log::log_enabled!(log::Level::Trace) {
                log::trace!("{a} {:?}: editor {editor_place:?}, scheduled {place:?} {pos:?}", self.form_name(npc));
            }
            pending.insert(a, (place, pos));
        }
        let done = end >= actors.len();
        self.whereabouts.actors = actors;
        self.whereabouts.cursor = end;
        if !done {
            self.whereabouts.pending = pending;
            return false;
        }
        let mut by_place: HashMap<Place, Vec<FormId>> = HashMap::new();
        for (&a, &(place, _)) in &pending {
            by_place.entry(place).or_default().push(a);
        }
        let moved = pending.iter().filter(|(a, v)| self.whereabouts.of.get(*a).is_some_and(|o| o.0 != v.0)).count();
        log::debug!("whereabouts of {} actors ({moved} changed place)", pending.len());
        self.whereabouts.prev = std::mem::replace(&mut self.whereabouts.of, pending);
        self.whereabouts.by_place = by_place;
        self.whereabouts.cursor = 0;
        self.whereabouts.next_refresh = REFRESH_INTERVAL;
        true
    }

    fn base_npc(&self, base: FormId) -> Option<FormId> {
        let rec = self.lo.get(base)?;
        match &rec.tag().0 {
            b"NPC_" => Some(base),
            _ => None,
        }
    }

    /// Which actor references to spawn into a freshly loaded cell, with optional positions
    /// overriding the editor location.
    pub fn actors_for_cell(&mut self, key: CellKey, refs: &[FormId]) -> Vec<(FormId, Option<Vec3>)> {
        if !self.whereabouts.built {
            self.refresh_whereabouts();
        }
        let here = self.place_of_key(key);
        let mut out = Vec::new();
        for &r in refs {
            if self.actor_cells.contains_key(&r) || self.lo.tag_of(r).map(|t| t.0) != Some(*b"ACHR") {
                continue;
            }
            match self.whereabouts.of.get(&r) {
                Some((p, _)) if Some(*p) != here => {}
                Some((_, pos)) => out.push((r, Some(*pos))),
                None => out.push((r, None)),
            }
        }
        if let Some(here) = here {
            for &r in self.whereabouts.by_place.get(&here).map(Vec::as_slice).unwrap_or(&[]) {
                if !self.actor_cells.contains_key(&r) && !out.iter().any(|(o, _)| *o == r) {
                    out.push((r, self.whereabouts.of.get(&r).map(|v| v.1)));
                }
            }
        }
        out
    }

    /// The spot just inside the current cell for a load door (its partner's teleport target).
    fn door_inside_spot(&self, door: &crate::world::cell::Door) -> Option<Vec3> {
        let (dest, _, _) = door.destination?;
        let rec = self.lo.get(dest)?;
        let d = rec.get(b"XTEL")?;
        if d.len() < 16 {
            return None;
        }
        let f = |o: usize| f32::from_le_bytes(d[o..o + 4].try_into().unwrap());
        Some(Vec3::new(f(4), f(8), f(12)))
    }

    /// The place a load door leads to.
    fn door_place(&self, door: &crate::world::cell::Door) -> Option<Place> {
        let (dest, pos, _) = door.destination?;
        self.place_of_ref(dest, pos)
    }

    /// The load door nearest `from` (in any loaded cell) leading to `target`, or failing
    /// that to `target`'s worldspace. Returns the door and the spot in front of it.
    fn door_towards(&self, from: Vec3, target: Place) -> Option<(FormId, Vec3)> {
        let mut best: Option<(bool, f32, FormId, Vec3)> = None;
        for rt in self.cells.values() {
            for d in &rt.doors {
                let Some(p) = self.door_place(d) else { continue };
                let exact = p == target;
                let same_world = matches!((p, target), (Place::Exterior(w, _), Place::Exterior(tw, _)) if w == tw);
                if !exact && !same_world {
                    continue;
                }
                let Some(spot) = self.door_inside_spot(d) else { continue };
                let dist = spot.distance(from);
                if best.is_none_or(|b| (exact, -dist) > (b.0, -b.1)) {
                    best = Some((exact, dist, d.ref_id, spot));
                }
            }
        }
        best.map(|b| (b.2, b.3))
    }

    /// Move loaded actors whose schedule changed: walk them out through doors, bring others in.
    pub(crate) fn update_whereabouts(&mut self, dt: f32) {
        self.whereabouts.next_refresh -= dt;
        if self.whereabouts.next_refresh > 0.0 || !self.ai_enabled {
            return;
        }
        if !self.refresh_whereabouts_step(REFRESH_BUDGET) {
            return;
        }
        let player = self.ref_position(crate::engine::PLAYER_REF).unwrap_or_default();
        // Leaving.
        let mut despawn = Vec::new();
        let mut exits = Vec::new();
        for (&key, rt) in &self.cells {
            let Some(here) = self.place_of_key(key) else { continue };
            for a in &rt.actors {
                let Some(&(place, _)) = self.whereabouts.of.get(&a.ref_id) else { continue };
                if place == here || self.key_of_place(place).is_some() || a.exiting.is_some() {
                    continue;
                }
                match self.door_towards(a.pos, place) {
                    Some((door, spot)) => exits.push((key, a.ref_id, door, spot)),
                    None if a.pos.distance(player) > UNSEEN_DISTANCE => despawn.push(a.ref_id),
                    None => {}
                }
            }
        }
        for (key, r, door, spot) in exits {
            if let Some(a) = self.cells.get_mut(&key).and_then(|rt| rt.actors.iter_mut().find(|a| a.ref_id == r)) {
                log::debug!("{r} leaving through door {door} at {spot:?} ({:.0} away)", spot.distance(a.pos));
                a.exiting = Some(door);
                a.goal = Some(super::Goal { behaviour: super::package::Behaviour::Travel, centre: spot, radius: 0.0 });
                a.halt(0.0);
            }
        }
        for r in despawn {
            self.despawn_actor(r);
        }
        // Arriving.
        let loaded: Vec<CellKey> = self.cells.keys().copied().collect();
        for key in loaded {
            let Some(here) = self.place_of_key(key) else { continue };
            let arrivals: Vec<FormId> = self
                .whereabouts
                .by_place
                .get(&here)
                .map(|v| v.iter().copied().filter(|r| !self.actor_cells.contains_key(r)).collect())
                .unwrap_or_default();
            let mut spawn = Vec::new();
            for r in arrivals {
                let Some(&(_, pos)) = self.whereabouts.of.get(&r) else { continue };
                let from = self.whereabouts.prev.get(&r).map(|p| p.0).filter(|p| *p != here);
                // Enter through the door from where they were, else appear if unseen.
                let near = if pos.is_nan() { player } else { pos };
                let door = from.and_then(|f| self.door_towards(near, f));
                match door {
                    Some((_, spot)) => spawn.push((r, Some(spot))),
                    None if pos.is_nan() || pos.distance(player) > UNSEEN_DISTANCE => spawn.push((r, Some(pos))),
                    None => {}
                }
            }
            if !spawn.is_empty() {
                log::debug!("{} actors arriving in {key:?}", spawn.len());
                self.spawn_actors(key, &spawn);
            }
        }
    }

    /// Remove a spawned actor from the scene.
    pub fn despawn_actor(&mut self, r: FormId) {
        let Some(key) = self.actor_cells.remove(&r) else { return };
        self.moved_refs.remove(&r);
        let (Some(rt), Some(rc)) = (self.cells.get_mut(&key), self.scene.cells.get_mut(&key)) else { return };
        let Some(i) = rt.actors.iter().position(|a| a.ref_id == r) else { return };
        let a = rt.actors.remove(i);
        rc.actors.remove(i);
        if let Some(c) = a.capsule {
            rt.colliders.retain(|h| *h != c);
            self.physics.remove_colliders(&[c]);
        }
        log::debug!("{r} despawned from {key:?}");
    }
}
