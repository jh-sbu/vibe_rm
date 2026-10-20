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
//! and `VM` / `VMAR` (`super::vm`: the Papyrus VM). Not yet saved:
//! active magic effects, scenes, AI progress, objects'
//! behaviour graphs; see `plans/ROADMAP.md`.
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
    pub open_doors: HashSet<FormId>,
    pub body_poses: HashMap<FormId, Vec<Mat4>>,
    pub wounds: HashMap<FormId, Wounds>,
    pub moved: HashMap<FormId, Moved>,
    pub item_charges: HashMap<FormId, Vec<f32>>,
    pub broken_joints: HashMap<FormId, Vec<usize>>,
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
            open_doors: ws.open_doors.clone(),
            body_poses: ws.body_poses.clone(),
            wounds: ws.wounds.clone(),
            moved: ws.moved.clone(),
            item_charges: ws.item_charges.clone(),
            broken_joints: ws.broken_joints.clone(),
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
                    place,
                    container: r.opt_form()?,
                    location: r.opt_form()?,
                    actor: r.bool()?,
                    count: r.i32()?,
                },
            );
        }
        r.finish()?;

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
        // Leave the old game's cells before the world state is replaced: their
        // unloading writes into it.
        e.unload_all();
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
        e.anim_events.restore(self.vm.anim_registrations);
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
        if dropped > 0 {
            log::warn!("{dropped} saved script threads can't be resumed");
        }
        e.inventories = self.inventories;
        let ws = &mut e.world_state;
        ws.alerted = self.alerted;
        ws.dead = self.dead;
        ws.open_doors = self.open_doors;
        ws.body_poses = self.body_poses;
        ws.wounds = self.wounds;
        ws.moved = self.moved;
        ws.item_charges = self.item_charges;
        ws.broken_joints = self.broken_joints;
        ws.object_graphs.clear();
        ws.graph_open.clear();
        ws.learned_effects = self.learned_effects;
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
