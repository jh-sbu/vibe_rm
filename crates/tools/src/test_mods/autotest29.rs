//! `test-mod autotest29`: whether NPCs catch a creature's disease, the first
//! batch in the test pens ([`super::pens`]). The records and loggers are
//! autotest21's ([`autotest21::build_batch`]), renamed `VrmAt29`, with the
//! pens cell (`VrmAt29Pens`) added; the helpers are autotest22's `base.psc`
//! and autotest28's `helpers.psc`.
//!
//! Why: UESP says followers aren't affected by diseases (*Skyrim:Followers*,
//! revision 3646982, unsourced), its talk page has players seeing followers
//! glow and cough after wolves, and nothing in the records keeps creature
//! diseases off NPCs (only Sanguinare Vampiris's effect asks `GetIsID
//! Player`). If NPCs catch them, later disease tests (the chance per blow,
//! Resist Disease) can run nine at once.
//!
//! - **PensEnter**: the player to the booth; the pens' distances.
//! - **NpcDisease**: in each pen an unaggressive NPC (DiseaseResist 0, held
//!   by `SetDontMove`, 5000 health kept topped up) and a bear (Bone Break
//!   Fever on every attack, 6% a blow) given `StartCombat` on it, 150 s.
//!   Per NPC: health drops, when `HasSpell(DiseaseBoneBreakFever)` and
//!   `HasMagicEffect(DisDamageStamina)` first read true, and its max
//!   stamina before and after (the fever drains 25).
//!
//! | reading | means |
//! | --- | --- |
//! | max stamina 25 lower, on most of the nine | NPCs catch creature diseases |
//! | no drop on any, after 40 or so blows each | they don't (UESP's note) |
//! | `HasSpell` or `HasMagicEffect` true without a drop | it is added but does nothing on NPCs, or is added on every blow |
//! | bears not fighting at 3 s, or 0 drops | the pens' navmesh or walls fault |
//!
//! Run: load a throwaway save (anywhere), wait for the "running"
//! notification and leave the game alone until the "done" box (2 tests,
//! about 4 minutes). The player is moved to our cell, then to the booth
//! above the pens, and stays there. Each test is announced and followed by
//! a save. After a crash, load the newest save, open the console at once
//! and `set VrmAt29SkipTest to <n>` to step past test n (see the module doc
//! of `test_mods`). A crash on entering the pens (test 1) would be the
//! navmeshes: report it. Read with `vrm-tool test-results`.

use anyhow::Result;
use esp::LoadOrder;

use super::TestMod;
use super::autotest21::{self, Batch};

const SOURCES: &[(&str, &str)] = &[
    ("base", include_str!("autotest22/base.psc")),
    ("helpers", include_str!("autotest28/helpers.psc")),
    ("pens", include_str!("autotest29/pens.psc")),
];

const AUTOTEST29: Batch = Batch {
    name: "autotest29",
    prefix: "VrmAt29",
    sources: SOURCES,
    probes_on_skill: true,
    cell_location: None,
    checkpoints: true,
    pens: true,
};

pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    autotest21::build_batch(lo, &AUTOTEST29)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tests_run_and_report() {
        autotest21::check_batch(&AUTOTEST29);
    }
}
