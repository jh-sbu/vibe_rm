//! Localised string tables (`.STRINGS`, `.DLSTRINGS`, `.ILSTRINGS`).

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringsKind {
    /// Null-terminated strings.
    Strings,
    /// Length-prefixed strings (`.dlstrings`, `.ilstrings`).
    LengthPrefixed,
}

impl StringsKind {
    pub fn from_extension(ext: &str) -> Self {
        if ext.eq_ignore_ascii_case("strings") {
            StringsKind::Strings
        } else {
            StringsKind::LengthPrefixed
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct StringTable {
    pub entries: HashMap<u32, String>,
}

impl StringTable {
    pub fn parse(data: &[u8], kind: StringsKind) -> Option<Self> {
        let rd = |o: usize| -> Option<u32> {
            data.get(o..o + 4)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        };
        let count = rd(0)? as usize;
        let _size = rd(4)?;
        let base = 8 + count * 8;
        let mut entries = HashMap::with_capacity(count);
        for i in 0..count {
            let id = rd(8 + i * 8)?;
            let off = base + rd(12 + i * 8)? as usize;
            let s = match kind {
                StringsKind::Strings => {
                    let rest = data.get(off..)?;
                    crate::decode_zstring(rest)
                }
                StringsKind::LengthPrefixed => {
                    let len = rd(off)? as usize;
                    crate::decode_zstring(data.get(off + 4..off + 4 + len)?)
                }
            };
            entries.insert(id, s);
        }
        Some(StringTable { entries })
    }
}
