# Plan: actor execution across loading and processing transitions

**Status: draft** (2026-10-24).

This plan covers the first three entries of ROADMAP Next item 4 (AI depth):
actor execution across loading, processing policies, and the comparison tests
against Skyrim SE. The roadmap names this as the foundation for package trees,
off-screen travel, unloaded actor values and the AI parts of saves. Those come
after it and are outside this plan, except where a step here has to leave room
for them.

## Where things stand

- `ActorRuntime` (`src/ai/mod.rs`) is one struct per *spawned* actor. It holds
  two kinds of state together:
  - durable execution: packages, `current`, `goal`, `stop`, `patrol`,
    `activations`, `finished_package`, `pinned`, `combat`, `health`, `stamina`,
    `bleeding`, `search`, `greet_*`, `alias_gen`, `next_eval`
  - things that only exist while loaded: `skeleton`, `graph`, `anim`,
    `capsule`, `seat`, `ragdoll`, `state` (walk path, furniture phase), the
    per-frame event buffers
- Unloading drops the whole struct (`Engine::unload_cell` in `src/engine.rs`,
  `despawn_actor` in `src/ai/schedule.rs`). Only a few things survive, in
  `world_state`: `Wounds` (health plus a regeneration timestamp), deaths and
  bodies, and script moves (`Moved`). `known_ambiguities/world-state.md` says
  so directly: "Living actors' positions aren't kept when they unload".
- Unloaded actors don't run. `Whereabouts` recomputes where everyone is every
  20 s and on each game half hour, from the current package and a home
  position, without remembering anything between passes. `unloaded_arrival`
  puts an actor at the end point of its journey right away. That end point is
  calibrated by `test-mod autotest6`–`autotest9`
  (`known_ambiguities/unloaded-placement.md`). The same tests also show the game
  moving unloaded actors *gradually*: two had gone 112.13 units within seconds.
- Waiting and sleeping (`jump_to_schedules`) despawn loaded actors and respawn
  them at their package's target. `heal_away` advances wounds by 180 s for
  each game hour.
- Scenes wait for their actors to be loaded (`known_ambiguities/scenes.md`).
- Package lifecycle events don't exist yet: no `OnPackageStart` / `End` /
  `Change`, and package fragments don't run.
- Saves: "Not yet saved: actor procedure progress, AI progress"
  (`src/save/state.rs`).

## Ground rules

- Public sources only (see the clean-room memory): `refs/CommonLibSSE-NG`
  (`AIProcess.h`: `PROCESS_TYPE` High / MiddleHigh / MiddleLow / Low;
  `ProcessLists.h`: the four handle arrays, `initPackageLocationsQueue`,
  `syncPositionQueue`), local UESP, xEdit definitions and Papyrus Index pages.
  CommonLibSSE gives us names and the structure only. It says nothing about
  eligibility rules, update rates or what each tier does, and the plan must not
  pretend it does.
- Matching what players can observe is the goal; copying four internal
  allocations is not. Where the evidence doesn't settle a choice, write it down
  in `known_ambiguities/actor-execution.md` (new). Queue the open questions for
  the comparison test mods.
- Keep current behaviour until a step deliberately changes it. Steps 2 and 3
  are refactors and must leave every existing test passing unchanged. The
  autotest6–9 placement rules stay as compatibility requirements until step 9's
  observations say otherwise.
- Every step is one commit (bigger steps may be a few), with synthetic tests,
  `cargo fmt`, and a ROADMAP update when it lands. New runtime state gets an
  optional save section with a legacy default, a `check_continuity_payload` test
  and a legacy-load test.

## Steps

### 1. Research note and test mod design

**Status (2026-10-25):** done. The research (the Sources section of
`known_ambiguities/actor-execution.md`), `test-mod autotest13` and
`autotest14` are built, and each has run once. Their readings are in their
module docs and in `known_ambiguities/actor-execution.md`, which lists what
the runs settled and what they left open. Steps 5 and 6 below follow them.

- Write up the public sources (now the Sources section of
  `known_ambiguities/actor-execution.md`). It should cover what
  CommonLibSSE names: the process types, `HighProcessData` /
  `MiddleHighProcessData` members, `ProcessLists` queues, and any INI settings
  or game settings about processing that the headers or xEdit's GMST lists
  name. Add what UESP pages say about NPCs travelling off-screen and about
  schedules. Keep source claims separate from inference.
- Design the comparison test mods now so the user can run them while steps 2–5
  are being built. Each one gets a `crates/tools/src/test_mods/autotestN.rs`,
  with report pages kept to about 20 lines. The roadmap lists the cases:
  - walk away and come back: an exterior, an interior, and a buffered cell
  - a long journey across cells and through load doors
  - schedule changes and multi-step packages while the actor is far away
  - interrupting travel with wait / sleep, fast travel, and save / reload
    mid-journey
  - unloading and reloading wounded actors and actors with timed effects
- What to record in each case: processing tier where Papyrus can see it
  (`Is3DLoaded`, `GetParentCell`, `IsInLocation`; vanilla natives only), the
  attached cell, position, current package (`GetCurrentPackage` once step 4
  adds it), game time and real time, and the order events arrive in. A quest
  alias can change an actor's persistence, so each case needs a control actor
  without one.
- Output: a list of open questions in `known_ambiguities/actor-execution.md`,
  each tagged with the test that answers it.

### 2. Split durable execution out of `ActorRuntime` (refactor)

- Add `ai::exec::ActorExec`, holding the durable fields listed above plus
  position, heading, current place and a procedure stage (step 3).
  `ActorRuntime` keeps the loaded-only state and gets an `exec: ActorExec`
  field. Starting with a field rather than a separate table keeps the change
  small: call sites become `a.exec.goal` and nothing else changes.
- Add `Engine::away: HashMap<FormId, ActorExec>`. `remember_actors` /
  `despawn_actor` move `exec` into it. `spawn_actors_with_headings` takes it
  back out when it exists, and otherwise builds a fresh one as it does today.
  `Wounds` becomes part of `ActorExec`, and `world_state.wounds` becomes a view
  kept for the wounds already saved in the `WRLD` section.
- Nothing changes in behaviour yet. Spawning still uses the whereabouts
  position and package choice, and `away` only carries health. Tests: unload
  and reload keeps the `exec` contents. A created actor that's deleted while
  away leaves no `away` entry (`created.rs`, `resets.rs`).

### 3. Package lifecycle

- Make the package run explicit in `ActorExec`: `PackageRun { id, started_at
  (game hours), stage, outcome: Running | Completed | Failed | Interrupted }`.
  It replaces `finished_package` and the scattered completion checks: scenes'
  "done" rules in `scene.rs`, practice and activate counts, multi-location
  sandbox stops.
- Define the transitions, and the rules for resuming versus restarting:
  - a package change ends the old run as Interrupted (or Completed)
  - combat, conversations and scenes suspend the run, and it resumes when they
    end
  - unloading neither ends nor restarts the run
- Papyrus events: `OnPackageStart`, `OnPackageEnd`, `OnPackageChange` on the
  actor's scripts, and the `PACK` begin / end / change fragments (check xEdit's
  `PACK` definition for the fragment layout). These fire for unloaded actors
  too. Whether the game fires them while the actor is away goes in
  `known_ambiguities/actor-execution.md` with a test-mod case.
- `GetCurrentPackage` (on the roadmap's "natives still doing nothing" list)
  returns the run's package for loaded and away actors alike.

### 4. Attaching and detaching

- Spawning from an existing `ActorExec` builds the body to match the run it
  holds:
  - an actor in furniture comes back seated (today's `fresh` path, but chosen
    because the run says so rather than guessed)
  - a walking actor gets a new path from its position to its goal
  - a combat or search state is restored or dropped by a written rule
- Unloading captures the stage the loaded AI reached: walking, using furniture
  (and which piece), idle.
- Scheduled placement (`unloaded_arrival`) becomes the fallback for actors
  that have no `ActorExec` yet. Those are actors the world hasn't touched since
  the game started.

### 5. Advancing away actors

- An away stepper runs every frame on a time budget, the way
  `refresh_whereabouts_step` already spreads its work. For each actor it
  evaluates packages on the package cadence and advances the procedure:
  - Travel depends on where the actor is (autotest13, autotest14). In a
    buffered cell (the exterior or interior the player just left) it moves
    toward its target at about loaded speed. Elsewhere it isn't walked:
    within seconds of the package starting it is put 40 short of a target
    in the same space within range (4,100 moved, 4,950 didn't; what decides
    is open), or stays where it is, and is put exactly onto a target in
    another space (with `OnLocationChange`). `OnPackageEnd` follows 2.5 s
    later. Loading the cell, fast travel and loading a save don't move it
    further.
  - Sandbox and Sit stay where they are, and pick furniture when the actor
    attaches.
  - Follow and Escort track their target's away position.
  - Completed runs move on to the next package.
- This replaces `Whereabouts`' recompute-from-scratch with real progress.
  `whereabouts.of` / `by_place` become indexes derived from `away` and the
  loaded actors, so `actors_for_cell` and door arrivals keep working.
  `unloaded_arrival`'s end point and stop distance still apply at the end of
  each journey.
- An actor walking out through a door (`exiting`) passes into `away` with its
  journey under way instead of being despawned and recomputed.

### 6. Processing policies and time

- `ai::exec::Processing`: `Full` (loaded and stepped every frame),
  `Coarse { cadence }` (away, stepped by the away stepper), and room for more
  tiers between those two. A policy function picks the tier from whether the
  actor is loaded, its distance, and its cell. Its numbers stay named constants
  until step 9 measures them.
- Timestamps: `ActorExec` records the game time and the real time of its last
  update. Catching up applies the elapsed game time to travel and schedules,
  and the elapsed real time only where the current code already uses real time
  (wound regeneration, `Wounds::health_at`). Write down which clock each piece
  of state follows.
- Waiting and sleeping call `advance_away(game_hours)` on every actor,
  loaded ones included, instead of `jump_to_schedules`' respawn. autotest14:
  a two-hour wait put a loaded walker exactly on its target, and moved no
  unloaded actor. Fast travel moved no unloaded actor either. Keep
  respawning as the fallback for loaded actors whose run can't be advanced.
  `heal_away` moves into this step.
- Continuity rules for wounds, regeneration, bleedout (essential actors
  brought down while away) and timed effects on away actors. autotest13:
  damage to an away actor is taken at once, and one brought to nothing dies
  then, with `OnDeath` (we now wait until it loads). Timed effects are gone
  within seconds of unloading, and abilities are applied again on load.

### 7. Scenes with away actors

- Today a scene waits for its actors to load. Once away actors have runs,
  scene packages can progress while their actors are away, and phases whose
  actions need only packages can advance. Dialogue phases keep waiting
  (they need voices and head tracking) until evidence says otherwise.
- Keep the current wait policy behind a switch until a test-mod case settles
  it.

### 8. Saves

- New optional section `AEXC`: every `ActorExec` in `away` plus loaded actors'
  `exec`. It holds the run, procedure stage, position and place, both
  timestamps and the processing tier. Loading installs it before cells attach
  actors (as scene packages are installed today). Saves without it fall back
  to scheduled placement.
- Update the `Not yet saved` doc line in `src/save/state.rs` and
  `known_ambiguities/saves.md`.

### 9. Calibrate against the comparison tests

- Read the step 1 reports (`vrm-tool test-results`) and turn the observed
  rules into tests:
  - re-entry placement and furniture state
  - the speed of off-screen travel and how it crosses load doors
  - the order of events
  - catch-up after wait, sleep and fast travel
  - what the tiers' cadences look like from outside
- Replace placeholder constants with the measured values and record what's
  still unresolved in `known_ambiguities/actor-execution.md`. Rules that were
  only approximations before (the instant arrival in `jump_to_schedules`, the
  whole-cell sandbox at load) either get confirmed or get changed.

## Order and dependencies

Step 1 has no dependencies and should start first, because the user's test
runs take the longest. Steps 2 → 3 → 4 → 5 must go in order. Step 6 needs 5.
Steps 7 and 8 need 3 and 5 and can go either way round. Step 9 needs the
reports and touches everything. Once step 5 lands, the roadmap entries it
unblocks (off-screen travel between worldspaces, package trees' progress kept
across loads, already-unloaded followers going through doors, alias "from
alias" locations followed through travel) can go ahead separately.

## Risks

- `ActorRuntime` is used throughout `src/ai/` and `src/engine.rs`. Step 2's
  field-first split keeps the diff mechanical, but it will be large. Do it in
  a worktree from the pool and land it as one commit.
- Performance: every `ACHR` (thousands) gets an away run. The stepper must be
  budgeted and must skip actors whose run can't change before the next
  schedule slot. `Whereabouts` already covers every `ACHR` in batches of 250 a
  frame, which is the baseline to beat.
- The order of events matters to quest scripts. Until step 9, keep the current
  order of package choice, door locks (`package_door_locks`) and scene starts,
  and only add the new events alongside it.
