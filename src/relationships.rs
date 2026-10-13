//! Relationships between NPCs (`RELA`): a rank from lover to archnemesis
//! between two NPC records, read by `GetRelationshipRank` and changed by
//! Papyrus (`SetRelationshipRank`), which sends a change relationship rank
//! story event. A change is kept between the two references, not their
//! NPCs: another reference of the same NPC keeps the record's rank (`test-mod
//! autotest2`); both ways read the same.

use std::collections::HashMap;

use esp::FormId;

use crate::engine::{Engine, PLAYER_REF};

/// The ranks as conditions and Papyrus count them: 4 lover, 3 ally,
/// 2 confidant, 1 friend, 0 acquaintance, -1 rival, -2 foe, -3 enemy,
/// -4 archnemesis. The record counts down from lover (0) instead.
fn rank_of_record(r: u16) -> i32 {
    4 - r.min(8) as i32
}

#[derive(Default)]
pub struct Relationships {
    /// Ranks from the records, by NPC pair (lower form id first).
    authored: std::cell::OnceCell<HashMap<(FormId, FormId), i32>>,
    /// Ranks changed since, by reference pair (the player as its
    /// reference; an NPC record when a change named one).
    changed: HashMap<(FormId, FormId), i32>,
    /// NPC records in each faction (rank 0 or more), for faction ownership.
    members: std::cell::OnceCell<HashMap<FormId, Vec<FormId>>>,
}

fn pair(a: FormId, b: FormId) -> (FormId, FormId) {
    if a <= b { (a, b) } else { (b, a) }
}

impl Engine {
    fn authored_relationships(&self) -> &HashMap<(FormId, FormId), i32> {
        self.relationships.authored.get_or_init(|| {
            let mut ranks = HashMap::new();
            for &id in self.lo.ids_of_type(b"RELA") {
                let Some(rec) = self.lo.get(id) else { continue };
                // DATA: parent NPC, child NPC, rank, unknown, flags, association type.
                let Some(d) = rec.get(b"DATA").filter(|d| d.len() >= 10) else {
                    continue;
                };
                let a = rec.fid(FormId(u32::from_le_bytes(d[0..4].try_into().unwrap())));
                let b = rec.fid(FormId(u32::from_le_bytes(d[4..8].try_into().unwrap())));
                ranks.insert(pair(a, b), rank_of_record(u16::from_le_bytes([d[8], d[9]])));
            }
            log::info!("relationships: {}", ranks.len());
            ranks
        })
    }

    /// The NPC record a relationship is kept on: a reference's base (the
    /// player's is the player NPC record).
    fn relationship_npc(&self, r: FormId) -> Option<FormId> {
        if self.lo.tag_of(r).is_some_and(|t| t.0 == *b"NPC_") {
            Some(r)
        } else {
            self.base_of(r)
        }
    }

    /// What a change is kept on: the reference, the player's reference for
    /// the player's NPC record.
    fn relationship_key(r: FormId) -> FormId {
        if r == FormId(0x7) { PLAYER_REF } else { r }
    }

    /// The rank between two actors (references or NPC records): changed
    /// between them, else their records'; acquaintances (0) when nothing is
    /// set.
    pub fn relationship_rank(&self, a: FormId, b: FormId) -> i32 {
        let keys = pair(Self::relationship_key(a), Self::relationship_key(b));
        if let Some(&r) = self.relationships.changed.get(&keys) {
            return r;
        }
        let (Some(a), Some(b)) = (self.relationship_npc(a), self.relationship_npc(b)) else {
            return 0;
        };
        self.authored_relationships()
            .get(&pair(a, b))
            .copied()
            .unwrap_or(0)
    }

    /// The highest (or lowest) rank an actor has with anyone; 0 with no one.
    pub(crate) fn relationship_extreme(&self, a: FormId, highest: bool) -> i32 {
        let Some(npc) = self.relationship_npc(a) else {
            return 0;
        };
        let key = Self::relationship_key(a);
        let changed = self
            .relationships
            .changed
            .iter()
            .filter(|((x, y), _)| *x == key || *y == key);
        let authored = self
            .authored_relationships()
            .iter()
            .filter(|((x, y), _)| *x == npc || *y == npc)
            .filter(|(p, _)| !self.relationships.changed.contains_key(p));
        let mine = changed.chain(authored).map(|(_, &r)| r);
        if highest { mine.max() } else { mine.min() }.unwrap_or(0)
    }

    /// The NPC records that belong to a faction (from their own or their
    /// templates' faction lists; membership doesn't change at runtime yet).
    pub(crate) fn faction_members(&self, faction: FormId) -> &[FormId] {
        let members = self.relationships.members.get_or_init(|| {
            let mut out: HashMap<FormId, Vec<FormId>> = HashMap::new();
            for &npc in self.lo.ids_of_type(b"NPC_") {
                for (f, rank) in self.npc_factions(npc) {
                    if rank >= 0 {
                        out.entry(f).or_default().push(npc);
                    }
                }
            }
            out
        });
        members.get(&faction).map_or(&[], Vec::as_slice)
    }

    /// Change the rank between two actors: a change relationship rank story
    /// event (`CHRR`: R1, R2, V1 the old rank, V2 the new) when it differs.
    pub fn set_relationship_rank(&mut self, a: FormId, b: FormId, rank: i32) {
        let (Some(na), Some(nb)) = (self.relationship_npc(a), self.relationship_npc(b)) else {
            return;
        };
        let rank = rank.clamp(-4, 4);
        let old = self.relationship_rank(a, b);
        if old == rank {
            return;
        }
        self.relationships.changed.insert(
            pair(Self::relationship_key(a), Self::relationship_key(b)),
            rank,
        );
        log::info!("relationship {a} / {b}: {old} -> {rank}");
        // Events name references: the player, or the NPC's placed reference.
        let as_ref = |e: &Engine, r: FormId, npc: FormId| match r {
            _ if r != npc => r,
            FormId(0x7) => PLAYER_REF,
            _ => e.npc_refs_index().get(&npc).copied().unwrap_or(r),
        };
        let mut ev = crate::story::StoryEvent::new(b"CHRR");
        ev.refs = [as_ref(self, a, na), as_ref(self, b, nb)];
        ev.values = [old as f32, rank as f32];
        self.send_story_event(ev);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_ranks() {
        assert_eq!(rank_of_record(0), 4); // lover
        assert_eq!(rank_of_record(3), 1); // friend
        assert_eq!(rank_of_record(4), 0); // acquaintance
        assert_eq!(rank_of_record(8), -4); // archnemesis
    }
}
