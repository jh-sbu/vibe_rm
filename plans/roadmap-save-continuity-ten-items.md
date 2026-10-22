# Ten save-continuity items

This batch advances Roadmap Next 9 (scene state) and the existing magic runtime's
continuity. Implement in order, with one reviewed commit per item. Native `.vrs`
policies do not establish original `.ess` layouts or Skyrim SE lifecycle parity.

1. **Scene phases and timers.** Add an optional scene snapshot section and resume
   a timer scene at its saved phase with its elapsed action time. Replace stale
   runs on load; absent sections clear runs. Preserve existing section layouts.
2. **Scene lifecycle.** Retain begun, stopping, paused and action lifecycle states
   so loading does not replay begin/phase fragments or restart completed actions.
3. **Scene package actions.** Restore active scene package ownership and actor
   bindings without restarting scene phases. Reconstruct packages from plugin
   definitions; document that actor procedure/animation progress is separate.
4. **Scene dialogue continuation.** Retain selected topic/INFO identity, remaining
   response order and current subtitle/deadline without rerunning INFO begin
   fragments. Do not serialize audio handles or asset bytes; document audio policy.
5. **Looping scene dialogue.** Retain the already selected repeat deadline so
   loading does not reroll the interval or immediately select another INFO.
6. **Pending scene repeats.** Save the ordered pending restart queue and restore
   it without starting scenes during snapshot application.
7. **Scene force-greet completion.** Retain scene-owned force-greet completion
   state so a completed interaction does not need to happen again after load.
8. **Scene snapshot validation.** Validate scene/action/phase and dialogue
   references against plugin definitions before mutation; malformed or incompatible
   scene data must leave live state intact. Add corruption and rejection regressions.
9. **Rolled spell-list persistence.** Retain per-actor leveled-spell results so
   reloading does not reroll them. Optional deterministic encoding, replacement,
   legacy clearing, malformed-input checks and actual lookup regression.
10. **Ward-hit state persistence.** Retain the existing absorbed/broken ward state
    and remaining timer for IsWardState across saves. Preserve expiry behavior,
    replace stale state, clear on legacy saves and reject malformed entries.

Each item needs meaningful synthetic behavior coverage and updated roadmap and
ambiguity notes. Research starts with the local UESP index, then public community
documentation/source or installed game assets if necessary. Cite permanent UESP
revision links. Never inspect engine executables/DLLs, leaked documents or SDK
mirrors; never commit game assets. Record unresolved compatibility questions in
`known_ambiguities/` and describe explicit native policies as such.

The Luna 6 High implementation agent must stop after each item for parent review,
make requested fixes, and commit only after the parent approves that concrete
item. Record outcomes and validation below as work proceeds.

## Item 1 outcome

Implemented optional `SCNE` snapshots for active scenes whose actions are all
timers. The snapshot keeps the current phase and, for each timer active in that
phase, its elapsed time or completed state. Applying a snapshot validates scene
and timer references before mutating the current game, replaces stale scene runs
without running fragments, and treats a missing section as an empty scene set.
Non-timer scenes remain unsaved at this step. Updated the scene/save ambiguity
notes and roadmap; local UESP title search for scene phase/timer save behavior
returned no matches, so the `.vrs` policy is explicitly not a claim about `.ess`
or Skyrim SE compatibility.

Validation: `cargo check` passed. Added synthetic tests for preserving phase,
elapsed time and completed/future timer action states, plus optional section
round-trip, deterministic output, legacy clearing, duplicate-section rejection
and non-finite timer rejection. `cargo test timer_` passed (4 passed),
`cargo test --no-run` compiled the synthetic Engine integration regression, and
the parent ran the production save/load regression with Vulkan (1 passed).

## Item 2 outcome

Added optional `SCLF` lifecycle data alongside the unchanged `SCNE` payload.
It retains begun/stopping/paused flags and each action's waiting/running/done/
stopped state for all running scenes. Timer elapsed time is also recorded in
`SCLF` for running timer actions in mixed scenes. Restore keeps the saved scene
phase and lifecycle, so begun/phase-start fragments do not replay and completed
actions remain complete. Running dialogue and package internals remain outside
this item's scope.

Validation: `cargo check` passed. `cargo test scene_lifecycle_section` and
`cargo test saved_action_lifecycle` each passed (1 passed); `cargo test timer_`
passed (4 passed, one GPU integration test ignored). The production integration
regression covers the pre-first-update state, completed timer action, elapsed
timer resumption, a stopping scene and legacy stale-run clearing; its Vulkan
run passed. Parent's full host-Vulkan engine suite passed (341 passed, 107
ignored).

## Item 3 outcome

Added optional `SCAB` records for every action's bound actor, including a saved
empty binding. The adjunct also records whether a package action had installed
scene ownership and the elapsed ownership clock. On load, package ownership is
rebuilt from authored plugin definitions before the saved cell attaches actors;
no Rust `Package` data is serialized. Package procedure/animation progress stays
outside this step, and saves without `SCAB` retain alias-based legacy behavior.

Validation: `cargo check` passed, and `cargo test scene_binding_section`
passed (1 passed). The host-Vulkan Engine regression passed; it covers
pre-dispatch saves, explicit/empty actor bindings, package restoration, elapsed
ownership time, a completed package action waiting for its phase condition, and
update without scene phase replay.

## Item 4 outcome

Added an optional `SDLG` adjunct for the selected topic and INFO, current
response position, remaining response positions, and remaining subtitle
deadline. Restoring the INFO does not replay its begin fragment, and does not
resume or replay its voice handle. A restored line advances through the saved
response suffix and executes the INFO end fragment through the normal finish
path. Legacy saves without `SDLG` retain their existing scene behavior; full
looping-dialogue deadline persistence remains item 5.

Validation: `cargo test scene_dialogue_section` passed (1 passed), and the
production Engine integration regression passed with host Vulkan. It covers
roundtrip continuation to the next response, duplicate response-number handling
by position, and rejecting an INFO belonging to a different topic before
snapshot mutation.

## Item 5 outcome

Added optional `SLRP` deadlines for looping dialogue actions between INFO
lines. The saved relative wait is restored against the loaded runtime clock;
loading and repeated save/load do not choose a new interval. Zero remaining is
valid and allows the next normal scene update to select an INFO. Validation
requires a running looping dialogue action with no active saved response.

Validation: `cargo test scene_repeat_section` passed (1 passed). The
production Engine integration drives a two-response looping action to its
between-line wait, roundtrips it twice, checks the same deadline before it is
due, then reloads a zero-remaining deadline and verifies the next update begins
the next line. The host-Vulkan production regression passed.

## Item 6 outcome

Added optional `SRSQ` storage for the exact pending scene-restart vector. It
preserves order and duplicates, clears on legacy saves, validates referenced
repeating scenes before mutation, and restores the queue without starting scenes
or running fragments during apply. The next ordinary scene update starts queued
scenes through the existing conditions and alias checks.

Validation: `cargo test scene_restart_section` passes the codec,
ordering, duplicate-entry, legacy-clear and malformed-null checks. The
production Engine integration covers queue creation by `end_scene`, rejection
of an invalid reference before mutation, no start during apply, and starting
both queued scenes on the next update; the host-Vulkan production regression
passed.

## Item 7 outcome

Added optional `SFGT` state for scene-owned actors whose force-greet package has
already accepted a greeting. Loading discards any live player conversation,
stopping its voice and clearing gestures without running goodbye, INFO-end or
arrest-completion behavior; the conversation itself is not resumed. A marker
inserted when a greet begins is retained, so a mid-greet save treats the greet
as accepted/completed after load. Force-greet evaluation checks that marker for
an active scene-owned ForceGreet package to avoid reopening the interaction.

Validation: `cargo test scene_force_greet_section` passed (1 passed).
The production Engine integration covers invalid marker rejection before
mutation, in-progress conversation discard, legacy marker clearing, package
completion after load and no repeat greet after another roundtrip. The focused
codec check and parent host-Vulkan production regression passed.

## Item 8 outcome

Snapshot application now checks finite nonnegative clocks and validates every
referenced scene definition, action/phase bounds, lifecycle flags and action
states against the plugin records before unloading the live game. It checks
timer/package start-time reconstruction, dialogue/repeat deadline overflow,
duplicate package owners, timer/lifecycle cross-section consistency and saved
dialogue topic/INFO membership. Optional adjunct absence remains accepted for
legacy files; in particular, a legacy SCNE phase may contain an earlier action
restored as `Done`. Null-topic dialogue actions remain valid for headtracking.

Added production regressions for table-driven transactional rejection, null-topic
headtracking loads, an overflowing repeat deadline and legacy SCNE load/resave
across phases. `cargo test scene_` passed 7 runnable codec tests; the agent's
host-Vulkan save suite passed all 45 tests, including the production Engine
regressions. The parent full host-Vulkan Engine suite passed 346 tests (115
ignored). Rustfmt and `git diff --check` passed.

## Item 9 outcome

Added optional deterministic `RLSP` records for the raw result cached by each
actor and leveled spell list. Empty results, order and duplicates are retained;
loading replaces the whole cache, while saves without the section clear it.
Validation checks nonnull keys/results, sorted unique keys, and that each list
key resolves to an `LVSP` record, without rejecting valid runtime results that
contain duplicate or otherwise unresolved terminal form ids. This is a native
save policy, not a claim about Skyrim SE save behavior.

Updated the NPC template and save ambiguity notes and the magic roadmap. The
local codec test passed, including deterministic encoding, empty and duplicate
results, legacy clearing, duplicate/null keys, duplicate sections and bounds
failures. The parent host-Vulkan Engine regression passed; it changes actor
level and a chance global across load and verifies that saved results (including
an empty result) remain stable, while a legacy load clears the cache and rolls
again. Rustfmt and `git diff --check` passed.

## Item 10 outcome

Added optional deterministic `WSTA` records for each actor's ward-hit outcome
(absorbed or broken) and remaining `IsWardState` hold time. Loading replaces the
map, and a legacy save clears it. The section validates actor ids, supported
states, positive finite remaining time, ordering, duplicates, bounds and
trailing bytes. It does not save or resume active ward effects, ward charge or
graph animation. The local UESP Ward page documents depletion, regeneration and
stagger on break, but not save persistence or this timer; the one-second hold
and its native save policy are explicitly not claims about Skyrim SE saves.

The codec regression passed deterministic encoding, replacement-format round
trip, legacy clearing, malformed state, duplicate section, truncation and
trailing-byte checks. The production regression caused an actual ward hit via
the combat magic path, saved and loaded it into another Engine, checked the
remaining timer, then expired it through `update_wards`; it passed on host
Vulkan. Parent final checks passed the full Engine suite (348 passed, 117
ignored) and complete `save::` suite (60 passed), with changed-project-file
rustfmt and `git diff --check` clean. `cargo fmt --all --check` still reports a
pre-existing formatting difference in `vendor/ruffle/core/src/avm1/globals/mouse.rs`,
which was left untouched.

## Batch completion

All ten planned items were implemented by Luna 6 High, reviewed by the parent,
and committed independently. Research used only local UESP material and public
references; tests use generated synthetic records. No engine binaries, leaked
or SDK material, or game assets were inspected or committed. Native `.vrs`
policies and unresolved Skyrim compatibility questions remain documented in
`known_ambiguities/`.
