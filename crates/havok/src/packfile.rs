use std::collections::HashMap;

use crate::{Error, Result};

/// A loaded packfile: the data section plus pointer fixups and object classes.
pub struct Packfile {
    pub version: String,
    /// Bytes of the `__data__` section.
    pub data: Vec<u8>,
    /// Pointer slot offset -> target offset (both within the data section).
    pointers: HashMap<u32, u32>,
    /// Objects: offset in data section -> class name.
    pub objects: Vec<Object>,
}

#[derive(Debug, Clone)]
pub struct Object {
    pub offset: u32,
    pub class: String,
}

fn u32_at(d: &[u8], o: usize) -> Result<u32> {
    d.get(o..o + 4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .ok_or_else(|| Error::Corrupt(format!("read past end at {o:#x}")))
}

fn cstr(d: &[u8], o: usize) -> String {
    let end = d[o..]
        .iter()
        .position(|&c| c == 0)
        .map(|p| o + p)
        .unwrap_or(d.len());
    String::from_utf8_lossy(&d[o..end]).into_owned()
}

struct Section {
    name: String,
    start: usize,
    local: usize,
    global: usize,
    virt: usize,
    exports: usize,
    end: usize,
}

impl Packfile {
    pub fn parse(d: &[u8]) -> Result<Packfile> {
        if d.len() < 0x40 || u32_at(d, 0)? != 0x57E0_E057 || u32_at(d, 4)? != 0x10C0_C010 {
            return Err(Error::BadMagic);
        }
        let file_version = u32_at(d, 12)?;
        let ptr_size = d[16];
        let little = d[17];
        if ptr_size != 8 || little != 1 {
            return Err(Error::Unsupported(format!(
                "pointer size {ptr_size}, little endian {little}"
            )));
        }
        let num_sections = u32_at(d, 20)? as usize;
        let version = cstr(d, 40);
        let mut header_size = 0x40;
        if file_version >= 11 {
            // Predicate array follows the header.
            let pad = u16::from_le_bytes([d[0x3E], d[0x3F]]) as usize;
            header_size += pad;
        }
        let mut sections = Vec::new();
        for i in 0..num_sections {
            let o = header_size + i * 0x30;
            let name = cstr(d, o);
            let start = u32_at(d, o + 20)? as usize;
            let rel =
                |k: usize| -> Result<usize> { Ok(start + u32_at(d, o + 24 + k * 4)? as usize) };
            sections.push(Section {
                name,
                start,
                local: rel(0)?,
                global: rel(1)?,
                virt: rel(2)?,
                exports: rel(3)?,
                end: rel(6)?,
            });
            let _ = sections.last().map(|s| s.exports);
        }
        let classnames = sections
            .iter()
            .find(|s| s.name == "__classnames__")
            .ok_or_else(|| Error::Corrupt("no classnames".into()))?;
        let data_idx = sections
            .iter()
            .position(|s| s.name == "__data__")
            .ok_or_else(|| Error::Corrupt("no data section".into()))?;
        let ds = &sections[data_idx];
        let data = d
            .get(ds.start..ds.local)
            .ok_or_else(|| Error::Corrupt("data section out of range".into()))?
            .to_vec();

        let mut pointers = HashMap::new();
        // Local fixups: (src, dst) pairs.
        let mut o = ds.local;
        while o + 8 <= ds.global {
            let src = u32_at(d, o)?;
            if src == u32::MAX {
                break;
            }
            pointers.insert(src, u32_at(d, o + 4)?);
            o += 8;
        }
        // Global fixups: (src, section, dst). We only resolve pointers into the data section.
        let mut o = ds.global;
        while o + 12 <= ds.virt {
            let src = u32_at(d, o)?;
            if src == u32::MAX {
                break;
            }
            let sec = u32_at(d, o + 4)? as usize;
            let dst = u32_at(d, o + 8)?;
            if sec == data_idx {
                pointers.insert(src, dst);
            }
            o += 12;
        }
        // Virtual fixups: (object offset, classname section, classname offset).
        let mut objects = Vec::new();
        let mut o = ds.virt;
        while o + 12 <= ds.exports.min(ds.end) {
            let src = u32_at(d, o)?;
            if src == u32::MAX {
                break;
            }
            let name_off = u32_at(d, o + 8)? as usize;
            let class = cstr(d, classnames.start + name_off);
            objects.push(Object { offset: src, class });
            o += 12;
        }
        Ok(Packfile {
            version,
            data,
            pointers,
            objects,
        })
    }

    pub fn ptr(&self, slot: u32) -> Option<u32> {
        self.pointers.get(&slot).copied()
    }

    pub fn u8(&self, o: u32) -> u8 {
        self.data.get(o as usize).copied().unwrap_or(0)
    }
    pub fn u16(&self, o: u32) -> u16 {
        let o = o as usize;
        self.data
            .get(o..o + 2)
            .map(|b| u16::from_le_bytes(b.try_into().unwrap()))
            .unwrap_or(0)
    }
    pub fn i16(&self, o: u32) -> i16 {
        self.u16(o) as i16
    }
    pub fn u32(&self, o: u32) -> u32 {
        let o = o as usize;
        self.data
            .get(o..o + 4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .unwrap_or(0)
    }
    pub fn i32(&self, o: u32) -> i32 {
        self.u32(o) as i32
    }
    pub fn f32(&self, o: u32) -> f32 {
        f32::from_bits(self.u32(o))
    }

    /// hkArray at `o`: (data offset, size).
    pub fn array(&self, o: u32) -> (Option<u32>, usize) {
        (self.ptr(o), self.i32(o + 8).max(0) as usize)
    }

    /// hkStringPtr / char* at `o`.
    pub fn string(&self, o: u32) -> Option<String> {
        let p = self.ptr(o)? as usize;
        Some(cstr(&self.data, p))
    }

    pub fn object_class(&self, offset: u32) -> Option<&str> {
        self.objects
            .iter()
            .find(|x| x.offset == offset)
            .map(|x| x.class.as_str())
    }

    pub fn objects_of<'a>(&'a self, class: &'a str) -> impl Iterator<Item = u32> + 'a {
        self.objects
            .iter()
            .filter(move |o| o.class == class)
            .map(|o| o.offset)
    }
}
