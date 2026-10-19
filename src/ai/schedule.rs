//! Where persistent actors are: package-driven placement across cells, and
//! actors leaving / arriving through load doors while the player watches.

use std::collections::HashMap;

use esp::FormId;
use glam::Vec3;

use super::package::{LocationKind, Package};
use crate::engine::{Engine, Location};
use crate::render::CellKey;

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

/// Whether two places share coordinates: the same interior, or the same worldspace.
pub fn same_space(a: Place, b: Place) -> bool {
    match (a, b) {
        (Place::Interior(x), Place::Interior(y)) => x == y,
        (Place::Exterior(x, _), Place::Exterior(y, _)) => x == y,
        _ => false,
    }
}

#[derive(Default)]
pub struct Whereabouts {
    /// Persistent actor references (built once).
    actors: Vec<FormId>,
    /// By the NPC records giving the AI packages and the default package list.
    packages: HashMap<(FormId, FormId), std::sync::Arc<Vec<Package>>>,
    /// By actor and NPC record: the NPC records its AI packages and default
    /// package list come from.
    sources: HashMap<(FormId, FormId), (FormId, FormId)>,
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
    /// The package each persistent actor is running (as of the last refresh), and
    /// the refresh in progress.
    pub package: HashMap<FormId, Option<FormId>>,
    pending_packages: HashMap<FormId, Option<FormId>>,
    /// The package whose start (and end) each actor's home doors last
    /// followed: a loaded actor's as its AI picks it, else the refresh's.
    pub lock_package: HashMap<FormId, Option<FormId>>,
    /// The half hour of game time the last refresh was due for.
    slot: Option<i64>,
}

impl Whereabouts {
    /// Whether game time (in hours) has entered a new half hour since the last
    /// call: packages start on the hour or half hour, so the schedule can change.
    fn crossed_slot(&mut self, hours: f64) -> bool {
        let slot = (hours * 2.0).floor() as i64;
        self.slot.replace(slot).is_some_and(|s| s != slot)
    }

    /// A created actor: one of the persistent actors from now on, at `place`
    /// until the next refresh says otherwise.
    pub fn add_actor(&mut self, r: FormId, place: Place, pos: Vec3) {
        if self.built && !self.actors.contains(&r) {
            self.actors.push(r);
        }
        self.of.insert(r, (place, pos));
        self.by_place.entry(place).or_default().push(r);
    }

    /// A created actor gone for good.
    pub fn remove_actor(&mut self, r: FormId) {
        self.actors.retain(|&a| a != r);
        self.of.remove(&r);
        self.prev.remove(&r);
        for v in self.by_place.values_mut() {
            v.retain(|&a| a != r);
        }
    }
}

impl Engine {
    /// The place containing a reference, from its cell (and position for worldspace-persistent refs).
    pub fn place_of_ref(&self, r: FormId, pos: Vec3) -> Option<Place> {
        if let Some(m) = self.world_state.moved.get(&r) {
            return Some(m.place);
        }
        self.editor_place_of(r, pos)
    }

    /// The place a reference's own cell is (as placed in the editor).
    pub fn editor_place_of(&self, r: FormId, pos: Vec3) -> Option<Place> {
        if let Some(c) = self.created(r) {
            return c.place;
        }
        let cell = self.lo.cell_of_ref(r)?;
        let idx = self.lo.cell(cell)?;
        match idx.world {
            None => Some(Place::Interior(cell)),
            Some(w) => Some(Place::Exterior(
                w,
                idx.grid
                    .unwrap_or_else(|| crate::engine::grid_of(pos.truncate())),
            )),
        }
    }

    pub fn place_of_key(&self, key: CellKey) -> Option<Place> {
        match (key, self.location) {
            (CellKey::Interior(c), _) => Some(Place::Interior(c)),
            (CellKey::Exterior(x, y), Location::Exterior { world, .. }) => {
                Some(Place::Exterior(world, (x, y)))
            }
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

    /// The packages of actor `achr` with NPC record `npc`, by the NPC its AI packages
    /// come from.
    pub(crate) fn npc_packages_cached(
        &mut self,
        achr: FormId,
        npc: FormId,
    ) -> std::sync::Arc<Vec<Package>> {
        let key = match self.whereabouts.sources.get(&(achr, npc)) {
            Some(&k) => k,
            None => {
                let src = crate::world::template::Sources::of_npc(&self.lo, npc, achr.0 as u64);
                let k = (
                    src.of(crate::world::template::AI_PACKAGES),
                    src.of(crate::world::template::DEF_PACK_LIST),
                );
                self.whereabouts.sources.insert((achr, npc), k);
                k
            }
        };
        if let Some(p) = self.whereabouts.packages.get(&key) {
            return p.clone();
        }
        let src = crate::world::template::Sources::of_npc(&self.lo, npc, achr.0 as u64);
        let p = std::sync::Arc::new(super::package::npc_packages(&self.lo, &src));
        self.whereabouts.packages.insert(key, p.clone());
        p
    }

    /// The current package of a persistent actor (index into `packages`).
    fn current_package(&self, achr: FormId, packages: &[Package]) -> Option<usize> {
        packages.iter().position(|p| {
            let ctx = crate::condition::Context {
                subject: Some(achr),
                quest: p.quest,
                ..Default::default()
            };
            p.schedule.matches(self.hour, self.day)
                && crate::condition::evaluate(self, &p.conditions, ctx)
        })
    }

    /// Where a package puts an actor, when its location names a place (a reference,
    /// linked reference or interior cell). The position is NaN for a whole cell.
    pub(crate) fn package_place(&self, achr: FormId, p: &Package) -> Option<(Place, Vec3)> {
        let target = match p.location.map(|l| l.kind) {
            Some(LocationKind::NearReference(r)) => Some(r),
            Some(LocationKind::NearLinkedRef(kw)) => {
                self.linked_ref(achr, (!kw.is_null()).then_some(kw))
            }
            Some(LocationKind::NearAlias(alias)) => p.quest.and_then(|q| self.alias_ref(q, alias)),
            Some(LocationKind::InLocAlias(alias)) => {
                let l = p.quest.and_then(|q| self.alias_ref(q, alias))?;
                if self
                    .editor_location(achr)
                    .is_some_and(|e| self.location_within(e, l))
                {
                    return None;
                }
                self.location_marker(l)
            }
            Some(LocationKind::InCell(c)) => {
                return match self.lo.cell(c) {
                    Some(idx) if idx.world.is_none() => Some((Place::Interior(c), Vec3::NAN)),
                    _ => None,
                };
            }
            _ => None,
        }?;
        let pos = self.ref_position(target)?;
        Some((self.place_of_ref(target, pos)?, pos))
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
            actors.extend(
                self.created_refs
                    .refs
                    .iter()
                    .filter(|(_, c)| c.actor)
                    .map(|(id, _)| *id),
            );
            self.whereabouts.actors = actors;
            self.whereabouts.built = true;
        }
        let actors = std::mem::take(&mut self.whereabouts.actors);
        let start = self.whereabouts.cursor;
        let end = start.saturating_add(budget).min(actors.len());
        let mut pending = std::mem::take(&mut self.whereabouts.pending);
        let mut pending_packages = std::mem::take(&mut self.whereabouts.pending_packages);
        for &a in &actors[start..end] {
            if self.is_disabled(a) {
                continue;
            }
            // The dead lie where they fell.
            if let Some(body) = self.body_of(a) {
                pending.insert(a, body);
                pending_packages.insert(a, None);
                continue;
            }
            let Some(rf) = self.reference_of(a) else {
                continue;
            };
            if rf.deleted() {
                continue;
            }
            let Some(editor_place) = self.place_of_ref(a, rf.position) else {
                continue;
            };
            let Some(npc) = self.base_npc(rf.base) else {
                continue;
            };
            // Those a scene or alias gives first, then the NPC's own (not
            // copied: there are many actors).
            let mut first = self.scene_packages(a);
            first.extend(self.alias_packages(a));
            let own = self.npc_packages_cached(a, npc);
            let current = self
                .current_package(a, &first)
                .map(|i| &first[i])
                .or_else(|| self.current_package(a, &own).map(|i| &own[i]));
            // Without a package place: where scripts moved it, else its editor place.
            let home = self
                .world_state
                .moved
                .get(&a)
                .map_or((editor_place, rf.position), |m| (m.place, m.pos));
            let (place, pos) = current
                .and_then(|p| self.package_place(a, p))
                .unwrap_or(home);
            pending_packages.insert(a, current.map(|p| p.id));
            if log::log_enabled!(log::Level::Trace) {
                log::trace!(
                    "{a} {:?}: editor {editor_place:?}, scheduled {place:?} {pos:?}",
                    self.form_name(npc)
                );
            }
            pending.insert(a, (place, pos));
        }
        let done = end >= actors.len();
        self.whereabouts.actors = actors;
        self.whereabouts.cursor = end;
        if !done {
            self.whereabouts.pending = pending;
            self.whereabouts.pending_packages = pending_packages;
            return false;
        }
        let mut by_place: HashMap<Place, Vec<FormId>> = HashMap::new();
        for (&a, &(place, _)) in &pending {
            by_place.entry(place).or_default().push(a);
        }
        let moved = pending
            .iter()
            .filter(|(a, v)| self.whereabouts.of.get(*a).is_some_and(|o| o.0 != v.0))
            .count();
        log::debug!(
            "whereabouts of {} actors ({moved} changed place)",
            pending.len()
        );
        self.whereabouts.prev = std::mem::replace(&mut self.whereabouts.of, pending);
        self.whereabouts.by_place = by_place;
        self.whereabouts.cursor = 0;
        self.whereabouts.package = pending_packages;
        // Loaded actors' doors follow their AI's own picks (`evaluate_packages`).
        let changed: Vec<_> = self
            .whereabouts
            .package
            .iter()
            .filter(|(a, _)| !self.actor_cells.contains_key(*a))
            .filter(|(a, now)| self.whereabouts.lock_package.get(*a) != Some(*now))
            .map(|(&a, &now)| {
                (
                    a,
                    self.whereabouts.lock_package.get(&a).copied().flatten(),
                    now,
                )
            })
            .collect();
        self.package_door_locks(changed);
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
    pub fn actors_for_cell(
        &mut self,
        key: CellKey,
        refs: &[FormId],
    ) -> Vec<(FormId, Option<Vec3>)> {
        if !self.whereabouts.built {
            self.refresh_whereabouts();
        }
        let here = self.place_of_key(key);
        let mut out = Vec::new();
        for &r in refs {
            if self.actor_cells.contains_key(&r) || self.lo.tag_of(r).map(|t| t.0) != Some(*b"ACHR")
            {
                continue;
            }
            let at = self
                .whereabouts
                .of
                .get(&r)
                .copied()
                .or_else(|| self.body_of(r));
            match at.as_ref() {
                Some((p, _)) if Some(*p) != here => {}
                Some((_, pos)) => out.push((r, Some(*pos))),
                None => out.push((r, None)),
            }
        }
        if let Some(here) = here {
            for &r in self
                .whereabouts
                .by_place
                .get(&here)
                .map(Vec::as_slice)
                .unwrap_or(&[])
            {
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
            for d in rt.doors.iter().filter(|d| !self.is_disabled(d.ref_id)) {
                let Some(p) = self.door_place(d) else {
                    continue;
                };
                let exact = p == target;
                let same_world = matches!((p, target), (Place::Exterior(w, _), Place::Exterior(tw, _)) if w == tw);
                if !exact && !same_world {
                    continue;
                }
                let Some(spot) = self.door_inside_spot(d) else {
                    continue;
                };
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
        // Packages start on the hour or half hour: look then rather than up to
        // a refresh later (unloaded actors' doors lock and unlock with them).
        if self.whereabouts.crossed_slot(self.game_hours_total()) {
            self.whereabouts.next_refresh = 0.0;
        }
        if self.whereabouts.next_refresh > 0.0 || !self.ai_enabled {
            return;
        }
        if !self.refresh_whereabouts_step(REFRESH_BUDGET) {
            return;
        }
        let player = self
            .ref_position(crate::engine::PLAYER_REF)
            .unwrap_or_default();
        // Leaving.
        let mut despawn = Vec::new();
        let mut exits = Vec::new();
        for (&key, rt) in &self.cells {
            let Some(here) = self.place_of_key(key) else {
                continue;
            };
            for a in &rt.actors {
                let Some(&(place, _)) = self.whereabouts.of.get(&a.ref_id) else {
                    continue;
                };
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
            if let Some(a) = self
                .cells
                .get_mut(&key)
                .and_then(|rt| rt.actors.iter_mut().find(|a| a.ref_id == r))
            {
                log::debug!(
                    "{r} leaving through door {door} at {spot:?} ({:.0} away)",
                    spot.distance(a.pos)
                );
                a.exiting = Some(door);
                a.goal = Some(super::Goal::travel(spot));
                a.interrupt(&mut self.furniture);
            }
        }
        for r in despawn {
            self.despawn_actor(r);
        }
        // Arriving.
        let loaded: Vec<CellKey> = self.cells.keys().copied().collect();
        for key in loaded {
            let Some(here) = self.place_of_key(key) else {
                continue;
            };
            let arrivals: Vec<FormId> = self
                .whereabouts
                .by_place
                .get(&here)
                .map(|v| {
                    v.iter()
                        .copied()
                        .filter(|r| !self.actor_cells.contains_key(r))
                        .collect()
                })
                .unwrap_or_default();
            let mut spawn = Vec::new();
            let mut through_door = Vec::new();
            for r in arrivals {
                let Some(&(_, pos)) = self.whereabouts.of.get(&r) else {
                    continue;
                };
                let from = self
                    .whereabouts
                    .prev
                    .get(&r)
                    .map(|p| p.0)
                    .filter(|p| *p != here);
                // Enter through the door from where they were, else appear if unseen.
                let near = if pos.is_nan() { player } else { pos };
                let door = from.and_then(|f| self.door_towards(near, f));
                match door {
                    Some((_, spot)) => {
                        spawn.push((r, Some(spot)));
                        through_door.push(r);
                    }
                    None if pos.is_nan() || pos.distance(player) > UNSEEN_DISTANCE => {
                        spawn.push((r, Some(pos)))
                    }
                    None => {}
                }
            }
            if !spawn.is_empty() {
                log::debug!("{} actors arriving in {key:?}", spawn.len());
                self.spawn_actors(key, &spawn);
                // Arrivals walk in rather than appearing already seated.
                if let Some(rt) = self.cells.get_mut(&key) {
                    for a in rt
                        .actors
                        .iter_mut()
                        .filter(|a| through_door.contains(&a.ref_id))
                    {
                        a.fresh = false;
                    }
                }
            }
        }
    }

    /// Time passed without the world running (the hours waited or slept):
    /// every actor is put where its schedule has it now. Those whose place is
    /// elsewhere go (and those whose place is a loaded cell turn up there);
    /// those whose package changed, or who are away from where it has them,
    /// start over at its location (in its furniture, as a cell loading puts
    /// them). The fallen, the fighting and the player's followers are left be.
    pub(crate) fn jump_to_schedules(&mut self) {
        let start = std::time::Instant::now();
        self.refresh_whereabouts();
        let mut despawn = Vec::new();
        let mut respawn: Vec<(FormId, CellKey, Vec3)> = Vec::new();
        for (&key, rt) in &self.cells {
            let Some(here) = self.place_of_key(key) else {
                continue;
            };
            for a in &rt.actors {
                if a.dead || a.pinned || a.combat.is_some() || a.bleeding.is_some() {
                    continue;
                }
                let follower = a.goal.is_some_and(|g| {
                    matches!(
                        g.behaviour,
                        super::Behaviour::Follow | super::Behaviour::Escort
                    ) && g.target == Some(crate::engine::PLAYER_REF)
                });
                if follower {
                    continue;
                }
                let pick = self.current_package(a.ref_id, &a.packages);
                let scheduled = self.whereabouts.of.get(&a.ref_id).copied();
                // Its place: the schedule's for the persistent, else the
                // package's when it names one.
                let place = match scheduled {
                    Some((p, _)) => p,
                    None => match pick.and_then(|i| self.package_place(a.ref_id, &a.packages[i])) {
                        Some((p, _)) if !same_space(p, here) => continue,
                        _ => here,
                    },
                };
                let to = if same_space(place, here) {
                    self.key_of_place(place)
                } else {
                    None
                };
                let Some(to) = to else {
                    despawn.push(a.ref_id);
                    continue;
                };
                let Some(i) = pick else {
                    if to != key {
                        respawn.push((a.ref_id, to, scheduled.map_or(a.pos, |s| s.1)));
                    }
                    continue;
                };
                let p = &a.packages[i];
                let (centre, radius) = self.package_target(a, p);
                let away = !matches!(
                    p.behaviour,
                    super::Behaviour::Follow
                        | super::Behaviour::Escort
                        | super::Behaviour::Hold
                        | super::Behaviour::ForceGreet
                ) && a.pos.truncate().distance(centre.truncate())
                    > radius.max(96.0) * 1.5;
                // (One not yet given a package keeps to what it does.)
                let changed = a.current.is_some_and(|c| c != i);
                if changed || to != key || away {
                    respawn.push((a.ref_id, to, centre));
                }
            }
        }
        for r in despawn {
            log::debug!("{r} gone where its schedule has it");
            self.despawn_actor(r);
        }
        for &(r, _, _) in &respawn {
            self.despawn_actor(r);
        }
        let mut by_key: HashMap<CellKey, Vec<(FormId, Option<Vec3>)>> = HashMap::new();
        for (r, key, at) in respawn {
            // Outdoors, in the cell it lands in (when loaded).
            let key = match key {
                CellKey::Exterior(..) if at.is_finite() => {
                    let (x, y) = crate::engine::grid_of(at.truncate());
                    let k = CellKey::Exterior(x, y);
                    if self.cells.contains_key(&k) { k } else { key }
                }
                _ => key,
            };
            log::debug!("{r} starts over at {at:?} in {key:?}");
            by_key.entry(key).or_default().push((r, Some(at)));
        }
        for (key, refs) in by_key {
            self.spawn_actors(key, &refs);
        }
        // Those whose place is a loaded cell turn up there.
        let loaded: Vec<CellKey> = self.cells.keys().copied().collect();
        for key in loaded {
            let Some(here) = self.place_of_key(key) else {
                continue;
            };
            let arrivals: Vec<(FormId, Option<Vec3>)> = self
                .whereabouts
                .by_place
                .get(&here)
                .into_iter()
                .flatten()
                .filter(|r| !self.actor_cells.contains_key(r))
                .map(|r| (*r, self.whereabouts.of.get(r).map(|v| v.1)))
                .collect();
            if !arrivals.is_empty() {
                log::debug!("{} actors turn up in {key:?}", arrivals.len());
                self.spawn_actors(key, &arrivals);
            }
        }
        // Their packages taken at once (the world doesn't run in the menu).
        for a in self.cells.values_mut().flat_map(|rt| &mut rt.actors) {
            a.next_eval = 0.0;
        }
        self.evaluate_packages(0.0);
        log::debug!("schedules jumped to in {:?}", start.elapsed());
    }

    /// Remove a spawned actor from the scene.
    pub fn despawn_actor(&mut self, r: FormId) {
        let Some(key) = self.actor_cells.remove(&r) else {
            return;
        };
        self.moved_refs.remove(&r);
        self.furniture.release(r);
        let (Some(rt), Some(rc)) = (self.cells.get_mut(&key), self.scene.cells.get_mut(&key))
        else {
            return;
        };
        let Some(i) = rt.actors.iter().position(|a| a.ref_id == r) else {
            return;
        };
        let a = rt.actors.remove(i);
        rc.actors.remove(i);
        // It comes back as hurt as it left (less what it heals meanwhile).
        if !a.dead {
            let w = crate::world_state::Wounds::of(&a, self.scripts.real_time);
            match w {
                Some(w) => self.world_state.wounds.insert(r, w),
                None => self.world_state.wounds.remove(&r),
            };
        }
        if let Some(c) = a.capsule {
            rt.colliders.retain(|h| *h != c);
            self.physics.remove_colliders(&[c]);
        }
        self.put_torch_away_for_now(&a);
        log::debug!("{r} despawned from {key:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_comes_due_each_half_hour() {
        let mut w = Whereabouts::default();
        // The first reading only sets the clock.
        assert!(!w.crossed_slot(7.2));
        assert!(!w.crossed_slot(7.45));
        assert!(w.crossed_slot(7.5));
        assert!(!w.crossed_slot(7.9));
        assert!(w.crossed_slot(8.0));
        // Midnight, and a day on.
        assert!(w.crossed_slot(24.0 * 3.0 + 0.1));
    }
}
