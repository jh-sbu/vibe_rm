//! NAVI map metadata. Public layout: UESP NAVI/NVMI/NVPP and xEdit TES5.
use crate::FormId;
use std::collections::BTreeMap;

/// NAVI overrides are incremental: merge NVMI entries from every plugin.
#[derive(Default)]
pub struct Index {
    pub infos: BTreeMap<FormId, Info>,
    pub preferred_paths: Vec<Vec<FormId>>,
    /// Plugin name, valid NVMI count, invalid NVMI count.
    pub coverage: Vec<(String, usize, usize)>,
}

impl Index {
    pub fn load(lo: &crate::LoadOrder) -> Self {
        let mut index = Self::default();
        for plugin in lo.plugins() {
            let (mut good, mut bad) = (0, 0);
            for item in plugin.plugin.top_level() {
                let crate::Item::Group(group, offset) = item else {
                    continue;
                };
                if group.label != *b"NAVI" {
                    continue;
                }
                for item in plugin.plugin.group_items(&group, offset) {
                    let crate::Item::Record(header, offset) = item else {
                        continue;
                    };
                    if header.tag.0 != *b"NAVI" || header.is_deleted() {
                        continue;
                    }
                    let Ok(record) = plugin.plugin.record_at(offset) else {
                        continue;
                    };
                    for sr in record.subrecords() {
                        match &sr.tag.0 {
                            b"NVMI" => {
                                if let Some(info) = Info::parse(sr.data, |f| plugin.globalize(f)) {
                                    good += 1;
                                    index.infos.insert(info.mesh, info);
                                } else {
                                    bad += 1;
                                }
                            }
                            b"NVSI" => {
                                for bytes in sr.data.chunks_exact(4) {
                                    index.infos.remove(&plugin.globalize(FormId(
                                        u32::from_le_bytes(bytes.try_into().unwrap()),
                                    )));
                                }
                            }
                            b"NVPP" => {
                                if let Some(paths) =
                                    PreferredPaths::parse(sr.data, |f| plugin.globalize(f))
                                {
                                    index.preferred_paths.extend(paths.paths);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            index
                .coverage
                .push((plugin.plugin.name().into(), good, bad));
        }
        index.infos.retain(|&id, _| {
            lo.get(id)
                .is_some_and(|r| r.tag().0 == *b"NAVM" && !r.header.is_deleted())
        });
        index
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Info {
    pub mesh: FormId,
    pub flags: u32,
    pub centre: [f32; 3],
    pub preferred: f32,
    pub links: Vec<FormId>,
    pub preferred_links: Vec<FormId>,
    pub doors: Vec<FormId>,
    pub bounds: Option<([f32; 3], [f32; 3])>,
    pub world: FormId,
    pub cell: FormId,
    pub grid: (i16, i16),
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}
impl Reader<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let out = self.data.get(self.pos..self.pos.checked_add(n)?)?;
        self.pos += n;
        Some(out)
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }
    fn float(&mut self) -> Option<f32> {
        let v = f32::from_bits(self.u32()?);
        v.is_finite().then_some(v)
    }
    fn vector(&mut self) -> Option<[f32; 3]> {
        Some([self.float()?, self.float()?, self.float()?])
    }
    fn count(&mut self, size: usize) -> Option<usize> {
        let n = self.u32()? as usize;
        (n.checked_mul(size)? <= self.data.len() - self.pos).then_some(n)
    }
    fn forms(&mut self, fid: &impl Fn(FormId) -> FormId) -> Option<Vec<FormId>> {
        let n = self.count(4)?;
        (0..n).map(|_| Some(fid(FormId(self.u32()?)))).collect()
    }
}

impl Info {
    pub fn parse(data: &[u8], fid: impl Fn(FormId) -> FormId) -> Option<Self> {
        let mut r = Reader { data, pos: 0 };
        let mesh = fid(FormId(r.u32()?));
        let flags = r.u32()?;
        let centre = r.vector()?;
        let preferred = r.float()?;
        let links = r.forms(&fid)?;
        let preferred_links = r.forms(&fid)?;
        let n = r.count(8)?;
        let mut doors = Vec::with_capacity(n);
        for _ in 0..n {
            r.u32()?;
            doors.push(fid(FormId(r.u32()?)));
        }
        let bounds = match *r.take(1)?.first()? {
            0 => None,
            1 => {
                let lo = r.vector()?;
                let hi = r.vector()?;
                if (0..3).any(|i| lo[i] > hi[i]) {
                    return None;
                }
                let n = r.count(6)?;
                r.take(n.checked_mul(6)?)?;
                let n = r.count(12)?;
                r.take(n.checked_mul(12)?)?;
                Some((lo, hi))
            }
            _ => return None,
        };
        r.u32()?;
        let world = fid(FormId(r.u32()?));
        let parent = r.take(4)?;
        let (cell, grid) = if world.is_null() {
            (
                fid(FormId(u32::from_le_bytes(parent.try_into().ok()?))),
                (0, 0),
            )
        } else {
            (
                FormId::NULL,
                (
                    i16::from_le_bytes(parent[2..4].try_into().ok()?),
                    i16::from_le_bytes(parent[..2].try_into().ok()?),
                ),
            )
        };
        if r.pos != data.len() || mesh.is_null() {
            return None;
        }
        Some(Self {
            mesh,
            flags,
            centre,
            preferred,
            links,
            preferred_links,
            doors,
            bounds,
            world,
            cell,
            grid,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreferredPaths {
    pub paths: Vec<Vec<FormId>>,
    pub markers: Vec<(FormId, u32)>,
}
impl PreferredPaths {
    pub fn parse(data: &[u8], fid: impl Fn(FormId) -> FormId) -> Option<Self> {
        let mut r = Reader { data, pos: 0 };
        let n = r.count(4)?;
        let paths = (0..n).map(|_| r.forms(&fid)).collect::<Option<Vec<_>>>()?;
        let n = r.count(8)?;
        let markers = (0..n)
            .map(|_| Some((fid(FormId(r.u32()?)), r.u32()?)))
            .collect::<Option<Vec<_>>>()?;
        (r.pos == data.len()).then_some(Self { paths, markers })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn info(island: bool, world: u32) -> Vec<u8> {
        let mut d = vec![];
        for v in [
            1u32,
            0,
            1f32.to_bits(),
            2f32.to_bits(),
            3f32.to_bits(),
            0.5f32.to_bits(),
            1,
            2,
            1,
            3,
            1,
            0,
            4,
        ] {
            d.extend(v.to_le_bytes());
        }
        d.push(island as u8);
        if island {
            for v in [0f32, 0., 0., 10., 10., 10.] {
                d.extend(v.to_le_bytes());
            }
            d.extend(1u32.to_le_bytes());
            d.extend([0; 6]);
            d.extend(1u32.to_le_bytes());
            d.extend([0; 12]);
        }
        for v in [0, world, if world == 0 { 5 } else { 0xFFFE0003 }] {
            d.extend(v.to_le_bytes());
        }
        d
    }
    #[test]
    fn remaps_links_doors_cells_and_worlds_and_preserves_signed_grids() {
        for island in [false, true] {
            for world in [0, 6] {
                let d = info(island, world);
                let i =
                    Info::parse(&d, |f| if f.is_null() { f } else { FormId(f.0 + 100) }).unwrap();
                assert_eq!(i.mesh, FormId(101));
                assert_eq!(i.links, vec![FormId(102)]);
                assert_eq!(i.preferred_links, vec![FormId(103)]);
                assert_eq!(i.doors, vec![FormId(104)]);
                assert_eq!(i.bounds.is_some(), island);
                if world == 0 {
                    assert_eq!(i.cell, FormId(105));
                } else {
                    assert_eq!(i.world, FormId(106));
                    assert_eq!(i.grid, (-2, 3));
                }
                for end in 0..d.len() {
                    assert!(Info::parse(&d[..end], |f| f).is_none(), "prefix {end}");
                }
                let mut trailing = d.clone();
                trailing.push(0);
                assert!(Info::parse(&trailing, |f| f).is_none());
            }
        }
    }
    #[test]
    fn malformed_counts_and_nonfinite_centres_are_rejected() {
        let mut d = info(false, 0);
        d[24..28].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Info::parse(&d, |f| f).is_none());
        let mut d = info(false, 0);
        d[8..12].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(Info::parse(&d, |f| f).is_none());
    }
    #[test]
    fn preferred_paths_are_bounded_and_keep_road_marker_indices() {
        let mut d = vec![];
        for v in [1u32, 2, 7, 8, 1, 9, 3] {
            d.extend(v.to_le_bytes());
        }
        let p = PreferredPaths::parse(&d, |f| f).unwrap();
        assert_eq!(p.paths, vec![vec![FormId(7), FormId(8)]]);
        assert_eq!(p.markers, vec![(FormId(9), 3)]);
        for end in 0..d.len() {
            assert!(PreferredPaths::parse(&d[..end], |f| f).is_none());
        }
    }
    #[test]
    fn load_order_merges_incremental_navi_overrides_and_deleted_meshes() {
        use crate::{
            LoadOrder,
            writer::{Node, RecordBuilder, write_plugin},
        };
        let dir = std::env::temp_dir().join(format!(
            "vrm-navi-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let mut first = info(false, 0);
        first[..4].copy_from_slice(&0x801u32.to_le_bytes());
        let mut second = info(false, 0);
        second[..4].copy_from_slice(&0x01000802u32.to_le_bytes());
        std::fs::write(
            dir.join("Base.esm"),
            write_plugin(
                1,
                &[],
                "test",
                "base nav",
                0x900,
                &[
                    Node::top(
                        b"NAVI",
                        vec![RecordBuilder::new(b"NAVI", FormId(0x800)).field(b"NVMI", &first)],
                    ),
                    Node::top(b"NAVM", vec![RecordBuilder::new(b"NAVM", FormId(0x801))]),
                ],
            ),
        )
        .unwrap();
        std::fs::write(
            dir.join("Extra.esp"),
            write_plugin(
                0,
                &["Base.esm"],
                "test",
                "incremental nav",
                0x900,
                &[
                    Node::top(
                        b"NAVI",
                        vec![RecordBuilder::new(b"NAVI", FormId(0x800)).field(b"NVMI", &second)],
                    ),
                    Node::top(
                        b"NAVM",
                        vec![RecordBuilder::new(b"NAVM", FormId(0x01000802))],
                    ),
                ],
            ),
        )
        .unwrap();
        let lo = LoadOrder::load(&dir, &["Base.esm".into(), "Extra.esp".into()]).unwrap();
        let map = Index::load(&lo);
        assert!(map.infos.contains_key(&FormId(0x801)));
        assert!(map.infos.contains_key(&FormId(0x01000802)));
        assert_eq!(map.infos.len(), 2);
        assert_eq!(map.coverage[1].1, 1);
        std::fs::write(
            dir.join("Remove.esp"),
            write_plugin(
                0,
                &["Base.esm", "Extra.esp"],
                "test",
                "deleted info",
                0x900,
                &[Node::top(
                    b"NAVI",
                    vec![
                        RecordBuilder::new(b"NAVI", FormId(0x800))
                            .field(b"NVSI", &0x801u32.to_le_bytes()),
                    ],
                )],
            ),
        )
        .unwrap();
        let lo = LoadOrder::load(
            &dir,
            &["Base.esm".into(), "Extra.esp".into(), "Remove.esp".into()],
        )
        .unwrap();
        let map = Index::load(&lo);
        assert!(!map.infos.contains_key(&FormId(0x801)));
        assert!(map.infos.contains_key(&FormId(0x01000802)));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
