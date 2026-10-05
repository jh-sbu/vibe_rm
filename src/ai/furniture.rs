//! Furniture actors can use (chairs, benches, beds, bedrolls, wall-lean markers):
//! marker positions from the model, ownership, reservations, and the clips to
//! enter, idle in and leave each kind.

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
    const ALL: [(Entry, u16); 4] = [(Entry::Front, 1), (Entry::Behind, 2), (Entry::Right, 4), (Entry::Left, 8)];

    fn name(self) -> &'static str {
        match self {
            Entry::Front => "front",
            Entry::Behind => "back",
            Entry::Right => "right",
            Entry::Left => "left",
        }
    }
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
}

pub struct Furniture {
    pub ref_id: FormId,
    pub cell: CellKey,
    pub owner: Option<FormId>,
    pub bedroll: bool,
    pub markers: Vec<Marker>,
}

/// Furniture in the loaded cells and who is using (or heading for) each marker.
#[derive(Default)]
pub struct FurnitureWorld {
    pub items: Vec<Furniture>,
    users: HashMap<(FormId, u8), FormId>,
}

/// A reserved marker and how to use it.
#[derive(Clone)]
pub struct Seat {
    pub furniture: FormId,
    pub pos: Vec3,
    pub heading: f32,
    pub clips: Arc<UseClips>,
    /// Seconds to stay once in (infinite while the package lasts).
    pub duration: f32,
}

impl Seat {
    /// Pose (feet, heading) at which the enter animation starts.
    pub fn enter_start(&self) -> (Vec3, f32) {
        let m = Marker { pos: self.pos, heading: self.heading, kind: Use::Sit, entries: 0 };
        enter_start(&m, self.clips.enter.as_deref())
    }
}

pub struct UseClips {
    pub enter: Option<Arc<BoundClip>>,
    pub idle: Arc<BoundClip>,
    pub exit: Option<Arc<BoundClip>>,
}

/// Rotate an actor-space offset (+Y forward, +X right) into the world by `heading`.
pub fn to_world(offset: Vec3, heading: f32) -> Vec3 {
    let (s, c) = heading.sin_cos();
    Vec3::new(offset.x * c + offset.y * s, -offset.x * s + offset.y * c, offset.z)
}

/// Pose (feet, heading) at which to start `enter` so that it ends on the marker.
pub fn enter_start(m: &Marker, enter: Option<&BoundClip>) -> (Vec3, f32) {
    let Some((t, yaw)) = enter.and_then(|c| c.motion.as_ref()).map(|mo| mo.end()) else { return (m.pos, m.heading) };
    // Motion yaw is counter-clockwise; headings are clockwise.
    let h0 = m.heading + yaw;
    (m.pos - to_world(t, h0), h0)
}

/// Owner of a reference (`XOWN`), falling back to its cell's owner.
fn owner_of(lo: &LoadOrder, r: FormId) -> Option<FormId> {
    let own = |rec: &esp::LoadedRecord<'_>| {
        rec.get(b"XOWN").filter(|d| d.len() >= 4).map(|d| rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap()))))
    };
    let rec = lo.get(r)?;
    own(&rec).or_else(|| lo.get(lo.cell_of_ref(r)?).and_then(|c| own(&c))).filter(|f| !f.is_null())
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
    faction_owned && rec.subrecords().filter(|s| s.tag.0 == *b"SNAM" && s.data.len() >= 4).any(|s| rec.fid(s.form_id(0)) == owner)
}

impl FurnitureWorld {
    /// Register the furniture among a cell's references. `markers` returns the
    /// model's furniture markers for a model path.
    pub fn add_cell(
        &mut self,
        lo: &LoadOrder,
        key: CellKey,
        refs: &[FormId],
        mut markers: impl FnMut(&str) -> Option<Arc<[nif::FurnitureMarker]>>,
    ) {
        for &r in refs {
            let Some(rec) = lo.get(r) else { continue };
            if rec.tag().0 != *b"REFR" {
                continue;
            }
            let rf = records::reference(&rec);
            if rf.deleted() || rf.initially_disabled() {
                continue;
            }
            let Some(base) = lo.get(rf.base) else { continue };
            if base.tag().0 != *b"FURN" {
                continue;
            }
            // Workbenches, alchemy labs, etc. need their own animations.
            let bench = base.get(b"WBDT").is_some_and(|d| !d.is_empty() && d[0] != 0);
            let Some(model) = records::model_path(&base) else { continue };
            let Some(nif_markers) = markers(&model) else { continue };
            // Active markers: bits 0..23 of MNAM (none set means all).
            let active = base.get(b"MNAM").filter(|d| d.len() >= 4).map(|d| u32::from_le_bytes(d[0..4].try_into().unwrap()) & 0xFF_FFFF);
            let transform: Mat4 = rf.transform();
            let ref_fwd = transform.transform_vector3(Vec3::Y);
            let ref_heading = ref_fwd.x.atan2(ref_fwd.y);
            let list: Vec<Marker> = nif_markers
                .iter()
                .enumerate()
                .filter(|(i, _)| active.is_none_or(|a| a == 0 || a & (1 << i) != 0))
                .filter_map(|(_, m)| {
                    let kind = match m.anim_type {
                        1 => Use::Sit,
                        2 => Use::Sleep,
                        4 => Use::Lean,
                        _ => return None,
                    };
                    let pos = transform.transform_point3(Vec3::new(m.offset.x, m.offset.y, 0.0));
                    // Marker headings turn counter-clockwise from the model's +Y.
                    Some(Marker { pos, heading: ref_heading - m.heading, kind, entries: m.entry })
                })
                .collect();
            if bench || list.is_empty() {
                continue;
            }
            self.items.push(Furniture {
                ref_id: r,
                cell: key,
                owner: owner_of(lo, r),
                bedroll: model.contains("bedroll"),
                markers: list,
            });
        }
    }

    pub fn count(&self, key: CellKey) -> (usize, usize) {
        let items = self.items.iter().filter(|f| f.cell == key);
        items.fold((0, 0), |(n, m), f| (n + 1, m + f.markers.len()))
    }

    pub fn remove_cell(&mut self, key: CellKey) {
        let gone: Vec<FormId> = self.items.iter().filter(|f| f.cell == key).map(|f| f.ref_id).collect();
        self.items.retain(|f| f.cell != key);
        self.users.retain(|(f, _), _| !gone.contains(f));
    }

    pub fn get(&self, r: FormId) -> Option<&Furniture> {
        self.items.iter().find(|f| f.ref_id == r)
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
        for f in &self.items {
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

/// Clip name stems for using furniture of `kind` from `entry`, as (enter, idle, exit).
fn clip_names(kind: Use, bedroll: bool, entry: Entry) -> Option<[String; 3]> {
    let side = entry.name();
    Some(match (kind, bedroll, entry) {
        (Use::Sit, _, Entry::Front) => ["chair_frontentervar1".into(), "chair_idlebasevar1".into(), "chair_frontexit".into()],
        // Stools and benches against a table or counter.
        (Use::Sit, _, Entry::Behind) => ["stoolbackenter".into(), "chair_idlebasevar1".into(), "stoolbackexit".into()],
        (Use::Sit, _, _) => [format!("chair_{side}enter"), "chair_idlebasevar1".into(), format!("chair_{side}exit")],
        (Use::Sleep, _, Entry::Behind) | (Use::Sleep, false, Entry::Front) => return None,
        (Use::Sleep, true, _) => [format!("bedroll_{side}enter"), format!("bedroll_{side}sleep"), format!("bedroll_{side}exit")],
        (Use::Sleep, false, _) => [format!("bed_{side}enter"), format!("bed_{side}sleep"), format!("bed_{side}exit")],
        (Use::Lean, _, Entry::Front) => ["wall_idlebackenter".into(), "wall_idlebackloop".into(), "wall_idlebackexit".into()],
        (Use::Lean, _, _) => return None,
    })
}

/// Ways to use `m` (entry side and clips), for an actor whose skeleton lives at
/// `skeleton_path`. `load` loads a clip by path.
pub fn ways_to_use(
    m: &Marker,
    bedroll: bool,
    skeleton_path: &str,
    female: bool,
    mut load: impl FnMut(&str) -> Option<Arc<BoundClip>>,
) -> Vec<(Entry, UseClips)> {
    let Some(base) = skeleton_path.split("/character assets").next().filter(|b| b.ends_with("actors/character")) else {
        return Vec::new();
    };
    let g = if female { "female" } else { "male" };
    let mut clip = |stem: &str| {
        [format!("{base}/animations/{g}/{stem}.hkx"), format!("{base}/animations/{stem}.hkx")].iter().find_map(|p| load(p))
    };
    let mut out = Vec::new();
    for (entry, bit) in Entry::ALL {
        // Markers with no entry flags can be used from the front.
        if m.entries & bit == 0 && !(m.entries & 0xF == 0 && entry == Entry::Front) {
            continue;
        }
        let Some([enter, idle, exit]) = clip_names(m.kind, bedroll, entry) else { continue };
        let Some(idle) = clip(&idle) else { continue };
        out.push((entry, UseClips { enter: clip(&enter), idle, exit: clip(&exit) }));
    }
    out
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
