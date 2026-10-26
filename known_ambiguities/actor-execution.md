# What actors do while the player is away

For `plans/ai-actor-execution.md`. Public sources and what can be inferred
from them are under Sources at the end. Where unloaded actors appear as
their cell loads is in `unloaded-placement.md`. The readings are
`test-mod autotest13`'s to `autotest16`'s (each run 2026-10-25, once);
their module docs have the numbers. autotest16 overturned the earlier
reading that unloaded actors don't walk (plan step 9 applied it).

## What is known

- Skyrim keeps four process levels: High, MiddleHigh, MiddleLow and Low
  (CommonLibSSE `PROCESS_TYPE`). Only the data layout is public: no rules,
  distances or update rates.
- Actors new to a save are already on their package's line, 30 to 50 short
  of its target, within seconds of the save loading (autotest6–9), each
  logging `OnPackageStart` then `OnPackageEnd` (autotest13). That could be a
  placement pass (CommonLibSSE names an `initPackageLocationsQueue`) rather
  than walking.
- **Unloaded actors walk** (autotest16). Outside the loaded and buffered
  cells a traveller moves along its way in steps of about 218 every 2.6 s,
  about 84 a second, and arrives 40 short with `OnPackageEnd`: 2,710 to
  3,162 in 34 to 39 s, 7,100 to 7,500 in 91 to 93 s, asked or not. Three
  sent 10 s apart arrived apart: no common pass. A journey under way as
  its cell unloads walks on the same way. autotest15's "late" arrivals
  (Near home at 34 s, Beyond partway home) were walking.
- **Asked actors in range are put there at once.** Within 2 s of a package
  starting with `EvaluatePackage` (or a scene), an actor is put 40 short of
  a target in the same space up to 4,032 away in 3D (autotest14: 2,700 to
  4,100 in XY; autotest16: 1,077 and 1,453 below), `OnPackageEnd` 2.5 s
  after `OnPackageStart`. At 4,399 (autotest16's High, 2,281 above) and
  beyond, it walks and is then put at its target: 28.7, 36.6 and 39 s on
  (High, autotest16's FarAsked at 7,566, autotest15's Beyond at 8,843).
  Height doesn't decide: High, MidHigh and MidLow were all 3,762 off in
  XY. A target in another space (through a load door, either way) moves
  the actor exactly onto it, with `OnLocationChange`.
- **Some stay.** Actors whose target was 4,950 to 8,200 away in the next
  cell and 1,900 or more below or above stayed put for minutes, asked or
  not, through fast travel and as their cell loaded (autotest13,
  autotest14). Inferred: they had no path; autotest16's actors, which had
  one, all moved.
- **Loading a cell doesn't move them on.** Actors are found where the jump
  left them (dropped to the ground), or at home if they stayed.
- **Fast travel** (`Game.FastTravel` to an exterior marker: 0.6 s, the clock
  3.2 hours on) moved no unloaded actor.
- **Waiting** two hours put a loaded actor that was 3,230 from its target
  exactly on it (1 off), with `OnPackageEnd`. Unloaded actors already at
  their targets stayed. An unloaded one 7,537 from its target was 40 short
  of it after, `OnPackageEnd` 2.6 s after the wait (autotest16). A seated
  loaded actor stayed seated; a loaded sandboxer got up and wandered on.
- **Sleeping** two hours put a loaded walker on its target (4 off), with
  `OnPackageEnd`, but left an unloaded one 7,288 off 3,605 short, about
  4,000 along its way, walking on (autotest16).
- **Loading a save** left unloaded actors where they were when it was made
  (40 short of targets they had jumped to), with no package events.
- **Packages are re-read while unloaded** within about 3 s of a condition
  changing, `EvaluatePackage` or not, and at once when the clock is set or
  an hour in a schedule passes. `OnPackageChange` (old package) then
  `OnPackageStart` (new) fire 0.08 s apart.
- **Coming back.** An actor seated in a chair as its cell unloaded was
  standing 18 from the chair as it loaded again 30 s later, and hadn't sat
  again 13 s on. A sandboxer was in other furniture, 438 from where it was
  (autotest16).
- **Dead actors** log `OnPackageStart` as a wait ends (autotest16) or
  fast travel (autotest13). One sent off unloaded with `EvaluatePackage`
  logged nothing until its cell loaded, then `OnPackageChange` and
  `OnPackageStart`, its body staying put.
- **Buffered cells keep running.** An actor in the exterior the player left
  for an interior keeps its 3D and walks on at loaded speed (autotest15's
  Buffered, moved from its exterior into the palace: 3D kept for a minute,
  walking along a path, 1,240 nearer its target 60 s on). Interiors the
  player left behaved two ways: in autotest13 the palace's actors lost their
  3D at once but walked on and ended exactly on their target; in autotest14
  they kept their 3D for 90 s, until the player went to another exterior.
- **Damage away:** brought to nothing, an unloaded actor dies at once, with
  `OnDeath` then. A hurt one doesn't heal (NPCs' `HealRate` is 0).
- **Timed effects away:** a 90 s effect on an actor was gone 6 s after its
  cell unloaded (autotest16: 0.43 s, as its 3D went), and racial abilities are applied again (with
  `OnMagicEffectApply`) each time an actor loads. So active effects appear to
  be dropped on unload rather than paused or run out. While the actor kept
  its 3D in a buffered cell, the effect stayed (autotest14).
- **Scenes play on with their actors unloaded** (autotest15). A scene
  started with its actor unloaded plays at once; its travel package action
  completes when the actor is put at its target (2.98 s after the start,
  2,711 off) and its phase ends then, the scene going on to its timer. One
  sent out of range completed 39 s on, as its actor was put there.
- **A scene's package lets go without `OnPackageChange`.** As a scene's
  travel action completed (`OnPackageEnd`), its actor took up its own
  package with `OnPackageStart` alone: 0.1 s later loaded, 2.1 s later
  unloaded (the scene's 2 s timer phase). Its own package, done before the
  scene, started afresh: the loaded one walked home and logged
  `OnPackageEnd` again. Starting the scene logged the usual
  `OnPackageChange` then `OnPackageStart`.
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
  time and progress (Activate count) kept, logging `OnPackageStart` again;
  a scene's package that completed logs no `OnPackageChange` as it lets go
  (autotest15). A run that had completed before the scene starts afresh
  (autotest15's actors went home again). Whether the game resumes a run
  that was still going, and what a scene's package still running logs as
  it lets go (a scene stopped), is untested. Any other change of package starts the next one afresh: an
  Activate package left and taken up again counts from nothing.
- **Fragments before events.** A package's begin / end / change fragment
  (`PACK` `VMAD`, xEdit's `wbVMADFragmentedPACK`) runs as its run changes,
  then the event is queued for the actor's scripts. The game's order is
  unknown.
- **Away actors' runs** start, change and end as loaded actors' do, with
  the same events and fragments ("Away" below). `GetCurrentPackage` on an
  away actor answers its run; on an actor the world hasn't touched, the
  package its schedule gives it. Saves keep runs (`AEXC`, step 8): actors
  loaded from a save carry on with theirs without events, as autotest14's
  reloaded actors logged nothing. Saves made before `AEXC` start them
  afresh (`OnPackageStart`).

## Attaching: what we chose

Step 4 of the plan (`ActorExec::stage`, `ActorExec::take_back`). The
readings say actors are found where they were left (loading a cell doesn't
move them on); the rest is our choice:

- **Where it left, until its package moves it on.** An actor that unloaded
  with its cell comes back where it stood, turned as it was, while it runs
  the package it left with and that package hasn't moved it. Once its
  packages give another, or its package moves it ("Away" below), it is
  placed where it went.
- **As it left, but not seated.** One seated (getting in or using
  furniture) comes back standing where it was, by its seat, and goes on as
  its package has it (it may sit again at once; autotest16's Sitter hadn't
  13 s on: open). A sandboxer that was seated starts out as the cell's
  actors do as it loads, in furniture of its choosing (autotest16's
  Sandboxer was in other furniture 438 off). One walking or standing
  carries on from there, without starting out in furniture. A sandbox's
  stop and a patrol's point carry on.
- **Fights and searches end** as an actor unloads: it comes back out of
  combat, not searching, and not bleeding out, as hurt as it left. The game's
  rule is untested (an actor fighting as the player leaves).
- **Not taken up:** actors on their way out through a door, or despawned
  far off for their schedule, and actors a script moved while away. Those
  despawned are where their schedule has them; those moved, where the move
  put them; either way their package takes them on from there.
- **Saves keep stages** (`AEXC`, step 8): an actor loaded as the save was
  made is saved as though its cell unloaded then, and attaches as it was
  when the save loads, its goal worked out again at once from its package.
  What the game does with a reloaded actor that was seated is untested. Actors in saves made before `AEXC` are placed by their
  schedules.

## Away: what we chose

Step 5 of the plan (`ai::away`). An actor away keeps running: its packages
are read every 3 s and as each half hour of game time begins (the readings
had them re-read within about 3 s, and at once as the clock was set or an
hour passed), a change of package logs the usual events, and its package
moves it:

- **Travel in the same space walks** (autotest16), from the cells just
  left at 80 a second in a straight line, elsewhere in strides: one every
  2.6 s, as far as 84 a second takes it in the time since the last (time
  passed in a menu included), to the end point on the line to its target,
  40 short (or 10 inside a wider radius: `unloaded_arrival`, as for actors
  new to a save). Its run ends as it arrives. Not walked along the navmesh
  and not through doors; height doesn't matter, and so nothing stays put
  for want of a path (autotest13's and autotest14's that stayed had, we
  infer, none).
- **Asked to evaluate its packages** (`EvaluatePackage`, or a scene giving
  it a package) with its target within 4,200 in 3D, outside the cells just
  left, it is put at the end point at once instead, `OnPackageEnd` 2.5 s
  after the start; with its target farther, it walks and is put there 35 s
  after the start (`LATE_JUMP`: autotest16's 28.7 and 36.6 s, autotest15's
  39 s), its run ending then.
- **Travel into another space** (through a load door): put exactly on its
  target, with `OnLocationChange` when the location changes, then
  `OnPackageEnd` 2.5 s later; unasked, put there 35 s after the start (the
  readings only sent actors through doors with `EvaluatePackage`).
- **The cells just left** are the places unloaded with the player's last
  change of location, until the next one. The game keeps such cells
  loaded, 3D and all, some of the time ("The buffer's reach", open); ours
  unload them.
- **Other packages with a place** (sandbox, sit, sleep, patrol...): put at
  it (on its centre, anywhere in a whole cell) when away from it by more
  than 1.5 times its radius (at least 96), as schedules had them, whatever
  the distance; else left where they are, to find furniture as they load.
  Untested: autotest13 and 14 only sent actors travelling.
- **Follow and Escort** keep with their target when it is away too (put on
  it once more than 256 beyond the follow distance off); following someone
  loaded (the player) they stay where they are.
- **A journey under way as the actor unloads** (it was walking, or hadn't
  started) walks on, as one starting unasked (autotest16's Leaving and
  LeavingFar).
- **Despawned actors** (gone through a door, or out of sight for their
  schedule) are put where their schedule has them, as before; their run
  ends 2.5 s later when that is a traveller's target.
- **Actors new to the world** (never loaded, no execution) are still placed
  by their schedules (`Whereabouts`, every 20 s and each half hour) without
  events, as autotest6–9 calibrated. `Whereabouts` reads every other away
  actor's place from its execution.
- **Waiting and sleeping** step every away actor once as the time has
  passed ("Time and condition" below): a traveller strides for all of it,
  so two hours take any one there (autotest16's wait; its sleep took one
  only partway: open). Fast travel does nothing more (the
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

Step 7 of the plan (`Engine::run_away`, `Engine::away_package_done`), by
autotest15 ("Scenes play on", above):

- **Package actions go on away.** A scene giving an actor away its
  package asks it to evaluate its packages, and it runs the package as it
  runs any other ("Away" above): a scene's travel in range is put 40 short
  of its target and its run ends 2.5 s on, one beyond range late; that
  ends the action, so the phase ends without the actor loading. A Sit or
  Sleep action is done once the actor is at its place (it takes the seat
  as it loads: untested). Force greets need the player and wait for the
  actor to load; Sandbox, Follow, Patrol and the like never finish, as
  loaded.
- **Letting go.** A scene's package that completed gives way to the
  actor's own with `OnPackageStart` alone (`PackageRun::scene`). The
  unloaded actor's 2.1 s wait before it isn't kept: ours lets go as the
  phase ends.
- **Actors new to the world** (no execution yet) get one from where their
  schedule has them as a scene gives them a package, and run away from
  there, with the usual package events.
- **Lines wait.** A dialogue action waits for its speaker to load (voices,
  subtitles and head tracking need a body), so a phase with a line holds
  its scene until then. Timers run regardless, as before.
- **The old policy** (package actions waiting for their actors to load)
  is gone: autotest15 settled it.

## Open questions

- **Unloaded speakers.** Does an unloaded speaker's scene line hold its
  phase, get skipped, or play on unheard? Not covered: the test writers
  don't build dialogue yet.
- **The range for jumping at once**: between 4,032 and 4,399 in 3D. And
  what decides when an asked one out of range is put at its target (28.7,
  36.6 and 39 s on, after walking 1,540, 3,000 and some): a time, or the
  distance left?
- **Sleeping and waiting.** Waiting put an unloaded traveller 7,537 off on
  its target; sleeping left one 7,288 off about 4,000 along. One reading
  each: a difference between them, or something about the actors?
- **Coming back seated.** autotest16's Sitter came back standing by its
  chair and didn't sit again in 13 s. Does it sit again later, and would
  one away longer come back seated (as actors new to a save do:
  autotest7)?
- **The buffer's reach.** Which cells keep their 3D after the player
  leaves: an exterior left for an interior always has (three times); an
  interior left for another interior once did (autotest16) and once
  didn't (autotest13); an interior left for an exterior kept it twice
  (autotest14, autotest16's inn) and lost it at once once (autotest16's
  Buffer3, after two interiors in a row). The actors walked on either way.
- **No path.** That the actors that stayed had no path is inferred, not
  tested.
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
