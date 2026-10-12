//! Test mods: small plugins built to settle what public sources and the
//! game's data leave ambiguous, by running them in Skyrim SE. Each module
//! documents its setup and the outcome each reading predicts, then the
//! result once tested. Vanilla forms are looked up by editor id in the
//! user's Skyrim.esm, so no game data is copied here; the plugins themselves
//! aren't kept (`.esp` is ignored).

use anyhow::{Result, bail};
use esp::{FormId, LoadOrder};

mod venv;

/// The test mods by name: what `test-mod` builds.
pub const NAMES: &[&str] = &["venv"];

/// A test mod's own forms: master index 1 (after Skyrim.esm, its only
/// master), object ids from 0x800 as the Creation Kit starts them.
pub fn own(object_id: u32) -> FormId {
    FormId(0x0100_0000 | object_id)
}

/// The plugin file of a test mod (one of [`NAMES`]), with Skyrim.esm loaded.
pub fn build(name: &str, lo: &LoadOrder) -> Result<Vec<u8>> {
    match name {
        "venv" => venv::build(lo),
        _ => bail!("unknown test mod {name:?}"),
    }
}
