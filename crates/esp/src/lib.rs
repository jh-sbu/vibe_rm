//! Reading of TES4-format plugin files (Skyrim `.esm`, `.esp`, `.esl`).
//!
//! The layer here is deliberately low level: a [`Plugin`] exposes the header
//! and lets you walk groups and records. [`LoadOrder`] stitches several
//! plugins together, resolving FormIDs into a single global space and
//! indexing records so higher-priority overrides win.

pub mod actor_value;
mod form_id;
mod load_order;
pub mod navmesh;
mod plugin;
mod record;
pub mod strings;

pub use form_id::FormId;
pub use load_order::{BASE_MASTERS, CellIndex, LoadOrder, LoadedRecord, PluginInfo, RecordRef, Slot, WorldIndex};
pub use plugin::{GroupHeader, Item, Plugin, PluginHeader, group_type};
pub use record::{Record, RecordHeader, SubRecord, SubRecords, record_flags};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}: not a TES4 plugin")]
    NotAPlugin(String),
    #[error("corrupt plugin data: {0}")]
    Corrupt(String),
    #[error("missing master {master} required by {plugin}")]
    MissingMaster { plugin: String, master: String },
}

pub type Result<T> = std::result::Result<T, Error>;

/// Four character record / subrecord / group label code.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Tag(pub [u8; 4]);

impl Tag {
    pub const fn new(s: &[u8; 4]) -> Self {
        Tag(*s)
    }
}

impl std::fmt::Debug for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", String::from_utf8_lossy(&self.0))
    }
}
impl std::fmt::Display for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", String::from_utf8_lossy(&self.0))
    }
}

impl PartialEq<&[u8; 4]> for Tag {
    fn eq(&self, other: &&[u8; 4]) -> bool {
        &self.0 == *other
    }
}

/// Decode a zero-terminated Windows-1252 string.
pub fn decode_zstring(b: &[u8]) -> String {
    let b = match b.iter().position(|&c| c == 0) {
        Some(p) => &b[..p],
        None => b,
    };
    let (s, _, _) = encoding_rs::WINDOWS_1252.decode(b);
    s.into_owned()
}

pub(crate) fn le_u16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
pub(crate) fn le_u32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}
