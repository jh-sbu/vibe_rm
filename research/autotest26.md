# autotest26 in Skyrim SE (run 2026-11-13)

`test-mod autotest26` (`crates/tools/src/test_mods/autotest26.rs`): a
lake's current, autotest25's current tests again. The same throwaway save
as autotest20 and autotest25.

## Getting there

The first run of autotest26 (and autotest25's current tests) never reached
the lake. Each `Player.MoveTo` brought up a loading screen and left the
player where they were (outside Riverwood), even to the dry-land marker on
the shore. The reason: the lake is outside Skyrim's playable area.
Skyrim.esm's region `BorderRegionSkyrim` (`000C5859`, Tamriel) has an area
(`RPLD`) that holds Riverwood but neither the lake's marker nor the shore
marker. Two ordinary regions hold both (`WeatherReach` and
`Brie2npassPineForest07`). The site is in the Reach's southern mountains,
about 11000 units south of Lost Valley Redoubt and Bard's Leap Summit and
16000 west-north-west of Glenmoril Coven.

The user then ran it with `bBorderRegionsEnabled=0` (`[General]`,
`Skyrim.ini` in My Games; Skyrim's default is 1, as the
[STEP forum's list of INI defaults](https://stepmodifications.org/forum/topic/6314-ini-defaults/)
shows). Each move went straight there. So **with border regions enabled,
SE refuses a script's `MoveTo` to a spot outside the border region**, after
starting a load. Without them it goes. The user put the setting back
afterwards.

## Readings

The site: Update.esm's `CellWaterCurrentMarker` `01002B8A` in Tamriel
(-23, -12), its heading 5.7° (`DATA` rotation z, 0.1 radians). The water
there is 372 deep at surface 6000, of water type `RiverWaterFlow`
(Update.esm `01001232`, with the flow-map texture `RiverFlow.dds`). The
player, then an NPC, were left still at each point for 10 s. Drift along
the marker's heading, across it, and the feet against the surface:

| point | player at 10 s | NPC at 10 s |
| --- | --- | --- |
| marker | 733.7 along, −0.6 across, −93.6 | 723.1, 1.3, −92 |
| 512 along | 733.0, −0.5 | 721.2, 1.3 |
| 512 back | 736.2, −0.6 | 722.3, 1.3 |
| 1024 along | 621.8, −58.4 (turned off at 7 s) | 645.4, −82.5 (turned off at 7 s) |
| 1024 back | 733.4, −0.8 | 720.8, 1.3 |
| 512 right | 315.2, stopped from 5 s | 350.8, stopped from 5 s |
| 512 left | 717.3, −0.6, at −122.6 | 718.5, 1.3 |

Every series ran at a steady pace from the first second: 70 to 72 a
second. From the second reading to the tenth the player made 70.7 a second
at the marker and the NPC 70.8. The first second read about 85 to 98:
`MoveTo` returns after a short while in the current. The user watched the
run. At 1024 along the swimmer hit the rocks by the cliff face and was
turned off its line. At 512 right it washed up on a strip of bed higher
than the lake around it, where the feet rose 1 to 3.

- *Settled*: **the current flows along the marker's heading**: drift
  across it was under 1.4 units in 10 s (0.1° or less) wherever the
  swimmer was free.
- *Settled*: **its speed is about 71 units a second**, the same for the
  player and an NPC, at every free point out to 1024 along and 512 across,
  and at 122.6 under as at 92. No fall-off with distance from the marker
  over that area. The swimmer moves at once (no build-up after the first
  second).
- *Settled*: a still swimmer keeps its depth while carried (the player at
  −122.6 throughout), as autotest25 found in still water.
- *Partial*, where the speed comes from: nothing in the marker carries it
  (only `NAME` and `DATA`). None of `RiverWaterFlow`'s `DNAM` floats is
  near 71, and UESP's WATR page
  ([revision 3507781](https://en.uesp.net/w/index.php?oldid=3507781))
  names no flow speed. Whether other flowing waters run at other speeds is
  unknown.
- The data's pattern: `RiverWaterFlow` is the water of 613 cells, 558 of
  them with a current marker; `DefaultWaterFlowBlend` of 14, 13 with one.
- *Settled* (found 2026-11-13, after the run): **the current didn't need
  a flowing water type**. The water type `RiverWaterFlow` is Update.esm's,
  set by its override of the cell (`XCWT`). Skyrim.esm's own record of
  the cell has none, so the cell takes Tamriel's `DefaultWater` (`NAM2`
  `00000018`, no flow map). autotest26's plugin overrides the cell
  (`places::worlds`, built from Skyrim.esm alone), so in this run the lake
  was `DefaultWater`, and the current flowed all the same. autotest20's
  lake (-5, -14) was the same (Update.esm's `RiverWaterFlow`, overridden
  away), and its current was plain to see. A marker in a cell of still
  water makes a current. Whether a flowing type without a marker does
  isn't known.

## Compared with ours (2026-11-13)

Ours had no currents: its dry run read no drift anywhere. Since then ours
has a cell's current along its marker's heading at 71 units a second
(`known_ambiguities/water.md`, Currents). Run again, ours carried the
player and the NPC 70.9 a second along the heading at every point. At
1024 along, the player was turned off its line at about 7 s as in SE and
the NPC stopped. At 512 right the NPC stopped on the higher strip as in
SE, while the player climbed it and went on.
