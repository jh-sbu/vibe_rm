//! References made while playing: quest aliases that create their reference
//! (`ALCO`, at or in another alias's reference) and Papyrus `PlaceAtMe` /
//! `PlaceActorAtMe`. Their form ids count up from `FF000800` as the game's do.
//! Actors join the persistent actors' whereabouts, so they appear where they
//! were made and follow their packages; items made in a container go to its
//! inventory. Items and objects made in the world aren't drawn yet.

use std::collections::HashMap;

use esp::FormId;
use glam::Vec3;

use crate::ai::schedule::Place;
use crate::engine::{Engine, Location, PLAYER_REF};
use crate::world::records;

const FIRST: u32 = 0xFF00_0800;

#[derive(Debug, Clone)]
pub struct Created {
    pub base: FormId,
    pub position: Vec3,
    pub rotation: Vec3,
    /// Where it was made (none inside a container).
    pub place: Option<Place>,
    /// The container it was made in.
    pub container: Option<FormId>,
    /// The location of the reference it was made at.
    pub location: Option<FormId>,
    pub actor: bool,
}

#[derive(Default)]
pub struct CreatedRefs {
    pub refs: HashMap<FormId, Created>,
    next: u32,
}

impl Engine {
    pub fn created(&self, r: FormId) -> Option<&Created> {
        self.created_refs.refs.get(&r)
    }

    /// A created reference as a placed one would read.
    pub fn created_reference(&self, r: FormId) -> Option<records::Reference> {
        let c = self.created(r)?;
        Some(records::Reference {
            id: r,
            base: c.base,
            position: c.position,
            rotation: c.rotation,
            scale: 1.0,
            flags: esp::record_flags::PERSISTENT,
            radius_override: None,
            teleport: None,
            enable_parent: None,
            lock: None,
        })
    }

    /// A placed or created reference.
    pub fn reference_of(&self, r: FormId) -> Option<records::Reference> {
        match self.lo.get(r) {
            Some(rec) => Some(records::reference(&rec)),
            None => self.created_reference(r),
        }
    }

    /// The place a reference is in now: the player's, a loaded actor's cell, else
    /// its own cell's.
    pub fn current_place_of(&self, r: FormId) -> Option<(Place, Vec3)> {
        let pos = self.ref_position(r)?;
        if r == PLAYER_REF {
            return match self.location {
                Location::Interior(c) => Some((Place::Interior(c), pos)),
                Location::Exterior { world, .. } => Some((Place::Exterior(world, crate::engine::grid_of(pos.truncate())), pos)),
                _ => None,
            };
        }
        if let Some(&key) = self.actor_cells.get(&r) {
            return Some((self.place_of_key(key)?, pos));
        }
        Some((self.place_of_ref(r, pos)?, pos))
    }

    /// Make a reference to `base` at reference `at`, or in its inventory.
    pub fn create_ref(&mut self, base: FormId, at: FormId, inside: bool) -> Option<FormId> {
        let tag = self.lo.tag_of(base)?.0;
        let actor = matches!(&tag, b"NPC_" | b"LVLN");
        // Locations: at their marker.
        let at = if self.is_location(at) { self.location_marker(at)? } else { at };
        if self.created_refs.next == 0 {
            self.created_refs.next = FIRST;
        }
        let id = FormId(self.created_refs.next);
        self.created_refs.next += 1;
        let location = self.ref_current_location(at);
        if inside && !actor {
            let item = if &tag == b"LVLI" { crate::world::actor::resolve_leveled(&self.lo, base, b"LVLI", id.0 as u64)? } else { base };
            self.inventory_mut(at).add(item, 1);
            let c = Created { base: item, position: Vec3::ZERO, rotation: Vec3::ZERO, place: None, container: Some(at), location, actor };
            self.created_refs.refs.insert(id, c);
            log::debug!("created {id} ({base}) in {at}");
            return Some(id);
        }
        let (place, position) = self.current_place_of(at)?;
        let rotation = self.reference_of(at).map(|r| r.rotation).unwrap_or_default();
        self.created_refs.refs.insert(id, Created { base, position, rotation, place: Some(place), container: None, location, actor });
        log::debug!("created {id} ({base}) at {at} {place:?} {position:?}");
        if actor {
            self.add_created_actor(id, place, position);
        }
        Some(id)
    }

    /// A created actor joins the persistent actors, spawned now if its place is loaded.
    fn add_created_actor(&mut self, id: FormId, place: Place, position: Vec3) {
        self.whereabouts.add_actor(id, place, position);
        if let Some(key) = self.key_of_place(place) {
            self.spawn_actors(key, &[(id, Some(position))]);
        }
    }
}
