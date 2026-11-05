# Plan: the unblocked AI depth work

**Status: in progress** (2026-11-05). Step 0 is implemented; steps 1–16 remain.

This plan covers ROADMAP Next item 4 (AI depth): everything in it that can be
built today from public sources, the game's data and the readings we already
have. Items that need evidence first are listed under "Left out: blocked",
with the reason for each. The roadmap records the same blockers. When one of
them is settled, it gets its own step here or its own plan.

`plans/ai-actor-execution.md` (done) built the foundation: `ai::exec::ActorExec`,
away actors (`ai::away`), processing tiers, package runs and their events, and
`AEXC` in saves. This plan builds on it and must not regress any of it.

## Ground rules

- Public sources only. That means the game's data (`vrm-tool pack-tree`,
  `pack-templates`, `alias-packages`, `esp-dump`), the local UESP index,
  xEdit's `wbDefinitionsTES5.pas` and `refs/CommonLibSSE-NG` (names and
  layouts only, never rules). The Creation Kit wiki's procedure pages are
  blocked (Cloudflare), so each procedure's meaning is read from its template's
  input names and from how the vanilla packages fill those inputs. Each step
  records what it reads that way.
- When a step's research is inconclusive and it makes a choice to get the
  behaviour working, it records the choice in `known_ambiguities/`. A choice
  is allowed only where the template's inputs say what the procedure is for
  and only a detail is open. A behaviour that would have to be invented
  wholesale belongs in "Left out: blocked", not in a step.
- An SE probe that would settle a step's choices is queued in the step's
  ambiguity note and in ROADMAP "Test plugins". It does not hold the step back.
- Keep existing behaviour unless a step deliberately changes it. The
  autotest13–16 rules for away actors stay as compatibility requirements.
- One commit per step, or a few for the big ones. Each step adds synthetic
  tests, and fixture tests on vanilla packages where the step needs them
  (`--include-ignored` runs here; mind the location, `alias_gen` and goal
  fixture traps). Run `cargo fmt` before each commit.

## What the data says (census, 2026-11-05)

This census covers all 7,573 `PACK` records in the five masters, by
template. The current `vrm-tool pack-templates` counts only NPC `PKID` slots
and its top 40. `pack-census <data> --masters` reproduces this census.

- Run today by their main procedure (single-procedure templates, and trees
  collapsed by `package::main_procedure`): Travel 2,488, Sandbox 1,243, Eat
  431, ForceGreet 409, Patrol 392, Sleep 365, SitTarget 324, HoldPosition 168,
  Follow 158, Sit 136, SandboxWorking 88, UseMagic 70, SandboxMultiLocation
  66, UseWeapon 58, Escort 51.
- Not run as designed (falling back to `Behaviour::Hold`, or one procedure of
  a sequence): UseIdleMarker 65, Orbit 58, SandboxAndKeepEyeOn 36,
  CarryAndDropItem 32, PatrolStaticPathing 30, TravelBackAndForthAndGuard 26,
  TravelBackAndForth 24, GroupMovement 23, Shout 22, EscortPlayerWhenNear 18,
  FollowPlayer 14, StayAtCurrentLocation 14, TravelCrouchWhenNearTemplate 12,
  DoNothing 12, CarryAndKeepItem 11, FleeFrom 11, FleeTo 8, FollowTo 6,
  ForceGreetWaitSitting 6, Say 5, Acquire 5, Hover 5, SayToPlayer 4,
  SandboxLockOnArrival 4, ActivateInSequence 2, StartConversation 1,
  KeepDistanceFrom 1, SingleFileFollow 1, and the dragons' and Alduin's
  templates.
- Procedures named in templates' and packages' own trees: Travel 294, Sandbox
  97, HoldPosition 93, Wait 90, Guard 75, Sit 47, Patrol 47, UnlockDoors 46,
  Find 36, Follow 34, Activate 26, Sleep 20, Shout 18, Say 16, UseWeapon 13,
  Hover 12, Eat 12, LockDoors 11, Acquire 10, Flee 10, Orbit 10, ForceGreet 9,
  Escort 6, Wander 5, UseIdleMarker 4, UseMagic 3, FlightGrab 3, KeepAnEyeOn 2,
  FollowTo 1, DialogueActivate 1.
- Location kinds (`PLDT`): 4 (Object ID), 5 (Object Type) and 7 (At package
  location) are never used. 11 (xEdit's "Unknown 11") is used 18 times.
  Target kinds (`PTDA`): Object ID 215 (UseMagic, Shout, Sit,
  TravelBackAndForthAndGuard...) and Object Type 1,747 (Eat 862, Sleep 343,
  Sit, UseWeapon, GroupMovement...). Kind 5 is used 0 times; the roadmap
  calls it "interrupt data", but xEdit names it "Unknown 5".
- Alias inventories (`QUST` `CNTO`): 160 entries are leveled items (`LVLI`),
  none are form lists, and the rest are direct items.

## Steps

### 0. A package census tool

**Status: done** (2026-11-05). `pack-census <data> [--masters]` counts all
PACK records, with expanded branch behaviours from the shared engine parser;
`SHOW=<template>` lists users. `--masters` selects the five base masters,
excluding Creation Club additions. The run confirms 7,573 records and the
procedure and location/target-kind counts above. Synthetic census and existing
package-data tests pass; all 69 condition regressions pass with GPU access.
The concrete Acquire permissions census finds one AllowStealing and one
AllowPickpocketing (both `MQ101RalofGetDoorKey`, `0008C816`), no AllowKilling.
This corrects step 6's planning assumption. Condition record parsing was extracted into a shared
module so the tool does not duplicate engine behaviour; NPC package collection
now lives with schedule collection. No package behaviour changed.

Add `vrm-tool pack-census <data>`. It counts all `PACK` records by template;
the procedures in templates' and packages' own trees; the location and target
kinds in use; and, for each template, the `Behaviour` the engine gives it
today (through `package::parse` / `expand`). `SHOW=<template>` lists the users.
Later steps quote its output, and each step's commit updates the "Not run as
designed" list above. This needs no engine changes.

### 1. A procedure-tree interpreter (the foundation)

Today `package::expand` splits a `Stacked` root into one package per branch,
and every other tree is collapsed to one `main_procedure`. Multi-location
sandboxing is a special case (`package::sandbox_stops`, `StopProgress`).
Most of the templates above need their trees run as written.

- Keep the parsed tree (`Node`: kind, conditions, `PRCB` flags with "Repeat
  when Complete", children, procedure name and input indices) on the
  `Package`. Bind its inputs to the package's values once, at parse time.
- Add `PackageRun::cursor`: the path to the running procedure(s), what each
  procedure has done so far (arrived, activations done, seconds waited, line
  spoken), and the `Random` picks. The node kinds behave as follows:
  - `Stacked`: the first child whose conditions pass. Re-checked on the
    package's evaluation cadence, so a branch switch happens mid-run.
  - `Sequence`: children in order, skipping those whose conditions fail. It
    completes when its last child does, or starts over with the repeat flag.
  - `Simultaneous`: every child runs. One procedure moves the body (the first
    moving one in the order `main_procedure` uses today); the others
    (Wait, Guard, KeepAnEyeOn, Find) only observe or time. It completes when
    its first completing child does. That is the multi-location reading
    already recorded in `known_ambiguities/package-trees.md`; extend that note.
  - `Random`: one passing child, another than the last.
- Each procedure has a completion rule:
  - Travel, FollowTo, Escort: on arrival.
  - Activate: after "Number to Activate" uses.
  - Wait: after its seconds.
  - Say: after the line ends.
  - Find: once the result list is filled.
  - Acquire: after "Num to acquire" items.
  - UseWeapon and UseMagic: as now.
  - Patrol without repeat: at its last point.
  - Sandbox, Follow, Guard, Hold, Sit and Sleep never complete.

  The package completes when its root does. That fires `OnPackageEnd` as
  today, then `done_once`.
- Move multi-location sandboxing onto the interpreter: `stops` and
  `StopProgress` go, and the tree's own Travel → UnlockDoors →
  (Sandbox ∥ Wait) runs. Its existing tests must pass unchanged, except the
  ones that pinned "progress isn't kept" (see step 2).
- Leave the `Stacked` split in `expand` for package *selection*, so that
  `GetCurrentPackage` and the package events keep naming the real `PACK`.
  The interpreter runs inside the chosen branch.
- Away actors: `ai::away::plan_move` asks the cursor for the current moving
  procedure and keeps the existing rules per procedure. A Wait advances on
  real time, as a loaded one does. A Say waits for its speaker to load
  (scenes' rule, `known_ambiguities/actor-execution.md`). Away UseMagic and
  UseWeapon runs count casts and barrages on their cooldown and pause
  ranges, so finite ones (the Shimmermist Falmer boss's single cast)
  complete away. They spend no magicka and fire nothing. Record that in
  `known_ambiguities/use-magic.md` and `use-weapon.md`.
- Saves: the cursor goes into `AEXC`. Old saves load with no cursor, which
  starts the run at its root.
- Tests: synthetic trees covering each node kind, repeat, a `Stacked` switch
  mid-run, completion events, save round-trips, and away stepping. Add
  fixture tests on `TravelBackAndForth`, `SandboxMultiLocation` and
  `CarryAndDropItem` packages.

### 2. Sequences' simple procedures, and multi-location leftovers

With step 1 in place, the remaining work here is the procedures themselves.

- **Wait** (90): stand, or sandbox when `Simultaneous` with a Sandbox, for
  its seconds. Real seconds, as already chosen for multi-location stops.
- **LockDoors / UnlockDoors** (11 / 46): lock or unlock the doors of the
  location given. Reuse the sleepers' home-door locking (`package_door_locks`,
  `known_ambiguities/locks.md`). This also gives multi-location stops their
  per-stop UnlockDoors and runs SandboxLockOnArrival.
- **HoldPosition and StayAtCurrentLocation / DoNothing**: hold where the run
  began. Keep a run start position, which also makes `PLDT` kind 2 ("near
  package start location") exact instead of the current position, as
  `package-trees.md` asks.
- **Progress kept across loads, waits and sleeps**: the cursor survives a
  cell unload in `ActorExec`. `jump_to_schedules` stops resetting a
  multi-location sandbox to Location 1 and advances it by the time passed.
  Unloaded actors' whereabouts use the current stop, not Location 1.
- **"In cell" stops in another cell** (Kynesgrove's campfire and inn): travel
  to that cell, through its load door, then sandbox anywhere in it. Use the
  whole-cell sandbox placement (`plans/whole-cell-sandbox-load.md`) on
  arrival.
- **TravelBackAndForth** (24) and the travel half of
  **TravelBackAndForthAndGuard** (26): two Travels in a repeating Sequence.
  The Guard half is left out (see Blocked); its branch is skipped, and the
  ambiguity note says so.
- **ActivateInSequence** (2: Delphine lighting torches, Blind Cliff): read
  its tree and run it as written.

### 3. Package data defaults and targets

- **Defaults**: templates carry their own `CNAM` values (`TravelBackAndForth`
  has them for its Bools). An input the package omits takes the template's
  value. `GetNumericPackageData` and the parsed fields read it the same way.
  First count the vanilla packages that omit an input their template has
  (add this to `pack-census`). If there are none, do the work for mods' sake
  and say so.
- **"Runtime changes"**: the roadmap names runtime changes to package data.
  Check in the data and the vanilla scripts whether anything can change a
  package's data at run time (a global-backed input, a native). If nothing
  vanilla does, take it off the roadmap with that finding.
- **Object ID / Object Type targets** (215 / 1,747 uses): add one resolver,
  "the nearest suitable reference of this base / type in the package's
  location, not taken by another actor". Eat, Sleep, Sit and UseWeapon
  already resolve their own; move them onto the resolver without changing
  their results (existing tests). Find, Acquire, Shout and
  TravelBackAndForthAndGuard use it next. Object-type numbers follow xEdit's
  `wbObjectTypeEnum`.
- Not done here: location kinds 4, 5 and 7 (no vanilla use; nothing to check
  against), location kind 11, and target kind 5 (see Blocked).

### 4. Dialogue procedures

- **Say** (5 Say packages, 16 tree uses; SayToPlayer 4): travel to the
  location, then speak the topic to the target. Use the inputs "Look At
  Target", "Finish Speaking" (a run that changes mid-line lets the line end)
  and "Force Subtitle". Reuse the scene dialogue action's line playing (voice,
  subtitles, head tracking), so the same "speaker must be loaded" rule holds.
- **SayToPlayer**: its tree is ForceGreet's minus the conversation. Its
  conditions need step 9's `IsPlayerMovingIntoNewSpace`; until then that
  condition fails, which blocks the branch, as today.
- **StartConversation / DialogueActivate** (1): walk to "Talk From" and start
  a conversation with the target. When the target is the player, this is a
  force greet with the given topic; when it is an NPC, it starts an
  NPC-to-NPC conversation through the ADIA path already in place.

### 5. Carrying, and the sawmills

- **CarryAndDropItem / CarryAndKeepItem** (32 / 11): the tree is Activate
  Carry Start (when the graph variable says empty-handed) → Travel to Carry
  End → Activate Carry End (when carrying) → Travel back, repeating. Wood
  piles' pick-up and put-down already work (ROADMAP Done, "Carry furniture"),
  so this is mostly step 1 running the tree with `GetGraphVariableInt` read
  from the actor's graph. "Wait after Carry End" and "StopMovement" are read
  from their names; record that.
- This replaces "any wood pile it happens to pick" for sandboxers whose
  package says where the load goes. Buckets and DLC2 stone piles stay blocked
  (the animation item's reasons).
- **The sawmill cycle**: the workers' Activate packages already use the lever
  and log pile. Trace `ResourceObjectSawMillScript` and the lever's and
  mill's scripts (`vrm-tool pex-dump`) and graphs (`hkb-events`) to find what
  is missing between the lever's activation, the mill's graph and the logs.
  Fill the gaps with the object-graph machinery already in place. If the
  trace finds a missing native or event, implement it here.

### 6. Find, Acquire and Flee procedures

- **Find** (36 tree uses): fill the package's ObjectList with the matching
  references in the search location (the step 3 resolver), and with matching
  items in the actor's own inventory when "Search own inventory" is set.
  Shuffle them when "Randomize Find List" is set.
- **Acquire** (5 packages: Ralof's door key, Jon's note, Thorald's weapon, the
  WIRemoveItem bystanders): Travel → (Find ∥ Wander) → Acquire. Walk to each
  found item and pick it up (`Engine::transfer_item`, with the pick-up as the
  player's activation would do it), until "Num to acquire".
  - AllowStealing: taking owned items is a crime, as the player's is
    (`src/crime.rs`); without it, owned items are skipped.
  - AllowPickpocketing: step 0 found `MQ101RalofGetDoorKey` (`0008C816`)
    explicitly sets it, and AllowStealing, true. Trace that package's target
    and MQ101 scripts before deciding what inventory transfers it requires;
    the planning assumption that no vanilla package sets it was incorrect.
    AllowKilling is never explicitly enabled; leave it unimplemented rather
    than guess.
- **Wander** (5): sandbox about the location without furniture.
- **Flee procedure** (FleeFrom 11, FleeTo 8, and MQ104's "back away from
  dragon"): run from the targets in the Find results or "Flee From Target"
  until "Distance to Flee" away, or to "Flee To Location" when one is given.
  Reuse combat's `flee_step` for movement and for the open-ground choice
  already recorded (`known_ambiguities/combat-fleeing.md` item 9). Read
  "Quiet" as no `FLEE` lines and "Forget dead threats" as dropping dead
  targets. Fall back to `fFleeDistanceExterior` / `Interior` when the
  distance input is 0; item 10 there suggested those settings belong to this
  procedure. Use the threat list only when the package names none.
- **KeepDistanceFrom** (1, a deer): flee only while nearer than the distance.

### 7. Idle markers, keeping an eye on, and groups

- **UseIdleMarker** (65): go to the marker and play its idles (the sandbox's
  idle-marker code, `src/ai/idles.rs`), for as long as the package runs.
  Don't wander off to other markers.
- **KeepAnEyeOn** (SandboxAndKeepEyeOn 36): while the target is in the
  "Observation Area", the actor head-tracks it, and turns to it when standing
  idle. The "EndPursuitArea" input suggests following the target out of the
  observation area until that area. Implement only the observing, and record
  pursuit as unread in `known_ambiguities/package-trees.md`. Head tracking
  for NPCs needs the look-at controller (`src/world/lookat.rs`); this step
  also gives `SetLookAt` / `ClearLookAt` (19 calls) their natives.
- **GroupMovement** (23; MQ103's soldiers, world encounters' scouts) without
  its Guard branch: the leader (Actor1, or the running actor when it is
  Actor1) travels or patrols. The others follow it with the follow inputs
  and `fFollowSpaceBetweenFollowers` (192) apart. **SingleFileFollow** (1):
  the same, each following the one before.
- **FollowTo** (6): follow until the target reaches the destination, then
  travel there. **FollowPlayer** and **FollowerPackageTemplate**: read their
  trees and map them onto Follow with their inputs.
- **TravelCrouchWhenNearTemplate** (12, MQ103 / MQ301 taking cover): travel,
  sneaking within its radius of the target, as read from its tree.

### 8. Escort and preferred paths

- **Escort inputs**: Follower Min / Max Distance (stand between them while
  waiting), Number of Followers with the Follower(s) list and ObjectList
  (wait for all of them, not only "Target to Escort"), Initial Location
  (travel there first), and Run If Behind Distance. Keep the reading already
  recorded in `known_ambiguities/escort.md`.
- **EscortPlayerWhenNear** (18; Hadvar and Ralof in MQ102, Neloth, Frea):
  step 1's Stacked switch between Escort and Travel on `GetWithinDistance`.
  Add a fixture test on `MQ102HadvarEscortPlayerToRiverwood`.
- **Preferred paths**: navmesh triangles carry a "preferred" flag
  (`esp::navmesh` `PREFERRED`). With Travel's, Patrol's, Escort's and
  ForceGreet's "Prefer Preferred Path" set, the A* cost of other triangles
  is raised by a factor. Pick the factor and record it; xEdit and UESP give
  the flag but no weight. The sandbox's "Preferred Path Only" restricts
  wandering to flagged triangles.
- Riding inputs wait for step 14.

### 9. Force greet leftovers

- `IsPlayerMovingIntoNewSpace` (358) is unsupported, so it fails, and it
  gates ForceGreet's, ForceGreetFromSitting's and SayToPlayer's branches.
  Make it true while the player is in a load-door transition (from activating
  a teleport door until the destination cell is attached and the fade ends).
  Record that reading.
- The player keyword check is `RidingInCarriage` (`000DD631`). It already
  works as a keyword condition once something gives the player the keyword.
  Step 15 does that; add a test.
- "Greet Using Preferred Path?": step 8's cost.
- **ForceGreetWaitSitting** (6): sit in the target seat while waiting, then
  get up to greet. Read its tree.
- Trigger locations given by an alias (`PLDT` kind 8 / 9) use the package
  location rule `GetWithinPackageLocation` already uses. Test that with an
  alias-filled trigger.

### 10. Alias leftovers

- **"From alias" locations followed through travel**: a location filled from
  a reference alias's current location (`Fill::RefLocation`) is recomputed
  when that reference changes location (`OnLocationChange`), not only at
  refresh. The same applies to away actors that cross locations in step 12's
  travel.
- **Leveled items in alias inventories** (160 entries): roll the `LVLI` at
  grant time with the reference's level, as containers' leveled entries are
  rolled (`src/leveled.rs`). Grant the result under the existing `CNTO`
  grant rules (once per new placement). Record the roll level and the timing
  in `known_ambiguities/locks.md`, next to the direct-item grant. Form lists:
  none in vanilla; support them only if it falls out of the same code.
- Take "packages for 'from event' aliases" off the roadmap: such aliases are
  now filled from story events (`Fill::FromEvent`), and alias packages run on
  whatever fills an alias (`ai::alias::alias_packages`). Add a regression test
  that proves it.

### 11. Followers

- **Unloaded followers through load doors and fast travel**: today only
  loaded teammates whose goal is Follow on the player come along
  (`known_ambiguities/followers.md`, "Travel and load-door continuity"). Add
  away actors whose current package follows the player, teammates or not
  (quest followers such as Hadvar or Esbern follow by package). They arrive
  with the player as loaded ones do. Record the inclusion of nonteammates,
  and that UESP's Followers notes (revision 3646982) speak only of followers.
- **The player waiting or sleeping**: actors following the player are put
  beside the player afterwards, not where their package's schedule would put
  them (`jump_to_schedules` excludes them).
- **Dismissal by death**: check `DialogueFollowerScript` and the follower
  alias scripts (`pex-dump`) for their death handling. Implement whatever
  native or event they lack, so that a dead follower frees the slot as the
  game's own scripts do. No engine-side dismissal should be invented.
- **`AllyKilled` lines**: when a combatant sees an ally die, it tries the
  `AllyKilled` subtype through the combat line timers that `Flee` and
  `Taunt` use. "Ally" means the same side as `threat.rs` uses; record that.

### 12. Off-screen travel along the navmesh, through doors and between worldspaces

Away travellers walk in a straight line within their space and are put onto
targets in other spaces (`known_ambiguities/actor-execution.md`, "Away").
autotest13 and autotest14 saw some actors with targets 1,900+ units above or
below stay put for minutes; the inference is that they had no path.

- **Parse `NAVI`** (xEdit `Navmesh Info Map`; UESP *Skyrim Mod:Mod File
  Format/NAVI*, revision 3089094). Keep each navmesh's connections, its door
  links (door reference, the navmesh on each side) and its cell / worldspace.
  This gives a coarse graph over every navmesh, loaded or not. Check
  `NVPP` (precomputed pathing) for whether it helps.
- **A coarse route**: A* over the `NAVI` graph from the actor's navmesh to
  the target's, with edge costs from navmesh centre distances, crossing load
  doors as edges. When there is no route, the actor stays put, matching
  autotest13/14. Record the reading.
- **Walking it away**: strides of 84 a second, as now (autotest16), along the
  route. Each crossed load door puts the actor in the door's destination
  cell, with `OnLocationChange` when the location changes. The arrival time
  is the route's length at that speed. Keep the asked-and-in-range jump
  (4,200) and the late jump (35 s) as they are. Being put straight onto a
  target in another space when asked stays as observed.
- **Between worldspaces**: the same route through the doors that link
  worldspaces to interiors. There is no special case beyond the graph.
- **Loaded actors' paths round locked doors**: `NavWorld` treats a locked
  door (load door or animated gate / portcullis that is closed) as a closed
  portal for actors without its key. Actors that own the key or the lock
  unlock it, as sleepers' home doors already are.
- **Stepping round loose clutter** (ROADMAP "Locks"): local steering that
  slows and sidesteps around dynamic physics objects in the capsule's path,
  instead of shoving them. Record the look-ahead and push threshold.
- Comparison: queue an SE probe of a traveller whose straight line and route
  differ a lot (round a mountain, or through an interior). It isn't needed to
  build this.

### 13. Sleepers asking the player to leave

The asking already works (`test-mod autotest12`, ROADMAP Done). An actor
with "Warn Before Locking?" whose home holds the player walks to the player
before asking. Then it stays up (not taking its bed) until the player has
left or the warning has escalated as it does today, and then goes to bed and
locks up. Record the approach distance and the staying-up choice in
`known_ambiguities/locks.md`.

### 14. NPCs riding (mounts)

Inputs named "Ride Horse if possible?" appear in Travel, Patrol, Escort,
Follow, GroupMovement and Acquire. Nothing in the engine mounts an actor
(`IsRidingMount` is always false).

- Which horse: the NPC's own horse, if the data gives one (find how vanilla
  links a rider to its horse: a linked-reference keyword, an alias, or the
  horse's ownership). Look in the data first and record the finding.
- Mounting and dismounting through the horse's and rider's behaviour graphs.
  The mount events and the rider's seated pose come from the graphs
  (`hkb-events`), with the rider attached to the horse's saddle node.
  Locomotion is the horse's, and the rider's package drives it.
- `IsRidingMount`, `GetIsRidingMount`-style conditions, and the natives
  `Actor.IsOnMount` and `Dismount` (vanilla ones only: check with
  `pex-natives`).
- Away: mounted travellers walk at the horse's speed. That is a choice to
  record, because autotest16 measured only walkers.
- The player riding is not part of this plan.

### 15. Carts

Depends on 14. The carriages and the MQ101 cart: `TetherToHorse` (the
carriage held to the horse by a physics constraint) and `SetVehicle`
(passengers seated in the carriage). The DoNothing packages DriveCart,
RideInCart and StayOnCart hold the passengers. Give the player
`RidingInCarriage` while seated (step 9's check). The physics constraint
uses the Havok constraints already simulated for ragdolls.

### 16. Dragons

This is the biggest item, and it comes last because nothing else waits on it.
NPC and dragon shouting are already done (ROADMAP Done), and so is the dragon
behaviour graph project.

- **Flight locomotion**: the dragon graph's flight states (take-off, fly,
  hover, glide, land), driven by a flight controller that steers to a target
  point at the race's flight speeds. `SetAllowFlying` and
  `SetForcedLandingMarker` (vanilla natives) are part of this.
- **Orbit** (58 packages; Orbit 10 tree uses; `fCombatOrbitDistance` 4000):
  circle the target location between the inner and outer radius at the
  height given.
- **Hover** (5, 12 tree uses): hold position in the air at the height.
- **Landing and perching**: land at perch markers (the dragon perch
  furniture), and in combat as `fCombatHoverTimeMin` and the combat style
  say. Record the choices.
- **Shout procedure** (22 packages, including the Greybeards' training,
  MQ105's phantoms and Paarthurnax's demo): cast the package's shout (an
  Object ID target) at its target, on the shouting cadence NPCs already use.
  This part doesn't need flight and can land early. Split it out to follow
  step 3 if that helps.
- **Dragons' default packages** (`DefaultDragonMasterPackageTemplate`):
  their trees through step 1.
- FlightGrab and Alduin's scripted templates come last; read their trees
  first.

## Order and dependencies

0 → 1 → (2, 3) → (4, 5, 6, 7, 8, 9) in any order. 10, 11 and 13 are
independent (11's door crossing for away actors is better after 12). Then
12 → 14 → 15 → 16 (16's Shout part can go right after 3).

## Left out: blocked

These are recorded in ROADMAP item 4 with the same reasons.

- **Guard procedure** (75 tree uses: GuardPost, TravelBackAndForthAndGuard,
  GroupMovement's "Actors Should Guard?", the CW hold packages): no public
  source describes what a guard does with its restricted area or "Suspicious
  Of" targets. The settings `fGuardPackageAttackRadiusMult` (0.5) and
  `iGuardWarnings` (2) suggest warnings and then an attack radius, but their
  rules are undocumented. It needs an SE probe: the player entering and
  staying in a GuardPost's restricted area.
- **Fleeing to doors, cover and allies**: the rules behind the
  door / cover / ally flee settings are undocumented
  (`known_ambiguities/combat-fleeing.md` item 9). Combat flight goes to open
  ground until an SE probe shows the choices.
- **Threat avoidance**: the combat style's "Avoid Threat Chance" (`CSTY`,
  UESP default 0.2; there is no `fCombatAvoidThreatsChance` setting) and the
  `AvoidThreat` dialogue subtype exist, but nothing public says what counts
  as a threat or how the actor moves.
- **Actors that can't reach their target backing away**: no public
  description. It needs an SE observation (the player on a ledge melee
  attackers can't reach).
- **Cleaning up created actors**: autotest22 saw nothing removed on `Clear`,
  `Stop` or `CompleteQuest`. When the game removes them (cell reset?) is
  unknown (`known_ambiguities/alias-fills.md`).
- **Target kind 5 and location kind 11**: xEdit names them only "Unknown 5"
  and "Unknown 11". Kind 5 has no vanilla use. Kind 11 is used 18 times, in
  packages' own trees; its meaning has to be read from those users first.
- **Disarmed NPCs picking a weapon back up**: the roadmap's source, UESP
  *Skyrim:Disarm* (revision 3246655), "Taking an enemy's weapon if you have
  lost yours to this shout", lists a possible use for the player, not NPC
  behaviour. No public page says NPCs pick weapons up; *Skyrim:Stalleo's
  Bodyguard* only has her fall back on a carried dagger, which is done. It
  needs an SE observation (shout-disarm an NPC with no spare weapon).
- **Followers' other combat behaviour** (friendly-fire tolerance, orders,
  fleeing): no source (`known_ambiguities/followers.md`).
- **NPC encumbrance effects** beyond `IsOverEncumbered`: unconfirmed
  (autotest21; `known_ambiguities/comparison-conditions.md`).
- Already noted elsewhere: ambush sleepers' reduced detection (waits on
  Detection), buckets and DLC2 stone piles (Animation).

## Open questions

- Whether `Simultaneous` completes on its first child or needs all of them.
  The multi-location reading is "first"; steps 1–2 keep it.
- Whether `Stacked` re-picks its branch mid-run on the evaluation cadence or
  only at package changes. Step 1 re-picks; EscortPlayerWhenNear depends on
  it. An SE probe (an escort whose player walks away and back) would settle
  it.
- The preferred-path cost factor (step 8).
- Whether away travellers follow navmesh routes or straight lines (step 12's
  queued probe).
