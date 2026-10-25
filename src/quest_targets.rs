//! Quest targets: where the tracked quests' objectives point. Each objective
//! (`QOBJ`) names its targets (`QSTA`: a reference alias, flags, and the
//! conditions after it); the compass marks those of the displayed, unfinished
//! objectives of the active quests: the target itself when it is in the
//! player's interior or worldspace, else the load door the way there starts
//! at (the shortest way through the data's load doors).
//! Choices: `known_ambiguities/quest-targets.md`.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use esp::FormId;
use glam::Vec3;

use crate::ai::schedule::Place;
use crate::condition::Condition;
use crate::engine::{Engine, Location, PLAYER_REF};
use crate::map_markers::{CompassMarker, frame, heading};

/// Seconds between workings out of where the targets are.
const UPDATE_INTERVAL: f32 = 0.5;

/// An interior, or a worldspace: somewhere positions compare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Space {
    Interior(FormId),
    World(FormId),
}

impl From<Place> for Space {
    fn from(p: Place) -> Space {
        match p {
            Place::Interior(c) => Space::Interior(c),
            Place::Exterior(w, _) => Space::World(w),
        }
    }
}

/// A load door: where it stands and where it leads.
#[derive(Debug, Clone, Copy)]
struct DoorLink {
    door: FormId,
    pos: Vec3,
    to: Space,
}

/// An objective's target: the alias and the conditions on it.
#[derive(Debug, Clone)]
struct Target {
    objective: i32,
    alias: u32,
    conditions: Vec<Condition>,
}

/// A spot the compass marks: the target, or the door towards it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuestSpot {
    pub pos: Vec3,
    pub door: bool,
}

#[derive(Default)]
pub struct QuestTargets {
    /// Every load door, by the space it stands in (read on first use).
    doors: std::cell::OnceCell<HashMap<Space, Vec<DoorLink>>>,
    /// Each quest's objective targets.
    targets: HashMap<FormId, Arc<Vec<Target>>>,
    /// The spots worked out last.
    pub spots: Vec<QuestSpot>,
    next: f32,
}

fn space_of_cell(lo: &esp::LoadOrder, cell: FormId) -> Option<Space> {
    Some(match lo.cell(cell)?.world {
        Some(w) => Space::World(w),
        None => Space::Interior(cell),
    })
}

fn read_doors(lo: &esp::LoadOrder) -> HashMap<Space, Vec<DoorLink>> {
    let mut out: HashMap<Space, Vec<DoorLink>> = HashMap::new();
    let mut n = 0;
    for &r in lo.ids_of_type(b"REFR") {
        let Some(rec) = lo.get(r) else { continue };
        let Some(d) = rec.get(b"XTEL").filter(|d| d.len() >= 4) else {
            continue;
        };
        let dest = rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap())));
        let from = lo.cell_of_ref(r).and_then(|c| space_of_cell(lo, c));
        let to = lo.cell_of_ref(dest).and_then(|c| space_of_cell(lo, c));
        let (Some(from), Some(to)) = (from, to) else {
            continue;
        };
        if from == to {
            continue;
        }
        let pos = crate::world::records::reference(&rec).position;
        out.entry(from)
            .or_default()
            .push(DoorLink { door: r, pos, to });
        n += 1;
    }
    log::info!("{n} load doors between spaces");
    out
}

/// The objectives' targets of a quest record, in order.
fn read_targets(lo: &esp::LoadOrder, q: FormId) -> Vec<Target> {
    let Some(rec) = lo.get(q) else {
        return Vec::new();
    };
    let mut out: Vec<Target> = Vec::new();
    let mut objective = None;
    // Conditions belong to the target before them.
    let mut in_target = false;
    for sr in rec.subrecords() {
        match &sr.tag.0 {
            b"QOBJ" => {
                objective = sr
                    .data
                    .get(0..2)
                    .map(|b| u16::from_le_bytes([b[0], b[1]]) as i32);
                in_target = false;
            }
            b"QSTA" if sr.data.len() >= 4 => {
                let alias = i32::from_le_bytes(sr.data[0..4].try_into().unwrap());
                in_target = false;
                if let Some(objective) = objective
                    && alias >= 0
                {
                    out.push(Target {
                        objective,
                        alias: alias as u32,
                        conditions: Vec::new(),
                    });
                    in_target = true;
                }
            }
            b"CTDA" if in_target => {
                if let Some(c) = crate::condition::parse(&rec, sr.data)
                    && let Some(t) = out.last_mut()
                {
                    t.conditions.push(c);
                }
            }
            b"ANAM" | b"ALST" | b"ALLS" => break,
            _ => {}
        }
    }
    out
}

impl Engine {
    /// The interior or worldspace the player is in.
    pub fn player_space(&self) -> Option<Space> {
        match self.location {
            Location::Interior(c) => Some(Space::Interior(c)),
            Location::Exterior { world, .. } => Some(Space::World(world)),
            Location::Nowhere => None,
        }
    }

    /// Where a reference is now, and in which space: an actor where it is
    /// or its schedule has it, what's in a container where the container
    /// is. Nothing for the player and what they carry, or what's disabled.
    fn target_spot(&self, r: FormId) -> Option<(Space, Vec3)> {
        let mut r = r;
        for _ in 0..4 {
            if r == PLAYER_REF || self.is_disabled(r) {
                return None;
            }
            match self.created(r).and_then(|c| c.container) {
                Some(holder) => r = holder,
                None => break,
            }
        }
        if let Some(a) = self.actor_ref(r) {
            let key = self.actor_cells.get(&r)?;
            return Some((self.place_of_key(*key)?.into(), a.exec.pos));
        }
        if let Some(&(place, pos)) = self.whereabouts.of.get(&r) {
            return Some((place.into(), pos));
        }
        let pos = self.ref_position(r)?;
        Some((self.place_of_ref(r, pos)?.into(), pos))
    }

    /// The first load door on the shortest way (fewest doors) from the
    /// player's space to `to`; of those as short, the nearest.
    fn door_on_way(&self, from: Space, to: Space, player: Vec3) -> Option<Vec3> {
        let doors = self
            .quest_targets
            .doors
            .get_or_init(|| read_doors(&self.lo));
        // Breadth first, each space remembering the first door taken.
        let mut first: HashMap<Space, Vec3> = HashMap::new();
        let mut seen: HashSet<Space> = HashSet::from([from]);
        let mut queue: VecDeque<Space> = VecDeque::new();
        let mut starts: Vec<&DoorLink> = doors
            .get(&from)
            .map(|v| v.iter().collect())
            .unwrap_or_default();
        starts.sort_by(|a, b| a.pos.distance(player).total_cmp(&b.pos.distance(player)));
        for d in starts {
            if self.is_disabled(d.door) || !seen.insert(d.to) {
                continue;
            }
            if d.to == to {
                return Some(d.pos);
            }
            first.insert(d.to, d.pos);
            queue.push_back(d.to);
        }
        while let Some(s) = queue.pop_front() {
            let start = first[&s];
            for d in doors.get(&s).into_iter().flatten() {
                if self.is_disabled(d.door) || !seen.insert(d.to) {
                    continue;
                }
                if d.to == to {
                    return Some(start);
                }
                first.insert(d.to, start);
                queue.push_back(d.to);
            }
        }
        None
    }

    /// Shared objective/alias/condition selection for the map and compass.
    fn active_quest_references(&mut self) -> Vec<(FormId, FormId)> {
        let mut quests: Vec<(FormId, Vec<i32>)> = self
            .scripts
            .quests
            .iter()
            .filter(|(_, st)| st.active && st.running && !st.completed && !st.failed)
            .map(|(&q, st)| {
                let open = st
                    .objectives_displayed
                    .iter()
                    .filter(|o| {
                        !st.objectives_completed.contains(o) && !st.objectives_failed.contains(o)
                    })
                    .copied()
                    .collect();
                (q, open)
            })
            .collect();
        quests.sort_by_key(|(q, _)| *q);
        let mut refs = Vec::new();
        for (q, open) in quests {
            let targets = match self.quest_targets.targets.get(&q) {
                Some(t) => t.clone(),
                None => {
                    let t = Arc::new(read_targets(&self.lo, q));
                    self.quest_targets.targets.insert(q, t.clone());
                    t
                }
            };
            for t in targets.iter().filter(|t| open.contains(&t.objective)) {
                let Some(r) = self.alias_ref(q, t.alias) else {
                    continue;
                };
                let ctx = crate::condition::Context {
                    subject: Some(r),
                    quest: Some(q),
                    ..Default::default()
                };
                if crate::condition::evaluate(self, &t.conditions, ctx) && !refs.contains(&(q, r)) {
                    refs.push((q, r));
                }
            }
        }
        refs
    }

    /// Twice a second: resolve tracked references or the first door towards them.
    pub(crate) fn update_quest_targets(&mut self, dt: f32) {
        self.quest_targets.next -= dt;
        if self.quest_targets.next > 0.0 {
            return;
        }
        self.quest_targets.next = UPDATE_INTERVAL;
        let (Some(here), player) = (self.player_space(), self.player.position) else {
            self.quest_targets.spots.clear();
            return;
        };
        let mut spots = Vec::new();
        for (_, r) in self.active_quest_references() {
            let Some((space, pos)) = self.target_spot(r) else {
                continue;
            };
            let spot = if space == here {
                QuestSpot { pos, door: false }
            } else {
                let Some(pos) = self.door_on_way(here, space, player) else {
                    continue;
                };
                QuestSpot { pos, door: true }
            };
            if !spots.contains(&spot) {
                spots.push(spot);
            }
        }
        if spots != self.quest_targets.spots {
            log::debug!("quest targets: {spots:?}");
        }
        self.quest_targets.spots = spots;
    }

    /// Map overlays use the movie's quest icon (62) and door icon (61).
    /// They have no travel identity; only actual destination markers can travel.
    pub(crate) fn map_quest_rows(&mut self, world: FormId) -> Vec<crate::map_view::MapRow> {
        let origin = match self.location {
            Location::Exterior { world: here, .. } => {
                crate::map_view::world_map_point(&self.lo, here, self.player.position.truncate())
                    .filter(|(owner, _)| *owner == world)
                    .map(|(_, p)| p.extend(0.0))
            }
            _ => None,
        }
        .unwrap_or(Vec3::ZERO);
        let mut rows: Vec<crate::map_view::MapRow> = Vec::new();
        for (q, r) in self.active_quest_references() {
            let Some((space, pos)) = self.target_spot(r) else {
                continue;
            };
            let direct = match space {
                Space::World(here) => {
                    crate::map_view::world_map_point(&self.lo, here, pos.truncate())
                        .filter(|(owner, _)| *owner == world)
                        .map(|(_, p)| p)
                }
                _ => None,
            };
            let (pos, kind) = match direct {
                Some(p) => (p, 62),
                None => {
                    let Some(p) = self.door_on_way(Space::World(world), space, origin) else {
                        continue;
                    };
                    (p.truncate(), 61)
                }
            };
            if !pos.is_finite() || rows.iter().any(|r| r.pos == pos && r.kind == kind) {
                continue;
            }
            let mut name = self.quest_name(q);
            if name.is_empty() {
                name = self
                    .gmst_string("sQuestTarget")
                    .unwrap_or_else(|| "Quest target".into());
            }
            rows.push(crate::map_view::MapRow {
                marker: None,
                name,
                kind,
                undiscovered: false,
                pos,
                rotation: 0.0,
            });
        }
        rows
    }

    /// The quest targets on the compass.
    pub fn compass_quest_markers(&self) -> Vec<CompassMarker> {
        let player = self.player.position;
        self.quest_targets
            .spots
            .iter()
            .map(|s| CompassMarker {
                heading: heading(player, s.pos),
                alpha: 100.0,
                frame: if s.door {
                    frame::QUEST_DOOR
                } else {
                    frame::QUEST
                },
                scale: 100.0,
            })
            .collect()
    }
}
