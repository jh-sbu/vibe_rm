//! `test-mod autotest24`: autotest23's faults and open items
//! (`research/autotest23.md`, "Faults and open items"). The records and
//! loggers are autotest21's ([`autotest21::build_batch`]), renamed
//! `VrmAt24`; the helpers are autotest22's `base.psc`. The actor logger now
//! names each spell cast (`cast#<form id>`).
//!
//! - **cell**: what makes `GetDisease` true (the traps' Ataxia, Ataxia cast
//!   as itself, cast by an NPC, Sanguinare Vampiris); the view cone's edge
//!   from 80° to 180° every 10°, the observer in the middle of the floor
//!   (each read gives the player's distance, to show they stayed put);
//!   Disarm at 12 and 30 on levels 1, 8, 10 and 20, some cases twice.
//! - **combat** (`WarehouseFurniture`): Fear on levels 1 and 8, Calm and Fury,
//!   all cast from 150 units and read for 8 s; what the Vigilant casts at
//!   90% to 20% health; Attack on Sight with a guard that is `IsGuard`
//!   (`IsGuardFaction` and our Attack on Sight crime faction) at 999 and
//!   1000; the lost rank-4 helper case, with rank 3 as the control.
//! - **death**, last: `IsKillerObject` run on the victim, then `GetTimeDead`
//!   read twice, the second time after timescale 100 for 30 s (autotest23's
//!   1000 crashed SE). Each is its own test, so a crash leaves the rest
//!   saved.
//!
//! Run as autotest23: load a throwaway save, wait for the "running"
//! notification and leave the game alone until the "done" box (about 15
//! minutes). Each test is announced and followed by a save; after a crash,
//! load the newest save, open the console at once and `set VrmAt24SkipTest
//! to <n>` to step past test n. Read with `vrm-tool test-results`. The
//! Disease test gives the player Ataxia and Sanguinare Vampiris for a few
//! seconds each and removes them.

use anyhow::Result;
use esp::LoadOrder;

use super::TestMod;
use super::autotest21::{self, Batch};

const SOURCES: &[(&str, &str)] = &[
    ("base", include_str!("autotest22/base.psc")),
    ("cell", include_str!("autotest24/cell.psc")),
    ("combat", include_str!("autotest24/combat.psc")),
    ("death", include_str!("autotest24/death.psc")),
];

const AUTOTEST24: Batch = Batch {
    name: "autotest24",
    prefix: "VrmAt24",
    sources: SOURCES,
    probes_on_skill: true,
    cell_location: None,
    checkpoints: true,
};

pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    autotest21::build_batch(lo, &AUTOTEST24)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tests_run_and_report() {
        autotest21::check_batch(&AUTOTEST24);
    }
}
