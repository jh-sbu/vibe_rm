# Plan: whole-cell sandboxers as their cell loads

**Status: implemented** (2026-10-31). The cell-wide search, load-only
placement, regression fixtures and offscreen autotest7 verification are complete.
The remaining SE questions are listed below and in
`known_ambiguities/unloaded-placement.md`.

The goal is a sandboxer whose package covers a whole interior cell
(`LocationKind::InCell`). In Skyrim SE, when its cell loads, it's at
something to use somewhere in the cell. Previously it started where it stood and
only took up furniture near that spot.

## The evidence

`test-mod autotest7` (run once in SE, 2026-10-21; the readings are in its
module doc, `crates/tools/src/test_mods/autotest7.rs`). `InCellUp` and
`InCellDown` sandboxed in the whole Riverwood Trader, one starting upstairs
and one on the ground floor. While the trader was unloaded, both stayed
where they stood. As the cell loaded, one was at a sweep idle marker and the
other at the cooking pot, both on the ground floor. `known_ambiguities/unloaded-placement.md`
("Sandbox in a whole cell", "The whole-cell sandbox at load") records it.
`known_ambiguities/actor-execution.md` ("Coming back seated") notes that
actors new to a save came back seated (autotest7), but autotest16's Sitter,
back from being away, came back standing.

## What exists (2026-10-30)

- `Engine::package_place` (`src/ai/schedule.rs`) gives an `InCell` package
  `(Place::Interior(c), NaN)`. `unloaded_arrival` keeps such an actor where
  it stood when it is already in that cell. Otherwise it returns NaN, and
  `spawn_actors_with_headings` (`src/engine.rs`) puts the actor on a random
  navmesh point of the cell.
- Spawned actors are `fresh`. Their first `plan` (`src/ai/mod.rs`) calls
  `seek_furniture(goal, snap = true)`, which seats them at once if it finds
  something. For a sandbox, three things limit that:
  - a chance roll (`0.75 - energy / 200`, or 0.9) can skip furniture
    altogether;
  - the search is centred on `goal.centre`, which is the editor position for
    `InCell`, with radius `max(radius, SANDBOX_MAX)` clamped to 200..1200;
  - `FurnitureWorld::free_near` also drops markers more than 400 units above
    or below the centre.

  So an actor upstairs in a large cell rarely ends up at the ground floor's
  cooking pot.
- Actors coming back to a cell they unloaded with resume where they left
  (`ActorExec::resumes_in`) and come back standing. Arrivals through a door
  walk in (`fresh = false`).

## Design

- **Who.** A spawn at cell load that meets all of these:
  - its current package is a Sandbox whose location is `InCell` for this
    cell;
  - it doesn't resume an away execution in this place (those keep
    autotest16's rule);
  - it isn't arriving through a door, isn't dead, and isn't explicitly
    moved (`world_state.moved` with a script position);
  - no scene or force-greet holds it.
- **Where.** One choice from every free furniture marker and idle marker in
  the cell that the package's allow flags permit (`sitting`, `idle_markers`,
  `special_furniture`, as `seek_furniture` builds `kinds`). Ownership
  (`may_use`), claims and disabled furniture apply as now. There's no
  distance or height limit and no chance roll. Two whole-cell sandboxers
  never get the same marker: the reservation happens as each is placed.
- **How.** The actor is put at the marker's entry spot and seated or set
  idling there at once, the way `snap` does now, so the existing
  `seek_furniture(.., snap)` path does the seating. If nothing in the cell
  is free, the actor stands where `unloaded_arrival` put it, as now.
- **Pick.** Uniform over the eligible markers, using the engine's random
  source. autotest7 shows two outcomes, which can't distinguish a uniform
  pick from a weighted one. Record this in
  `known_ambiguities/unloaded-placement.md`.

## Steps

0. **Reproduce it here.** Load autotest7's plugin into this engine
   (`--plugin`, built with `vrm-tool test-mod autotest7` into
   `tests/assets/`). Starting outside Riverwood, log (`RUST_LOG=debug`) which
   path `InCellUp` and `InCellDown` take as the trader loads: the whereabouts
   pending position, whether there's an away stage, `fresh`, and what
   `seek_furniture` reports. Write the findings at the end of this plan.
   Don't commit the plugin; it stays under ignored `tests/assets/`.
1. **Whole-cell furniture search.** Add
   `FurnitureWorld::free_in_cell(kind, cell, may_use)` beside `free_near`,
   with the same filters except distance and height. Unit test with a
   synthetic two-floor cell: a marker 600 units below the actor is offered,
   and one in another cell isn't.
2. **Placement at load.** `seek_furniture` takes a `whole_cell` case:
   - It applies when the goal is an `InCell` Sandbox and the actor is fresh
     from a cell-load spawn (a new flag on the runtime, set in
     `spawn_scheduled_actors` for actors that pass the "who" rules above and
     cleared after the first plan).
   - It searches with `free_in_cell`, skips the chance roll, and keeps the
     existing shuffle and the six-option limit.
   - Engine test (GPU-ignored like the other fixture tests) on a synthetic
     two-floor interior with one chair upstairs and one idle marker
     downstairs, holding the actor upstairs with sitting disallowed. After
     load the actor is at the downstairs marker. A second test: a resumed
     away actor in the same cell isn't moved.
3. **Docs.** In `known_ambiguities/unloaded-placement.md`, replace "The
   whole-cell sandbox at load" with the implemented rule and its open parts
   (the pick's weighting; actors away for a long time). Move the ROADMAP
   item to Done.

## Verification

- Unit and engine tests above, with `cargo test --bin vibe_rm --
  --include-ignored` for the GPU-ignored fixture.
- Offscreen run with autotest7's plugin. Enter the trader through its front
  door, then `--console` to print positions (or `--watch` each actor). Both
  in-cell actors should be at a marker or furniture, on whichever floor it
  is.

## Open questions

- Whether the game weights the choice (nearer, by type, by the package's
  preferences) or picks uniformly. Two readings can't tell.
- Whether an actor away for a long time (not new to the save) is also put at
  something to use, or comes back standing as autotest16's Sitter did. Until
  that's observed, resumed actors keep the resume rule.

## Implementation and verification (2026-10-31)

- `load_cell` is set only by interior cell loading for non-resumed, living,
  unmoved spawns. Ordinary scheduled spawns (including arrivals) do not set it.
  The first plan consumes it and verifies the current package is a matching
  InCell Sandbox, without a scene or force-greet. Existing animation, ownership,
  claims, disabled-marker, shuffle, six-candidate and reservation paths apply.
- The two-floor regression uses synthetic navmesh geometry and package state,
  with an installed vanilla actor and idle animation from the trader. This uses
  real animation acceptance rather than mocking successful seating. It reproduces
  the former local search failing across a 600-unit floor gap, then verifies the
  cell-wide search succeeds despite a random value that rejects ordinary
  sandbox furniture. The upstairs chair is disallowed; the downstairs idle is
  reserved, and a second actor cannot take it. The other fixture reloads a
  standing away execution through Engine and checks it retains its position,
  is not fresh and has no load placement flag.
- Rebuilt `vrm-tools` and generated autotest7 into ignored `tests/assets/`.
  Offscreen runs use an absolute plugin path and `VRM_SEED=7` with debug logs.
  The initial spawn logs for InCellUp/Down show pending positions
  (-244, -13, 240.756) and (100, -40, 0), resume=None, fresh=true and the
  trader load-cell flag. Their first package evaluation selects
  VrmAutoTest7InCell; searches find 7 and 5 candidates, respectively, and
  they start in distinct idle markers 0006410A and 0006410B.
  A separate front-door run (`activate 0001341F`, allowing 480 frames for
  the loading transition) finds 8 and 7 candidates and starts them in
  0006410A and 00084285. At frame 450 their positions are
  (-385.8, -288.4, 0.3) and (-75.1, -298.9, 0.3), both on the ground floor.
  The prior local-search failure is reproduced in the regression fixture;
  the offscreen run checks the implemented path, not a historical binary.
- Full binary suite including ignored tests: 962 passed. The final reload
  fixture was also rerun after strengthening it to exercise Engine re-entry.
  `cargo fmt --check` and `git diff --check` pass.

Reproduction command (requires GPU access and installed vanilla assets):

```sh
VRM_SEED=7 RUST_LOG=vibe_rm=debug WGPU_BACKEND=vulkan \
  target/debug/vibe_rm --cell Riverwood --radius 0 \
  --plugin "$PWD/tests/assets/VrmAutoTest7.esp" \
  --console 'unlock 0001341F' --console 'activate 0001341F' \
  --console '@450 getpos 07000847' --console '@450 getpos 07000848' \
  --wait 480 --watch 07000847 --screenshot /tmp/whole-cell-door.png
```

The plugin slot above is from this local load order; resolve the actors by their
plugin slot when reproducing elsewhere. Door 0001341F is the exterior front-door
reference, confirmed by the interior door 000133E9's XTEL game-asset subrecord.
The wait allows the loading-screen transition to finish before AI verification.
Generated plugins, scripts, captures and raw logs remain uncommitted.
