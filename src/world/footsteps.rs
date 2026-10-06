//! Footsteps: an actor's footstep set (FSTS, from the armor addon on its feet) names
//! a footstep (FSTP) per animation event tag and gait; each footstep's impact data set
//! (IPDS) pairs ground materials (MATT) with impacts (IPCT), whose sound plays.

use std::collections::HashMap;

use esp::{FormId, LoadOrder};

/// How the actor is moving: the footstep set's groups, in `XCNT` order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gait {
    Walk = 0,
    Run = 1,
    Sprint = 2,
    Sneak = 3,
}

/// A footstep set: per gait, the event tags (lowercase) and their impact data sets.
#[derive(Debug, Default)]
pub struct FootstepSet {
    groups: [Vec<(String, FormId)>; 5],
}

impl FootstepSet {
    pub fn load(lo: &LoadOrder, fsts: FormId) -> Option<FootstepSet> {
        let rec = lo.get(fsts).filter(|r| r.tag().0 == *b"FSTS")?;
        let counts: Vec<usize> = rec.get(b"XCNT")?.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap()) as usize).collect();
        let steps: Vec<FormId> = rec.get(b"DATA")?.chunks_exact(4).map(|c| rec.fid(FormId(u32::from_le_bytes(c.try_into().unwrap())))).collect();
        // The counts run walk to swim, but the footsteps are stored swim first.
        let mut set = FootstepSet::default();
        let mut next = steps.into_iter();
        for (g, &n) in counts.iter().enumerate().take(5).rev() {
            for fstp in next.by_ref().take(n) {
                let Some(step) = lo.get(fstp) else { continue };
                let (Some(tag), Some(d)) = (step.get(b"ANAM"), step.get(b"DATA")) else { continue };
                let ipds = step.fid(FormId(u32::from_le_bytes(d.get(..4)?.try_into().ok()?)));
                set.groups[g].push((esp::decode_zstring(tag).to_ascii_lowercase(), ipds));
            }
        }
        Some(set)
    }

    /// Whether an animation event (lowercase) is one of the set's footsteps.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.groups.iter().flatten().any(|(t, _)| t == tag)
    }

    /// The impact data set for an event (lowercase) at a gait, or, failing that, the
    /// first group that has the tag (sprint-only tags, a creature's breathing).
    pub fn impacts(&self, tag: &str, gait: Gait) -> Option<FormId> {
        let find = |g: &Vec<(String, FormId)>| g.iter().find(|(t, _)| t == tag).map(|s| s.1);
        find(&self.groups[gait as usize]).or_else(|| self.groups.iter().find_map(find))
    }
}

/// Impact data sets' material -> impact pairs, and materials' parents.
#[derive(Default)]
pub struct Impacts {
    sets: HashMap<FormId, Vec<(FormId, FormId)>>,
}

impl Impacts {
    /// The sound (SNDR) an impact data set makes on a material: its own entry, or
    /// its parent material's (stone stairs sound as stone).
    pub fn sound(&mut self, lo: &LoadOrder, ipds: FormId, material: FormId) -> Option<FormId> {
        let pairs = self.sets.entry(ipds).or_insert_with(|| {
            let Some(rec) = lo.get(ipds) else { return Vec::new() };
            rec.subrecords()
                .filter(|s| s.tag.0 == *b"PNAM" && s.data.len() >= 8)
                .map(|s| (rec.fid(s.form_id(0)), rec.fid(s.form_id(4))))
                .collect()
        });
        let mut m = material;
        for _ in 0..8 {
            if let Some(&(_, ipct)) = pairs.iter().find(|p| p.0 == m) {
                let rec = lo.get(ipct)?;
                let d = rec.get(b"SNAM")?;
                let s = rec.fid(FormId(u32::from_le_bytes(d.get(..4)?.try_into().ok()?)));
                return (!s.is_null()).then_some(s);
            }
            let rec = lo.get(m)?;
            let d = rec.get(b"PNAM")?;
            m = rec.fid(FormId(u32::from_le_bytes(d.get(..4)?.try_into().ok()?)));
        }
        None
    }
}

/// The material type (MATT) of a landscape texture (LTEX `MNAM`).
pub fn land_material(lo: &LoadOrder, ltex: FormId) -> Option<FormId> {
    let rec = lo.get(ltex)?;
    let d = rec.get(b"MNAM")?;
    Some(rec.fid(FormId(u32::from_le_bytes(d.get(..4)?.try_into().ok()?))))
}

/// Dirt: the default landscape texture's material; stone, for collision whose
/// material no MATT names.
pub const DIRT: FormId = FormId(0x12F38);
pub const STONE: FormId = FormId(0x12F34);

/// Material types (MATT) by the Havok material ids collision shapes carry: the
/// CRC-32 of the type's lowercase name (`MNAM`; zero start, no final inversion).
pub fn havok_materials(lo: &LoadOrder) -> HashMap<u32, FormId> {
    lo.ids_of_type(b"MATT")
        .iter()
        .filter_map(|&id| {
            let rec = lo.get(id)?;
            let name = esp::decode_zstring(rec.get(b"MNAM")?).to_ascii_lowercase();
            Some((crc32(name.as_bytes()), id))
        })
        .collect()
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut c = 0u32;
    for &b in bytes {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { (c >> 1) ^ 0xEDB8_8320 } else { c >> 1 };
        }
    }
    c
}

#[cfg(test)]
mod tests {
    #[test]
    fn havok_material_ids() {
        // SKY_HAV_MAT_STONE, _GRASS, _STAIRS_STONE.
        assert_eq!(super::crc32(b"stone"), 3741512247);
        assert_eq!(super::crc32(b"grass"), 1848600814);
        assert_eq!(super::crc32(b"stairsstone"), 899511101);
    }
}
