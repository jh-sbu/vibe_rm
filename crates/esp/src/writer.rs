//! Writing small TES4-format plugins (test mods): records built field by
//! field, nested in groups, behind a `TES4` header. Records are written
//! uncompressed with the Special Edition form version. [`vmad`] attaches
//! scripts.

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
    /// One uncompressed record, including its TES4-format header.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        self.write(&mut bytes);
        bytes
    }

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
    /// A runtime name derived from decoded plugin text, preserving Windows-1252.
    pub fn windows1252_zstring(self, tag: &[u8; 4], s: &str) -> Self {
        let (bytes, _, unmappable) = encoding_rs::WINDOWS_1252.encode(s);
        assert!(
            !unmappable,
            "runtime name cannot be encoded as Windows-1252"
        );
        let mut bytes = bytes.into_owned();
        bytes.push(0);
        self.field(tag, &bytes)
    }

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

/// The VMAD version and object format the Special Edition Creation Kit
/// writes: format 2 objects are (unused u16, alias i16, form id u32).
pub const VMAD_VERSION: i16 = 5;
pub const VMAD_OBJECT_FORMAT: i16 = 2;

/// A script property's value in a VMAD.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyValue {
    /// A form, or none (the null form id).
    Object(FormId),
    /// A quest's alias, by its id.
    Alias {
        quest: FormId,
        alias: i16,
    },
    String(String),
    Int(i32),
    Float(f32),
    Bool(bool),
    /// Elements of one of the kinds above.
    Array(Vec<PropertyValue>),
}

impl PropertyValue {
    fn kind(&self) -> u8 {
        match self {
            PropertyValue::Object(_) | PropertyValue::Alias { .. } => 1,
            PropertyValue::String(_) => 2,
            PropertyValue::Int(_) => 3,
            PropertyValue::Float(_) => 4,
            PropertyValue::Bool(_) => 5,
            // An empty array's element kind is moot: say objects.
            PropertyValue::Array(v) => 10 + v.first().map_or(1, PropertyValue::kind),
        }
    }

    fn write(&self, out: &mut Vec<u8>) {
        match self {
            PropertyValue::Object(f) => {
                out.extend_from_slice(&0u16.to_le_bytes());
                out.extend_from_slice(&(-1i16).to_le_bytes());
                out.extend_from_slice(&f.0.to_le_bytes());
            }
            PropertyValue::Alias { quest, alias } => {
                out.extend_from_slice(&0u16.to_le_bytes());
                out.extend_from_slice(&alias.to_le_bytes());
                out.extend_from_slice(&quest.0.to_le_bytes());
            }
            PropertyValue::String(s) => wstring(out, s),
            PropertyValue::Int(v) => out.extend_from_slice(&v.to_le_bytes()),
            PropertyValue::Float(v) => out.extend_from_slice(&v.to_le_bytes()),
            PropertyValue::Bool(v) => out.push((*v).into()),
            PropertyValue::Array(v) => {
                let kind = self.kind();
                assert!(
                    v.iter().all(|e| e.kind() == kind - 10),
                    "array elements of mixed or nested kinds"
                );
                out.extend_from_slice(&(v.len() as u32).to_le_bytes());
                for e in v {
                    e.write(out);
                }
            }
        }
    }
}

/// A script attached to a form, and the properties it sets.
#[derive(Debug, Clone, PartialEq)]
pub struct AttachedScript {
    pub name: String,
    pub properties: Vec<(String, PropertyValue)>,
}

/// A string of u16 length then bytes, no terminator (test mods keep to
/// ASCII).
fn wstring(out: &mut Vec<u8>, s: &str) {
    assert!(s.is_ascii(), "{s:?}: not ASCII");
    let len = u16::try_from(s.len()).expect("string over 64 KiB");
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(s.as_bytes());
}

/// A VMAD subrecord attaching `scripts`, with no fragments: as the
/// Creation Kit writes it for any record, quests included (their fragment
/// section is left out when there are no fragments, as `DialogueGeneric`'s).
pub fn vmad(scripts: &[AttachedScript]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&VMAD_VERSION.to_le_bytes());
    out.extend_from_slice(&VMAD_OBJECT_FORMAT.to_le_bytes());
    out.extend_from_slice(&(scripts.len() as u16).to_le_bytes());
    for s in scripts {
        wstring(&mut out, &s.name);
        out.push(0); // status: local
        out.extend_from_slice(&(s.properties.len() as u16).to_le_bytes());
        for (name, v) in &s.properties {
            wstring(&mut out, name);
            out.push(v.kind());
            out.push(1); // status: edited
            v.write(&mut out);
        }
    }
    out
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vmad_is_written_as_the_creation_kit_writes_it() {
        // DialogueGeneric's VMAD in Update.esm, with its two properties.
        let v = vmad(&[AttachedScript {
            name: "DialogueGenericScript".into(),
            properties: vec![
                ("Gold".into(), PropertyValue::Object(FormId(0xF))),
                (
                    "RoomRentalCost".into(),
                    PropertyValue::Object(FormId(0x9CC98)),
                ),
            ],
        }]);
        let mut expected = vec![5, 0, 2, 0, 1, 0, 0x15, 0];
        expected.extend_from_slice(b"DialogueGenericScript");
        expected.extend_from_slice(&[0, 2, 0, 4, 0]);
        expected.extend_from_slice(b"Gold");
        expected.extend_from_slice(&[1, 1, 0, 0, 0xFF, 0xFF, 0x0F, 0, 0, 0, 0x0E, 0]);
        expected.extend_from_slice(b"RoomRentalCost");
        expected.extend_from_slice(&[1, 1, 0, 0, 0xFF, 0xFF, 0x98, 0xCC, 0x09, 0]);
        assert_eq!(v, expected);
    }

    #[test]
    fn vmad_arrays_and_scalars() {
        let v = vmad(&[AttachedScript {
            name: "S".into(),
            properties: vec![
                (
                    "A".into(),
                    PropertyValue::Array(vec![PropertyValue::Int(1), PropertyValue::Int(-2)]),
                ),
                ("B".into(), PropertyValue::Bool(true)),
            ],
        }]);
        let mut expected = vec![5, 0, 2, 0, 1, 0, 1, 0, b'S', 0, 2, 0];
        expected.extend_from_slice(&[1, 0, b'A', 13, 1, 2, 0, 0, 0, 1, 0, 0, 0]);
        expected.extend_from_slice(&(-2i32).to_le_bytes());
        expected.extend_from_slice(&[1, 0, b'B', 5, 1, 1]);
        assert_eq!(v, expected);
    }
}
