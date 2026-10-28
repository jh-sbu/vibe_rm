//! Acoustic-space records and oriented placed volumes (game assets only).
use esp::{FormId, LoadOrder};
use glam::{Mat4, Vec3};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug)]
pub struct Space {
    pub ambient: Option<FormId>,
    pub region: Option<FormId>,
    pub reverb: Option<FormId>,
}
#[derive(Clone, Debug)]
pub struct Marker {
    pub reference: FormId,
    pub space: FormId,
    pub cell: FormId,
    pub world: Option<FormId>,
    pub transform: Mat4,
    pub min: Vec3,
    pub max: Vec3,
}
impl Marker {
    pub fn contains(&self, position: Vec3) -> bool {
        let p = self.transform.inverse().transform_point3(position);
        p.is_finite() && p.cmpge(self.min).all() && p.cmple(self.max).all()
    }
    pub fn volume(&self) -> f32 {
        (self.max - self.min).element_product().abs() * self.transform.determinant().abs()
    }
}
#[derive(Default)]
pub struct Spaces {
    pub records: HashMap<FormId, Space>,
    pub markers: Vec<Marker>,
}
impl Spaces {
    pub fn load(lo: &LoadOrder) -> Self {
        let mut result = Self::default();
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
        for &id in lo.ids_of_type(b"REFR") {
            let Some(r) = lo.get(id) else { continue };
            let Some(d) = r.get(b"NAME").filter(|d| d.len() >= 4) else {
                continue;
            };
            let base = r.fid(FormId(u32::from_le_bytes(d[..4].try_into().unwrap())));
            if !result.records.contains_key(&base) {
                continue;
            }
            let rf = crate::world::records::reference(&r);
            if rf.deleted() || !rf.scale.is_finite() || rf.scale <= 0.0 {
                continue;
            }
            let Some(cell) = lo.cell_of_ref(id) else {
                continue;
            };
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
            if let Some((min, max)) = bounds {
                result.markers.push(Marker {
                    reference: id,
                    space: base,
                    cell,
                    world: lo.cell(cell).and_then(|c| c.world),
                    transform: rf.transform(),
                    min,
                    max,
                });
            }
        }
        result
    }
    pub fn choose(
        &self,
        cell: FormId,
        world: Option<FormId>,
        position: Vec3,
        enabled: impl Fn(FormId) -> bool,
    ) -> Option<FormId> {
        self.markers
            .iter()
            .filter(|m| {
                if world.is_some() {
                    m.world == world
                } else {
                    m.cell == cell
                }
            })
            .filter(|m| enabled(m.reference) && m.contains(position))
            .min_by(|a, b| {
                a.volume()
                    .total_cmp(&b.volume())
                    .then(a.reference.0.cmp(&b.reference.0))
            })
            .map(|m| m.space)
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
            .or_else(|| {
                if world.is_some() {
                    return None;
                }
                let r = self.lo.get(cell)?;
                let d = r.get(b"XCAS")?.get(..4)?;
                let id = r.fid(FormId(u32::from_le_bytes(d.try_into().ok()?)));
                (!id.is_null()).then_some(id)
            })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rotated_scaled_volumes_choose_smallest_containing_marker() {
        let m = Marker {
            reference: FormId(1),
            space: FormId(10),
            cell: FormId(2),
            world: None,
            transform: Mat4::from_scale_rotation_translation(
                Vec3::splat(2.0),
                glam::Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
                Vec3::new(10.0, 0.0, 0.0),
            ),
            min: Vec3::new(-3.0, -1.0, -1.0),
            max: Vec3::new(3.0, 1.0, 1.0),
        };
        assert!(m.contains(Vec3::new(10.0, 5.0, 0.0)));
        assert!(!m.contains(Vec3::new(15.0, 0.0, 0.0)));
        let mut small = m.clone();
        small.reference = FormId(3);
        small.space = FormId(11);
        small.min *= 0.5;
        small.max *= 0.5;
        let c = Spaces {
            markers: vec![m, small],
            ..Default::default()
        };
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
}
