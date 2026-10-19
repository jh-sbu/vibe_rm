//! Placed decals: references whose base is a texture set (TXST) with decal
//! data (`DODT`), projected onto whatever lies in their box.
//!
//! A decal projects along its reference's +Y (2460 of the 2713 vanilla ground
//! blood and burn decals point it down; `vrm-tool decals`): a ray from the
//! reference along it finds the surface, and the decal's box is centred there,
//! a width (along X) and height (along Z) picked per reference between the
//! decal data's minimum and maximum, and its depth along Y. See
//! `known_ambiguities/decals.md`.

use esp::{FormId, LoadOrder};
use glam::{Mat4, Vec2, Vec3, Vec4};

use super::records::{self, texture_path};
use crate::engine::Engine;

/// How far a decal's ray looks for its surface.
pub const REACH: f32 = 1024.0;

/// The most `DODT` parallax passes (xEdit: it can't be higher than 30).
pub const MAX_PASSES: u8 = 30;

/// `DODT` flags. Decals blend whatever the alpha blending flag says, except
/// an alpha tested one that lacks it: that's cut out hard-edged.
pub mod flags {
    pub const PARALLAX: u8 = 0x01;
    pub const ALPHA_BLENDING: u8 = 0x02;
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
    /// The height map (texture set slot 4), for parallax.
    pub height: Option<String>,
    pub min_width: f32,
    pub max_width: f32,
    pub min_height: f32,
    pub max_height: f32,
    pub depth: f32,
    /// Specular exponent: 0 for no specular shine.
    pub shininess: f32,
    pub parallax_scale: f32,
    pub parallax_passes: u8,
    pub flags: u8,
    pub color: Vec4,
}

impl DecalData {
    pub fn load(lo: &LoadOrder, id: FormId) -> Option<DecalData> {
        let rec = lo.get(id)?;
        if rec.tag().0 != *b"TXST" {
            return None;
        }
        DecalData::with_dodt(lo, id, rec.get(b"DODT")?)
    }

    /// A texture set's textures with the decal data `d` (a `DODT`: the texture
    /// set's own, or an impact's).
    pub fn with_dodt(lo: &LoadOrder, id: FormId, d: &[u8]) -> Option<DecalData> {
        let rec = lo.get(id)?;
        if rec.tag().0 != *b"TXST" || d.len() < 36 {
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
            height: tex(b"TX04"),
            min_width: f(0),
            max_width: f(4).max(f(0)),
            min_height: f(8),
            max_height: f(12).max(f(8)),
            depth: f(16),
            shininess: f(20),
            parallax_scale: f(24),
            parallax_passes: d[28].min(MAX_PASSES),
            flags: d[29],
            color: Vec4::new(d[32] as f32, d[33] as f32, d[34] as f32, 255.0) / 255.0,
        })
    }

    /// It's shown with parallax: it has a height map and the flag.
    pub fn parallax(&self) -> bool {
        self.flags & flags::PARALLAX != 0
            && self.height.is_some()
            && self.parallax_passes > 0
            && self.parallax_scale > 0.0
    }

    /// Its alpha test cuts it out with hard edges, not blending.
    pub fn hard_edge(&self) -> bool {
        self.flags & flags::ALPHA_TESTING != 0 && self.flags & flags::ALPHA_BLENDING == 0
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
        let decals: Vec<_> = found
            .into_iter()
            .map(|(p, d)| {
                let mut g = self.gpu_decal(d.id, &d, p.transform, p.subtexture);
                g.ref_id = p.ref_id.0;
                g.hidden = self.is_disabled(p.ref_id);
                g
            })
            .collect();
        log::debug!("{key:?}: {} decals", decals.len());
        if let Some(rc) = self.scene.cells.get_mut(&key) {
            rc.decals = decals;
        }
    }
}

impl Engine {
    /// A decal for the renderer: `data`'s textures (loaded if need be) in a
    /// material kept under `key` (its texture set, or the impact placing
    /// it), in the box `transform`, showing subtexture `subtexture` (0..4).
    pub(crate) fn gpu_decal(
        &mut self,
        key: FormId,
        data: &DecalData,
        transform: Mat4,
        subtexture: u32,
    ) -> crate::render::decal::GpuDecal {
        let material = match self.decal_materials.get(&key) {
            Some(m) => m.clone(),
            None => {
                let tex: Vec<String> = [
                    Some(&data.diffuse),
                    data.normal.as_ref(),
                    data.glow.as_ref(),
                    data.height.as_ref().filter(|_| data.parallax()),
                ]
                .into_iter()
                .flatten()
                .filter(|t| !self.renderer.textures.contains(t))
                .cloned()
                .collect();
                super::loader::load_textures(&mut self.renderer, &self.vfs, tex);
                let m = std::sync::Arc::new(self.renderer.create_material(&material(data)));
                self.decal_materials.insert(key, m.clone());
                m
            }
        };
        let uv = if data.subtextures() {
            let (i, j) = (subtexture % 2, subtexture / 2);
            Vec4::new(i as f32 * 0.5, j as f32 * 0.5, 0.5, 0.5)
        } else {
            Vec4::new(0.0, 0.0, 1.0, 1.0)
        };
        let (scale, _, center) = transform.to_scale_rotation_translation();
        let radius = scale.length() * 0.5;
        crate::render::decal::GpuDecal {
            ref_id: 0,
            hidden: false,
            material,
            transform,
            uv,
            tint: data.color,
            lights: crate::render::pick_lights(&self.scene.lights, center, radius),
            center,
            radius,
        }
    }
}

/// The object material a decal is drawn with (its textures, specular, alpha
/// test, parallax).
fn material(d: &DecalData) -> crate::render::model::MaterialDesc {
    let mut m = crate::render::model::MaterialDesc {
        diffuse: Some(d.diffuse.clone()),
        normal: d.normal.clone(),
        glow: d.glow.clone(),
        hard_edge: d.hard_edge(),
        ..Default::default()
    };
    if d.flags & flags::ALPHA_TESTING != 0 {
        m.alpha_test = Some(0.5);
    }
    if d.glow.is_some() {
        m.emissive = Vec4::ONE;
    }
    // The normal map's alpha masks a white shine of the data's shininess
    // (the exponent runs to 40000 in vanilla: far past a shine at all).
    if d.shininess > 0.0 {
        m.specular = Vec3::ONE;
        m.glossiness = d.shininess.min(SHININESS_MAX);
    }
    if d.parallax() {
        m.env_mask = d.height.clone();
        m.parallax = Vec2::new(d.parallax_scale, d.parallax_passes as f32);
    }
    m
}

/// The highest specular exponent a decal uses.
const SHININESS_MAX: f32 = 1024.0;

#[cfg(test)]
mod tests {
    use super::*;

    fn data(flags: u8) -> DecalData {
        DecalData {
            id: FormId(1),
            diffuse: "d.dds".into(),
            normal: Some("n.dds".into()),
            glow: None,
            height: Some("h.dds".into()),
            min_width: 1.0,
            max_width: 1.0,
            min_height: 1.0,
            max_height: 1.0,
            depth: 1.0,
            shininess: 400.0,
            parallax_scale: 0.5,
            parallax_passes: 4,
            flags,
            color: Vec4::ONE,
        }
    }

    #[test]
    fn specular_from_shininess() {
        let mut d = data(0);
        let m = material(&d);
        assert_eq!(m.specular, Vec3::ONE);
        assert_eq!(m.glossiness, 400.0);
        d.shininess = 40000.0;
        assert_eq!(material(&d).glossiness, SHININESS_MAX);
        d.shininess = 0.0;
        assert_eq!(material(&d).specular, Vec3::ZERO);
    }

    #[test]
    fn parallax_needs_flag_and_height_map() {
        let mut d = data(flags::PARALLAX);
        let m = material(&d);
        assert_eq!(m.env_mask.as_deref(), Some("h.dds"));
        assert_eq!(m.parallax, Vec2::new(0.5, 4.0));
        d.height = None;
        assert_eq!(material(&d).parallax, Vec2::ZERO);
        assert_eq!(material(&data(0)).parallax, Vec2::ZERO);
        let mut d = data(flags::PARALLAX);
        d.parallax_passes = 0;
        assert!(!d.parallax());
    }

    #[test]
    fn alpha_test_without_blending_is_hard_edged() {
        assert!(data(flags::ALPHA_TESTING).hard_edge());
        assert!(!data(flags::ALPHA_TESTING | flags::ALPHA_BLENDING).hard_edge());
        assert!(!data(flags::NO_SUBTEXTURES).hard_edge());
        let m = material(&data(flags::ALPHA_TESTING));
        assert!(m.hard_edge);
        assert_eq!(m.alpha_test, Some(0.5));
    }
}
