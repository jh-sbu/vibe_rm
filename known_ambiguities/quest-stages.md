# Setting a stage below the current one

Implemented in `Engine::run_stage` (`src/engine.rs`, 2026-11-08).

## Evidence

- The CK wiki's `SetCurrentStageID` page (the target of `Quest.SetStage`),
  read by the user and pasted into the session on 2026-11-08 (the wiki
  blocks automated access): "To set stage to a value lower than current
  stage you should use reset() function before setstage()", and "Although
  you can't set the current stage number to a lower value, this function
  can still display the journal entry and run script fragments from lower
  numbered stages, if they hadn't previously been completed."
- Vanilla data depends on it: `MQ101Scene1`'s phase 2 fragment
  (`SF_MQ101Scene1New_000BECD4.Fragment_4`) sets stage 15 and its phase 5
  fragment (`Fragment_7`) sets 14 later. The cart horses' `Patrol1`
  packages are conditioned on 15 <= `GetStage` < 20. Were the current stage
  lowered to 14, the cart ride would stop.

## Choices made

- A stage below the current one leaves the current stage unchanged. If it
  isn't done yet it is marked done (`GetStageDone` true) and its log
  entries and fragments run once; if it is done, nothing happens.
- **The return value** of such a call stays true. The page says the
  function returns false when the stage isn't "successfully set" and is
  unchanged, but not whether running a lower stage's fragments counts as
  set. Our `SetStage` returns before the stage is applied in any case.
- Setting the current stage again, or a done stage above it, runs it again
  as before. The page doesn't cover it.

A test plugin could settle the open points: `SetStage` 20, then 10 and 10
again, reporting each return value, `GetStage` and `GetStageDone(10)`, and
whether stage 10's fragment ran each time.
