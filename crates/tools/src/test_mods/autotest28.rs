//! `test-mod autotest28`: autotest27's faults and open items
//! (`research/autotest27.md`, "Faults and open items"). The records and
//! loggers are autotest21's ([`autotest21::build_batch`]), renamed
//! `VrmAt28`; the helpers are autotest22's `base.psc` and
//! `autotest28/helpers.psc`. New: a `GetTimeDead` probe on Gamma, and the
//! wolf, the bear and their diseases named.
//!
//! - **warehouse** (`WarehouseFurniture`):
//!   - DisarmCap: our Disarm cast mid-swing (`IsAttacking`) at a level 15
//!     fighter, at 12 and at 30 three times each, a level 1 one at 12 as the
//!     control.
//!   - Healer, HealerWarlock: the Vigilant with 600 health, free, fighting
//!     a fighter from 70%, 50% and 30% health, then a fire warlock from 70%
//!     and 40%; health, magicka and spells cast every 4 s.
//!   - SheatheDelay: an unalerted, free NPC told to `DrawWeapon`, polled
//!     every 0.05 s for when it reads drawn and sheathed; twice more with a
//!     second `DrawWeapon` 0.5 s after it reads drawn.
//!   - AttackOnSightHigh: our `IsGuard` guard at bounties 2000 and 5000.
//!   - FuryTarget: our Fury with the other NPC nearer than the caster.
//!   - RestrainedMidFight: `SetRestrained` on a fighter already fighting.
//!   - HostileSpellHeld: a script fire spell at a lawful NPC held by
//!     `SetDontMove`.
//! - **outdoors** (where the run began; skipped if indoors): Disease. A
//!   skeever, a wolf and a bear, whose attacks carry Ataxia, Rockjoint and
//!   Bone Break Fever, each attack the player for up to 90 s or until the
//!   player has the disease; then `GetDisease`.
//! - **death** (our cell): `GetTimeDead >= 1` polled from a kill made 0.9 to
//!   0.95 into the hour and from one 0.02 to 0.1 into it.
//!
//! | reading | means |
//! | --- | --- |
//! | level 15 disarmed at 12 mid-swing | the cap isn't the target's level against the magnitude |
//! | level 15 kept its sword at 12, lost it at 30 | the cap holds as UESP has it |
//! | TimeDeadBeforeHour holds within about 20 s, TimeDeadAfterHour after about 170 s | hours struck on the clock (ours) |
//! | both hold after about 60 s | whole real minutes |
//! | both hold after about 180 s | whole game hours elapsed |
//! | a disease caught, then `GetDisease` yes | a caught disease counts; an added one doesn't |
//!
//! Run: load a throwaway save **outdoors** (the Disease test lets a bear at
//! the player: the player gets 1000 extra health, kept topped up), wait for
//! the "running" notification and leave the game alone until the "done"
//! box (13 tests, about 20 to 35 minutes; the time-dead tests wait for the
//! clock). The player is moved to our cell, the warehouse, back where the
//! run began and to our cell again. The diseases caught are removed. Each
//! test is announced and followed by a save. After a crash, load the newest
//! save, open the console at once and `set VrmAt28SkipTest to <n>` to step
//! past test n (see the module doc of `test_mods`). Read with `vrm-tool
//! test-results`.

use anyhow::Result;
use esp::LoadOrder;

use super::TestMod;
use super::autotest21::{self, Batch};

const SOURCES: &[(&str, &str)] = &[
    ("base", include_str!("autotest22/base.psc")),
    ("helpers", include_str!("autotest28/helpers.psc")),
    ("warehouse", include_str!("autotest28/warehouse.psc")),
    ("outdoors", include_str!("autotest28/outdoors.psc")),
    ("death", include_str!("autotest28/death.psc")),
];

const AUTOTEST28: Batch = Batch {
    name: "autotest28",
    prefix: "VrmAt28",
    sources: SOURCES,
    probes_on_skill: true,
    cell_location: None,
    checkpoints: true,
    pens: false,
};

pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    autotest21::build_batch(lo, &AUTOTEST28)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tests_run_and_report() {
        autotest21::check_batch(&AUTOTEST28);
    }
}
