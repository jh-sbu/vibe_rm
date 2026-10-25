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

## Package runs: what we chose

`ai::exec::PackageRun` (step 3 of the plan). The readings above settle the
order of a change (`OnPackageChange` for the old package, then
`OnPackageStart`) and that a package that ended still gets
`OnPackageChange` when another takes over (autotest13's home packages ended
as the save loaded and changed at Go1). The rest is our choice:

- **What completes a run.** Loaded Travel and Escort runs complete on
  arrival (scenes' arrival rule) and stay the actor's package; Activate,
  UseWeapon and UseMagic runs complete when their count or time runs out and
  give way until they stop applying. Sit, Sleep, Sandbox, Follow, Patrol and
  ForceGreet runs don't complete, though scenes count a seat taken or a
  greeting held as done: whether those log `OnPackageEnd` is untested.
- **Combat and conversations** leave the run as it is (no events), its
  procedure waiting until they end. Whether the game logs
  `OnPackageChange` / `OnPackageStart` for a combat package is untested.
- **Scenes** suspend the run: the scene's package logs the usual change
  events, and when the scene lets go, the old run resumes with its start
  time and progress (Activate count) kept, logging `OnPackageStart` again.
  Whether the game restarts it instead, and which events it logs, is
  untested. Any other change of package starts the next one afresh: an
  Activate package left and taken up again counts from nothing.
- **Fragments before events.** A package's begin / end / change fragment
  (`PACK` `VMAD`, xEdit's `wbVMADFragmentedPACK`) runs as its run changes,
  then the event is queued for the actor's scripts. The game's order is
  unknown.
- **Away actors' runs** start, change and end as loaded actors' do, with
  the same events and fragments ("Away" below). `GetCurrentPackage` on an
  away actor answers its run; on an actor the world hasn't touched, the
  package its schedule gives it. Actors loaded from a save start their runs
  afresh (`OnPackageStart`) until saves keep them (step 8), where
  autotest14's reloaded actors logged nothing.

## Attaching: what we chose

Step 4 of the plan (`ActorExec::stage`, `ActorExec::take_back`). The
readings say actors are found where they were left (loading a cell doesn't
move them on); the rest is our choice:

- **Where it left, until its package moves it on.** An actor that unloaded
  with its cell comes back where it stood, turned as it was, while it runs
  the package it left with and that package hasn't moved it. Once its
  packages give another, or its package moves it ("Away" below), it is
  placed where it went.
- **As it left.** One seated (getting in or using furniture) starts out in
  the same marker when it is still free and usable, else as fresh actors do
  (a free seat nearby). One walking or standing carries on from there,
  without starting out in furniture. A sandbox's stop and a patrol's point
  carry on. Whether the game seats an actor in the same furniture is
  untested.
- **Fights and searches end** as an actor unloads: it comes back out of
  combat, not searching, and not bleeding out, as hurt as it left. The game's
  rule is untested (an actor fighting as the player leaves).
- **Not taken up:** actors on their way out through a door, or despawned
  far off for their schedule, and actors a script moved while away. Those
  despawned are where their schedule has them; those moved, where the move
  put them; either way their package takes them on from there.
- **Saves don't keep stages** (step 8): actors loaded from a save are
  placed by their schedules.

## Away: what we chose

Step 5 of the plan (`ai::away`). An actor away keeps running: its packages
are read every 3 s and as each half hour of game time begins (the readings
had them re-read within about 3 s, and at once as the clock was set or an
hour passed), a change of package logs the usual events, and its package
moves it:

- **Travel in the same space**, its target within 4,500 (3D): put on the
  line to it, 40 short (or 10 inside a wider radius: `unloaded_arrival`, as
  for actors new to a save), then `OnPackageEnd` 2.5 s after the start.
  Beyond 4,500 it stays where it is, its run going on. The readings put the
  line between 4,100 and 4,950; height may be what decides (open, below).
- **Travel into another space** (through a load door): put exactly on its
  target, with `OnLocationChange` when the location changes, then
  `OnPackageEnd` 2.5 s later.
- **In a cell the player has just left** (the places unloaded with the
  last change of location, until the next one) a traveller walks on in a
  straight line at 80 a second to the same end point, its run ending as it
  arrives (as loaded travellers'). Not walked along the navmesh, and not
  through doors: a target in another space is reached as above. The game
  keeps such cells loaded (3D and all); ours unload them.
- **Other packages with a place** (sandbox, sit, sleep, patrol...): put at
  it (on its centre, anywhere in a whole cell) when away from it by more
  than 1.5 times its radius (at least 96), as schedules had them, whatever
  the distance; else left where they are, to find furniture as they load.
  Untested: autotest13 and 14 only sent actors travelling.
- **Follow and Escort** keep with their target when it is away too (put on
  it once more than 256 beyond the follow distance off); following someone
  loaded (the player) they stay where they are.
- **A journey under way as the actor unloads** (it was walking, or hadn't
  started) is taken up as one starting: put at the end point when in range.
  The game's rule is untested (open, below).
- **Despawned actors** (gone through a door, or out of sight for their
  schedule) are put where their schedule has them, as before; their run
  ends 2.5 s later when that is a traveller's target.
- **Actors new to the world** (never loaded, no execution) are still placed
  by their schedules (`Whereabouts`, every 20 s and each half hour) without
  events, as autotest6–9 calibrated. `Whereabouts` reads every other away
  actor's place from its execution.
- **Waiting and sleeping** step every away actor once as the time has
  passed ("Time and condition" below). Fast travel does nothing more (the
  readings: it moved no unloaded actor).
- The stepper visits 100 away actors a frame, in turn.

## Time and condition: what we chose

Step 6 of the plan (`ai::exec::Processing`, `Engine::pass_time`,
`Engine::hurt_away`).

- **Tiers.** An actor runs `Full` (loaded, every frame), `Buffered` (away
  in a place unloaded with the player's last change of location: walks on)
  or `Coarse` (away elsewhere: put where its package has it), its packages
  read every 3 s at both away tiers. The cell decides, not the distance:
  autotest13's actors 2,700 off in a cell the player hadn't been in jumped,
  while those in the cell just left walked. The game's four process levels
  have no public rules; nothing here claims to match them one for one.
- **Clocks.** Game time drives schedules, the half hour packages are read
  again in, runs' start times and catching up. Real time (which stands
  still in menus) drives the reading cadence, the 2.5 s to a jumped
  traveller's `OnPackageEnd`, walking in a buffered cell, bleeding out and
  healing. Each execution records both clocks as it is stepped.
- **Time passed in a menu** (waiting, sleeping, fast travel) counts as the
  real seconds it stands for (180 a game hour; fast travel its travel
  time's): wounds heal and bleeding runs out for them; away actors read
  their packages for the new time, jump or are put as above, end jumped
  runs, and in a buffered cell walk on for those seconds; loaded actors
  walking on with the package they had walk on along their paths for those
  seconds at their walk speed, finishing the walk (and a Travel run) when
  they reach its end (autotest14: a loaded walker 3,230 off was exactly on
  its target after two hours). Loaded actors whose package changed, or
  who stand away from where it has them, still start over there
  (respawned, in their furniture), and those whose place is elsewhere go:
  only walking has been seen. The fallen, the fighting and the player's
  followers are left be. The same applies after every hour of a wait.
- **Damage away** (`DamageActorValue` / `RestoreActorValue` on health) is
  taken at once into the execution (one is made for an actor never
  loaded, its schedule still placing it), by autotest13. Brought to
  nothing, it dies then: `OnDying`, `OnDeath` and the kill event, its body
  where it was (its schedule's place for one never loaded; its editor
  place when its package had it anywhere in a cell).
- **Bleedout away.** An essential actor brought to nothing away bleeds out
  where it is (`OnEnterBleedout`), takes no more damage, and gets up 12 s of
  real time later (as loaded ones) with a quarter of its health; loading
  while still down, it gets up as it loads. Untested.
- **Healing away** follows the actor's `HealRate` (0 for NPCs: autotest13's
  HurtAway stayed at half).
- **Timed effects** end as their actor unloads (within a frame; the
  reading had one gone 6 s after), and abilities are applied again as it
  loads, as autotest13 saw. Nothing is paused.

## Scenes: what we chose

Step 7 of the plan (`scene::AwayActors`, `Engine::run_away`). No reading
covers scenes with unloaded actors; autotest4 and autotest5 only played
timers with loaded actors.

- **Package actions go on away.** An actor away runs its scene package as
  it runs any other ("Away" above): a scene's travel is put 40 short of its
  target in range, or onto it through a door, and its run ends 2.5 s on;
  that ends the action, so the phase can end without the actor loading. A
  Sit or Sleep action is done once the actor is at its place (it takes the
  seat as it loads). Force greets need the player and wait for the actor
  to load; Sandbox, Follow, Patrol and the like never finish, as loaded.
  A traveller out of jumping range stays put, so its phase waits.
- **Actors new to the world** (no execution yet) get one from where their
  schedule has them as a scene gives them a package, and run away from
  there, with the usual package events.
- **Lines wait.** A dialogue action waits for its speaker to load (voices,
  subtitles and head tracking need a body), so a phase with a line holds
  its scene until then. Timers run regardless, as before.
- **The old policy** (package actions waiting for their actors to load,
  whose scene packages still moved them away) is kept behind
  `VRM_SCENES_WAIT_FOR_LOADED` until `test-mod autotest15` settles it.

## Open questions

- **Scenes with actors away** (`test-mod autotest15`, not run yet). Does a
  scene's package action finish while its actor is unloaded (and its phase
  end), or does the scene wait for the actor to load? Does a scene start at
  all with its actor unloaded? Does an unloaded speaker's line hold its
  phase, get skipped, or play on unheard (not covered: the test writers
  don't build dialogue yet)?

- **What decides the jump.** Every actor moved had its target within 4,100
  and 830 of height; every one left had it 4,950 or more away and 1,900 or
  more above or below. Distance, height or something else (a path, the
  cells between) may decide. autotest15's Beyond (8,843 away, 139 below)
  separates distance from height. autotest13's Scheduled, 600 to 1,000 from home,
  didn't jump home in the 5 s after its schedule changed, without
  `EvaluatePackage`: whether the jump needs it, or only takes longer, is
  open.
- **A journey under way as the actor unloads.** Does a walker outside the
  buffer jump to its target (as one starting a package does), stay, or walk
  on? We have it jump (when in range).
- **What reaches far targets.** An actor whose target is out of jumping
  range: does anything ever move it (game hours, waiting, a later
  re-evaluation), or does it wait for the player?
- **Waiting and sleeping for unloaded actors** out of jumping range, and
  sleeping at all: not tested. Nor loaded actors in furniture or sandboxing
  through a wait: we start them over where their packages have them.
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
