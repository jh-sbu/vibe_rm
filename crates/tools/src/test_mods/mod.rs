//! Test mods: small plugins built to settle what public sources and the
//! game's data leave ambiguous, by running them in Skyrim SE. Each module
//! documents its setup and the outcome each reading predicts, then the
//! result once tested. Vanilla forms are looked up by editor id in the
//! user's Skyrim.esm, so no game data is copied here; the plugins themselves
//! aren't kept (`.esp` is ignored). Scripts are assembled with
//! `papyrus::build` and go in `Scripts/` beside the plugin.
//!
//! A script that tests on its own keeps each report in a `Result<Name>`
//! string variable and asks the game to save once done
//! (`Game.RequestSave`); `vrm-tool test-results` then reads the reports
//! from that save ([`results`]), so no one has to transcribe them.

use anyhow::{Result, bail};
use esp::{FormId, LoadOrder};

mod autotest;
mod autotest10;
mod autotest11;
mod autotest12;
mod autotest13;
mod autotest14;
mod autotest15;
mod autotest16;
mod autotest17;
mod autotest2;
mod autotest3;
mod autotest4;
mod autotest5;
mod autotest6;
mod autotest7;
mod autotest8;
mod autotest9;
mod dualcast;
mod pex;
mod places;
mod records;
pub mod results;
mod script;
mod spell_visuals;
mod taper;
mod venv;

/// The test mods by name: what `test-mod` builds.
pub const NAMES: &[&str] = &[
    "autotest",
    "autotest2",
    "autotest3",
    "autotest4",
    "autotest5",
    "autotest6",
    "autotest7",
    "autotest8",
    "autotest9",
    "autotest10",
    "autotest11",
    "autotest12",
    "autotest13",
    "autotest14",
    "autotest15",
    "autotest16",
    "autotest17",
    "dualcast",
    "pex",
    "taper",
    "venv",
    "spell-visuals",
];

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
        "autotest" => autotest::build(lo),
        "autotest2" => autotest2::build(lo),
        "autotest3" => autotest3::build(lo),
        "autotest4" => autotest4::build(lo),
        "autotest5" => autotest5::build(lo),
        "autotest6" => autotest6::build(lo),
        "autotest7" => autotest7::build(lo),
        "autotest8" => autotest8::build(lo),
        "autotest9" => autotest9::build(lo),
        "autotest10" => autotest10::build(lo),
        "autotest11" => autotest11::build(lo),
        "autotest12" => autotest12::build(lo),
        "autotest13" => autotest13::build(lo),
        "autotest14" => autotest14::build(lo),
        "autotest15" => autotest15::build(lo),
        "autotest16" => autotest16::build(lo),
        "autotest17" => autotest17::build(lo),
        "dualcast" => dualcast::build(lo),
        "pex" => pex::build(),
        "taper" => taper::build(lo),
        "spell-visuals" => spell_visuals::build(lo),
        "venv" => Ok(TestMod {
            plugin: venv::build(lo)?,
            scripts: Vec::new(),
        }),
        _ => bail!("unknown test mod {name:?}"),
    }
}
