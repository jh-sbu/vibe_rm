# autotest25 in Skyrim SE (run 2026-11-13)

`test-mod autotest25` (`crates/tools/src/test_mods/autotest25.rs`):
swimming in two authored interior cells, then a lake current. The same
throwaway save as autotest20 (a level 6 character). Every test ran; no
crash. Both cells have the water's surface at z 0, steps of exact depth
and a ramp of 32-unit treads rising 4.76 each (see the module doc).

*Settled*, *Partial* and *Fault* as in `research/autotest23.md`.

## The level-water cell: no water

`VrmAt25LevelWater` (`DATA` 0x3, interior and has water, `XCLW` 0, `XCWT`
`DefaultIntWaterWaistHeightStill`) had no water. The user saw none, and the
detector read dry on every step to 300, on the ramp, for the player and for
the NPC. Feet stood on each floor. *Settled* for an authored cell: **SE
makes no water from an interior cell's water level** (with this water
type). Vanilla interiors get theirs from placed water. 19 vanilla interiors
have the flag and a finite `XCLW` with no placed water (`BronzeWaterCave`,
`RiftenRatway01`, `AnisesCabin01`, `Mzulft01`, ...). Whether any of them
shows water at that level in SE wasn't looked at.

## The placed-water cell

`VrmAt25PlacedWater`: six `Water2048DefaultIntWaistStill` at z 0, the cell
flagged as having water with the "none" `XCLW`. The detector worked
throughout (events and gains agree). Events came 0.2 to 0.5 s after a
move, sooner than autotest20's 0.7 to 0.8 outdoors.

**Steps, moved by `MoveTo`** (feet against the surface at 1 and 3 s):

| step | up | down |
| ---: | --- | --- |
| 40 to 80 | dry, on the floor | dry, on the floor |
| 90 | dry, −89.7 | dry, −89.6 (events at 1.1 s) |
| 95 | swimming, −94.7 | dry at 1 s, swimming by 3 s, −93.2 (dry at 0.3, swimming at 1.3) |
| 100 to 103 | swimming, −92.5 to −94.1 | swimming, −92.4 to −93.6 |
| 104, 105 | swimming, −100.4, −95.0 | swimming, −94.0, −101.5 |
| 110, 120, 180, 300 | swimming, −100, −110, −176, −290 | the same, 180 at −170 |

The NPC: dry to 80 (on the floor), swimming from 90 (feet −90.2) to 300,
always on or within 1.4 of the floor; dry again on the dry step.

**Float** (player to the 300 step, NPC to the 180): the player held
−299.7 and the NPC −181.4 for 20 s, both swimming.

**Ramp, moved by `TranslateTo`** at 40 units/s (its feet 3 above the
treads going in; held 93 under going out until the treads came up to that):

- In: dry down to the sample at bed 77 (feet −74). Swimming began at 37.1
  s, with the bed 96.2 and the feet −95.6 under the player then.
- Out: swimming at −93 over beds 220 to 96. Still swimming at the sample
  with bed 72.4 and feet −67.7. Dry at 35.5 s, with the bed 62.9 and the
  feet −55.4.

### What they show

- *Settled*: **swimming begins with the feet about 90 under the surface**.
  The player was dry with the feet at 89.7 and swimming at 94.7; the NPC
  swam at 90.2. The ramp's entry, at about 92 to 96 after the detector's
  lag, agrees. The player and NPCs use one depth.
- *Settled*: **moving continuously, the state holds much shallower going
  out**: still swimming at 67.7 under, dry by about 55 (lag included), so
  the exit lies between 55 and 68. It was on stairs, with feet and bed 3
  apart, so whether the feet or the bed decide it isn't separated. Moved by
  `MoveTo`, the state is judged afresh (dry at 89.6 coming down from
  swimming), so autotest20's teleport readings didn't show this
  hysteresis.
- *Settled*: **a still swimmer doesn't rise**. Placed 100 to 103 deep, the
  player came up to about 92.4 to 94 under. Placed deeper, it stayed: about
  10 over the floor (110 to 300), unchanged for 20 s. The NPC stayed on the
  floor at every depth. autotest19 and autotest20's "float about 92 under"
  holds only where the bed is about that deep.
- autotest20's lake bound (dry at bed 100.4, swimming at 105.1) doesn't
  match the 90 here. There the feet stood 24 to 37 above the terrain's
  heights for reasons unknown, so the lake's bed depths weren't the feet's.
  These authored readings replace them.

## The current: *Fault*

Both tests read the player and the NPC still in the placed-water cell's
dry step: the drift is the marker's offset from (0, 0), the feet at 64,
constant. `GoCurrent`'s branch ran (its "disabled 0 fish" line), so
`Player.MoveTo` to our marker in Tamriel was called, and it didn't move
the player. The NPC's `MoveTo` didn't move it either. The markers were
persistent `XMarkerHeading` references in Tamriel's persistent cell, as
autotest19's and autotest20's lake markers (which worked), but floating 92
under the surface, not on the ground, and the move came from an interior.
Which of these stopped it isn't known. The end line's player distance,
−2147483648, is `Math.Floor` of a distance across worldspaces (overflowed).

## Compared with ours (2026-11-13)

- Interior water levels: ours makes water from an interior cell's `XCLW`
  (`src/engine.rs`, `add_water`) and swam there; SE makes none.
- Placed water: ours has none (`plans/ROADMAP.md`); SE swims in it.
- Entry depth: ours swims with the feet more than 100 under
  (`src/water.rs` `SWIM_DEPTH`); SE about 90.
- Exit: ours leaves where the bed is shallower than 96; SE keeps swimming
  to between 55 and 68 when moving continuously.
- Floating: ours lifts every swimmer to 92 under; SE leaves a still
  swimmer at its depth.
