# Local map implementation observations

On 2026-11-02, offscreen captures using the installed Skyrim SE assets were
inspected for Bleak Falls Barrow and Riverwood. The first interior capture
exposed full-range `OBND` boxes on particle effects; filtering those produced
a recognizable fixed projection of the dungeon. The exterior capture showed
the loaded Riverwood cell aligned north-up. The images are derived from game
assets and are kept outside the repository.

The local render logged approximately 115–126 ms for Bleak Falls before the
bounds correction and 138 ms for the single exterior cell with the exploration
mask on the test machine's integrated GPU. The UI re-renders at most four times a second while moving,
and once on drag release. Times include GPU readback and greyscale conversion;
PNG encoding for the host image is additional.

Reproduce after building `vibe_rm` (the data directory is the normal Skyrim SE
installation, referenced in place):

```sh
VRM_SETTINGS=/tmp/vrm-local-map-settings.ini VRM_SAVES=/tmp/vrm-local-map-saves \
  target/debug/vibe_rm --cell BleakFallsBarrow01 --radius 0 --no-scripts \
  --console map --menu-key pause --screenshot /tmp/vrm-local-interior.png
VRM_SETTINGS=/tmp/vrm-local-map-settings.ini VRM_SAVES=/tmp/vrm-local-map-saves \
  target/debug/vibe_rm --cell Riverwood --radius 0 --no-scripts \
  --console map --menu-key KeyL --menu-key pause --screenshot /tmp/vrm-local-exterior.png
```

The final captures show the explored area and a door marker over unrevealed
space. These are implementation captures, not evidence of vanilla appearance.

Validation: the application suite passed on the host GPU (600 passed, 394
opt-in tests skipped). The installed `map.swf` integration test passed for
toggling, extents, marker creation and coordinates, drag, zoom, reload and
recentering. The local-map save tests passed for round trips, legacy saves,
truncated and malformed payloads, and runtime restoration. The sandbox's
software GPU has too few texture bindings for the renderer, so renderer-backed
tests were run on the host GPU.
