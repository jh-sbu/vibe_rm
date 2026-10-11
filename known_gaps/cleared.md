# Cleared locations

Implemented in `src/locations.rs` (`set_cleared`, `boss_died`). Sources: the
data (`LCTN` static references with the `Boss` location ref type); the CK
wiki (`Location.IsCleared` / `SetCleared`, `GetLocationCleared`); the game's
scripts (`defaultClearLocationOnDeath` and others call `SetCleared` for
locations their bosses don't clear); the game settings `sCleared`; the music
type `MUSDungeonCleared`.

## Unconfirmed: choices made

- **Clearing by bosses.** When an actor with the `Boss` ref type dies, its
  editor location is cleared once every `Boss` reference of it (and of the
  locations within it) is dead. Parent locations aren't cleared by it.
- **Clearing** (by a boss or `SetCleared`) counts in Dungeons Cleared, and
  when the player is in the location shows the banner with `sCleared`
  ("CLEARED: EMBERSHARD") and plays `MUSDungeonCleared` once. `SetCleared(false)`
  only forgets it. Cells don't respawn, so `iHoursToRespawnCellCleared` and
  the scripts' `OnReset` aren't used.
