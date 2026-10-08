//! Story Manager nodes: event nodes (`SMEN`, the root under which an event
//! type's quests are found), branch nodes (`SMBN`) and quest nodes (`SMQN`,
//! the quests they may start). Nodes name their parent (`PNAM`) and the sibling
//! before them (`SNAM`); conditions follow their count (`CITC`).

use crate::scene::RawCondition;
use crate::{FormId, LoadedRecord};

pub mod flags {
    /// Node flags (`DNAM`, low half).
    pub const RANDOM: u16 = 0x1;
    pub const WARN_IF_NO_CHILD_QUEST_STARTED: u16 = 0x2;
    /// Quest node flags (`DNAM`, high half).
    pub const DO_ALL_BEFORE_REPEATING: u16 = 0x1;
    pub const SHARES_EVENT: u16 = 0x2;
    pub const NUM_QUESTS_TO_RUN: u16 = 0x4;
    /// A quest node's per-quest flags (`FNAM`).
    pub const QUEST_SHARES_EVENT: u32 = 0x1;
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeKind {
    /// The event type's four characters (`ENAM`: `CLOC`, `SCPT`, `KILL`...).
    Event([u8; 4]),
    Branch,
    /// Quests, each with its flags (`FNAM`) and hours before it may run again
    /// (`RNAM`); how many to start (`MNAM`, with the flag).
    Quest {
        quests: Vec<NodeQuest>,
        num_to_run: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodeQuest {
    pub quest: FormId,
    pub flags: u32,
    pub reset_hours: f32,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub id: FormId,
    pub editor_id: String,
    pub kind: NodeKind,
    pub parent: FormId,
    pub previous: FormId,
    /// Most quests from this node running at once (0: no limit).
    pub max_concurrent: u32,
    pub node_flags: u16,
    pub quest_flags: u16,
    pub conditions: Vec<RawCondition>,
}

impl Node {
    pub fn event(&self) -> Option<[u8; 4]> {
        match self.kind {
            NodeKind::Event(e) => Some(e),
            _ => None,
        }
    }
}

pub fn parse(rec: &LoadedRecord<'_>, id: FormId) -> Option<Node> {
    let mut kind = match &rec.tag().0 {
        b"SMEN" => NodeKind::Event([0; 4]),
        b"SMBN" => NodeKind::Branch,
        b"SMQN" => NodeKind::Quest {
            quests: Vec::new(),
            num_to_run: 0,
        },
        _ => return None,
    };
    let mut n = Node {
        id,
        editor_id: rec.editor_id().unwrap_or_default(),
        kind: NodeKind::Branch,
        parent: FormId::NULL,
        previous: FormId::NULL,
        max_concurrent: 0,
        node_flags: 0,
        quest_flags: 0,
        conditions: Vec::new(),
    };
    for sr in rec.subrecords() {
        let u32_of = || {
            sr.data
                .get(0..4)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                .unwrap_or(0)
        };
        let form = || rec.fid(FormId(u32_of()));
        match (&sr.tag.0, &mut kind) {
            (b"PNAM", _) => n.parent = form(),
            (b"SNAM", _) => n.previous = form(),
            (b"XNAM", _) => n.max_concurrent = u32_of(),
            (b"DNAM", _) if sr.data.len() >= 4 => {
                n.node_flags = u16::from_le_bytes([sr.data[0], sr.data[1]]);
                n.quest_flags = u16::from_le_bytes([sr.data[2], sr.data[3]]);
            }
            (b"CTDA", _) => n.conditions.push(RawCondition {
                ctda: sr.data.to_vec(),
                ..Default::default()
            }),
            (b"CIS1", _) => {
                if let Some(c) = n.conditions.last_mut() {
                    c.cis1 = Some(sr.zstring());
                }
            }
            (b"CIS2", _) => {
                if let Some(c) = n.conditions.last_mut() {
                    c.cis2 = Some(sr.zstring());
                }
            }
            (b"ENAM", NodeKind::Event(e)) if sr.data.len() >= 4 => {
                e.copy_from_slice(&sr.data[0..4])
            }
            (b"MNAM", NodeKind::Quest { num_to_run, .. }) => *num_to_run = u32_of(),
            (b"NNAM", NodeKind::Quest { quests, .. }) => quests.push(NodeQuest {
                quest: form(),
                flags: 0,
                reset_hours: 0.0,
            }),
            (b"FNAM", NodeKind::Quest { quests, .. }) => {
                if let Some(q) = quests.last_mut() {
                    q.flags = u32_of();
                }
            }
            (b"RNAM", NodeKind::Quest { quests, .. }) => {
                if let Some(q) = quests.last_mut() {
                    q.reset_hours = f32::from_bits(u32_of());
                }
            }
            _ => {}
        }
    }
    n.kind = kind;
    Some(n)
}

/// An event type or member code as text (`CLOC`, `R1`).
pub fn code(b: &[u8]) -> String {
    b.iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as char)
        .collect()
}
