# Map markers and the compass

Implemented in `src/map_markers.rs` (the markers, discovery) and
`src/swf_ui.rs` (`compass`). Sources: the data (`XMRK`, `FULL`, `TNAM`,
`FNAM` on references); the game settings `iMapMarkerVisibleDistance`
(12500), `iMapMarkerRevealDistance` (1000), `iMapMarkerFadeStartDistance`
(10000), `fHUDCompassLocationMaxDist` (20000), `sDiscoveredText`,
`sMapMarkerAdded`; `hudmenu.swf` (`SetCompassMarkers` reads
`CompassTargetDataA` as heading, alpha, frame and scale for each marker; the
"Compass Marker" clip's frame labels Quest 1, QuestDoor 2, PlayerSet 3,
Enemy 4, LocationMarkers 5, UndiscoveredMarkers 72); `map.swf`
(`MapMarker.IconTypes`, which `TNAM` indexes: 2 is the town icon, 15 the
mine's, 40 Whiterun's); the CK wiki (`AddToMap`, `EnableFastTravel`,
`IsMapMarkerVisible`, `GetMapMarkerVisible`).

## Confirmed in the game (`test-mod autotest2`, 2026-10-13)

- `FNAM` is two flags: 1 on the map from the start (the holds' capitals), 2
  travel allowed without discovering. A far marker with 1 was visible and
  not travelled to; with 3 (the military camps), visible and travelled to;
  with 0, neither. Skyrim.esm's markers have only 0, 1 and 3.
- `AddToMap(false)` puts a marker on the map without travel to it,
  `AddToMap(true)` with; neither counts it in Locations Discovered.

## Unconfirmed: choices made

- **Coming near.** A marker comes onto the map when the player, in its
  worldspace, is within `iMapMarkerVisibleDistance`, and is discovered within
  `iMapMarkerRevealDistance` (straight-line distances to the marker,
  checked twice a second). Disabled markers are neither.
- **Discovering** shows the quest banner with the name and `sDiscoveredText`
  ("DISCOVERED: RIVERWOOD"), counts Locations Discovered and plays the
  discovery music once over the location's: `MUSDiscoveryCity` for cities
  and their castles (types 1, 35 to 52), `MUSDiscoveryTown` for towns and
  settlements (2, 3), `MUSDiscoveryDungeon` for caves, forts, ruins, lairs,
  mines, towers, strongholds and giant camps, `MUSDiscoveryGeneric` for the
  rest. Which type the game picks for which marker isn't published.
  `iXPRewardDiscoverMapMarker` isn't given (no character XP for it).
- **The compass frame** for a location is `LocationMarkers + TNAM - 1`
  (discovered) or `UndiscoveredMarkers + TNAM - 1`: the clip has an icon on
  frames 5 to 64 and 72 to 131, sixty each, taken as `IconTypes` 1 (the
  city) to 60 (Castle Karstaag) in that order (the empty marker has no
  frame; the icons drawn match the types). Types 0 and from 61 (door, quest
  and player markers) aren't shown as locations.
- **On the compass:** discovered markers within
  `fHUDCompassLocationMaxDist`, at full alpha; undiscovered ones within
  `iMapMarkerVisibleDistance`, fading in from there to
  `iMapMarkerFadeStartDistance`. Every marker's scale is 100. Indoors there
  are none.
- **`FNAM` 4** (the DLC's) is kept for the map.
- **`AddToMap`**: "Map updated." shows when a marker wasn't on the map yet.
  One added with travel isn't discovered: coming near it later discovers
  it (the banner, the statistic) as any other.

## Custom destination

The map movie supplies icon 65 and the compass movie frame 3. The public
CommonLibSSE-NG `PlayerCharacter::playerMapMarker` handle (MIT; layout lead
only, see `plans/map-menu.md`) suggests a reference in Skyrim. This engine
stores one optional `(map owner worldspace, position)` as plain state: vanilla
Papyrus declares no native that exposes this reference. The representation
is a choice, not a claim about Skyrim's save format. Native saves use the
optional PMRK section; absent means no destination. Invalid worlds (null) and
nonfinite coordinates are rejected before applying the snapshot.

The user observed in vanilla Skyrim SE (2026-11-01) that reaching the custom
destination does not remove it. Replacement occurs only after another map
click and the Move It answer, immediately at that click position. Empty-map
placement without an existing marker is immediate. These are observed behavior,
not unresolved choices (`research/ui/map-menu.md`).
