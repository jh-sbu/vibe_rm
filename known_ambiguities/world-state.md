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
  restored meanwhile (real seconds). Waiting and sleeping also advance that
  healing through `heal_away`: each game hour adds 180 seconds of recovery
  (`HOUR_SECONDS`, currently fixed at `3600 / 20` in `src/sleep.rs`).
- Game-start quest scripts call `MoveTo` on 17 references once it works (MG01
  puts Faralda at the College bridge and the apprentices in the Hall of the
  Elements, the Companions are gathered in Jorrvaskr...).

## Open questions

- Living actors' positions aren't kept when they unload: their schedules (or a
  script move) decide where they are next.
- Whether the game's unloaded regeneration follows these timing rules,
  including waiting / sleeping, is unconfirmed; fast-travel recovery remains
  to be established.
