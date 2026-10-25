# Reference setters: SetOpen, SetDestroyed, SetNoFavorAllowed, SetScale, GetHeadingAngle

Public pages consulted 2026-10-24 (Papyrus Index / community CK wiki text,
summarized), https://papyrus.bellcube.dev/skyrimse/script/objectreference/function/
`setopen/` (opens or closes; opening a door also removes its lock level),
`setdestroyed/` (sets or clears the destroyed flag), `setnofavorallowed/`
(followers refuse to operate it, loot it or pick its lock), `setscale/` (does not
change collision; unloaded actors keep colliding at the old size),
`getheadingangle/` (degrees, -180..180: add it to the reference's z rotation to
face the other). All of the setters are `void`; the old stubs returned a bool.

Implementation (`src/ref_state.rs`, `animated.rs`, `destruction.rs`; inference
where marked):
- `SetOpen`: already in the requested state does nothing. Animated doors toggle
  (as the player's use does, so they stay open across loads); objects with a
  behaviour graph get the `Open` / `Close` event. Opening a door clears its lock.
  No activation events (inference: the page names none).
- `SetDestroyed(true)`: a reference with a destruction definition goes to its
  first stage with Destroy Object (`DSTD` 0x04), else its last, at zero health,
  with no explosion, debris or `OnDestructionStageChanged` (inference: setting a
  flag is not damage). `false` is `ClearDestruction`.
- `SetScale`: replaces the reference's own scale (`XSCL`); an actor keeps NPC and
  race heights on top. Loaded instances are resized about the reference's
  origin; collision is left alone, as the page says. Cells load the override.
  The player is not rescaled. Saved as `RSCL`.
- `SetNoFavorAllowed`: stored (and saved as `NOFV`); nothing asks followers for
  favors yet, so nothing reads it.
- `GetHeadingAngle`: `atan2(dx, dy)` minus the heading, wrapped to -180..180.

Tests: synthetic records; 0 / 90 / 180 / -90 heading cases; payload and
legacy-load tests for `RSCL` and `NOFV`.
