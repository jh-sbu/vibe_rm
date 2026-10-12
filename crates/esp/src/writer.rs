//! Writing small TES4-format plugins (test mods): records built field by
//! field, nested in groups, behind a `TES4` header. Records are written
//! uncompressed with the Special Edition form version.

use crate::plugin::GROUP_HEADER_SIZE;
use crate::record::RECORD_HEADER_SIZE;
use crate::{FormId, Tag};

/// The form version Skyrim Special Edition's Creation Kit writes.
pub const FORM_VERSION: u16 = 44;
/// `HEDR` version of Special Edition plugins.
pub const HEADER_VERSION: f32 = 1.71;

/// A record and its fields, in the order they were added.
#[derive(Debug, Clone)]
pub struct RecordBuilder {
    pub tag: Tag,
    pub flags: u32,
    pub form_id: FormId,
    data: Vec<u8>,
}

impl RecordBuilder {
    pub fn new(tag: &[u8; 4], form_id: FormId) -> Self {
        RecordBuilder {
            tag: Tag(*tag),
            flags: 0,
            form_id,
            data: Vec::new(),
        }
    }

    pub fn flags(mut self, flags: u32) -> Self {
        self.flags = flags;
        self
    }

    pub fn field(mut self, tag: &[u8; 4], data: &[u8]) -> Self {
        let len = u16::try_from(data.len()).expect("field over 64 KiB needs XXXX");
        self.data.extend_from_slice(tag);
        self.data.extend_from_slice(&len.to_le_bytes());
        self.data.extend_from_slice(data);
        self
    }

    /// A zero-terminated string (Windows-1252; test mods keep to ASCII).
    pub fn zstring(self, tag: &[u8; 4], s: &str) -> Self {
        assert!(s.is_ascii(), "{s:?}: not ASCII");
        let mut b = s.as_bytes().to_vec();
        b.push(0);
        self.field(tag, &b)
    }

    pub fn u32(self, tag: &[u8; 4], v: u32) -> Self {
        self.field(tag, &v.to_le_bytes())
    }

    pub fn form(self, tag: &[u8; 4], id: FormId) -> Self {
        self.u32(tag, id.0)
    }

    pub fn f32(self, tag: &[u8; 4], v: f32) -> Self {
        self.field(tag, &v.to_le_bytes())
    }

    fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.tag.0);
        out.extend_from_slice(&(self.data.len() as u32).to_le_bytes());
        out.extend_from_slice(&self.flags.to_le_bytes());
        out.extend_from_slice(&self.form_id.0.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes()); // version control info
        out.extend_from_slice(&FORM_VERSION.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&self.data);
    }
}

/// A record, or a group of them.
#[derive(Debug, Clone)]
pub enum Node {
    Record(RecordBuilder),
    Group {
        label: [u8; 4],
        group_type: i32,
        children: Vec<Node>,
    },
}

impl Node {
    /// A top group of records of one type.
    pub fn top(tag: &[u8; 4], records: Vec<RecordBuilder>) -> Self {
        Node::Group {
            label: *tag,
            group_type: crate::group_type::TOP,
            children: records.into_iter().map(Node::Record).collect(),
        }
    }

    /// Records and groups, the plugin header's count.
    fn count(&self) -> u32 {
        match self {
            Node::Record(_) => 1,
            Node::Group { children, .. } => 1 + children.iter().map(Node::count).sum::<u32>(),
        }
    }

    fn write(&self, out: &mut Vec<u8>) {
        match self {
            Node::Record(r) => r.write(out),
            Node::Group {
                label,
                group_type,
                children,
            } => {
                let start = out.len();
                out.extend_from_slice(b"GRUP");
                out.extend_from_slice(&0u32.to_le_bytes()); // size, patched below
                out.extend_from_slice(label);
                out.extend_from_slice(&group_type.to_le_bytes());
                out.extend_from_slice(&0u32.to_le_bytes()); // timestamp, version control
                out.extend_from_slice(&FORM_VERSION.to_le_bytes());
                out.extend_from_slice(&0u16.to_le_bytes());
                debug_assert_eq!(out.len() - start, GROUP_HEADER_SIZE);
                for c in children {
                    c.write(out);
                }
                let size = (out.len() - start) as u32;
                out[start + 4..start + 8].copy_from_slice(&size.to_le_bytes());
            }
        }
    }
}

/// A plugin file: its masters, author and description, and top level groups.
/// `next_object_id` is the first object id (low 24 bits) not yet used.
pub fn write_plugin(
    flags: u32,
    masters: &[&str],
    author: &str,
    description: &str,
    next_object_id: u32,
    groups: &[Node],
) -> Vec<u8> {
    let count: u32 = groups.iter().map(Node::count).sum();
    let mut hedr = HEADER_VERSION.to_le_bytes().to_vec();
    hedr.extend_from_slice(&count.to_le_bytes());
    hedr.extend_from_slice(&next_object_id.to_le_bytes());
    let mut header = RecordBuilder::new(b"TES4", FormId(0))
        .flags(flags)
        .field(b"HEDR", &hedr)
        .zstring(b"CNAM", author)
        .zstring(b"SNAM", description);
    for m in masters {
        header = header
            .zstring(b"MAST", m)
            .field(b"DATA", &0u64.to_le_bytes());
    }
    let mut out = Vec::new();
    header.write(&mut out);
    debug_assert!(out.len() >= RECORD_HEADER_SIZE);
    for g in groups {
        g.write(&mut out);
    }
    out
}
