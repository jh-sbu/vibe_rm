# Actor state

Public pages consulted 2026-10-24 (Papyrus Index / community CK wiki text,
summarized), https://papyrus.bellcube.dev/skyrimse/script/actor/function/
`setcriticalstage` (0 none, 1 goo start, 2 goo end, 3 disintegrate start,
4 disintegrate end; the two ends remove the object from the world),
`resurrect` (won't work on an actor flagged "starts dead" in the reference
window), `resethealthandlimbs`, `unequipall`, `unequipitemslot` (a biped slot
from the race), `getsitstate` / `getsleepstate` (0 not, 2 not but wants to,
3 in it, 4 in it and wants to leave; a bed with a sit marker reads sit 2 until the
actor has lain down, then sit 0 and sleep 3), `clearextraarrows` (clears arrow 3D
left on an actor), `setnotshowonstealthmeter` (the actor doesn't affect the meter
while not hostile), `setvoicerecoverytime` / `getvoicerecoverytime` (seconds).

Implementation (`src/actor_flags.rs`; inference where marked). Saved as `ACST`
(critical stages, meter exemptions, other actors' voice recovery left):
- `SetCriticalStage` stores 0..=4 (the condition `IsInCriticalStage` reads it,
  new); the two end stages disable the actor. Ash piles, goo and shaders wait for
  effect-shader rendering (ROADMAP).
- `Resurrect`: clears dead, the death record, the body's pose and wounds; a loaded
  actor gets its full health, loses its ragdoll, re-enables its capsule, drops
  bleedout and combat, and the graph resumes. The inventory is as it died with.
  The "starts dead" caveat isn't modelled (we revive it). The player: clears the
  death timer and restores health.
- `ResetHealthAndLimbs`: health to its most; nothing here cripples limbs.
- `UnequipAll`: everything worn or held except readied spells, powers and shouts
  (inference: they aren't items). `UnequipItemSlot(30..=61)`: the armor whose
  body slots cover that bit; other numbers do nothing (inference on the numbering).
- Sit and sleep: the actor's furniture phase (2 heading for or getting in, 3 in
  it, 4 getting out) when the furniture is of that kind; the player's furniture
  use isn't tracked, so reads 0.
- `ClearExtraArrows`: arrows that strike actors aren't kept attached here
  (`ai/archery.rs`), so nothing is cleared.
- `SetNotShowOnStealthMeter`: `sneak_eye` (HUD) skips the actor while it isn't
  hostile to the player; detection itself is unchanged.
- Voice recovery: the player's is the shout meter's; others' is a timer on the
  unpaused real-time clock.

Tests: synthetic records; `ACST` payload and legacy-load tests.
