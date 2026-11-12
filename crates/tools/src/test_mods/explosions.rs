//! `test-mod explosions`: the choices in `known_ambiguities/explosions.md`
//! a script can read (`plans/explosions.md`, step 8). The records and
//! loggers are autotest21's ([`autotest21::build_batch`]), renamed
//! `VrmExpl`; the helpers are autotest22's `base.psc`. Explosions are
//! placed with `PlaceAtMe` at markers near a spot 600 north of the test
//! cell's centre, the player at its entry (1080 away). The actor log's
//! `OnHit` gives the source's and aggressor's form ids
//! (`hit#<source><<aggressor>`).
//!
//! **Round 1, run 2026-11-12** (`research/test-mod-explosions.md`): the
//! placed reference's life, damage by distance (1 − (d / r)², d to the feet)
//! and through armor, the `Unarmed` `OnHit` beside the explosion's: applied.
//! Shading (nobody was hurt, even in the open, from an explosion on the
//! floor) and knockdowns (held NPCs didn't move) stayed open.
//!
//! **Round 2** (this source):
//!
//! - **ShadeLifted**: `dunHighGateFlamingThrallFireBallExp01` at a marker
//!   0, 60 and 120 above the floor, an NPC 90 north, one 200 north behind it
//!   and one 200 east in the open.
//! - **FreeKnockdowns**: NPCs left free (no `SetDontMove`), an explosion 50
//!   west of each, 30 up: the ballista explosion (no knockdown flag, force
//!   20) as a control, "Always" at full health, "By Formula" at 100%, 60%,
//!   45% and 20%. `GetKnockedStateEnum` through a probe, and how far it moved
//!   in 0.3 s.
//!
//! | Reading | Ours predicts |
//! | --- | --- |
//! | ShadeLifted | at every height: 90: about 60 × 0.92 and the fire; behind: nothing, only `OnHit`; open: about 60 × 0.61 and the fire |
//! | FreeKnockdowns | ballista: no; Always: yes; By Formula: no at 100% and 60%, yes at 45% and 20% |
//!
//! **Round 2, run 2026-11-12** (`research/test-mod-explosions.md`):
//! damage from 60 and 120 up matched ours to the thousandth; at 0 up nobody
//! was hurt again. The NPC behind another was hurt as the one in the open:
//! actors don't shade (applied). "Always" knocked a free NPC down; "By
//! Formula" without damage knocked nobody down at any health (applied: the
//! share of health taken).
//!
//! Round 2's dry run in our engine (2026-11-12): nothing shaded (whether
//! created actors' capsules follow a script's `MoveTo` is open, as after
//! round 1's), and the probe's 2 s read By Formula at 45%
//! as standing (our NPC got up within it): read SE's knockdowns with that
//! in mind.
//!
//! Run as autotest24: load a throwaway save, wait for the "running"
//! notification and leave the game alone until the "done" box (2 tests,
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
