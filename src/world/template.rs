//! NPC templates (`TPLT`): which record an actor takes each part of its definition
//! from. An NPC's template flags (`ACBS`, "Use Traits", "Use Stats"...) send each part
//! to its template, itself an NPC or a leveled NPC list, which may be templated in
//! turn. Leveled templates are picked per reference, as the actor's own base is, so
//! a bandit's stats, factions and packages come from the same NPC as its looks.

use esp::{FormId, LoadOrder, LoadedRecord};

pub const TRAITS: u16 = 0x1;
pub const STATS: u16 = 0x2;
pub const FACTIONS: u16 = 0x4;
#[allow(dead_code)] // no magic yet
pub const SPELLS: u16 = 0x8;
pub const AI_DATA: u16 = 0x10;
pub const AI_PACKAGES: u16 = 0x20;
pub const BASE_DATA: u16 = 0x80;
pub const INVENTORY: u16 = 0x100;
pub const SCRIPT: u16 = 0x200;
#[allow(dead_code)] // default package lists aren't used yet
pub const DEF_PACK_LIST: u16 = 0x400;
pub const ATTACK_DATA: u16 = 0x800;
pub const KEYWORDS: u16 = 0x1000;

const FLAG_COUNT: usize = 13;
/// The parts by flag bit, as the Creation Kit names them.
pub const NAMES: [&str; FLAG_COUNT] = [
    "traits", "stats", "factions", "spells", "ai data", "ai packages", "model / animation", "base data",
    "inventory", "script", "def pack list", "attack data", "keywords",
];
const MAX_DEPTH: usize = 8;

/// For each template flag, the NPC record that part comes from; and the chain of
/// records followed (the NPC, its template, that one's template...).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Sources {
    by_flag: [FormId; FLAG_COUNT],
    chain: Vec<FormId>,
}

impl Sources {
    /// Where an actor placed with `base` (an NPC or a leveled NPC list) by reference
    /// `seed` takes each part from; `None` when a leveled list picks nothing.
    pub fn for_ref(lo: &LoadOrder, base: FormId, seed: u64) -> Option<Sources> {
        let npc = super::actor::resolve_leveled(lo, base, b"LVLN", seed)?;
        lo.get(npc).filter(|r| r.tag().0 == *b"NPC_")?;
        Some(Sources::of_npc(lo, npc, seed))
    }

    /// The sources of an NPC record, leveled templates picked by `seed`.
    pub fn of_npc(lo: &LoadOrder, npc: FormId, seed: u64) -> Sources {
        Sources::resolve(lo, npc, seed, 0)
    }

    fn resolve(lo: &LoadOrder, npc: FormId, seed: u64, depth: usize) -> Sources {
        let own = Sources { by_flag: [npc; FLAG_COUNT], chain: vec![npc] };
        let Some(rec) = lo.get(npc) else { return own };
        let flags = template_flags(&rec);
        let template = rec
            .get(b"TPLT")
            .filter(|d| d.len() >= 4)
            .map(|d| rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap()))))
            .filter(|t| !t.is_null() && *t != npc);
        let Some(t) = template
            .filter(|_| depth < MAX_DEPTH)
            .and_then(|t| super::actor::resolve_leveled(lo, t, b"LVLN", seed))
            .filter(|t| lo.get(*t).is_some_and(|r| r.tag().0 == *b"NPC_"))
        else {
            return own;
        };
        let inner = Sources::resolve(lo, t, seed, depth + 1);
        let mut out = own;
        for (i, slot) in out.by_flag.iter_mut().enumerate() {
            if flags & (1 << i) != 0 {
                *slot = inner.by_flag[i];
            }
        }
        out.chain.extend(inner.chain);
        out
    }

    /// The NPC giving the part `flag` (one of the constants here).
    pub fn of(&self, flag: u16) -> FormId {
        self.by_flag[flag.trailing_zeros() as usize]
    }

    /// Each part's name and source NPC.
    pub fn parts(&self) -> impl Iterator<Item = (&'static str, FormId)> + '_ {
        NAMES.iter().copied().zip(self.by_flag.iter().copied())
    }

    /// The records followed: the NPC, its template, that one's template...
    pub fn chain(&self) -> &[FormId] {
        &self.chain
    }

    /// The record with the subrecord `tag` for part `flag`: the part's source, else
    /// (lacking it) the templates after it.
    pub fn record<'a>(&self, lo: &'a LoadOrder, flag: u16, tag: &[u8; 4]) -> Option<LoadedRecord<'a>> {
        let from = self.of(flag);
        let start = self.chain.iter().position(|&n| n == from).unwrap_or(0);
        self.chain[start..].iter().filter_map(|&n| lo.get(n)).find(|r| r.get(tag).is_some())
    }

    /// The subrecord `tag` of part `flag` (see `record`).
    pub fn field(&self, lo: &LoadOrder, flag: u16, tag: &[u8; 4]) -> Option<Vec<u8>> {
        self.record(lo, flag, tag).and_then(|r| r.get(tag).map(<[u8]>::to_vec))
    }

    /// A form id held by the subrecord `tag` of part `flag`.
    pub fn form(&self, lo: &LoadOrder, flag: u16, tag: &[u8; 4]) -> Option<FormId> {
        let r = self.record(lo, flag, tag)?;
        let d = r.get(tag).filter(|d| d.len() >= 4)?;
        Some(r.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap())))).filter(|f| !f.is_null())
    }

    /// Factions and ranks: the factions part's, or the next template's when it lists
    /// none. A negative rank isn't membership (potential followers'
    /// `CurrentFollowerFaction`).
    pub fn factions(&self, lo: &LoadOrder) -> Vec<(FormId, i8)> {
        let from = self.of(FACTIONS);
        let start = self.chain.iter().position(|&n| n == from).unwrap_or(0);
        for rec in self.chain[start..].iter().filter_map(|&n| lo.get(n)) {
            let own: Vec<(FormId, i8)> = rec
                .subrecords()
                .filter(|s| s.tag.0 == *b"SNAM" && s.data.len() >= 5)
                .map(|s| (rec.fid(s.form_id(0)), s.data[4] as i8))
                .collect();
            if !own.is_empty() {
                return own;
            }
        }
        Vec::new()
    }

    /// The factions it belongs to (rank 0 or more).
    pub fn member_of(&self, lo: &LoadOrder) -> Vec<FormId> {
        self.factions(lo).into_iter().filter(|(_, r)| *r >= 0).map(|(f, _)| f).collect()
    }

    /// Keywords (`KWDA`) of the keywords part.
    pub fn keywords(&self, lo: &LoadOrder) -> Vec<FormId> {
        let Some(rec) = lo.get(self.of(KEYWORDS)) else { return Vec::new() };
        rec.get(b"KWDA").unwrap_or(&[]).chunks_exact(4).map(|c| rec.fid(FormId(u32::from_le_bytes(c.try_into().unwrap())))).collect()
    }
}

/// An NPC's template flags (`ACBS` bytes 18..20).
pub fn template_flags(rec: &LoadedRecord<'_>) -> u16 {
    rec.get(b"ACBS").filter(|d| d.len() >= 20).map_or(0, |d| u16::from_le_bytes([d[18], d[19]]))
}
