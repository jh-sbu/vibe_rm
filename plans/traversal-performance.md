# Traversal performance

## Evidence and first optimization (2026-11-06)

User traversal captures isolated recurring ~103 ms / ~40 ms schedule-evaluation
batches at actor indices 11000..11250 and 11250..11500. Package selection accounted
for ~101 ms / ~38 ms. All eight retained slow entries in each large spike selected
Dawnguard's `DLC1SoulPackageIgnoreCombat` (`02014EDB` in that load order).
Examination of the externally installed plugin found its single condition to be
`GetHealthPercentage >= 1`; many entries were `DLC01SoulCairnKeeperSoul` actors.
No game files were copied into the repository.

The health fraction fallback computed record-based maximum health twice. It now
computes it once, adds damage to obtain current health, and keeps the existing
live-health path and fraction clamping. Race DATA is reused through a per-load-order
cache of small immutable plugin fields. The cache stores raw bytes, not computed
stats, levels, template choices, or modifiers. Runtime records bypass the cache
because save restoration can clear and reuse their IDs. A different load order
owns a separate cache; winning overrides determine its cached bytes.

Autonomous evaluation used the ignored `health_query_benchmark` test against the
installed Data directory, with a headless renderer and no loaded cells. The test
queries actor `0200ED3A` 100 times per sample and refreshes the entire global
schedule three times. These are development-profile timings, **not traversal FPS**.

| Measurement | Before | After (serial validation) |
| --- | --- | --- |
| Health query, ms/call (three samples) | 0.562, 0.548, 0.553 | 0.291, 0.291, 0.293 |
| First full schedule refresh, ms | 1228.429 | 1250.412 |
| Subsequent full refreshes, ms | 334.004, 330.822 | 271.481, 270.104 |

This is about a 47% reduction in the direct query and an 18% reduction in warmed
full refreshes. First-refresh cost has not improved. An earlier after-run overlapped
other build work and was noisy (289 / 442 ms warmed); the table uses serial
validation. No timing thresholds are asserted in tests.

Validation covers plugin override precedence, cache reuse, different load orders,
missing fields, runtime ID reuse, health modifiers, zero/negative current health,
player/live offscreen health, and level changes after the race field is cached.

## Release traversal check (2026-11-06)

Windowed release builds (`--features remote-console`) of 8ff7ab0 with and without
the health change ran the same scripted route: `--cell Riverwood --yaw 0`,
`VRM_SEED=1`, `VRM_FRAME_TIMINGS_MS=25`, the Survival Mode prompt answered "No"
over the socket, then `tcl` and `pwalk 1000000 forward sprint` for ~125 s
(about 66,000 units north, roughly 16 cells). AMD Renoir iGPU, RADV.

| `ai.schedule.evaluate` per occurrence | Before | After |
| --- | --- | --- |
| Batch 11000..11250 (~20 occurrences) | 100–104 ms | 55–57 ms |
| Batch 11250..11500 | ~39.5 ms | ~21.5 ms |
| Next-worst batches | 9–10 ms | 9–10 ms |
| Frames with work >= 100 ms | 26 | 10 |

The paired stalls fell about 46%, matching the benchmark's per-query gain. The
55 ms batch is still the most frequent large hitch. Each health query still
resolves templates and reads `DNAM`/`ACBS`, the class `DATA` and the level without
caching.

Both runs had the same larger, rarer hitches at the same frames. None of them is
in the schedule. Their stages have no finer timings yet:

- `sim.scripts_weather` about 3.0 s once, plus 75–250 ms several times.
- `sim.gameplay` 1.13 s and 0.37 s during exterior cell loads.
- `sim.streaming` 245–272 ms for first-time cells (textures and actor setup).
- `ai.npc_hellos` 88 ms once.

Throughout the run, frames took ~50 ms (`engine.update` ~18 ms, `surface.acquire`
~25 ms), which is separate from the stutter.

## Reproduce

```sh
cargo test --offline health_query_benchmark -- --ignored --nocapture --test-threads=1
cargo test --offline health_ -- --ignored --nocapture --test-threads=1
cargo test --offline -p vrm-esp
```

The first command needs installed Skyrim SE Data (`SKYRIM_DATA` or the normal
Steam path) and a headless GPU adapter. It runs this engine's test binary only,
never the original game. No asset directory links are needed. Run benchmarks
without other builds or heavy work competing for CPU time.

## Remaining work

- Cut the remaining ~55 ms Soul Cairn batch: cache the rest of the record stats
  the health query reads, or budget schedule evaluation by elapsed time.
- Add sub-stage timings to `sim.scripts_weather` and `sim.gameplay` to find the
  multi-second hitches.
- Profile remaining package-selection cost before introducing broader caches.
  The scheduler still budgets 250 entries, not elapsed time, per frame.
- Investigate the separately observed scripts/weather, audio/UI, and streaming
  spikes; they are not addressed by this health-query change.
