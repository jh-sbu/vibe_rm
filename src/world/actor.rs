//! Assembling actors (NPC_ + RACE + ARMO/ARMA + FaceGen) into renderable parts.

use esp::{FormId, LoadOrder, LoadedRecord};
use glam::Mat4;

use super::records::{self, mesh_path, texture_path};
use super::template::Sources;

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
fn eligible(rec: &LoadedRecord<'_>) -> Vec<(FormId, i32)> {
    let entries: Vec<(u16, FormId, i32)> = rec
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
    let pc = crate::skills::player_level();
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
/// chance of none (`LVLD`) comes up.
pub(crate) fn resolve_leveled(
    lo: &LoadOrder,
    mut id: FormId,
    list_tag: &[u8; 4],
    seed: u64,
) -> Option<FormId> {
    for _ in 0..8 {
        let rec = lo.get(id)?;
        if rec.tag().0 != *list_tag {
            return Some(id);
        }
        let roll = mix(seed, id);
        let none = rec
            .get(b"LVLD")
            .and_then(|d| d.first().copied())
            .unwrap_or(0);
        if (roll % 100) < none as u64 {
            return None;
        }
        let entries = eligible(&rec);
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
            .flat_map(|i| resolve_items_once(lo, &rec, id, 1, mix(seed, FormId(i as u32)), depth))
            .collect();
    }
    resolve_items_once(lo, &rec, id, count, seed, depth)
}

fn resolve_items_once(
    lo: &LoadOrder,
    rec: &LoadedRecord<'_>,
    id: FormId,
    count: i32,
    seed: u64,
    depth: u32,
) -> Vec<(FormId, i32)> {
    let flags = rec
        .get(b"LVLF")
        .and_then(|d| d.first().copied())
        .unwrap_or(0);
    let roll = mix(seed, id);
    let none = rec
        .get(b"LVLD")
        .and_then(|d| d.first().copied())
        .unwrap_or(0);
    if (roll % 100) < none as u64 {
        return Vec::new();
    }
    let entries = eligible(rec);
    if flags & LVL_USE_ALL != 0 {
        return entries
            .into_iter()
            .flat_map(|(e, n)| resolve_items(lo, e, n * count, seed, depth + 1))
            .collect();
    }
    if entries.is_empty() {
        return Vec::new();
    }
    let (e, n) = entries[((roll >> 8) % entries.len() as u64) as usize];
    resolve_items(lo, e, n * count, seed, depth + 1)
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

/// Armor addon models for an armor on a given race: (model path, biped slot mask).
fn armor_models(lo: &LoadOrder, armo: FormId, race: FormId, female: bool) -> Vec<(String, u32)> {
    let mut out = Vec::new();
    for a in race_addons(lo, armo, race) {
        let slots = super::inventory::armor_slots(&a);
        let model = if female {
            a.get(b"MOD3").or_else(|| a.get(b"MOD2"))
        } else {
            a.get(b"MOD2").or_else(|| a.get(b"MOD3"))
        };
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
fn outfit_armors(lo: &LoadOrder, outfit: FormId, seed: u64) -> Vec<FormId> {
    let Some(rec) = lo.get(outfit) else {
        return Vec::new();
    };
    let Some(items) = rec.get(b"INAM") else {
        return Vec::new();
    };
    items
        .chunks_exact(4)
        .map(|c| rec.fid(FormId(u32::from_le_bytes(c.try_into().unwrap()))))
        .flat_map(|id| resolve_items(lo, id, 1, seed, 0))
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

/// Biped slot for the head (30) and hair (31) etc.
const SLOT_HEAD: u32 = 1 << 0;
const SLOT_HAIR: u32 = 1 << 1;

/// Describe an actor reference (placed or created).
pub fn describe_reference(lo: &LoadOrder, r: &records::Reference) -> Option<ActorDesc> {
    // Disabled references (initially, or through their enable parent) are left
    // out by the caller.
    if r.deleted() {
        return None;
    }
    // Each reference picks its own entry from leveled lists, the same every time.
    let seed = r.id.0 as u64;
    let npc = resolve_leveled(lo, r.base, b"LVLN", seed)?;
    let base = lo.get(npc)?;
    if base.tag().0 != *b"NPC_" {
        return None;
    }
    let templates = Sources::of_npc(lo, npc, seed);
    let traits = npc_traits(lo, &templates)?;
    let (skeleton, race_skin, race_height) = race_info(lo, traits.race, traits.female)?;
    let skin = if traits.skin.is_null() {
        race_skin
    } else {
        traits.skin
    };

    // Carried items, then the outfit (worn), then a weapon and shield to wield.
    let mut inventory = super::inventory::Inventory::default();
    if let Some(rec) = lo.get(traits.npc_for_inventory) {
        for (f, n) in super::inventory::listed_items(lo, &rec, seed) {
            inventory.add(f, n);
        }
    }
    let outfit = outfit_armors(lo, traits.outfit, seed);
    for &armo in &outfit {
        inventory.add(armo, 1);
        inventory.equipped.push(armo);
    }
    super::inventory::equip_weapons(
        lo,
        &mut inventory,
        traits.npc_for_stats,
        traits.combat_style,
    );

    let mut models: Vec<(String, FormId)> = Vec::new();
    // Worn armors conflict by their own slots; their addons' slots (which can
    // overlap, as boots and a robe both on the calves) hide the skin beneath.
    let mut worn = 0u32;
    let mut covered = 0u32;
    for &armo in &inventory.equipped {
        let armo_slots = lo
            .get(armo)
            .map_or(0, |r| super::inventory::armor_slots(&r));
        if armo_slots & worn != 0 {
            continue;
        }
        worn |= armo_slots;
        for (m, slots) in armor_models(lo, armo, traits.race, traits.female) {
            covered |= slots;
            models.push((m, armo));
        }
    }
    for (m, slots) in armor_models(lo, skin, traits.race, traits.female) {
        if slots & covered == 0 {
            covered |= slots;
            models.push((m, FormId::NULL));
        }
    }
    // The pre-built FaceGen head (head, hair, eyes, brows) unless a helmet hides it.
    if covered & SLOT_HEAD == 0 || covered & SLOT_HAIR == 0 {
        if let Some(f) = facegen_path(lo, traits.npc_for_face) {
            models.push((f, FormId::NULL));
        }
    }
    // Weapons are rigid models hung from the bone their model names (sheathed).
    if let Some((w, m)) = inventory
        .weapon(lo)
        .and_then(|w| Some((w, super::inventory::weapon_model(lo, w)?)))
    {
        models.push((m, w));
    }
    let name = traits.name.clone();
    let scale = r.scale * traits.height * race_height;
    let transform = Mat4::from_scale_rotation_translation(
        glam::Vec3::splat(scale),
        r.rotation_quat(),
        r.position,
    );
    let behavior = race_behavior(lo, traits.race, traits.female);
    let footsteps = footstep_set(lo, &inventory.equipped, skin, traits.race);
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
    })
}
