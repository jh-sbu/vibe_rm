//! Turning a cell's references into a renderable scene description.

use esp::{FormId, LoadOrder};
use glam::{Mat4, Vec3};

use super::records::{self, CellInfo, Lighting, Reference};

#[derive(Debug, Clone)]
pub struct PlacedObject {
    pub ref_id: FormId,
    pub base: FormId,
    pub model: String,
    pub transform: Mat4,
}

#[derive(Debug, Clone, Copy)]
pub struct PointLight {
    /// The light's reference (lights of disabled references are left out).
    pub ref_id: FormId,
    pub position: Vec3,
    pub radius: f32,
    pub color: Vec3,
}

#[derive(Debug, Clone)]
pub struct Door {
    pub ref_id: FormId,
    pub position: Vec3,
    pub destination: Option<(FormId, Vec3, Vec3)>,
}

/// Parsed once for both scene construction and acoustic-volume discovery.
#[derive(Debug, Clone)]
pub struct CellReference {
    pub tag: esp::Tag,
    pub reference: Reference,
    pub primitive_bounds: Option<(Vec3, Vec3)>,
}
impl CellReference {
    fn read(lo: &LoadOrder, id: FormId) -> Option<Self> {
        let rec = lo.get(id)?;
        let primitive_bounds = rec.get(b"XPRM").filter(|d| d.len() >= 32).map(|d| {
            let extents = Vec3::from_array(std::array::from_fn(|i| {
                f32::from_le_bytes(d[i * 4..i * 4 + 4].try_into().unwrap())
            }))
            .abs();
            (-extents, extents)
        });
        Some(Self {
            tag: rec.tag(),
            reference: records::reference(&rec),
            primitive_bounds,
        })
    }
}
#[derive(Default)]
pub struct CellReferences {
    pub world: Option<FormId>,
    pub interior: bool,
    pub acoustic_space: Option<FormId>,
    pub entries: Vec<CellReference>,
    persistent_len: usize,
}
impl CellReferences {
    pub fn exterior_entries(&self) -> impl Iterator<Item = &CellReference> {
        self.entries[self.persistent_len..]
            .iter()
            .chain(&self.entries[..self.persistent_len])
    }
}
#[derive(Default)]
pub struct ReferenceCache {
    cells: std::collections::HashMap<FormId, std::sync::Arc<CellReferences>>,
}
impl ReferenceCache {
    pub fn get(&mut self, lo: &LoadOrder, cell: FormId) -> std::sync::Arc<CellReferences> {
        self.cells
            .entry(cell)
            .or_insert_with(|| {
                let (interior, acoustic_space) = lo.get(cell).map_or((false, None), |r| {
                    let interior = r
                        .get(b"DATA")
                        .and_then(|d| d.first())
                        .is_some_and(|f| f & 1 != 0);
                    let space = if interior {
                        r.get(b"XCAS")
                            .and_then(|d| d.get(..4))
                            .map(|d| r.fid(FormId(u32::from_le_bytes(d.try_into().unwrap()))))
                            .filter(|id| !id.is_null())
                    } else {
                        None
                    };
                    (interior, space)
                });
                let index = lo.cell(cell);
                let mut entries: Vec<_> = index
                    .into_iter()
                    .flat_map(|c| &c.persistent)
                    .filter_map(|&id| CellReference::read(lo, id))
                    .collect();
                let persistent_len = entries.len();
                entries.extend(
                    index
                        .into_iter()
                        .flat_map(|c| &c.temporary)
                        .filter_map(|&id| CellReference::read(lo, id)),
                );
                std::sync::Arc::new(CellReferences {
                    world: index.and_then(|c| c.world),
                    interior,
                    acoustic_space,
                    entries,
                    persistent_len,
                })
            })
            .clone()
    }
    pub fn retain(&mut self, cells: &std::collections::HashSet<FormId>) {
        self.cells.retain(|id, _| cells.contains(id));
    }
}

pub struct CellContents {
    pub info: CellInfo,
    pub objects: Vec<PlacedObject>,
    pub lights: Vec<PointLight>,
    pub doors: Vec<Door>,
    pub lighting: Lighting,
}

/// The references in a cell, disabled ones too (they are built hidden, so that
/// enabling them shows them).
pub fn load_cell(
    lo: &LoadOrder,
    cell: FormId,
    references: &CellReferences,
) -> Option<CellContents> {
    let info = records::cell_info(lo, cell)?;
    lo.cell(cell)?;
    let mut lighting = info.lighting.unwrap_or_default();
    if !info.lighting_template.is_null()
        && let Some(t) = lo.get(info.lighting_template)
        && let Some(d) = t.get(b"DATA")
    {
        let tmpl = Lighting::parse(d);
        match info.lighting {
            // Inherit flags select which values come from the template.
            Some(own) => {
                let f = own.inherit;
                let pick = |bit: u32| f & bit != 0;
                if pick(0x1) {
                    lighting.ambient = tmpl.ambient;
                    lighting.dalc = tmpl.dalc;
                }
                if pick(0x2) {
                    lighting.directional = tmpl.directional;
                }
                if pick(0x4) {
                    lighting.fog_near_color = tmpl.fog_near_color;
                    lighting.fog_far_color = tmpl.fog_far_color;
                }
                if pick(0x8) {
                    lighting.fog_near = tmpl.fog_near;
                }
                if pick(0x10) {
                    lighting.fog_far = tmpl.fog_far;
                }
                if pick(0x20) {
                    lighting.directional_rot_xy = tmpl.directional_rot_xy;
                    lighting.directional_rot_z = tmpl.directional_rot_z;
                }
                if pick(0x40) {
                    lighting.directional_fade = tmpl.directional_fade;
                }
                if pick(0x80) {
                    lighting.fog_clip = tmpl.fog_clip;
                }
                if pick(0x100) {
                    lighting.fog_power = tmpl.fog_power;
                }
                if pick(0x200) {
                    lighting.fog_max = tmpl.fog_max;
                }
                if pick(0x400) {
                    lighting.light_fade_begin = tmpl.light_fade_begin;
                    lighting.light_fade_end = tmpl.light_fade_end;
                }
            }
            None => lighting = tmpl,
        }
    }

    let mut out = CellContents {
        info,
        objects: Vec::new(),
        lights: Vec::new(),
        doors: Vec::new(),
        lighting,
    };
    for placed in &references.entries {
        add_cached_reference(
            lo,
            placed,
            &mut out.objects,
            &mut out.lights,
            &mut out.doors,
        );
    }
    Some(out)
}

pub fn add_reference(
    lo: &LoadOrder,
    rid: FormId,
    objects: &mut Vec<PlacedObject>,
    lights: &mut Vec<PointLight>,
    doors: &mut Vec<Door>,
) {
    if let Some(placed) = CellReference::read(lo, rid) {
        add_cached_reference(lo, &placed, objects, lights, doors);
    }
}

pub fn add_cached_reference(
    lo: &LoadOrder,
    placed: &CellReference,
    objects: &mut Vec<PlacedObject>,
    lights: &mut Vec<PointLight>,
    doors: &mut Vec<Door>,
) {
    if placed.tag.0 != *b"REFR" {
        return;
    }
    let r = &placed.reference;
    let rid = r.id;
    if r.deleted() {
        return;
    }
    let Some(base) = lo.get(r.base) else { return };
    let tag = base.tag().0;
    if !records::is_renderable_base(&tag) {
        return;
    }
    // Editor-only markers (flag 0x00800000 on STAT marks a marker).
    if tag == *b"STAT" && base.flags() & 0x0080_0000 != 0 {
        return;
    }
    if tag == *b"LIGH"
        && let Some(l) = records::light_data(&base)
        && !l.off_by_default()
        && !l.negative()
    {
        lights.push(PointLight {
            ref_id: rid,
            position: r.position,
            radius: r.radius_override.unwrap_or(l.radius).max(1.0),
            color: l.color * l.fade,
        });
    }
    if tag == *b"DOOR" {
        doors.push(Door {
            ref_id: rid,
            position: r.position,
            destination: r.teleport,
        });
    }
    if let Some(model) = records::model_path(&base) {
        let lower = model.as_str();
        if lower.contains("marker") && lower.starts_with("meshes/marker") {
            return;
        }
        objects.push(PlacedObject {
            ref_id: rid,
            base: r.base,
            model,
            transform: r.transform(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esp::writer::{Node, RecordBuilder, write_plugin};
    #[test]
    fn shared_references_preserve_render_data_and_interior_exterior_order() {
        let dir = std::env::temp_dir().join(format!("vrm-shared-cell-refs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let reference = |id: u32, x: f32| {
            let mut data = Vec::new();
            for v in [x, 0.0, 0.0, 0.0, 0.0, 0.0] {
                data.extend(v.to_le_bytes());
            }
            Node::Record(
                RecordBuilder::new(b"REFR", FormId(id))
                    .u32(b"NAME", 0x810)
                    .f32(b"XSCL", 2.0)
                    .field(b"DATA", &data),
            )
        };
        let cell = 0x800u32;
        let groups = [
            Node::top(
                b"STAT",
                vec![RecordBuilder::new(b"STAT", FormId(0x810)).zstring(b"MODL", "props/test.nif")],
            ),
            Node::Group {
                label: *b"CELL",
                group_type: 0,
                children: vec![
                    Node::Record(RecordBuilder::new(b"CELL", FormId(cell)).u32(b"DATA", 1)),
                    Node::Group {
                        label: cell.to_le_bytes(),
                        group_type: esp::group_type::CELL_CHILDREN,
                        children: vec![
                            Node::Group {
                                label: cell.to_le_bytes(),
                                group_type: esp::group_type::CELL_PERSISTENT,
                                children: vec![reference(0x821, 2.0)],
                            },
                            Node::Group {
                                label: cell.to_le_bytes(),
                                group_type: esp::group_type::CELL_TEMPORARY,
                                children: vec![reference(0x820, 1.0)],
                            },
                        ],
                    },
                ],
            },
        ];
        std::fs::write(
            dir.join("Cell.esp"),
            write_plugin(0, &[], "test", "shared references", 0x900, &groups),
        )
        .unwrap();
        let lo = LoadOrder::load(&dir, &["Cell.esp".into()]).unwrap();
        let mut cache = ReferenceCache::default();
        let first = cache.get(&lo, FormId(cell));
        let reused = cache.get(&lo, FormId(cell));
        assert!(std::sync::Arc::ptr_eq(&first, &reused));
        let contents = load_cell(&lo, FormId(cell), &reused).unwrap();
        assert_eq!(
            contents
                .objects
                .iter()
                .map(|o| o.ref_id.0)
                .collect::<Vec<_>>(),
            [0x821, 0x820]
        );
        let exterior: Vec<_> = reused
            .exterior_entries()
            .map(|r| r.reference.id.0)
            .collect();
        assert_eq!(exterior, [0x820, 0x821]);
        assert_eq!(contents.objects[0].model, "meshes/props/test.nif");
        assert_eq!(
            contents.objects[0].transform.transform_point3(Vec3::X),
            Vec3::new(4.0, 0.0, 0.0)
        );
        cache.retain(&std::collections::HashSet::new());
        assert!(!std::sync::Arc::ptr_eq(
            &first,
            &cache.get(&lo, FormId(cell))
        ));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
