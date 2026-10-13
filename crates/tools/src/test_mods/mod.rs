//! Test mods: small plugins built to settle what public sources and the
//! game's data leave ambiguous, by running them in Skyrim SE. Each module
//! documents its setup and the outcome each reading predicts, then the
//! result once tested. Vanilla forms are looked up by editor id in the
//! user's Skyrim.esm, so no game data is copied here; the plugins themselves
//! aren't kept (`.esp` is ignored). Scripts are assembled with
//! `papyrus::build` and go in `Scripts/` beside the plugin.

use anyhow::{Result, bail};
use esp::{FormId, LoadOrder};

mod pex;
mod venv;

/// The test mods by name: what `test-mod` builds.
pub const NAMES: &[&str] = &["pex", "venv"];

/// A test mod's files: the plugin, and its compiled scripts by name.
pub struct TestMod {
    pub plugin: Vec<u8>,
    pub scripts: Vec<(String, Vec<u8>)>,
}

/// A test mod's own forms: master index 1 (after Skyrim.esm, its only
/// master), object ids from 0x800 as the Creation Kit starts them.
pub fn own(object_id: u32) -> FormId {
    FormId(0x0100_0000 | object_id)
}

/// The files of a test mod (one of [`NAMES`]), with Skyrim.esm loaded.
pub fn build(name: &str, lo: &LoadOrder) -> Result<TestMod> {
    match name {
        "pex" => pex::build(),
        "venv" => Ok(TestMod {
            plugin: venv::build(lo)?,
            scripts: Vec::new(),
        }),
        _ => bail!("unknown test mod {name:?}"),
    }
}
