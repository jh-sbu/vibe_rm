# Local map implementation observations

On 2026-11-02, offscreen captures using the installed Skyrim SE assets were
inspected for Bleak Falls Barrow and Riverwood. The first interior capture
exposed full-range `OBND` boxes on particle effects; filtering those produced
a recognizable fixed projection of the dungeon. The exterior capture showed
the loaded Riverwood cell aligned north-up. The images are derived from game
assets and are kept outside the repository.

The local render logged approximately 115–126 ms for Bleak Falls before the
bounds correction and 75 ms for the single exterior cell on the test machine's
integrated GPU. The UI re-renders at most four times a second while moving,
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

These are implementation captures, not evidence of vanilla appearance.
