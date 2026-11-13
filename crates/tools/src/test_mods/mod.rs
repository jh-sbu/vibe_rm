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
//!
//! **Checkpoints and skipping a crashing test.** A batch built with
//! `autotest21::Batch { checkpoints: true, .. }` (autotest23 on) announces
//! each test as it starts (`vibe_rm autotest24 10/12: KillerObject`) and
//! saves after each one. It also reads its global `<prefix>SkipTest`
//! (`VrmAt24SkipTest`) before each test: a test whose number it holds is
//! skipped and reported as "skipped by SkipTest". If the game crashes to
//! desktop, the newest save holds every report so far, and the crash is the
//! test after the last report (or the last notification seen). The script is
//! suspended mid-run in that save, so loading it resumes the run. To step
//! past the crash, load the save, open the console at once (the script waits
//! about 3 s after each save, and paused time doesn't count) and enter `set
//! VrmAt24SkipTest to <n>`. The run goes on and the reports are read as usual.
//! A batch's run instructions should repeat this, with its own prefix.

use anyhow::{Result, bail};
use esp::{FormId, LoadOrder};

mod ammo;
mod autotest;
mod autotest10;
mod autotest11;
mod autotest12;
mod autotest13;
mod autotest14;
mod autotest15;
mod autotest16;
mod autotest17;
mod autotest18;
mod autotest19;
mod autotest2;
mod autotest20;
mod autotest21;
mod autotest22;
mod autotest23;
mod autotest24;
mod autotest25;
mod autotest3;
mod autotest4;
mod autotest5;
mod autotest6;
mod autotest7;
mod autotest8;
mod autotest9;
mod controltags;
mod crafting1;
mod dualcast;
mod explosions;
mod firstperson;
mod loadscreens;
mod pex;
mod places;
mod ragdolls;
mod records;
mod regen;
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
    "autotest18",
    "autotest19",
    "autotest20",
    "autotest21",
    "autotest22",
    "autotest23",
    "autotest24",
    "autotest25",
    "explosions",
    "dualcast",
    "crafting1",
    "controltags",
    "pex",
    "taper",
    "venv",
    "spell-visuals",
    "loadscreens",
    "ammo",
    "firstperson",
    "regen",
    "ragdolls",
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
        "firstperson" => firstperson::build(lo),
        "ammo" => ammo::build(lo),
        "regen" => regen::build(lo),
        "ragdolls" => ragdolls::build(lo),
        "controltags" => controltags::build(),
        "crafting1" => crafting1::build(lo),
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
        "autotest18" => autotest18::build(lo),
        "autotest19" => autotest19::build(lo),
        "autotest20" => autotest20::build(lo),
        "autotest21" => autotest21::build(lo),
        "autotest22" => autotest22::build(lo),
        "autotest23" => autotest23::build(lo),
        "autotest24" => autotest24::build(lo),
        "autotest25" => autotest25::build(lo),
        "explosions" => explosions::build(lo),
        "dualcast" => dualcast::build(lo),
        "loadscreens" => Ok(TestMod {
            plugin: loadscreens::build(lo)?,
            scripts: Vec::new(),
        }),
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
