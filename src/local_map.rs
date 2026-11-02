//! Geometry used to fit the local map to loaded cells.
use esp::LoadOrder;
use glam::Vec3;

use crate::engine::Location;
use crate::world::cell::CellReferences;
use crate::world::terrain::CELL_SIZE;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub min: Vec3,
    pub max: Vec3,
}

/// Bounds of the authored objects in an interior. Transform all eight OBND
/// corners: rotating just the two opposite corners does not bound a box.
pub fn interior_bounds(lo: &LoadOrder, cell: &CellReferences) -> Option<Bounds> {
    let mut result: Option<Bounds> = None;
    for entry in &cell.entries {
        let reference = &entry.reference;
        if reference.deleted() {
            continue;
        }
        let Some(base) = lo.get(reference.base) else {
            continue;
        };
        if base.flags() & 0x0080_0000 != 0
            || (matches!(&base.tag().0, b"STAT" | b"MSTT" | b"ACTI" | b"TACT")
                && base.flags() & 0x200 != 0)
        {
            continue;
        }
        let Some(data) = base.get(b"OBND").filter(|d| d.len() >= 12) else {
            continue;
        };
        let component = |i: usize| i16::from_le_bytes([data[i * 2], data[i * 2 + 1]]) as f32;
        let min = Vec3::new(component(0), component(1), component(2));
        let max = Vec3::new(component(3), component(4), component(5));
        let transform = reference.transform();
        for x in [min.x, max.x] {
            for y in [min.y, max.y] {
                for z in [min.z, max.z] {
                    let p = transform.transform_point3(Vec3::new(x, y, z));
                    if !p.is_finite() {
                        continue;
                    }
                    match &mut result {
                        Some(b) => {
                            b.min = b.min.min(p);
                            b.max = b.max.max(p);
                        }
                        None => result = Some(Bounds { min: p, max: p }),
                    }
                }
            }
        }
    }
    result
}

/// A scene containing only the static environment for the local-map pass.
/// GPU handles are shared; no game scene state is changed.
pub fn map_scene(lo: &LoadOrder, source: &crate::render::Scene) -> crate::render::Scene {
    use crate::render::{Environment, Instance, RenderCell, Scene};
    let mut scene = Scene {
        local_map: true,
        env: Environment {
            clear_color: Vec3::splat(0.05),
            sun_dir: Vec3::new(0.3, 0.4, 0.85).normalize(),
            sun_color: Vec3::splat(0.7),
            ambient: Vec3::splat(0.4),
            ..Default::default()
        },
        ..Default::default()
    };
    for (&key, source_cell) in &source.cells {
        let instances = source_cell
            .instances
            .iter()
            .filter_map(|object| {
                if object.hidden {
                    return None;
                }
                let base = lo.get(esp::FormId(object.base))?;
                if !matches!(
                    &base.tag().0,
                    b"STAT" | b"MSTT" | b"ACTI" | b"TACT" | b"DOOR" | b"FURN" | b"CONT" | b"TREE"
                ) || base.flags() & 0x0080_0000 != 0
                    || (matches!(&base.tag().0, b"STAT" | b"MSTT" | b"ACTI" | b"TACT")
                        && base.flags() & 0x200 != 0)
                {
                    return None;
                }
                let mut instance = Instance::new(object.model.clone(), object.transform);
                instance.ref_id = object.ref_id;
                instance.base = object.base;
                Some(instance)
            })
            .collect();
        scene.cells.insert(
            key,
            RenderCell {
                instances,
                terrain: source_cell.terrain.clone(),
                water: source_cell.water.clone(),
                ..Default::default()
            },
        );
    }
    scene
}

/// Heading of an interior's NorthMarker; exteriors are aligned to true north.
/// Reference angles turn clockwise about Z, as does Camera::yaw. The marker's
/// +Y facing is therefore camera-up at the same angle. The UESP map export
/// tutorial identifies the same marker angle as the export rotation.
pub fn north_rotation(cell: &CellReferences) -> f32 {
    if !cell.interior {
        return 0.0;
    }
    cell.entries
        .iter()
        .map(|entry| &entry.reference)
        .find(|reference| reference.base == esp::FormId(3) && !reference.deleted())
        .map_or(0.0, |reference| reference.rotation.z)
}

/// Exterior grid square loaded around the player, in world units.
pub fn exterior_bounds(location: Location, radius: i32) -> Option<Bounds> {
    let Location::Exterior { center: (x, y), .. } = location else {
        return None;
    };
    let radius = radius.max(0);
    Some(Bounds {
        min: Vec3::new(
            (x - radius) as f32 * CELL_SIZE,
            (y - radius) as f32 * CELL_SIZE,
            0.0,
        ),
        max: Vec3::new(
            (x + radius + 1) as f32 * CELL_SIZE,
            (y + radius + 1) as f32 * CELL_SIZE,
            0.0,
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::cell::ReferenceCache;
    use esp::{
        FormId,
        writer::{Node, RecordBuilder, write_plugin},
    };

    #[test]
    fn interior_bounds_transform_corners_and_exclude_flagged_objects() {
        let dir = std::env::temp_dir().join(format!("vrm-local-map-bounds-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut bounds = Vec::new();
        for v in [-10i16, -20, -5, 10, 20, 5] {
            bounds.extend(v.to_le_bytes());
        }
        let reference = |id, base, x: f32, rotation: f32| {
            let mut data = Vec::new();
            for v in [x, 0.0, 0.0, 0.0, 0.0, rotation] {
                data.extend(v.to_le_bytes());
            }
            Node::Record(
                RecordBuilder::new(b"REFR", FormId(id))
                    .u32(b"NAME", base)
                    .field(b"DATA", &data),
            )
        };
        let groups = [
            Node::top(
                b"STAT",
                vec![
                    RecordBuilder::new(b"STAT", FormId(3))
                        .flags(0x0080_0000)
                        .field(b"OBND", &bounds),
                    RecordBuilder::new(b"STAT", FormId(0x810)).field(b"OBND", &bounds),
                    RecordBuilder::new(b"STAT", FormId(0x811))
                        .flags(0x200)
                        .field(b"OBND", &bounds),
                    RecordBuilder::new(b"STAT", FormId(0x812))
                        .flags(0x0080_0000)
                        .field(b"OBND", &bounds),
                ],
            ),
            Node::Group {
                label: *b"CELL",
                group_type: 0,
                children: vec![
                    Node::Record(RecordBuilder::new(b"CELL", FormId(0x800)).u32(b"DATA", 1)),
                    Node::Group {
                        label: 0x800u32.to_le_bytes(),
                        group_type: esp::group_type::CELL_CHILDREN,
                        children: vec![Node::Group {
                            label: 0x800u32.to_le_bytes(),
                            group_type: esp::group_type::CELL_TEMPORARY,
                            children: vec![
                                reference(0x820, 0x810, 100.0, std::f32::consts::FRAC_PI_2),
                                reference(0x821, 0x811, 1000.0, 0.0),
                                reference(0x822, 0x812, -1000.0, 0.0),
                                reference(0x823, 3, -2000.0, 1.25),
                            ],
                        }],
                    },
                ],
            },
        ];
        std::fs::write(
            dir.join("Map.esp"),
            write_plugin(0, &[], "test", "local map bounds", 0x900, &groups),
        )
        .unwrap();
        let lo = LoadOrder::load(&dir, &["Map.esp".into()]).unwrap();
        let cell = ReferenceCache::default().get(&lo, FormId(0x800));
        assert!((north_rotation(&cell) - 1.25).abs() < 0.0001);
        let bounds = interior_bounds(&lo, &cell).unwrap();
        assert!(bounds.min.abs_diff_eq(Vec3::new(80.0, -10.0, -5.0), 0.001));
        assert!(bounds.max.abs_diff_eq(Vec3::new(120.0, 10.0, 5.0), 0.001));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
