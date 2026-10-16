//! Relationships between NPCs (`RELA`): a rank from lover to archnemesis
//! between two NPC records, read by `GetRelationshipRank` and changed by
//! Papyrus (`SetRelationshipRank`), which sends a change relationship rank
//! story event. A change is kept between the two references, not their
//! NPCs: another reference of the same NPC keeps the record's rank (`test-mod
//! autotest2`); both ways read the same.

use std::collections::HashMap;

use esp::{FormId, LoadOrder};

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
    associations: std::cell::OnceCell<AssociationIndex>,
    /// Ranks changed since, by reference pair (the player as its
    /// reference; an NPC record when a change named one).
    changed: HashMap<(FormId, FormId), i32>,
    /// NPC records in each faction (rank 0 or more), for faction ownership.
    members: std::cell::OnceCell<HashMap<FormId, Vec<FormId>>>,
}

/// Keep direction and secrecy even though association/family queries are symmetric.
/// Neither the rank nor the parent/child labels imply a family association.
#[derive(Debug)]
struct Association {
    parent: FormId,
    child: FormId,
    kind: FormId,
    family: bool,
    secret: bool,
}

#[derive(Default)]
struct AssociationIndex {
    records: Vec<Association>,
    by_npc: HashMap<FormId, Vec<usize>>,
}

impl AssociationIndex {
    fn load(lo: &LoadOrder) -> Self {
        let mut out = Self::default();
        for &id in lo.ids_of_type(b"RELA") {
            let Some(rec) = lo.get(id) else { continue };
            let Some(d) = rec.get(b"DATA").filter(|d| d.len() >= 16) else {
                continue;
            };
            let fid = |offset| {
                rec.fid(FormId(u32::from_le_bytes(
                    d[offset..offset + 4].try_into().unwrap(),
                )))
            };
            let kind = fid(12);
            let family = lo
                .get(kind)
                .filter(|r| r.tag().0 == *b"ASTP")
                .and_then(|r| {
                    r.get(b"DATA")
                        .filter(|d| d.len() >= 4)
                        .map(|d| u32::from_le_bytes(d[..4].try_into().unwrap()))
                })
                .is_some_and(|flags| flags & 1 != 0);
            let a = Association {
                parent: fid(0),
                child: fid(4),
                kind,
                family,
                // xEdit describes both representations; behavior is unresolved.
                secret: d[11] & 0x80 != 0 || rec.flags() & 0x40 != 0,
            };
            if a.parent.is_null() || a.child.is_null() {
                continue;
            }
            let i = out.records.len();
            out.by_npc.entry(a.parent).or_default().push(i);
            if a.child != a.parent {
                out.by_npc.entry(a.child).or_default().push(i);
            }
            out.records.push(a);
        }
        out
    }

    fn matching(&self, npc: FormId, other: Option<FormId>) -> impl Iterator<Item = &Association> {
        self.by_npc
            .get(&npc)
            .into_iter()
            .flatten()
            .map(|&i| &self.records[i])
            .filter(move |a| {
                other.is_none_or(|other| {
                    (a.parent == npc && a.child == other) || (a.child == npc && a.parent == other)
                })
            })
    }

    fn has_family(&self, npc: FormId, other: Option<FormId>) -> bool {
        self.matching(npc, other).any(|a| a.family)
    }

    fn has_association(&self, npc: FormId, other: Option<FormId>, kind: FormId) -> bool {
        !kind.is_null() && self.matching(npc, other).any(|a| a.kind == kind)
    }
}

fn pair(a: FormId, b: FormId) -> (FormId, FormId) {
    if a <= b { (a, b) } else { (b, a) }
}

impl Engine {
    fn association_index(&self) -> &AssociationIndex {
        self.relationships
            .associations
            .get_or_init(|| AssociationIndex::load(&self.lo))
    }

    /// A direct authored family association with another actor, or anyone.
    /// Runtime rank changes do not change the association type.
    pub fn has_family_relationship(&self, actor: FormId, other: Option<FormId>) -> bool {
        let Some(npc) = self.relationship_npc(actor) else {
            return false;
        };
        let other = match other {
            Some(other) => match self.relationship_npc(other) {
                Some(npc) => Some(npc),
                None => return false,
            },
            None => None,
        };
        self.association_index().has_family(npc, other)
    }

    /// A direct authored association of the requested type, in either direction.
    pub fn has_association(&self, actor: FormId, other: Option<FormId>, kind: FormId) -> bool {
        let Some(npc) = self.relationship_npc(actor) else {
            return false;
        };
        let other = match other {
            Some(other) => match self.relationship_npc(other) {
                Some(npc) => Some(npc),
                None => return false,
            },
            None => None,
        };
        self.association_index().has_association(npc, other, kind)
    }

    /// Authored secrecy metadata, for future social knowledge handling.
    /// This does not hide the rank or association from direct queries.
    pub fn relationship_is_secret(&self, actor: FormId, other: FormId) -> bool {
        let (Some(npc), Some(other)) = (self.relationship_npc(actor), self.relationship_npc(other))
        else {
            return false;
        };
        self.association_index()
            .matching(npc, Some(other))
            .any(|a| a.secret)
    }

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
    use esp::writer::{Node, RecordBuilder, write_plugin};

    struct Fixture(std::path::PathBuf);

    impl Fixture {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let dir = std::env::temp_dir().join(format!(
                "vrm-relationships-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn load(&self, relationships: Vec<RecordBuilder>) -> LoadOrder {
            let master = write_plugin(1, &[], "test", "synthetic", 0x800, &[]);
            std::fs::write(self.0.join("Base.esm"), master).unwrap();
            let associations = write_plugin(
                1,
                &["Base.esm"],
                "test",
                "synthetic",
                0x900,
                &[Node::top(
                    b"ASTP",
                    vec![
                        RecordBuilder::new(b"ASTP", id(10)).u32(b"DATA", 1),
                        RecordBuilder::new(b"ASTP", id(11)).u32(b"DATA", 0),
                        RecordBuilder::new(b"ASTP", id(12)).field(b"DATA", &[1]),
                    ],
                )],
            );
            std::fs::write(self.0.join("Associations.esm"), associations).unwrap();
            // Reverse the master table: local slot 0 resolves to global slot 1.
            let plugin = write_plugin(
                0,
                &["Associations.esm", "Base.esm"],
                "test",
                "synthetic",
                0x900,
                &[Node::top(b"RELA", relationships)],
            );
            std::fs::write(self.0.join("Relationships.esp"), plugin).unwrap();
            LoadOrder::load(
                &self.0,
                &[
                    "Base.esm".into(),
                    "Associations.esm".into(),
                    "Relationships.esp".into(),
                ],
            )
            .unwrap()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    // Global NPC/association IDs in the second plugin.
    fn id(local: u32) -> FormId {
        FormId(0x0100_0800 + local)
    }

    fn relation(local: u32, a: u32, b: u32, kind: u32, flags: u8) -> RecordBuilder {
        let local_id = |n| 0x800u32 + n; // Associations.esm is local master 0, global slot 1
        let mut d = local_id(a).to_le_bytes().to_vec();
        d.extend_from_slice(&local_id(b).to_le_bytes());
        d.extend_from_slice(&[4, 0, 0, flags]); // acquaintance: family is independent of rank
        d.extend_from_slice(&local_id(kind).to_le_bytes());
        RecordBuilder::new(b"RELA", FormId(0x0200_0800 + local)).field(b"DATA", &d)
    }

    #[test]
    fn association_condition_parameters_resolve_reordered_masters() {
        let fixture = Fixture::new();
        let mut ctda = [0; 32];
        ctda[8..10].copy_from_slice(&258u16.to_le_bytes());
        ctda[12..16].copy_from_slice(&0x801u32.to_le_bytes());
        ctda[16..20].copy_from_slice(&0x80au32.to_le_bytes());
        let lo = fixture.load(vec![relation(20, 1, 2, 10, 0).field(b"CTDA", &ctda)]);
        let rec = lo.get(FormId(0x0200_0814)).unwrap();
        let conds = crate::condition::parse_all(&rec);
        assert_eq!(conds.len(), 1);
        assert_eq!(conds[0].p1, id(1).0);
        assert_eq!(conds[0].p2, id(10).0);
        let index = AssociationIndex::load(&lo);
        assert!(index.has_association(FormId(conds[0].p1), Some(id(2)), FormId(conds[0].p2)));
    }

    #[test]
    fn authored_family_and_associations_are_symmetric_and_direct() {
        let fixture = Fixture::new();
        let lo = fixture.load(vec![
            relation(20, 1, 2, 10, 0),
            relation(21, 2, 3, 10, 0),
            relation(22, 1, 4, 11, 0),
        ]);
        let index = AssociationIndex::load(&lo);
        assert!(index.has_family(id(1), Some(id(2))));
        assert!(index.has_family(id(2), Some(id(1))));
        assert!(index.has_family(id(1), None));
        assert!(!index.has_family(id(1), Some(id(3)))); // no inferred relatives
        assert!(!index.has_family(id(1), Some(id(4)))); // non-family association
        assert!(!index.has_family(id(4), None));
        assert!(!index.has_family(id(99), None));
        assert!(index.has_association(id(4), Some(id(1)), id(11)));
        assert!(index.has_association(id(4), None, id(11)));
        assert!(!index.has_association(id(4), None, id(10)));
        assert!(!index.has_association(id(1), None, FormId(0)));
        assert_eq!(index.records[0].parent, id(1));
        assert_eq!(index.records[0].child, id(2));
    }

    #[test]
    fn both_secret_flags_are_preserved_without_erasing_associations() {
        let fixture = Fixture::new();
        let lo = fixture.load(vec![
            relation(20, 1, 2, 10, 0x80),
            relation(21, 2, 3, 10, 0).flags(0x40),
            relation(22, 3, 4, 10, 0),
        ]);
        let index = AssociationIndex::load(&lo);
        assert!(index.matching(id(1), Some(id(2))).all(|a| a.secret));
        assert!(index.matching(id(3), Some(id(2))).all(|a| a.secret));
        assert!(index.matching(id(3), Some(id(4))).all(|a| !a.secret));
        assert!(index.has_family(id(1), Some(id(2))));
    }

    #[test]
    fn malformed_or_missing_associations_do_not_create_family_ties() {
        let fixture = Fixture::new();
        let lo = fixture.load(vec![
            relation(20, 1, 2, 99, 0), // missing ASTP
            relation(21, 1, 3, 12, 0), // short ASTP flags
            RecordBuilder::new(b"RELA", id(22)).field(b"DATA", &[0; 15]),
            RecordBuilder::new(b"RELA", id(23)).field(b"DATA", &[0; 16]),
        ]);
        let index = AssociationIndex::load(&lo);
        assert_eq!(index.records.len(), 2);
        assert!(!index.has_family(id(1), None));
        assert!(index.has_association(id(1), Some(id(2)), id(99)));
    }

    #[test]
    fn record_ranks() {
        assert_eq!(rank_of_record(0), 4); // lover
        assert_eq!(rank_of_record(3), 1); // friend
        assert_eq!(rank_of_record(4), 0); // acquaintance
        assert_eq!(rank_of_record(8), -4); // archnemesis
    }
}
