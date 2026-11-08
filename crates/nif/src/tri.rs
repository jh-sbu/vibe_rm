//! FaceGen `.tri` morph files (`FRTRI003`), as head parts name them (HDPT
//! `NAM0` / `NAM1`): the part's vertices, then named morphs, each a scale
//! and a signed 16-bit offset per vertex. Layout in
//! `research/audio/lip-format.md`; layouts with other header fields set
//! (nine vanilla files, none of them chargen or race morphs) are refused.

use glam::Vec3;

use crate::{Error, Result};

/// A morph: one offset per vertex of the file.
#[derive(Debug, Clone, PartialEq)]
pub struct Morph {
    pub name: String,
    pub deltas: Vec<Vec3>,
}

/// A `.tri` file's base vertices and morphs.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Tri {
    pub positions: Vec<Vec3>,
    pub morphs: Vec<Morph>,
}

impl Tri {
    /// The morph of that name (case-insensitively: the assets vary).
    pub fn morph(&self, name: &str) -> Option<&Morph> {
        self.morphs
            .iter()
            .find(|m| m.name.eq_ignore_ascii_case(name))
    }

    pub fn parse(d: &[u8]) -> Result<Tri> {
        let word = |at: usize| -> Result<u32> {
            d.get(at..at + 4)
                .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                .ok_or(Error::Eof(at))
        };
        if d.get(..8) != Some(b"FRTRI003") || d.len() < 64 {
            return Err(Error::Invalid("not a FRTRI003 file".into()));
        }
        let n = word(8)? as usize;
        let tris = word(12)? as usize;
        let quads = word(16)? as usize;
        let uvs = word(28)? as usize;
        let flags = word(32)?;
        let morphs = word(36)? as usize;
        for at in [20, 24, 40, 44, 48, 52, 56, 60] {
            if word(at)? != 0 {
                return Err(Error::Invalid(format!("TRI header field {at} set")));
            }
        }
        if flags > 1 {
            return Err(Error::Invalid(format!("TRI flags {flags}")));
        }
        let overflow = || Error::Invalid("TRI sizes overflow".into());
        let topology = tris
            .checked_mul(12)
            .zip(quads.checked_mul(16))
            .and_then(|(t, q)| t.checked_add(q))
            .ok_or_else(overflow)?;
        let vertex_bytes = n.checked_mul(12).ok_or_else(overflow)?;
        let positions_end = 64usize.checked_add(vertex_bytes).ok_or_else(overflow)?;
        let positions = d
            .get(64..positions_end)
            .ok_or(Error::Eof(64))?
            .chunks_exact(12)
            .map(|c| {
                let f = |i: usize| f32::from_le_bytes(c[i..i + 4].try_into().unwrap());
                Vec3::new(f(0), f(4), f(8))
            })
            .collect();
        let mut at = uvs
            .checked_mul(8)
            .and_then(|u| positions_end.checked_add(topology)?.checked_add(u))
            .and_then(|v| v.checked_add(if flags & 1 != 0 { topology } else { 0 }))
            .ok_or_else(overflow)?;
        if at > d.len() {
            return Err(Error::Eof(d.len()));
        }
        let mut out = Vec::with_capacity(morphs.min(1024));
        for _ in 0..morphs {
            let len = word(at)? as usize;
            at += 4;
            if !(1..=1024).contains(&len) {
                return Err(Error::Invalid(format!("TRI morph name length {len}")));
            }
            let raw = d.get(at..at + len).ok_or(Error::Eof(at))?;
            at += len;
            let name = String::from_utf8_lossy(raw.strip_suffix(&[0]).unwrap_or(raw)).into_owned();
            let scale = f32::from_bits(word(at)?);
            at += 4;
            let end = n
                .checked_mul(6)
                .and_then(|b| at.checked_add(b))
                .ok_or_else(overflow)?;
            let deltas = d
                .get(at..end)
                .ok_or(Error::Eof(at))?
                .chunks_exact(6)
                .map(|c| {
                    let i = |o: usize| i16::from_le_bytes([c[o], c[o + 1]]) as f32;
                    Vec3::new(i(0), i(2), i(4)) * scale
                })
                .collect();
            at = end;
            out.push(Morph { name, deltas });
        }
        if at != d.len() {
            return Err(Error::Invalid(format!(
                "{} trailing TRI bytes",
                d.len() - at
            )));
        }
        Ok(Tri {
            positions,
            morphs: out,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One vertex, no faces, one morph `Aah` of scale 0.5.
    pub(crate) fn one_morph() -> Vec<u8> {
        let mut d = vec![0; 64];
        d[..8].copy_from_slice(b"FRTRI003");
        d[8..12].copy_from_slice(&1u32.to_le_bytes());
        d[36..40].copy_from_slice(&1u32.to_le_bytes());
        for f in [1.0f32, 2.0, 3.0] {
            d.extend(f.to_le_bytes());
        }
        d.extend(4u32.to_le_bytes());
        d.extend(b"Aah\0");
        d.extend(0.5f32.to_le_bytes());
        for value in [-2i16, 4, 0] {
            d.extend(value.to_le_bytes());
        }
        d
    }

    #[test]
    fn reads_positions_and_scaled_morphs() {
        let t = Tri::parse(&one_morph()).unwrap();
        assert_eq!(t.positions, vec![Vec3::new(1.0, 2.0, 3.0)]);
        assert_eq!(
            t.morph("aah").unwrap().deltas,
            vec![Vec3::new(-1.0, 2.0, 0.0)]
        );
    }

    #[test]
    fn refuses_truncated_trailing_and_unknown_layouts() {
        let d = one_morph();
        assert!(Tri::parse(&d[..d.len() - 1]).is_err());
        let mut longer = d.clone();
        longer.push(0);
        assert!(Tri::parse(&longer).is_err());
        let mut stat = d;
        stat[40] = 1;
        assert!(Tri::parse(&stat).is_err());
    }
}
