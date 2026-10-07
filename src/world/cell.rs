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

pub struct CellContents {
    pub info: CellInfo,
    pub objects: Vec<PlacedObject>,
    pub lights: Vec<PointLight>,
    pub doors: Vec<Door>,
    pub lighting: Lighting,
}

/// The references in a cell; `disabled` says which are disabled now (left out).
pub fn load_cell(lo: &LoadOrder, cell: FormId, disabled: &dyn Fn(FormId) -> bool) -> Option<CellContents> {
    let info = records::cell_info(lo, cell)?;
    let index = lo.cell(cell)?;
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

    let mut out = CellContents { info, objects: Vec::new(), lights: Vec::new(), doors: Vec::new(), lighting };
    for &rid in index.persistent.iter().chain(index.temporary.iter()) {
        add_reference(lo, rid, disabled, &mut out.objects, &mut out.lights, &mut out.doors);
    }
    Some(out)
}

pub fn add_reference(
    lo: &LoadOrder,
    rid: FormId,
    disabled: &dyn Fn(FormId) -> bool,
    objects: &mut Vec<PlacedObject>,
    lights: &mut Vec<PointLight>,
    doors: &mut Vec<Door>,
) {
    let Some(rec) = lo.get(rid) else { return };
    if rec.tag().0 != *b"REFR" {
        return;
    }
    let r: Reference = records::reference(&rec);
    if r.deleted() || disabled(rid) {
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
            position: r.position,
            radius: r.radius_override.unwrap_or(l.radius).max(1.0),
            color: l.color * l.fade,
        });
    }
    if tag == *b"DOOR" {
        doors.push(Door { ref_id: rid, position: r.position, destination: r.teleport });
    }
    if let Some(model) = records::model_path(&base) {
        let lower = model.as_str();
        if lower.contains("marker") && lower.starts_with("meshes/marker") {
            return;
        }
        objects.push(PlacedObject { ref_id: rid, base: r.base, model, transform: r.transform() });
    }
}
