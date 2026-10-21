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
- NPCs regenerate their `HealRate` percent of their health a second out of
  combat, which is 0 for NPCs (`actor-values.md`: they don't heal); one that
  unloads hurt is given what that rate would have restored meanwhile (real
  seconds). Waiting and sleeping also advance that
  healing through `heal_away`: each game hour adds 180 seconds of recovery
  (`HOUR_SECONDS`, currently fixed at `3600 / 20` in `src/sleep.rs`).
- Game-start quest scripts call `MoveTo` on 17 references once it works (MG01
  puts Faralda at the College bridge and the apprentices in the Hall of the
  Elements, the Companions are gathered in Jorrvaskr...).

## Open questions

- Health and stamina damage scripts do to an unloaded actor
  (`DamageActorValue`) is kept and taken when it loads, as if done then:
  brought to nothing it dies (the essential bleed out) as it loads, with
  no killer. Whether the game kills it while away (`OnDeath` sooner, the
  body where it was) is unconfirmed.

- Living actors' positions aren't kept when they unload: their schedules (or a
  script move) decide where they are next.
- Whether the game's unloaded regeneration follows these timing rules,
  including waiting / sleeping, is unconfirmed; fast-travel recovery remains
  to be established.

- Doors actors open (walking up to an animated door, `update_animated`) close
  again by themselves: once nobody walking or the player is within 160 units for
  2.5 seconds (`CLEAR_DISTANCE`, `CLOSE_DELAY`, chosen by hand; no public source
  gives the game's values). Whether the game also closes a door an NPC left open
  while its cell is unloaded, and whether it reloads open, is unconfirmed. We keep
  such a door open across unloading (`ADOR` in saves) and let it close once clear
  after loading. UESP's DOOR page (revision 2574204,
  https://en.uesp.net/w/index.php?oldid=2574204) lists the `FNAM` flags, including
  "Automatic Door" (0x02) and "Minimal Use" (0x08), with no behaviour; CommonLibSSE's
  `TESObjectDOOR.h` names the same bits. The engine reads no door flags, so every
  animated door opens and closes this way, whatever its "Automatic" bit. Whether
  that bit gates the auto-open (and "Minimal Use" the activation prompt) is
  unconfirmed.
