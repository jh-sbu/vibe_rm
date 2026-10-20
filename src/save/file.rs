//! The save file: a magic, a format version and tagged sections, each with
//! its length and a CRC-32, so a truncated or damaged file is refused rather
//! than half-applied and sections can be added without breaking old saves
//! (unknown tags are kept and skipped). Layout, little-endian:
//!
//! ```text
//! "VRMSAVE\0"  version u32  sections u32
//! per section: tag [u8; 4]  length u32  crc32 u32  payload
//! ```

use super::codec::{Corrupt, Result};

const MAGIC: &[u8; 8] = b"VRMSAVE\0";
/// Bumped when a section's layout changes in a way readers can't skip.
pub const VERSION: u32 = 1;

pub type Tag = [u8; 4];

#[derive(Debug, Default, Clone, PartialEq)]
pub struct SaveFile {
    pub sections: Vec<(Tag, Vec<u8>)>,
}

/// CRC-32 (IEEE, reflected), bitwise: sections are small.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 & (crc & 1).wrapping_neg());
        }
    }
    !crc
}

impl SaveFile {
    pub fn add(&mut self, tag: &Tag, payload: Vec<u8>) {
        self.sections.push((*tag, payload));
    }

    pub fn section(&self, tag: &Tag) -> Option<&[u8]> {
        self.sections
            .iter()
            .find(|(t, _)| t == tag)
            .map(|(_, p)| p.as_slice())
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&(self.sections.len() as u32).to_le_bytes());
        for (tag, payload) in &self.sections {
            out.extend_from_slice(tag);
            out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            out.extend_from_slice(&crc32(payload).to_le_bytes());
            out.extend_from_slice(payload);
        }
        out
    }

    pub fn from_bytes(data: &[u8]) -> Result<SaveFile> {
        let bad = |what: &str| Corrupt(what.to_string());
        if data.get(..8) != Some(MAGIC) {
            return Err(bad("not a save file"));
        }
        let word = |at: usize| {
            data.get(at..at + 4)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                .ok_or_else(|| bad("cut short"))
        };
        let version = word(8)?;
        if version != VERSION {
            return Err(Corrupt(format!(
                "save version {version}, this game reads {VERSION}"
            )));
        }
        let count = word(12)? as usize;
        let mut at = 16;
        let mut file = SaveFile::default();
        for _ in 0..count {
            let tag: Tag = data
                .get(at..at + 4)
                .ok_or_else(|| bad("cut short"))?
                .try_into()
                .unwrap();
            let len = word(at + 4)? as usize;
            let crc = word(at + 8)?;
            at += 12;
            let payload = data
                .get(at..at.checked_add(len).ok_or_else(|| bad("cut short"))?)
                .ok_or_else(|| bad("cut short"))?;
            if crc32(payload) != crc {
                return Err(Corrupt(format!(
                    "section {} is damaged",
                    String::from_utf8_lossy(&tag)
                )));
            }
            file.sections.push((tag, payload.to_vec()));
            at += len;
        }
        if at != data.len() {
            return Err(bad("data after the last section"));
        }
        Ok(file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_matches_the_standard_check_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
    }

    fn sample() -> SaveFile {
        let mut f = SaveFile::default();
        f.add(b"HEAD", vec![1, 2, 3]);
        f.add(b"EMPT", vec![]);
        f.add(b"NEWR", vec![9; 40]);
        f
    }

    #[test]
    fn sections_round_trip_in_order() {
        let f = sample();
        let back = SaveFile::from_bytes(&f.to_bytes()).unwrap();
        assert_eq!(back, f);
        assert_eq!(back.section(b"HEAD"), Some(&[1, 2, 3][..]));
        assert_eq!(back.section(b"NONE"), None);
    }

    #[test]
    fn damage_truncation_and_foreign_files_are_refused() {
        let bytes = sample().to_bytes();
        // Any single flipped payload byte fails its section's CRC.
        let mut damaged = bytes.clone();
        let last = damaged.len() - 1;
        damaged[last] ^= 1;
        assert!(SaveFile::from_bytes(&damaged).is_err());
        // Every truncation is refused, none panics.
        for n in 0..bytes.len() {
            assert!(SaveFile::from_bytes(&bytes[..n]).is_err(), "{n} bytes");
        }
        // Trailing bytes, other files, other versions.
        let mut longer = bytes.clone();
        longer.push(0);
        assert!(SaveFile::from_bytes(&longer).is_err());
        assert!(SaveFile::from_bytes(b"TES4 not ours at all").is_err());
        let mut future = bytes;
        future[8] = 99;
        assert!(
            SaveFile::from_bytes(&future)
                .unwrap_err()
                .0
                .contains("version 99")
        );
    }
}
