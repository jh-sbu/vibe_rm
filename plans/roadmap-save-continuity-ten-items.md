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

## Item 1 outcome (parent review pending)

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
