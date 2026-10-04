use std::fs::File;
use std::path::{Path, PathBuf};

use memmap2::Mmap;

use crate::record::{RECORD_HEADER_SIZE, Record, RecordHeader, record_flags};
use crate::{Error, Result, Tag, le_u32};

pub const GROUP_HEADER_SIZE: usize = 24;

#[derive(Debug, Clone)]
pub struct PluginHeader {
    pub flags: u32,
    pub version: f32,
    pub num_records: u32,
    pub next_object_id: u32,
    pub author: String,
    pub description: String,
    pub masters: Vec<String>,
}

impl PluginHeader {
    pub fn is_master(&self) -> bool {
        self.flags & record_flags::MASTER != 0
    }
    pub fn is_localized(&self) -> bool {
        self.flags & record_flags::LOCALIZED != 0
    }
    pub fn is_light(&self) -> bool {
        self.flags & record_flags::LIGHT != 0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GroupHeader {
    /// Total size including this header.
    pub size: u32,
    pub label: [u8; 4],
    pub group_type: i32,
}

impl GroupHeader {
    pub fn label_u32(&self) -> u32 {
        u32::from_le_bytes(self.label)
    }
    pub fn label_tag(&self) -> Tag {
        Tag(self.label)
    }
    /// For exterior block / sub-block groups: (y, x) packed as two i16.
    pub fn label_grid(&self) -> (i16, i16) {
        let y = i16::from_le_bytes([self.label[0], self.label[1]]);
        let x = i16::from_le_bytes([self.label[2], self.label[3]]);
        (x, y)
    }
}

pub mod group_type {
    pub const TOP: i32 = 0;
    pub const WORLD_CHILDREN: i32 = 1;
    pub const INTERIOR_BLOCK: i32 = 2;
    pub const INTERIOR_SUB_BLOCK: i32 = 3;
    pub const EXTERIOR_BLOCK: i32 = 4;
    pub const EXTERIOR_SUB_BLOCK: i32 = 5;
    pub const CELL_CHILDREN: i32 = 6;
    pub const TOPIC_CHILDREN: i32 = 7;
    pub const CELL_PERSISTENT: i32 = 8;
    pub const CELL_TEMPORARY: i32 = 9;
}

/// An item inside a group: either a nested group or a record.
#[derive(Debug, Clone, Copy)]
pub enum Item {
    /// Group header and absolute offset of its *contents* (after the header).
    Group(GroupHeader, usize),
    /// Record header and absolute offset of the record (start of header).
    Record(RecordHeader, usize),
}

pub struct Plugin {
    name: String,
    path: PathBuf,
    map: Mmap,
    header: PluginHeader,
    /// Offset of the first top-level group.
    body_start: usize,
}

impl Plugin {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let file = File::open(&path)?;
        // SAFETY: plugin files are read-only for the process lifetime.
        let map = unsafe { Mmap::map(&file)? };
        if map.len() < RECORD_HEADER_SIZE || &map[0..4] != b"TES4" {
            return Err(Error::NotAPlugin(name));
        }
        let hdr = RecordHeader::parse(&map);
        let rec = Record::from_raw(&map[..RECORD_HEADER_SIZE + hdr.data_size as usize])?;
        let mut header = PluginHeader {
            flags: hdr.flags,
            version: 0.0,
            num_records: 0,
            next_object_id: 0,
            author: String::new(),
            description: String::new(),
            masters: Vec::new(),
        };
        for sr in rec.subrecords() {
            match &sr.tag.0 {
                b"HEDR" => {
                    header.version = sr.f32(0);
                    header.num_records = sr.u32(4);
                    header.next_object_id = sr.u32(8);
                }
                b"CNAM" => header.author = sr.zstring(),
                b"SNAM" => header.description = sr.zstring(),
                b"MAST" => header.masters.push(sr.zstring()),
                _ => {}
            }
        }
        let body_start = RECORD_HEADER_SIZE + hdr.data_size as usize;
        Ok(Plugin { name, path, map, header, body_start })
    }

    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn header(&self) -> &PluginHeader {
        &self.header
    }
    pub fn bytes(&self) -> &[u8] {
        &self.map
    }

    /// Items at the top level of the file (should all be groups).
    pub fn top_level(&self) -> Items<'_> {
        Items { data: &self.map, pos: self.body_start, end: self.map.len() }
    }

    /// Items inside a group given its content offset (as returned by [`Item::Group`]).
    pub fn group_items(&self, header: &GroupHeader, content_offset: usize) -> Items<'_> {
        let end = content_offset + header.size as usize - GROUP_HEADER_SIZE;
        Items { data: &self.map, pos: content_offset, end: end.min(self.map.len()) }
    }

    /// Load a record at the given absolute offset.
    pub fn record_at(&self, offset: usize) -> Result<Record<'_>> {
        let hdr = RecordHeader::parse(&self.map[offset..]);
        let end = offset + RECORD_HEADER_SIZE + hdr.data_size as usize;
        if end > self.map.len() {
            return Err(Error::Corrupt(format!("record at {offset:#x} overruns file")));
        }
        Record::from_raw(&self.map[offset..end])
    }
}

impl std::fmt::Debug for Plugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Plugin").field("name", &self.name).field("header", &self.header).finish()
    }
}

pub struct Items<'a> {
    data: &'a [u8],
    pos: usize,
    end: usize,
}

impl Iterator for Items<'_> {
    type Item = Item;
    fn next(&mut self) -> Option<Item> {
        if self.pos + RECORD_HEADER_SIZE > self.end {
            return None;
        }
        let b = &self.data[self.pos..];
        let at = self.pos;
        if &b[0..4] == b"GRUP" {
            let size = le_u32(b, 4);
            if size < GROUP_HEADER_SIZE as u32 {
                log::error!("corrupt group at {at:#x}");
                self.pos = self.end;
                return None;
            }
            let gh = GroupHeader {
                size,
                label: b[8..12].try_into().unwrap(),
                group_type: le_u32(b, 12) as i32,
            };
            self.pos += size as usize;
            Some(Item::Group(gh, at + GROUP_HEADER_SIZE))
        } else {
            let rh = RecordHeader::parse(b);
            self.pos += RECORD_HEADER_SIZE + rh.data_size as usize;
            Some(Item::Record(rh, at))
        }
    }
}
