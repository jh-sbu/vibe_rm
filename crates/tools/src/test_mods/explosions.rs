//! `test-mod explosions`: the choices in `known_ambiguities/explosions.md`
//! a script can read (`plans/explosions.md`, step 8). The records and
//! loggers are autotest21's ([`autotest21::build_batch`]), renamed
//! `VrmExpl`; the helpers are autotest22's `base.psc`. Explosions are
//! placed with `PlaceAtMe` at a marker 600 north of the test cell's centre,
//! the player at its entry (1080 away).
//!
//! - **PlacedRef**: the reference `PlaceAtMe` gives for an explosion
//!   (`TrapBallistaBoltExplosion01`), read at once, at 1 s and at 6 s:
//!   deleted, enabled, 3D loaded, where.
//! - **Falloff**: the same explosion (damage 80, radius 320, ignores line of
//!   sight) among NPCs 40 to 360 units north; one in iron armor and shield
//!   200 south. Health lost and events (the actor log's `OnHit` now gives
//!   the source's and aggressor's form ids: `hit#<source><<aggressor>`).
//! - **Shade**: `dunHighGateFlamingThrallFireBallExp01` (damage 60 and a
//!   fire enchantment, line of sight checked) with an NPC 200 north behind
//!   one at 90, and one 200 east in the open.
//! - **Knockdowns**: `fakeForceBallLite1024` ("Knock Down - Always") and
//!   `fakeForceBall1024ByFormula` placed at an NPC left at 100%, 60%, 45%
//!   and 20% health; `GetKnockedStateEnum` through a probe.
//!
//! | Reading | Ours predicts |
//! | --- | --- |
//! | PlacedRef | base the explosion; not deleted at once or at 1 s; gone (deleted) by 6 s |
//! | Falloff | 80 at 40, 120, 200 and 280; 0 at 360; the armored one 80 too; `hit#<explosion>` with no aggressor |
//! | Shade | 90: 60 and the fire; behind: nothing; open: 60 and the fire |
//! | Knockdowns | none: no; Always: yes; By Formula: no at 100% and 60%, yes at 45% and 20% |
//!
//! **Dry run in our engine** (2026-11-11, offscreen in Riverwood): PlacedRef,
//! Falloff and Knockdowns read as predicted. Two readings our engine can't
//! give: the actor log stays empty (`[None]`: created actors don't get
//! their base's scripts here, so the cast to the logger fails), and
//! nothing was shaded in Shade, though an engine test with actors' capsules
//! in place is (`an_actor_in_the_way_shades_another_from_a_blast`):
//! whether a held actor's capsule follows a script's `MoveTo` is open.
//!
//! Run as autotest24: load a throwaway save, wait for the "running"
//! notification and leave the game alone until the "done" box (4 tests,
//! a few minutes). Each test is announced and followed by a save; after a
//! crash, load the newest save, open the console at once and `set
//! VrmExplSkipTest to <n>` to step past test n (see the module doc of
//! `test_mods`). Read with `vrm-tool test-results`.

use anyhow::Result;
use esp::LoadOrder;

use super::TestMod;
use super::autotest21::{self, Batch};

const SOURCES: &[(&str, &str)] = &[
    ("base", include_str!("autotest22/base.psc")),
    ("explosions", include_str!("explosions/explosions.psc")),
];

const EXPLOSIONS: Batch = Batch {
    name: "explosions",
    prefix: "VrmExpl",
    sources: SOURCES,
    probes_on_skill: true,
    cell_location: None,
    checkpoints: true,
};

pub fn build(lo: &LoadOrder) -> Result<TestMod> {
    autotest21::build_batch(lo, &EXPLOSIONS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tests_run_and_report() {
        autotest21::check_batch(&EXPLOSIONS);
    }
}
