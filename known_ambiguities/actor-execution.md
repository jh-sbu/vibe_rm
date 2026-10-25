# What actors do while the player is away

For `plans/ai-actor-execution.md`. Public sources and what can be inferred
from them are under Sources at the end. Where unloaded actors appear as
their cell loads is in `unloaded-placement.md`. The readings are
`test-mod autotest13`'s and `autotest14`'s (each run 2026-10-25, once);
their module docs have the numbers.

## What is known

- Skyrim keeps four process levels: High, MiddleHigh, MiddleLow and Low
  (CommonLibSSE `PROCESS_TYPE`). Only the data layout is public: no rules,
  distances or update rates.
- Actors new to a save are already on their package's line, 30 to 50 short
  of its target, within seconds of the save loading (autotest6–9), each
  logging `OnPackageStart` then `OnPackageEnd` (autotest13). That could be a
  placement pass (CommonLibSSE names an `initPackageLocationsQueue`) rather
  than walking.
- **Unloaded actors aren't walked: they jump or stay.** Outside the loaded
  and buffered cells, within 2 s of a package starting (with
  `EvaluatePackage`), an actor is put 40 short of a target in the same
  space 2,700 to 4,100 away, in its own cell or the next, in one step;
  `OnPackageEnd` follows 2.5 s after `OnPackageStart`. Actors whose target
  was 4,950 to 8,200 away stayed put for minutes, through fast travel and as
  their cell loaded. A target in another space (through a load door,
  either way) moves the actor exactly onto it, with `OnLocationChange`.
- **Loading a cell doesn't move them on.** Actors are found where the jump
  left them (dropped to the ground), or at home if they stayed.
- **Fast travel** (`Game.FastTravel` to an exterior marker: 0.6 s, the clock
  3.2 hours on) moved no unloaded actor.
- **Waiting** two hours put a loaded actor that was 3,230 from its target
  exactly on it (1 off), with `OnPackageEnd`. Unloaded actors already at
  their targets stayed.
- **Loading a save** left unloaded actors where they were when it was made
  (40 short of targets they had jumped to), with no package events.
- **Packages are re-read while unloaded** within about 3 s of a condition
  changing, `EvaluatePackage` or not, and at once when the clock is set or
  an hour in a schedule passes. `OnPackageChange` (old package) then
  `OnPackageStart` (new) fire 0.08 s apart.
- **Buffered cells keep running.** An actor in the exterior the player left
  for an interior keeps its 3D and walks on at loaded speed. Interiors the
  player left behaved two ways: in autotest13 the palace's actors lost their
  3D at once but walked on and ended exactly on their target; in autotest14
  they kept their 3D for 90 s, until the player went to another exterior.
- **Damage away:** brought to nothing, an unloaded actor dies at once, with
  `OnDeath` then. A hurt one doesn't heal (NPCs' `HealRate` is 0).
- **Timed effects away:** a 90 s effect on an actor was gone 6 s after its
  cell unloaded, and racial abilities are applied again (with
  `OnMagicEffectApply`) each time an actor loads. So active effects appear to
  be dropped on unload rather than paused or run out. While the actor kept
  its 3D in a buffered cell, the effect stayed (autotest14).
- **Event order:** loading logs `OnCellAttach`, `OnMagicEffectApply` for
  abilities, then `OnLoad`. Unloading logs `OnCellDetach`, then `OnUnload`
  0.2 s later. A buffered cell's actors get `OnCellDetach` but keep their
  3D, with no `OnUnload` until the player goes elsewhere.

## Open questions

- **What decides the jump.** Every actor moved had its target within 4,100
  and 830 of height; every one left had it 4,950 or more away and 1,900 or
  more above or below. Distance, height or something else (a path, the
  cells between) may decide. autotest13's Scheduled, 600 to 1,000 from home,
  didn't jump home in the 5 s after its schedule changed, without
  `EvaluatePackage`: whether the jump needs it, or only takes longer, is
  open.
- **What reaches far targets.** An actor whose target is out of jumping
  range: does anything ever move it (game hours, waiting, a later
  re-evaluation), or does it wait for the player?
- **Waiting and sleeping for unloaded actors** out of jumping range, and
  sleeping at all: not tested.
- **When effects end.** Are active effects dropped at the moment of
  unloading, or a few seconds later? Only one reading (6 s after) exists;
  autotest14's actor kept its 3D throughout.
- **The buffer's reach.** Which cells keep running, and keep their 3D,
  after the player leaves, and for how long: one exterior and one interior
  did, the interior's actors with 3D once and without once.
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
