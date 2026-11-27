//! `test-mod autotest30`: seven open items, each run in the nine test pens
//! at once ([`super::pens`]). The records and loggers are autotest21's
//! ([`autotest21::build_batch`]), renamed `VrmAt30`; the helpers are
//! autotest22's `base.psc`, autotest28's `helpers.psc` and the pens'
//! `pens/helpers.psc`. The player watches from the booth throughout.
//!
//! - **PensEnter**: the player to the booth.
//! - **NpcRegen** (`test-mod regen`'s open item): whether NPCs'
//!   `HealRateMult`, `StaminaRateMult` and `MagickaRateMult` count (100, 0
//!   and 300 each); the health pens' `HealRate` set to 0.7 (NPCs' reads 0).
//! - **KnockdownHolds** (`test-mod explosions`' open item): "Always"
//!   (`fakeForceBallLite1024`) beside NPCs held by `SetDontMove`, free, and
//!   held by `SetRestrained`, three each; knocked read by the probe.
//! - **HostileHeld** (autotest28): a script fire spell from an NPC 100 or
//!   400 off at a lawful NPC held or free, unarmed or with a sword: state 2
//!   (search) from the hold, or from being out of reach?
//! - **FuryUnseen**: our Fury cast by the player from the booth (unseen) at
//!   an NPC with another nearby, or alone; cast by an NPC who is then taken
//!   away. Our Fury is a target-actor spell; whether one cast by script
//!   lands through the booth's floor is unread (Aggression 3 in the reads
//!   says it did).
//! - **Healing** (autotest28): Vigilants against melee and fire from 0.20,
//!   0.18, 0.15 and 0.10 of 600 health, and one with no magicka.
//! - **DisarmDagger** (`known_ambiguities/disarm.md`): disarmed mid-swing,
//!   with a dagger or without, its opponent there or gone: when and with
//!   what it arms itself again.
//! - **DiseaseResist** (autotest29): Disease Resist 0, 50 and 100 against a
//!   bear; once infected, the bear gone and max stamina read every second
//!   for 120 s; `GetDisease` on the NPC.
//!
//! | reading | means |
//! | --- | --- |
//! | NpcRegen: 0 at multiplier 0, about 3× at 300 | the multipliers count for NPCs |
//! | KnockdownHolds: held pens knocked | the holds don't keep a knockdown off |
//! | HostileHeld: held at 100 fights, held at 400 searches | reach, not the hold |
//! | FuryUnseen 0-5: other / none / player | what Fury does without its caster in sight |
//! | Healing: heals at 0.18 or 0.15 against melee | spell attack isn't needed |
//! | DisarmDagger: armed again after n s | how long ours should wait |
//! | DiseaseResist 6-8: never caught; 3-5 about half as often | resist as a separate roll |
//! | DiseaseResist: drained in every read after catching | rolled once (ours) |
//! | DiseaseResist: changes, or drained in few reads | rolled again |
//!
//! Run: load a throwaway save (anywhere), wait for the "running"
//! notification and leave the game alone until the "done" box (8 tests,
//! about 15 to 20 minutes). The player is moved to our cell, then to the
//! booth above the pens, and stays there. Each test is announced and
//! followed by a save. After a crash, load the newest save, open the
//! console at once and `set VrmAt30SkipTest to <n>` to step past test n
//! (see the module doc of `test_mods`). Read with `vrm-tool test-results`.

use anyhow::Result;
use esp::LoadOrder;

use super::TestMod;
use super::autotest21::{self, Batch};

const SOURCES: &[(&str, &str)] = &[
    ("base", include_str!("autotest22/base.psc")),
    ("helpers", include_str!("autotest28/helpers.psc")),
    ("pen helpers", include_str!("pens/helpers.psc")),
    ("basics", include_str!("autotest30/basics.psc")),
    ("magic", include_str!("autotest30/magic.psc")),
    ("combat", include_str!("autotest30/combat.psc")),
    ("disease", include_str!("autotest30/disease.psc")),
];

const AUTOTEST30: Batch = Batch {
    name: "autotest30",
    prefix: "VrmAt30",
    sources: SOURCES,
    probes_on_skill: true,
    cell_location: None,
    checkpoints: true,
    pens: true,
};

pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    autotest21::build_batch(lo, &AUTOTEST30)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tests_run_and_report() {
        autotest21::check_batch(&AUTOTEST30);
    }
}
