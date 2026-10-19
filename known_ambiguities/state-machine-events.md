# State-machine change notifications and delayed changes

## Evidence

The community-maintained [CommonLibSSE-NG state-machine declaration](https://ng.commonlib.dev/hkb_state_machine_8h_source.html)
identifies `eventToSendWhenStateOrTransitionChanges` at the state machine's
`0x48` member, independently of each state's enter / exit event arrays.
This is a community declaration, not a Havok SDK source mirror. It establishes
the authored member, not its exact notification schedule.

An in-memory audit of installed Skyrim SE assets found no nonnegative IDs in
this member: all 1,136 state machines in the humanoid project's 17 graphs
leave it unset. A broader scan of behavior HKX assets in `Skyrim - Animations.bsa`,
`Skyrim - Meshes0.bsa` and `Skyrim - Meshes1.bsa` found the same result for
3,602 state machines across 423 behavior graph files. These counts describe
the installed assets inspected, not every possible mod or game version.
No game asset files were copied into this repository.

## Implemented scope and unresolved timing

The reader now retains an authored nonnegative event ID and string payload.
The runtime raises it when a transition commits, after installing the
destination state, through the usual graph-local to project-wide event mapping.
The queued event can trigger other transitions, including in the destination.
Transitions waiting for an initiate interval do not raise it until they begin;
a cross-fade raises it when it begins. Clip-triggered and condition transitions
use the same path as external events.

This is a compatibility approximation based on the member's declared purpose.
The declarations and inspected assets do not resolve notification on initial
activation, deactivation, transition completion or interruption; coalescing
several changes in one update; or ordering relative to enter / exit events,
modifiers and variable publication. This implementation adds no extra
notifications at those unresolved boundaries. Observable game tests using an
authored notification and listeners are needed before extending the schedule.
Synthetic tests validate event routing, payloads, pending transitions and
cross-fades; they do not establish the original game's timing.

`FLAG_DELAY_STATE_CHANGE` remains unimplemented. No reliable public community
account or authored asset observation found in this pass established whether
it delays the logical state change until blend completion, delays processing
until a later update, or follows another rule. Its interaction with current
state outputs, enter / exit events, transition notifications, interruption and
nested state machines also remains unresolved. The runtime's existing initiate
interval handling must not be taken as an implementation of that flag.

No engine binary data, disassembly, decompilation or leaked SDK documents were
used to implement this change. Search results pointing to proprietary SDK
mirrors were excluded as evidence.

## Blending effects' self-transition and event modes

`hkbBlendingTransitionEffect` carries a `selfTransitionMode` (byte at 0x48)
and an `eventMode` (0x49). The installed humanoid project holds self modes
0..3 (153, 24, 17 and 36 effects) and event modes 0..2 (150, 75 and 5); no
effect uses event mode 3. The value order and the meanings below are taken from
the enumerators' names as they appear in community-posted behaviour XML
(`CONTINUE_IF_CYCLIC_BLEND_IF_ACYCLIC`, `CONTINUE`, `RESET`, `BLEND`;
`DEFAULT`, `PROCESS_ALL`, `IGNORE_FROM_GENERATOR`, `IGNORE_TO_GENERATOR`); no
public source describes what they do.

Unconfirmed: what we chose.

- A transition into the state the machine is already in (allowed by
  `FLAG_ALLOW_SELF_TRANSITION`) with `CONTINUE` does nothing: no restart, no
  exit / enter events, no change notification. `RESET` restarts the state with
  no blend. `BLEND` (and a transition with no effect) restarts it with the
  authored blend. `CONTINUE_IF_CYCLIC_BLEND_IF_ACYCLIC` continues when the
  state's generator is a looping or ping-pong clip, or a selector / wrapper /
  blender of only such clips, else blends; machines count as acyclic.
- `PROCESS_ALL` also offers events to the state being left while it fades out
  (its nested machines and modifiers hear them); every other mode, including
  `DEFAULT`, leaves it deaf, as before. `IGNORE_TO_GENERATOR` is not acted on.
