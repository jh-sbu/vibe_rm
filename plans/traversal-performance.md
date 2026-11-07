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

## Kept inflated records (2026-11-06)

Every vanilla NPC_ record is zlib-compressed (5,118 in Skyrim.esm, 623 in
Dawnguard.esm), and `LoadOrder::get` inflated them again on every read. Temporary
counters showed 52 record reads per health query, 47 of them inflates, and about
120,000 inflates per full schedule refresh. Temporary timings split the large
hitches by call: the ~3 s one was `update_scripts` in the frame where a
`SendStoryEvent` (`SCPT`) starts the Companions quests CR01 and CR06. The 1.1 s
and 0.37 s ones were `update_story`, where a hold `CLOC` starts Favor017,
Favor110, CRHoldExpansion, DLC1ScrollHandlingChangeLoc and a Heimskr scene
quest. Both were starting quests and filling aliases.

`get` now keeps a compressed plugin record's inflated payload after its first
read. Each slot is made while indexing and keyed by record version, so overrides
and other load orders are unaffected. LAND and NAVM are excluded: vanilla
inflates them to ~350 MB and ~110 MB and reads them once per cell load. So is
anything over 64 KiB inflated. In vanilla, that leaves NPC_ (~5 MB) and CELL
(~33 MB if every cell is read).

| Release-profile benchmark | Before | After |
| --- | --- | --- |
| Health query, ms/call | 0.265 | 0.005 |
| First full schedule refresh, ms | ~1084 | ~466 |
| Subsequent full refreshes, ms | ~243 | ~40 |

Same traversal route as above:

| | Health change only | + kept records |
| --- | --- | --- |
| Worst schedule batch (11000..11250) | 55–57 ms | ~4 ms |
| Companions SCPT quest start (`update_scripts`) | 2.97 s | 0.73 s |
| Whiterun hold CLOC quest starts (`update_story`) | 1.14 s | 0.28 s |
| Hold CLOC quest starts | 0.37 s | 0.10 s |
| `engine.update` median | 19.1 ms | 13.5 ms |
| Frames with work >= 100 ms | 10 | 8 |

First-time cell streaming (~240 ms) and 75–200 ms `sim.scripts_weather` spikes
remain. In this run the `sim.scripts_weather` median rose from 6.3 to 8.8 ms;
whether that is noise is unchecked.

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

- Find which alias fills make quest starts cost 0.1–0.7 s. `Fill::Matching` with
  Closest tests every persistent ref against the full alias conditions; a cheap
  prefilter is the likely fix.
- Budget schedule evaluation by elapsed time instead of 250 entries per frame.
- Add permanent sub-stage timings to `sim.scripts_weather` and `sim.gameplay`.
- Investigate first-time cell streaming (~240 ms: textures and actor setup)
  and the remaining 75–200 ms `sim.scripts_weather` spikes.
