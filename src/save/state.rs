//! What a save holds, as the engine's own types: `Snapshot::capture` copies
//! it from a running `Engine`, `write` / `read` turn it into sections (so a
//! damaged file fails before any of the engine is touched), and `apply`
//! puts it back.
//!
//! Sections: `HEAD` (the load order, the date, where the player stood),
//! `TIME`, `GLOB`, `QUST`, `STAT`, `AVAL` (actor value changes), `INVT`
//! (inventories with what is equipped), `SCRP` (what scripts did to
//! references), `WRLD` (the world state kept across cell loads and the
//! references made while playing), `PLYR` (health, skills, perks, spells,
//! shouts, favorites, dialogue heard), `CRIM` (bounties) and `BART`
//! (restocks), `STOR` (Story Manager memory), `RELA` (relationship overrides),
//! `ADOR` (doors actors left open), `DSTR` (destructible health/stage), `WUSE`
//! (Favorite Weapon usage), `PWCD` (Greater Power cooldowns), `SOUL`
//! (bodies already soul-trapped), `WTHR`
//! (the weather, its transition and override), `SCNE` (timer-only scene phases),
//! `SCLF` (scene lifecycle flags and action states), `SCAB` (scene actor and
//! package ownership bindings), `SDLG` (selected scene dialogue responses),
//! `SLRP` (looping scene dialogue deadlines),
//! `SRSQ` (ordered pending scene restarts),
//! `SFGT` (scene force-greet completion markers),
//! `RLSP` (rolled leveled spell-list results),
//! `WSTA` (ward-hit outcomes and their remaining `IsWardState` hold),
//! `DETH` (killers and times of death), `STPT` (stealth-point reservoirs and
//! regeneration delays), `COWN` (the owners of dropped stolen items), `ORIG`
//! (world-created references' initial transforms), and `VM` / `VMAR`
//! (`super::vm`: the Papyrus VM) and `MBXW` (message boxes and supported
//! message-box VM waits), and `ANMW` (semantic animation-event VM waits).
//! Not yet saved:
//! active magic effects, actor
//! procedure progress, AI progress, objects' behaviour graphs; see
//! `plans/ROADMAP.md`.
//!
//! Form ids are saved as loaded (their top byte is the load-order index), so a
//! save only loads with the same plugins in the same order: `HEAD` lists them
//! and a mismatch is refused.

use std::collections::{BTreeMap, HashMap, HashSet};

use esp::FormId;
use glam::{Mat4, Vec3};

use super::codec::{Corrupt, Reader, Result, Writer};
use super::file::SaveFile;
use crate::actor_values::Modifiers;
use crate::ai::schedule::Place;
use crate::created::Created;
use crate::engine::{Engine, Location};
use crate::script::QuestState;
use crate::skills::Skills;
use crate::story::StoryEvent;
use crate::weather::SavedWeather;
use crate::world::destruction::DestructionState;
use crate::world::inventory::Inventory;
use crate::world_state::{Moved, Wounds};

/// Where the player stood.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spot {
    pub location: Location,
    pub feet: Vec3,
    pub yaw: f32,
}

#[derive(Default)]
pub struct Snapshot {
    pub plugins: Vec<String>,
    pub spot: Option<Spot>,
    pub day: u32,
    pub hour: f32,
    pub real_time: f64,
    pub wall_time: f64,
    pub globals: HashMap<FormId, f32>,
    pub quests: HashMap<FormId, QuestState>,
    pub stats: HashMap<String, i32>,
    pub weapon_usage: Vec<(FormId, u64)>,
    pub power_uses: HashMap<FormId, f32>,
    pub spent_souls: HashSet<FormId>,
    pub fast_travel_disabled: bool,
    pub map_shown: HashSet<FormId>,
    pub map_discovered: HashSet<FormId>,
    pub map_travel: HashSet<FormId>,
    pub map_denied: HashSet<FormId>,
    pub favorite_weapon: Option<FormId>,
    pub actor_values: HashMap<(FormId, u32), Modifiers>,
    pub other_actor_values: HashMap<(FormId, String), f32>,
    pub inventories: HashMap<FormId, Inventory>,
    pub looted: HashSet<FormId>,
    pub disabled: HashMap<FormId, bool>,
    pub blocked_activation: HashSet<FormId>,
    pub locked: HashMap<FormId, bool>,
    pub lock_levels: HashMap<FormId, u8>,
    pub cell_public: HashMap<FormId, bool>,
    pub owners: HashMap<FormId, Option<FormId>>,
    pub faction_changes: HashMap<FormId, HashMap<FormId, i8>>,
    pub alerted: HashSet<FormId>,
    pub dead: HashMap<FormId, (Place, Vec3)>,
    pub deaths: HashMap<FormId, crate::world_state::Death>,
    pub stealth_points: HashMap<FormId, (f32, f32)>,
    pub open_doors: HashSet<FormId>,
    pub actor_open_doors: HashSet<FormId>,
    pub body_poses: HashMap<FormId, Vec<Mat4>>,
    pub wounds: HashMap<FormId, Wounds>,
    pub moved: HashMap<FormId, Moved>,
    pub item_charges: HashMap<FormId, Vec<f32>>,
    pub broken_joints: HashMap<FormId, Vec<usize>>,
    pub destruction: HashMap<FormId, DestructionState>,
    pub learned_effects: HashSet<(FormId, usize)>,
    pub created: HashMap<FormId, Created>,
    pub created_next: u32,
    pub health: f32,
    pub stamina: f32,
    pub skills: Option<Skills>,
    pub perks_added: HashMap<FormId, Vec<(FormId, u8)>>,
    pub perks_removed: HashMap<FormId, HashSet<FormId>>,
    pub spells_added: HashMap<FormId, Vec<FormId>>,
    pub spells_removed: HashMap<FormId, HashSet<FormId>>,
    pub words_known: HashSet<FormId>,
    pub words_unlocked: HashSet<FormId>,
    pub shouts_added: HashMap<FormId, Vec<FormId>>,
    pub shouts_removed: HashMap<FormId, HashSet<FormId>>,
    pub favorites: Vec<FormId>,
    pub hotkeys: [Option<FormId>; 8],
    pub infos_said: HashSet<FormId>,
    pub talked_to_pc: HashSet<FormId>,
    pub bounties: BTreeMap<FormId, crate::crime::Bounty>,
    pub stolen_value: BTreeMap<FormId, (i32, i32)>,
    pub restocked: HashMap<FormId, f64>,
    pub(crate) story: crate::story::StoryState,
    pub relationship_ranks: HashMap<(FormId, FormId), i32>,
    pub weather: SavedWeather,
    pub scenes: Vec<crate::scene::SavedScene>,
    pub scene_lifecycle: Vec<crate::scene::SavedSceneLifecycle>,
    pub scene_bindings: Vec<crate::scene::SavedSceneBinding>,
    pub scene_dialogue: Vec<crate::scene::SavedSceneDialogue>,
    pub scene_repeats: Vec<crate::scene::SavedSceneRepeat>,
    pub scene_restarts: Vec<FormId>,
    pub scene_force_greeted: Vec<FormId>,
    pub rolled_spells: Vec<crate::magic::SavedRolledSpell>,
    pub ward_states: HashMap<FormId, (i32, f32)>,
    pub vm: super::vm::VmSnapshot,
}

// ------------------------------------------------------------ write helpers

/// A map's entries in key order, so equal states make equal files.
fn sorted<K: Ord + Copy, V>(m: &HashMap<K, V>) -> Vec<(K, &V)> {
    let mut v: Vec<_> = m.iter().map(|(k, v)| (*k, v)).collect();
    v.sort_by_key(|(k, _)| *k);
    v
}

fn sorted_set<K: Ord + Copy>(s: &HashSet<K>) -> Vec<K> {
    let mut v: Vec<K> = s.iter().copied().collect();
    v.sort();
    v
}

fn put_forms(w: &mut Writer, s: &HashSet<FormId>) {
    w.len(s.len());
    for f in sorted_set(s) {
        w.form(f);
    }
}

fn get_forms(r: &mut Reader) -> Result<HashSet<FormId>> {
    (0..r.count(4)?).map(|_| r.form()).collect()
}

fn put_place(w: &mut Writer, p: Place) {
    match p {
        Place::Interior(c) => {
            w.u8(0);
            w.form(c);
        }
        Place::Exterior(world, (x, y)) => {
            w.u8(1);
            w.form(world);
            w.i32(x);
            w.i32(y);
        }
    }
}

fn get_place(r: &mut Reader) -> Result<Place> {
    match r.u8()? {
        0 => Ok(Place::Interior(r.form()?)),
        1 => Ok(Place::Exterior(r.form()?, (r.i32()?, r.i32()?))),
        n => Err(Corrupt(format!("place kind {n}"))),
    }
}

fn put_location(w: &mut Writer, l: Location) {
    match l {
        Location::Nowhere => w.u8(0),
        Location::Interior(c) => {
            w.u8(1);
            w.form(c);
        }
        Location::Exterior { world, center } => {
            w.u8(2);
            w.form(world);
            w.i32(center.0);
            w.i32(center.1);
        }
    }
}

fn get_location(r: &mut Reader) -> Result<Location> {
    match r.u8()? {
        0 => Ok(Location::Nowhere),
        1 => Ok(Location::Interior(r.form()?)),
        2 => Ok(Location::Exterior {
            world: r.form()?,
            center: (r.i32()?, r.i32()?),
        }),
        n => Err(Corrupt(format!("location kind {n}"))),
    }
}

fn put_story_event(w: &mut Writer, e: &StoryEvent) {
    w.0.extend_from_slice(&e.code);
    for f in e.refs.iter().chain(&e.locs) {
        w.form(*f);
    }
    w.form(e.keyword);
    w.f32(e.values[0]);
    w.f32(e.values[1]);
    w.form(e.form);
}

fn get_story_event(r: &mut Reader) -> Result<StoryEvent> {
    let mut e = StoryEvent::default();
    for b in &mut e.code {
        *b = r.u8()?;
    }
    for f in e.refs.iter_mut().chain(&mut e.locs) {
        *f = r.form()?;
    }
    e.keyword = r.form()?;
    e.values = [r.f32()?, r.f32()?];
    e.form = r.form()?;
    Ok(e)
}

fn put_perks(w: &mut Writer, added: &HashMap<FormId, Vec<(FormId, u8)>>) {
    w.len(added.len());
    for (actor, perks) in sorted(added) {
        w.form(actor);
        w.len(perks.len());
        for &(perk, rank) in perks {
            w.form(perk);
            w.u8(rank);
        }
    }
}

fn get_perks(r: &mut Reader) -> Result<HashMap<FormId, Vec<(FormId, u8)>>> {
    let mut out = HashMap::new();
    for _ in 0..r.count(8)? {
        let actor = r.form()?;
        let perks = (0..r.count(5)?)
            .map(|_| Ok((r.form()?, r.u8()?)))
            .collect::<Result<_>>()?;
        out.insert(actor, perks);
    }
    Ok(out)
}

fn put_form_lists(w: &mut Writer, m: &HashMap<FormId, Vec<FormId>>) {
    w.len(m.len());
    for (k, list) in sorted(m) {
        w.form(k);
        w.len(list.len());
        for &f in list {
            w.form(f);
        }
    }
}

fn get_form_lists(r: &mut Reader) -> Result<HashMap<FormId, Vec<FormId>>> {
    let mut out = HashMap::new();
    for _ in 0..r.count(8)? {
        let k = r.form()?;
        out.insert(
            k,
            (0..r.count(4)?).map(|_| r.form()).collect::<Result<_>>()?,
        );
    }
    Ok(out)
}

fn put_form_sets(w: &mut Writer, m: &HashMap<FormId, HashSet<FormId>>) {
    w.len(m.len());
    for (k, set) in sorted(m) {
        w.form(k);
        put_forms(w, set);
    }
}

fn get_form_sets(r: &mut Reader) -> Result<HashMap<FormId, HashSet<FormId>>> {
    let mut out = HashMap::new();
    for _ in 0..r.count(8)? {
        let k = r.form()?;
        out.insert(k, get_forms(r)?);
    }
    Ok(out)
}

fn put_inventory(w: &mut Writer, inv: &Inventory) {
    w.len(inv.items.len());
    for &(f, n) in &inv.items {
        w.form(f);
        w.i32(n);
    }
    let worn = inv.equipped();
    w.len(worn.len());
    for &f in worn {
        w.form(f);
    }
    let (held, both) = inv.hands();
    for h in held {
        w.opt_form(h);
    }
    w.bool(both);
    w.len(inv.owned.len());
    for &(item, owner, n) in &inv.owned {
        w.form(item);
        w.form(owner);
        w.i32(n);
    }
    w.len(inv.souls.len());
    for &(gem, soul, n) in &inv.souls {
        w.form(gem);
        w.u8(soul);
        w.i32(n);
    }
    w.len(inv.poisoned.len());
    for &(weapon, poison, doses) in &inv.poisoned {
        w.form(weapon);
        w.form(poison);
        w.i32(doses);
    }
    w.len(inv.charges.len());
    for &(weapon, left) in &inv.charges {
        w.form(weapon);
        w.f32(left);
    }
}

fn get_inventory(r: &mut Reader) -> Result<Inventory> {
    let mut inv = Inventory::default();
    for _ in 0..r.count(8)? {
        inv.items.push((r.form()?, r.i32()?));
    }
    let worn = (0..r.count(4)?)
        .map(|_| r.form())
        .collect::<Result<Vec<_>>>()?;
    let held = [r.opt_form()?, r.opt_form()?, r.opt_form()?];
    let both = r.bool()?;
    for _ in 0..r.count(12)? {
        inv.owned.push((r.form()?, r.form()?, r.i32()?));
    }
    for _ in 0..r.count(9)? {
        inv.souls.push((r.form()?, r.u8()?, r.i32()?));
    }
    for _ in 0..r.count(12)? {
        inv.poisoned.push((r.form()?, r.form()?, r.i32()?));
    }
    for _ in 0..r.count(8)? {
        inv.charges.push((r.form()?, r.f32()?));
    }
    Ok(inv.with_equipment(worn, held, both))
}

fn put_quest(w: &mut Writer, q: &QuestState) {
    w.bool(q.running);
    w.u16(q.stage);
    w.len(q.done.len());
    for n in sorted_set(&q.done) {
        w.u16(n);
    }
    for set in [
        &q.objectives_displayed,
        &q.objectives_completed,
        &q.objectives_failed,
    ] {
        w.len(set.len());
        for n in sorted_set(set) {
            w.i32(n);
        }
    }
    w.len(q.objective_texts.len());
    for (i, text) in sorted(&q.objective_texts) {
        w.i32(i);
        w.str(text);
    }
    w.bool(q.completed);
    w.bool(q.failed);
    w.len(q.log.len());
    for line in &q.log {
        w.str(line);
    }
    w.bool(q.announced);
    w.bool(q.active);
    w.u64(q.journal_seq);
    w.len(q.aliases.len());
    for (id, f) in sorted(&q.aliases) {
        w.u32(id);
        w.form(*f);
    }
    w.len(q.filled_at.len());
    for (id, at) in sorted(&q.filled_at) {
        w.u32(id);
        w.u64(*at);
    }
    w.bool(q.event.is_some());
    if let Some(e) = &q.event {
        put_story_event(w, e);
    }
}

fn get_quest(r: &mut Reader) -> Result<QuestState> {
    let mut q = QuestState {
        running: r.bool()?,
        stage: r.u16()?,
        ..Default::default()
    };
    for _ in 0..r.count(2)? {
        q.done.insert(r.u16()?);
    }
    for set in [
        &mut q.objectives_displayed,
        &mut q.objectives_completed,
        &mut q.objectives_failed,
    ] {
        for _ in 0..r.count(4)? {
            set.insert(r.i32()?);
        }
    }
    for _ in 0..r.count(8)? {
        let i = r.i32()?;
        q.objective_texts.insert(i, r.str()?);
    }
    q.completed = r.bool()?;
    q.failed = r.bool()?;
    for _ in 0..r.count(4)? {
        q.log.push(r.str()?);
    }
    q.announced = r.bool()?;
    q.active = r.bool()?;
    q.journal_seq = r.u64()?;
    for _ in 0..r.count(8)? {
        let id = r.u32()?;
        q.aliases.insert(id, r.form()?);
    }
    for _ in 0..r.count(12)? {
        let id = r.u32()?;
        q.filled_at.insert(id, r.u64()?);
    }
    if r.bool()? {
        q.event = Some(get_story_event(r)?);
    }
    Ok(q)
}

impl Snapshot {
    /// Copy what saves keep out of a running engine.
    pub fn capture(e: &Engine) -> Snapshot {
        let ws = &e.world_state;
        let s = &e.scripts;
        let (weapon_usage, favorite_weapon) = s.weapon_usage.saved();
        Snapshot {
            plugins: e
                .lo
                .plugins()
                .iter()
                .map(|p| p.plugin.name().to_string())
                .collect(),
            spot: match e.location {
                Location::Nowhere => None,
                location => Some(Spot {
                    location,
                    // What `place_player` takes, undoing its clearance.
                    feet: e.player_feet() - Vec3::Z * crate::engine::PLACEMENT_CLEARANCE,
                    yaw: e.camera.yaw,
                }),
            },
            day: e.day,
            hour: e.hour,
            real_time: s.real_time,
            wall_time: s.wall_time,
            globals: s.globals.clone(),
            quests: s.quests.clone(),
            stats: s.stats.clone(),
            weapon_usage,
            power_uses: e.casting.power_uses().clone(),
            spent_souls: e.magic.souls_taken.clone(),
            fast_travel_disabled: e.fast_travel_disabled,
            map_shown: e.map_markers.shown.clone(),
            map_discovered: e.map_markers.discovered.clone(),
            map_travel: e.map_markers.travel.clone(),
            map_denied: e.map_markers.denied.clone(),
            favorite_weapon,
            actor_values: s.actor_values.clone(),
            other_actor_values: s.other_actor_values.clone(),
            inventories: e.inventories.clone(),
            looted: s.looted.clone(),
            disabled: s.disabled.clone(),
            blocked_activation: s.blocked_activation.clone(),
            locked: s.locked.clone(),
            lock_levels: s.lock_levels.clone(),
            owners: s.owners.clone(),
            faction_changes: s.faction_changes.clone(),
            alerted: ws.alerted.clone(),
            dead: ws.dead.clone(),
            deaths: ws.deaths.clone(),
            stealth_points: e.detection.saved_stealth_points(),
            open_doors: ws.open_doors.clone(),
            actor_open_doors: ws.actor_open_doors.clone(),
            body_poses: ws.body_poses.clone(),
            wounds: ws.wounds.clone(),
            moved: ws.moved.clone(),
            item_charges: ws.item_charges.clone(),
            broken_joints: ws.broken_joints.clone(),
            destruction: ws.destruction.clone(),
            learned_effects: ws.learned_effects.clone(),
            created: e.created_refs.refs.clone(),
            created_next: e.created_refs.next,
            health: e.player_health,
            stamina: e.player_stamina,
            skills: Some(e.skills.clone()),
            perks_added: e.perks.added.clone(),
            perks_removed: e.perks.removed.clone(),
            spells_added: e.magic.added.clone(),
            spells_removed: e.magic.removed.clone(),
            words_known: e.shouts.known.clone(),
            words_unlocked: e.shouts.unlocked.clone(),
            shouts_added: e.shouts.added.clone(),
            shouts_removed: e.shouts.removed.clone(),
            favorites: e.favorites.clone(),
            hotkeys: e.hotkeys,
            infos_said: e.infos_said.clone(),
            talked_to_pc: e.talked_to_pc.clone(),
            bounties: e.crime.bounties.clone(),
            stolen_value: e.crime.stolen_value.clone(),
            restocked: e.restocks.stocked.clone(),
            cell_public: e.scripts.cell_public.clone(),
            story: e.story.saved_state(),
            relationship_ranks: e.relationships.saved_ranks().clone(),
            weather: e.saved_weather(),
            scenes: e.saved_timer_scenes(),
            scene_lifecycle: e.saved_scene_lifecycles(),
            scene_bindings: e.saved_scene_bindings(),
            scene_dialogue: e.saved_scene_dialogue(),
            scene_repeats: e.saved_scene_repeats(),
            scene_restarts: e.saved_scene_restarts(),
            scene_force_greeted: e.saved_scene_force_greeted(),
            rolled_spells: e.saved_rolled_spells(),
            ward_states: e.magic.ward_states.clone(),
            vm: super::vm::VmSnapshot {
                vm: e.vm.export(),
                initialized: s.initialized.iter().copied().collect(),
                timers: s.timers.clone(),
                sleep_listeners: s.sleep_listeners.clone(),
                inventory_filters: s
                    .inventory_filters
                    .iter()
                    .map(|(o, items)| (*o, items.iter().copied().collect()))
                    .collect(),
                anim_registrations: e.anim_events.registered(),
                message_boxes: e.messages.boxes.clone(),
                message_next_key: e.messages.next_key,
                animation_waits: e.anim_events.waiting(),
            },
        }
    }

    pub fn write(&self) -> SaveFile {
        let mut file = SaveFile::default();

        let mut w = Writer::default();
        w.len(self.plugins.len());
        for p in &self.plugins {
            w.str(p);
        }
        w.bool(self.spot.is_some());
        if let Some(spot) = &self.spot {
            put_location(&mut w, spot.location);
            w.vec3(spot.feet);
            w.f32(spot.yaw);
        }
        file.add(b"HEAD", w.0);

        let mut w = Writer::default();
        w.u32(self.day);
        w.f32(self.hour);
        w.f64(self.real_time);
        w.f64(self.wall_time);
        file.add(b"TIME", w.0);

        let mut w = Writer::default();
        w.len(self.globals.len());
        for (g, v) in sorted(&self.globals) {
            w.form(g);
            w.f32(*v);
        }
        file.add(b"GLOB", w.0);

        let mut w = Writer::default();
        w.len(self.quests.len());
        for (id, q) in sorted(&self.quests) {
            w.form(id);
            put_quest(&mut w, q);
        }
        file.add(b"QUST", w.0);

        let mut w = Writer::default();
        let mut stats: Vec<_> = self.stats.iter().collect();
        stats.sort();
        w.len(stats.len());
        for (name, n) in stats {
            w.str(name);
            w.i32(*n);
        }
        file.add(b"STAT", w.0);

        let mut w = Writer::default();
        let mut weapon_usage = self.weapon_usage.clone();
        weapon_usage.sort_by_key(|(weapon, _)| *weapon);
        w.len(weapon_usage.len());
        for (weapon, count) in weapon_usage {
            w.form(weapon);
            w.u64(count);
        }
        w.opt_form(self.favorite_weapon);
        file.add(b"WUSE", w.0);

        let mut w = Writer::default();
        w.len(self.power_uses.len());
        for (power, last_used) in sorted(&self.power_uses) {
            w.form(power);
            w.f32(*last_used);
        }
        file.add(b"PWCD", w.0);

        let mut w = Writer::default();
        put_forms(&mut w, &self.spent_souls);
        file.add(b"SOUL", w.0);

        let mut w = Writer::default();
        w.len(self.actor_values.len());
        for ((actor, index), m) in {
            let mut v: Vec<_> = self.actor_values.iter().collect();
            v.sort_by_key(|(k, _)| **k);
            v
        } {
            w.form(*actor);
            w.u32(*index);
            w.bool(m.base.is_some());
            w.f32(m.base.unwrap_or(0.0));
            w.f32(m.permanent);
            w.f32(m.temporary);
            w.f32(m.damage);
        }
        let mut others: Vec<_> = self.other_actor_values.iter().collect();
        others.sort_by(|a, b| a.0.cmp(b.0));
        w.len(others.len());
        for ((actor, name), v) in others {
            w.form(*actor);
            w.str(name);
            w.f32(*v);
        }
        file.add(b"AVAL", w.0);

        let mut w = Writer::default();
        w.len(self.inventories.len());
        for (owner, inv) in sorted(&self.inventories) {
            w.form(owner);
            put_inventory(&mut w, inv);
        }
        file.add(b"INVT", w.0);

        let mut w = Writer::default();
        put_forms(&mut w, &self.looted);
        w.len(self.disabled.len());
        for (f, on) in sorted(&self.disabled) {
            w.form(f);
            w.bool(*on);
        }
        put_forms(&mut w, &self.blocked_activation);
        w.len(self.locked.len());
        for (f, on) in sorted(&self.locked) {
            w.form(f);
            w.bool(*on);
        }
        w.len(self.lock_levels.len());
        for (f, level) in sorted(&self.lock_levels) {
            w.form(f);
            w.u8(*level);
        }
        w.len(self.owners.len());
        for (f, owner) in sorted(&self.owners) {
            w.form(f);
            w.bool(owner.is_some());
            w.opt_form(*owner);
        }
        w.len(self.faction_changes.len());
        for (actor, ranks) in sorted(&self.faction_changes) {
            w.form(actor);
            w.len(ranks.len());
            for (faction, rank) in sorted(ranks) {
                w.form(faction);
                w.u8(*rank as u8);
            }
        }
        file.add(b"SCRP", w.0);

        let mut w = Writer::default();
        put_forms(&mut w, &self.alerted);
        w.len(self.dead.len());
        for (actor, (place, at)) in sorted(&self.dead) {
            w.form(actor);
            put_place(&mut w, *place);
            w.vec3(*at);
        }
        put_forms(&mut w, &self.open_doors);
        w.len(self.body_poses.len());
        for (actor, bodies) in sorted(&self.body_poses) {
            w.form(actor);
            w.len(bodies.len());
            for m in bodies {
                for c in m.to_cols_array() {
                    w.f32(c);
                }
            }
        }
        w.len(self.wounds.len());
        for (actor, x) in sorted(&self.wounds) {
            w.form(actor);
            w.f32(x.health);
            w.f32(x.max);
            w.f32(x.rate);
            w.f64(x.at);
        }
        w.len(self.moved.len());
        for (r, m) in sorted(&self.moved) {
            w.form(r);
            put_place(&mut w, m.place);
            w.vec3(m.pos);
            w.vec3(m.rot);
        }
        w.len(self.item_charges.len());
        for (r, charges) in sorted(&self.item_charges) {
            w.form(r);
            w.len(charges.len());
            for &c in charges {
                w.f32(c);
            }
        }
        w.len(self.broken_joints.len());
        for (r, joints) in sorted(&self.broken_joints) {
            w.form(r);
            w.len(joints.len());
            for &j in joints {
                w.u32(j as u32);
            }
        }
        let mut learned: Vec<_> = self.learned_effects.iter().copied().collect();
        learned.sort();
        w.len(learned.len());
        for (ingredient, index) in learned {
            w.form(ingredient);
            w.u32(index as u32);
        }
        w.u32(self.created_next);
        w.len(self.created.len());
        for (r, c) in sorted(&self.created) {
            w.form(r);
            w.form(c.base);
            w.vec3(c.position);
            w.vec3(c.rotation);
            w.bool(c.place.is_some());
            if let Some(p) = c.place {
                put_place(&mut w, p);
            }
            w.opt_form(c.container);
            w.opt_form(c.location);
            w.bool(c.actor);
            w.i32(c.count);
        }
        file.add(b"WRLD", w.0);

        // Created references' initial world transforms were added after WRLD's
        // layout shipped; keep them in an optional section for older readers.
        let origins: Vec<_> = sorted(&self.created)
            .into_iter()
            .filter_map(|(id, c)| Some((id, c.origin?)))
            .collect();
        if !origins.is_empty() {
            let mut w = Writer::default();
            w.len(origins.len());
            for (id, origin) in origins {
                w.form(id);
                w.vec3(origin.position);
                w.vec3(origin.rotation);
            }
            file.add(b"ORIG", w.0);
        }

        let mut w = Writer::default();
        put_forms(&mut w, &self.actor_open_doors);
        file.add(b"ADOR", w.0);

        let mut w = Writer::default();
        w.len(self.destruction.len());
        for (reference, state) in sorted(&self.destruction) {
            w.form(reference);
            w.f64(state.health);
            w.i32(state.stage);
        }
        file.add(b"DSTR", w.0);

        let mut w = Writer::default();
        w.f32(self.health);
        w.f32(self.stamina);
        let skills = self.skills.clone().unwrap_or_default();
        w.len(skills.xp.len());
        for x in skills.xp {
            w.f32(x);
        }
        for n in skills.legendary {
            w.u32(n);
        }
        w.u16(skills.level);
        w.f32(skills.level_xp);
        w.u32(skills.pending);
        w.u32(skills.perk_points);
        w.u32(skills.trained);
        put_forms(&mut w, &skills.read_books);
        put_forms(&mut w, &skills.books_seen);
        put_perks(&mut w, &self.perks_added);
        put_form_sets(&mut w, &self.perks_removed);
        put_form_lists(&mut w, &self.spells_added);
        put_form_sets(&mut w, &self.spells_removed);
        put_forms(&mut w, &self.words_known);
        put_forms(&mut w, &self.words_unlocked);
        put_form_lists(&mut w, &self.shouts_added);
        put_form_sets(&mut w, &self.shouts_removed);
        w.len(self.favorites.len());
        for &f in &self.favorites {
            w.form(f);
        }
        for h in self.hotkeys {
            w.opt_form(h);
        }
        put_forms(&mut w, &self.infos_said);
        put_forms(&mut w, &self.talked_to_pc);
        file.add(b"PLYR", w.0);

        let mut w = Writer::default();
        w.len(self.bounties.len());
        for (faction, b) in &self.bounties {
            w.form(*faction);
            for n in [
                b.violent,
                b.nonviolent,
                b.infamy_violent,
                b.infamy_nonviolent,
            ] {
                w.i32(n);
            }
        }
        w.len(self.stolen_value.len());
        for (faction, (unwitnessed, witnessed)) in &self.stolen_value {
            w.form(*faction);
            w.i32(*unwitnessed);
            w.i32(*witnessed);
        }
        file.add(b"CRIM", w.0);

        let mut w = Writer::default();
        w.len(self.restocked.len());
        for (chest, at) in sorted(&self.restocked) {
            w.form(chest);
            w.f64(*at);
        }
        file.add(b"BART", w.0);
        let mut w = Writer::default();
        w.len(self.cell_public.len());
        for (cell, public) in sorted(&self.cell_public) {
            w.form(cell);
            w.bool(*public);
        }
        file.add(b"CPUB", w.0);
        let mut w = Writer::default();
        w.len(self.story.last_run.len());
        for (quest, at) in sorted(&self.story.last_run) {
            w.form(quest);
            w.f64(*at);
        }
        put_form_sets(&mut w, &self.story.ran);
        w.len(self.story.next_social.len());
        for (actor, at) in sorted(&self.story.next_social) {
            w.form(actor);
            w.f64(*at);
        }
        w.bool(self.story.last_location.is_some());
        if let Some(location) = self.story.last_location {
            w.opt_form(location);
        }
        w.len(self.story.found_bodies.len());
        // Finder/body pairs are directional, unlike relationship pairs.
        for (finder, body) in sorted_set(&self.story.found_bodies) {
            w.form(finder);
            w.form(body);
        }
        w.f64(self.story.next_body_check);
        file.add(b"STOR", w.0);

        let mut w = Writer::default();
        w.len(self.relationship_ranks.len());
        for ((a, b), rank) in sorted(&self.relationship_ranks) {
            w.form(a);
            w.form(b);
            w.i32(*rank);
        }
        file.add(b"RELA", w.0);

        let mut w = Writer::default();
        let weather = self.weather;
        w.opt_form(weather.current);
        w.opt_form(weather.outgoing);
        w.f32(weather.pct);
        w.bool(weather.overridden);
        w.opt_form(weather.before_override);
        w.f32(weather.next_roll);
        file.add(b"WTHR", w.0);

        let mut w = Writer::default();
        w.len(self.deaths.len());
        for (actor, death) in sorted(&self.deaths) {
            w.form(actor);
            w.opt_form(death.killer);
            w.f64(death.at);
        }
        file.add(b"DETH", w.0);

        // Stealth reservoirs are optional so older saves restore as empty.
        let mut w = Writer::default();
        w.len(self.stealth_points.len());
        for (actor, (points, regen_wait)) in sorted(&self.stealth_points) {
            w.form(actor);
            w.f32(*points);
            w.f32(*regen_wait);
        }
        file.add(b"STPT", w.0);

        // Dropped stolen items' owners, beside `WRLD`'s created references so
        // saves from before them still read.
        let mut w = Writer::default();
        let owned: Vec<_> = sorted(&self.created)
            .into_iter()
            .filter_map(|(id, c)| Some((id, c.owner?)))
            .collect();
        w.len(owned.len());
        for (id, owner) in owned {
            w.form(id);
            w.form(owner);
        }
        file.add(b"COWN", w.0);

        if !self.scenes.is_empty() {
            let mut w = Writer::default();
            w.len(self.scenes.len());
            for scene in &self.scenes {
                w.form(scene.scene);
                w.u32(scene.phase);
                w.len(scene.timers.len());
                for timer in &scene.timers {
                    w.u32(timer.action);
                    w.bool(timer.elapsed.is_some());
                    if let Some(elapsed) = timer.elapsed {
                        w.f64(elapsed);
                    }
                }
            }
            file.add(b"SCNE", w.0);
        }
        if !self.scene_lifecycle.is_empty() {
            let mut w = Writer::default();
            w.len(self.scene_lifecycle.len());
            for scene in &self.scene_lifecycle {
                w.form(scene.scene);
                w.bool(scene.phase.is_some());
                if let Some(phase) = scene.phase {
                    w.u32(phase);
                }
                w.bool(scene.begun);
                w.bool(scene.stopping);
                w.bool(scene.paused);
                w.len(scene.actions.len());
                for action in &scene.actions {
                    w.u32(action.action);
                    w.u8(match action.state {
                        crate::scene::SavedActionState::Waiting => 0,
                        crate::scene::SavedActionState::Running => 1,
                        crate::scene::SavedActionState::Done => 2,
                        crate::scene::SavedActionState::Stopped => 3,
                    });
                    w.bool(action.elapsed.is_some());
                    if let Some(elapsed) = action.elapsed {
                        w.f64(elapsed);
                    }
                }
            }
            file.add(b"SCLF", w.0);
        }
        if !self.scene_bindings.is_empty() {
            let mut bindings = self.scene_bindings.clone();
            bindings.sort_by_key(|binding| (binding.scene, binding.action));
            let mut w = Writer::default();
            w.len(bindings.len());
            for binding in bindings {
                w.form(binding.scene);
                w.u32(binding.action);
                w.bool(binding.actor.is_some());
                if let Some(actor) = binding.actor {
                    w.form(actor);
                }
                w.bool(binding.package_active);
                w.bool(binding.elapsed.is_some());
                if let Some(elapsed) = binding.elapsed {
                    w.f64(elapsed);
                }
            }
            file.add(b"SCAB", w.0);
        }
        if !self.scene_dialogue.is_empty() {
            let mut dialogue = self.scene_dialogue.clone();
            dialogue.sort_by_key(|saved| (saved.scene, saved.action));
            let mut w = Writer::default();
            w.len(dialogue.len());
            for saved in dialogue {
                w.form(saved.scene);
                w.u32(saved.action);
                w.form(saved.topic);
                w.form(saved.info);
                w.u32(saved.current_response);
                w.len(saved.queued_responses.len());
                for response in saved.queued_responses {
                    w.u32(response);
                }
                w.f64(saved.remaining);
            }
            file.add(b"SDLG", w.0);
        }
        if !self.scene_repeats.is_empty() {
            let mut repeats = self.scene_repeats.clone();
            repeats.sort_by_key(|saved| (saved.scene, saved.action));
            let mut w = Writer::default();
            w.len(repeats.len());
            for saved in repeats {
                w.form(saved.scene);
                w.u32(saved.action);
                w.f64(saved.remaining);
            }
            file.add(b"SLRP", w.0);
        }
        if !self.scene_restarts.is_empty() {
            let mut w = Writer::default();
            w.len(self.scene_restarts.len());
            for &scene in &self.scene_restarts {
                w.form(scene);
            }
            file.add(b"SRSQ", w.0);
        }
        if !self.scene_force_greeted.is_empty() {
            let mut actors = self.scene_force_greeted.clone();
            actors.sort_unstable();
            let mut w = Writer::default();
            w.len(actors.len());
            for actor in actors {
                w.form(actor);
            }
            file.add(b"SFGT", w.0);
        }

        if !self.rolled_spells.is_empty() {
            let mut results = self.rolled_spells.clone();
            results.sort_by_key(|entry| (entry.actor, entry.list));
            let mut w = Writer::default();
            w.len(results.len());
            for entry in results {
                w.form(entry.actor);
                w.form(entry.list);
                w.len(entry.spells.len());
                for spell in entry.spells {
                    w.form(spell);
                }
            }
            file.add(b"RLSP", w.0);
        }

        if !self.ward_states.is_empty() {
            let mut w = Writer::default();
            w.len(self.ward_states.len());
            for (actor, (state, remaining)) in sorted(&self.ward_states) {
                w.form(actor);
                w.i32(*state);
                w.f32(*remaining);
            }
            file.add(b"WSTA", w.0);
        }
        let mut w = Writer::default();
        w.bool(self.fast_travel_disabled);
        for set in [
            &self.map_shown,
            &self.map_discovered,
            &self.map_travel,
            &self.map_denied,
        ] {
            put_forms(&mut w, set);
        }
        file.add(b"MAPS", w.0);
        self.vm.write(&mut file);
        file
    }
}

fn section<'a>(file: &'a SaveFile, tag: &[u8; 4]) -> Result<Reader<'a>> {
    file.section(tag)
        .map(Reader::new)
        .ok_or_else(|| Corrupt(format!("no {} section", String::from_utf8_lossy(tag))))
}

impl Snapshot {
    /// Read every section; nothing is applied until all of them read.
    pub fn read(file: &SaveFile) -> Result<Snapshot> {
        let mut s = Snapshot::default();

        if file
            .sections
            .iter()
            .filter(|(tag, _)| tag == b"MAPS")
            .count()
            > 1
        {
            return Err(Corrupt("duplicate MAPS section".into()));
        }
        if let Some(data) = file.section(b"MAPS") {
            let mut r = Reader::new(data);
            s.fast_travel_disabled = r.bool()?;
            for set in [
                &mut s.map_shown,
                &mut s.map_discovered,
                &mut s.map_travel,
                &mut s.map_denied,
            ] {
                let mut previous = None;
                for _ in 0..r.count(4)? {
                    let id = r.form()?;
                    if id.is_null() || previous.is_some_and(|p| p >= id) {
                        return Err(Corrupt("invalid or unsorted map marker".into()));
                    }
                    previous = Some(id);
                    set.insert(id);
                }
            }
            if !s.map_travel.is_disjoint(&s.map_denied) {
                return Err(Corrupt("conflicting map travel permissions".into()));
            }
            r.finish()?;
        }

        let mut r = section(file, b"HEAD")?;
        for _ in 0..r.count(4)? {
            s.plugins.push(r.str()?);
        }
        if r.bool()? {
            s.spot = Some(Spot {
                location: get_location(&mut r)?,
                feet: r.vec3()?,
                yaw: r.f32()?,
            });
        }
        r.finish()?;

        let mut r = section(file, b"TIME")?;
        s.day = r.u32()?;
        s.hour = r.f32()?;
        s.real_time = r.f64()?;
        s.wall_time = r.f64()?;
        r.finish()?;

        let mut r = section(file, b"GLOB")?;
        for _ in 0..r.count(8)? {
            let g = r.form()?;
            s.globals.insert(g, r.f32()?);
        }
        r.finish()?;

        let mut r = section(file, b"QUST")?;
        for _ in 0..r.count(8)? {
            let id = r.form()?;
            s.quests.insert(id, get_quest(&mut r)?);
        }
        r.finish()?;

        let mut r = section(file, b"STAT")?;
        for _ in 0..r.count(8)? {
            let name = r.str()?;
            s.stats.insert(name, r.i32()?);
        }
        r.finish()?;

        if let Some(data) = file.section(b"WUSE") {
            let mut r = Reader::new(data);
            let mut previous = None;
            for _ in 0..r.count(12)? {
                let weapon = r.form()?;
                let count = r.u64()?;
                if weapon.is_null() || count == 0 || previous.is_some_and(|p| p >= weapon) {
                    return Err(Corrupt(
                        "invalid, duplicate, or unsorted favorite-weapon usage".into(),
                    ));
                }
                previous = Some(weapon);
                s.weapon_usage.push((weapon, count));
            }
            s.favorite_weapon = r.opt_form()?;
            r.finish()?;
            crate::stats::WeaponUsage::restore(s.weapon_usage.clone(), s.favorite_weapon)
                .map_err(|e| Corrupt(e.to_string()))?;
        }

        if let Some(data) = file.section(b"PWCD") {
            let mut r = Reader::new(data);
            let mut previous = None;
            for _ in 0..r.count(8)? {
                let power = r.form()?;
                let last_used = r.f32()?;
                if power.is_null()
                    || !last_used.is_finite()
                    || last_used < 0.0
                    || previous.is_some_and(|p| p >= power)
                {
                    return Err(Corrupt(
                        "invalid, duplicate, or unsorted power cooldown".into(),
                    ));
                }
                previous = Some(power);
                s.power_uses.insert(power, last_used);
            }
            r.finish()?;
        }

        if file
            .sections
            .iter()
            .filter(|(tag, _)| tag == b"SCNE")
            .count()
            > 1
        {
            return Err(Corrupt("duplicate SCNE section".into()));
        }
        if let Some(data) = file.section(b"SCNE") {
            let mut r = Reader::new(data);
            let mut seen_scenes = HashSet::new();
            for _ in 0..r.count(12)? {
                let scene = r.form()?;
                let phase = r.u32()?;
                if scene.is_null() || !seen_scenes.insert(scene) {
                    return Err(Corrupt("invalid or duplicate scene".into()));
                }
                let mut timers = Vec::new();
                let mut previous_action = None;
                for _ in 0..r.count(5)? {
                    let action = r.u32()?;
                    let elapsed = if r.bool()? { Some(r.f64()?) } else { None };
                    if previous_action.is_some_and(|p| p >= action)
                        || elapsed.is_some_and(|elapsed| !elapsed.is_finite() || elapsed < 0.0)
                    {
                        return Err(Corrupt(
                            "invalid, duplicate, or unsorted scene timer".into(),
                        ));
                    }
                    previous_action = Some(action);
                    timers.push(crate::scene::SavedSceneTimer { action, elapsed });
                }
                s.scenes.push(crate::scene::SavedScene {
                    scene,
                    phase,
                    timers,
                });
            }
            r.finish()?;
        }
        if file
            .sections
            .iter()
            .filter(|(tag, _)| tag == b"SCLF")
            .count()
            > 1
        {
            return Err(Corrupt("duplicate SCLF section".into()));
        }
        if let Some(data) = file.section(b"SCLF") {
            let mut r = Reader::new(data);
            let mut seen_scenes = HashSet::new();
            for _ in 0..r.count(12)? {
                let scene = r.form()?;
                if scene.is_null() || !seen_scenes.insert(scene) {
                    return Err(Corrupt("invalid or duplicate scene lifecycle".into()));
                }
                let phase = if r.bool()? { Some(r.u32()?) } else { None };
                let begun = r.bool()?;
                let stopping = r.bool()?;
                let paused = r.bool()?;
                let mut actions = Vec::new();
                let mut previous_action = None;
                for _ in 0..r.count(6)? {
                    let action = r.u32()?;
                    let state = match r.u8()? {
                        0 => crate::scene::SavedActionState::Waiting,
                        1 => crate::scene::SavedActionState::Running,
                        2 => crate::scene::SavedActionState::Done,
                        3 => crate::scene::SavedActionState::Stopped,
                        n => return Err(Corrupt(format!("scene action state {n}"))),
                    };
                    let elapsed = if r.bool()? { Some(r.f64()?) } else { None };
                    if previous_action.is_some_and(|previous| previous >= action) {
                        return Err(Corrupt("duplicate or unsorted scene action".into()));
                    }
                    previous_action = Some(action);
                    actions.push(crate::scene::SavedSceneAction {
                        action,
                        state,
                        elapsed,
                    });
                }
                s.scene_lifecycle.push(crate::scene::SavedSceneLifecycle {
                    scene,
                    phase,
                    begun,
                    stopping,
                    paused,
                    actions,
                });
            }
            r.finish()?;
        }
        if file
            .sections
            .iter()
            .filter(|(tag, _)| tag == b"SCAB")
            .count()
            > 1
        {
            return Err(Corrupt("duplicate SCAB section".into()));
        }
        if let Some(data) = file.section(b"SCAB") {
            let mut r = Reader::new(data);
            let mut previous = None;
            for _ in 0..r.count(11)? {
                let scene = r.form()?;
                let action = r.u32()?;
                let actor = if r.bool()? { Some(r.form()?) } else { None };
                let package_active = r.bool()?;
                let elapsed = if r.bool()? { Some(r.f64()?) } else { None };
                let binding = crate::scene::SavedSceneBinding {
                    scene,
                    action,
                    actor,
                    package_active,
                    elapsed,
                };
                let key = (binding.scene, binding.action);
                if binding.scene.is_null()
                    || binding.actor.is_some_and(FormId::is_null)
                    || (binding.package_active
                        && (!binding.actor.is_some() || binding.elapsed.is_none()))
                    || (!binding.package_active && binding.elapsed.is_some())
                    || binding
                        .elapsed
                        .is_some_and(|elapsed| !elapsed.is_finite() || elapsed < 0.0)
                    || previous.is_some_and(|previous| previous >= key)
                {
                    return Err(Corrupt(
                        "invalid, duplicate or unsorted scene binding".into(),
                    ));
                }
                previous = Some(key);
                s.scene_bindings.push(binding);
            }
            r.finish()?;
        }
        if file
            .sections
            .iter()
            .filter(|(tag, _)| tag == b"SDLG")
            .count()
            > 1
        {
            return Err(Corrupt("duplicate SDLG section".into()));
        }
        if let Some(data) = file.section(b"SDLG") {
            let mut r = Reader::new(data);
            let mut previous = None;
            for _ in 0..r.count(32)? {
                let scene = r.form()?;
                let action = r.u32()?;
                let topic = r.form()?;
                let info = r.form()?;
                let current_response = r.u32()?;
                let mut queued_responses = Vec::new();
                let mut last = Some(current_response);
                for _ in 0..r.count(4)? {
                    let response = r.u32()?;
                    if last.and_then(|last| last.checked_add(1)) != Some(response) {
                        return Err(Corrupt("invalid scene response suffix".into()));
                    }
                    last = Some(response);
                    queued_responses.push(response);
                }
                let remaining = r.f64()?;
                let key = (scene, action);
                if scene.is_null()
                    || topic.is_null()
                    || info.is_null()
                    || !remaining.is_finite()
                    || remaining < 0.0
                    || previous.is_some_and(|previous| previous >= key)
                {
                    return Err(Corrupt(
                        "invalid, duplicate or unsorted scene dialogue".into(),
                    ));
                }
                previous = Some(key);
                s.scene_dialogue.push(crate::scene::SavedSceneDialogue {
                    scene,
                    action,
                    topic,
                    info,
                    current_response,
                    queued_responses,
                    remaining,
                });
            }
            r.finish()?;
        }
        if file
            .sections
            .iter()
            .filter(|(tag, _)| tag == b"SLRP")
            .count()
            > 1
        {
            return Err(Corrupt("duplicate SLRP section".into()));
        }
        if let Some(data) = file.section(b"SLRP") {
            let mut r = Reader::new(data);
            let mut previous = None;
            for _ in 0..r.count(16)? {
                let saved = crate::scene::SavedSceneRepeat {
                    scene: r.form()?,
                    action: r.u32()?,
                    remaining: r.f64()?,
                };
                let key = (saved.scene, saved.action);
                if saved.scene.is_null()
                    || !saved.remaining.is_finite()
                    || saved.remaining < 0.0
                    || previous.is_some_and(|previous| previous >= key)
                {
                    return Err(Corrupt(
                        "invalid, duplicate or unsorted scene repeat deadline".into(),
                    ));
                }
                previous = Some(key);
                s.scene_repeats.push(saved);
            }
            r.finish()?;
        }
        if file
            .sections
            .iter()
            .filter(|(tag, _)| tag == b"SRSQ")
            .count()
            > 1
        {
            return Err(Corrupt("duplicate SRSQ section".into()));
        }
        if let Some(data) = file.section(b"SRSQ") {
            let mut r = Reader::new(data);
            for _ in 0..r.count(4)? {
                let scene = r.form()?;
                if scene.is_null() {
                    return Err(Corrupt("null pending scene restart".into()));
                }
                s.scene_restarts.push(scene);
            }
            r.finish()?;
        }
        if file
            .sections
            .iter()
            .filter(|(tag, _)| tag == b"SFGT")
            .count()
            > 1
        {
            return Err(Corrupt("duplicate SFGT section".into()));
        }
        if let Some(data) = file.section(b"SFGT") {
            let mut r = Reader::new(data);
            let mut previous = None;
            for _ in 0..r.count(4)? {
                let actor = r.form()?;
                if actor.is_null() || previous.is_some_and(|previous| previous >= actor) {
                    return Err(Corrupt(
                        "invalid, duplicate or unsorted scene force-greet marker".into(),
                    ));
                }
                previous = Some(actor);
                s.scene_force_greeted.push(actor);
            }
            r.finish()?;
        }
        if file
            .sections
            .iter()
            .filter(|(tag, _)| tag == b"RLSP")
            .count()
            > 1
        {
            return Err(Corrupt("duplicate RLSP section".into()));
        }
        if let Some(data) = file.section(b"RLSP") {
            let mut r = Reader::new(data);
            let mut previous = None;
            for _ in 0..r.count(12)? {
                let actor = r.form()?;
                let list = r.form()?;
                let key = (actor, list);
                if actor.is_null()
                    || list.is_null()
                    || previous.is_some_and(|previous| previous >= key)
                {
                    return Err(Corrupt(
                        "invalid, duplicate or unsorted rolled spell-list key".into(),
                    ));
                }
                previous = Some(key);
                let mut spells = Vec::new();
                for _ in 0..r.count(4)? {
                    let spell = r.form()?;
                    if spell.is_null() {
                        return Err(Corrupt("null spell in rolled spell-list result".into()));
                    }
                    spells.push(spell);
                }
                s.rolled_spells.push(crate::magic::SavedRolledSpell {
                    actor,
                    list,
                    spells,
                });
            }
            r.finish()?;
        }
        if file
            .sections
            .iter()
            .filter(|(tag, _)| tag == b"WSTA")
            .count()
            > 1
        {
            return Err(Corrupt("duplicate WSTA section".into()));
        }
        if let Some(data) = file.section(b"WSTA") {
            let mut r = Reader::new(data);
            let mut previous = None;
            for _ in 0..r.count(12)? {
                let actor = r.form()?;
                let state = r.i32()?;
                let remaining = r.f32()?;
                if actor.is_null()
                    || !matches!(state, 1 | 2)
                    || !remaining.is_finite()
                    || remaining <= 0.0
                    || previous.is_some_and(|previous| previous >= actor)
                {
                    return Err(Corrupt(
                        "invalid, duplicate or unsorted ward-hit state".into(),
                    ));
                }
                previous = Some(actor);
                s.ward_states.insert(actor, (state, remaining));
            }
            r.finish()?;
        }

        if let Some(data) = file.section(b"SOUL") {
            let mut r = Reader::new(data);
            for _ in 0..r.count(4)? {
                let body = r.form()?;
                if body.is_null() || !s.spent_souls.insert(body) {
                    return Err(Corrupt("null or duplicate spent-soul body".into()));
                }
            }
            r.finish()?;
        }

        let mut r = section(file, b"AVAL")?;
        for _ in 0..r.count(25)? {
            let (actor, index) = (r.form()?, r.u32()?);
            let has_base = r.bool()?;
            let base = r.f32()?;
            s.actor_values.insert(
                (actor, index),
                Modifiers {
                    base: has_base.then_some(base),
                    permanent: r.f32()?,
                    temporary: r.f32()?,
                    damage: r.f32()?,
                },
            );
        }
        for _ in 0..r.count(12)? {
            let (actor, name) = (r.form()?, r.str()?);
            s.other_actor_values.insert((actor, name), r.f32()?);
        }
        r.finish()?;

        let mut r = section(file, b"INVT")?;
        for _ in 0..r.count(4)? {
            let owner = r.form()?;
            s.inventories.insert(owner, get_inventory(&mut r)?);
        }
        r.finish()?;

        let mut r = section(file, b"SCRP")?;
        s.looted = get_forms(&mut r)?;
        for _ in 0..r.count(5)? {
            let f = r.form()?;
            s.disabled.insert(f, r.bool()?);
        }
        s.blocked_activation = get_forms(&mut r)?;
        for _ in 0..r.count(5)? {
            let f = r.form()?;
            s.locked.insert(f, r.bool()?);
        }
        for _ in 0..r.count(5)? {
            let f = r.form()?;
            s.lock_levels.insert(f, r.u8()?);
        }
        for _ in 0..r.count(9)? {
            let f = r.form()?;
            let has = r.bool()?;
            let owner = r.opt_form()?;
            s.owners.insert(f, owner.filter(|_| has));
        }
        for _ in 0..r.count(8)? {
            let actor = r.form()?;
            let mut ranks = HashMap::new();
            for _ in 0..r.count(5)? {
                let faction = r.form()?;
                ranks.insert(faction, r.u8()? as i8);
            }
            s.faction_changes.insert(actor, ranks);
        }
        r.finish()?;

        let mut r = section(file, b"WRLD")?;
        s.alerted = get_forms(&mut r)?;
        for _ in 0..r.count(21)? {
            let actor = r.form()?;
            let place = get_place(&mut r)?;
            s.dead.insert(actor, (place, r.vec3()?));
        }
        s.open_doors = get_forms(&mut r)?;
        for _ in 0..r.count(8)? {
            let actor = r.form()?;
            let mut bodies = Vec::new();
            for _ in 0..r.count(64)? {
                let mut cols = [0.0f32; 16];
                for c in &mut cols {
                    *c = r.f32()?;
                }
                bodies.push(Mat4::from_cols_array(&cols));
            }
            s.body_poses.insert(actor, bodies);
        }
        for _ in 0..r.count(24)? {
            let actor = r.form()?;
            s.wounds.insert(
                actor,
                Wounds {
                    health: r.f32()?,
                    max: r.f32()?,
                    rate: r.f32()?,
                    at: r.f64()?,
                },
            );
        }
        for _ in 0..r.count(33)? {
            let f = r.form()?;
            s.moved.insert(
                f,
                Moved {
                    place: get_place(&mut r)?,
                    pos: r.vec3()?,
                    rot: r.vec3()?,
                },
            );
        }
        for _ in 0..r.count(8)? {
            let f = r.form()?;
            s.item_charges
                .insert(f, (0..r.count(4)?).map(|_| r.f32()).collect::<Result<_>>()?);
        }
        for _ in 0..r.count(8)? {
            let f = r.form()?;
            s.broken_joints.insert(
                f,
                (0..r.count(4)?)
                    .map(|_| Ok(r.u32()? as usize))
                    .collect::<Result<_>>()?,
            );
        }
        for _ in 0..r.count(8)? {
            s.learned_effects.insert((r.form()?, r.u32()? as usize));
        }
        s.created_next = r.u32()?;
        for _ in 0..r.count(42)? {
            let id = r.form()?;
            let base = r.form()?;
            let position = r.vec3()?;
            let rotation = r.vec3()?;
            let place = if r.bool()? {
                Some(get_place(&mut r)?)
            } else {
                None
            };
            s.created.insert(
                id,
                Created {
                    base,
                    position,
                    rotation,
                    origin: None,
                    place,
                    container: r.opt_form()?,
                    location: r.opt_form()?,
                    actor: r.bool()?,
                    count: r.i32()?,
                    owner: None,
                },
            );
        }
        r.finish()?;

        if let Some(data) = file.section(b"ORIG") {
            let mut r = Reader::new(data);
            for _ in 0..r.count(28)? {
                let id = r.form()?;
                let origin = crate::created::Origin {
                    position: r.vec3()?,
                    rotation: r.vec3()?,
                };
                let Some(created) = s.created.get_mut(&id) else {
                    return Err(Corrupt(format!(
                        "origin for unknown created reference {id}"
                    )));
                };
                if created.origin.is_some()
                    || !origin.position.is_finite()
                    || !origin.rotation.is_finite()
                {
                    return Err(Corrupt(format!("invalid or duplicate origin for {id}")));
                }
                created.origin = Some(origin);
            }
            r.finish()?;
        }

        let mut r = section(file, b"PLYR")?;
        s.health = r.f32()?;
        s.stamina = r.f32()?;
        let mut skills = Skills::default();
        if r.count(4)? != skills.xp.len() {
            return Err(Corrupt("skill count".into()));
        }
        for x in &mut skills.xp {
            *x = r.f32()?;
        }
        for n in &mut skills.legendary {
            *n = r.u32()?;
        }
        skills.level = r.u16()?;
        skills.level_xp = r.f32()?;
        skills.pending = r.u32()?;
        skills.perk_points = r.u32()?;
        skills.trained = r.u32()?;
        skills.read_books = get_forms(&mut r)?;
        skills.books_seen = get_forms(&mut r)?;
        s.skills = Some(skills);
        s.perks_added = get_perks(&mut r)?;
        s.perks_removed = get_form_sets(&mut r)?;
        s.spells_added = get_form_lists(&mut r)?;
        s.spells_removed = get_form_sets(&mut r)?;
        s.words_known = get_forms(&mut r)?;
        s.words_unlocked = get_forms(&mut r)?;
        s.shouts_added = get_form_lists(&mut r)?;
        s.shouts_removed = get_form_sets(&mut r)?;
        for _ in 0..r.count(4)? {
            s.favorites.push(r.form()?);
        }
        for h in &mut s.hotkeys {
            *h = r.opt_form()?;
        }
        s.infos_said = get_forms(&mut r)?;
        s.talked_to_pc = get_forms(&mut r)?;
        r.finish()?;

        let mut r = section(file, b"CRIM")?;
        for _ in 0..r.count(20)? {
            let faction = r.form()?;
            s.bounties.insert(
                faction,
                crate::crime::Bounty {
                    violent: r.i32()?,
                    nonviolent: r.i32()?,
                    infamy_violent: r.i32()?,
                    infamy_nonviolent: r.i32()?,
                },
            );
        }
        for _ in 0..r.count(12)? {
            let faction = r.form()?;
            s.stolen_value.insert(faction, (r.i32()?, r.i32()?));
        }
        r.finish()?;

        let mut r = section(file, b"BART")?;
        for _ in 0..r.count(12)? {
            let chest = r.form()?;
            s.restocked.insert(chest, r.f64()?);
        }
        r.finish()?;
        // Added sections are optional so older native saves remain readable.
        if let Some(data) = file.section(b"CPUB") {
            let mut r = Reader::new(data);
            for _ in 0..r.count(5)? {
                s.cell_public.insert(r.form()?, r.bool()?);
            }
            r.finish()?;
        }
        if let Some(data) = file.section(b"STOR") {
            let mut r = Reader::new(data);
            for _ in 0..r.count(12)? {
                s.story.last_run.insert(r.form()?, r.f64()?);
            }
            s.story.ran = get_form_sets(&mut r)?;
            for _ in 0..r.count(12)? {
                s.story.next_social.insert(r.form()?, r.f64()?);
            }
            if r.bool()? {
                s.story.last_location = Some(r.opt_form()?);
            }
            for _ in 0..r.count(8)? {
                s.story.found_bodies.insert((r.form()?, r.form()?));
            }
            s.story.next_body_check = r.f64()?;
            r.finish()?;
        }
        if let Some(data) = file.section(b"RELA") {
            let mut r = Reader::new(data);
            for _ in 0..r.count(12)? {
                let (a, b, rank) = (r.form()?, r.form()?, r.i32()?);
                if a > b || !(-4..=4).contains(&rank) {
                    return Err(Corrupt("invalid relationship pair or rank".into()));
                }
                s.relationship_ranks.insert((a, b), rank);
            }
            r.finish()?;
        }
        // Optional: saves from before actors' open doors had none.
        if let Some(data) = file.section(b"ADOR") {
            let mut r = Reader::new(data);
            s.actor_open_doors = get_forms(&mut r)?;
            r.finish()?;
        }
        if let Some(data) = file.section(b"DSTR") {
            let mut r = Reader::new(data);
            for _ in 0..r.count(16)? {
                let reference = r.form()?;
                let health = r.f64()?;
                let stage = r.i32()?;
                if reference.is_null()
                    || !health.is_finite()
                    || !(0.0..=u32::MAX as f64).contains(&health)
                    || !(-1..=u8::MAX as i32).contains(&stage)
                    || s.destruction
                        .insert(reference, DestructionState { health, stage })
                        .is_some()
                {
                    return Err(Corrupt(
                        "invalid or duplicate destructible reference state".into(),
                    ));
                }
            }
            r.finish()?;
        }
        if let Some(data) = file.section(b"WTHR") {
            let mut r = Reader::new(data);
            let weather = SavedWeather {
                current: r.opt_form()?,
                outgoing: r.opt_form()?,
                pct: r.f32()?,
                overridden: r.bool()?,
                before_override: r.opt_form()?,
                next_roll: r.f32()?,
            };
            r.finish()?;
            if !(0.0..=1.0).contains(&weather.pct)
                || !weather.next_roll.is_finite()
                || (weather.outgoing.is_some() && weather.current.is_none())
            {
                return Err(Corrupt("invalid weather transition or roll".into()));
            }
            s.weather = weather;
        }
        if let Some(data) = file.section(b"DETH") {
            let mut r = Reader::new(data);
            for _ in 0..r.count(16)? {
                let actor = r.form()?;
                let death = crate::world_state::Death {
                    killer: r.opt_form()?,
                    at: r.f64()?,
                };
                if !death.at.is_finite() || s.deaths.insert(actor, death).is_some() {
                    return Err(Corrupt("invalid or duplicate death".into()));
                }
            }
            r.finish()?;
        }
        if let Some(data) = file.section(b"STPT") {
            let mut r = Reader::new(data);
            for _ in 0..r.count(12)? {
                let actor = r.form()?;
                let points = r.f32()?;
                let regen_wait = r.f32()?;
                if actor.is_null()
                    || !points.is_finite()
                    || !(0.0..=100.0).contains(&points)
                    || !regen_wait.is_finite()
                    || regen_wait < 0.0
                    || s.stealth_points
                        .insert(actor, (points, regen_wait))
                        .is_some()
                {
                    return Err(Corrupt("invalid or duplicate stealth-point state".into()));
                }
            }
            r.finish()?;
        }
        if let Some(data) = file.section(b"COWN") {
            let mut r = Reader::new(data);
            for _ in 0..r.count(8)? {
                let id = r.form()?;
                let owner = r.form()?;
                match s.created.get_mut(&id) {
                    Some(c) if c.owner.is_none() && !owner.is_null() => c.owner = Some(owner),
                    _ => return Err(Corrupt("invalid dropped item owner".into())),
                }
            }
            r.finish()?;
        }
        s.vm = super::vm::VmSnapshot::read(file)?;
        Ok(s)
    }

    /// Put this state into the engine, in place of its own, and go where the
    /// player stood. The engine must have the plugins the save was made with.
    pub fn apply(self, e: &mut Engine) -> anyhow::Result<()> {
        let have: Vec<String> =
            e.lo.plugins()
                .iter()
                .map(|p| p.plugin.name().to_string())
                .collect();
        anyhow::ensure!(
            have == self.plugins,
            "the save was made with other plugins ({} against {} loaded)",
            self.plugins.len(),
            have.len()
        );
        anyhow::ensure!(
            self.real_time.is_finite() && self.real_time >= 0.0,
            "save has invalid real-time clock"
        );
        anyhow::ensure!(
            self.wall_time.is_finite() && self.wall_time >= 0.0,
            "save has invalid wall-time clock"
        );
        let weapon_usage =
            crate::stats::WeaponUsage::restore(self.weapon_usage, self.favorite_weapon)?;
        for (weapon, _) in weapon_usage.saved().0 {
            anyhow::ensure!(
                crate::stats::WeaponUsage::valid_form(&e.lo, weapon),
                "save has invalid favorite-weapon form {weapon}"
            );
        }
        for (power, last_used) in &self.power_uses {
            anyhow::ensure!(
                !power.is_null()
                    && last_used.is_finite()
                    && *last_used >= 0.0
                    && e.lo.get(*power).is_some_and(|r| r.tag().0 == *b"SPEL")
                    && e.magic_item(*power)
                        .is_some_and(|item| { item.spell_type == crate::magic::spell_type::POWER }),
                "save has invalid Greater Power cooldown for {power}"
            );
        }
        anyhow::ensure!(
            self.spent_souls.iter().all(|body| !body.is_null()),
            "save has a null spent-soul body"
        );
        for f in [
            self.weather.current,
            self.weather.outgoing,
            self.weather.before_override,
        ]
        .into_iter()
        .flatten()
        {
            anyhow::ensure!(
                e.lo.get(f).is_some_and(|r| r.tag().0 == *b"WTHR"),
                "save has invalid weather form {f}"
            );
        }
        e.validate_saved_scenes(
            self.real_time,
            &self.scenes,
            &self.scene_lifecycle,
            &self.scene_bindings,
            &self.scene_dialogue,
            &self.scene_repeats,
            &self.scene_restarts,
            &self.scene_force_greeted,
        )?;
        e.validate_saved_rolled_spells(&self.rolled_spells)?;
        anyhow::ensure!(
            self.ward_states.iter().all(|(actor, (state, remaining))| {
                !actor.is_null()
                    && matches!(state, 1 | 2)
                    && remaining.is_finite()
                    && *remaining > 0.0
            }),
            "save has invalid ward-hit state"
        );
        // Leave the old game's cells before the world state is replaced: their
        // unloading writes into it.
        e.discard_conversation_for_load();
        e.unload_all();
        e.waiting = None;
        if matches!(e.menu, Some(crate::items::Menu::SleepWait { .. })) {
            e.menu = None;
        }
        e.clear_sleep_imagespace();
        e.story.restore(self.story);
        e.relationships.restore_ranks(self.relationship_ranks);
        e.day = self.day;
        e.hour = self.hour;
        let s = &mut e.scripts;
        s.real_time = self.real_time;
        s.wall_time = self.wall_time;
        s.globals = self.globals;
        s.quests = self.quests;
        s.stats = self.stats;
        s.weapon_usage = weapon_usage;
        s.actor_values = self.actor_values;
        s.other_actor_values = self.other_actor_values;
        s.looted = self.looted;
        s.disabled = self.disabled;
        s.blocked_activation = self.blocked_activation;
        s.locked = self.locked;
        s.lock_levels = self.lock_levels;
        s.owners = self.owners;
        s.faction_changes = self.faction_changes;
        // What was queued or running before the load belongs to the old game.
        s.pending_stages.clear();
        s.pending_events.clear();
        s.pending_quest_inits.clear();
        s.initialized = self.vm.initialized.into_iter().collect();
        s.timers = self.vm.timers;
        s.sleep_listeners = self.vm.sleep_listeners;
        s.inventory_filters = self
            .vm
            .inventory_filters
            .into_iter()
            .map(|(o, items)| (o, items.into_iter().collect()))
            .collect();
        s.visibility_dirty = true;
        s.alias_gen += 1;
        // The scripts attached with their variables and the threads waiting,
        // before cells load and attach their own (attaching keeps what is there).
        let mut vm = std::mem::take(&mut e.vm);
        vm.time = e.scripts.real_time;
        vm.menu_time = e.scripts.wall_time;
        let dropped = {
            let mut host = crate::script::EngineHost { engine: e };
            vm.import(&mut host, self.vm.vm)
        };
        e.vm = vm;
        e.restore_message_boxes(self.vm.message_boxes, self.vm.message_next_key);
        let animation_waits = self
            .vm
            .animation_waits
            .iter()
            .filter(|(key, _, _)| e.vm.is_waiting_for(*key))
            .map(|(_, sender, event)| (*sender, event.clone()))
            .collect();
        e.anim_events
            .restore(self.vm.anim_registrations, animation_waits);
        if dropped > 0 {
            log::warn!("{dropped} saved script threads can't be resumed");
        }
        e.inventories = self.inventories;
        e.casting.restore_power_uses(self.power_uses);
        e.magic.souls_taken = self.spent_souls;
        e.fast_travel_disabled = self.fast_travel_disabled;
        e.map_markers.shown = self.map_shown;
        e.map_markers.discovered = self.map_discovered;
        e.map_markers.travel = self.map_travel;
        e.map_markers.denied = self.map_denied;
        e.restore_rolled_spells(self.rolled_spells);
        e.magic.ward_states = self.ward_states;
        let ws = &mut e.world_state;
        ws.alerted = self.alerted;
        ws.dead = self.dead;
        ws.deaths = self.deaths;
        ws.open_doors = self.open_doors;
        ws.actor_open_doors = self.actor_open_doors;
        ws.body_poses = self.body_poses;
        ws.wounds = self.wounds;
        ws.moved = self.moved;
        ws.item_charges = self.item_charges;
        ws.broken_joints = self.broken_joints;
        ws.destruction = self.destruction;
        ws.object_graphs.clear();
        ws.graph_open.clear();
        ws.learned_effects = self.learned_effects;
        e.detection.restore_stealth_points(self.stealth_points);
        e.created_refs.refs = self.created;
        e.created_refs.next = self.created_next;
        e.player_health = self.health;
        e.player_stamina = self.stamina;
        e.skills = self.skills.unwrap_or_default();
        e.perks.replace(self.perks_added, self.perks_removed);
        e.magic.added = self.spells_added;
        e.magic.removed = self.spells_removed;
        e.shouts.known = self.words_known;
        e.shouts.unlocked = self.words_unlocked;
        e.shouts.added = self.shouts_added;
        e.shouts.removed = self.shouts_removed;
        e.favorites = self.favorites;
        e.hotkeys = self.hotkeys;
        e.infos_said = self.infos_said;
        e.talked_to_pc = self.talked_to_pc;
        e.crime.bounties = self.bounties;
        e.crime.stolen_value = self.stolen_value;
        e.restocks.stocked = self.restocked;
        e.scripts.cell_public = self.cell_public;
        // The saved weather replaces the launch's `--weather` (taken when the
        // player first goes outdoors) and any the old game had.
        if self.weather.current.is_some() {
            e.forced_weather = None;
        }
        e.restore_weather(self.weather);
        e.restore_scenes(
            self.scenes,
            self.scene_lifecycle,
            self.scene_bindings,
            self.scene_dialogue,
            self.scene_repeats,
            self.scene_restarts,
            self.scene_force_greeted,
        );
        match self.spot {
            Some(Spot {
                location: Location::Interior(cell),
                feet,
                yaw,
            }) => e.enter_interior(cell, Some((feet, yaw)))?,
            Some(Spot {
                location: Location::Exterior { world, .. },
                feet,
                yaw,
            }) => e.enter_exterior(world, feet, yaw)?,
            _ => {}
        }
        Ok(())
    }
}
