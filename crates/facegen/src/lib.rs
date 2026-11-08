//! FaceGen heads built from records rather than read pre-built: which head
//! parts a face has and which `.tri` morphs each part gets from the race,
//! the face sliders (NPC_ `NAM9`) and the presets (`NAMA`).
//! `research/facegen.md` has the evidence: a vanilla NPC's pre-built head
//! is its parts' models with these morphs applied.

use esp::{FormId, LoadOrder, LoadedRecord};
use glam::Vec3;

pub mod tint;

/// The face sliders in `NAM9`'s order: the morph a negative value moves
/// toward, then the positive one (`research/facegen.md`).
pub const SLIDERS: [(Option<&str>, &str); 19] = [
    (Some("NoseShort"), "NoseLong"),
    (Some("NoseDown"), "NoseUp"),
    (Some("JawUp"), "JawDown"),
    (Some("JawNarrow"), "JawWide"),
    (Some("JawBack"), "JawForward"),
    (Some("CheeksDown"), "CheeksUp"),
    (Some("CheeksIn"), "CheeksOut"),
    (Some("EyesMoveDown"), "EyesMoveUp"),
    (Some("EyesMoveIn"), "EyesMoveOut"),
    (Some("BrowDown"), "BrowUp"),
    (Some("BrowIn"), "BrowOut"),
    (Some("BrowBack"), "BrowForward"),
    (Some("LipMoveDown"), "LipMoveUp"),
    (Some("LipMoveIn"), "LipMoveOut"),
    (Some("ChinThin"), "ChinWide"),
    (Some("ChinMoveUp"), "ChinMoveDown"),
    (Some("Overbite"), "Underbite"),
    (Some("EyesBack"), "EyesForward"),
    (None, "VampireMorph"),
];

/// Head part types (HDPT `PNAM`).
pub const PART_MISC: u32 = 0;
pub const PART_FACE: u32 = 1;
pub const PART_EYES: u32 = 2;
pub const PART_HAIR: u32 = 3;
pub const PART_FACIAL_HAIR: u32 = 4;
pub const PART_SCAR: u32 = 5;
pub const PART_EYEBROWS: u32 = 6;

/// The `.tri` kinds a head part names (HDPT `NAM0`).
pub const TRI_RACE: u32 = 0;
pub const TRI_EXPRESSION: u32 = 1;
pub const TRI_CHARGEN: u32 = 2;

/// What shapes a face: its race and sex, the parts chosen in place of the
/// race's defaults, the sliders and the presets.
#[derive(Debug, Clone, PartialEq)]
pub struct Face {
    pub race: FormId,
    pub female: bool,
    /// The face's own head parts (NPC_ `PNAM`), each replacing the race's
    /// default of its type.
    pub parts: Vec<FormId>,
    /// `NAM9`, each −1..1.
    pub sliders: [f32; 19],
    /// `NAMA`: nose, eyes and mouth presets (`NoseType<n>`, `EyesType<n>`,
    /// `LipType<n>`).
    pub nose: i32,
    pub eyes: i32,
    pub mouth: i32,
    /// `NAM7` / 100: 0 thin, 1 heavy.
    pub weight: f32,
    /// The face's own texture set (`FTST`).
    pub texture_set: Option<FormId>,
    /// Its hair colour (`HCLF`, a CLFM).
    pub hair_color: Option<FormId>,
}

fn fid_at(rec: &LoadedRecord<'_>, d: &[u8]) -> FormId {
    d.get(0..4)
        .map(|b| rec.fid(FormId(u32::from_le_bytes(b.try_into().unwrap()))))
        .unwrap_or_default()
}

fn f32_at(d: &[u8], i: usize) -> Option<f32> {
    d.get(i..i + 4)
        .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
}

fn i32_at(d: &[u8], i: usize) -> Option<i32> {
    d.get(i..i + 4)
        .map(|b| i32::from_le_bytes(b.try_into().unwrap()))
}

impl Face {
    /// The face an NPC_ record describes (its own fields; templates are the
    /// caller's to follow).
    pub fn of_npc(lo: &LoadOrder, npc: FormId) -> Option<Face> {
        let rec = lo.get(npc).filter(|r| r.tag().0 == *b"NPC_")?;
        let female = rec
            .get(b"ACBS")
            .and_then(|d| d.first())
            .is_some_and(|f| f & 1 != 0);
        let mut sliders = [0.0; 19];
        if let Some(d) = rec.get(b"NAM9") {
            for (i, s) in sliders.iter_mut().enumerate() {
                // The last is 0 or FLT_MAX in vanilla records (UESP: "only
                // ever 00 00 00 00 or FF FF 7F 7F"): taken as unset.
                *s = f32_at(d, i * 4)
                    .filter(|v| v.is_finite() && v.abs() <= 16.0)
                    .unwrap_or(0.0);
            }
        }
        let nama = rec.get(b"NAMA").unwrap_or(&[]);
        Some(Face {
            race: rec
                .get(b"RNAM")
                .map(|d| fid_at(&rec, d))
                .unwrap_or_default(),
            female,
            parts: rec
                .subrecords()
                .filter(|s| s.tag.0 == *b"PNAM")
                .map(|s| fid_at(&rec, s.data))
                .collect(),
            sliders,
            nose: i32_at(nama, 0).unwrap_or(-1),
            eyes: i32_at(nama, 8).unwrap_or(-1),
            mouth: i32_at(nama, 12).unwrap_or(-1),
            weight: rec
                .get(b"NAM7")
                .and_then(|d| f32_at(d, 0))
                .map_or(0.5, |w| (w / 100.0).clamp(0.0, 1.0)),
            texture_set: rec
                .get(b"FTST")
                .map(|d| fid_at(&rec, d))
                .filter(|f| !f.is_null()),
            hair_color: rec
                .get(b"HCLF")
                .map(|d| fid_at(&rec, d))
                .filter(|f| !f.is_null()),
        })
    }
}

/// A head part's type (`PNAM`).
pub fn part_type(lo: &LoadOrder, part: FormId) -> Option<u32> {
    let rec = lo.get(part).filter(|r| r.tag().0 == *b"HDPT")?;
    rec.get(b"PNAM")
        .and_then(|d| d.get(0..4))
        .map(|d| u32::from_le_bytes(d.try_into().unwrap()))
}

/// The race's default head parts for a sex (`HEAD` after `NAM0` and its
/// `MNAM` / `FNAM` marker).
pub fn race_default_parts(lo: &LoadOrder, race: FormId, female: bool) -> Vec<FormId> {
    let Some(rec) = lo.get(race) else {
        return Vec::new();
    };
    let mut head_data = false;
    let mut sex: Option<bool> = None;
    let mut out = Vec::new();
    for sr in rec.subrecords() {
        match &sr.tag.0 {
            b"NAM0" => head_data = true,
            b"MNAM" if head_data => sex = Some(false),
            b"FNAM" if head_data => sex = Some(true),
            b"HEAD" if sex == Some(female) => out.push(fid_at(&rec, sr.data)),
            _ => {}
        }
    }
    out
}

/// The head parts a face shows: the race's defaults for the sex, each
/// replaced by the face's own part of the same type, then every part's
/// extra parts (`HNAM`: hairlines), hair and eyes last (drawn over the
/// head and its other parts).
pub fn head_parts(lo: &LoadOrder, face: &Face) -> Vec<FormId> {
    let mut parts: Vec<(u32, FormId)> = race_default_parts(lo, face.race, face.female)
        .into_iter()
        .filter_map(|id| Some((part_type(lo, id)?, id)))
        .collect();
    for &id in &face.parts {
        let Some(t) = part_type(lo, id) else { continue };
        parts.retain(|&(pt, _)| pt != t);
        parts.push((t, id));
    }
    parts.sort_by_key(|&(t, _)| matches!(t, PART_EYES | PART_HAIR));
    let mut ids: Vec<FormId> = parts.into_iter().map(|(_, id)| id).collect();
    let mut i = 0;
    while i < ids.len() && ids.len() < 32 {
        if let Some(rec) = lo.get(ids[i]) {
            for sr in rec.subrecords().filter(|s| s.tag.0 == *b"HNAM") {
                let extra = fid_at(&rec, sr.data);
                if !ids.contains(&extra) {
                    ids.push(extra);
                }
            }
        }
        i += 1;
    }
    ids
}

/// A race's tint mask for a sex (RACE `TINI` ... `TIRS`, after the head
/// data's sex marker).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TintMask {
    /// The index an NPC's tint layer names (`TINI`).
    pub index: u16,
    /// The mask texture (`TINT`) as a VFS path under `textures/`.
    pub file: String,
    /// What it is (`TINP`: 6 skin tone, 7 paint, 14 dirt, ...; xEdit's
    /// `wbTintMaskTypeEnum`), 0 when absent.
    pub kind: u16,
    /// The colour form it starts with (`TIND`, a CLFM).
    pub default_color: FormId,
    /// The colours offered (`TINC`), their default values (`TINV`) and
    /// indexes (`TIRS`).
    pub presets: Vec<(FormId, f32, u16)>,
}

/// A VFS path under `textures/` for a record's texture path.
pub fn texture_path(s: &str) -> String {
    resource_path(s, "textures/")
}

/// A texture set's (TXST) textures in a FaceGen shader's slots: `TX00`
/// diffuse, `TX01` normal, `TX03` subsurface, `TX04` detail and `TX07`
/// specular go to slots 0, 1, 2, 3 and 7, as the pre-built heads have them
/// (slot 6 is the face tint). Empty where the set has none.
pub fn face_texture_slots(lo: &LoadOrder, txst: FormId) -> Option<[String; 9]> {
    let rec = lo.get(txst).filter(|r| r.tag().0 == *b"TXST")?;
    let tex = |t: &[u8; 4]| {
        rec.get(t)
            .map(esp::decode_zstring)
            .filter(|s| !s.trim().is_empty())
            .map(|s| texture_path(&s))
            .unwrap_or_default()
    };
    let mut out: [String; 9] = Default::default();
    for (slot, tag) in [
        (0, b"TX00"),
        (1, b"TX01"),
        (2, b"TX03"),
        (3, b"TX04"),
        (7, b"TX07"),
    ] {
        out[slot] = tex(tag);
    }
    Some(out)
}

/// The race's tint masks for a sex, in record order.
pub fn race_tint_masks(lo: &LoadOrder, race: FormId, female: bool) -> Vec<TintMask> {
    let Some(rec) = lo.get(race) else {
        return Vec::new();
    };
    let mut head_data = false;
    let mut sex: Option<bool> = None;
    let mut out: Vec<TintMask> = Vec::new();
    for sr in rec.subrecords() {
        match &sr.tag.0 {
            b"NAM0" => head_data = true,
            b"MNAM" if head_data => sex = Some(false),
            b"FNAM" if head_data => sex = Some(true),
            _ if sex != Some(female) => {}
            b"TINI" => out.push(TintMask {
                index: sr.u16(0),
                ..Default::default()
            }),
            b"TINT" => {
                if let Some(m) = out.last_mut() {
                    m.file = texture_path(&sr.zstring());
                }
            }
            b"TINP" => {
                if let Some(m) = out.last_mut() {
                    m.kind = sr.u16(0);
                }
            }
            b"TIND" => {
                if let Some(m) = out.last_mut() {
                    m.default_color = fid_at(&rec, sr.data);
                }
            }
            b"TINC" => {
                if let Some(m) = out.last_mut() {
                    m.presets.push((fid_at(&rec, sr.data), 0.0, 0));
                }
            }
            b"TINV" => {
                if let Some(p) = out.last_mut().and_then(|m| m.presets.last_mut()) {
                    p.1 = sr.f32(0);
                }
            }
            b"TIRS" => {
                if let Some(p) = out.last_mut().and_then(|m| m.presets.last_mut()) {
                    p.2 = sr.u16(0);
                }
            }
            _ => {}
        }
    }
    out
}

/// An NPC's tint layer (NPC_ `TINI` / `TINC` / `TINV` / `TIAS`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TintLayer {
    pub index: u16,
    /// RGBA, 0..255.
    pub color: [u8; 4],
    /// `TINV` / 100.
    pub value: f32,
    pub preset: i16,
}

/// An NPC_ record's tint layers, in record order.
pub fn npc_tint_layers(lo: &LoadOrder, npc: FormId) -> Vec<TintLayer> {
    let Some(rec) = lo.get(npc) else {
        return Vec::new();
    };
    let mut out: Vec<TintLayer> = Vec::new();
    for sr in rec.subrecords() {
        match &sr.tag.0 {
            b"TINI" => out.push(TintLayer {
                index: sr.u16(0),
                ..Default::default()
            }),
            b"TINC" => {
                if let Some(l) = out.last_mut() {
                    l.color = [sr.u8(0), sr.u8(1), sr.u8(2), sr.u8(3)];
                }
            }
            b"TINV" => {
                if let Some(l) = out.last_mut() {
                    l.value = sr.u32(0) as f32 / 100.0;
                }
            }
            b"TIAS" => {
                if let Some(l) = out.last_mut() {
                    l.preset = sr.u16(0) as i16;
                }
            }
            _ => {}
        }
    }
    out
}

/// An NPC_ record's skin tone (`QNAM`, RGB 0..1).
pub fn npc_skin_tone(lo: &LoadOrder, npc: FormId) -> Option<[f32; 3]> {
    let rec = lo.get(npc)?;
    let d = rec.get(b"QNAM")?;
    Some([f32_at(d, 0)?, f32_at(d, 4)?, f32_at(d, 8)?])
}

/// A colour form's colour (CLFM `CNAM`, RGB 0..255).
pub fn clfm_color(lo: &LoadOrder, clfm: FormId) -> Option<[u8; 3]> {
    let rec = lo.get(clfm).filter(|r| r.tag().0 == *b"CLFM")?;
    let d = rec.get(b"CNAM")?;
    Some([*d.first()?, *d.get(1)?, *d.get(2)?])
}

/// The race's default face texture set for a sex (`DFTM` / `DFTF`).
pub fn race_default_face_texture(lo: &LoadOrder, race: FormId, female: bool) -> Option<FormId> {
    let rec = lo.get(race)?;
    let d = rec.get(if female { b"DFTF" } else { b"DFTM" })?;
    Some(fid_at(&rec, d)).filter(|f| !f.is_null())
}

/// The texture set a face's head shows: the face's own (NPC_ `FTST`), else
/// its face part's (HDPT `TNAM`), else its race's default
/// (`research/facegen.md`, "Texture sets").
pub fn face_texture_set(
    lo: &LoadOrder,
    own: Option<FormId>,
    face_part: Option<FormId>,
    race: FormId,
    female: bool,
) -> Option<FormId> {
    own.filter(|f| !f.is_null())
        .or_else(|| {
            let rec = lo.get(face_part?)?;
            Some(fid_at(&rec, rec.get(b"TNAM")?)).filter(|f| !f.is_null())
        })
        .or_else(|| race_default_face_texture(lo, race, female))
}

/// A VFS path under `meshes/` for a record's model path.
pub fn mesh_path(s: &str) -> String {
    resource_path(s, "meshes/")
}

/// A resource path rooted at `root` (`meshes/`, `textures/`), also from
/// authoring paths such as `data\textures\...`.
fn resource_path(s: &str, root: &str) -> String {
    let n = vfs::normalize_path(s);
    if n.starts_with(root) {
        return n;
    }
    if let Some(i) = n.find(&format!("/{root}")) {
        return n[i + 1..].to_owned();
    }
    format!("{root}{}", n.strip_prefix("data/").unwrap_or(&n))
}

/// A head part's model (`MODL`) as a VFS path.
pub fn part_model(lo: &LoadOrder, part: FormId) -> Option<String> {
    let rec = lo.get(part)?;
    let m = esp::decode_zstring(rec.get(b"MODL")?);
    (!m.is_empty()).then(|| mesh_path(&m))
}

/// The `.tri` file a head part names for a kind (`NAM0` / `NAM1`).
pub fn part_tri(lo: &LoadOrder, part: FormId, kind: u32) -> Option<String> {
    let rec = lo.get(part)?;
    let mut current = None;
    for sr in rec.subrecords() {
        match &sr.tag.0 {
            b"NAM0" => current = Some(sr.u32(0)),
            b"NAM1" if current == Some(kind) => {
                let s = sr.zstring();
                return (!s.is_empty()).then(|| mesh_path(&s));
            }
            _ => {}
        }
    }
    None
}

/// A morph of a `.tri` file, at a weight.
#[derive(Debug, Clone, PartialEq)]
pub struct MorphUse {
    pub file: String,
    pub morph: String,
    pub weight: f32,
}

/// The editor ID of the race morph a race's heads take: its morph race's
/// (`NAM8`: vampire races name their living race, `DremoraRace` names
/// `DarkElfRace`), else its own.
pub fn race_morph(lo: &LoadOrder, race: FormId) -> Option<String> {
    let rec = lo.get(race)?;
    let morph = rec
        .get(b"NAM8")
        .map(|d| fid_at(&rec, d))
        .filter(|f| !f.is_null())
        .and_then(|f| lo.get(f));
    match morph {
        Some(m) => m.editor_id(),
        None => rec.editor_id(),
    }
}

/// Whether a race has the `Vampire` keyword (`KWDA`), as the vampire races
/// do and their living races don't.
pub fn race_is_vampire(lo: &LoadOrder, race: FormId) -> bool {
    let Some(rec) = lo.get(race) else {
        return false;
    };
    rec.get(b"KWDA").is_some_and(|d| {
        d.chunks_exact(4).any(|c| {
            lo.get(fid_at(&rec, c))
                .and_then(|k| k.editor_id())
                .is_some_and(|e| e.eq_ignore_ascii_case("Vampire"))
        })
    })
}

/// The morphs a head part gets for a face: its race's morph, the sliders
/// and the presets, from the part's race and chargen `.tri` files, and
/// thinness from its expression file's `SkinnyMorph` (× 1 − weight).
pub fn part_morphs(lo: &LoadOrder, part: FormId, face: &Face) -> Vec<MorphUse> {
    let mut out = Vec::new();
    if let Some(file) = part_tri(lo, part, TRI_EXPRESSION)
        && face.weight < 1.0
    {
        out.push(MorphUse {
            file,
            morph: "SkinnyMorph".into(),
            weight: 1.0 - face.weight,
        });
    }
    if let Some(file) = part_tri(lo, part, TRI_RACE)
        && let Some(morph) = race_morph(lo, face.race)
    {
        out.push(MorphUse {
            file,
            morph,
            weight: 1.0,
        });
    }
    // Vampires' heads take the whole `VampireMorph`: from the chargen file,
    // or the expression file of parts without one (children's mouths).
    if race_is_vampire(lo, face.race)
        && let Some(file) =
            part_tri(lo, part, TRI_CHARGEN).or_else(|| part_tri(lo, part, TRI_EXPRESSION))
    {
        out.push(MorphUse {
            file,
            morph: "VampireMorph".into(),
            weight: 1.0,
        });
    }
    if let Some(file) = part_tri(lo, part, TRI_CHARGEN) {
        for (&(neg, pos), &v) in SLIDERS.iter().zip(&face.sliders) {
            let name = if v > 0.0 { Some(pos) } else { neg };
            if let Some(name) = name.filter(|_| v != 0.0) {
                out.push(MorphUse {
                    file: file.clone(),
                    morph: name.into(),
                    weight: v.abs(),
                });
            }
        }
        for (prefix, n) in [
            ("NoseType", face.nose),
            ("EyesType", face.eyes),
            ("LipType", face.mouth),
        ] {
            if n >= 0 {
                out.push(MorphUse {
                    file: file.clone(),
                    morph: format!("{prefix}{n}"),
                    weight: 1.0,
                });
            }
        }
    }
    out
}

/// Morphs written into a model path's variant (`<model>#facegen=<this>`):
/// `file|morph|weight` entries joined by `;` (no path has those, or `#`).
pub fn encode_morphs(uses: &[MorphUse]) -> String {
    uses.iter()
        .map(|u| format!("{}|{}|{}", u.file, u.morph, u.weight))
        .collect::<Vec<_>>()
        .join(";")
}

/// The morphs [`encode_morphs`] wrote; malformed entries are left out.
pub fn decode_morphs(s: &str) -> Vec<MorphUse> {
    s.split(';')
        .filter_map(|e| {
            let mut it = e.split('|');
            let (file, morph, weight) = (it.next()?, it.next()?, it.next()?);
            Some(MorphUse {
                file: file.into(),
                morph: morph.into(),
                weight: weight.parse().ok()?,
            })
        })
        .collect()
}

/// How a head part built live differs from its model, carried in its model
/// path (`<model>#facegen=<encoded>`): its morphs, and for its FaceGen
/// shaders (type 4) a texture set and the face tint, for its hair-tinted
/// shaders (type 6) the hair colour.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PartVariant {
    pub morphs: Vec<MorphUse>,
    /// The face tint texture (slot 6).
    pub tint: Option<String>,
    /// Texture slots replacing the model's where not empty
    /// ([`face_texture_slots`]).
    pub textures: Option<[String; 9]>,
    /// The hair tint colour (2 × the CLFM's colour / 255).
    pub hair: Option<[f32; 3]>,
}

impl PartVariant {
    pub fn is_empty(&self) -> bool {
        *self == PartVariant::default()
    }

    /// The variant string: the morphs ([`encode_morphs`]), then `!tint=`,
    /// `!tex=` (slots joined by `|`) and `!hair=r,g,b` where set. No path
    /// has `!`, `|`, `;` or `#`.
    pub fn encode(&self) -> String {
        let mut s = encode_morphs(&self.morphs);
        if let Some(t) = &self.tint {
            s.push_str("!tint=");
            s.push_str(t);
        }
        if let Some(t) = &self.textures {
            s.push_str("!tex=");
            s.push_str(&t.join("|"));
        }
        if let Some([r, g, b]) = self.hair {
            s.push_str(&format!("!hair={r},{g},{b}"));
        }
        s
    }

    pub fn decode(s: &str) -> PartVariant {
        let mut parts = s.split('!');
        let mut v = PartVariant {
            morphs: decode_morphs(parts.next().unwrap_or("")),
            ..Default::default()
        };
        for p in parts {
            if let Some(t) = p.strip_prefix("tint=") {
                v.tint = Some(t.to_owned());
            } else if let Some(t) = p.strip_prefix("tex=") {
                let slots: Vec<&str> = t.split('|').collect();
                if slots.len() == 9 {
                    v.textures = Some(std::array::from_fn(|i| slots[i].to_owned()));
                }
            } else if let Some(h) = p.strip_prefix("hair=") {
                let c: Vec<f32> = h.split(',').filter_map(|x| x.parse().ok()).collect();
                if let [r, g, b] = c[..] {
                    v.hair = Some([r, g, b]);
                }
            }
        }
        v
    }

    /// Apply it to a parsed model: its shapes morphed (see [`apply`]), its
    /// shaders' textures and hair colour set. Returns how many morphs
    /// applied over all shapes.
    pub fn apply_to(
        &self,
        n: &mut nif::Nif,
        mut read: impl FnMut(&str) -> Option<std::sync::Arc<nif::tri::Tri>>,
    ) -> usize {
        let mut sets = Vec::new();
        for b in &mut n.blocks {
            if let nif::Block::LightingShader(s) = b {
                if s.shader_type == 4 && (self.tint.is_some() || self.textures.is_some()) {
                    sets.push(s.texture_set);
                }
                if s.shader_type == 6
                    && let Some(h) = self.hair
                {
                    s.hair_tint_color = Vec3::from(h);
                }
            }
        }
        for r in sets {
            if let Some(nif::Block::TextureSet(t)) = n.get_mut(r) {
                if t.len() < 9 {
                    t.resize(9, String::new());
                }
                if let Some(slots) = &self.textures {
                    for (dst, src) in t.iter_mut().zip(slots) {
                        if !src.is_empty() {
                            *dst = src.clone();
                        }
                    }
                }
                if let Some(tint) = &self.tint {
                    t[6] = tint.clone();
                }
            }
        }
        let mut applied = 0;
        for b in &mut n.blocks {
            if let nif::Block::TriShape(t) = b {
                applied += apply(&mut t.geometry.positions, &self.morphs, &mut read);
            }
        }
        applied
    }
}

/// Everything a head part built live for a face takes: its morphs, the
/// hair colour (the face's, else its race's default), and for the face
/// part, its texture set ([`face_texture_set`]) and `tint`.
pub fn part_variant(lo: &LoadOrder, part: FormId, face: &Face, tint: Option<&str>) -> PartVariant {
    let is_face = part_type(lo, part) == Some(PART_FACE);
    PartVariant {
        morphs: part_morphs(lo, part, face),
        tint: tint.filter(|_| is_face).map(str::to_owned),
        textures: is_face
            .then(|| face_texture_set(lo, face.texture_set, Some(part), face.race, face.female))
            .flatten()
            .and_then(|t| face_texture_slots(lo, t)),
        hair: face
            .hair_color
            .or_else(|| race_default_hair_color(lo, face.race, face.female))
            .and_then(|c| hair_tint(lo, c)),
    }
}

/// The hair tint a hair colour (CLFM) gives hair-tinted shaders: twice its
/// colour, as the pre-built heads have it (`research/facegen.md`).
pub fn hair_tint(lo: &LoadOrder, clfm: FormId) -> Option<[f32; 3]> {
    clfm_color(lo, clfm).map(|c| c.map(|v| v as f32 * 2.0 / 255.0))
}

/// The race's default hair colour for a sex (RACE `HCLF`: male, female).
pub fn race_default_hair_color(lo: &LoadOrder, race: FormId, female: bool) -> Option<FormId> {
    let rec = lo.get(race)?;
    let d = rec.get(b"HCLF")?;
    let at = if female { 4 } else { 0 };
    Some(fid_at(&rec, d.get(at..at + 4)?)).filter(|f| !f.is_null())
}

/// Apply morphs to a part's vertices. A morph whose file can't be read, or
/// whose vertex count differs from the model's, or that the file lacks, is
/// skipped; returns how many applied.
pub fn apply(
    positions: &mut [Vec3],
    uses: &[MorphUse],
    mut read: impl FnMut(&str) -> Option<std::sync::Arc<nif::tri::Tri>>,
) -> usize {
    let mut applied = 0;
    for u in uses {
        let Some(tri) = read(&u.file) else { continue };
        if tri.positions.len() != positions.len() {
            continue;
        }
        let Some(m) = tri.morph(&u.morph) else {
            continue;
        };
        for (p, d) in positions.iter_mut().zip(&m.deltas) {
            *p += *d * u.weight;
        }
        applied += 1;
    }
    applied
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sliders_pick_the_morph_their_sign_names() {
        let mut face = Face {
            race: FormId::NULL,
            female: false,
            parts: Vec::new(),
            sliders: [0.0; 19],
            nose: -1,
            eyes: -1,
            mouth: -1,
            weight: 1.0,
            texture_set: None,
            hair_color: None,
        };
        face.sliders[0] = -0.5;
        face.sliders[17] = 0.25;
        let picked: Vec<(&str, f32)> = SLIDERS
            .iter()
            .zip(&face.sliders)
            .filter(|(_, v)| **v != 0.0)
            .filter_map(|(&(n, p), &v)| Some((if v > 0.0 { p } else { n? }, v.abs())))
            .collect();
        assert_eq!(picked, vec![("NoseShort", 0.5), ("EyesForward", 0.25)]);
    }

    #[test]
    fn resource_paths_root_authoring_paths() {
        assert_eq!(
            texture_path("data\\Textures\\Actors\\A.dds"),
            "textures/actors/a.dds"
        );
        assert_eq!(texture_path("Actors\\A.dds"), "textures/actors/a.dds");
        assert_eq!(mesh_path("meshes\\b.nif"), "meshes/b.nif");
    }

    #[test]
    fn morph_lists_round_trip_through_a_model_variant() {
        let uses = vec![
            MorphUse {
                file: "meshes/actors/character/character assets/maleheadraces.tri".into(),
                morph: "NordRace".into(),
                weight: 1.0,
            },
            MorphUse {
                file: "meshes/a.tri".into(),
                morph: "NoseShort".into(),
                weight: 0.25,
            },
        ];
        assert_eq!(decode_morphs(&encode_morphs(&uses)), uses);
        assert!(decode_morphs("").is_empty());
    }

    #[test]
    fn part_variants_round_trip() {
        let v = PartVariant {
            morphs: vec![MorphUse {
                file: "meshes/a.tri".into(),
                morph: "NordRace".into(),
                weight: 1.0,
            }],
            tint: Some("textures/vrm/facetint/1.dds".into()),
            textures: Some(std::array::from_fn(|i| {
                if i == 1 {
                    "textures/n.dds".into()
                } else {
                    String::new()
                }
            })),
            hair: Some([0.25, 0.5, 1.0]),
        };
        assert_eq!(PartVariant::decode(&v.encode()), v);
        assert!(PartVariant::decode("").is_empty());
    }

    #[test]
    fn apply_skips_mismatched_files_and_missing_morphs() {
        let tri = std::sync::Arc::new(nif::tri::Tri {
            positions: vec![Vec3::ZERO; 2],
            morphs: vec![nif::tri::Morph {
                name: "NordRace".into(),
                deltas: vec![Vec3::X, Vec3::Y],
            }],
        });
        let uses = [
            MorphUse {
                file: "a.tri".into(),
                morph: "nordrace".into(),
                weight: 0.5,
            },
            MorphUse {
                file: "a.tri".into(),
                morph: "Missing".into(),
                weight: 1.0,
            },
        ];
        let mut p = vec![Vec3::ZERO; 2];
        assert_eq!(apply(&mut p, &uses, |_| Some(tri.clone())), 1);
        assert_eq!(p, vec![Vec3::X * 0.5, Vec3::Y * 0.5]);
        let mut short = vec![Vec3::ZERO; 3];
        assert_eq!(apply(&mut short, &uses, |_| Some(tri.clone())), 0);
    }
}
