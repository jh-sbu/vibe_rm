use std::borrow::Cow;
use std::io::Read;

use crate::{Error, FormId, Result, Tag, le_u16, le_u32};

pub const RECORD_HEADER_SIZE: usize = 24;

pub mod record_flags {
    pub const MASTER: u32 = 0x1;
    pub const DELETED: u32 = 0x20;
    pub const LOCALIZED: u32 = 0x80;
    pub const LIGHT: u32 = 0x200;
    pub const PERSISTENT: u32 = 0x400;
    pub const INITIALLY_DISABLED: u32 = 0x800;
    pub const COMPRESSED: u32 = 0x0004_0000;
}

#[derive(Debug, Clone, Copy)]
pub struct RecordHeader {
    pub tag: Tag,
    pub data_size: u32,
    pub flags: u32,
    pub form_id: FormId,
    pub version: u16,
}

impl RecordHeader {
    pub fn parse(b: &[u8]) -> Self {
        RecordHeader {
            tag: Tag(b[0..4].try_into().unwrap()),
            data_size: le_u32(b, 4),
            flags: le_u32(b, 8),
            form_id: FormId(le_u32(b, 12)),
            version: le_u16(b, 20),
        }
    }
    pub fn is_compressed(&self) -> bool {
        self.flags & record_flags::COMPRESSED != 0
    }
    pub fn is_deleted(&self) -> bool {
        self.flags & record_flags::DELETED != 0
    }
}

/// A record with its (decompressed) field data. The form id is whatever the
/// source provided (local when read from a [`crate::Plugin`], global when
/// fetched through a [`crate::LoadOrder`]).
#[derive(Debug, Clone)]
pub struct Record<'a> {
    pub header: RecordHeader,
    pub data: Cow<'a, [u8]>,
}

impl<'a> Record<'a> {
    /// Build from the raw record bytes (header + payload) inside a plugin.
    pub fn from_raw(raw: &'a [u8]) -> Result<Self> {
        let header = RecordHeader::parse(raw);
        let payload = &raw[RECORD_HEADER_SIZE..RECORD_HEADER_SIZE + header.data_size as usize];
        let data = if header.is_compressed() {
            if payload.len() < 4 {
                return Err(Error::Corrupt("compressed record too small".into()));
            }
            let size = le_u32(payload, 0) as usize;
            let mut out = Vec::with_capacity(size);
            flate2::read::ZlibDecoder::new(&payload[4..])
                .read_to_end(&mut out)
                .map_err(|e| Error::Corrupt(format!("zlib: {e}")))?;
            Cow::Owned(out)
        } else {
            Cow::Borrowed(payload)
        };
        Ok(Record { header, data })
    }

    pub fn tag(&self) -> Tag {
        self.header.tag
    }
    pub fn form_id(&self) -> FormId {
        self.header.form_id
    }
    pub fn flags(&self) -> u32 {
        self.header.flags
    }
    pub fn subrecords(&self) -> SubRecords<'_> {
        SubRecords { data: &self.data, pos: 0, next_size: None }
    }
    /// First subrecord with the given tag.
    pub fn get(&self, tag: &[u8; 4]) -> Option<&[u8]> {
        self.subrecords().find(|s| s.tag.0 == *tag).map(|s| s.data)
    }
    pub fn editor_id(&self) -> Option<String> {
        self.get(b"EDID").map(crate::decode_zstring)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SubRecord<'a> {
    pub tag: Tag,
    pub data: &'a [u8],
}

impl<'a> SubRecord<'a> {
    pub fn u8(&self, o: usize) -> u8 {
        self.data.get(o).copied().unwrap_or(0)
    }
    pub fn u16(&self, o: usize) -> u16 {
        if o + 2 <= self.data.len() { le_u16(self.data, o) } else { 0 }
    }
    pub fn u32(&self, o: usize) -> u32 {
        if o + 4 <= self.data.len() { le_u32(self.data, o) } else { 0 }
    }
    pub fn i32(&self, o: usize) -> i32 {
        self.u32(o) as i32
    }
    pub fn f32(&self, o: usize) -> f32 {
        f32::from_bits(self.u32(o))
    }
    pub fn form_id(&self, o: usize) -> FormId {
        FormId(self.u32(o))
    }
    pub fn zstring(&self) -> String {
        crate::decode_zstring(self.data)
    }
}

pub struct SubRecords<'a> {
    data: &'a [u8],
    pos: usize,
    next_size: Option<usize>,
}

impl<'a> Iterator for SubRecords<'a> {
    type Item = SubRecord<'a>;
    fn next(&mut self) -> Option<SubRecord<'a>> {
        loop {
            if self.pos + 6 > self.data.len() {
                return None;
            }
            let tag = Tag(self.data[self.pos..self.pos + 4].try_into().unwrap());
            let mut size = le_u16(self.data, self.pos + 4) as usize;
            self.pos += 6;
            if let Some(s) = self.next_size.take() {
                size = s;
            }
            if self.pos + size > self.data.len() {
                log::warn!("subrecord {tag} overruns record");
                return None;
            }
            let data = &self.data[self.pos..self.pos + size];
            self.pos += size;
            if tag.0 == *b"XXXX" && size == 4 {
                self.next_size = Some(le_u32(data, 0) as usize);
                continue;
            }
            return Some(SubRecord { tag, data });
        }
    }
}
