# World state across cell loads

Implemented in `src/world_state.rs` (dead actors, wounds, open doors, script
moves), with enable state in `Engine::is_disabled` / `set_disabled`. In memory
only: nothing is written to disk until saves exist.

## What is known

- Enable parents (`XESP`: parent, flag 1 = opposite) decide a reference's
  state unless scripts set the reference itself; `Enable` / `Disable` on a
  parent reach its children however deep.
- `ACHR` record flag `0x200` is "Starts Dead": such actors (C04's Silver Hand
  bodies, `TreasCorpseSkeletonRigid`) are placed as corpses, quietly (no
  `OnDying` / `OnDeath`).
- NPCs regenerate `health_regen` percent (race `DATA`) of their health a second
  out of combat; one that unloads hurt is given what that rate would have
  restored meanwhile (real seconds).
- Game-start quest scripts call `MoveTo` on 17 references once it works (MG01
  puts Faralda at the College bridge and the apprentices in the Hall of the
  Elements, the Companions are gathered in Jorrvaskr...).

## Open questions

- Nothing moves clutter about yet (no dynamic Havok objects), so there is no
  knocked-over state to keep; only ragdolls are simulated.
- An object moved from an unloaded cell into a loaded one appears only when a
  cell holding it loads again; one moved between two loaded cells stays drawn
  with the cell it came from (and goes when that cell unloads).
- Living actors' positions aren't kept when they unload: their schedules (or a
  script move) decide where they are next.
- Regeneration while away is by real time, not game time (waiting, sleeping
  and fast travel don't heal them).
- Looping sounds of references enabled in place start (and stop when
  disabled); not heard in a test (headless runs have no audio device).
- Doors opened by actors close behind them and aren't remembered; locks and
  taken items were already kept (`ScriptState`).
