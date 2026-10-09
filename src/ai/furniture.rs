//! Furniture actors can use (chairs, benches, beds, bedrolls, wall-lean markers):
//! marker positions from the model, ownership, reservations, and the clips to
//! enter, idle in and leave each kind (picked from the idle tree).

use std::collections::HashMap;
use std::sync::Arc;

use esp::{FormId, LoadOrder};
use glam::{Mat4, Vec3};

use crate::render::CellKey;
use crate::world::animation::BoundClip;
use crate::world::records;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Use {
    Sit,
    Sleep,
    Lean,
    /// Furniture with its own idles (crafting stations, special chairs), entered
    /// through the behaviour graph event from its keyword's idle tree.
    Special,
    /// An idle marker (`IDLM`): stand there and play one of its idles.
    Idle,
}

/// Side an actor gets on from (marker entry flags).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Entry {
    Front,
    Behind,
    Right,
    Left,
}

impl Entry {
    pub const ALL: [(Entry, u16); 4] = [
        (Entry::Front, 1),
        (Entry::Behind, 2),
        (Entry::Right, 4),
        (Entry::Left, 8),
    ];

    /// `IsFurnitureEntryType` value of the side.
    pub fn entry_type(self) -> u32 {
        Self::ALL
            .iter()
            .find(|(e, _)| *e == self)
            .map_or(0, |(_, bit)| u32::from(*bit) << 16)
    }

    /// Sides a marker can be used from: its entry flags, or the front if it has none.
    pub fn of(entries: u16) -> impl Iterator<Item = (Entry, u16)> {
        Self::ALL.into_iter().filter(move |&(e, bit)| {
            entries & bit != 0 || (entries & 0xF == 0 && e == Entry::Front)
        })
    }
}

/// How to get on and off one marker from one side: the behaviour events picked
/// from the idle tree (`ActivateRootChar`) for adults or children, or for a
/// creature's graph from its branch of `ActionActivate` (draugr sarcophagi,
/// thrones and alcoves).
#[derive(Debug, Clone)]
pub struct Way {
    pub marker: u8,
    pub entry: Entry,
    pub child: bool,
    pub enter: String,
    pub exit: Option<String>,
    /// The creature project the events are for (none: humanoids).
    pub graph: Option<String>,
    /// A way only for users holding an item, or only for those without it (a
    /// wood pile's put-down and pick-up).
    pub holding: Option<(FormId, bool)>,
    /// What leaving it at once plays (`IsExitingInstant`): the default state, or
    /// carrying the load away (`OffsetCarryLogStart` from a wood pile).
    pub instant_exit: Option<String>,
}

/// One usable position, in world space.
#[derive(Debug, Clone, Copy)]
pub struct Marker {
    /// Where the actor's feet go (the furniture's floor level under the marker).
    pub pos: Vec3,
    /// Facing while in use, radians clockwise from +Y.
    pub heading: f32,
    pub kind: Use,
    pub entries: u16,
    /// The model marker's animation type (`IsFurnitureAnimType`: 1 sit, 2 lay, 4 lean).
    pub anim_type: u32,
}

pub struct Furniture {
    pub ref_id: FormId,
    pub cell: CellKey,
    pub owner: Option<FormId>,
    pub markers: Vec<Marker>,
    /// Behaviour events played here: the enter event of special furniture, or an
    /// idle marker's idles.
    pub events: Vec<String>,
    /// Seconds an idle marker is used for (0: package default).
    pub idle_time: f32,
    /// Furniture markers: the ways on and off (filled in after loading).
    pub ways: Vec<Way>,
}

/// Furniture in the loaded cells and who is using (or heading for) each marker.
#[derive(Default)]
pub struct FurnitureWorld {
    pub items: Vec<Furniture>,
    users: HashMap<(FormId, u8), FormId>,
    /// Furniture that is disabled now (not offered).
    disabled: std::collections::HashSet<FormId>,
}

/// A reserved marker and how to use it.
#[derive(Clone)]
pub struct Seat {
    pub furniture: FormId,
    pub kind: Use,
    pub anim_type: u32,
    pub entry: Entry,
    pub pos: Vec3,
    pub heading: f32,
    pub clips: Arc<UseClips>,
    /// Seconds to stay once in (infinite while the package lasts).
    pub duration: f32,
}

impl Seat {
    /// Pose (feet, heading) at which the enter animation starts.
    pub fn enter_start(&self) -> (Vec3, f32) {
        let m = Marker {
            pos: self.pos,
            heading: self.heading,
            kind: Use::Sit,
            entries: 0,
            anim_type: 1,
        };
        enter_start(&m, &self.clips.enter)
    }
}

/// What using a marker (or playing an idle) will look like, worked out ahead from
/// the behaviour graphs: the event that starts it and the clips it plays to get
/// in, hold and get out (with the events that end it). The graph plays them; the
/// AI uses them to plan where the actor must start and how long each part takes.
pub struct UseClips {
    pub event: String,
    /// Events that may end it, in order of preference.
    pub exits: Vec<String>,
    pub enter: Vec<Arc<BoundClip>>,
    pub idle: Arc<BoundClip>,
    pub exit: Vec<Arc<BoundClip>>,
    /// False when the last clip is a one-shot (a gesture rather than a pose to hold).
    pub idle_loops: bool,
    /// The event that leaves at once after a one-shot (see [`Way::instant_exit`]).
    pub instant_exit: Option<String>,
}

impl UseClips {
    pub fn enter_time(&self) -> f32 {
        self.enter.iter().map(|c| c.duration()).sum()
    }

    pub fn exit_time(&self) -> f32 {
        self.exit.iter().map(|c| c.duration()).sum()
    }
}

/// Rotate an actor-space offset (+Y forward, +X right) into the world by `heading`.
pub fn to_world(offset: Vec3, heading: f32) -> Vec3 {
    let (s, c) = heading.sin_cos();
    Vec3::new(
        offset.x * c + offset.y * s,
        -offset.x * s + offset.y * c,
        offset.z,
    )
}

/// Pose (feet, heading) at which to start the `enter` clips so that they end on the marker.
pub fn enter_start(m: &Marker, enter: &[Arc<BoundClip>]) -> (Vec3, f32) {
    // Each clip moves the actor relative to where the previous one left it.
    // Motion yaw is counter-clockwise; headings are clockwise.
    let ends: Vec<(Vec3, f32)> = enter
        .iter()
        .filter_map(|c| c.motion.as_ref())
        .map(|mo| mo.end())
        .collect();
    let total_yaw: f32 = ends.iter().map(|e| e.1).sum();
    let h0 = m.heading + total_yaw;
    let mut h = h0;
    let mut offset = Vec3::ZERO;
    for (t, yaw) in ends {
        offset += to_world(t, h);
        h -= yaw;
    }
    (m.pos - offset, h0)
}

/// Owner of a reference (`XOWN`), falling back to its cell's owner.
pub(crate) fn owner_of(lo: &LoadOrder, r: FormId) -> Option<FormId> {
    let own = |rec: &esp::LoadedRecord<'_>| {
        rec.get(b"XOWN")
            .filter(|d| d.len() >= 4)
            .map(|d| rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap()))))
    };
    let rec = lo.get(r)?;
    own(&rec)
        .or_else(|| lo.get(lo.cell_of_ref(r)?).and_then(|c| own(&c)))
        .filter(|f| !f.is_null())
}

/// Whether `npc` may use `kind` of furniture owned by `owner`. Beds must be its own
/// (or its faction's, or nobody's); seats are only off limits when they belong to
/// another person, since inns and halls own all their chairs.
pub fn may_use(lo: &LoadOrder, npc: FormId, kind: Use, owner: Option<FormId>) -> bool {
    let Some(owner) = owner else { return true };
    if owner == npc {
        return true;
    }
    let faction_owned = lo.get(owner).is_some_and(|r| r.tag().0 == *b"FACT");
    if kind != Use::Sleep {
        return faction_owned;
    }
    let Some(rec) = lo.get(npc) else { return false };
    faction_owned
        && rec
            .subrecords()
            .filter(|s| s.tag.0 == *b"SNAM" && s.data.len() >= 4)
            .any(|s| rec.fid(s.form_id(0)) == owner)
}

impl FurnitureWorld {
    /// Register the furniture among a cell's references. `markers` returns the
    /// model's furniture markers for a model path.
    pub fn add_cell(
        &mut self,
        lo: &LoadOrder,
        idles: &super::idles::IdleIndex,
        key: CellKey,
        refs: &[FormId],
        mut markers: impl FnMut(&str) -> Option<Arc<[nif::FurnitureMarker]>>,
    ) {
        let furniture_special = lo.find_editor_id("FurnitureSpecial");
        for &r in refs {
            let Some(rec) = lo.get(r) else { continue };
            if rec.tag().0 != *b"REFR" {
                continue;
            }
            let rf = records::reference(&rec);
            if rf.deleted() {
                continue;
            }
            let Some(base) = lo.get(rf.base) else {
                continue;
            };
            if base.tag().0 == *b"IDLM" {
                self.add_idle_marker(lo, idles, key, r, &rf, &base);
                continue;
            }
            if base.tag().0 != *b"FURN" {
                continue;
            }
            // Furniture whose keywords select an idle tree (crafting stations,
            // thrones, writing desks...) is entered through that idle's event.
            let special = idles.furniture_event(lo, &base);
            // Wood piles, pour spots, levers...: their own idles, but no keyword idle event.
            let keyword_special = furniture_special.is_some_and(|kw| {
                base.get(b"KWDA")
                    .unwrap_or(&[])
                    .chunks_exact(4)
                    .any(|c| base.fid(FormId(u32::from_le_bytes(c.try_into().unwrap()))) == kw)
            });
            let bench = base
                .get(b"WBDT")
                .is_some_and(|d| !d.is_empty() && d[0] != 0);
            if bench && special.is_none() {
                continue;
            }
            let Some(model) = records::model_path(&base) else {
                continue;
            };
            let Some(nif_markers) = markers(&model) else {
                continue;
            };
            // Active markers: bits 0..23 of MNAM (none set means all).
            let active = base
                .get(b"MNAM")
                .filter(|d| d.len() >= 4)
                .map(|d| u32::from_le_bytes(d[0..4].try_into().unwrap()) & 0xFF_FFFF);
            let transform: Mat4 = rf.transform();
            let ref_fwd = transform.transform_vector3(Vec3::Y);
            let ref_heading = ref_fwd.x.atan2(ref_fwd.y);
            let list: Vec<Marker> = nif_markers
                .iter()
                .enumerate()
                .filter(|(i, _)| active.is_none_or(|a| a == 0 || a & (1 << i) != 0))
                .filter_map(|(_, m)| {
                    let kind = match m.anim_type {
                        _ if special.is_some() => Use::Special,
                        // Wall-lean markers carry FurnitureSpecial too but are leans.
                        4 => Use::Lean,
                        _ if keyword_special => Use::Special,
                        1 => Use::Sit,
                        2 => Use::Sleep,
                        _ => return None,
                    };
                    let pos = transform.transform_point3(Vec3::new(m.offset.x, m.offset.y, 0.0));
                    // Marker headings turn clockwise from the model's +Y, like ours: the
                    // smithing workbench's marker stands at -X with heading +90 degrees,
                    // facing the bench (chairs and beds only use 0 or 180).
                    Some(Marker {
                        pos,
                        heading: ref_heading + m.heading,
                        kind,
                        entries: m.entry,
                        anim_type: u32::from(m.anim_type),
                    })
                })
                .collect();
            if list.is_empty() {
                continue;
            }
            self.items.push(Furniture {
                ref_id: r,
                cell: key,
                owner: owner_of(lo, r),
                markers: list,
                events: special.into_iter().collect(),
                idle_time: 0.0,
                ways: Vec::new(),
            });
        }
    }

    fn add_idle_marker(
        &mut self,
        lo: &LoadOrder,
        idles: &super::idles::IdleIndex,
        key: CellKey,
        r: FormId,
        rf: &records::Reference,
        base: &esp::LoadedRecord<'_>,
    ) {
        let events: Vec<String> = base
            .get(b"IDLA")
            .unwrap_or(&[])
            .chunks_exact(4)
            .filter_map(|c| {
                idles.humanoid_event(base.fid(FormId(u32::from_le_bytes(c.try_into().unwrap()))))
            })
            .collect();
        if events.is_empty() {
            return;
        }
        let idle_time = base
            .get(b"IDLT")
            .filter(|d| d.len() >= 4)
            .map_or(0.0, |d| f32::from_le_bytes(d[0..4].try_into().unwrap()));
        let fwd = rf.transform().transform_vector3(Vec3::Y);
        let marker = Marker {
            pos: rf.position,
            heading: fwd.x.atan2(fwd.y),
            kind: Use::Idle,
            entries: 1,
            anim_type: 0,
        };
        self.items.push(Furniture {
            ref_id: r,
            cell: key,
            owner: owner_of(lo, r),
            markers: vec![marker],
            events,
            idle_time,
            ways: Vec::new(),
        });
    }

    pub fn count(&self, key: CellKey) -> (usize, usize) {
        let items = self.items.iter().filter(|f| f.cell == key);
        items.fold((0, 0), |(n, m), f| (n + 1, m + f.markers.len()))
    }

    pub fn remove_cell(&mut self, key: CellKey) {
        let gone: Vec<FormId> = self
            .items
            .iter()
            .filter(|f| f.cell == key)
            .map(|f| f.ref_id)
            .collect();
        self.items.retain(|f| f.cell != key);
        self.users.retain(|(f, _), _| !gone.contains(f));
    }

    pub fn get(&self, r: FormId) -> Option<&Furniture> {
        self.items
            .iter()
            .find(|f| f.ref_id == r && !self.disabled.contains(&r))
    }

    pub fn set_disabled(&mut self, r: FormId, off: bool) {
        if off {
            self.disabled.insert(r);
        } else {
            self.disabled.remove(&r);
        }
    }

    pub fn reserve(&mut self, furniture: FormId, marker: u8, actor: FormId) -> bool {
        match self.users.get(&(furniture, marker)) {
            Some(&a) if a != actor => false,
            _ => {
                self.users.insert((furniture, marker), actor);
                true
            }
        }
    }

    pub fn release(&mut self, actor: FormId) {
        self.users.retain(|_, a| *a != actor);
    }

    /// Free markers of `kind` within `radius` of `centre` that `may_use` allows,
    /// as (furniture, marker index, marker).
    pub fn free_near(
        &self,
        kind: Use,
        centre: Vec3,
        radius: f32,
        may_use: impl Fn(&Furniture, Use) -> bool,
    ) -> Vec<(FormId, u8, Marker)> {
        let mut out = Vec::new();
        for f in self
            .items
            .iter()
            .filter(|f| !self.disabled.contains(&f.ref_id))
        {
            for (i, m) in f.markers.iter().enumerate() {
                if m.kind == kind
                    && m.pos.truncate().distance(centre.truncate()) <= radius
                    && (m.pos.z - centre.z).abs() < 400.0
                    && !self.users.contains_key(&(f.ref_id, i as u8))
                    && may_use(f, kind)
                {
                    out.push((f.ref_id, i as u8, *m));
                }
            }
        }
        out
    }
}

/// Ways to use `m` of furniture `f` (entry side and clips), for an actor whose
/// skeleton lives at `skeleton_path`.
#[allow(clippy::too_many_arguments)]
pub fn ways_to_use(
    f: &Furniture,
    mi: u8,
    m: &Marker,
    child: bool,
    project: &crate::world::behavior::ProjectRuntime,
    skeleton_path: &str,
    female: bool,
    skeleton: &crate::world::skeleton::Skeleton,
    clips: &mut super::Clips,
    rand: &mut dyn FnMut() -> u64,
    holds: &dyn Fn(FormId) -> bool,
) -> Vec<(Entry, UseClips)> {
    // Creatures only use the furniture their own ways were picked for.
    let graph = (!project.humanoid()).then_some(project.name.as_str());
    if graph.is_some()
        && (m.kind == Use::Idle
            || !f
                .ways
                .iter()
                .any(|w| w.marker == mi && w.graph.as_deref() == graph))
    {
        return Vec::new();
    }
    if graph.is_none()
        && (m.kind == Use::Idle
            || (m.kind == Use::Special && !f.ways.iter().any(|w| w.marker == mi)))
        || (m.kind == Use::Special && !f.ways.iter().any(|w| w.marker == mi))
    {
        // One of the marker's idles, or special furniture's keyword idle event.
        if f.events.is_empty() {
            return Vec::new();
        }
        let start = (rand() % f.events.len() as u64) as usize;
        let pick = (0..f.events.len()).find_map(|i| {
            let e = &f.events[(start + i) % f.events.len()];
            clips.event(e, project, skeleton_path, female, skeleton)
        });
        return pick.map(|c| vec![(Entry::Front, c)]).unwrap_or_default();
    }
    let ways: Vec<(Entry, UseClips)> = f
        .ways
        .iter()
        .filter(|w| w.marker == mi && w.child == child && w.graph.as_deref() == graph)
        .filter(|w| w.holding.is_none_or(|(item, held)| holds(item) == held))
        .filter_map(|w| {
            // The tree's exit (IdleChairFrontExit...), else the generic ones.
            let exits: Vec<&str> = w
                .exit
                .as_deref()
                .into_iter()
                .chain(["IdleChairExitStart", "IdleStop"])
                .collect();
            let mut use_clips = clips.event_with_exits(
                &w.enter,
                &exits,
                project,
                skeleton_path,
                female,
                skeleton,
            )?;
            use_clips.instant_exit = w.instant_exit.clone();
            Some((w.entry, use_clips))
        })
        .collect();
    if ways.is_empty() {
        log::debug!(
            "{}: marker {mi} ({:?}) has no usable way on",
            f.ref_id,
            m.kind
        );
    }
    ways
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_world_turns_clockwise() {
        let fwd = to_world(Vec3::Y, std::f32::consts::FRAC_PI_2);
        assert!((fwd - Vec3::X).length() < 1e-5, "{fwd}");
        let right = to_world(Vec3::X, 0.0);
        assert!((right - Vec3::X).length() < 1e-5);
    }
}
