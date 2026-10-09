//! Typed views over the plugin records the engine cares about.

use esp::{FormId, LoadOrder, LoadedRecord};
use glam::{EulerRot, Mat4, Quat, Vec3};

pub fn rgb(b: &[u8], o: usize) -> Vec3 {
    if o + 3 > b.len() {
        return Vec3::ZERO;
    }
    Vec3::new(b[o] as f32, b[o + 1] as f32, b[o + 2] as f32) / 255.0
}

fn f32_at(b: &[u8], o: usize) -> f32 {
    b.get(o..o + 4)
        .map(|s| f32::from_le_bytes(s.try_into().unwrap()))
        .unwrap_or(0.0)
}
fn u32_at(b: &[u8], o: usize) -> u32 {
    b.get(o..o + 4)
        .map(|s| u32::from_le_bytes(s.try_into().unwrap()))
        .unwrap_or(0)
}
fn i32_at(b: &[u8], o: usize) -> i32 {
    u32_at(b, o) as i32
}

/// Interior lighting (XCLL on CELL, DATA on LGTM).
#[derive(Debug, Clone, Copy)]
pub struct Lighting {
    pub ambient: Vec3,
    pub directional: Vec3,
    pub fog_near_color: Vec3,
    pub fog_near: f32,
    pub fog_far: f32,
    pub directional_rot_xy: i32,
    pub directional_rot_z: i32,
    pub directional_fade: f32,
    pub fog_clip: f32,
    pub fog_power: f32,
    pub fog_far_color: Vec3,
    pub fog_max: f32,
    pub light_fade_begin: f32,
    pub light_fade_end: f32,
    pub inherit: u32,
    pub dalc: Option<[Vec3; 6]>,
}

impl Default for Lighting {
    fn default() -> Self {
        Lighting {
            ambient: Vec3::splat(0.25),
            directional: Vec3::splat(0.4),
            fog_near_color: Vec3::ZERO,
            fog_near: 0.0,
            fog_far: 100_000.0,
            directional_rot_xy: 45,
            directional_rot_z: 45,
            directional_fade: 1.0,
            fog_clip: 0.0,
            fog_power: 1.0,
            fog_far_color: Vec3::ZERO,
            fog_max: 1.0,
            light_fade_begin: 0.0,
            light_fade_end: 0.0,
            inherit: 0,
            dalc: None,
        }
    }
}

impl Lighting {
    pub fn parse(d: &[u8]) -> Lighting {
        let mut l = Lighting {
            ambient: rgb(d, 0),
            directional: rgb(d, 4),
            fog_near_color: rgb(d, 8),
            fog_near: f32_at(d, 12),
            fog_far: f32_at(d, 16),
            directional_rot_xy: i32_at(d, 20),
            directional_rot_z: i32_at(d, 24),
            directional_fade: f32_at(d, 28),
            fog_clip: f32_at(d, 32),
            fog_power: f32_at(d, 36),
            ..Default::default()
        };
        if d.len() >= 72 {
            let mut dalc = [Vec3::ZERO; 6];
            for (i, c) in dalc.iter_mut().enumerate() {
                *c = rgb(d, 40 + i * 4);
            }
            if dalc.iter().any(|c| *c != Vec3::ZERO) {
                l.dalc = Some(dalc);
            }
        }
        if d.len() >= 92 {
            l.fog_far_color = rgb(d, 72);
            l.fog_max = f32_at(d, 76);
            l.light_fade_begin = f32_at(d, 80);
            l.light_fade_end = f32_at(d, 84);
            l.inherit = u32_at(d, 88);
        } else {
            l.fog_far_color = l.fog_near_color;
        }
        l
    }

    /// Direction the directional light travels (pointing away from the light).
    pub fn directional_dir(&self) -> Vec3 {
        let xy = (self.directional_rot_xy as f32).to_radians();
        let z = (self.directional_rot_z as f32).to_radians();
        // Rotation about Z (xy) then elevation (z).
        let d = Vec3::new(xy.sin() * z.cos(), xy.cos() * z.cos(), z.sin());
        -d.normalize_or(Vec3::NEG_Z)
    }
}

pub struct CellInfo {
    pub id: FormId,
    pub editor_id: String,
    pub name: String,
    pub interior: bool,
    pub has_water: bool,
    pub lighting: Option<Lighting>,
    pub lighting_template: FormId,
    /// XCLW, if present. `f32::MAX` means no water.
    pub water_height: Option<f32>,
    pub water_type: FormId,
    /// XCIM: the interior's image space.
    pub image_space: FormId,
}

pub fn cell_info(lo: &LoadOrder, id: FormId) -> Option<CellInfo> {
    let rec = lo.get(id)?;
    let mut c = CellInfo {
        id,
        editor_id: rec.editor_id().unwrap_or_default(),
        name: String::new(),
        interior: false,
        has_water: false,
        lighting: None,
        lighting_template: FormId::NULL,
        water_height: None,
        water_type: FormId::NULL,
        image_space: FormId::NULL,
    };
    for sr in rec.subrecords() {
        match &sr.tag.0 {
            b"FULL" => c.name = lo.lstring(&rec, sr.data),
            b"DATA" => {
                let f = sr.u8(0);
                c.interior = f & 1 != 0;
                c.has_water = f & 2 != 0;
            }
            b"XCLL" => c.lighting = Some(Lighting::parse(sr.data)),
            b"LTMP" => c.lighting_template = rec.fid(sr.form_id(0)),
            b"XCLW" => c.water_height = Some(sr.f32(0)),
            b"XCWT" => c.water_type = rec.fid(sr.form_id(0)),
            b"XCIM" => c.image_space = rec.fid(sr.form_id(0)),
            _ => {}
        }
    }
    Some(c)
}

#[derive(Debug, Clone)]
pub struct Reference {
    pub id: FormId,
    pub base: FormId,
    pub position: Vec3,
    /// Euler angles in radians (X, Y, Z).
    pub rotation: Vec3,
    pub scale: f32,
    pub flags: u32,
    pub radius_override: Option<f32>,
    /// Door teleport destination: (destination door ref, position, rotation).
    pub teleport: Option<(FormId, Vec3, Vec3)>,
    pub enable_parent: Option<(FormId, bool)>,
    pub lock: Option<Lock>,
}

/// A door's or container's lock (`XLOC`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lock {
    /// 0 / 1 novice, 25 apprentice, 50 adept, 75 expert, 100 master, 255 needs the key.
    pub level: u8,
    pub key: Option<FormId>,
    /// The level follows the player's (flag 0x04).
    pub leveled: bool,
}

impl Lock {
    pub const NEEDS_KEY: u8 = 255;
}

impl Reference {
    pub fn rotation_quat(&self) -> Quat {
        rotation_from_euler(self.rotation)
    }
    pub fn transform(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            Vec3::splat(self.scale),
            self.rotation_quat(),
            self.position,
        )
    }
    pub fn deleted(&self) -> bool {
        self.flags & esp::record_flags::DELETED != 0
    }
}

/// Creation Engine reference rotation: angles applied Z, then Y, then X about the world
/// axes, each clockwise (the Gamebryo XYZ matrix). A group tilted about world Y in the
/// editor keeps (0, tilt, heading) on every piece, as the Riverwood signpost's arms do.
pub fn rotation_from_euler(r: Vec3) -> Quat {
    Quat::from_euler(EulerRot::XYZ, -r.x, -r.y, -r.z)
}

pub fn reference(rec: &LoadedRecord<'_>) -> Reference {
    let mut r = Reference {
        id: rec.form_id,
        base: FormId::NULL,
        position: Vec3::ZERO,
        rotation: Vec3::ZERO,
        scale: 1.0,
        flags: rec.flags(),
        radius_override: None,
        teleport: None,
        enable_parent: None,
        lock: None,
    };
    for sr in rec.subrecords() {
        match &sr.tag.0 {
            b"NAME" => r.base = rec.fid(sr.form_id(0)),
            b"DATA" => {
                r.position = Vec3::new(sr.f32(0), sr.f32(4), sr.f32(8));
                r.rotation = Vec3::new(sr.f32(12), sr.f32(16), sr.f32(20));
            }
            b"XSCL" => r.scale = sr.f32(0),
            b"XRDS" => r.radius_override = Some(sr.f32(0)),
            b"XTEL" => {
                r.teleport = Some((
                    rec.fid(sr.form_id(0)),
                    Vec3::new(sr.f32(4), sr.f32(8), sr.f32(12)),
                    Vec3::new(sr.f32(16), sr.f32(20), sr.f32(24)),
                ))
            }
            b"XLOC" if sr.data.len() >= 9 => {
                let key = sr.form_id(4);
                r.lock = Some(Lock {
                    level: sr.u8(0),
                    key: (!key.is_null()).then(|| rec.fid(key)),
                    leveled: sr.u8(8) & 0x04 != 0,
                });
            }
            b"XESP" => r.enable_parent = Some((rec.fid(sr.form_id(0)), sr.u8(4) & 1 != 0)),
            _ => {}
        }
    }
    r
}

/// Model path of a base object (`MODL`), normalised to a VFS path under `meshes/`.
pub fn model_path(rec: &LoadedRecord<'_>) -> Option<String> {
    // Armor world models are MOD2 (male) / MOD4 (female); MODL holds ARMA ids.
    let m = if rec.tag().0 == *b"ARMO" {
        rec.get(b"MOD2").or_else(|| rec.get(b"MOD4"))?
    } else {
        rec.get(b"MODL")?
    };
    if m.len() < 2 || m.iter().take(m.len() - 1).any(|&b| b < 0x20) {
        log::debug!("{} {}: odd MODL {:?}", rec.tag(), rec.form_id, m);
        return None;
    }
    let s = esp::decode_zstring(m);
    if s.is_empty() {
        return None;
    }
    Some(mesh_path(&s))
}

pub fn mesh_path(s: &str) -> String {
    resource_path(s, "meshes/")
}

pub fn texture_path(s: &str) -> String {
    resource_path(s, "textures/")
}

/// Normalise a resource reference into a VFS path rooted at `root` ("meshes/", "textures/").
/// Handles absolute authoring paths such as `skyrimhd\build\pc\data\textures\...`.
fn resource_path(s: &str, root: &str) -> String {
    let n = vfs::normalize_path(s);
    if n.starts_with(root) {
        return n;
    }
    if let Some(i) = n.find(&format!("/{root}")) {
        return n[i + 1..].to_owned();
    }
    let n = n.strip_prefix("data/").unwrap_or(&n);
    format!("{root}{n}")
}

#[derive(Debug, Clone, Copy)]
pub struct LightData {
    pub radius: f32,
    pub color: Vec3,
    pub flags: u32,
    pub falloff: f32,
    pub fov: f32,
    pub fade: f32,
}

impl LightData {
    pub fn negative(&self) -> bool {
        self.flags & 0x4 != 0
    }
    pub fn off_by_default(&self) -> bool {
        self.flags & 0x20 != 0
    }
}

pub fn light_data(rec: &LoadedRecord<'_>) -> Option<LightData> {
    let d = rec.get(b"DATA")?;
    let fade = rec.get(b"FNAM").map(|f| f32_at(f, 0)).unwrap_or(1.0);
    Some(LightData {
        radius: u32_at(d, 4) as f32,
        color: rgb(d, 8),
        flags: u32_at(d, 12),
        falloff: f32_at(d, 16),
        fov: f32_at(d, 20),
        fade,
    })
}

/// Base object record types that are placed in the world with a model.
pub fn is_renderable_base(tag: &[u8; 4]) -> bool {
    matches!(
        tag,
        b"STAT"
            | b"MSTT"
            | b"FURN"
            | b"DOOR"
            | b"CONT"
            | b"LIGH"
            | b"TREE"
            | b"FLOR"
            | b"ACTI"
            | b"MISC"
            | b"WEAP"
            | b"ARMO"
            | b"BOOK"
            | b"ALCH"
            | b"INGR"
            | b"KEYM"
            | b"SLGM"
            | b"AMMO"
            | b"SCRL"
            | b"TACT"
            | b"IDLM"
            | b"BNDS"
            | b"ADDN"
            | b"ARTO"
            | b"GRAS"
    )
}

pub fn water_params(lo: &LoadOrder, id: FormId) -> Option<crate::render::water::WaterParams> {
    let rec = lo.get(id)?;
    let d = rec.get(b"DNAM")?;
    let noise = rec
        .get(b"NAM2")
        .map(esp::decode_zstring)
        .filter(|s| !s.is_empty())
        .map(|s| texture_path(&s))
        .unwrap_or_else(|| "textures/water/defaultwater.dds".into());
    Some(crate::render::water::WaterParams {
        sun_power: f32_at(d, 16),
        reflectivity: f32_at(d, 20),
        fresnel: f32_at(d, 24),
        shallow: rgb(d, 40),
        deep: rgb(d, 44),
        reflection: rgb(d, 48),
        noise_texture: noise,
    })
}
