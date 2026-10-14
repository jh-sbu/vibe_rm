# Falling damage

Implemented in `src/falling.rs` (the fall's height from `src/player.rs`).
Sources: UESP's Skyrim:Damage (the formula with the `fJumpFall*` settings,
Cushioned, "landing in moderately deep water", difficulty), the settings'
values in `Skyrim.esm` (player 600 / 0.1 / 1.45, NPCs 450 / 0.1 / 1.65),
the Cushioned perk (entry point 58, x0.5).

## Choices made without a source

- **Height**: from the highest the player's capsule got since leaving the
  ground to where it lands, centre to centre. Moved more than 256 units in
  one update (a load door, `MoveTo`, `setpos`), the fall starts over.
  `fJumpFallVelocityMin` (700) isn't used: UESP doesn't say what it does
  (a speed past which the actor counts as falling, perhaps).
- **Water**: a landing spares the player when the water's surface is at or
  above the capsule's middle; UESP says only "moderately deep".
- **Difficulty**: none is modelled, so falls do Adept's damage (UESP: less
  below Adept, no more above).
- Becoming ethereal or being paralyzed on landing (UESP: both spare the
  player) aren't checked yet.
