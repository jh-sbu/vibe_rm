//! What happens to references stays when their cells unload and load again (in
//! memory; what saves will write out): dead actors stay dead, lying where they
//! fell, in the pose they lay in; the hurt come back as hurt as they left, less
//! what they would have healed meanwhile; doors the player opened stay open, and
//! so do doors actors left open when their cell unloaded (they close once clear);
//! objects' behaviour graphs carry on in the state they were left in.
//! Taken items, locks, enable state and inventories are kept elsewhere
//! (`ScriptState`, `Engine::inventories`).

use std::collections::HashMap;

use esp::FormId;
use glam::{Mat4, Vec3};

use crate::ai::schedule::Place;
use crate::engine::Engine;

#[derive(Default)]
pub struct WorldState {
    pub(crate) pending_knocks: HashMap<FormId, crate::save::knock::SavedKnock>,
    /// Explicit Papyrus `SetAlert` flags, independent of loaded actor runtimes.
    /// Automatic detection-state changes are not mapped to this flag yet.
    pub alerted: std::collections::HashSet<FormId>,
    /// Papyrus `SetUnconscious` flags (`GetUnconscious`). Only the flag: the
    /// state's effect on the actor isn't modelled (saved in `UNCN`).
    pub unconscious: std::collections::HashSet<FormId>,
    /// Papyrus `SetNoBleedoutRecovery(true)`: actors kept bleeding out
    /// until it is cleared. Not saved yet.
    pub no_bleedout_recovery: std::collections::HashSet<FormId>,
    /// Actors the player bribed or intimidated (`SetBribed`,
    /// `SetIntimidated`; saved in `BRIB` / `INTI`).
    pub bribed: std::collections::HashSet<FormId>,
    pub intimidated: std::collections::HashSet<FormId>,
    /// `SetContainerAllowStolenItems`: containers told whether stolen items
    /// show in their menu (kept, with no effect yet; not saved).
    pub container_stolen: HashMap<FormId, bool>,
    /// `SetPlayerControls(true)`: NPCs put under the player's controls (not
    /// saved).
    pub player_controlled: std::collections::HashSet<FormId>,
    /// `SetHeadTracking(false)`: actors that don't turn their heads to look
    /// (not saved).
    pub no_head_tracking: std::collections::HashSet<FormId>,
    /// `SetDontMove(true)`: actors held where they stand and not pushed
    /// about (CK wiki). They turn, idle and fight in place, and scripts can
    /// still move them (`known_ambiguities/actor-holds.md`; not saved).
    pub dont_move: std::collections::HashSet<FormId>,
    /// `SetRestrained(true)` (`GetRestrained`): actors that don't move from
    /// where they are but still pick packages, detect and talk; script
    /// movement such as `KeepOffsetFromActor` still moves them, and the flag
    /// clears `SetUnconscious`'s and the other way round (CK wiki; not
    /// saved).
    pub restrained: std::collections::HashSet<FormId>,
    /// Actors in `StartDeferredKill`'s state: they don't die until
    /// `EndDeferredKill`; the value is who dealt a killing blow meanwhile
    /// (saved in `DFKL`).
    pub deferred_kills: HashMap<FormId, Option<FormId>>,
    /// `SetActorCause`: the actor to blame for what a trap or other object
    /// does (saved in `ACAU`).
    pub actor_causes: HashMap<FormId, FormId>,
    /// `AllowPCDialogue` overrides of the race's flag (saved in `PCDL`).
    pub pc_dialogue: HashMap<FormId, bool>,
    /// `AllowBleedoutDialogue(true)`: talkable while bleeding out (saved in
    /// `BLDL`).
    pub bleedout_dialogue: std::collections::HashSet<FormId>,
    /// References on their way by `TranslateTo` and its kin (not saved:
    /// a load leaves them where they were).
    pub translations: std::collections::HashMap<FormId, crate::translation::Translation>,
    /// Ash piles made by `AttachAshPile`, and whose body each holds:
    /// activating one searches the body (not saved yet).
    pub ash_piles: std::collections::HashMap<FormId, FormId>,
    /// Dead actors and where their bodies lie.
    pub dead: HashMap<FormId, (Place, Vec3)>,
    /// Actors killed in play (`kill_actor_by`): who dealt the killing blow and
    /// when (saved in `DETH`; older saves load without them).
    pub deaths: HashMap<FormId, Death>,
    /// Deaths in play by actor base, never lowered (`GetDeadCount`; saved in
    /// `DCNT`).
    pub dead_counts: HashMap<FormId, u32>,
    /// Animated doors the player left open.
    pub open_doors: std::collections::HashSet<FormId>,
    /// Animated doors an actor opened and was still holding open when its cell
    /// unloaded: they load open and close again once nobody is near.
    pub actor_open_doors: std::collections::HashSet<FormId>,
    /// References `SetDestroyed(true)` flagged, their stage untouched (saved
    /// in `DSFL`).
    pub destroyed_flags: std::collections::HashSet<FormId>,
    /// Dead actors' ragdoll bodies as they lay (world transforms).
    pub body_poses: HashMap<FormId, Vec<Mat4>>,
    /// References scripts moved (`MoveTo`, `SetPosition`, `SetAngle`): where to.
    pub moved: HashMap<FormId, Moved>,
    /// Dropped weapon references' spent charge, one entry per depleted copy
    /// in the reference's stack. Unlisted copies are full. This remains here
    /// while cells unload and moves back to inventory state on pickup.
    pub item_charges: HashMap<FormId, Vec<f32>>,
    /// Loose objects' breakable joints that broke (by index in the model).
    pub broken_joints: HashMap<FormId, Vec<usize>>,
    /// Destructible objects' health and current stage, by reference. Missing
    /// entries are untouched; damage initializes health from the base `DEST`.
    pub destruction: HashMap<FormId, crate::world::destruction::DestructionState>,
    /// Objects' behaviour graphs as their cells unloaded (a lever pulled, a
    /// portcullis raised, a rockfall come down), to run on from there.
    pub object_graphs: HashMap<FormId, crate::world::behavior::GraphAnim>,
    /// Where objects' graphs stand opening and closing (`GetOpenState`), from
    /// the `Open` / `Close` events sent them and the `Opening` / `Closing`
    /// they raise. Saved (`GOPN`) when `object_graphs` isn't: graphs built
    /// afresh after a load replay them.
    pub graph_open: HashMap<FormId, crate::world::animated::OpenState>,
    /// The player's learned ingredient effects: (ingredient, index in its list).
    pub learned_effects: std::collections::HashSet<(FormId, usize)>,
}

/// How an actor died in play: the killer (none when no one is known to have
/// dealt the blow), and the game hour (`Engine::game_hours_total`) it died.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Death {
    pub killer: Option<FormId>,
    pub at: f64,
}

impl WorldState {
    /// Whether an animated door was left open: `Some(false)` for one the
    /// player opened (it stays open), `Some(true)` for one an actor opened
    /// (it closes again once clear), `None` for a closed door.
    pub fn door_left_open(&self, door: FormId) -> Option<bool> {
        if self.open_doors.contains(&door) {
            Some(false)
        } else if self.actor_open_doors.contains(&door) {
            Some(true)
        } else {
            None
        }
    }
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
    /// How hurt a living actor leaves (none when well).
    pub(crate) fn of(a: &crate::ai::ActorRuntime, now: f64) -> Option<Wounds> {
        (a.exec.health < a.stats.max_health - 0.5).then(|| Wounds {
            health: a.exec.health,
            max: a.stats.max_health,
            rate: a.stats.health_regen / 100.0,
            at: now,
        })
    }

    /// Health now, having healed since.
    pub fn health_at(&self, now: f64) -> f32 {
        (self.health + self.max * self.rate * (now - self.at).max(0.0) as f32).min(self.max)
    }
}

/// Loaded actor `a`'s execution as it leaves `place` at real and game time
/// `clocks`: at the stage it reached, unless it was on its way out.
pub(crate) fn leaving(
    a: &crate::ai::ActorRuntime,
    place: Place,
    clocks: (f64, f64),
) -> crate::ai::exec::ActorExec {
    let stage = a.stage().filter(|_| a.exiting.is_none());
    let mut exec = a
        .exec
        .clone()
        .away(Some(place), Wounds::of(a, clocks.0), stage, clocks);
    if let Some(run) = exec.run.as_mut() {
        let seed = a.ref_id.0 as u64 ^ run.started_at.to_bits();
        let behaviour = a
            .exec
            .current
            .and_then(|i| a.exec.packages.get(i))
            .map(|p| a.exec.effective(p).behaviour)
            .or_else(|| a.exec.goal.map(|g| g.behaviour));
        match behaviour {
            Some(crate::ai::package::Behaviour::UseWeapon) => {
                if let Some(p) = &a.practice {
                    run.practice = Some(p.away_progress(seed));
                }
            }
            Some(crate::ai::package::Behaviour::UseMagic) => {
                if let Some(p) = &a.magic_practice {
                    run.practice = Some(p.away_progress(seed));
                }
            }
            _ => {}
        }
    }
    exec
}

impl Engine {
    /// An actor died: remember it, and where (until its body settles elsewhere).
    pub(crate) fn remember_dead(&mut self, actor: FormId) {
        if let Some(at) = self.current_place_of(actor) {
            self.world_state.dead.insert(actor, at);
        }
    }

    /// Before a cell unloads: where its dead actors' bodies came to rest (and
    /// how), and the living's executions (how hurt they are among them).
    pub(crate) fn remember_actors(&mut self, key: crate::render::CellKey) {
        let knocked: Vec<_> = self
            .cells
            .get(&key)
            .into_iter()
            .flat_map(|r| &r.actors)
            .filter(|a| a.knock.is_some())
            .map(|a| a.ref_id)
            .collect();
        for actor in knocked {
            self.finish_knock(actor);
        }
        let Some(place) = self.place_of_key(key) else {
            return;
        };
        let Some(rt) = self.cells.get(&key) else {
            return;
        };
        let clocks = (self.scripts.real_time, self.game_hours_total());
        let mut rest = Vec::new();
        let mut away = Vec::new();
        for a in &rt.actors {
            if !a.dead {
                away.push((a.ref_id, leaving(a, place, clocks)));
                continue;
            }
            let bodies = a
                .ragdoll
                .as_ref()
                .map(|(rd, _)| self.physics.ragdoll_bodies(rd));
            let mut pos = bodies
                .as_ref()
                .and_then(|b| b.first())
                .map_or(a.exec.pos, |m| m.w_axis.truncate());
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
        // Where they are until their packages move them on (`step_away`).
        for (r, x) in &away {
            self.whereabouts.put(*r, place, x.pos);
            self.whereabouts.placement_heading.remove(r);
        }
        self.away.extend(away);
    }

    /// Hurt actors that unloaded, and how hurt.
    #[cfg(test)]
    pub(crate) fn away_wounds(&self) -> HashMap<FormId, Wounds> {
        self.away
            .iter()
            .filter_map(|(&r, x)| Some((r, x.wounds?)))
            .collect()
    }

    /// What saves keep of actors' executions (`AEXC`) and wounds (`WRLD`):
    /// those away, and loaded ones' as they would be were their cells to
    /// unload now, so that they load back where and as they were. Executions
    /// that are only wounds aren't listed.
    pub(crate) fn saved_executions(
        &self,
    ) -> (
        HashMap<FormId, crate::ai::exec::SavedExec>,
        HashMap<FormId, Wounds>,
    ) {
        let clocks = (self.scripts.real_time, self.game_hours_total());
        let mut loaded = Vec::new();
        for (key, rt) in &self.cells {
            let Some(place) = self.place_of_key(*key) else {
                continue;
            };
            for a in rt.actors.iter().filter(|a| !a.dead) {
                loaded.push((a.ref_id, leaving(a, place, clocks)));
            }
        }
        let all = loaded.iter().map(|(r, x)| (*r, x)).chain(
            self.away
                .iter()
                .filter(|(r, _)| !self.actor_cells.contains_key(r))
                .map(|(&r, x)| (r, x)),
        );
        let (mut executions, mut wounds) = (HashMap::new(), HashMap::new());
        for (r, x) in all {
            if let Some(w) = x.wounds {
                wounds.insert(r, w);
            }
            if x.place.is_some() || x.run.is_some() {
                executions.insert(r, x.saved());
            }
        }
        (executions, wounds)
    }

    /// A save's executions, over the wounds `restore_away_wounds` put back:
    /// every one is away until its cell loads and attaches it, and is where
    /// it is until its packages move it on (`rebuild_whereabouts` reads them).
    pub(crate) fn restore_executions(
        &mut self,
        saved: HashMap<FormId, crate::ai::exec::SavedExec>,
    ) {
        for (r, s) in saved {
            if self.world_state.dead.contains_key(&r) {
                continue;
            }
            let wounds = self.away.remove(&r).and_then(|x| x.wounds);
            let x = crate::ai::exec::ActorExec::from_saved(s, wounds);
            self.away.insert(r, x);
        }
    }

    /// A save's hurt actors away: they are all that is away.
    pub(crate) fn restore_away_wounds(&mut self, wounds: HashMap<FormId, Wounds>) {
        self.away_steps = Default::default();
        self.away = wounds
            .into_iter()
            .map(|(r, w)| (r, crate::ai::exec::ActorExec::wounded(w)))
            .collect();
    }

    /// An actor away is well again (resurrected, reset).
    pub(crate) fn heal_wounds_away(&mut self, actor: FormId) {
        if let Some(x) = self.away.get_mut(&actor) {
            x.wounds = None;
        }
    }

    /// Time passed without running (hours waited): the hurt actors away heal
    /// for `secs` more (`Engine::pass_time`).
    pub(crate) fn heal_away(&mut self, secs: f32) {
        for w in self.away.values_mut().filter_map(|x| x.wounds.as_mut()) {
            w.at -= secs as f64;
        }
    }

    /// The health a loading actor comes back with (its most, unless it left
    /// hurt), its execution away taken back. One bleeding out away (or with
    /// no health left: saves don't keep bleeding) gets up as it loads, with a
    /// quarter of its health, as at the end of a bleedout.
    pub(crate) fn returning_health(&mut self, actor: FormId, max: f32) -> f32 {
        let now = self.scripts.real_time;
        let down = |x: &crate::ai::exec::ActorExec| {
            x.bleeding.is_some() || x.wounds.is_some_and(|w| w.health <= 0.0)
        };
        match self.away.remove(&actor) {
            Some(x) if down(&x) => max * 0.25,
            Some(x) => x.wounds.map_or(max, |w| w.health_at(now).min(max)),
            None => max,
        }
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
            return Vec3::new(0.0, 0.0, a.exec.heading);
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

    /// Whether a loaded actor is held where it stands this frame:
    /// `SetDontMove`, or `SetRestrained` unless a script's
    /// `KeepOffsetFromActor` moves it (CK wiki).
    pub(crate) fn held_in_place(&self, r: FormId) -> bool {
        self.world_state.dont_move.contains(&r)
            || (self.world_state.restrained.contains(&r)
                && !self.script_moves.keep_offsets.contains_key(&r))
    }

    /// Whether the player is held by `SetDontMove` or `SetRestrained`: no
    /// moving, switching camera or turning it, the HUD as it was (CK wiki).
    pub(crate) fn player_held(&self) -> bool {
        let p = crate::engine::PLAYER_REF;
        self.world_state.dont_move.contains(&p) || self.world_state.restrained.contains(&p)
    }

    /// Move a reference (scripts' `MoveTo`, `SetPosition`, `SetAngle`): to `pos`
    /// with rotation `rot`, in the place of `near` (the reference moved to, or
    /// itself). Remembered across cell loads.
    pub fn move_ref(&mut self, r: FormId, near: FormId, pos: Vec3, rot: Vec3) -> bool {
        if r == crate::engine::PLAYER_REF {
            self.queue_player_move(near, pos, rot.z);
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

    /// `MoveToPackageLocation`: to where its running package puts it (the
    /// procedure's destination, or the package's location or target). A
    /// location that is a whole cell, with no point in it, moves nothing.
    pub fn move_to_package_location(&mut self, r: FormId) -> bool {
        let Some(p) = self
            .running_package(r)
            .and_then(|id| crate::ai::package::parse(&self.lo, id))
        else {
            return false;
        };
        let Some((place, pos)) = self.package_place(r, &p).filter(|(_, pos)| pos.is_finite())
        else {
            return false;
        };
        let rot = self.ref_rotation(r);
        self.move_ref_to(r, place, pos, rot)
    }

    /// `MoveToNode(akTarget, asNodeName)`: to where the node of the target's
    /// model is, turned as it is (the weapon racks hang a weapon on its
    /// pivot). The node's rest transform in the model counts, not an
    /// animated one; an actor's nodes and a missing node move nothing.
    pub fn move_to_node(&mut self, r: FormId, target: FormId, node: &str) -> bool {
        let Some((at, local)) = self.ref_node_transform(target, node) else {
            return false;
        };
        let (_, rot, pos) = (at * local).to_scale_rotation_translation();
        self.move_ref(r, target, pos, crate::loose::euler_of(rot))
    }

    /// A placed object's transform and its model's node `node` in it (the
    /// rest transform; case ignored). None for actors and missing nodes.
    pub(crate) fn ref_node_transform(&self, target: FormId, node: &str) -> Option<(Mat4, Mat4)> {
        if self.actor_ref(target).is_some() || target == crate::engine::PLAYER_REF {
            return None;
        }
        let at = self.ref_transform(target)?;
        let Some(model) = self
            .base_of(target)
            .and_then(|b| self.lo.get(b))
            .and_then(|rec| rec.get(b"MODL").map(esp::decode_zstring))
            .filter(|m| !m.is_empty())
            .map(|m| crate::world::records::mesh_path(&m))
        else {
            return None;
        };
        let Some(local) = self
            .vfs
            .read(&model)
            .and_then(|b| nif::Nif::parse(&b).ok())
            .and_then(|nif| {
                crate::render::model::node_transforms(&nif)
                    .into_iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case(node))
                    .map(|(_, m)| m)
            })
        else {
            log::debug!("{target} has no node {node}");
            return None;
        };
        Some((at, local))
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
        // Not where it left any more: it carries on from where it was moved
        // to, its package taking it on from there.
        if let Some(x) = self.away.get_mut(&r).filter(|x| x.place.is_some()) {
            x.stage = None;
            x.settled = false;
            x.ends_at = None;
            x.jump_at = None;
            x.on_arrival = None;
            (x.place, x.pos, x.heading) = (Some(place), pos, rot.z);
        }
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
                    a.turn_after_arrival = None;
                    a.exec.pos = pos;
                    a.exec.heading = rot.z;
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
        let holder = self
            .cells
            .iter()
            .find(|(_, rt)| rt.refs.contains(&r))
            .map(|(&k, _)| k);
        let target = self.key_of_place(place);
        if holder.is_some() && holder == target {
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
            return;
        }
        // Into another cell: out of the one holding it, if loaded, and into
        // the new one, if loaded, as that cell's load would place it; it
        // then loads and unloads with its new cell.
        if let Some(h) = holder {
            self.take_out_of_cell(h, r);
        }
        if let Some(t) = target {
            self.put_in_cell(t, r);
        }
    }

    /// The parts (drawn object, light, door) of a plugin reference moved
    /// into `place`, where and as it was moved to.
    pub(crate) fn moved_reference_parts(
        &self,
        place: Place,
        r: FormId,
        objects: &mut Vec<crate::world::cell::PlacedObject>,
        lights: &mut Vec<crate::world::cell::PointLight>,
        doors: &mut Vec<crate::world::cell::Door>,
    ) {
        let Some(m) = self.world_state.moved.get(&r).filter(|m| m.place == place) else {
            return;
        };
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
        let moved: Vec<FormId> = self
            .world_state
            .moved
            .iter()
            .filter(|(_, m)| m.place == place)
            .map(|(&r, _)| r)
            .filter(|&r| {
                self.created(r).is_none() && self.lo.tag_of(r).is_none_or(|t| t.0 != *b"ACHR")
            })
            .collect();
        for r in moved {
            objects.retain(|o| o.ref_id != r);
            lights.retain(|l| l.ref_id != r);
            doors.retain(|d| d.ref_id != r);
            self.moved_reference_parts(place, r, objects, lights, doors);
        }
    }
}

#[cfg(test)]
mod moved_ref_tests {
    use super::*;
    use crate::engine::PLAYER_REF;
    use crate::render::CellKey;

    const BREEZEHOME: FormId = FormId(0x165A8);
    /// A cast iron pot (a loose object) in the Bannered Mare.
    const POT: FormId = FormId(0x6170C);
    /// A candle light in the Bannered Mare.
    const CANDLE: FormId = FormId(0xC9037);

    fn drawn(e: &Engine, key: CellKey, r: FormId) -> usize {
        e.scene.cells[&key]
            .instances
            .iter()
            .filter(|i| i.ref_id == r.0 && !i.hidden)
            .count()
    }

    /// Moved out of an unloaded cell into the loaded one, a reference is
    /// there at once, with its body and light; moved away again, it is gone.
    #[test]
    #[ignore = "requires installed vanilla assets and a headless GPU adapter"]
    fn references_moved_out_of_unloaded_cells_appear_at_once() {
        let data = vfs::locate_data_dir().unwrap();
        let names: Vec<_> = esp::BASE_MASTERS.iter().map(|s| s.to_string()).collect();
        let mut e = crate::stats::action_tests::headless_engine_plugins(&data, &names);
        e.vfs = vfs::Vfs::new(&data, &names).into();
        let here = e.lo.find_editor_id("WhiterunBreezehome").unwrap();
        assert_eq!(here, BREEZEHOME);
        e.enter_interior(here, None).unwrap();
        let key = CellKey::Interior(here);
        assert_eq!(drawn(&e, key, POT), 0);
        let at = e.ref_position(PLAYER_REF).unwrap();
        assert!(e.move_ref(POT, PLAYER_REF, at + Vec3::X * 100.0, Vec3::ZERO));
        assert!(e.move_ref(CANDLE, PLAYER_REF, at + Vec3::Y * 100.0, Vec3::ZERO));
        assert!(drawn(&e, key, POT) > 0, "the pot isn't drawn");
        assert!(e.cells[&key].loose.iter().any(|l| l.ref_id == POT));
        assert!(e.cells[&key].lights.iter().any(|l| l.ref_id == CANDLE));
        assert!(e.ref_position(POT).unwrap().distance(at + Vec3::X * 100.0) < 1.0);
        // Moved within the cell: the same pot, moved.
        assert!(e.move_ref(POT, PLAYER_REF, at + Vec3::X * 200.0, Vec3::ZERO));
        assert_eq!(
            e.cells[&key]
                .loose
                .iter()
                .filter(|l| l.ref_id == POT)
                .count(),
            1
        );
        // Back where it was placed: out of this cell.
        assert!(e.move_to_editor_location(POT));
        assert!(e.move_to_editor_location(CANDLE));
        assert_eq!(drawn(&e, key, POT), 0);
        assert!(!e.cells[&key].refs.contains(&POT));
        assert!(!e.cells[&key].loose.iter().any(|l| l.ref_id == POT));
        assert!(!e.cells[&key].lights.iter().any(|l| l.ref_id == CANDLE));
        assert!(!e.physics.owners.values().any(|&o| o == POT));
        // Reloading the cell doesn't bring it back.
        e.enter_interior(here, None).unwrap();
        assert_eq!(drawn(&e, key, POT), 0);
    }
}
