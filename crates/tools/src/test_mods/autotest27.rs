//! `test-mod autotest27`: autotest24's faults and open items
//! (`research/autotest24.md`), autotest20's charge (`research/autotest20.md`)
//! and the SE readings earlier batches took on held actors, repeated with the
//! actor free (`plans/ROADMAP.md`). The records and loggers are autotest21's
//! ([`autotest21::build_batch`]), renamed `VrmAt27`; the helpers are
//! autotest22's `base.psc`. New records: target-actor copies of Calm, Fear
//! and Fury (the vanilla spells' main effects at their magnitudes), a level
//! 15 fighter with a sword (above Disarm's cap of 12) and a voiceless
//! `IsGuard` guard of Whiterun's crime faction.
//!
//! - **warehouse** (`WarehouseFurniture`):
//!   - Holds: our Fear on levels 1 and 8, our Calm and our Fury, cast from
//!     150 units and read for 8 s, autotest24's fights.
//!   - Disarm (12), four casts each: an alerted NPC drawn out of combat; a
//!     fighter caught mid-swing (`IsAttacking`); one caught between swings.
//!     Above the cap: a level 15 NPC at 12 three times, at 30 twice.
//!   - Disease: a skeever, whose attacks carry `DiseaseAtaxia`, attacks the
//!     player for up to 120 s; `GetDisease` once the player has Ataxia.
//!   - HealerHurt, WardsFree: the Vigilant, free and at 40% health, fighting
//!     a fighter and then a fire warlock, read every 4 s for 40 s.
//!   - HostileSpellFree: a lawful NPC, free, hit by a script fire spell.
//!   - SheatheFree: an unalerted NPC, free, told to `DrawWeapon`, twice once
//!     and twice drawn again at 0.3 s.
//!   - RestrainedFights: a fighter restrained, `SetDontMove` or free, an
//!     enemy 100 off; swings, damage dealt, movement over 12 s.
//!   - AttackOnSightLong: autotest24's guard at bounty 1000, watched for
//!     40 s.
//! - **outdoors** (where the run began; skipped if indoors):
//!   - Charge1500, Charge2500: two level 15 fighters fighting each other,
//!     both free. Once engaged, the second is moved 1500 or 2500 away and
//!     both are sampled closing in: `S` sprinting, `R` running, `W` neither.
//!   - AttackOnSightHold: the Whiterun guard at a Whiterun bounty of 1000,
//!     watched for 40 s. Only in Whiterun Hold; the bounty is put back
//!     after.
//! - **death** (our cell), last: Victim killed, `GetTimeDead` read twice
//!   (the second after timescale 100 for 30 s), then `IsKillerObject` run
//!   on the dead victim, which is expected to crash SE.
//!
//! | reading | means |
//! | --- | --- |
//! | idle casts all disarm, mid-swing ones miss | Disarm fails while the target attacks (the stagger it can't take) |
//! | misses in every group | something else (a chance) decides |
//! | the level 15 NPC disarmed at 12 | the cap isn't the target's level against the magnitude |
//! | `S` while far, then `R` | NPCs sprint to charge; the switch distance can be read off |
//! | `R` or `W` throughout | NPCs don't sprint to charge |
//! | KillerObjectDead crashes | `IsKillerObject` run on a dead reference crashes SE |
//!
//! Run: load a throwaway save **outdoors, on open, flat ground in Whiterun
//! Hold** (near Riverwood is fine), wait for the "running" notification and
//! leave the game alone until the "done" box (21 tests, about 25 minutes).
//! The player is moved to our cell, the warehouse, back where the run
//! began, and to our cell again. The Disease test lets a skeever give the
//! player Ataxia and removes it after. AttackOnSightHold gives the player a
//! Whiterun bounty of 1000 for 40 s and then puts back what it was; a real
//! guard nearby may come over, and if one starts arrest dialogue, the
//! script waits until it's closed. Each test is announced and followed by a
//! save. After a crash, load the newest save, open the console at once and
//! `set VrmAt27SkipTest to <n>` to step past test n (see the module doc of
//! `test_mods`). The last test, KillerObjectDead, is expected to crash: if
//! it does, there is nothing to step past, as every other report is already
//! in the newest save. Read with `vrm-tool test-results`.
//!
//! **Run 2026-11-19** (`research/autotest27.md`): every test reported but
//! KillerObjectDead, which crashed SE as expected. Disease (the skeever
//! never attacked) and SheatheFree (the loop stopped before the draw
//! showed) were faults.

use anyhow::Result;
use esp::LoadOrder;

use super::TestMod;
use super::autotest21::{self, Batch};

const SOURCES: &[(&str, &str)] = &[
    ("base", include_str!("autotest22/base.psc")),
    ("warehouse", include_str!("autotest27/warehouse.psc")),
    ("outdoors", include_str!("autotest27/outdoors.psc")),
    ("death", include_str!("autotest27/death.psc")),
];

const AUTOTEST27: Batch = Batch {
    name: "autotest27",
    prefix: "VrmAt27",
    sources: SOURCES,
    probes_on_skill: true,
    cell_location: None,
    checkpoints: true,
};

pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    autotest21::build_batch(lo, &AUTOTEST27)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tests_run_and_report() {
        autotest21::check_batch(&AUTOTEST27);
    }
}
