# Falling damage

Implemented in `src/falling.rs` (the fall's height from `src/player.rs`).
Sources: UESP's Skyrim:Damage (the formula with the `fJumpFall*` settings,
Cushioned, "landing in moderately deep water", difficulty), the settings'
values in `Skyrim.esm` (player 600 / 0.1 / 1.45, NPCs 450 / 0.1 / 1.65),
the Cushioned perk (entry point 58, x0.5).

## NPC integration blocker

[UESP Skyrim:Damage](https://en.uesp.net/w/index.php?oldid=3485809),
"Fall Damage", explicitly documents NPC damage as
`((height - 450) * 0.1)^1.65 * modifiers`, and says to append `NPC` to the
three game-setting names. `fall_damage` already supports those settings.
The missing part is actual NPC falling: `ActorRuntime::stand_on_ground`
(`src/ai/mod.rs`) eases a short collision-ray offset above the navmesh, while
movement follows navmesh height. It has no airborne vertical velocity or
landing event. This is an observed implementation limitation, not uncertainty
about UESP's formula. Applying damage to a drop between navmesh positions would
also mistake path-height changes or relocation for physical landings. A vertical
movement/landing integration must precede NPC fall damage; none was fabricated
from the current navmesh movement. NPC-specific water, paralysis and perk
interactions still need permitted behavioral evidence. No engine binary was
examined.

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
