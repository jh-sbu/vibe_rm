# Plan: the local map

**Status: implemented** (2026-11-02). ROADMAP Done (UI):
"the local map for interiors and nearby cells (`SetLocalMapExtents`, the
movie's `img://Local_Map`)". The local map is a top-down picture of the
player's area, with the doors, quest targets, the custom marker and the player
on it, a title, and the explored parts revealed. It follows
`plans/map-menu.md` (the world map's controls, the custom destination, Show
on Map and the marker rebuild), whose evidence on `map.swf`, the control map
and the game settings it shares. Each numbered step (or lettered sub-step) is
one commit. Run `cargo fmt` before each.

Rules as for all work here (AGENTS.md): public sources and game assets only.
Describe the movie's behaviour in prose and don't copy its bytecode into the
repository (the disassembly stays outside it, in `$SPIKE_SWF_HOME/out`;
`spike/swf/paths.sh`). Choices made without a source go in a new
`known_ambiguities/local-map.md`. Open questions stay in this plan.

## The evidence

### `map.swf` (game asset, read through the spike's `dis` + `fold.py`)

- **`Map.LocalMap`** is the clip at `_root.WorldMap.LocalMapMenu`
  (`MapMovie.LocalMapFader.MapClip`). It holds:
  - Its own marker layer: `IconDisplay`, a second `Map.MapMenu` whose marker
    positions are fractions of the texture, not of the stage.
  - An 800 × 450 texture (`TextureWidth`, `TextureHeight`).
  - `InitMap()`: the first time only, it loads `img://Local_Map` into its
    holder and calls `SetLocalMapExtents(left, top, right, bottom)` with the
    texture's corners in stage coordinates.
  - `Show(show)`: fades the local map in or out, hides `RightButton` (no Set
    Destination on the local map) and relabels `LocalMapButton` between
    `$Local Map` and `$World Map`.
  - `SetTitle(name, cleared)`: the location's name, and `"(" + cleared + ")"`
    under it when the second argument is given.
- **Callback names are shared.** Both `Map.MapMenu` instances register the same
  `GameDelegate` callback names (`RefreshMarkers`, `SetSelectedMarker` and the
  rest), and the callback table is keyed by name, so whichever registers last
  receives them. The local map's layer has to be driven by `invoke` on its
  path. `plans/map-menu.md` step 7a checks which instance the world map's
  existing `callback`s reach today.

### `controlmap.txt`, the "Map Menu" context (game asset)

- `LocalMap`: key 0x26 (L), gamepad 0x4000 (X). `LocalMapMoveMode`: mouse
  button 0 (left). Zoom In / Zoom Out: the wheel (mouse 8 / 9). The full
  table is in `plans/map-menu.md`.

### Game settings (Skyrim.esm, `vrm-tool gmst`)

- `sCleared` "Cleared"
- No game setting names a local-map size, fog-of-war radius or camera height,
  and `Skyrim_Default.ini` has none either.

### Data

- `NorthMarker` (`STAT` 00000003) has one reference in each interior. Its Z
  rotation is the angle that interior's local map is turned by:
  `GoldenglowEstate01` 306.1, `BleakFallsBarrow01` 180.0 (`vrm-tool
  cell-refs`). UESP's [Skyrim Map Design
  Tutorial](https://en.uesp.net/w/index.php?oldid=2395286) gives the same
  306.14 for Goldenglow as "the amount of rotation" of the Creation Kit's
  local-map export.
- Record flag 0x200, **Not On Local Map**, on `STAT`, `MSTT`, `ACTI` and
  `TACT`: UESP [STAT](https://en.uesp.net/w/index.php?oldid=3507776),
  [MSTT](https://en.uesp.net/w/index.php?oldid=2309002),
  [ACTI](https://en.uesp.net/w/index.php?oldid=3596007),
  [TACT](https://en.uesp.net/w/index.php?oldid=2309040).

### Public community research (`refs/CommonLibSSE-NG`, MIT)

These are names and layouts only, used as leads, not as behaviour.
- `LocalMapCamera` has area bounds (`SetAreaBounds(max, min)`), a north
  rotation (`SetNorthRotation`, `zRotation`) and a default state with a zoom
  and minimum frustum half-width and half-height. That suggests an
  orthographic camera fitted to the area's bounds and turned to north.
- `LocalMapMenu` keeps its own culling process, camera and an
  `ImageSpaceShaderParam`: the local map is rendered with its own image-space
  pass.
- UESP's save notes name a CELL change flag `CHANGE_CELL_SEENDATA` (bit 28;
  Oblivion's layout is in UESP [Oblivion Mod:Save File
  Format/CELL](https://en.uesp.net/w/index.php?oldid=2491886)). The
  explored-area data is kept per cell. Our saves are native, so only the
  existence of that state matters here, not its layout.

### UESP on behaviour

- [Skyrim:Maps](https://en.uesp.net/w/index.php?oldid=3651114), Bugs:
  "Areas of the local map that were previously mapped may unmap themselves.
  The unmapped areas will be in rectangular shapes." The local map reveals
  explored areas and keeps them, in rectangular pieces.
- [Skyrim:Controls](https://en.uesp.net/w/index.php?oldid=3637273), Journal:
  setting a quest active "adds its quest target(s) to your compass and your
  world and local maps".

## What exists (2026-11-01)

- Everything `plans/map-menu.md` lists, plus what it adds: the map's keys
  from the control map (L toggles the local map once this plan lands), the
  custom destination, the row identities (`RowAction`) and the marker
  rebuild (`MapMenu::refresh_rows`).
- `src/swf_ui/map_menu.rs` ignores `ToggleMapCallback` and
  `SetLocalMapExtents`.
- `src/quest_targets.rs`: the targets of the tracked quests' objectives
  (`active_quest_references`, `target_spot`, `door_on_way`), used by the
  compass and the map.
- `crates/ui-swf`: `Menu::set_host_image` serves `img://` images (the save
  list's screenshots).
- `src/render`: `render_to_image` renders a scene offscreen at the window's
  size. `Camera` is perspective only.
- `src/save/continuity.rs`: `cleared_locations`, used for "(Cleared)".

## Steps

Steps 1–4 are implemented. The user observations L1, L2, L3 and L5 are
incorporated; L4's range remains provisional, as does the world-map player
marker position when leaving an interior. Choices are documented in
`known_ambiguities/local-map.md`, and captures and validation in
`research/ui/local-map.md`.

- **Step 1. An orthographic camera.**
  - `render::Camera` gets a projection choice: the current perspective, or
    orthographic with half-width and half-height.
  - `render_to_image` takes a size instead of always using the window's.
  - No change to existing frames. Test that the projection maps the
    bounds' corners to the image's corners.
- **Step 2. The area and its picture.** `src/local_map.rs`.
  - **2a. Bounds.**
    - Interior: the loaded cell's references' world-space bounds
      (`OBND` × transform), excluding markers (record flag 0x00800000 Is
      Marker, as on `NorthMarker`) and 0x200 Not On Local Map.
    - Exterior: the loaded cells around the player (the
      `uGridsToLoad` square the engine loads).
    - The game's exact rule isn't published (record the choice in
      `known_ambiguities/local-map.md`).
  - **2b. North.**
    - Interior: the camera turns by the cell's `NorthMarker` Z rotation, so
      the marker's facing is up.
    - Exterior: true north (L2, observed in vanilla).
    - Check the turn's sign against the UESP tutorial's statement: its
      exports needed rotating by that angle to put north up, so the in-game
      image already has north up. Write down the reasoning.
  - **2c. What is drawn.**
    - Statics, terrain and water, without actors, effects, sky or fog, and
      without base records flagged Not On Local Map.
    - Seen from above with back-face culling, so ceilings, which face down,
      don't hide the floors. That is an inference: nothing published says
      how the game removes ceilings.
    - Lit by a fixed light in a grey tone. The game's own image-space pass
      (CommonLibSSE's `ImageSpaceShaderParam`) has no public description,
      so the look is a choice. Record it.
    - Multi-level interiors draw every level in a fixed top-down projection;
      higher levels obscure lower ones (L3, observed in vanilla).
  - **2d. Fit.**
    - The orthographic box covers the bounds at the texture's 16:9 aspect
      (800 × 450, `map.swf`), padded by the bounds' larger side over 20 as
      a choice.
    - Rendered once on opening (or on the first toggle), at 2× for
      sharpness, then served as `img://Local_Map` with `set_host_image`.
  - Tests: bounds from a fixture cell with a flagged static left out; the
    north angle from the fixture's `NorthMarker`.
- **Step 3. The menu.**
  - **3a.** `ToggleMapCallback` and L toggle between the world and the
    local map.
    - On the first show, invoke `_root.WorldMap.LocalMapMenu.InitMap`. It
      loads the image and answers `SetLocalMapExtents(l, t, r, b)` in stage
      coordinates; keep those on `MapMenu`.
    - Then `Show(true)` and `SetTitle(name, cleared)`. The name comes from
      the current location's (or cell's) `FULL`. `cleared` is
      `sCleared` when the location is in `cleared_locations`, else
      undefined.
    - `Show(false)` returns to the world map.
    - While the local map shows, the world map's markers are hidden, and
      dragging, the wheel and keys act on the local map.
  - **3b.** The markers, through `_root.WorldMap.LocalMapMenu.IconDisplay`
    with `invoke` (the shared callback names, above).
    - Doors (61) are the loaded cell's load doors, named by their
      destination's cell or location.
    - Quest targets (62, or 63 for a door on the way) and the custom marker
      (65) when they fall inside the bounds.
    - The player (66), turned by their heading minus north.
    - Positions are fractions of the texture, the same create-then-refresh
      protocol as the world map.
  - **3c. Input.**
    - Left-drag pans (LocalMapMoveMode) and the wheel zooms.
    - Both act by changing the orthographic box and re-rendering each changed
      view in the same UI update, before projecting markers. Unchanged views
      reuse their image. CPU exploration-square caching and bulk readback
      conversion reduce the render cost (`research/ui/local-map.md`).
    - Hovering shows the name (`SetDescription`, as on the world map).
    - Clicking a door or a marker does nothing. The game's local map has no
      travel; a click without a marker is ignored.
  - **3d.** Which map opens first indoors: M opens the local map (L1, observed
    in vanilla). When the world map opens from
    an interior, put the player's marker at the exterior door the way out
    leads to (`door_on_way` from the interior to the map's world). The
    player gets a marker instead of the map's middle; its vanilla position
    remains open.
- **Step 4. The explored area.**
  - **4a.** A per-cell grid of seen squares.
    - Interiors: over the cell's bounds. Exteriors: one grid per exterior
      cell.
    - 16 × 16 squares per exterior cell (256 units each), after Oblivion's
      32-byte seen data. Interiors use the same square size.
    - Both are choices; UESP's "rectangular shapes" bug says only that
      the revealed pieces are rectangles.
  - **4b.** Revealing: squares within a radius of the player, every half
    second, while loaded.
    - Start with approximately 20 player body lengths indoors and outdoors
      (L4); the exact range remains unverified.
    - Lights or line of sight aren't considered.
  - **4c.** Drawing: unrevealed squares are covered in the local map image
    (dark, at the texture's alpha the movie shows behind). Doors, quest
    targets and the player remain visible over unrevealed parts (L5, observed
    in vanilla).
  - **4d.** Saved: the revealed squares of each cell, in `Continuity`,
    as bitsets keyed by cell (interior form id, or world and grid).
    Round-trip and corrupt-payload tests. Size: Skyrim has about 11,000
    exterior cells; only touched cells are stored.
  - **4e.** Write `known_ambiguities/local-map.md` (bounds, north, the
    look, grid, radius, revealing rules), move the ROADMAP's local-map clause
    to Done and point it here.

## Observations to settle in Skyrim SE

None of these can be read from a save (vanilla Papyrus can't see the map),
so they are direct observations for the user, batched with
`plans/map-menu.md`'s. Each answer replaces a choice above.

- **L1.** M opens the local map in interiors. The world-map player marker's
  position when toggled from an interior remains open.
- **L2.** Exterior local maps are not turned.
- **L3.** A fixed top-down projection shows all levels; a higher level blocks
  the lower level beneath it, and changing level does not change the map.
- **L4.** Roughly 20 body lengths, provisionally the same indoors and outdoors;
  the exact range and whether it differs are still open.
- **L5.** Quest targets and doors are shown in unexplored parts.

## Order and size

Step 1 (the renderer's orthographic camera and sized offscreen renders)
comes first. Steps 1 to 4 are about ten commits. Offscreen captures of the
local map in Bleak Falls Barrow and outside Riverwood go with the commits
that make them, per the existing capture recipes.

## Performance follow-up (2026-11-02)

The CPU exploration mask dominates warmed redraws in the measured scenes;
the first render also pays substantial startup costs. The stage measurements
and temporary optimization experiments are in `research/ui/local-map.md`.
The exploration-square cache and format-specific bulk readback conversion
are now implemented, preserving mask pixels, channel conversion and opaque
readback alpha. The 250 ms redraw throttle is removed: changed views render
before markers are projected, including consecutive drag, wheel and keyboard
updates. Idle views and zoom input against a limit keep the existing image.
Remaining work:

- Investigate serving the rendered image without a PNG encode/decode round
  trip, and measure the subsequent movie texture upload separately.
