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

## Narrowed alias candidates (2026-11-06)

Temporary per-alias timings showed "find matching reference" fills scanning
all ~49,000 persistent references with the full alias conditions, at about 60
ms per alias. Most found nothing: CR01 alone has ten homeowner aliases, and
Favor quest-giver, BQ01/BQ02 and DA16 aliases behaved the same way.

`fill_alias` now narrows its candidates when a condition group (an OR group,
or one condition ANDed alone) can be answered from a static index of the
persistent references. The index covers `GetIsID`, `GetIsReference`,
`GetInCell`, `GetIsEditorLocation`, `GetIsEditorLocAlias`, `GetInFaction`
(actors, plus references with script faction changes) and `IsUnique` (actors).
Quest and global functions with no subject are evaluated once. Candidates stay
in list order and the player is always one, so the first match and the
Closest sort are unchanged. An ignored test checks against the installed load
order that no reference left out passes its group.

| Same route | Kept records only | + narrowed candidates |
| --- | --- | --- |
| Companions SCPT quest start | 727 ms | 78 ms |
| Whiterun hold CLOC quest starts | 284 ms | 24 ms |
| Hold CLOC quest starts | 99 ms | < 20 ms |

`GetIsVoiceType` isn't narrowed: talking activators have voice types, and
leveled template picks depend on globals. CR04's Brute alias (~50 ms) and
CR01's BeastVictim alias (~14 ms) are the largest remaining fills. `Near` fills
(WE09, ~10 ms each) scan `refs_near` unindexed.

## Script/weather spikes are world encounter starts (2026-11-06)

Temporary timings in `update_scripts`, at each native call, in story manager
quest starts, in `fill_quest_aliases` and in `create_ref` traced the remaining
60–125 ms `sim.scripts_weather` spikes. Each was one `Keyword.SendStoryEvent`
call from Papyrus that started a world encounter quest (WE09, WE07, WE22,
WEDL01b, WEDLVigilOfStendarr03; the WEQuests node picks at random, so runs
differ). Weather wasn't involved.

- **Created actors:** `create_ref` → `add_created_actor` → `spawn_actors` into
  the loaded cells, 5–27 ms per actor (30–52 ms per quest). In frame 442, 42.5
  ms of the 74 ms was loading 29 textures (26.7 ms read/parse, 15.8 ms upload)
  and about 5 ms models. First-time cell streaming has the same synchronous
  asset cost (`cell.actors` ~137 ms).
- **`Near` fills:** 7–13 ms each, 2–4 per WE quest. When the center is loaded,
  `refs_near` returns every loaded reference (~15,000), each tested against
  `IsLinkedTo(<center alias>)` and `HasRefType`.

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

- Remaining slow alias fills: CR04 Brute and CR01 BeastVictim (anchored only by
  `GetIsVoiceType` and location functions), and `Near` fills.
- `Near` fills now run the alias conditions before the eligibility checks
  (both pure): 7–13 ms became 4.5–11 ms per fill. What's left is evaluating
  `IsLinkedTo`/`HasRefType` over ~18,000 loaded references. A reverse `XLKR`
  index over all 878,000 placed references takes ~178 ms to build, so it would
  have to be built per cell during streaming to help. That would save roughly
  10–15 ms per world encounter start; not done.
- Textures and spawning actors' models now load in the background
  ([async textures](async-textures.md)): the first-time cells hitch fell from
  243 to 75 ms. Placed objects' models are still parsed synchronously
  (`objects.models` ~17 ms in that frame).
- The Companions SCPT start's script stage fell from ~75 to ~30 ms
  (2026-11-07): `GetIsVoiceType` narrows matching candidates (CR01 BeastVictim
  ~15 → ~3 ms); factions are memoized per actor during a fill, and authored
  reactions per faction (CR04 Brute ~14 → ~6 ms per attempt). Also,
  `update_scripts`' later passes no longer give a thread that used its budget
  another 20,000 instructions.
- Papyrus busy loops cost ~7 ms every frame (vanilla
  `CWArrowVolleyParentScript.startFiring` polls `CWBattlePhase` without
  waiting outside a siege). The user chose to deviate here
  ([papyrus budget](../deviations/papyrus-budget.md)): busy threads now share a
  1.2 ms slice a frame. On the route, the median `sim.scripts_weather` fell
  from 8.8 to 1.4 ms and `engine.update` from 13.4 to 8.7 ms; the worst frame
  from 105 to 79 ms.
- The first `Disable` of a session builds the enable-parent index over all
  878,000 placed references (~170 ms; startup's nirnroot `OnCellAttach`).
- One run had a 52 ms `cell.triggers_scripts_audio` at a cell load (frame 844);
  not reproduced or looked into.
- `ai.loaded.enemies` once took 34 ms (frame 710 of one run); not looked into.
- Budget schedule evaluation by elapsed time instead of 250 entries per frame.
- Add permanent sub-stage timings to `sim.scripts_weather` and `sim.gameplay`.
