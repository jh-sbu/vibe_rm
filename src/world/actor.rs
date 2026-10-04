//! Assembling actors (NPC_ + RACE + ARMO/ARMA + FaceGen) into renderable parts.

use esp::{FormId, LoadOrder, LoadedRecord};
use glam::Mat4;

use super::records::{self, mesh_path, texture_path};

#[derive(Debug, Clone)]
pub struct ActorDesc {
    pub ref_id: FormId,
    pub npc: FormId,
    pub name: String,
    pub transform: Mat4,
    pub skeleton: String,
    /// Skinned models to attach (body parts, armor, head).
    pub models: Vec<String>,
    pub female: bool,
}

fn fid_at(rec: &LoadedRecord<'_>, d: &[u8]) -> FormId {
    if d.len() < 4 {
        return FormId::NULL;
    }
    rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap())))
}

/// Follow leveled lists (picking the first entry) until reaching a record of one of `tags`.
fn resolve_leveled(lo: &LoadOrder, mut id: FormId, list_tag: &[u8; 4]) -> Option<FormId> {
    for _ in 0..8 {
        let rec = lo.get(id)?;
        if rec.tag().0 != *list_tag {
            return Some(id);
        }
        // LVLO: level u16, pad u16, reference formid, count u16, pad
        let first = rec.subrecords().find(|s| s.tag.0 == *b"LVLO")?;
        id = rec.fid(first.form_id(4));
    }
    None
}

struct NpcTraits {
    npc_for_face: FormId,
    race: FormId,
    female: bool,
    skin: FormId,
    outfit: FormId,
    height: f32,
}

const TPL_TRAITS: u16 = 0x1;
const TPL_INVENTORY: u16 = 0x100;

/// Resolve the effective traits of an NPC, following templates.
fn npc_traits(lo: &LoadOrder, npc: FormId) -> Option<NpcTraits> {
    let rec = lo.get(npc)?;
    let acbs = rec.get(b"ACBS").unwrap_or(&[]);
    let flags = if acbs.len() >= 4 { u32::from_le_bytes(acbs[0..4].try_into().unwrap()) } else { 0 };
    let tpl_flags = if acbs.len() >= 20 { u16::from_le_bytes([acbs[18], acbs[19]]) } else { 0 };
    let template = rec.get(b"TPLT").map(|d| fid_at(&rec, d)).filter(|f| !f.is_null());
    let tmpl = template
        .and_then(|t| resolve_leveled(lo, t, b"LVLN"))
        .filter(|t| *t != npc)
        .and_then(|t| npc_traits(lo, t));

    let mut out = NpcTraits {
        npc_for_face: npc,
        race: rec.get(b"RNAM").map(|d| fid_at(&rec, d)).unwrap_or_default(),
        female: flags & 1 != 0,
        skin: rec.get(b"WNAM").map(|d| fid_at(&rec, d)).unwrap_or_default(),
        outfit: rec.get(b"DOFT").map(|d| fid_at(&rec, d)).unwrap_or_default(),
        height: rec.get(b"NAM6").map(|d| f32::from_le_bytes(d[0..4].try_into().unwrap())).unwrap_or(1.0),
    };
    if let Some(t) = tmpl {
        if tpl_flags & TPL_TRAITS != 0 {
            out.npc_for_face = t.npc_for_face;
            out.race = t.race;
            out.female = t.female;
            out.skin = t.skin;
            out.height = t.height;
        }
        if tpl_flags & TPL_INVENTORY != 0 {
            out.outfit = t.outfit;
        }
    }
    Some(out)
}

/// Race skeleton path and default skin armor.
fn race_info(lo: &LoadOrder, race: FormId, female: bool) -> Option<(String, FormId, f32)> {
    let rec = lo.get(race)?;
    let mut gender_marker: Option<bool> = None;
    let mut skeleton = None;
    let mut skin = FormId::NULL;
    let mut height = 1.0;
    for sr in rec.subrecords() {
        match &sr.tag.0 {
            b"MNAM" => gender_marker = Some(false),
            b"FNAM" => gender_marker = Some(true),
            b"ANAM" => {
                if gender_marker == Some(female) || (skeleton.is_none() && gender_marker == Some(false)) {
                    skeleton = Some(mesh_path(&sr.zstring()));
                }
            }
            b"WNAM" => skin = rec.fid(sr.form_id(0)),
            b"DATA" if sr.data.len() >= 24 => height = sr.f32(if female { 20 } else { 16 }),
            // Stop at the first body data section; skeletons always come first.
            b"NAM1" => gender_marker = None,
            _ => {}
        }
    }
    Some((skeleton?, skin, height))
}

/// Armor addon models for an armor on a given race: (model path, biped slot mask).
fn armor_models(lo: &LoadOrder, armo: FormId, race: FormId, female: bool) -> Vec<(String, u32)> {
    let Some(rec) = lo.get(armo) else { return Vec::new() };
    if rec.tag().0 != *b"ARMO" {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut armor_race = FormId::NULL;
    if let Some(d) = rec.get(b"RNAM") {
        armor_race = fid_at(&rec, d);
    }
    let addons: Vec<FormId> = rec.subrecords().filter(|s| s.tag.0 == *b"MODL" && s.data.len() == 4).map(|s| rec.fid(s.form_id(0))).collect();
    for aa in addons {
        let Some(a) = lo.get(aa) else { continue };
        let primary = a.get(b"RNAM").map(|d| fid_at(&a, d)).unwrap_or_default();
        let extra: Vec<FormId> = a.subrecords().filter(|s| s.tag.0 == *b"MODL" && s.data.len() == 4).map(|s| a.fid(s.form_id(0))).collect();
        let matches = primary == race || extra.contains(&race) || (primary == armor_race && extra.is_empty() && race == armor_race);
        if !matches {
            continue;
        }
        let slots = a.get(b"BOD2").map(|d| u32::from_le_bytes(d[0..4].try_into().unwrap())).unwrap_or(0);
        let model = if female { a.get(b"MOD3").or_else(|| a.get(b"MOD2")) } else { a.get(b"MOD2").or_else(|| a.get(b"MOD3")) };
        if let Some(m) = model {
            let s = esp::decode_zstring(m);
            if !s.is_empty() {
                out.push((mesh_path(&s), slots));
            }
        }
    }
    out
}

/// Equipment worn by default: armors from the outfit, resolving leveled item lists.
fn outfit_armors(lo: &LoadOrder, outfit: FormId) -> Vec<FormId> {
    let Some(rec) = lo.get(outfit) else { return Vec::new() };
    let Some(items) = rec.get(b"INAM") else { return Vec::new() };
    items
        .chunks_exact(4)
        .map(|c| rec.fid(FormId(u32::from_le_bytes(c.try_into().unwrap()))))
        .filter_map(|id| resolve_leveled(lo, id, b"LVLI"))
        .filter(|id| lo.tag_of(*id).map(|t| t.0) == Some(*b"ARMO"))
        .collect()
}

pub fn facegen_path(lo: &LoadOrder, npc: FormId) -> Option<String> {
    let (plugin, local) = lo.origin(npc)?;
    Some(format!("meshes/actors/character/facegendata/facegeom/{}/{:08x}.nif", plugin.to_ascii_lowercase(), local))
}

pub fn facetint_path(lo: &LoadOrder, npc: FormId) -> Option<String> {
    let (plugin, local) = lo.origin(npc)?;
    Some(texture_path(&format!("actors/character/facegendata/facetint/{}/{:08x}.dds", plugin.to_ascii_lowercase(), local)))
}

/// Biped slot for the head (30) and hair (31) etc.
const SLOT_HEAD: u32 = 1 << 0;
const SLOT_HAIR: u32 = 1 << 1;

pub fn describe_actor(lo: &LoadOrder, achr: &LoadedRecord<'_>) -> Option<ActorDesc> {
    let r = records::reference(achr);
    if r.deleted() || r.initially_disabled() {
        return None;
    }
    let npc = resolve_leveled(lo, r.base, b"LVLN")?;
    let base = lo.get(npc)?;
    if base.tag().0 != *b"NPC_" {
        return None;
    }
    let traits = npc_traits(lo, npc)?;
    let (skeleton, race_skin, race_height) = race_info(lo, traits.race, traits.female)?;
    let skin = if traits.skin.is_null() { race_skin } else { traits.skin };

    let mut models: Vec<String> = Vec::new();
    let mut covered = 0u32;
    for armo in outfit_armors(lo, traits.outfit) {
        for (m, slots) in armor_models(lo, armo, traits.race, traits.female) {
            if slots & covered != 0 {
                continue;
            }
            covered |= slots;
            models.push(m);
        }
    }
    for (m, slots) in armor_models(lo, skin, traits.race, traits.female) {
        if slots & covered == 0 {
            covered |= slots;
            models.push(m);
        }
    }
    // The pre-built FaceGen head (head, hair, eyes, brows) unless a helmet hides it.
    if covered & SLOT_HEAD == 0 || covered & SLOT_HAIR == 0 {
        if let Some(f) = facegen_path(lo, traits.npc_for_face) {
            models.push(f);
        }
    }
    let name = base.get(b"FULL").map(|d| lo.lstring(&base, d)).unwrap_or_default();
    let scale = r.scale * traits.height * race_height;
    let transform = Mat4::from_scale_rotation_translation(glam::Vec3::splat(scale), r.rotation_quat(), r.position);
    Some(ActorDesc { ref_id: r.id, npc, name, transform, skeleton, models, female: traits.female })
}
