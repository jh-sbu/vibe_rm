//! Assembling actors (NPC_ + RACE + ARMO/ARMA + FaceGen) into renderable parts.

use esp::{FormId, LoadOrder, LoadedRecord};
use glam::{Mat4, Vec3};

use super::records::{self, mesh_path, texture_path};
use super::template::Sources;

/// Global variables' values as the scripts have set them (`GLOB`s missing
/// here have their record's value).
#[derive(Default, Clone)]
pub struct Globals {
    values: std::collections::HashMap<FormId, f32>,
    /// Entries scripts added to leveled lists, which every roll sees.
    pub(crate) leveled: crate::leveled::Additions,
    /// Default outfits scripts gave NPC records (`ActorBase.SetOutfit`), by
    /// the record an actor takes its inventory from.
    pub(crate) base_outfits: std::collections::HashMap<FormId, FormId>,
}

impl Globals {
    pub fn values(&self) -> &std::collections::HashMap<FormId, f32> {
        &self.values
    }

    pub fn set_values(&mut self, values: std::collections::HashMap<FormId, f32>) {
        self.values = values;
    }
}

impl std::ops::Deref for Globals {
    type Target = std::collections::HashMap<FormId, f32>;
    fn deref(&self) -> &Self::Target {
        &self.values
    }
}

impl std::ops::DerefMut for Globals {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.values
    }
}

/// A leveled list's chance of none: its global's value (`LVLG`) if it names
/// one, else `LVLD`.
fn chance_none(lo: &LoadOrder, rec: &LoadedRecord<'_>, globals: &Globals) -> u64 {
    let global = rec
        .get(b"LVLG")
        .filter(|d| d.len() >= 4)
        .map(|d| rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap()))))
        .filter(|g| !g.is_null());
    if let Some(g) = global {
        let value = globals.get(&g).copied().or_else(|| {
            let d = lo.get(g)?.get(b"FLTV").filter(|d| d.len() >= 4)?.to_vec();
            Some(f32::from_le_bytes(d[0..4].try_into().unwrap()))
        });
        if let Some(v) = value {
            return v.clamp(0.0, 100.0) as u64;
        }
    }
    rec.get(b"LVLD")
        .and_then(|d| d.first().copied())
        .unwrap_or(0) as u64
}

#[derive(Debug, Clone)]
pub struct ActorDesc {
    pub ref_id: FormId,
    pub npc: FormId,
    pub name: String,
    pub transform: Mat4,
    pub skeleton: String,
    /// Models to attach (body parts, armor, head, weapon) and the item each comes
    /// from (null for the body and head).
    pub models: Vec<(String, FormId)>,
    pub female: bool,
    /// The race's behaviour project file (`meshes/actors/canine/dogproject.hkx`).
    pub behavior: Option<String>,
    /// What it carries and has equipped.
    pub inventory: super::inventory::Inventory,
    /// Its race (from its template when that gives its traits).
    pub race: FormId,
    /// The footstep set of what it wears.
    pub footsteps: Option<FormId>,
    /// Where it takes each part of its definition from (templates).
    pub templates: super::template::Sources,
    /// Its skin tone (`QNAM`), the colour skin-tint shapes take in place of
    /// their model's own; none when it gives none.
    pub skin_tone: Option<Vec3>,
    /// Its weight (`NAM7`, 0..100) over 100: how far from its thin models
    /// (`_0`) toward its heavy ones (`_1`) it's drawn.
    pub weight: f32,
    /// The face tint to composite for a head built live (the player's);
    /// NPCs' pre-built heads name their own.
    pub face_tint: Option<FaceTint>,
}

/// A face tint composited at run time (`facegen::tint`): the texture path
/// the head's tint slot names (unique to what it's made of), and its
/// inputs.
#[derive(Debug, Clone, PartialEq)]
pub struct FaceTint {
    pub path: String,
    pub skin: [f32; 3],
    pub layers: Vec<facegen::TintLayer>,
    pub masks: Vec<facegen::TintMask>,
}

impl FaceTint {
    /// The tint a face's record gives: its skin tone and tint layers over
    /// its race's masks. None for a race without masks.
    pub fn of(lo: &LoadOrder, face_npc: FormId, face: &facegen::Face) -> Option<FaceTint> {
        use std::hash::{Hash, Hasher};
        let masks = facegen::race_tint_masks(lo, face.race, face.female);
        if masks.is_empty() {
            return None;
        }
        let skin = facegen::npc_skin_tone(lo, face_npc).unwrap_or([1.0; 3]);
        let layers = facegen::npc_tint_layers(lo, face_npc);
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (face.race.0, face.female).hash(&mut h);
        for v in skin {
            v.to_bits().hash(&mut h);
        }
        for l in &layers {
            (l.index, l.color, l.value.to_bits()).hash(&mut h);
        }
        Some(FaceTint {
            path: format!("textures/vrm/facetint/{:016x}.dds", h.finish()),
            skin,
            layers,
            masks,
        })
    }
}

fn fid_at(rec: &LoadedRecord<'_>, d: &[u8]) -> FormId {
    if d.len() < 4 {
        return FormId::NULL;
    }
    rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap())))
}

/// A well-mixed hash of a seed and a list, so each reference picks its own entry
/// and picks it again next time.
fn mix(seed: u64, list: FormId) -> u64 {
    let mut z = seed ^ (list.0 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Leveled list flags (`LVLF`).
const LVL_ALL_LEVELS: u8 = 0x01;
const LVL_EACH_ITEM: u8 = 0x02;
const LVL_USE_ALL: u8 = 0x04;

/// A leveled list's entries (`LVLO`: level, form, count) eligible at the player's
/// level: with "calculate from all levels" every entry up to it, otherwise those of
/// the highest level reached; the lowest-level entries if none is.
fn eligible(rec: &LoadedRecord<'_>, id: FormId, globals: &Globals) -> Vec<(FormId, i32)> {
    eligible_at(rec, id, crate::skills::player_level(), globals)
}

/// `eligible` at `level` rather than the player's.
fn eligible_at(
    rec: &LoadedRecord<'_>,
    id: FormId,
    level: u16,
    globals: &Globals,
) -> Vec<(FormId, i32)> {
    let mut entries: Vec<(u16, FormId, i32)> = rec
        .subrecords()
        .filter(|s| s.tag.0 == *b"LVLO" && s.data.len() >= 8)
        .map(|s| {
            (
                s.u16(0),
                rec.fid(s.form_id(4)),
                if s.data.len() >= 10 {
                    s.u16(8).max(1) as i32
                } else {
                    1
                },
            )
        })
        .collect();
    entries.extend(
        globals
            .leveled
            .of(id)
            .iter()
            .map(|&(l, f, n)| (l, f, n.max(1) as i32)),
    );
    let pc = level;
    let flags = rec
        .get(b"LVLF")
        .and_then(|d| d.first().copied())
        .unwrap_or(0);
    let reached = entries.iter().map(|e| e.0).filter(|&l| l <= pc).max();
    let pick = |want: &dyn Fn(u16) -> bool| {
        entries
            .iter()
            .filter(|e| want(e.0))
            .map(|e| (e.1, e.2))
            .collect::<Vec<_>>()
    };
    match reached {
        Some(_) if flags & LVL_ALL_LEVELS != 0 => pick(&|l| l <= pc),
        Some(top) => pick(&|l| l == top),
        None => {
            let low = entries.iter().map(|e| e.0).min().unwrap_or(0);
            pick(&|l| l == low)
        }
    }
}

/// Follow leveled lists (`list_tag`: LVLN, LVLI) down to a record, picking among
/// each list's eligible entries by `seed` (the reference); `None` when a list's
/// chance of none comes up.
pub(crate) fn resolve_leveled(
    lo: &LoadOrder,
    mut id: FormId,
    list_tag: &[u8; 4],
    seed: u64,
    globals: &Globals,
) -> Option<FormId> {
    for _ in 0..8 {
        let rec = lo.get(id)?;
        if rec.tag().0 != *list_tag {
            return Some(id);
        }
        let roll = mix(seed, id);
        if (roll % 100) < chance_none(lo, &rec, globals) {
            return None;
        }
        let entries = eligible(&rec, id, globals);
        if entries.is_empty() {
            return None;
        }
        id = entries[((roll >> 8) % entries.len() as u64) as usize].0;
    }
    None
}

/// Items from a leveled item list (or a plain item), `count` times: "use all" gives
/// every entry, otherwise one entry is picked by `seed`, its own count multiplying
/// `count`; "each item in count" picks again for each one. Chance of none can leave
/// nothing.
pub(crate) fn resolve_items(
    lo: &LoadOrder,
    id: FormId,
    count: i32,
    seed: u64,
    globals: &Globals,
    depth: u32,
) -> Vec<(FormId, i32)> {
    resolve_items_at(
        lo,
        id,
        count,
        seed,
        globals,
        crate::skills::player_level(),
        depth,
    )
}

/// The same roll at an explicit recipient level; nested lists keep that level.
pub(crate) fn resolve_items_at(
    lo: &LoadOrder,
    id: FormId,
    count: i32,
    seed: u64,
    globals: &Globals,
    level: u16,
    depth: u32,
) -> Vec<(FormId, i32)> {
    let Some(rec) = lo.get(id) else {
        return Vec::new();
    };
    if rec.tag().0 != *b"LVLI" {
        return vec![(id, count)];
    }
    if depth > 8 || count <= 0 {
        return Vec::new();
    }
    let flags = rec
        .get(b"LVLF")
        .and_then(|d| d.first().copied())
        .unwrap_or(0);
    if flags & LVL_EACH_ITEM != 0 && count > 1 {
        return (0..count.min(64))
            .flat_map(|i| {
                resolve_items_once(
                    lo,
                    &rec,
                    id,
                    1,
                    mix(seed, FormId(i as u32)),
                    globals,
                    level,
                    depth,
                )
            })
            .collect();
    }
    resolve_items_once(lo, &rec, id, count, seed, globals, level, depth)
}

fn resolve_items_once(
    lo: &LoadOrder,
    rec: &LoadedRecord<'_>,
    id: FormId,
    count: i32,
    seed: u64,
    globals: &Globals,
    level: u16,
    depth: u32,
) -> Vec<(FormId, i32)> {
    let flags = rec
        .get(b"LVLF")
        .and_then(|d| d.first().copied())
        .unwrap_or(0);
    let roll = mix(seed, id);
    if (roll % 100) < chance_none(lo, rec, globals) {
        return Vec::new();
    }
    let entries = eligible_at(rec, id, level, globals);
    if flags & LVL_USE_ALL != 0 {
        return entries
            .into_iter()
            .flat_map(|(e, n)| {
                resolve_items_at(
                    lo,
                    e,
                    n.saturating_mul(count),
                    seed,
                    globals,
                    level,
                    depth + 1,
                )
            })
            .collect();
    }
    if entries.is_empty() {
        return Vec::new();
    }
    let (e, n) = entries[((roll >> 8) % entries.len() as u64) as usize];
    resolve_items_at(
        lo,
        e,
        n.saturating_mul(count),
        seed,
        globals,
        level,
        depth + 1,
    )
}

/// The spells a leveled spell list (`LVSP`, or a plain spell) gives an actor
/// of `level`: every eligible entry with "use all", otherwise one picked by
/// `seed`; its chance of none can leave nothing. Nested lists are followed.
pub(crate) fn resolve_spells(
    lo: &LoadOrder,
    id: FormId,
    level: u16,
    seed: u64,
    globals: &Globals,
    depth: u32,
) -> Vec<FormId> {
    let Some(rec) = lo.get(id).filter(|r| r.tag().0 == *b"LVSP") else {
        return if id.is_null() { Vec::new() } else { vec![id] };
    };
    if depth > 8 {
        return Vec::new();
    }
    let roll = mix(seed, id);
    if (roll % 100) < chance_none(lo, &rec, globals) {
        return Vec::new();
    }
    let entries = eligible_at(&rec, id, level, globals);
    let flags = rec
        .get(b"LVLF")
        .and_then(|d| d.first().copied())
        .unwrap_or(0);
    if flags & LVL_USE_ALL != 0 {
        return entries
            .into_iter()
            .flat_map(|(e, _)| resolve_spells(lo, e, level, seed, globals, depth + 1))
            .collect();
    }
    if entries.is_empty() {
        return Vec::new();
    }
    let e = entries[((roll >> 8) % entries.len() as u64) as usize].0;
    resolve_spells(lo, e, level, seed, globals, depth + 1)
}

struct NpcTraits {
    /// Display name (`FULL`).
    name: String,
    npc_for_face: FormId,
    /// The NPC whose items (`CNTO`) and skills it has.
    npc_for_inventory: FormId,
    npc_for_stats: FormId,
    /// Combat style (`ZNAM`), weighing the weapons it would wield.
    combat_style: FormId,
    race: FormId,
    female: bool,
    skin: FormId,
    outfit: FormId,
    height: f32,
    skin_tone: Option<Vec3>,
    weight: f32,
}

/// The effective traits of an NPC, from the records its templates give each part.
fn npc_traits(lo: &LoadOrder, src: &Sources) -> Option<NpcTraits> {
    use super::template::{AI_DATA, BASE_DATA, INVENTORY, STATS, TRAITS};
    let traits = lo.get(src.of(TRAITS))?;
    let acbs = traits.get(b"ACBS").unwrap_or(&[]);
    let flags = if acbs.len() >= 4 {
        u32::from_le_bytes(acbs[0..4].try_into().unwrap())
    } else {
        0
    };
    let name = src
        .record(lo, BASE_DATA, b"FULL")
        .and_then(|r| r.get(b"FULL").map(|d| lo.lstring(&r, d)))
        .unwrap_or_default();
    Some(NpcTraits {
        name,
        npc_for_face: src.of(TRAITS),
        npc_for_inventory: src.of(INVENTORY),
        npc_for_stats: src.of(STATS),
        // Combat style comes with the AI data.
        combat_style: src.form(lo, AI_DATA, b"ZNAM").unwrap_or_default(),
        race: traits
            .get(b"RNAM")
            .map(|d| fid_at(&traits, d))
            .unwrap_or_default(),
        female: flags & 1 != 0,
        skin: traits
            .get(b"WNAM")
            .map(|d| fid_at(&traits, d))
            .unwrap_or_default(),
        outfit: lo
            .get(src.of(INVENTORY))
            .and_then(|r| r.get(b"DOFT").map(|d| fid_at(&r, d)))
            .unwrap_or_default(),
        height: traits
            .get(b"NAM6")
            .map(|d| f32::from_le_bytes(d[0..4].try_into().unwrap()))
            .unwrap_or(1.0),
        // All zero in templated NPCs (their template gives it) and Dremora:
        // taken as none.
        skin_tone: traits
            .get(b"QNAM")
            .filter(|d| d.len() >= 12)
            .map(|d| {
                let c = |i: usize| f32::from_le_bytes(d[i..i + 4].try_into().unwrap());
                Vec3::new(c(0), c(4), c(8))
            })
            .filter(|c| *c != Vec3::ZERO),
        // Its FaceGen head is made for this weight.
        weight: traits
            .get(b"NAM7")
            .filter(|d| d.len() >= 4)
            .map_or(50.0, |d| f32::from_le_bytes(d[0..4].try_into().unwrap()))
            / 100.0,
    })
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
                if gender_marker == Some(female)
                    || (skeleton.is_none() && gender_marker == Some(false))
                {
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

/// The race's behaviour project for the sex (the MNAM / FNAM model after NAM3).
fn race_behavior(lo: &LoadOrder, race: FormId, female: bool) -> Option<String> {
    let rec = lo.get(race)?;
    let mut in_section = false;
    let mut gender_marker: Option<bool> = None;
    let mut found = None;
    for sr in rec.subrecords() {
        match &sr.tag.0 {
            b"NAM3" => in_section = true,
            b"MNAM" => gender_marker = Some(false),
            b"FNAM" => gender_marker = Some(true),
            b"MODL" if in_section && (gender_marker == Some(female) || found.is_none()) => {
                let m = sr.zstring();
                if !m.is_empty() {
                    found = Some(mesh_path(&m));
                }
                if gender_marker == Some(female) {
                    break;
                }
            }
            b"NAM4" | b"NAM5" if in_section => break,
            _ => {}
        }
    }
    found
}

/// The addons (ARMA) of an armor that a race wears.
fn race_addons<'a>(lo: &'a LoadOrder, armo: FormId, race: FormId) -> Vec<LoadedRecord<'a>> {
    let Some(rec) = lo.get(armo) else {
        return Vec::new();
    };
    if rec.tag().0 != *b"ARMO" {
        return Vec::new();
    }
    let mut armor_race = FormId::NULL;
    if let Some(d) = rec.get(b"RNAM") {
        armor_race = fid_at(&rec, d);
    }
    let addons: Vec<FormId> = rec
        .subrecords()
        .filter(|s| s.tag.0 == *b"MODL" && s.data.len() == 4)
        .map(|s| rec.fid(s.form_id(0)))
        .collect();
    addons
        .into_iter()
        .filter_map(|aa| lo.get(aa))
        .filter(|a| {
            let primary = a.get(b"RNAM").map(|d| fid_at(a, d)).unwrap_or_default();
            let extra: Vec<FormId> = a
                .subrecords()
                .filter(|s| s.tag.0 == *b"MODL" && s.data.len() == 4)
                .map(|s| a.fid(s.form_id(0)))
                .collect();
            primary == race
                || extra.contains(&race)
                || (primary == armor_race && extra.is_empty() && race == armor_race)
        })
        .collect()
}

/// The footstep set (FSTS) of what an actor wears: the addon on its feet (biped
/// slot 37) that has one, else the first that does (creatures' skins), the worn
/// armor before the skin.
pub fn footstep_set(lo: &LoadOrder, worn: &[FormId], skin: FormId, race: FormId) -> Option<FormId> {
    const FEET: u32 = 1 << 7;
    let mut first = None;
    for &armo in worn.iter().chain([&skin]) {
        for a in race_addons(lo, armo, race) {
            let Some(d) = a.get(b"SNDD") else { continue };
            let set = fid_at(&a, d);
            let slots = super::inventory::armor_slots(&a);
            if slots & FEET != 0 {
                return Some(set);
            }
            first.get_or_insert(set);
        }
    }
    first
}

/// Model pair used when dressing an actor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Viewpoint {
    Third,
    First,
}

/// Armor addon models for an armor on a given race: (model path, biped slot mask).
fn armor_models(
    lo: &LoadOrder,
    armo: FormId,
    race: FormId,
    female: bool,
    view: Viewpoint,
) -> Vec<(String, u32)> {
    let mut out = Vec::new();
    for a in race_addons(lo, armo, race) {
        let slots = super::inventory::armor_slots(&a);
        let (male, female_tag) = match view {
            Viewpoint::Third => (b"MOD2", b"MOD3"),
            Viewpoint::First => (b"MOD4", b"MOD5"),
        };
        let (preferred, fallback) = if female {
            (female_tag, male)
        } else {
            (male, female_tag)
        };
        // Empty model fields are absent too, including when falling across sex.
        let model = a
            .get(preferred)
            .filter(|d| !esp::decode_zstring(d).is_empty())
            .or_else(|| {
                a.get(fallback)
                    .filter(|d| !esp::decode_zstring(d).is_empty())
            });
        // Rigid shields commonly have a single model for both viewpoints
        // (IronShieldAA has MOD2 only). Unlike head/body meshes, they cannot
        // surround the eye; UESP's all-viewpoints fallback applies here.
        let model = model.or_else(|| {
            (view == Viewpoint::First && slots & (1 << 9) != 0)
                .then(|| {
                    let (preferred, fallback) = if female {
                        (b"MOD3", b"MOD2")
                    } else {
                        (b"MOD2", b"MOD3")
                    };
                    a.get(preferred)
                        .filter(|d| !esp::decode_zstring(d).is_empty())
                        .or_else(|| {
                            a.get(fallback)
                                .filter(|d| !esp::decode_zstring(d).is_empty())
                        })
                })
                .flatten()
        });
        if let Some(m) = model {
            let s = esp::decode_zstring(m);
            if !s.is_empty() {
                out.push((mesh_path(&s), slots));
            }
        }
    }
    out
}

/// WNAM names the first-person STAT; an absent/invalid model uses the weapon's MODL.
pub fn weapon_first_person_model(lo: &LoadOrder, weapon: FormId) -> Option<String> {
    let rec = lo.get(weapon).filter(|r| r.tag().0 == *b"WEAP")?;
    rec.get(b"WNAM")
        .map(|d| fid_at(&rec, d))
        .and_then(|f| lo.get(f))
        .filter(|r| r.tag().0 == *b"STAT")
        .and_then(|r| super::records::model_path(&r))
        .or_else(|| super::inventory::weapon_model(lo, weapon))
}

/// Explicit first-person skin/armor pairs, weapons, and carried lights. Head
/// parts and quivers do not belong to the rig surrounding the eye. See
/// known_ambiguities/first-person.md for addons without MOD4/MOD5.
pub fn dress_first_person(lo: &LoadOrder, d: &ActorDesc) -> Vec<(String, FormId)> {
    let Some(traits) = npc_traits(lo, &d.templates) else {
        return Vec::new();
    };
    let skin = if traits.skin.is_null() {
        race_info(lo, d.race, d.female)
            .map(|(_, s, _)| s)
            .unwrap_or_default()
    } else {
        traits.skin
    };
    dress(
        lo,
        &d.inventory,
        skin,
        d.race,
        d.female,
        Vec::new(),
        Viewpoint::First,
    )
}

/// Equipment worn by default: armors from the outfit, resolving leveled item lists.
pub(crate) fn outfit_armors(
    lo: &LoadOrder,
    outfit: FormId,
    seed: u64,
    globals: &Globals,
) -> Vec<FormId> {
    let Some(rec) = lo.get(outfit) else {
        return Vec::new();
    };
    let Some(items) = rec.get(b"INAM") else {
        return Vec::new();
    };
    items
        .chunks_exact(4)
        .map(|c| rec.fid(FormId(u32::from_le_bytes(c.try_into().unwrap()))))
        .flat_map(|id| resolve_items(lo, id, 1, seed, globals, 0))
        .map(|(id, _)| id)
        .filter(|id| lo.tag_of(*id).map(|t| t.0) == Some(*b"ARMO"))
        .collect()
}

pub fn facegen_path(lo: &LoadOrder, npc: FormId) -> Option<String> {
    let (plugin, local) = lo.origin(npc)?;
    Some(format!(
        "meshes/actors/character/facegendata/facegeom/{}/{:08x}.nif",
        plugin.to_ascii_lowercase(),
        local
    ))
}

pub fn facetint_path(lo: &LoadOrder, npc: FormId) -> Option<String> {
    let (plugin, local) = lo.origin(npc)?;
    Some(texture_path(&format!(
        "actors/character/facegendata/facetint/{}/{:08x}.dds",
        plugin.to_ascii_lowercase(),
        local
    )))
}

/// The models an actor shows: what it wears (armors conflicting by their own
/// slots, their addons' slots hiding the skin beneath), the skin's parts left
/// uncovered, the `head` models unless a helmet hides head and hair, and its
/// weapon (a rigid model hung from the bone it names, sheathed).
fn dress(
    lo: &LoadOrder,
    inventory: &super::inventory::Inventory,
    skin: FormId,
    race: FormId,
    female: bool,
    head: Vec<String>,
    view: Viewpoint,
) -> Vec<(String, FormId)> {
    let mut models: Vec<(String, FormId)> = Vec::new();
    // Worn armors conflict by their own slots; their addons' slots (which can
    // overlap, as boots and a robe both on the calves) hide the skin beneath.
    let mut worn = 0u32;
    let mut covered = 0u32;
    for &armo in inventory.equipped() {
        let armo_slots = lo
            .get(armo)
            .map_or(0, |r| super::inventory::armor_slots(&r));
        if armo_slots & worn != 0 {
            continue;
        }
        worn |= armo_slots;
        for (m, slots) in armor_models(lo, armo, race, female, view) {
            covered |= slots;
            models.push((m, armo));
        }
    }
    for (m, slots) in armor_models(lo, skin, race, female, view) {
        if slots & covered == 0 {
            covered |= slots;
            models.push((m, FormId::NULL));
        }
    }
    if view == Viewpoint::Third && (covered & SLOT_HEAD == 0 || covered & SLOT_HAIR == 0) {
        models.extend(head.into_iter().map(|m| (m, FormId::NULL)));
    }
    let mut weapons: Vec<FormId> = [super::inventory::RIGHT, super::inventory::LEFT]
        .into_iter()
        .filter_map(|h| inventory.weapon_in(lo, h))
        .collect();
    weapons.dedup();
    for w in weapons {
        let model = match view {
            Viewpoint::Third => super::inventory::weapon_model(lo, w),
            Viewpoint::First => weapon_first_person_model(lo, w),
        };
        if let Some(m) = model {
            models.push((m, w));
        }
    }
    // Ammunition worn shows as its quiver, with the bow or without (UESP
    // Skyrim:Archery).
    if view == Viewpoint::Third
        && let Some((ammo, m)) = inventory.equipped().iter().find_map(|&f| {
            let rec = lo.get(f).filter(|r| r.tag().0 == *b"AMMO")?;
            (inventory.count(f) > 0).then_some((f, super::records::model_path(&rec)?))
        })
    {
        models.push((format!("{m}#quiver"), ammo));
    }
    models
}

/// The head part models of a face built at run time rather than from a
/// pre-built FaceGen head (the player): `facegen::head_parts`, each with
/// the morphs its race, sliders, presets and weight give it
/// (`#facegen=`, `research/facegen.md`), the face part naming `tint` for
/// its tint texture (`!tint=`). The parts' texture sets aren't applied.
pub fn head_part_models(lo: &LoadOrder, face: &facegen::Face, tint: Option<&str>) -> Vec<String> {
    facegen::head_parts(lo, face)
        .into_iter()
        .filter_map(|id| {
            let rec = lo.get(id)?;
            let m = esp::decode_zstring(rec.get(b"MODL")?);
            if m.is_empty() {
                return None;
            }
            let m = mesh_path(&m);
            let uses = facegen::part_morphs(lo, id, face);
            let tint = tint.filter(|_| facegen::part_type(lo, id) == Some(facegen::PART_FACE));
            Some(if uses.is_empty() && tint.is_none() {
                m
            } else {
                let mut v = format!("{m}#facegen={}", facegen::encode_morphs(&uses));
                if let Some(t) = tint {
                    v.push_str("!tint=");
                    v.push_str(t);
                }
                v
            })
        })
        .collect()
}

/// The face an actor's traits give it: its face NPC's head data, with the
/// race, sex and weight its templates settle.
fn face_of(lo: &LoadOrder, traits: &NpcTraits) -> facegen::Face {
    let mut face = facegen::Face::of_npc(lo, traits.npc_for_face).unwrap_or(facegen::Face {
        race: traits.race,
        female: traits.female,
        parts: Vec::new(),
        sliders: [0.0; 19],
        nose: -1,
        eyes: -1,
        mouth: -1,
        weight: traits.weight,
    });
    face.race = traits.race;
    face.female = traits.female;
    face.weight = traits.weight;
    face
}

/// Describe the player's body: their NPC record's race and sex, its skin, what
/// they have equipped and a head of the race's default parts and their own
/// (no pre-built FaceGen head exists for them). Placed each frame, so the
/// transform holds only their scale.
pub fn describe_player(
    lo: &LoadOrder,
    player: FormId,
    npc: FormId,
    inventory: &super::inventory::Inventory,
) -> Option<ActorDesc> {
    let templates = Sources::of_npc(lo, npc, 0, &Globals::default());
    let traits = npc_traits(lo, &templates)?;
    let (skeleton, race_skin, race_height) = race_info(lo, traits.race, traits.female)?;
    let skin = if traits.skin.is_null() {
        race_skin
    } else {
        traits.skin
    };
    let face = face_of(lo, &traits);
    let face_tint = FaceTint::of(lo, traits.npc_for_face, &face);
    let head = head_part_models(lo, &face, face_tint.as_ref().map(|t| t.path.as_str()));
    let models = dress(
        lo,
        inventory,
        skin,
        traits.race,
        traits.female,
        head,
        Viewpoint::Third,
    );
    Some(ActorDesc {
        ref_id: player,
        npc,
        name: traits.name.clone(),
        transform: Mat4::from_scale(glam::Vec3::splat(traits.height * race_height)),
        skeleton,
        models,
        female: traits.female,
        behavior: race_behavior(lo, traits.race, traits.female),
        inventory: inventory.clone(),
        race: traits.race,
        footsteps: footstep_set(lo, inventory.equipped(), skin, traits.race),
        templates,
        skin_tone: traits.skin_tone,
        weight: traits.weight,
        face_tint,
    })
}

/// What an NPC starts with: the items it carries, then its outfit (worn),
/// then a weapon and shield to wield.
fn npc_inventory(
    lo: &LoadOrder,
    traits: &NpcTraits,
    seed: u64,
    globals: &Globals,
) -> super::inventory::Inventory {
    let mut inventory = super::inventory::Inventory::default();
    if let Some(rec) = lo.get(traits.npc_for_inventory) {
        for (f, n) in super::inventory::listed_items(lo, &rec, seed, globals) {
            inventory.add(f, n);
        }
    }
    let outfit = globals
        .base_outfits
        .get(&traits.npc_for_inventory)
        .copied()
        .unwrap_or(traits.outfit);
    for armo in outfit_armors(lo, outfit, seed, globals) {
        inventory.add(armo, 1);
        inventory.equip(lo, armo);
    }
    super::inventory::equip_weapons(
        lo,
        &mut inventory,
        traits.npc_for_stats,
        traits.combat_style,
    );
    inventory
}

/// The traits of the NPC an actor reference is (its leveled list's pick).
fn reference_traits(
    lo: &LoadOrder,
    r: &records::Reference,
    globals: &Globals,
) -> Option<NpcTraits> {
    let seed = r.id.0 as u64;
    let npc = resolve_leveled(lo, r.base, b"LVLN", seed, globals)?;
    if lo.tag_of(npc)?.0 != *b"NPC_" {
        return None;
    }
    npc_traits(lo, &Sources::of_npc(lo, npc, seed, globals))
}

/// What an actor reference starts with (see `describe_reference`), before it
/// was ever loaded.
pub fn starting_inventory(
    lo: &LoadOrder,
    r: &records::Reference,
    globals: &Globals,
) -> Option<super::inventory::Inventory> {
    let traits = reference_traits(lo, r, globals)?;
    Some(npc_inventory(lo, &traits, r.id.0 as u64, globals))
}

/// The weapon an actor reference would wield of those in `inv`, as it picks
/// one when it loads (`equip_weapons`).
pub fn best_weapon(
    lo: &LoadOrder,
    r: &records::Reference,
    globals: &Globals,
    inv: &super::inventory::Inventory,
) -> Option<FormId> {
    let traits = reference_traits(lo, r, globals)?;
    let mut inv = inv.clone();
    inv.take_off_where(|f| lo.tag_of(f).map(|t| t.0) == Some(*b"WEAP"));
    super::inventory::equip_weapons(lo, &mut inv, traits.npc_for_stats, traits.combat_style);
    inv.weapon(lo)
}

/// Whether an actor reference's race and sex have a model of an armor to
/// wear (children have none of the adults' armor).
pub fn can_wear(lo: &LoadOrder, r: &records::Reference, globals: &Globals, armo: FormId) -> bool {
    reference_traits(lo, r, globals)
        .is_some_and(|t| !armor_models(lo, armo, t.race, t.female, Viewpoint::Third).is_empty())
}

/// Biped slot for the head (30) and hair (31) etc.
const SLOT_HEAD: u32 = 1 << 0;
const SLOT_HAIR: u32 = 1 << 1;

/// Describe an actor reference (placed or created), wearing what `stored`
/// says it has equipped when it has an inventory already (else its record's
/// items, outfit and the weapon it would pick).
pub fn describe_reference(
    lo: &LoadOrder,
    r: &records::Reference,
    globals: &Globals,
    stored: Option<&super::inventory::Inventory>,
) -> Option<ActorDesc> {
    // Disabled references (initially, or through their enable parent) are left
    // out by the caller.
    if r.deleted() {
        return None;
    }
    // Each reference picks its own entry from leveled lists, the same every time.
    let seed = r.id.0 as u64;
    let npc = resolve_leveled(lo, r.base, b"LVLN", seed, globals)?;
    let base = lo.get(npc)?;
    if base.tag().0 != *b"NPC_" {
        return None;
    }
    let templates = Sources::of_npc(lo, npc, seed, globals);
    let traits = npc_traits(lo, &templates)?;
    let (skeleton, race_skin, race_height) = race_info(lo, traits.race, traits.female)?;
    let skin = if traits.skin.is_null() {
        race_skin
    } else {
        traits.skin
    };
    let inventory = match stored {
        Some(inv) => inv.clone(),
        None => npc_inventory(lo, &traits, seed, globals),
    };

    let face = facegen_path(lo, traits.npc_for_face).into_iter().collect();
    let models = dress(
        lo,
        &inventory,
        skin,
        traits.race,
        traits.female,
        face,
        Viewpoint::Third,
    );
    let name = traits.name.clone();
    let scale = r.scale * traits.height * race_height;
    let transform = Mat4::from_scale_rotation_translation(
        glam::Vec3::splat(scale),
        r.rotation_quat(),
        r.position,
    );
    let behavior = race_behavior(lo, traits.race, traits.female);
    let footsteps = footstep_set(lo, inventory.equipped(), skin, traits.race);
    Some(ActorDesc {
        ref_id: r.id,
        npc,
        name,
        transform,
        skeleton,
        models,
        female: traits.female,
        behavior,
        inventory,
        race: traits.race,
        footsteps,
        templates,
        skin_tone: traits.skin_tone,
        weight: traits.weight,
        face_tint: None,
    })
}

#[cfg(test)]
#[path = "first_person_tests.rs"]
mod first_person_tests;
