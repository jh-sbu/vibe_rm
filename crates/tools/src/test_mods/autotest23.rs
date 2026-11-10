//! `test-mod autotest23`: what autotest21 and autotest22 left open
//! (`research/autotest22.md`, "Faults and open items" and "Compared with
//! ours"; `plans/ROADMAP.md`), with cheap audit items that share their
//! setups (`plans/autotest-audit.md`). The records and loggers are
//! autotest21's ([`autotest21::build_batch`]), renamed `VrmAt23`; the helpers
//! are autotest22's `base.psc`. Our test cell's location is Whiterun Hold
//! this time, for the location and hold-crime readings.
//!
//! - **natives** (test cell): game settings the fleeing, healing and
//!   assistance items name; `GameTimeToString` on this save;
//!   `HasParentRelationship(None)`; `IsGuard` on a hold guard, a copy without
//!   its crime faction and our guard-class / `IsGuardFaction` NPCs with and
//!   without a hold crime faction; a deleted reference still held; Spells
//!   Learned after `AddSpell`; locations in a cell that has one, and
//!   `GetCrimeGold` without a faction there.
//! - **conditions**: `GetXPForNextLevel` and `GetTimeDead` bisected (the time
//!   dead read again after game time races), `IsKillerObject` run on the
//!   victim, `GetRelativeAngle` with its axis, `GetDisease` on the player,
//!   torch and shield readings in combat.
//! - **magic**: `KILL` for an essential actor's `Kill`, effects on the dead,
//!   a fire-and-forget effect whose condition fails, Slow Time's world and
//!   clock shares through and after each word, Disarm (the shout from close
//!   by, a target-actor copy at 12 and 30, levels 1 to 20, a greatsword),
//!   the view cone's edge between 90° and 135°, create levels (only at
//!   player level 5 or more).
//! - **combat** (`WarehouseFurniture`): NPCs' enchanted blows' charge by
//!   Enchanting, staffs (hands, charge by skill and `DestructionMod`, a
//!   fire-and-forget staff, a staff alone), Fear / Calm / Fury read by
//!   `IsFleeing` and effect events, the Vigilant healing and warding with
//!   magicka enough, Average vs Coward at 18%, ethereal targets,
//!   `SetAttackActorOnSight` at aggression 1 to 3, an Attack on Sight crime
//!   faction at 999 / 1000, relationship ranks 1, 2 and 4 bringing help, a
//!   follower-faction member helping without the teammate flag.
//! - **outdoors** (where the run began): locations of the player, created
//!   and far references, and `GetCrimeGold` without a faction in a hold.
//!
//! Run as autotest22: load a throwaway save outdoors (a player of level 5 or
//! more also runs the create levels), wait for the "running" notification
//! and leave the game alone until the "done" box (about 35 minutes). The
//! player is moved to our cell, the warehouse and back where the run began;
//! the last test sets small Whiterun and Rift bounties for a few seconds
//! outdoors, so a guard nearby may come over (answer them if so). Read with
//! `vrm-tool test-results`.
//!
//! Each test is announced as it starts (`vibe_rm autotest23 9/33: TimeDead`)
//! and the game saves after each one ([`Batch::checkpoints`]). Run 1
//! (2026-11-10) crashed to desktop between `XPNext`'s skill increase and the
//! first save (after test 20), with no Papyrus log. After a crash, the
//! newest save holds every result so far, and the test after the last one
//! is the culprit. To go past it, load that save, open the console within a
//! few seconds, `set VrmAt23SkipTest to <its number>`, and close it. The
//! script resumes there and reports that test as skipped.

use anyhow::Result;
use esp::LoadOrder;

use super::TestMod;
use super::autotest21::{self, Batch};

const SOURCES: &[(&str, &str)] = &[
    ("base", include_str!("autotest22/base.psc")),
    ("natives", include_str!("autotest23/natives.psc")),
    ("conditions", include_str!("autotest23/conditions.psc")),
    ("magic", include_str!("autotest23/magic.psc")),
    ("combat", include_str!("autotest23/combat.psc")),
    ("outdoors", include_str!("autotest23/outdoors.psc")),
];

const AUTOTEST23: Batch = Batch {
    name: "autotest23",
    prefix: "VrmAt23",
    sources: SOURCES,
    probes_on_skill: true,
    cell_location: Some("WhiterunHoldLocation"),
    checkpoints: true,
};

pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    autotest21::build_batch(lo, &AUTOTEST23)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tests_run_and_report() {
        autotest21::check_batch(&AUTOTEST23);
    }
}
