//! Reader for Bethesda Softworks Archive (BSA) files.
//!
//! Supports version 103 (Oblivion), 104 (Fallout 3 / New Vegas / Skyrim LE) and
//! 105 (Skyrim Special Edition). Archives are memory mapped; file contents are
//! decompressed on demand (zlib for <=104, LZ4 frame for 105).

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use memmap2::Mmap;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("not a BSA archive")]
    BadMagic,
    #[error("unsupported BSA version {0}")]
    UnsupportedVersion(u32),
    #[error("corrupt archive: {0}")]
    Corrupt(&'static str),
    #[error("decompression failed: {0}")]
    Decompress(String),
}

pub type Result<T> = std::result::Result<T, Error>;

pub mod flags {
    pub const INCLUDE_DIR_NAMES: u32 = 0x1;
    pub const INCLUDE_FILE_NAMES: u32 = 0x2;
    pub const COMPRESSED: u32 = 0x4;
    pub const EMBED_FILE_NAMES: u32 = 0x100;
}

/// Normalise a virtual path: lowercase, forward slashes, no leading slash.
pub fn normalize_path(p: &str) -> String {
    let mut s: String = p
        .chars()
        .map(|c| if c == '\\' { '/' } else { c.to_ascii_lowercase() })
        .collect();
    while s.starts_with('/') {
        s.remove(0);
    }
    s
}

#[derive(Debug, Clone, Copy)]
struct Entry {
    offset: u64,
    size: u32,
    compressed: bool,
}

pub struct Archive {
    path: PathBuf,
    map: Mmap,
    version: u32,
    archive_flags: u32,
    files: HashMap<String, Entry>,
}

struct Cursor<'a> {
    d: &'a [u8],
    p: usize,
}

impl<'a> Cursor<'a> {
    fn need(&self, n: usize) -> Result<()> {
        if self.p + n > self.d.len() {
            Err(Error::Corrupt("unexpected end of data"))
        } else {
            Ok(())
        }
    }
    fn u8(&mut self) -> Result<u8> {
        self.need(1)?;
        let v = self.d[self.p];
        self.p += 1;
        Ok(v)
    }
    fn u32(&mut self) -> Result<u32> {
        self.need(4)?;
        let v = u32::from_le_bytes(self.d[self.p..self.p + 4].try_into().unwrap());
        self.p += 4;
        Ok(v)
    }
    fn u64(&mut self) -> Result<u64> {
        self.need(8)?;
        let v = u64::from_le_bytes(self.d[self.p..self.p + 8].try_into().unwrap());
        self.p += 8;
        Ok(v)
    }
    fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        self.need(n)?;
        let v = &self.d[self.p..self.p + n];
        self.p += n;
        Ok(v)
    }
    fn cstr(&mut self) -> Result<&'a [u8]> {
        let start = self.p;
        while self.p < self.d.len() && self.d[self.p] != 0 {
            self.p += 1;
        }
        if self.p >= self.d.len() {
            return Err(Error::Corrupt("unterminated string"));
        }
        let s = &self.d[start..self.p];
        self.p += 1;
        Ok(s)
    }
}

fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| c as char).collect()
}

impl Archive {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = File::open(&path)?;
        // SAFETY: game archives are treated as read-only for the lifetime of the process.
        let map = unsafe { Mmap::map(&file)? };
        let mut c = Cursor { d: &map, p: 0 };
        if c.bytes(4)? != b"BSA\0" {
            return Err(Error::BadMagic);
        }
        let version = c.u32()?;
        if !(103..=105).contains(&version) {
            return Err(Error::UnsupportedVersion(version));
        }
        let folder_offset = c.u32()? as usize;
        let archive_flags = c.u32()?;
        let folder_count = c.u32()? as usize;
        let file_count = c.u32()? as usize;
        let _total_folder_name_len = c.u32()?;
        let _total_file_name_len = c.u32()?;
        let _file_flags = c.u32()?;
        c.p = folder_offset;

        let mut folders = Vec::with_capacity(folder_count);
        for _ in 0..folder_count {
            let _hash = c.u64()?;
            let count = c.u32()? as usize;
            if version == 105 {
                let _pad = c.u32()?;
                let _off = c.u64()?;
            } else {
                let _off = c.u32()?;
            }
            folders.push(count);
        }

        let include_dir = archive_flags & flags::INCLUDE_DIR_NAMES != 0;
        let default_compressed = archive_flags & flags::COMPRESSED != 0;
        let mut pending: Vec<(String, Entry)> = Vec::with_capacity(file_count);
        for &count in &folders {
            let dir = if include_dir {
                let len = c.u8()? as usize;
                let raw = c.bytes(len)?;
                let raw = raw.strip_suffix(&[0]).unwrap_or(raw);
                latin1(raw)
            } else {
                String::new()
            };
            for _ in 0..count {
                let _hash = c.u64()?;
                let size = c.u32()?;
                let offset = c.u32()? as u64;
                let toggle = size & 0x4000_0000 != 0;
                pending.push((
                    dir.clone(),
                    Entry {
                        offset,
                        size: size & 0x3FFF_FFFF,
                        compressed: default_compressed ^ toggle,
                    },
                ));
            }
        }

        let mut files = HashMap::with_capacity(pending.len());
        if archive_flags & flags::INCLUDE_FILE_NAMES != 0 {
            for (dir, entry) in pending {
                let name = latin1(c.cstr()?);
                let full = if dir.is_empty() { name } else { format!("{dir}\\{name}") };
                files.insert(normalize_path(&full), entry);
            }
        } else {
            return Err(Error::Corrupt("archives without file names are not supported"));
        }

        Ok(Archive { path, map, version, archive_flags, files })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn version(&self) -> u32 {
        self.version
    }
    pub fn len(&self) -> usize {
        self.files.len()
    }
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
    pub fn contains(&self, path: &str) -> bool {
        self.files.contains_key(&normalize_path(path))
    }
    /// Iterate all (normalised) file paths contained in the archive.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.files.keys().map(|s| s.as_str())
    }

    /// Read and decompress a file. Returns `Ok(None)` if not present.
    pub fn read(&self, path: &str) -> Result<Option<Vec<u8>>> {
        match self.files.get(&normalize_path(path)) {
            Some(e) => self.read_entry(e).map(Some),
            None => Ok(None),
        }
    }

    fn read_entry(&self, e: &Entry) -> Result<Vec<u8>> {
        let mut c = Cursor { d: &self.map, p: e.offset as usize };
        let mut size = e.size as usize;
        if self.archive_flags & flags::EMBED_FILE_NAMES != 0 && self.version >= 104 {
            let n = c.u8()? as usize;
            c.bytes(n)?;
            size = size.checked_sub(n + 1).ok_or(Error::Corrupt("bad embedded name"))?;
        }
        if !e.compressed {
            return Ok(c.bytes(size)?.to_vec());
        }
        let original = c.u32()? as usize;
        let data = c.bytes(size.checked_sub(4).ok_or(Error::Corrupt("bad compressed size"))?)?;
        let mut out = Vec::with_capacity(original);
        if self.version == 105 {
            lz4_flex::frame::FrameDecoder::new(data)
                .read_to_end(&mut out)
                .map_err(|e| Error::Decompress(e.to_string()))?;
        } else {
            flate2::read::ZlibDecoder::new(data)
                .read_to_end(&mut out)
                .map_err(|e| Error::Decompress(e.to_string()))?;
        }
        if out.len() != original {
            return Err(Error::Decompress(format!(
                "size mismatch: expected {original}, got {}",
                out.len()
            )));
        }
        Ok(out)
    }
}

impl std::fmt::Debug for Archive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Archive")
            .field("path", &self.path)
            .field("version", &self.version)
            .field("files", &self.files.len())
            .finish()
    }
}
