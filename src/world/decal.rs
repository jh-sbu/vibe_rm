//! Placed decals: references whose base is a texture set (TXST) with decal
//! data (`DODT`), projected onto whatever lies in their box.
//!
//! A decal projects along its reference's +Y (2460 of the 2713 vanilla ground
//! blood and burn decals point it down; `vrm-tool decals`): a ray from the
//! reference along it finds the surface, and the decal's box is centred there,
//! a width (along X) and height (along Z) picked per reference between the
//! decal data's minimum and maximum, and its depth along Y. See
//! `known_gaps/decals.md`.

use esp::{FormId, LoadOrder};
use glam::{Mat4, Vec3, Vec4};

use super::records::{self, texture_path};
use crate::engine::Engine;

/// `DODT` flags.
/// How far a decal's ray looks for its surface.
pub const REACH: f32 = 1024.0;

/// `DODT` flags used (0x01 parallax and 0x02 alpha blending aren't: decals
/// always blend).
pub mod flags {
    pub const ALPHA_TESTING: u8 = 0x04;
    pub const NO_SUBTEXTURES: u8 = 0x08;
}

/// A texture set's decal data and textures.
#[derive(Debug, Clone)]
pub struct DecalData {
    pub id: FormId,
    pub diffuse: String,
    pub normal: Option<String>,
    pub glow: Option<String>,
    pub min_width: f32,
    pub max_width: f32,
    pub min_height: f32,
    pub max_height: f32,
    pub depth: f32,
    pub flags: u8,
    pub color: Vec4,
}

impl DecalData {
    pub fn load(lo: &LoadOrder, id: FormId) -> Option<DecalData> {
        let rec = lo.get(id)?;
        if rec.tag().0 != *b"TXST" {
            return None;
        }
        let d = rec.get(b"DODT")?;
        if d.len() < 36 {
            return None;
        }
        let f = |i: usize| f32::from_le_bytes(d[i..i + 4].try_into().unwrap());
        let tex = |tag: &[u8; 4]| {
            rec.get(tag)
                .map(esp::decode_zstring)
                .filter(|s| !s.trim().is_empty())
                .map(|s| texture_path(&s))
        };
        Some(DecalData {
            id,
            diffuse: tex(b"TX00")?,
            normal: tex(b"TX01"),
            glow: tex(b"TX03"),
            min_width: f(0),
            max_width: f(4).max(f(0)),
            min_height: f(8),
            max_height: f(12).max(f(8)),
            depth: f(16),
            flags: d[29],
            color: Vec4::new(d[32] as f32, d[33] as f32, d[34] as f32, 255.0) / 255.0,
        })
    }

    /// Its texture is an atlas of 2x2 subtextures, one picked per decal.
    pub fn subtextures(&self) -> bool {
        self.flags & flags::NO_SUBTEXTURES == 0
    }
}

/// A placed decal: its reference and box (a unit cube about the
/// origin turned and scaled to the decal's width, depth and height).
#[derive(Debug, Clone)]
pub struct PlacedDecal {
    pub ref_id: FormId,
    pub transform: Mat4,
    /// Which of the 2x2 subtextures it shows (0..4).
    pub subtexture: u32,
}

/// A value in [0, 1) from a reference and a salt.
fn rand(r: FormId, salt: u32) -> f32 {
    let mut h = r.0.wrapping_mul(0x9E37_79B1) ^ salt.wrapping_mul(0x85EB_CA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// The decal a reference places, if its base is a texture set with decal data.
/// `surface(origin, direction)`: where a decal's ray meets what it's
/// projected onto (none: it's left out).
pub fn placed(
    lo: &LoadOrder,
    rid: FormId,
    data: &DecalData,
    surface: impl Fn(Vec3, Vec3) -> Option<Vec3>,
) -> Option<PlacedDecal> {
    let rec = lo.get(rid)?;
    let r = records::reference(&rec);
    if r.deleted() {
        return None;
    }
    let w = data.min_width + (data.max_width - data.min_width) * rand(rid, 1);
    let h = data.min_height + (data.max_height - data.min_height) * rand(rid, 2);
    let size = Vec3::new(w, data.depth.max(1.0), h) * r.scale;
    let rotation = records::rotation_from_euler(r.rotation);
    let hit = surface(r.position, rotation * Vec3::Y)?;
    Some(PlacedDecal {
        ref_id: rid,
        transform: Mat4::from_scale_rotation_translation(size, rotation, hit),
        subtexture: (rand(rid, 3) * 4.0) as u32 % 4,
    })
}

/// The base of a reference, if it's a texture set (a decal).
pub fn decal_base(lo: &LoadOrder, rid: FormId) -> Option<FormId> {
    let rec = lo.get(rid)?;
    if rec.tag().0 != *b"REFR" {
        return None;
    }
    let base = records::reference(&rec).base;
    (lo.get(base)?.tag().0 == *b"TXST").then_some(base)
}

impl Engine {
    /// The decals among a loaded cell's references, placed after the next
    /// physics step ([`Engine::place_decals`]).
    pub(crate) fn load_decals(&mut self, key: crate::render::CellKey, refs: &[FormId]) {
        if std::env::var_os("VRM_NO_DECALS").is_some() {
            return;
        }
        let refs: Vec<FormId> = refs
            .iter()
            .copied()
            .filter(|&r| decal_base(&self.lo, r).is_some())
            .collect();
        if !refs.is_empty() {
            self.pending_decals.push((key, refs));
        }
    }

    /// Place the decals of cells loaded since the last physics step: their
    /// rays need the cells' collision, which queries see once stepped.
    pub(crate) fn place_decals(&mut self) {
        for (key, refs) in std::mem::take(&mut self.pending_decals) {
            if self.scene.cells.contains_key(&key) {
                self.place_cell_decals(key, &refs);
            }
        }
    }

    fn place_cell_decals(&mut self, key: crate::render::CellKey, refs: &[FormId]) {
        let mut found = Vec::new();
        for &r in refs {
            let Some(base) = decal_base(&self.lo, r) else {
                continue;
            };
            let data = self
                .decal_data
                .entry(base)
                .or_insert_with(|| DecalData::load(&self.lo, base))
                .clone();
            let physics = &self.physics;
            let surface =
                |o: Vec3, d: Vec3| physics.ground_ray(o, d, REACH).map(|(t, _)| o + d * t);
            if let Some(data) = data
                && let Some(p) = placed(&self.lo, r, &data, surface)
            {
                found.push((p, data));
            }
        }
        if found.is_empty() {
            return;
        }
        let tex: Vec<String> = found
            .iter()
            .flat_map(|(_, d)| [Some(&d.diffuse), d.normal.as_ref(), d.glow.as_ref()])
            .flatten()
            .filter(|t| !self.renderer.textures.contains(t))
            .cloned()
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        super::loader::load_textures(&mut self.renderer, &self.vfs, tex);
        let mut decals = Vec::new();
        for (p, d) in found {
            let material = match self.decal_materials.get(&d.id) {
                Some(m) => m.clone(),
                None => {
                    let m = std::sync::Arc::new(self.renderer.create_material(&material(&d)));
                    self.decal_materials.insert(d.id, m.clone());
                    m
                }
            };
            let uv = if d.subtextures() {
                let (i, j) = (p.subtexture % 2, p.subtexture / 2);
                Vec4::new(i as f32 * 0.5, j as f32 * 0.5, 0.5, 0.5)
            } else {
                Vec4::new(0.0, 0.0, 1.0, 1.0)
            };
            let (scale, _, center) = p.transform.to_scale_rotation_translation();
            let radius = scale.length() * 0.5;
            decals.push(crate::render::decal::GpuDecal {
                ref_id: p.ref_id.0,
                hidden: self.is_disabled(p.ref_id),
                material,
                transform: p.transform,
                uv,
                tint: d.color,
                lights: crate::render::pick_lights(&self.scene.lights, center, radius),
                center,
                radius,
            });
        }
        log::debug!("{key:?}: {} decals", decals.len());
        if let Some(rc) = self.scene.cells.get_mut(&key) {
            rc.decals = decals;
        }
    }
}

/// The object material a decal is drawn with (its textures, alpha test).
fn material(d: &DecalData) -> crate::render::model::MaterialDesc {
    let mut m = crate::render::model::MaterialDesc {
        diffuse: Some(d.diffuse.clone()),
        normal: d.normal.clone(),
        glow: d.glow.clone(),
        ..Default::default()
    };
    if d.flags & flags::ALPHA_TESTING != 0 {
        m.alpha_test = Some(0.5);
    }
    if d.glow.is_some() {
        m.emissive = Vec4::ONE;
    }
    m
}
