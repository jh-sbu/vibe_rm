//! `test-mod autotest22`: autotest21's faults run again
//! (`research/autotest21.md`, "Faults"). The records and loggers are
//! autotest21's ([`autotest21::build_batch`]), its forms and scripts renamed
//! `VrmAt22`; these tests differ:
//!
//! - **Probes** raise `OneHanded` (actor value 6), and `Probe` reads only
//!   that gain: `HasMagicEffect` reports an ability's effect whose
//!   condition fails. `IsWin32` (holds) and `IsPC1stPerson` in third person
//!   (fails) check the probe itself. This reruns every condition test
//!   (autotest21's `conditions.psc`), `InCombatParam` and `Recheck`.
//! - **Created aliases** are optional and made at the player, as autotest4's
//!   that worked (`CreatedRefs`, `CreateLevel`).
//! - **Casters** are copies of the vanilla fire warlock and Vigilant of
//!   Stendarr with our logger (`Casters`, `NpcCostReduction`, `Healer`,
//!   `Wards`), and a spell-less warlock with a staff (`NpcStaff`).
//! - **Fights** get health to last (`FightAfterAlly`, `Flee` against a
//!   level 20 fighter); `PlayerEnemy` and `AttackOnSight` print each NPC's
//!   hostility and detection of the player beside its combat state.
//! - Smaller ones: `UniquePlain` (no template), `Disarm` on a level 1 NPC in
//!   combat, `InvisibleFood` and `InvisibleActivate` each alone,
//!   `StoryPickups` with 4 s between events, `ViewCone` on the floor with
//!   two reads, `Holds` read by fleeing and combat state, `SunPosition`
//!   outdoors.
//!
//! Run as autotest21: load a throwaway save outdoors, wait for the
//! "running" notification and leave the game alone until the "done" box
//! (about 25 minutes, mostly the probes). The player is moved to our cell,
//! the warehouse and back where the run began. Read with `vrm-tool
//! test-results`.
//!
//! **Run 2026-11-01** (`research/autotest22.md`): the probes work (the
//! controls hold and fail as they should), so the condition readings stand.
//! Created aliases, vanilla casters, fights and most small faults gave
//! readings. Still faults: `GetRelativeAngle` (no axis), the "drawn" torch and
//! shield reads, Disarm, Holds and the crime guard (no Attack on Sight).

use anyhow::Result;
use esp::LoadOrder;

use super::TestMod;
use super::autotest21::{self, Batch};

const SOURCES: &[(&str, &str)] = &[
    ("base", include_str!("autotest22/base.psc")),
    ("natives", include_str!("autotest22/natives.psc")),
    ("conditions", include_str!("autotest21/conditions.psc")),
    ("magic", include_str!("autotest22/magic.psc")),
    ("detect", include_str!("autotest22/detect.psc")),
    ("story", include_str!("autotest22/story.psc")),
    ("combat", include_str!("autotest22/combat.psc")),
    ("outdoors", include_str!("autotest22/outdoors.psc")),
];

const AUTOTEST22: Batch = Batch {
    name: "autotest22",
    prefix: "VrmAt22",
    sources: SOURCES,
    probes_on_skill: true,
    cell_location: None,
};

pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    autotest21::build_batch(lo, &AUTOTEST22)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tests_run_and_report() {
        autotest21::check_batch(&AUTOTEST22);
    }
}
