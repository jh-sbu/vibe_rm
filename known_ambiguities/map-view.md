# The world map (map.swf and the camera under it)

Implemented in `src/map_view.rs` (the camera, the LOD scene, the markers'
projection), `src/swf_ui/map_menu.rs` (the movie) and `crates/esp/src/
worldspace.rs` (`MNAM`, `ONAM`). Sources: `WRLD` `MNAM` / `ONAM` / `PNAM` in
Skyrim.esm and Dragonborn.esm (`vrm-tool world-maps`; layout from
`refs/CommonLibSSE-NG` `TESWorldSpace.h` and the UESP page
https://en.uesp.net/w/index.php?oldid=3192583); the movie's ActionScript
(`MapMenu`: `SetNumMarkers`, `MarkerData`, `CreateMarkers`,
`GetCreatingMarkers`, `RefreshMarkers`, `SetSelectedMarker`,
`ClickSelectedMarker`, `SetDateString`, `ShowJournal`; its calls
`ClickCallback`, `MarkerClick`, `ToggleMapCallback`, `OpenJournalCallback`,
`CurrentLocationCallback`, `SetLocalMapExtents`; `MapMarker.IconTypes`);
`vrm-tool map-markers` for the markers' flags and places.

## From the data

- Only Tamriel (cells -30,15 to 40,-40; camera 50000 to 80000, pitch 50) and
  Solstheim (8,17 to 17,1; 31000 to 48000, pitch 70) have `MNAM`; every child
  worldspace (the cities, the dungeons' worlds) has `PNAM` bit 4, Use Map
  Data, and takes its parent's. The usable width and height are 0 in both.
- `ONAM` (scale and offset, 1 and 0 for the cities) is set for the dungeons'
  worldspaces: Blackreach 2.5 and (-8192, 73728), Alftand 0.001 and so on.
- `map.swf`'s `IconTypes` has 67 entries: 0 the empty marker, 1 to 59 the
  places (Castle Karstaag is 59), 60 empty, 61 door, 62 and 63 and 64 the
  quest targets, 65 the player-set marker, 66 "you are here".

## Unconfirmed: choices made

- **The map's extent** is the corner cells' outer edges: from the north-west
  cell's west and north edges to the south-east cell's east and south edges
  (Tamriel: 71 by 56 cells). Whether the game counts the corner cells in
  (as here) or stops at their corners isn't published.
- **The camera** looks north, down at the pitch `MNAM` gives (50 degrees above
  the horizon on opening) at what it centres on: the player (the middle of the
  map from an interior). Its height over the centre is `MNAM`'s minimum to
  maximum, the zoom running between them, opening at the highest. Whether
  those are heights over the ground, and whether the pitch is from the horizon,
  isn't published. The field of view is whatever shows the map's whole height
  from the highest at straight down (110 degrees in Tamriel, 72 in
  Solstheim; 40 to 120 allowed), a guess: it makes the limits mean something,
  and the view is more distorted at its edges than the game's looks.
- **Tilt** runs from 25 to 89 degrees (not 90: straight down has no up for
  the camera); the keys and wheel below are this engine's.
- **Drawn:** only the distant LOD (terrain and objects, `world::lod`, a block
  size by distance from the camera including its height), lit by a fixed sun,
  no sky, fog or clouds, over a dark blue where there is no land (the LOD's
  water isn't drawn). The game's map has its own lighting, clouds and sea
  (research-gated).
- **The player's marker** is on the map in the exterior worldspace they are
  in, or in the parent's at `position * scale + offset` from `ONAM` for a
  child worldspace, turned as the camera looks (degrees clockwise from
  north). Child worldspaces' other markers aren't on their parent's map.
  From an interior there is none, the map opening on the middle of Tamriel
  (the game's local map for interiors isn't built).
- **Markers on it** are those the data or the player's progress put on the
  map (`map_markers`), enabled, of type 1 to 60, placed at ground level (z 0)
  rather than at their own height. Undiscovered ones without travel allowed
  are hollow (the movie's undiscovered icon). Quest targets and custom markers
  aren't there yet.
- **Hovering** a marker (within 3% of the screen's height of the pointer, the
  nearest) selects it; the movie shows the name as the marker's description
  (`MarkerDescriptionObj.SetDescription(name, [])`, with no lines under it:
  what else the game lists there isn't known). The marker's own label field is
  left as it is: the movie's icons replace it as they change state.
- **Input:** WASD and the arrows pan, Q and E (or the wheel) zoom, Page Up and
  Page Down tilt, dragging with the left button pans, a click on a marker
  without dragging sends `MarkerClick` (which only logs: fast travel isn't
  built), Esc, Tab or M close it (straight back to the world, not to the tween
  menu), "Journal" opens the journal, "Current Location" re-centres on the
  player. The bottom bar's "Local Map" does nothing yet. M opens it as the
  control map's Quick Map; the tween menu's Map does too.
- **The movie's clock** is `SetDateString` with the journal's date text.
- **Stage.visibleRect** has the edges (`left`, `top`, `right`, `bottom`) the
  movie reads in the vendored Ruffle, as Scaleform's rectangle.
- **The LOD blocks** are placed by their own scale and translation (their
  shapes carry the scale of their level; the engine used to scale them again,
  which is why distant LOD was marked experimental); the blocks to draw come
  from a quadtree refined within 1.2 times the block's size of the camera,
  reading the camera's height in.
