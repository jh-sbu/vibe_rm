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

## Render cost investigation (2026-11-02)

The earlier 115–138 ms measurements combine CPU work, GPU completion and
readback. They do not measure GPU drawing alone. A later uninstrumented
Bleak Falls opening took 168 ms on the same host.

Temporary timers separated `Renderer::render_to_image`, greyscale conversion,
`mask_unseen`, `Menu::set_host_image`, and the movie's `InitMap` invocation.
The renderer timer was further divided into target resize/allocation, CPU
render preparation/submission, GPU completion wait, copy/map, CPU pixel
conversion and size restoration. For attribution, the profiling path drained
previous queue work before rendering and waited for the scene render before
submitting the readback copy. These extra synchronization points affect
end-to-end timing; completion waits are wall-clock measurements, not GPU
timestamp queries.

The runs used the dev profile (`opt-level = 1`, dependencies at 3), a
1600 by 900 local-map image, and the host's AMD Radeon integrated GPU
(RADV RENOIR, Vulkan, Mesa 26.2.3). Installed assets were read in place,
with scripts disabled and temporary settings, saves and captures outside
the repository. Each map was rendered six times without changing its frame;
the table gives medians of the five warmed renders. The radius-2 exterior
check used four renders and the median of its three warmed renders.

| Stage (milliseconds) | Bleak Falls | Riverwood, radius 0 | Riverwood, radius 2 |
| --- | ---: | ---: | ---: |
| CPU render preparation/submission | 5.8 | 1.3 | 3.8 |
| GPU completion wait after submission | 5.6 | 3.8 | 5.6 |
| Readback copy/map | 0.6 | 0.6 | 0.6 |
| CPU readback pixel conversion | 9.7 | 9.4 | 9.5 |
| Greyscale conversion | 2.6 | 2.6 | 2.7 |
| Exploration mask | 61.0 | 57.4 | 56.6 |
| Complete local render, before PNG encoding | 87.3 | 75.0 | 78.9 |
| Host-image PNG encoding, additional | 1.3 | 5.7 | 1.3 |

Warm target setup and size restoration were generally below 0.2 ms combined.
The scenes submitted 1,530 draws / 5,158 object instances in Bleak Falls,
171 / 383 at radius 0, and 861 / 3,995 at radius 2. These draw counts exclude
some separate terrain and water passes.

The first renders took 187 ms, 136 ms and 203 ms respectively. They had much
larger submission and completion costs than warmed renders. A separate
instrumented run measured six initially uncached interior pipelines taking
about 4 ms each to create on the CPU. The additional first-render completion
wait was not broken down into shader compilation, driver work or execution.
The large initial cost should not be used as the steady redraw cost.

The dominant warmed cost is the exploration mask. For every one of the
1,440,000 pixels, `mask_unseen` transforms the pixel into world coordinates
and calls `Seen::contains`. That repeats square/page calculation and a
`HashMap<Page, [u64; 4]>` lookup, even for adjacent pixels in the same square.
The CPU readback conversion also processes each pixel separately, including
format dispatch, instead of copying an RGBA row in bulk. The measured
copy/map stage itself is much smaller than these CPU loops.

Two temporary experiments checked the diagnosis:

- Cache the previous exploration-square result within each image row.
  This reduced lookups to 108,901 in Bleak Falls and 28,800 in radius-0
  Riverwood, with mask times of 12.3 ms and 9.3 ms respectively.
- Copy RGBA8 rows in bulk, retaining the original forced opaque alpha.
  This reduced CPU readback conversion to about 2 ms in both scenes.

Together these experiments reduced warmed local renders to 28.5 ms indoors
and 19.4 ms outdoors, before PNG encoding and movie loading. An isolated
mask probe compared original and cached output at north angles 0, 0.8 and
pi with identical pixels. Compiling the original isolated loop at optimization
level 3 still took about 53–54 ms; a release optimization setting alone did
not remove the repeated lookup cost in that probe. This is not a full
release-build application benchmark.

The movie reload has further costs outside the existing local-render log.
`set_host_image` PNG-encodes the CPU pixels, and `InitMap` loads those bytes
through the vendored Ruffle image loader, which decodes them into bitmap
pixels again. The instrumented `InitMap` calls took 6.7 ms indoors and
10.6 ms in radius-0 Riverwood. Subsequent movie rendering and texture upload
were not isolated. The existing Ruffle source is dual MIT/Apache-2.0;
its MIT option is compatible with this project's MIT license. Only its
existing image-loading path was inspected, with no external code copied.

The profiling instrumentation and experimental optimizations were removed
after measurement; runtime behavior is unchanged. The diagnostic patch,
logs, standalone probe and captures were kept under `/tmp/vrm-local-profile*`,
outside the repository. Repeating the investigation requires adding the
timers described above around the existing capture recipes and repeating
`LocalMap::render` before serving the image. Follow-up work is listed in
`plans/local-map.md`. These measurements do not justify delaying visible
movement by 250 ms.

## CPU optimizations implemented

The exploration-square cache and bulk readback conversion are now in the
application. Mask tests compare every output pixel with individual point
queries across positive and negative page boundaries, rotated maps, and
zoomed views. Readback tests cover padded rows, RGBA/BGRA channel order,
10-bit and half-float conversion, and forced opaque alpha. The profiling
instrumentation remains outside the repository.
