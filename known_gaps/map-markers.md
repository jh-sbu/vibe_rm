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
- **The compass frame** for a location is `LocationMarkers + TNAM`
  (discovered) or `UndiscoveredMarkers + TNAM`: the clip has a frame for
  each `IconTypes` entry, in that order (the icons drawn match the types).
  Types 0 (the empty marker) and from 60 (door, quest and player markers)
  aren't shown as locations.
- **On the compass:** discovered markers within
  `fHUDCompassLocationMaxDist`, at full alpha; undiscovered ones within
  `iMapMarkerVisibleDistance`, fading in from there to
  `iMapMarkerFadeStartDistance`. Every marker's scale is 100. Indoors there
  are none.
- **`FNAM`** 1 is taken as on the map from the start (the holds' capitals
  have it). 2 (the military camps, the DLC ferries) as travel allowed
  without discovering, and 4, are kept for the map.
- **`AddToMap(true)`** counts as discovered (fast travel allowed) without the
  banner; "Map updated." shows when a marker wasn't on the map yet.
