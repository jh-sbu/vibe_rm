//! The player's appearance as engine state (`plans/character-creation.md`,
//! step 5): race, sex, weight, head parts, face sliders and presets, texture
//! set, hair colour, skin tone and tint layers. Until something changes it
//! (the race menu, `SetRace`), the player's NPC record gives it. Saved as
//! `PAPP`.

use esp::{FormId, LoadOrder};

use crate::engine::Engine;
use crate::save::codec::{Reader, Writer};

/// What the player looks like: a FaceGen face (`facegen::Face`), the skin
/// tone (NPC_ `QNAM`, RGB 0..1) and the tint layers drawn over it.
#[derive(Debug, Clone, PartialEq)]
pub struct Appearance {
    pub face: facegen::Face,
    pub skin: [f32; 3],
    pub tints: Vec<facegen::TintLayer>,
}

impl Appearance {
    /// The appearance an NPC_ record gives.
    pub fn of_npc(lo: &LoadOrder, npc: FormId) -> Option<Appearance> {
        Some(Appearance {
            face: facegen::Face::of_npc(lo, npc)?,
            skin: facegen::npc_skin_tone(lo, npc).unwrap_or([1.0; 3]),
            tints: facegen::npc_tint_layers(lo, npc),
        })
    }

    pub fn write(&self, w: &mut Writer) {
        let f = &self.face;
        w.form(f.race);
        w.bool(f.female);
        w.f32(f.weight);
        w.len(f.parts.len());
        for &p in &f.parts {
            w.form(p);
        }
        for &s in &f.sliders {
            w.f32(s);
        }
        w.i32(f.nose);
        w.i32(f.eyes);
        w.i32(f.mouth);
        w.form(f.texture_set.unwrap_or_default());
        w.form(f.hair_color.unwrap_or_default());
        for &c in &self.skin {
            w.f32(c);
        }
        w.len(self.tints.len());
        for t in &self.tints {
            w.u16(t.index);
            for &c in &t.color {
                w.u8(c);
            }
            w.f32(t.value);
            w.u16(t.preset as u16);
        }
    }

    pub fn read(r: &mut Reader<'_>) -> crate::save::codec::Result<Appearance> {
        let race = r.form()?;
        let female = r.bool()?;
        let weight = r.f32()?;
        let mut parts = Vec::new();
        for _ in 0..r.count(4)? {
            parts.push(r.form()?);
        }
        let mut sliders = [0.0; 19];
        for s in &mut sliders {
            *s = r.f32()?;
        }
        let (nose, eyes, mouth) = (r.i32()?, r.i32()?, r.i32()?);
        let some = |f: FormId| (!f.is_null()).then_some(f);
        let texture_set = some(r.form()?);
        let hair_color = some(r.form()?);
        let skin = [r.f32()?, r.f32()?, r.f32()?];
        let mut tints = Vec::new();
        for _ in 0..r.count(12)? {
            let index = r.u16()?;
            let color = [r.u8()?, r.u8()?, r.u8()?, r.u8()?];
            let value = r.f32()?;
            let preset = r.u16()? as i16;
            tints.push(facegen::TintLayer {
                index,
                color,
                value,
                preset,
            });
        }
        Ok(Appearance {
            face: facegen::Face {
                race,
                female,
                parts,
                sliders,
                nose,
                eyes,
                mouth,
                weight,
                texture_set,
                hair_color,
            },
            skin,
            tints,
        })
    }
}

impl Engine {
    /// The player's appearance: as last set, else their NPC record's.
    pub fn player_appearance(&self) -> Option<Appearance> {
        self.player_appearance
            .clone()
            .or_else(|| Appearance::of_npc(&self.lo, crate::player_body::PLAYER_NPC))
    }

    /// Change the player's appearance and rebuild their body with it.
    pub fn set_player_appearance(&mut self, a: Appearance) {
        if self.player_appearance.as_ref() == Some(&a) {
            return;
        }
        self.player_appearance = Some(a);
        self.build_player_body();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slider_ids_round_trip() {
        let kinds = [
            SliderKind::Sex,
            SliderKind::Preset,
            SliderKind::SkinTone,
            SliderKind::Weight,
            SliderKind::Complexion,
            SliderKind::Dirt,
            SliderKind::WarPaint,
            SliderKind::WarPaintColor,
            SliderKind::Hair,
            SliderKind::HairColor,
            SliderKind::Beard,
            SliderKind::Scar,
            SliderKind::Eyes,
            SliderKind::Brows,
            SliderKind::NoseType,
            SliderKind::EyeShape,
            SliderKind::MouthShape,
            SliderKind::Morph(0),
            SliderKind::Morph(17),
            SliderKind::TintColor(1),
            SliderKind::TintValue(13),
        ];
        for k in kinds {
            assert_eq!(SliderKind::from_id(k.id()), Some(k));
        }
        assert_eq!(SliderKind::from_id(99), None);
    }

    #[test]
    fn appearances_round_trip_through_their_save_payload() {
        let mut sliders = [0.0; 19];
        sliders[3] = -0.5;
        let a = Appearance {
            face: facegen::Face {
                race: FormId(0x13746),
                female: true,
                parts: vec![FormId(0x51407), FormId(0xE7AEE)],
                sliders,
                nose: 5,
                eyes: -1,
                mouth: 10,
                weight: 0.25,
                texture_set: Some(FormId(0x3B521)),
                hair_color: None,
            },
            skin: [0.7, 0.6, 0.5],
            tints: vec![facegen::TintLayer {
                index: 8,
                color: [172, 0, 0, 0],
                value: 0.19,
                preset: -1,
            }],
        };
        let mut w = Writer::default();
        a.write(&mut w);
        let mut r = Reader::new(&w.0);
        assert_eq!(Appearance::read(&mut r).unwrap(), a);
        r.finish().unwrap();
    }
}

/// The race menu's categories (`SetCategoriesList`): label and filter bit.
/// The movie fixes the first three as Race, Body and Head
/// (`RaceSexPanels.RACE_CATEGORY` ...); the rest, and every label, are
/// provisional (`known_ambiguities/chargen.md`).
pub const CATEGORIES: [(&str, u32); 8] = [
    ("RACE", 1),
    ("BODY", 2),
    ("HEAD", 4),
    ("FACE", 8),
    ("EYES", 16),
    ("BROW", 32),
    ("MOUTH", 64),
    ("MAKEUP", 128),
];

const BODY: u32 = 2;
const HEAD: u32 = 4;
const FACE: u32 = 8;
const EYES: u32 = 16;
const BROW: u32 = 32;
const MOUTH: u32 = 64;
const MAKEUP: u32 = 128;

/// What a slider changes; its `sliderID` in the movie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliderKind {
    Sex,
    Preset,
    SkinTone,
    Weight,
    Complexion,
    Dirt,
    WarPaint,
    WarPaintColor,
    Hair,
    HairColor,
    Beard,
    Scar,
    Eyes,
    Brows,
    NoseType,
    EyeShape,
    MouthShape,
    /// A face slider (`NAM9`'s index).
    Morph(usize),
    /// A tint mask type's colour (`TINP`).
    TintColor(u16),
    /// A tint mask type's strength.
    TintValue(u16),
}

impl SliderKind {
    pub fn id(self) -> u32 {
        match self {
            SliderKind::Sex => 0,
            SliderKind::Preset => 1,
            SliderKind::SkinTone => 2,
            SliderKind::Weight => 3,
            SliderKind::Complexion => 4,
            SliderKind::Dirt => 5,
            SliderKind::WarPaint => 6,
            SliderKind::WarPaintColor => 7,
            SliderKind::Hair => 10,
            SliderKind::HairColor => 11,
            SliderKind::Beard => 12,
            SliderKind::Scar => 13,
            SliderKind::Eyes => 14,
            SliderKind::Brows => 15,
            SliderKind::NoseType => 20,
            SliderKind::EyeShape => 21,
            SliderKind::MouthShape => 22,
            SliderKind::Morph(i) => 100 + i as u32,
            SliderKind::TintColor(k) => 200 + k as u32,
            SliderKind::TintValue(k) => 300 + k as u32,
        }
    }

    pub fn from_id(id: u32) -> Option<SliderKind> {
        Some(match id {
            0 => SliderKind::Sex,
            1 => SliderKind::Preset,
            2 => SliderKind::SkinTone,
            3 => SliderKind::Weight,
            4 => SliderKind::Complexion,
            5 => SliderKind::Dirt,
            6 => SliderKind::WarPaint,
            7 => SliderKind::WarPaintColor,
            10 => SliderKind::Hair,
            11 => SliderKind::HairColor,
            12 => SliderKind::Beard,
            13 => SliderKind::Scar,
            14 => SliderKind::Eyes,
            15 => SliderKind::Brows,
            20 => SliderKind::NoseType,
            21 => SliderKind::EyeShape,
            22 => SliderKind::MouthShape,
            100..=117 => SliderKind::Morph((id - 100) as usize),
            200..=215 => SliderKind::TintColor((id - 200) as u16),
            300..=315 => SliderKind::TintValue((id - 300) as u16),
            _ => return None,
        })
    }
}

/// A slider as the movie takes it (`SetOptionSliders`).
#[derive(Debug, Clone, PartialEq)]
pub struct Slider {
    pub label: String,
    pub filter: u32,
    pub min: f32,
    pub max: f32,
    pub position: f32,
    pub interval: f32,
    pub kind: SliderKind,
}

/// The face slider labels, in `NAM9`'s order (xEdit's and UESP's names).
const MORPH_LABELS: [(&str, u32); 18] = [
    ("Nose Length", FACE),
    ("Nose Height", FACE),
    ("Jaw Height", FACE),
    ("Jaw Width", FACE),
    ("Jaw Forward", FACE),
    ("Cheekbone Height", FACE),
    ("Cheekbone Width", FACE),
    ("Eye Height", EYES),
    ("Eye Depth In/Out", EYES),
    ("Brow Height", BROW),
    ("Brow Width", BROW),
    ("Brow Forward", BROW),
    ("Mouth Height", MOUTH),
    ("Mouth Forward", MOUTH),
    ("Chin Width", FACE),
    ("Chin Length", FACE),
    ("Chin Forward", FACE),
    ("Eye Forward", EYES),
];

/// Tint mask types (`TINP`) offered as makeup: their labels.
const TINT_LABELS: [(u16, &str); 11] = [
    (1, "Lip Color"),
    (2, "Cheek Color"),
    (3, "Eyeliner Color"),
    (4, "Upper Eye Socket"),
    (5, "Lower Eye Socket"),
    (8, "Laugh Lines"),
    (9, "Cheek Color Lower"),
    (10, "Nose Color"),
    (11, "Chin Color"),
    (12, "Neck Color"),
    (13, "Forehead Color"),
];

const MASK_PAINT: u16 = 7;
const MASK_DIRT: u16 = 14;

/// The races the menu offers: playable (`DATA` flags, offset 32, 0x1), not
/// vampires.
/// Ordered by name.
pub fn playable_races(lo: &LoadOrder) -> Vec<FormId> {
    let mut races: Vec<(String, FormId)> = lo
        .ids_of_type(b"RACE")
        .iter()
        .copied()
        .filter(|&r| {
            lo.get(r)
                .and_then(|rec| {
                    rec.get(b"DATA").map(|d| {
                        d.len() >= 36 && u32::from_le_bytes(d[32..36].try_into().unwrap()) & 1 != 0
                    })
                })
                .unwrap_or(false)
                && !facegen::race_is_vampire(lo, r)
        })
        .map(|r| (race_name(lo, r), r))
        .collect();
    races.sort();
    races.dedup_by_key(|(_, r)| *r);
    races.into_iter().map(|(_, r)| r).collect()
}

/// A race's name (`FULL`).
pub fn race_name(lo: &LoadOrder, race: FormId) -> String {
    lo.get(race)
        .and_then(|r| r.get(b"FULL").map(|d| lo.lstring(&r, d)))
        .unwrap_or_default()
}

/// A race's description (`DESC`).
pub fn race_description(lo: &LoadOrder, race: FormId) -> String {
    lo.get(race)
        .and_then(|r| r.get(b"DESC").map(|d| lo.lstring(&r, d)))
        .unwrap_or_default()
}

/// The race's presets for a sex (`RPRM` / `RPRF`: NPCs).
pub fn race_presets(lo: &LoadOrder, race: FormId, female: bool) -> Vec<FormId> {
    race_list(lo, race, if female { b"RPRF" } else { b"RPRM" })
}

/// A list of forms in the race's head data (each sex has its own tags:
/// `RPRM` / `RPRF`, `AHCM` / `AHCF`, `FTSM` / `FTSF`).
fn race_list(lo: &LoadOrder, race: FormId, tag: &[u8; 4]) -> Vec<FormId> {
    let Some(rec) = lo.get(race) else {
        return Vec::new();
    };
    rec.subrecords()
        .filter(|s| s.tag.0 == *tag && s.data.len() >= 4)
        .map(|s| rec.fid(s.form_id(0)))
        .filter(|f| !f.is_null())
        .collect()
}

/// The head parts of a type a race and sex can choose: playable (HDPT
/// `DATA` 0x1), for the sex (0x2 male, 0x4 female), not extra parts (0x8),
/// and valid for the race (`RNAM`, a form list). In record order.
pub fn choosable_parts(lo: &LoadOrder, race: FormId, female: bool, part_type: u32) -> Vec<FormId> {
    lo.ids_of_type(b"HDPT")
        .iter()
        .copied()
        .filter(|&p| {
            let Some(rec) = lo.get(p) else { return false };
            let flags = rec
                .get(b"DATA")
                .and_then(|d| d.first().copied())
                .unwrap_or(0);
            let sex = if female { 0x4 } else { 0x2 };
            if flags & 0x1 == 0 || flags & sex == 0 || flags & 0x8 != 0 {
                return false;
            }
            if facegen::part_type(lo, p) != Some(part_type) {
                return false;
            }
            let Some(list) = rec
                .get(b"RNAM")
                .filter(|d| d.len() >= 4)
                .map(|d| rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap()))))
            else {
                return false;
            };
            lo.get(list).is_some_and(|l| {
                l.subrecords()
                    .filter(|s| s.tag.0 == *b"LNAM")
                    .any(|s| l.fid(s.form_id(0)) == race)
            })
        })
        .collect()
}

/// The part of a type a face has: its own, else its race's default.
fn current_part(lo: &LoadOrder, face: &facegen::Face, part_type: u32) -> Option<FormId> {
    face.parts
        .iter()
        .copied()
        .find(|&p| facegen::part_type(lo, p) == Some(part_type))
        .or_else(|| {
            facegen::race_default_parts(lo, face.race, face.female)
                .into_iter()
                .find(|&p| facegen::part_type(lo, p) == Some(part_type))
        })
}

/// How many of a preset's morphs (`NoseType<n>`) the face part's chargen
/// file has: the highest n.
fn preset_count(face_tri: Option<&nif::tri::Tri>, prefix: &str) -> i32 {
    face_tri.map_or(0, |t| {
        t.morphs
            .iter()
            .filter_map(|m| m.name.strip_prefix(prefix)?.parse::<i32>().ok())
            .max()
            .unwrap_or(0)
    })
}

impl Appearance {
    fn layer(&self, index: u16) -> Option<&facegen::TintLayer> {
        self.tints.iter().find(|l| l.index == index)
    }

    /// Set (or add) the tint layer of a mask.
    fn set_layer(&mut self, index: u16, color: [u8; 4], value: f32, preset: i16) {
        match self.tints.iter_mut().find(|l| l.index == index) {
            Some(l) => {
                l.color = color;
                l.value = value;
                l.preset = preset;
            }
            None => self.tints.push(facegen::TintLayer {
                index,
                color,
                value,
                preset,
            }),
        }
    }

    /// Put a part of its type in place of the face's own of that type.
    fn set_part(&mut self, lo: &LoadOrder, part: FormId) {
        let t = facegen::part_type(lo, part);
        self.face.parts.retain(|&p| facegen::part_type(lo, p) != t);
        self.face.parts.push(part);
    }
}

fn color_of(lo: &LoadOrder, clfm: FormId) -> [u8; 4] {
    let [r, g, b] = facegen::clfm_color(lo, clfm).unwrap_or([255; 3]);
    [r, g, b, 0]
}

/// The sliders for an appearance: what the race and sex offer, each at the
/// appearance's current choice. `face_tri` is the face part's chargen
/// `.tri` (for how many presets it has).
pub fn sliders(lo: &LoadOrder, a: &Appearance, face_tri: Option<&nif::tri::Tri>) -> Vec<Slider> {
    let f = &a.face;
    let mut out = Vec::new();
    let mut push = |label: &str, filter: u32, max: f32, position: f32, kind: SliderKind| {
        out.push(Slider {
            label: label.into(),
            filter,
            min: 0.0,
            max: max.max(0.0),
            position,
            interval: 1.0,
            kind,
        });
    };
    let index_of = |list: &[FormId], f: Option<FormId>| {
        f.and_then(|f| list.iter().position(|&x| x == f))
            .unwrap_or(0) as f32
    };
    push("Sex", BODY, 1.0, f.female as u8 as f32, SliderKind::Sex);
    let presets = race_presets(lo, f.race, f.female);
    push(
        "Presets",
        BODY,
        presets.len() as f32,
        0.0,
        SliderKind::Preset,
    );
    let masks = facegen::race_tint_masks(lo, f.race, f.female);
    if let Some(skin) = masks
        .iter()
        .find(|m| m.kind == facegen::tint::MASK_SKIN_TONE)
    {
        let pos = a
            .layer(skin.index)
            .and_then(|l| skin.presets.iter().position(|p| p.2 as i16 == l.preset))
            .unwrap_or(0);
        push(
            "Skin Tone",
            BODY,
            skin.presets.len().saturating_sub(1) as f32,
            pos as f32,
            SliderKind::SkinTone,
        );
    }
    push(
        "Weight",
        BODY,
        100.0,
        (f.weight * 100.0).round(),
        SliderKind::Weight,
    );
    let complexions = race_list(lo, f.race, if f.female { b"FTSF" } else { b"FTSM" });
    push(
        "Complexion",
        BODY,
        complexions.len().saturating_sub(1) as f32,
        index_of(&complexions, f.texture_set),
        SliderKind::Complexion,
    );
    let chosen_mask = |kind: u16| {
        masks
            .iter()
            .filter(|m| m.kind == kind)
            .position(|m| a.layer(m.index).is_some_and(|l| l.value > 0.0))
            .map_or(0.0, |i| (i + 1) as f32)
    };
    let dirt = masks.iter().filter(|m| m.kind == MASK_DIRT).count();
    push(
        "Dirt",
        BODY,
        dirt as f32,
        chosen_mask(MASK_DIRT),
        SliderKind::Dirt,
    );
    let paints = masks.iter().filter(|m| m.kind == MASK_PAINT).count();
    push(
        "War Paint",
        HEAD,
        paints as f32,
        chosen_mask(MASK_PAINT),
        SliderKind::WarPaint,
    );
    let paint_presets = masks
        .iter()
        .find(|m| m.kind == MASK_PAINT)
        .map_or(0, |m| m.presets.len());
    push(
        "War Paint Color",
        HEAD,
        paint_presets.saturating_sub(1) as f32,
        0.0,
        SliderKind::WarPaintColor,
    );
    for (label, filter, t, kind) in [
        ("Hair", HEAD, facegen::PART_HAIR, SliderKind::Hair),
        ("Beard", HEAD, facegen::PART_FACIAL_HAIR, SliderKind::Beard),
        ("Face Scars", HEAD, facegen::PART_SCAR, SliderKind::Scar),
        ("Eyes", EYES, facegen::PART_EYES, SliderKind::Eyes),
        ("Brow Type", BROW, facegen::PART_EYEBROWS, SliderKind::Brows),
    ] {
        let parts = choosable_parts(lo, f.race, f.female, t);
        if parts.is_empty() {
            continue;
        }
        let pos = index_of(&parts, current_part(lo, f, t));
        push(label, filter, (parts.len() - 1) as f32, pos, kind);
    }
    let colors = race_list(lo, f.race, if f.female { b"AHCF" } else { b"AHCM" });
    push(
        "Hair Color",
        HEAD,
        colors.len().saturating_sub(1) as f32,
        index_of(&colors, f.hair_color),
        SliderKind::HairColor,
    );
    for (label, filter, prefix, value, kind) in [
        ("Nose Type", FACE, "NoseType", f.nose, SliderKind::NoseType),
        ("Eye Shape", EYES, "EyesType", f.eyes, SliderKind::EyeShape),
        (
            "Mouth Shape",
            MOUTH,
            "LipType",
            f.mouth,
            SliderKind::MouthShape,
        ),
    ] {
        let n = preset_count(face_tri, prefix);
        if n > 0 {
            push(label, filter, n as f32, value.max(0) as f32, kind);
        }
    }
    for (i, &(label, filter)) in MORPH_LABELS.iter().enumerate() {
        out.push(Slider {
            label: label.into(),
            filter,
            min: -1.0,
            max: 1.0,
            position: f.sliders[i],
            interval: 0.1,
            kind: SliderKind::Morph(i),
        });
    }
    for (kind, label) in TINT_LABELS {
        let Some(m) = masks.iter().find(|m| m.kind == kind) else {
            continue;
        };
        let layer = a.layer(m.index);
        let pos = layer
            .and_then(|l| m.presets.iter().position(|p| p.2 as i16 == l.preset))
            .unwrap_or(0);
        out.push(Slider {
            label: label.into(),
            filter: MAKEUP,
            min: 0.0,
            max: m.presets.len().saturating_sub(1) as f32,
            position: pos as f32,
            interval: 1.0,
            kind: SliderKind::TintColor(kind),
        });
        out.push(Slider {
            label: format!("{label} Strength"),
            filter: MAKEUP,
            min: 0.0,
            max: 1.0,
            position: layer.map_or(0.0, |l| l.value),
            interval: 0.05,
            kind: SliderKind::TintValue(kind),
        });
    }
    out
}

/// The appearance a race and sex start with: the race's first preset's
/// face (its parts, sliders, presets and tints), else a bare face.
pub fn race_start(lo: &LoadOrder, race: FormId, female: bool, preset: usize) -> Appearance {
    let presets = race_presets(lo, race, female);
    let mut a = presets
        .get(preset.min(presets.len().saturating_sub(1)))
        .and_then(|&p| Appearance::of_npc(lo, p))
        .unwrap_or(Appearance {
            face: facegen::Face {
                race,
                female,
                parts: Vec::new(),
                sliders: [0.0; 19],
                nose: -1,
                eyes: -1,
                mouth: -1,
                weight: 0.5,
                texture_set: None,
                hair_color: None,
            },
            skin: [1.0; 3],
            tints: Vec::new(),
        });
    a.face.race = race;
    a.face.female = female;
    a
}

/// Apply a slider's new position to an appearance; whether it changed.
pub fn apply_slider(lo: &LoadOrder, a: &mut Appearance, kind: SliderKind, value: f32) -> bool {
    let before = a.clone();
    let i = value.round().max(0.0) as usize;
    let f = &a.face;
    let (race, female) = (f.race, f.female);
    let masks = facegen::race_tint_masks(lo, race, female);
    match kind {
        SliderKind::Sex => {
            let female = value >= 0.5;
            if female != a.face.female {
                let weight = a.face.weight;
                *a = race_start(lo, race, female, 0);
                a.face.weight = weight;
            }
        }
        SliderKind::Preset => {
            let weight = a.face.weight;
            *a = race_start(lo, race, female, i.saturating_sub(1));
            a.face.weight = weight;
        }
        SliderKind::Weight => a.face.weight = (value / 100.0).clamp(0.0, 1.0),
        SliderKind::SkinTone => {
            if let Some(m) = masks
                .iter()
                .find(|m| m.kind == facegen::tint::MASK_SKIN_TONE)
                && let Some(&(clfm, v, idx)) = m.presets.get(i)
            {
                let c = color_of(lo, clfm);
                a.skin = [c[0], c[1], c[2]].map(|x| x as f32 / 255.0);
                a.set_layer(m.index, c, v.clamp(0.0, 1.0), idx as i16);
            }
        }
        SliderKind::Complexion => {
            let list = race_list(lo, race, if female { b"FTSF" } else { b"FTSM" });
            a.face.texture_set = list.get(i).copied();
        }
        SliderKind::Dirt | SliderKind::WarPaint => {
            let kind = if kind == SliderKind::Dirt {
                MASK_DIRT
            } else {
                MASK_PAINT
            };
            let of_kind: Vec<&facegen::TintMask> =
                masks.iter().filter(|m| m.kind == kind).collect();
            // The colour already chosen carries over to the new mask.
            let color = of_kind.iter().find_map(|m| {
                a.layer(m.index)
                    .filter(|l| l.value > 0.0)
                    .map(|l| (l.color, l.preset))
            });
            for m in &of_kind {
                a.tints.retain(|l| l.index != m.index);
            }
            if i > 0
                && let Some(m) = of_kind.get(i - 1)
            {
                let (c, p) = color.unwrap_or_else(|| {
                    m.presets.first().map_or(([0; 4], -1), |&(clfm, _, idx)| {
                        (color_of(lo, clfm), idx as i16)
                    })
                });
                a.set_layer(m.index, c, 1.0, p);
            }
        }
        SliderKind::WarPaintColor => {
            let of_kind: Vec<&facegen::TintMask> =
                masks.iter().filter(|m| m.kind == MASK_PAINT).collect();
            for m in of_kind {
                if a.layer(m.index).is_some_and(|l| l.value > 0.0)
                    && let Some(&(clfm, _, idx)) = m.presets.get(i)
                {
                    let value = a.layer(m.index).map_or(1.0, |l| l.value);
                    a.set_layer(m.index, color_of(lo, clfm), value, idx as i16);
                }
            }
        }
        SliderKind::Hair
        | SliderKind::Beard
        | SliderKind::Scar
        | SliderKind::Eyes
        | SliderKind::Brows => {
            let t = match kind {
                SliderKind::Hair => facegen::PART_HAIR,
                SliderKind::Beard => facegen::PART_FACIAL_HAIR,
                SliderKind::Scar => facegen::PART_SCAR,
                SliderKind::Eyes => facegen::PART_EYES,
                _ => facegen::PART_EYEBROWS,
            };
            if let Some(&p) = choosable_parts(lo, race, female, t).get(i) {
                a.set_part(lo, p);
            }
        }
        SliderKind::HairColor => {
            let list = race_list(lo, race, if female { b"AHCF" } else { b"AHCM" });
            a.face.hair_color = list.get(i).copied();
        }
        SliderKind::NoseType => a.face.nose = i as i32,
        SliderKind::EyeShape => a.face.eyes = i as i32,
        SliderKind::MouthShape => a.face.mouth = i as i32,
        SliderKind::Morph(n) => {
            if let Some(s) = a.face.sliders.get_mut(n) {
                *s = value.clamp(-1.0, 1.0);
            }
        }
        SliderKind::TintColor(k) => {
            if let Some(m) = masks.iter().find(|m| m.kind == k)
                && let Some(&(clfm, v, idx)) = m.presets.get(i)
            {
                let value = a.layer(m.index).map_or(v, |l| l.value);
                a.set_layer(m.index, color_of(lo, clfm), value, idx as i16);
            }
        }
        SliderKind::TintValue(k) => {
            if let Some(m) = masks.iter().find(|m| m.kind == k) {
                let (c, p) = a
                    .layer(m.index)
                    .map(|l| (l.color, l.preset))
                    .unwrap_or_else(|| {
                        m.presets.first().map_or(([0; 4], -1), |&(clfm, _, idx)| {
                            (color_of(lo, clfm), idx as i16)
                        })
                    });
                a.set_layer(m.index, c, value.clamp(0.0, 1.0), p);
            }
        }
    }
    *a != before
}

/// How far the race menu's camera stands from the player: the whole body,
/// and the face (`ZoomPC`). Chosen (`known_ambiguities/chargen.md`).
const RACE_MENU_BODY_DISTANCE: f32 = 170.0;
const RACE_MENU_FACE_DISTANCE: f32 = 45.0;

impl Engine {
    /// Open the race menu (`Game.ShowRaceMenu`, `ShowLimitedRaceMenu`): the
    /// player's appearance becomes editable state, and the camera turns to
    /// face them.
    pub fn show_race_menu(&mut self, limited: bool) {
        if self.player_appearance.is_none() {
            self.player_appearance = self.player_appearance();
        }
        if self.race_menu_camera.is_none() {
            self.race_menu_camera = Some((
                self.third_person,
                self.test_orbit,
                self.third_person_distance,
            ));
        }
        self.set_third_person(true);
        self.test_orbit = std::f32::consts::PI;
        self.third_person_distance = RACE_MENU_BODY_DISTANCE;
        self.menu = Some(crate::items::Menu::RaceSex { limited });
        self.build_player_body();
    }

    /// `Actor.SetRace` on the player: their appearance takes the race (none:
    /// their record's), keeping its face, and their scripts get
    /// `OnRaceSwitchComplete`.
    pub fn set_player_race(&mut self, race: Option<FormId>) {
        let Some(mut a) = self.player_appearance() else {
            return;
        };
        let race = race
            .or_else(|| {
                Appearance::of_npc(&self.lo, crate::player_body::PLAYER_NPC).map(|r| r.face.race)
            })
            .unwrap_or(a.face.race);
        if a.face.race != race {
            a.face.race = race;
            self.set_player_appearance(a);
        }
        self.send_script_event(
            crate::engine::PLAYER_REF,
            "OnRaceSwitchComplete",
            Vec::new(),
        );
    }

    /// The race menu's `ZoomPC`: the camera on the face or the body.
    pub fn race_menu_zoom(&mut self, face: bool) {
        self.third_person_distance = if face {
            RACE_MENU_FACE_DISTANCE
        } else {
            RACE_MENU_BODY_DISTANCE
        };
    }

    /// Close the race menu, the camera back where it was.
    pub fn close_race_menu(&mut self) {
        log::info!("race menu closed");
        if let Some((third, orbit, distance)) = self.race_menu_camera.take() {
            self.set_third_person(third);
            self.test_orbit = orbit;
            self.third_person_distance = distance;
        }
        if matches!(self.menu, Some(crate::items::Menu::RaceSex { .. })) {
            self.menu = None;
        }
    }

    /// The face part's chargen `.tri` for the player's appearance (how many
    /// nose, eye and mouth presets it has).
    pub fn player_face_tri(&self) -> Option<nif::tri::Tri> {
        let a = self.player_appearance.as_ref()?;
        let part = current_part(&self.lo, &a.face, facegen::PART_FACE)?;
        let file = facegen::part_tri(&self.lo, part, facegen::TRI_CHARGEN)?;
        nif::tri::Tri::parse(&self.vfs.read(&file)?).ok()
    }
}
