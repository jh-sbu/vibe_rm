//! Acoustic metadata at startup; placed volumes are discovered with cell streaming.
use esp::{FormId, LoadOrder};
use glam::{Mat4, Vec3};
use std::collections::{HashMap, HashSet};
#[derive(Clone, Copy, Debug)]
pub struct Space {
    pub ambient: Option<FormId>,
    pub region: Option<FormId>,
    pub reverb: Option<FormId>,
    pub bounds: Option<(Vec3, Vec3)>,
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
    defaults: HashMap<FormId, FormId>,
    focus: Option<(FormId, Option<FormId>, (i32, i32))>,
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
                    bounds: r.get(b"OBND").filter(|d| d.len() >= 12).map(|d| {
                        let v: [f32; 6] = std::array::from_fn(|i| {
                            i16::from_le_bytes(d[i * 2..i * 2 + 2].try_into().unwrap()) as f32
                        });
                        (Vec3::new(v[0], v[1], v[2]), Vec3::new(v[3], v[4], v[5]))
                    }),
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
    /// Consumes the same parsed references as the scene loader; no reference rereads.
    pub fn ingest_cell(&mut self, cell: FormId, references: &crate::world::cell::CellReferences) {
        if !self.loaded_cells.insert(cell) {
            return;
        }
        if references.interior
            && let Some(space) = references.acoustic_space
        {
            self.defaults.insert(cell, space);
        }
        for placed in &references.entries {
            let rf = &placed.reference;
            if placed.tag.0 != *b"REFR" || rf.deleted() || !rf.scale.is_finite() || rf.scale <= 0.0
            {
                continue;
            }
            let Some(space) = self.records.get(&rf.base) else {
                continue;
            };
            if let Some((min, max)) = placed.primitive_bounds.or(space.bounds)
                && let Some(marker) = Marker::new(
                    rf.id,
                    rf.base,
                    cell,
                    references.world,
                    rf.transform(),
                    min,
                    max,
                )
            {
                self.insert(marker);
            }
        }
    }
    /// Discover adjacent cells even when their visuals/scripts have never streamed.
    /// Revisit only when the listener changes cell; the scene loader shares this cache.
    fn discover(
        &mut self,
        lo: &LoadOrder,
        cache: &mut crate::world::cell::ReferenceCache,
        cell: FormId,
        world: Option<FormId>,
        position: Vec3,
    ) -> Option<HashSet<FormId>> {
        let grid = if world.is_some() {
            crate::engine::grid_of(position.truncate())
        } else {
            (0, 0)
        };
        let focus = (cell, world, grid);
        if self.focus == Some(focus) {
            return None;
        }
        self.focus = Some(focus);
        let mut keep = HashSet::new();
        keep.insert(cell);
        if let Some(world) = world
            && let Some(index) = lo.world(world)
        {
            keep.extend(index.persistent_cell);
            for dx in -1..=1 {
                for dy in -1..=1 {
                    if let (Some(x), Some(y)) = (grid.0.checked_add(dx), grid.1.checked_add(dy))
                        && let Some(&id) = index.cells.get(&(x, y))
                    {
                        keep.insert(id);
                    }
                }
            }
        }
        for &id in &keep {
            let refs = cache.get(lo, id);
            self.ingest_cell(id, &refs);
        }
        Some(keep)
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
                    self.defaults.get(&cell).copied()
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
    pub(crate) fn acoustic_space(&mut self) -> Option<FormId> {
        let cell = self.audio_cell()?;
        let world = match self.location {
            crate::engine::Location::Exterior { world, .. } => Some(world),
            _ => None,
        };
        if let Some(mut keep) = self.audio_spaces.discover(
            &self.lo,
            &mut self.cell_references,
            cell,
            world,
            self.camera.position,
        ) {
            for key in self.cells.keys() {
                let id = match *key {
                    crate::render::CellKey::Interior(id) => Some(id),
                    crate::render::CellKey::Exterior(x, y) => world
                        .and_then(|w| self.lo.world(w))
                        .and_then(|w| w.cells.get(&(x, y)))
                        .copied(),
                };
                keep.extend(id);
            }
            self.cell_references.retain(&keep);
        }
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
    fn unloaded_neighbours_are_discovered_without_exterior_xcas_or_duplicate_reference_reads() {
        use esp::writer::{Node, RecordBuilder, write_plugin};
        let dir = std::env::temp_dir().join(format!("vrm-audio-neighbours-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut children = Vec::new();
        for (cell, grid, position, reference) in [
            (0x800u32, (0i32, 0i32), Vec3::ZERO, None),
            (
                0x801,
                (1, 0),
                Vec3::new(4100.0, 2048.0, 0.0),
                Some(0x820u32),
            ),
            (0x802, (-1, 0), Vec3::new(-10.0, 2048.0, 0.0), Some(0x821)),
            (0x803, (1, 1), Vec3::new(4100.0, 4100.0, 0.0), Some(0x822)),
        ] {
            let grid_bytes: Vec<u8> = [grid.0, grid.1]
                .into_iter()
                .flat_map(i32::to_le_bytes)
                .collect();
            children.push(Node::Record(
                RecordBuilder::new(b"CELL", FormId(cell))
                    .u32(b"DATA", 0)
                    .u32(b"XCAS", 0x810)
                    .field(b"XCLC", &grid_bytes),
            ));
            if let Some(reference) = reference {
                let mut placement: Vec<u8> = position
                    .to_array()
                    .into_iter()
                    .flat_map(f32::to_le_bytes)
                    .collect();
                placement.extend([0; 12]);
                let mut primitive: Vec<u8> =
                    [100f32; 3].into_iter().flat_map(f32::to_le_bytes).collect();
                primitive.extend([0; 20]);
                children.push(Node::Group {
                    label: cell.to_le_bytes(),
                    group_type: esp::group_type::CELL_CHILDREN,
                    children: vec![Node::Group {
                        label: cell.to_le_bytes(),
                        group_type: esp::group_type::CELL_TEMPORARY,
                        children: vec![Node::Record(
                            RecordBuilder::new(b"REFR", FormId(reference))
                                .u32(b"NAME", 0x810)
                                .field(b"DATA", &placement)
                                .field(b"XPRM", &primitive),
                        )],
                    }],
                });
            }
        }
        let records = [
            Node::top(
                b"ASPC",
                vec![RecordBuilder::new(b"ASPC", FormId(0x810)).u32(b"BNAM", 0x840)],
            ),
            Node::Group {
                label: *b"WRLD",
                group_type: 0,
                children: vec![
                    Node::Record(RecordBuilder::new(b"WRLD", FormId(0x900))),
                    Node::Group {
                        label: 0x900u32.to_le_bytes(),
                        group_type: esp::group_type::WORLD_CHILDREN,
                        children,
                    },
                ],
            },
        ];
        std::fs::write(
            dir.join("Neighbours.esp"),
            write_plugin(0, &[], "test", "acoustic neighbours", 0xA00, &records),
        )
        .unwrap();
        let lo = LoadOrder::load(&dir, &["Neighbours.esp".into()]).unwrap();
        let mut cache = crate::world::cell::ReferenceCache::default();
        let mut c = Spaces::load(&lo);
        c.ingest_cell(FormId(0x800), &cache.get(&lo, FormId(0x800)));
        assert!(c.markers.is_empty());
        let listener = Vec3::new(4080.0, 2048.0, 0.0);
        assert_eq!(
            c.choose(FormId(0x800), Some(FormId(0x900)), listener, |_| true),
            None
        );
        c.discover(
            &lo,
            &mut cache,
            FormId(0x800),
            Some(FormId(0x900)),
            listener,
        )
        .unwrap();
        for (point, reference) in [
            (listener, 0x820),
            (Vec3::new(10.0, 2048.0, 0.0), 0x821),
            (Vec3::new(4080.0, 4080.0, 0.0), 0x822),
        ] {
            assert_eq!(
                c.choose(FormId(0x800), Some(FormId(0x900)), point, |id| id
                    == FormId(reference)),
                Some(FormId(0x810))
            );
        }
        assert!(
            c.defaults.is_empty(),
            "exterior XCAS is neither parsed as a fallback nor retained"
        );
        assert_eq!(c.records[&FormId(0x810)].reverb, Some(FormId(0x840)));
        let before = cache.get(&lo, FormId(0x801));
        let after = cache.get(&lo, FormId(0x801));
        assert!(std::sync::Arc::ptr_eq(&before, &after));
        let contents = crate::world::cell::load_cell(&lo, FormId(0x801), &after).unwrap();
        assert!(contents.objects.is_empty());
        assert_eq!(c.markers.len(), 3);
        c.ingest_cell(FormId(0x801), &after);
        assert_eq!(c.markers.len(), 3);
        assert!(
            c.discover(
                &lo,
                &mut cache,
                FormId(0x800),
                Some(FormId(0x900)),
                listener
            )
            .is_none()
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

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
        let mut references = crate::world::cell::ReferenceCache::default();
        let parsed = references.get(&lo, FormId(0x800));
        c.ingest_cell(FormId(0x800), &parsed);
        assert_eq!(c.markers.len(), 1);
        let reused = references.get(&lo, FormId(0x800));
        assert!(std::sync::Arc::ptr_eq(&parsed, &reused));
        let contents = crate::world::cell::load_cell(&lo, FormId(0x800), &reused).unwrap();
        assert!(contents.objects.is_empty());
        c.ingest_cell(FormId(0x800), &reused);
        assert_eq!(c.markers.len(), 1);
        assert_eq!(
            c.choose(FormId(0x800), None, Vec3::splat(100.0), |_| false),
            Some(FormId(0x810)),
            "cached XCAS fallback"
        );
        assert_eq!(c.choose(FormId(0x801), None, Vec3::ZERO, |_| true), None);
        c.ingest_cell(FormId(0x801), &references.get(&lo, FormId(0x801)));
        assert_eq!(c.markers.len(), 2);
        assert_eq!(
            c.choose(FormId(0x801), None, Vec3::ZERO, |id| id == FormId(0x821)),
            Some(FormId(0x810))
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
