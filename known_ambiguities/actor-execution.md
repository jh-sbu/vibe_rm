# What actors do while the player is away

For `plans/ai-actor-execution.md`. Public sources and what can be inferred
from them are under Sources at the end. Where unloaded actors appear as
their cell loads is in `unloaded-placement.md`. The readings are `test-mod autotest13`'s
(run 2026-10-25, once); its module doc has the numbers.

## What is known

- Skyrim keeps four process levels: High, MiddleHigh, MiddleLow and Low
  (CommonLibSSE `PROCESS_TYPE`). Only the data layout is public: no rules,
  distances or update rates.
- Actors new to a save are already on their package's line, 30 to 50 short
  of its target, within seconds of the save loading (autotest6–9), each
  logging `OnPackageStart` then `OnPackageEnd` (autotest13). That could be a
  placement pass (CommonLibSSE names an `initPackageLocationsQueue`) rather
  than walking.
- **Unloaded actors don't walk mid-session.** In an exterior that isn't
  loaded or buffered, an actor sent 4,500 to 7,800 away in the same
  worldspace stays put, walking or running, for 3.5 minutes and past two
  hours on the clock. A target in another cell (through a load door, either way) moves it
  there at once, exactly onto the target, with `OnLocationChange`; its
  `OnPackageEnd` follows 2.6 s later.
- **Packages are re-read while unloaded** within about 3 s of a condition
  changing, `EvaluatePackage` or not, and at once when the clock is set or
  an hour in a schedule passes. `OnPackageChange` (old package) then
  `OnPackageStart` (new) fire 0.08 s apart.
- **Buffered cells keep running.** An actor in the exterior the player left
  for an interior keeps its 3D and walks on at loaded speed. An actor in the
  interior the player left loses its 3D but walks on at about loaded speed,
  and ends exactly on its target.
- **Damage away:** brought to nothing, an unloaded actor dies at once, with
  `OnDeath` then. A hurt one doesn't heal (NPCs' `HealRate` is 0).
- **Timed effects away:** a 90 s effect on an actor was gone 6 s after its
  cell unloaded, and racial abilities are applied again (with
  `OnMagicEffectApply`) each time an actor loads. So active effects appear to
  be dropped on unload rather than paused or run out.
- **Event order:** loading logs `OnCellAttach`, `OnMagicEffectApply` for
  abilities, then `OnLoad`. Unloading logs `OnCellDetach`, then `OnUnload`
  0.2 s later. A buffered exterior's actors get `OnCellDetach` but keep their
  3D, with no `OnUnload` until the player goes elsewhere.

## Open questions

- **Frozen walkers when their cell loads.** Is an actor that was sent off
  while frozen found at home, on its target (40 short, as at save load),
  or partway, when the player arrives? autotest13 never visited the long
  walkers' cell.
- **Save and reload mid-journey.** Does loading a save put frozen walkers on
  their targets, as it does actors new to the save?
- **Fast travel, waiting and sleeping.** Do they move frozen walkers?
  autotest13's `Game.FastTravel` to an interior entry marker did nothing; a
  map marker may be needed.
- **When effects end.** Are active effects dropped at the moment of
  unloading, or a few seconds later? Only one reading (6 s after) exists.
- **The buffer's reach.** How many cells, and for how long, keep running
  after the player leaves: one exterior and one interior did here.
- **A dead actor's package events.** Dead KilledAway logged an
  `OnPackageStart` minutes after dying.
- **Not covered:**
  - multi-step packages (package trees): the test writers don't build them
    yet
  - which process level an actor is in at each distance (no script can read
    it)

## Sources

### CommonLibSSE-NG

Headers at commit `b93280e832f263dbef44e44cbe2936622a02f91a` (2024-09-03,
the ignored `refs/CommonLibSSE-NG` checkout). These are reverse-engineered
*layouts*: they name types, members and virtual functions, and give no
rules, timings or thresholds.

- `AIProcess.h`: `enum class PROCESS_TYPE { kNone = -1, kHigh = 0,
  kMiddleHigh = 1, kMiddleLow = 2, kLow = 3 }`. Each actor's `AIProcess`
  (`Actor::currentProcess`) records its `processLevel` and points to three
  optional per-level blocks, `middleLow`, `middleHigh` and `high`. The base
  block, present at every level, holds `currentPackage`, `followTarget`,
  `target`, the equipped forms, `essentialDownTimer`, `deathTime`,
  `trackedDamage`, `lowProcessFlags` (`kTargetActivated`,
  `kCurrentActionComplete`, `kAlert`, `kFollower`, `kPackageDoneOnce`,
  `kPackageIdleDone`), `skippedTimeStampForPathing` and `cachedValues`
  (with `cachedWalkSpeed`, `cachedRunSpeed` and others).
- `MiddleLowProcessData`: one member, `std::int32_t hourPackageEvaluated`.
- `MiddleHighProcessData`: `runOncePackage`, `occupiedFurniture`,
  `animationGraphManager`, `charController`, `activeEffects`,
  `commandedActors`, `lastSeenPosition`, `currentMovementSpeed`,
  `deferredKillTimer`, `forceNextUpdate` and others.
- `HighProcessData`: voice state, the regeneration delays, head-tracking
  targets, `fadeState`, and an `AITimeStamp`.
- `ProcessLists.h`: a handle array per level; round-robin cursors for every
  level but high (`currentLowActor`, `currentMiddleHighActor`,
  `currentMiddleLowActor`); switches `processHigh` / `processMHigh` /
  `processMLow` / `processLow`, `runSchedules`, `runMovement`; queues
  `initPackageLocationsQueue`, `initAnimPositionQueue`, `syncPositionQueue`.
- `Actor.h`: virtuals `MoveToHigh`, `MovetoLow`, `MovetoMiddleLow`,
  `MoveToMiddleHigh`, `TryMoveToMiddleLow`, `SetActorStartingPosition`,
  `EvaluatePackage(immediate, resetAI)`.

Inference only, not stated anywhere:

- The cursors suggest the lower levels update a few actors per frame in
  turn, and high-level actors every frame.
- A middle-low actor keeps only the hour its packages were last evaluated,
  which suggests hourly re-evaluation at that level. autotest13's unloaded
  actors re-read their packages within seconds, so they weren't (only) on
  that rule.
- The animation graph, character controller, furniture and active effects
  sit in the middle-high block, which suggests an actor below middle-high
  has a package but no body, and fits autotest13's frozen walkers and lost
  effects.
- `initPackageLocationsQueue` suggests a placement pass, fitting actors new
  to a save being put on their package's line as it loads.

### Vanilla scripts

Checked with `vrm-tool pex-dump` on the installed game's scripts
(2026-10-25); none of these come from SKSE:

- `Actor`: `OnPackageStart`, `OnPackageChange`, `OnPackageEnd`, `OnDeath`,
  `OnLocationChange`; `GetCurrentPackage`, `EvaluatePackage`, `IsDead`.
- `ObjectReference`: `OnLoad`, `OnUnload`, `OnCellAttach`, `OnCellDetach`,
  `OnAttachedToCell`, `OnDetachedFromCell`; `GetParentCell`, `Is3DLoaded`.
- `Game.FastTravel(ObjectReference)`, `Math.sqrt(Float)`.

No script can read an actor's process level; the tests read it from
outside (3D, cell, position, package, event order).

### UESP

The local dump (2026-10-11) has nothing on Skyrim's actor processing:
searches for "high process", "low processing", "AI process", "processing
distance", "uGridsToLoad" and "Cell Buffer" find Oblivion, Morrowind and
ESO pages. Oblivion's NPC format has a "No Low Level Processing" flag
(0x200, [Oblivion Mod:Mod File Format/NPC](https://en.uesp.net/w/index.php?oldid=3089161));
xEdit's TES5 `ACBS` flags call bit 0x200 "Unknown 9" (`wbDefinitionsTES5.pas`,
dev-4.1.6), so Skyrim has no counterpart to cite.
