//! Acoustic metadata at startup; placed volumes are discovered with cell streaming.
use esp::{FormId, LoadOrder};
use glam::{Mat4, Vec3};
use std::collections::{HashMap, HashSet};
#[derive(Clone, Copy, Debug)]
pub struct Space {
    pub ambient: Option<FormId>,
    pub region: Option<FormId>,
    pub reverb: Option<FormId>,
}
#[derive(Clone, Debug)]
struct Marker {
    reference: FormId,
    space: FormId,
    cell: FormId,
    world: Option<FormId>,
    inverse: Mat4,
    min: Vec3,
    max: Vec3,
    volume: f32,
    world_min: Vec3,
    world_max: Vec3,
}
impl Marker {
    fn new(
        reference: FormId,
        space: FormId,
        cell: FormId,
        world: Option<FormId>,
        transform: Mat4,
        min: Vec3,
        max: Vec3,
    ) -> Option<Self> {
        let determinant = transform.determinant();
        if !determinant.is_finite()
            || determinant == 0.0
            || !min.is_finite()
            || !max.is_finite()
            || !min.cmple(max).all()
        {
            return None;
        }
        let mut world_min = Vec3::splat(f32::INFINITY);
        let mut world_max = Vec3::splat(f32::NEG_INFINITY);
        for x in [min.x, max.x] {
            for y in [min.y, max.y] {
                for z in [min.z, max.z] {
                    let p = transform.transform_point3(Vec3::new(x, y, z));
                    if !p.is_finite() {
                        return None;
                    }
                    world_min = world_min.min(p);
                    world_max = world_max.max(p);
                }
            }
        }
        Some(Self {
            reference,
            space,
            cell,
            world,
            inverse: transform.inverse(),
            min,
            max,
            volume: (max - min).element_product().abs() * determinant.abs(),
            world_min,
            world_max,
        })
    }
    fn contains(&self, position: Vec3) -> bool {
        let p = self.inverse.transform_point3(position);
        p.is_finite() && p.cmpge(self.min).all() && p.cmple(self.max).all()
    }
}
#[derive(Default)]
pub struct Spaces {
    pub records: HashMap<FormId, Space>,
    pub regions: HashMap<FormId, Vec<super::regions::Sound>>,
    pub reverbs: HashMap<FormId, super::reverb::Params>,
    markers: Vec<Marker>,
    loaded_cells: HashSet<FormId>,
    defaults: HashMap<FormId, Option<FormId>>,
    interiors: HashMap<FormId, Vec<usize>>,
    exteriors: HashMap<(FormId, i32, i32), Vec<usize>>,
    /// Avoid creating millions of bins for an unusually large acoustic volume.
    large: HashMap<FormId, Vec<usize>>,
}
impl Spaces {
    pub fn load(lo: &LoadOrder) -> Self {
        let mut result = Self {
            regions: super::regions::load(lo),
            ..Default::default()
        };
        for &id in lo.ids_of_type(b"ASPC") {
            let Some(r) = lo.get(id) else { continue };
            let fid = |tag| {
                r.get(tag)
                    .filter(|d| d.len() >= 4)
                    .map(|d| r.fid(FormId(u32::from_le_bytes(d[..4].try_into().unwrap()))))
                    .filter(|id| !id.is_null())
            };
            result.records.insert(
                id,
                Space {
                    ambient: fid(b"SNAM"),
                    region: fid(b"RDAT"),
                    reverb: fid(b"BNAM"),
                },
            );
        }
        for &id in lo.ids_of_type(b"REVB") {
            if let Some(params) = lo
                .get(id)
                .and_then(|r| r.get(b"DATA").and_then(super::reverb::Params::parse))
            {
                result.reverbs.insert(id, params);
            }
        }
        result
    }
    /// Called once per encountered cell, alongside the existing reference loader.
    /// Cached cells survive unloads; their immutable volumes remain spatially indexed.
    pub fn load_cell(&mut self, lo: &LoadOrder, cell: FormId) {
        if !self.loaded_cells.insert(cell) {
            return;
        }
        let fallback = lo
            .get(cell)
            .and_then(|r| {
                r.get(b"XCAS")
                    .and_then(|d| d.get(..4))
                    .map(|d| r.fid(FormId(u32::from_le_bytes(d.try_into().unwrap()))))
            })
            .filter(|id| !id.is_null());
        self.defaults.insert(cell, fallback);
        let Some(index) = lo.cell(cell) else { return };
        for &id in index.persistent.iter().chain(&index.temporary) {
            if lo.tag_of(id).is_none_or(|t| t.0 != *b"REFR") {
                continue;
            }
            let Some(r) = lo.get(id) else { continue };
            let Some(d) = r.get(b"NAME").filter(|d| d.len() >= 4) else {
                continue;
            };
            let base = r.fid(FormId(u32::from_le_bytes(d[..4].try_into().unwrap())));
            if !self.records.contains_key(&base) {
                continue;
            }
            let rf = crate::world::records::reference(&r);
            if rf.deleted() || !rf.scale.is_finite() || rf.scale <= 0.0 {
                continue;
            }
            let bounds = if let Some(d) = r.get(b"XPRM").filter(|d| d.len() >= 32) {
                let extents = Vec3::from_array(std::array::from_fn(|i| {
                    f32::from_le_bytes(d[i * 4..i * 4 + 4].try_into().unwrap())
                }))
                .abs();
                Some((-extents, extents))
            } else {
                lo.get(base).and_then(|b| {
                    b.get(b"OBND").filter(|d| d.len() >= 12).map(|d| {
                        let v: [f32; 6] = std::array::from_fn(|i| {
                            i16::from_le_bytes(d[i * 2..i * 2 + 2].try_into().unwrap()) as f32
                        });
                        (Vec3::new(v[0], v[1], v[2]), Vec3::new(v[3], v[4], v[5]))
                    })
                })
            };
            if let Some((min, max)) = bounds
                && let Some(marker) =
                    Marker::new(id, base, cell, index.world, rf.transform(), min, max)
            {
                self.insert(marker);
            }
        }
    }
    fn insert(&mut self, marker: Marker) {
        let index = self.markers.len();
        if let Some(world) = marker.world {
            let min = crate::engine::grid_of(marker.world_min.truncate());
            let max = crate::engine::grid_of(marker.world_max.truncate());
            let width = max.0 as i64 - min.0 as i64 + 1;
            let height = max.1 as i64 - min.1 as i64 + 1;
            if width.saturating_mul(height) > 256 {
                self.large.entry(world).or_default().push(index);
            } else {
                for x in min.0..=max.0 {
                    for y in min.1..=max.1 {
                        self.exteriors.entry((world, x, y)).or_default().push(index);
                    }
                }
            }
        } else {
            self.interiors.entry(marker.cell).or_default().push(index);
        }
        self.markers.push(marker);
    }
    fn candidates(
        &self,
        cell: FormId,
        world: Option<FormId>,
        position: Vec3,
    ) -> impl Iterator<Item = &Marker> {
        let local = if let Some(world) = world {
            let (x, y) = crate::engine::grid_of(position.truncate());
            self.exteriors.get(&(world, x, y))
        } else {
            self.interiors.get(&cell)
        };
        let large = world.and_then(|w| self.large.get(&w));
        local
            .into_iter()
            .flatten()
            .chain(large.into_iter().flatten())
            .map(|&i| &self.markers[i])
    }
    pub fn choose(
        &self,
        cell: FormId,
        world: Option<FormId>,
        position: Vec3,
        enabled: impl Fn(FormId) -> bool,
    ) -> Option<FormId> {
        self.candidates(cell, world, position)
            .filter(|m| m.contains(position) && enabled(m.reference))
            .min_by(|a, b| {
                a.volume
                    .total_cmp(&b.volume)
                    .then(a.reference.0.cmp(&b.reference.0))
            })
            .map(|m| m.space)
            .or_else(|| {
                if world.is_none() {
                    self.defaults.get(&cell).copied().flatten()
                } else {
                    None
                }
            })
    }
}
impl crate::engine::Engine {
    pub(crate) fn audio_cell(&self) -> Option<FormId> {
        match self.location {
            crate::engine::Location::Interior(id) => Some(id),
            crate::engine::Location::Exterior { world, .. } => self
                .lo
                .world(world)?
                .cells
                .get(&crate::engine::grid_of(self.camera.position.truncate()))
                .copied(),
            crate::engine::Location::Nowhere => None,
        }
    }
    pub(crate) fn acoustic_space(&self) -> Option<FormId> {
        let cell = self.audio_cell()?;
        let world = match self.location {
            crate::engine::Location::Exterior { world, .. } => Some(world),
            _ => None,
        };
        self.audio_spaces
            .choose(cell, world, self.camera.position, |id| {
                !self.is_disabled(id)
            })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rotated_scaled_volumes_choose_smallest_containing_marker() {
        let transform = Mat4::from_scale_rotation_translation(
            Vec3::splat(2.0),
            glam::Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
            Vec3::new(10.0, 0.0, 0.0),
        );
        let min = Vec3::new(-3.0, -1.0, -1.0);
        let max = -min;
        let m = Marker::new(FormId(1), FormId(10), FormId(2), None, transform, min, max).unwrap();
        assert!(m.contains(Vec3::new(10.0, 5.0, 0.0)));
        assert!(!m.contains(Vec3::new(15.0, 0.0, 0.0)));
        let mut c = Spaces::default();
        c.insert(m);
        c.insert(
            Marker::new(
                FormId(3),
                FormId(11),
                FormId(2),
                None,
                transform,
                min * 0.5,
                max * 0.5,
            )
            .unwrap(),
        );
        assert_eq!(
            c.choose(FormId(2), None, Vec3::new(10.0, 0.0, 0.0), |_| true),
            Some(FormId(11))
        );
        assert_eq!(
            c.choose(FormId(2), None, Vec3::new(10.0, 0.0, 0.0), |id| id
                != FormId(3)),
            Some(FormId(10))
        );
    }
    #[test]
    fn exterior_lookup_uses_local_bins_and_keeps_volumes_crossing_cell_boundaries() {
        let world = Some(FormId(20));
        let mut c = Spaces::default();
        for i in 1..=1000 {
            let pos = Vec3::new(i as f32 * 8192.0 + 128.0, 0.0, 0.0);
            c.insert(
                Marker::new(
                    FormId(i),
                    FormId(10),
                    FormId(2),
                    world,
                    Mat4::from_translation(pos),
                    Vec3::splat(-1.0),
                    Vec3::ONE,
                )
                .unwrap(),
            );
        }
        c.insert(
            Marker::new(
                FormId(2000),
                FormId(11),
                FormId(2),
                world,
                Mat4::from_translation(Vec3::new(4090.0, 0.0, 0.0)),
                Vec3::splat(-20.0),
                Vec3::splat(20.0),
            )
            .unwrap(),
        );
        let at = Vec3::new(4100.0, 0.0, 0.0);
        assert_eq!(c.candidates(FormId(3), world, at).count(), 1);
        assert_eq!(c.choose(FormId(3), world, at, |_| true), Some(FormId(11)));
        assert_eq!(c.choose(FormId(3), Some(FormId(21)), at, |_| true), None);
        c.insert(
            Marker::new(
                FormId(3000),
                FormId(12),
                FormId(2),
                world,
                Mat4::IDENTITY,
                Vec3::splat(-1e9),
                Vec3::splat(1e9),
            )
            .unwrap(),
        );
        assert_eq!(c.candidates(FormId(3), world, at).count(), 2);
        assert_eq!(c.large[&FormId(20)].len(), 1);
        assert_eq!(c.choose(FormId(3), world, at, |_| true), Some(FormId(11)));
    }
    #[test]
    fn marker_discovery_is_lazy_per_cell_and_cached_including_xcas() {
        use esp::writer::{Node, RecordBuilder, write_plugin};
        let dir =
            std::env::temp_dir().join(format!("vrm-lazy-audio-spaces-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut primitive = Vec::new();
        for _ in 0..3 {
            primitive.extend(10f32.to_le_bytes());
        }
        primitive.extend([0; 20]);
        let mut children = Vec::new();
        for (cell, reference) in [(0x800u32, 0x820), (0x801, 0x821)] {
            children.push(Node::Record(
                RecordBuilder::new(b"CELL", FormId(cell))
                    .u32(b"XCAS", 0x810)
                    .u32(b"DATA", 1),
            ));
            children.push(Node::Group {
                label: cell.to_le_bytes(),
                group_type: esp::group_type::CELL_CHILDREN,
                children: vec![Node::Group {
                    label: cell.to_le_bytes(),
                    group_type: esp::group_type::CELL_TEMPORARY,
                    children: vec![Node::Record(
                        RecordBuilder::new(b"REFR", FormId(reference))
                            .u32(b"NAME", 0x810)
                            .field(b"DATA", &[0; 24])
                            .field(b"XPRM", &primitive),
                    )],
                }],
            });
        }
        let records = [
            Node::top(b"ASPC", vec![RecordBuilder::new(b"ASPC", FormId(0x810))]),
            Node::Group {
                label: *b"CELL",
                group_type: 0,
                children,
            },
        ];
        std::fs::write(
            dir.join("Audio.esp"),
            write_plugin(0, &[], "test", "lazy spaces", 0x900, &records),
        )
        .unwrap();
        let lo = LoadOrder::load(&dir, &["Audio.esp".into()]).unwrap();
        let mut c = Spaces::load(&lo);
        assert!(c.markers.is_empty());
        assert!(c.loaded_cells.is_empty());
        c.load_cell(&lo, FormId(0x800));
        assert_eq!(c.markers.len(), 1);
        c.load_cell(&lo, FormId(0x800));
        assert_eq!(c.markers.len(), 1);
        assert_eq!(
            c.choose(FormId(0x800), None, Vec3::splat(100.0), |_| false),
            Some(FormId(0x810)),
            "cached XCAS fallback"
        );
        assert_eq!(c.choose(FormId(0x801), None, Vec3::ZERO, |_| true), None);
        c.load_cell(&lo, FormId(0x801));
        assert_eq!(c.markers.len(), 2);
        assert_eq!(
            c.choose(FormId(0x801), None, Vec3::ZERO, |id| id == FormId(0x821)),
            Some(FormId(0x810))
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
