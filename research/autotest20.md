# autotest20 in Skyrim SE (run 2026-11-13)

`test-mod autotest20` (`crates/tools/src/test_mods/autotest20.rs`):
autotest19's open swimming and charging questions. A throwaway save (a
level 6 character) outdoors; Survival Mode's plugin was loaded. Every test
ran; no crash. The lake: Tamriel cell (-5, -14), surface 600, the markers
on a line out from the shore. Bed depths below are the surface minus the
marker's height, which is the terrain's (`LAND`) height under it.

*Settled*, *Partial* and *Fault* as in `research/autotest23.md`.

## Readings

**Dry** (the detector on land, the player and an NPC): `-D` from both
readings (the events and the skill gains) at 1 and 3 s. The dry effect
started 0.2 s (player) and 0.4 s (NPC) after `AddSpell`. *Settled*: the
detector works. An ability effect whose condition changes fires
`OnEffectStart` and `OnEffectFinish`, and the gains agree with the events
everywhere below.

**SwimUp** (the player, shallow to deep; 2 hostiles disabled first):

| bed | swimming | feet − surface (1 s / 3 s) |
| ---: | --- | --- |
| 43.1 | no | −19.0 / −19.0 |
| 60.6 | no | −37.1 / −37.1 |
| 81.4 | no | −42.6 / −42.5 |
| 90.2 | no | −45.9 / −45.8 |
| 95.5 | no | −49.5 / −49.3 |
| 100.4 | no | −63.8 / −63.7 |
| 105.1 | yes (events at 0.8 s) | −92.7 / −92.7 |
| 110.2 | yes | −93.1 / −93.1 |
| 120.3 | yes | −93.9 / −93.9 |
| 142.8 | yes | −109.7 / −109.7 |
| 181.3 | yes | −149.3 / −149.3 |

**SwimDown** (the player, deep to shallow, starting from the shore):
swimming from 181.3 to 105.1 and dry from 100.4 to 43.1. The state changed
about 0.7 s after each move. Feet: −182.5 at 181.3, −109.7 at 142.8, about
−93 at 120.3 to 105.1, and the SwimUp heights again from 100.4 down.

**SwimBreathing** (`AbWaterbreathing`, shallow to deep): the same as
SwimUp. Dry to 100.4, swimming from 105.1. The heights match to within
about 3 units.

**SwimNpc** (an NPC, the player on the shore): dry to 100.4, swimming from
105.1 (events at 0.7 to 0.8 s); back at 43.1, dry again. Its feet at 3 s:
−41, −40, −45, −49, −57, −78 for the dry markers. While swimming they were
at about the bed's depth (−103, −110, −120, −142, −181), with the NPC
200 to 220 units from the marker. On the dry markers it was 22 to 44 off.

### What they show

- *Settled* for this lake: the swim state begins between bed depths 100.4
  and 105.1. The bound is the same for the player, for an NPC and with
  Waterbreathing, so **Waterbreathing doesn't change the state**, and
  **NPCs use the player's depth** (for this humanoid race and scale).
- *Settled*: moved by `MoveTo`, the state follows the new spot within
  about 0.8 s in both directions. Leaving shows the same bound (dry at
  100.4, swimming at 105.1). A teleport may re-judge the state from
  scratch, so this doesn't show whether swimming *out* has hysteresis.
- *Partial*, the floating height: a swimmer moved where the bed is 105 to
  120 deep held at about −93, as autotest19's −92. Moved deeper, it stayed
  deeper: the player at −110 (bed 143) and −149 or −183 (bed 181), the
  NPC at about its placed depth throughout. Nothing changed between 1 and
  3 s. So a still swimmer doesn't rise to the float height from any
  depth, at least not within 3 s. autotest19's 180 rose slowly (−148 to
  −132).
- *Partial*: where the player didn't swim, the feet were above the bed by
  24 (bed 43) to 37 (bed 100). autotest19's other lake had them on the bed
  at 40 to 80. Something here stands higher than the terrain, perhaps the
  large `MountainTrim02RiverbedEdge01` 410 units from the line. Or SE
  lifts a wading actor. Not settled.
- The NPC's 200-unit drift while swimming was the lake's **current**.
  The user saw it plainly in the run. A `CellWaterCurrentMarker`
  (Update.esm `01002A99`, a static `00000061` with only a position and a
  heading of 103°) lies 48 units from the 100.4 marker. The deep
  swimmers' heights (an NPC holding its placed depth while carried off)
  may be the current's doing too. The player's drift wasn't read.

## Water currents

The engine doesn't model currents at all. What the data shows:
Update.esm places about 570 `CellWaterCurrentMarker` references in
Tamriel. They reach 1326 of its 11186 cells counting each marker's own
and neighbouring cells, and every lake line `places::shore` finds has one
within 2048 units. autotest19's lake (-3, -15) has one too (`01002A93`).
Each holds only `NAME` and `DATA`: no `XCVL` velocity, which UESP's REFR
page ([revision 2354868](https://en.uesp.net/w/index.php?oldid=2354868))
finds once in the whole game. UESP's WATR page
([revision 3507781](https://en.uesp.net/w/index.php?oldid=3507781)) names
a displacement velocity but nothing for flow. So what sets a current's
strength and reach, and whether the marker's heading is its direction, is
open. Still water for a later swimming batch can't be found by excluding
markers. A batch could measure the current instead: the drift of a still
swimmer by position and time near a marker, against its heading.

**Charge1500**, **Charge2500**: *Fault*. The fighter never engaged the
held target. After 8 s it was 218 to 358 units off, in combat state 2
(searching), drawn, the target unhurt. Moved away, the target was never
chased: the fighter walked further off (to 2469 and 3149), turned back at
a walk, and had left combat (state 0) by 14 s. No `S` or `R` in either.
autotest19's Draw page saw the same (a fighter given `StartCombat`
against a held commoner searched). A one-sided `StartCombat` against a
restrained, unaggressive target leaves the fighter searching, so the
charge is still unobserved.

## Compared with ours (2026-11-13)

- Entry depth: ours swims when the feet are more than 100 deep
  (`src/water.rs` `SWIM_DEPTH`). A `MoveTo` to the 100.4 marker would put
  ours' feet 100.4 deep, so ours swims there and SE doesn't. The
  difference is under 5 units: SE's bound is in (100.4, 105.1].
- Exit: ours keeps swimming until the bed is shallower than 96 (4 units of
  hysteresis). Moved to 100.4, ours stays swimming and SE is dry. The
  reading doesn't settle real hysteresis (see above).
- Waterbreathing and NPCs: **agree** (ours uses one depth for everyone and
  ignores Waterbreathing).
- Float height: ours holds every swimmer's feet at 92 under. SE agrees
  where the bed is 105 to 120, but a swimmer placed deeper stays deeper
  (player and NPC).
- Wading heights: ours stands on the ground. Here SE's feet were 24 to 37
  above the terrain, cause unknown.
- Charging: no reading.

## Faults and open items, for a follow-up batch

- The charge again: the target must fight back (both given `StartCombat`,
  as autotest19's Duel pair, which did engage), or be left free.
- Swimming out with hysteresis: walking or pushing an actor out of the
  water (a Travel package to the shore) rather than teleporting.
- What holds the wading player above this lake's bed, and whether a
  swimmer placed deep ever rises (a longer watch than 3 s).
- The current: a still swimmer's drift (direction, speed) by distance
  from the marker and against its heading, and whether it carries
  wading actors.
