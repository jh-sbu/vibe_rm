//! NIF (NetImmerse/Gamebryo) model reader targeting Skyrim LE (BS version 83)
//! and Skyrim SE (BS version 100).
//!
//! Every block in a v20.2.0.7 file is preceded in the header by its size, so
//! block types that aren't understood (or are only partially parsed) are
//! skipped safely. The parsed representation lives in [`blocks`].

pub mod blocks;
pub mod collision;
mod reader;

use std::collections::HashMap;

pub use blocks::*;
pub use collision::{CollisionObject, HAVOK_SCALE, MotionSystem, RigidBody, Shape};
use reader::Reader;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not a NIF file")]
    BadMagic,
    #[error("unsupported NIF version {0:#010x}")]
    UnsupportedVersion(u32),
    #[error("unexpected end of data at offset {0}")]
    Eof(usize),
    #[error("invalid data: {0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone)]
pub struct Header {
    pub version: u32,
    pub user_version: u32,
    pub bs_version: u32,
    pub block_types: Vec<String>,
    pub block_type_index: Vec<u16>,
    pub block_sizes: Vec<u32>,
    /// Absolute file offset of each block.
    pub block_offsets: Vec<usize>,
    pub strings: Vec<String>,
}

pub struct Nif {
    pub header: Header,
    pub blocks: Vec<Block>,
    pub roots: Vec<u32>,
}

/// Outcome of parsing a block, used by verification tooling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockParse {
    /// Parsed and consumed exactly the declared size.
    Exact,
    /// Type not (fully) handled; skipped using the declared size.
    Skipped,
    /// Parsed but the consumed size didn't match the declared size.
    SizeMismatch { consumed: usize, declared: usize },
    Failed,
}

impl Nif {
    pub fn parse(data: &[u8]) -> Result<Nif> {
        Self::parse_with_report(data).map(|(n, _)| n)
    }

    pub fn parse_with_report(data: &[u8]) -> Result<(Nif, Vec<BlockParse>)> {
        let mut r = Reader::new(data);
        let line = r.line()?;
        if !line.starts_with("Gamebryo File Format") && !line.starts_with("NetImmerse File Format") {
            return Err(Error::BadMagic);
        }
        let version = r.u32()?;
        if version != 0x1402_0007 {
            return Err(Error::UnsupportedVersion(version));
        }
        let _endian = r.u8()?;
        let user_version = r.u32()?;
        let num_blocks = r.u32()? as usize;
        let mut bs_version = 0;
        if user_version >= 3 {
            bs_version = r.u32()?;
            r.short_string()?; // author
            if bs_version > 130 {
                r.u32()?;
            }
            r.short_string()?; // process script
            r.short_string()?; // export script
            if bs_version >= 103 {
                r.short_string()?; // max filepath
            }
        }
        let num_types = r.u16()? as usize;
        let mut block_types = Vec::with_capacity(num_types);
        for _ in 0..num_types {
            block_types.push(r.sized_string()?);
        }
        let mut block_type_index = Vec::with_capacity(num_blocks);
        for _ in 0..num_blocks {
            block_type_index.push(r.u16()? & 0x7FFF);
        }
        let mut block_sizes = Vec::with_capacity(num_blocks);
        for _ in 0..num_blocks {
            block_sizes.push(r.u32()?);
        }
        let num_strings = r.u32()? as usize;
        let _max_len = r.u32()?;
        let mut strings = Vec::with_capacity(num_strings);
        for _ in 0..num_strings {
            strings.push(r.sized_string()?);
        }
        let num_groups = r.u32()?;
        for _ in 0..num_groups {
            r.u32()?;
        }
        let header = Header {
            version,
            user_version,
            bs_version,
            block_types,
            block_type_index,
            block_offsets: Vec::with_capacity(block_sizes.len()),
            block_sizes,
            strings,
        };
        let mut header = header;
        {
            let mut off = r.pos();
            for &sz in &header.block_sizes {
                header.block_offsets.push(off);
                off += sz as usize;
            }
        }
        r.bs_version = bs_version;
        r.strings = &header.strings;

        let mut blocks = Vec::with_capacity(num_blocks);
        let mut report = Vec::with_capacity(num_blocks);
        for i in 0..num_blocks {
            let ty = header
                .block_types
                .get(header.block_type_index[i] as usize)
                .ok_or_else(|| Error::Invalid("block type index out of range".into()))?;
            let start = r.pos();
            let size = header.block_sizes[i] as usize;
            let end = start + size;
            if end > data.len() {
                return Err(Error::Eof(end));
            }
            let mut sub = Reader { data: &data[..end], pos: start, bs_version, strings: &header.strings };
            let (block, status) = match blocks::parse_block(ty, &mut sub) {
                Ok(Some(b)) => {
                    let consumed = sub.pos() - start;
                    let st = if consumed == size {
                        BlockParse::Exact
                    } else {
                        BlockParse::SizeMismatch { consumed, declared: size }
                    };
                    (b, st)
                }
                Ok(None) => (Block::Unknown(ty.clone()), BlockParse::Skipped),
                Err(e) => {
                    log::debug!("block {i} ({ty}) failed to parse: {e}");
                    (Block::Unknown(ty.clone()), BlockParse::Failed)
                }
            };
            blocks.push(block);
            report.push(status);
            r.set_pos(end);
        }
        let mut roots = Vec::new();
        if let Ok(n) = r.u32() {
            for _ in 0..n {
                if let Ok(v) = r.i32()
                    && v >= 0
                {
                    roots.push(v as u32);
                }
            }
        }
        if roots.is_empty() && !blocks.is_empty() {
            roots.push(0);
        }
        Ok((Nif { header, blocks, roots }, report))
    }

    pub fn block_type_name(&self, i: usize) -> &str {
        &self.header.block_types[self.header.block_type_index[i] as usize]
    }

    pub fn get(&self, r: Ref) -> Option<&Block> {
        r.index().and_then(|i| self.blocks.get(i))
    }

    pub fn string(&self, s: StringRef) -> Option<&str> {
        s.0.and_then(|i| self.header.strings.get(i as usize)).map(String::as_str)
    }

    /// Count of block types by name, for diagnostics.
    pub fn type_histogram(&self) -> HashMap<&str, usize> {
        let mut h = HashMap::new();
        for i in 0..self.blocks.len() {
            *h.entry(self.block_type_name(i)).or_default() += 1;
        }
        h
    }
}
