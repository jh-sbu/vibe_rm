//! Read-only reader for Skyrim Special Edition save games (`.ess`), as far
//! as test mods need: the header, the plugin list, the global data tables
//! (misc stats, global variables, ...) and Papyrus's script instances with
//! their variables, which is where a test mod's script leaves its results.
//! Change forms are split out but not decoded. This is no save system: the
//! engine neither writes nor loads saves.
//!
//! The layout follows UESP's public notes on the Skyrim save format, with
//! what Special Edition's version 12 saves were found to do differently
//! (checked on saves made in the game):
//!
//! - The body after the screenshot is LZ4 block compressed, preceded by
//!   its uncompressed and compressed lengths. The file location table's
//!   offsets count from the start of the file as if it weren't compressed
//!   (header and screenshot, without those two lengths).
//! - The screenshot is 4 bytes a pixel (RGBA).
//! - The third global data table holds one more entry than its count says.
//! - In the Papyrus section the string table's count and every string
//!   reference are 32 bits, and object handles and ids are 64 bits.

use std::collections::HashMap;
use std::path::Path;

mod papyrus;

pub use papyrus::{Array, Instance, Papyrus, Script, Variable};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("not a Skyrim save")]
    BadMagic,
    #[error("unsupported save version {0} (Special Edition's is 12)")]
    UnsupportedVersion(u32),
    #[error("unsupported compression type {0}")]
    UnsupportedCompression(u16),
    #[error("decompression failed: {0}")]
    Decompress(String),
    #[error("corrupt save: {0}")]
    Corrupt(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// The global data types, as UESP names them.
pub mod global {
    pub const MISC_STATS: u32 = 0;
    pub const PLAYER_LOCATION: u32 = 1;
    pub const TES: u32 = 2;
    pub const GLOBAL_VARIABLES: u32 = 3;
    pub const CREATED_OBJECTS: u32 = 4;
    pub const EFFECTS: u32 = 5;
    pub const WEATHER: u32 = 6;
    pub const AUDIO: u32 = 7;
    pub const SKY_CELLS: u32 = 8;
    pub const PAPYRUS: u32 = 1001;

    /// A global data type's name, if known.
    pub fn name(kind: u32) -> Option<&'static str> {
        Some(match kind {
            0 => "Misc Stats",
            1 => "Player Location",
            2 => "TES",
            3 => "Global Variables",
            4 => "Created Objects",
            5 => "Effects",
            6 => "Weather",
            7 => "Audio",
            8 => "Sky Cells",
            100 => "Process Lists",
            101 => "Combat",
            102 => "Interface",
            103 => "Actor Causes",
            104 => "Unknown 104",
            105 => "Detection Manager",
            106 => "Location MetaData",
            107 => "Quest Static Data",
            108 => "StoryTeller",
            109 => "Magic Favorites",
            110 => "PlayerControls",
            111 => "Story Event Manager",
            112 => "Ingredient Shared",
            113 => "MenuControls",
            114 => "MenuTopicManager",
            1000 => "Temp Effects",
            1001 => "Papyrus",
            1002 => "Anim Objects",
            1003 => "Timer",
            1004 => "Synchronized Animations",
            1005 => "Main",
            _ => return None,
        })
    }
}

/// The change form types by number (a change form's type byte, low 6 bits).
pub const CHANGE_FORM_TYPES: &[&str] = &[
    "REFR", "ACHR", "PMIS", "PGRE", "PBEA", "PFLA", "CELL", "INFO", "QUST", "NPC_", "ACTI", "TACT",
    "ARMO", "BOOK", "CONT", "DOOR", "INGR", "LIGH", "MISC", "APPA", "STAT", "MSTT", "FURN", "WEAP",
    "AMMO", "KEYM", "ALCH", "IDLM", "NOTE", "ECZN", "CLAS", "FACT", "PACK", "NAVM", "WOOP", "MGEF",
    "SMQN", "SCEN", "LCTN", "RELA", "PHZD", "PBAR", "PCON", "FLST", "LVLN", "LVLI", "LVSP", "PARW",
    "ENCH",
];

/// The save's header: what the load menu shows.
#[derive(Debug, Clone)]
pub struct Header {
    pub version: u32,
    pub save_number: u32,
    pub player_name: String,
    pub player_level: u32,
    pub player_location: String,
    /// Play time, `hours.minutes.seconds`.
    pub game_date: String,
    pub player_race: String,
    /// 0 male, 1 female.
    pub player_sex: u16,
    pub player_cur_exp: f32,
    pub player_lvl_up_exp: f32,
    /// When saved: a Windows FILETIME (100 ns since 1601).
    pub filetime: u64,
    pub shot_width: u32,
    pub shot_height: u32,
    pub compression: u16,
}

impl Header {
    /// When saved, in seconds since the Unix epoch.
    pub fn unix_time(&self) -> i64 {
        (self.filetime / 10_000_000) as i64 - 11_644_473_600
    }
}

/// A reference to a form in the save: 2 bits of kind and 22 of value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RefId(pub u32);

/// Where the player is: worldspace (none indoors), cell grid coordinates,
/// position in the worldspace or cell.
#[derive(Debug, Clone, Copy)]
pub struct PlayerLocation {
    pub world: Option<u32>,
    pub grid: [i32; 2],
    pub position: [f32; 3],
}

/// A change form: a form whose state differs from its plugin's.
#[derive(Debug, Clone)]
pub struct ChangeForm {
    pub form: RefId,
    pub flags: u32,
    /// The type (an index into [`CHANGE_FORM_TYPES`]).
    pub kind: u8,
    pub version: u8,
    /// Its data, zlib compressed when `uncompressed_len` isn't 0.
    pub data: Vec<u8>,
    pub uncompressed_len: u32,
}

#[derive(Debug, Clone)]
pub struct Save {
    pub header: Header,
    /// The screenshot, RGBA, `shot_width` x `shot_height`.
    pub screenshot: Vec<u8>,
    pub form_version: u8,
    pub plugins: Vec<String>,
    pub light_plugins: Vec<String>,
    /// The global data tables' entries in order: type, data.
    pub global_data: Vec<(u32, Vec<u8>)>,
    pub change_forms: Vec<ChangeForm>,
    /// What [`RefId`]s of kind 0 index (from 1).
    pub form_ids: Vec<u32>,
    pub visited_worldspaces: Vec<u32>,
}

/// A byte reader. Strings are cp1252; read as Latin-1, which differs only
/// in a few punctuation marks.
pub(crate) struct Cursor<'a> {
    pub d: &'a [u8],
    pub p: usize,
}

impl<'a> Cursor<'a> {
    pub fn new(d: &'a [u8]) -> Self {
        Cursor { d, p: 0 }
    }

    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self
            .p
            .checked_add(n)
            .filter(|&e| e <= self.d.len())
            .ok_or_else(|| {
                Error::Corrupt(format!(
                    "{n} bytes wanted at {} of {}",
                    self.p,
                    self.d.len()
                ))
            })?;
        let b = &self.d[self.p..end];
        self.p = end;
        Ok(b)
    }

    pub fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        Ok(self.bytes(N)?.try_into().unwrap())
    }

    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.array::<1>()?[0])
    }

    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    pub fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    pub fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_le_bytes(self.array()?))
    }

    /// A string with a 16-bit length.
    pub fn wstring(&mut self) -> Result<String> {
        let n = self.u16()? as usize;
        Ok(self.bytes(n)?.iter().map(|&b| b as char).collect())
    }

    /// A variable-size count: its low 2 bits say whether it's 1, 2 or 4
    /// bytes long, the rest is the value.
    pub fn vsval(&mut self) -> Result<u32> {
        let n = match self.d.get(self.p).map(|b| b & 3) {
            Some(0) => 1,
            Some(1) => 2,
            Some(2) => 4,
            _ => return Err(Error::Corrupt(format!("bad vsval at {}", self.p))),
        };
        let mut b = [0u8; 4];
        b[..n].copy_from_slice(self.bytes(n)?);
        Ok(u32::from_le_bytes(b) >> 2)
    }

    /// A [`RefId`]: 3 bytes, most significant first.
    pub fn ref_id(&mut self) -> Result<RefId> {
        let [a, b, c] = self.array()?;
        Ok(RefId(u32::from_be_bytes([0, a, b, c])))
    }
}

impl Save {
    pub fn open(path: &Path) -> Result<Save> {
        Save::parse(&std::fs::read(path)?)
    }

    pub fn parse(file: &[u8]) -> Result<Save> {
        let mut c = Cursor::new(file);
        if c.bytes(13).ok() != Some(b"TESV_SAVEGAME") {
            return Err(Error::BadMagic);
        }
        let header_size = c.u32()? as usize;
        let header_start = c.p;
        let version = c.u32()?;
        if version != 12 {
            return Err(Error::UnsupportedVersion(version));
        }
        let header = Header {
            version,
            save_number: c.u32()?,
            player_name: c.wstring()?,
            player_level: c.u32()?,
            player_location: c.wstring()?,
            game_date: c.wstring()?,
            player_race: c.wstring()?,
            player_sex: c.u16()?,
            player_cur_exp: c.f32()?,
            player_lvl_up_exp: c.f32()?,
            filetime: c.u64()?,
            shot_width: c.u32()?,
            shot_height: c.u32()?,
            compression: c.u16()?,
        };
        if c.p != header_start + header_size {
            return Err(Error::Corrupt(format!(
                "header is {} bytes, says {header_size}",
                c.p - header_start
            )));
        }
        let shot_len = header.shot_width as usize * header.shot_height as usize * 4;
        let screenshot = c.bytes(shot_len)?.to_vec();
        // Where the body would start uncompressed: what the offsets count from.
        let body_base = c.p;
        let body: Vec<u8> = match header.compression {
            0 => file[c.p..].to_vec(),
            2 => {
                let len = c.u32()? as usize;
                let compressed_len = c.u32()? as usize;
                let compressed = c.bytes(compressed_len)?;
                lz4_flex::block::decompress(compressed, len)
                    .map_err(|e| Error::Decompress(e.to_string()))?
            }
            n => return Err(Error::UnsupportedCompression(n)),
        };

        let mut c = Cursor::new(&body);
        let form_version = c.u8()?;
        let plugin_info_size = c.u32()? as usize;
        let plugins_start = c.p;
        let plugins = (0..c.u8()?)
            .map(|_| c.wstring())
            .collect::<Result<Vec<_>>>()?;
        let light_plugins = if form_version >= 78 {
            (0..c.u16()?)
                .map(|_| c.wstring())
                .collect::<Result<Vec<_>>>()?
        } else {
            Vec::new()
        };
        if c.p != plugins_start + plugin_info_size {
            return Err(Error::Corrupt("plugin info size".into()));
        }

        // The file location table.
        let mut t = [0u32; 10];
        for v in &mut t {
            *v = c.u32()?;
        }
        let [
            form_ids_at,
            _unknown_table_at,
            table1_at,
            table2_at,
            change_forms_at,
            table3_at,
            table1_count,
            table2_count,
            _table3_count,
            change_form_count,
        ] = t;
        let at = |offset: u32| -> Result<usize> {
            (offset as usize)
                .checked_sub(body_base)
                .filter(|&o| o <= body.len())
                .ok_or_else(|| Error::Corrupt(format!("offset {offset} outside the body")))
        };

        let mut global_data = Vec::new();
        let mut entry = |c: &mut Cursor| -> Result<()> {
            let kind = c.u32()?;
            let len = c.u32()? as usize;
            global_data.push((kind, c.bytes(len)?.to_vec()));
            Ok(())
        };
        for (offset, count) in [(table1_at, table1_count), (table2_at, table2_count)] {
            c.p = at(offset)?;
            for _ in 0..count {
                entry(&mut c)?;
            }
        }
        // The third table's count is one short: read up to the form ids.
        c.p = at(table3_at)?;
        let table3_end = at(form_ids_at)?;
        while c.p < table3_end {
            entry(&mut c)?;
        }

        c.p = at(change_forms_at)?;
        let mut change_forms = Vec::with_capacity(change_form_count as usize);
        for _ in 0..change_form_count {
            let form = c.ref_id()?;
            let flags = c.u32()?;
            let kind = c.u8()?;
            let version = c.u8()?;
            let mut len = || -> Result<u32> {
                Ok(match kind >> 6 {
                    0 => c.u8()? as u32,
                    1 => c.u16()? as u32,
                    _ => c.u32()?,
                })
            };
            let (len1, len2) = (len()?, len()?);
            change_forms.push(ChangeForm {
                form,
                flags,
                kind: kind & 0x3F,
                version,
                data: c.bytes(len1 as usize)?.to_vec(),
                uncompressed_len: len2,
            });
        }

        c.p = at(form_ids_at)?;
        let form_ids = (0..c.u32()?).map(|_| c.u32()).collect::<Result<Vec<_>>>()?;
        let visited_worldspaces = (0..c.u32()?).map(|_| c.u32()).collect::<Result<Vec<_>>>()?;

        Ok(Save {
            header,
            screenshot,
            form_version,
            plugins,
            light_plugins,
            global_data,
            change_forms,
            form_ids,
            visited_worldspaces,
        })
    }

    /// A global data entry's bytes.
    pub fn global(&self, kind: u32) -> Option<&[u8]> {
        self.global_data
            .iter()
            .find(|(k, _)| *k == kind)
            .map(|(_, d)| d.as_slice())
    }

    /// The form id a [`RefId`] stands for, in the save's load order: kind 0
    /// indexes the form id array, 1 is a form id below 0x400000 (one of
    /// Skyrim.esm's), 2 a created form (0xFF......). None for an index out of
    /// range, the null reference or kind 3.
    pub fn form_id(&self, r: RefId) -> Option<u32> {
        let value = r.0 & 0x3F_FFFF;
        match r.0 >> 22 {
            0 if value == 0 => None,
            0 => self.form_ids.get(value as usize - 1).copied(),
            1 => Some(value),
            2 => Some(0xFF00_0000 | value),
            _ => None,
        }
    }

    /// A plugin's load order index as form ids carry it: a full plugin's
    /// index, or 0xFE with the light plugin's index above the object id
    /// (returned as `0xFE000 | index`, the top 20 bits of its form ids).
    pub fn plugin_index(&self, name: &str) -> Option<u32> {
        if let Some(i) = self
            .plugins
            .iter()
            .position(|p| p.eq_ignore_ascii_case(name))
        {
            return Some(i as u32);
        }
        let i = self
            .light_plugins
            .iter()
            .position(|p| p.eq_ignore_ascii_case(name))?;
        Some(0xFE000 | i as u32)
    }

    /// The misc stats (the journal's General Stats): name, category, value.
    pub fn misc_stats(&self) -> Result<Vec<(String, u8, i32)>> {
        let Some(d) = self.global(global::MISC_STATS) else {
            return Ok(Vec::new());
        };
        let mut c = Cursor::new(d);
        (0..c.u32()?)
            .map(|_| Ok((c.wstring()?, c.u8()?, c.u32()? as i32)))
            .collect()
    }

    /// The global variables' values by form id.
    pub fn global_variables(&self) -> Result<HashMap<u32, f32>> {
        let Some(d) = self.global(global::GLOBAL_VARIABLES) else {
            return Ok(HashMap::new());
        };
        let mut c = Cursor::new(d);
        let mut out = HashMap::new();
        for _ in 0..c.vsval()? {
            let r = c.ref_id()?;
            let value = c.f32()?;
            if let Some(id) = self.form_id(r) {
                out.insert(id, value);
            }
        }
        Ok(out)
    }

    /// Where the player is.
    pub fn player_location(&self) -> Result<Option<PlayerLocation>> {
        let Some(d) = self.global(global::PLAYER_LOCATION) else {
            return Ok(None);
        };
        let mut c = Cursor::new(d);
        let _next_object_id = c.u32()?;
        let world = self.form_id(c.ref_id()?);
        let grid = [c.u32()? as i32, c.u32()? as i32];
        let _world2 = c.ref_id()?;
        let position = [c.f32()?, c.f32()?, c.f32()?];
        Ok(Some(PlayerLocation {
            world,
            grid,
            position,
        }))
    }

    /// The Papyrus section, decoded as far as script instances' variables
    /// and arrays.
    pub fn papyrus(&self) -> Result<Papyrus> {
        let d = self
            .global(global::PAPYRUS)
            .ok_or_else(|| Error::Corrupt("no Papyrus section".into()))?;
        Papyrus::parse(d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Saves put in the workspace's (ignored) `tests/` directory or its
    /// `results/` (saves test mods made), if any.
    fn local_saves() -> Vec<std::path::PathBuf> {
        let tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests");
        let mut saves: Vec<_> = [tests.clone(), tests.join("results")]
            .into_iter()
            .flat_map(std::fs::read_dir)
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("ess")))
            .collect();
        saves.sort();
        saves
    }

    #[test]
    fn local_saves_parse_through() {
        for path in local_saves() {
            let save = Save::open(&path).unwrap();
            assert_eq!(save.plugins[0], "Skyrim.esm");
            let p = save.papyrus().unwrap();
            // Every instance holds as many values as its script's chain has
            // variables, each of its declared type: parents' first.
            for i in &p.instances {
                let members = p.members(&i.script);
                assert_eq!(i.variables.len(), members.len(), "{}", i.script);
                for ((name, ty), v) in members.iter().zip(&i.variables) {
                    let ok = match v {
                        Variable::Null => true,
                        Variable::String(_) => ty.eq_ignore_ascii_case("String"),
                        Variable::Int(_) => ty.eq_ignore_ascii_case("Int"),
                        Variable::Float(_) => ty.eq_ignore_ascii_case("Float"),
                        Variable::Bool(_) => ty.eq_ignore_ascii_case("Bool"),
                        Variable::Array(..) => ty.ends_with("[]"),
                        Variable::Object(..) => !ty.ends_with("[]"),
                    };
                    assert!(ok, "{}.{name}: {ty} holds {v:?}", i.script);
                }
            }
            let globals = save.global_variables().unwrap();
            // GameYear, TimeScale.
            assert!(globals[&0x35] >= 201.0);
            assert!(globals[&0x3A] > 0.0);
            eprintln!(
                "{}: {} scripts, {} instances, {} arrays",
                path.display(),
                p.scripts.len(),
                p.instances.len(),
                p.arrays.len()
            );
        }
    }

    #[test]
    fn vsval_and_ref_ids() {
        let mut c = Cursor::new(&[0xD1, 0x0F, 0x40, 0x00, 0x35, 0x00]);
        assert_eq!(c.vsval().unwrap(), 1012);
        assert_eq!(c.ref_id().unwrap(), RefId(0x40_0035));
        assert_eq!(c.vsval().unwrap(), 0);
    }
}
