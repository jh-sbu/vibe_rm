# Plan: the map's rest (controls, custom destination, Show on Map)

**Status: implemented** (2026-11-01). ROADMAP Next item 6 (UI), "The map's rest".
This plan covers the world-map parts of that entry that no other item
blocks: the map's controls as the game binds them, the player-set marker
("Custom Destination"), the journal's Show on Map, and keeping the map's
markers current while it stays open. The local map is its own plan,
`plans/local-map.md`, which builds on this one. Each numbered step (or
lettered sub-step) is one commit. Run `cargo fmt` before each.

Not in this plan, and staying where they are in the ROADMAP:

- **The local map**: `plans/local-map.md`.
- **The map's clouds, lighting, sea and fog.** These are research-gated.
  Leads from the assets: `meshes/sky/skyrimworldmapcloudbank.nif`,
  `meshes/dlc02/sky/dlc2worldmapcloudbank_low.nif`,
  `textures/terrain/worldmapoverlay_n.dds` and `worldmapoverlaysnow_n.dds`.
  No `WRLD` field names them: Tamriel has no `MODL`.
- **Fast travel's remaining timing and restrictions**
  (`known_ambiguities/fast-travel.md`).
- **Distant LOD around the loaded cells on by default** (`VRM_LOD=1`). That is
  a rendering and performance decision, separate from the map menu.
- **Gamepad input.** The map context's gamepad columns are recorded below so
  the gamepad work can reuse them.

Rules as for all work here (AGENTS.md): public sources and game assets only.
Describe the movies' behaviour in prose and don't copy their bytecode into the
repository (the disassembly in `spike/swf/out/` is ignored and stays that
way). Choices made without a source go in `known_ambiguities/map-view.md`
(the world map), `known_ambiguities/map-markers.md` (markers and the
compass). Open questions stay in this plan.

## The evidence

### `map.swf` (game asset, read through the spike's `dis` + `fold.py`)

- **`Map.MapMenu`** is the world map's marker layer, `_root.WorldMap`. Besides
  what `src/swf_ui/map_menu.rs` already drives (`SetNumMarkers`,
  `MarkerData`, `CreateMarkers`, `GetCreatingMarkers`, `RefreshMarkers`,
  `SetSelectedMarker`, `ClickSelectedMarker`, `SetDateString`), it has:
  - `ShowJournal(show)`: hides the bottom bar while the journal is open over
    the map.
  - `SetCurrentLocationEnabled(enabled)`: greys out the Current Location
    button (on PC only for the NX-mouse platform).
  - `SetJournalInput(art)`: the journal button's key art.
  - The bottom bar's buttons: `LocalMapButton` (calls
    `ToggleMapCallback`), `JournalButton` (`OpenJournalCallback`) and
    `PlayerLocButton` (`CurrentLocationCallback`). `LeftButton` and
    `RightButton` show key art only. The translation file has
    `$Set Destination`, `$Local Map` and `$World Map`.
  - A mouse press anywhere outside the bottom bar calls `ClickCallback`.
- **Callback names are shared.** Both `Map.MapMenu` instances register the same
  `GameDelegate` callback names (`RefreshMarkers`, `SetSelectedMarker` and the
  rest), and the callback table is keyed by name, so whichever registers last
  receives them. The local map's layer has to be driven by `invoke` on its
  path. Step 7a checks which instance the world map's existing `callback`s
  reach today.
- **Icon types** (`MapMarker.IconTypes`): 61 door, 62 quest target, 63 quest
  target door, 64 multiple quest targets, 65 `PlayerSetMarker`, 66
  `YouAreHereMarker`.

### `quest_journal.swf` (game asset)

- An objective entry may carry `questTargetID`. The Show on Map button is at
  full alpha when the selected objective (or the first one) has one, and at
  half alpha otherwise.
- The fifth argument of the `RequestQuestsData` answer
  (`onQuestsDataComplete`'s `abAllowShowOnMap`) shows or hides the button.
- Pressing it (the button, M on PC, or gamepad X) closes the journal
  (`CloseMenu`) and calls `ShowTargetOnMap(questTargetID)`. Without a target
  it plays `UIMenuCancel`.

### `controlmap.txt`, the "Map Menu" context (game asset)

| Event | Keyboard | Mouse | Gamepad |
| --- | --- | --- | --- |
| Cancel | as Tween Menu, Pause (Tab, Esc) | as Tween Menu, Pause | 0x2000 (B) |
| Look | | | 0x000c (right stick) |
| Zoom In / Zoom Out | | 8 / 9 (wheel) | 0x000a / 0x0009 (triggers) |
| MapLookMode | | 1 (right button) | |
| Click | | | 0x1000 (A) |
| PlacePlayerMarker | 0x19 (P) | | |
| Cursor | | 0xa (mouse move) | 0x000b (left stick) |
| PlayerPosition | 0x12 (E) | | 0x8000 (Y) |
| LocalMap | 0x26 (L) | | 0x4000 (X) |
| LocalMapMoveMode | | 0 (left button) | |
| Journal | 0x24 (J) | | 0x0004 (Back) |
| Up / Down / Left / Right | as Forward / Back / Strafe Left / Strafe Right (WASD) | | |

The engine's current keys disagree with this table in three places. E zooms
here but is PlayerPosition in the game. Q, Page Up and Page Down aren't bound
in the game at all. The game's Map Menu context has no Quick Map row; the user confirmed that
M nevertheless closes the map. It follows the gameplay Quick Map binding.

### Game settings (Skyrim.esm, `vrm-tool gmst`)

- `sPlaceMarker` "Place Marker"
- `sPlaceMarkerUndiscovered` "You have not discovered this location yet.\r\nPlace marker?"
- `sMoveMarkerQuestion` "Do you want to move your marker or remove it?"
- `sMoveMarker` "Move It", `sRemoveMarker` "Remove It", `sLeaveMarker` "Leave It"
- `sPlayerSetMarkerName` "Custom Destination"
- `sFastTravelConfirm` "Fast travel to %s?". `src/fast_travel.rs` asks for
  `sConfirmFastTravel`, which doesn't exist. Its fallback is the same text,
  so this has gone unnoticed (step 1).

### Public community research (`refs/CommonLibSSE-NG`, MIT)

These are names and layouts only, used as leads, not as behaviour.

- `PlayerCharacter::playerMapMarker` is an `ObjectRefHandle`: the custom
  destination is a reference.
- `MapMenu` has `PlaceMarker()`, a `mapMarker` handle and a
  `playerMarkerPosition`.

### UESP on behaviour

- [Skyrim:Controls](https://en.uesp.net/w/index.php?oldid=3637273), Journal:
  setting a quest active "adds its quest target(s) to your compass and your
  world and local maps".

## What exists (2026-11-01)

- `src/map_view.rs`: the camera, the LOD scene and the projection.
  - Marker rows (`MapRow`) are built once on opening, in the order
    `map_rows_for_world`, then `map_quest_rows`, then the player.
  - `world_map_point` resolves child worldspaces through `ONAM`.
  - The map's camera is saved (`Continuity::map_camera`).
- `src/swf_ui/map_menu.rs`:
  - Keys: WASD and the arrows pan, Q/E and the wheel zoom, Page Up/Down tilt.
    Esc, Tab and M close it.
  - Left-dragging pans. A click without dragging sends `ClickSelectedMarker`.
  - `MarkerClick` asks to fast travel.
  - `ToggleMapCallback` and `SetLocalMapExtents` are ignored.
  - `OpenJournalCallback` replaces the map with the journal.
- `src/fast_travel.rs`: `request_map_travel` refuses undiscovered markers
  with a notification (`sNoFastTravelUndiscovered`).
- `src/swf_ui/journal_menu.rs`: answers `RequestQuestsData` with Show on Map
  not allowed, gives objectives no `questTargetID`, and ignores
  `ShowTargetOnMap`.
- `src/quest_targets.rs`: the targets of the tracked quests' objectives
  (`active_quest_references`, `target_spot`, `door_on_way`), used by the
  compass and the map.
- `src/map_markers.rs` and `src/swf_ui.rs`: the compass (`compass_markers`).
  Frame 3 (PlayerSet) is unused.

## Part 1: the controls and small fixes

- **Step 1.** Ask for `sFastTravelConfirm` instead of the nonexistent
  `sConfirmFastTravel`. Add a test that the key resolves against Skyrim.esm's
  settings, or a unit test on the name if fixtures are needed.
- **Step 2.** The map's keys from the control map's "Map Menu" context
  through `Engine::input_map`, so rebinding follows:
  - **2a.** Resolve the context's events: Cancel, Up / Down / Left / Right,
    Zoom In / Zoom Out, MapLookMode, PlacePlayerMarker, PlayerPosition,
    LocalMap, LocalMapMoveMode and Journal. The `input_map` module already
    parses contexts. Add a lookup by context name if there isn't one.
  - **2b.** `MapMenu::key` and `MapMenu::button` dispatch those events:
    - E re-centres on the player, as `CurrentLocationCallback` does.
    - J opens the journal.
    - L toggles the local map. Until `plans/local-map.md` it does nothing, as
      `ToggleMapCallback` does now.
    - P places a destination at the pointer, or asks about the existing one.
    - Holding the right button and moving the mouse tilts the camera
      (MapLookMode). Vertical motion tilts and horizontal motion turns,
      confirmed by observation O2.
    - Drop Q/E zoom and Page Up / Page Down tilt. No extra tilt keys.
    - M (Quick Map) closes the map, confirmed by observation O1; its binding
      follows gameplay Quick Map.
  - **2c.** Update `known_ambiguities/map-view.md`'s Input bullet: the
    game's bindings and the remaining camera/sensitivity choices.

## Part 2: the custom destination

The player sets one marker on the world map. It shows on the map, on the
compass (PlayerSet frame) and later on the local map. It is saved.

- **Step 3. The marker's state.**
  - `Engine::player_marker: Option<PlayerMarker { world, pos: Vec3 }>`, in
    `src/map_markers.rs`.
    - `world` is the map's owner worldspace, so it is in map coordinates as
      `MapRow.pos` is.
    - `pos.z` is the terrain height under it where the LOD or loaded land
      gives one, else 0. The compass only needs x and y.
  - Kept as plain state, not a created reference. CommonLibSSE's handle
    says the game uses a reference. Nothing vanilla can reach it from
    Papyrus (`vrm-tool pex-natives` lists no map-marker natives taking it),
    so state is enough. Record that in `known_ambiguities/map-markers.md`.
  - Saved in `Continuity`, with absent meaning none, as `map_camera` is.
    Add round-trip and corrupt-payload tests next to `map_camera`'s in
    `src/save/tests.rs`.
- **Step 4. On the map.**
  - **4a.** `map_view_rows` appends a `PlayerSetMarker` row (icon 65, name
    `sPlayerSetMarkerName`) before the player's row, when the marker's world
    is the view's. `MapRow.marker` stays `None`: it can't be travelled to.
  - **4b.** A row identity for callbacks. `MarkerClick(i)` on the player-set
    row must reach the move / remove question (step 5c), not travel.
    `row_destination` returns `Option<FormId>`. Replace it with an enum
    `RowAction { Travel(FormId), PlayerMarker, None }` and keep its
    untrusted-index checks and their tests.
- **Step 5. Placing, moving and removing.** All of this is on the world map
  only, since the local map hides Set Destination.
  - **5a.** Screen to map. `MapView::ground_at(screen, aspect) -> Option<Vec2>`
    is the inverse of `screen_of`: intersect the pointer's ray with z = 0, as
    the markers are drawn at z 0. Test it as the inverse of `screen_of` at
    several pitches.
  - **5b.** A click (`ClickCallback`, then a release in place) on no marker,
    or P over the map, places the marker at the pointer.
    - Placing again asks the move / leave / remove question at that new spot.
    - Which sound plays on placing isn't known; play none until it is.
    - The rows are rebuilt (step 8).
  - **5c.** When a custom marker already exists, a subsequent map click
    asks `sMoveMarkerQuestion` with `sMoveMarker`, `sLeaveMarker`, and
    `sRemoveMarker`, in that order. Clicking its icon uses the same question.
    - Move immediately uses the captured new click position (O4).
    - Leave closes the question; Remove clears the marker.
    - There is no moving flag or subsequent placement click.
  - **5d.** A click on an undiscovered marker asks `sPlaceMarkerUndiscovered`
    (Yes / No) instead of notifying `sNoFastTravelUndiscovered`. Yes places
    the custom marker on that location's map position.
    - This question takes precedence over combat and other travel refusals,
      as observed in O5. Discovered locations retain normal travel preflight
      and revalidation after confirmation.
- **Step 6. On the compass.**
  - `compass_markers` adds the player-set marker with frame 3 (PlayerSet).
    It shows at any distance, full alpha and scale 100, like quest markers,
    when the player is in a worldspace whose map owner is the marker's.
  - The heading is taken in map coordinates (the player through
    `world_map_point`), so it stays right in child worldspaces with `ONAM`.
  - Not shown indoors, as the other location markers aren't.
  - Reaching the marker does not remove it (observed in O3).
  - Record these choices in `known_ambiguities/map-markers.md`.

## Part 3: Show on Map, and markers that stay current

- **Step 7. The journal over the map.**
  - **7a.** Check how callback routing behaves: which `Map.MapMenu` receives
    `RefreshMarkers` and `SetSelectedMarker` today (the world map's, since
    markers draw). Write the finding in this plan. If the local map's layer
    registers last on some path, switch the world map to `invoke` on
    `_root.WorldMap.*` as well.
  - **7b.** `OpenJournalCallback` opens the journal over the map instead of
    replacing it.
    - The map view stays alive while the journal is shown.
    - `ShowJournal(true)` on opening, `ShowJournal(false)` on closing back.
    - Closing the journal returns to the map, not to the world.
    - Esc returns to the map (confirmed by O6).
- **Step 8. Rebuilding the markers while open.**
  - `MapMenu::refresh_rows(engine)` rebuilds `rows` and gives the movie the
    markers again (`SetNumMarkers`, `CreateMarkers`), keeping the selection
    if the same row still exists.
  - Called when the journal closes back to the map (a quest may have been
    tracked or untracked), and after the custom marker is placed, moved or
    removed.
  - The map pauses the game (CommonLibSSE: `kPausesGame`), so nothing else
    changes the rows while it is open. This step is what "quest targets
    updating while the map stays open" in the ROADMAP comes to.
- **Step 9. Show on Map.**
  - **9a.** The journal gives each objective with a resolvable target a
    `questTargetID`: an index into a per-open table of `(quest, objective)`
    held on the journal menu.
    - "Resolvable" uses the map's rule: one of the objective's targets
      lands on the map, directly or as the door on the way (factor that
      test out of `map_quest_rows`).
    - Answer `RequestQuestsData` with Show on Map allowed when the map can
      open, that is when `map_world()` and the LOD exist.
  - **9b.** `ShowTargetOnMap(id)` validates the index like `row_destination`
    does.
    - It opens the map centred on that objective's first target that is on
      the map (direct, else the door), at the zoom it opens at.
    - The map opens with `map_start` set to the target. Extend
      `map_start`, or add `map_focus`, so the saved camera isn't
      overwritten.
    - No extra selection or highlighting (confirmed by O7). Opening zoom
      uses 1, the normal opening zoom; its exact vanilla value remains open.
  - **9c.** Update `known_ambiguities/journal.md` and the ROADMAP's journal
    and map entries.

## Observed in vanilla Skyrim SE (user, 2026-11-01)

These observations supersede the provisional choices in the steps above.

- **O1:** M also closes the map.
- **O2:** Right-button mouse movement both tilts and turns the camera.
- **O3:** Reaching the custom destination does not remove it.
- **O4:** The Move It / Leave It / Remove It question opens only when a
  custom marker exists and the player clicks again on the map. Move It
  immediately moves the marker to that new click position; no moving mode.
- **O5:** An undiscovered location shows the Place Marker question even in combat.
- **O6:** Esc in the journal opened from the map returns to the map.
- **O7:** Show on Map adds no extra target highlighting. Its precise opening
  zoom remains an open question; this implementation uses the opening zoom.
- **O8:** Empty-map placement is immediate, without a confirmation question.

## Order and size

Steps 1 to 9, with each lettered sub-step in its own commit, are mostly in `src/swf_ui/map_menu.rs`,
`src/map_view.rs`, `src/map_markers.rs`, `src/swf_ui/journal_menu.rs` and the
save continuity. Parts 1 to 3 don't depend on each other beyond Part 2's row
identity (step 4b) coming before Part 3's rebuild (step 8). Offscreen
captures of each state (the world map with a custom marker, the journal's
Show on Map) go with the commits that make them, per the existing capture
recipes. `plans/local-map.md` follows.

## Step 7a callback routing finding

The game asset's local-map constructor creates its own Map.MapMenu, whose
Init registers the same callback names. GameDelegate stores just one scope
and method per name; root initialization subsequently constructs WorldMap,
which registers again. This accounts for the existing world-map markers
working. Every world-map operation now invokes `_root.WorldMap.*` explicitly,
including refresh, selection, click, date and (step 7b) ShowJournal. This also
keeps routing correct if local-map construction order changes. The installed-asset
movie integration test checks both delegates against the world-layer marker
index and position.
No game bytecode is included here; evidence is the permitted map.swf asset.

## Completion and validation

All numbered steps and lettered sub-steps are implemented in the listed order.
The final validation also covers button art from the live map context, short
mouse clicks arriving between movie ticks, and suppressing camera input while
a marker question is open. Saved marker questions use native action tag 10;
legacy enchanting tags 8 and 9 retain their existing decoding.

Tests and offscreen capture recipes are recorded in `research/ui/map-menu.md`.
The local map, rendering research and remaining travel behavior listed above
remain separate work. The precise Show on Map opening zoom remains open; the
working choice is recorded in `known_ambiguities/journal.md`.
