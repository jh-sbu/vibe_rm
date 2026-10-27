# State-machine output variables

Implemented in `crates/havok/src/behavior/runtime.rs`.

## Public and asset evidence

- The community-maintained [CommonLibSSE-NG state-machine header](https://ng.commonlib.dev/hkb_state_machine_8h_source.html)
  identifies `currentStateID`, separately from `startStateID` and
  `syncVariableIndex`. It establishes the field's existence, not the timing of
  variable publication.
- Inspecting the installed Skyrim SE humanoid behavior project with
  `vrm-tool hkb-vars <Data> meshes/actors/character` finds six
  `currentStateId` bindings across its behavior files. These include
  `Blocking_Behavior` -> `iBlockState`, `1HM_Behavior` -> `i1HMState`,
  `MRhLeftHandOverride` -> `iEquippedItemState`, and `BHR_DragonRider` ->
  `currentDefaultState`. Two magic graphs bind the member to `testInt`.
- The blocking graph's sound expressions read `iBlockState == 0`; the master
  graph's transition conditions read `currentDefaultState`. Together with the
  member name, these bindings support interpreting it as an output of the
  machine's active state ID, rather than an input selecting a state.

## Confirmed in Skyrim SE

Observed with test plugins on the player's `1HM_Behavior` (`i1HMState`) and
`Blocking_Behavior` (`iBlockState`), 2026-10-27:

- **Published as a transition begins**, not when its blend ends
  (`test-mod autotest18`): over 8 trials each, `i1HMState` read the destination
  a mean 0.134 s after `blockStart` (0.1 s blend), 0.132 s after `shoutStart`
  and 0.136 s after `shoutStop` (0.2 s each), against 0.125 s after `blockStop`
  (no blend). The 0.12 s floor is the send and one read.
- **A pending transition keeps the source's ID** until its initiate interval
  opens (`test-mod autotest17`): `blockStop` sent 0.13 s into a block hit left
  `i1HMState` at 1 until 0.75 s into the hit, past the clip's 0.5 s
  `InitiateWinBegin`.
- **Deactivation keeps the last value; activation publishes the start state**
  (autotest17): `iBlockState` stayed 1 after `BlockState` was left, and read 0
  once `blockStart` activated `Blocking_Behavior` again.

The runtime already does all three, so nothing changed.

## Choices still unconfirmed

- Publishing before the new state's child activates, so child selectors and
  nested start states read the value at once; Papyrus reads it only between
  updates, so the order within an update isn't observable this way.
- Publishing the ID actually selected when an invalid start ID falls back to
  the first state (no vanilla graph was checked for an invalid start ID).
- Not overwriting script / expression writes on frames without a state change,
  and competing writers during blends.

Synthetic tests cover nonsequential IDs, child start-state reads, pending
transitions and cross-fades, clip-triggered transitions, and local-to-project
variable mapping across referenced graphs. They validate our chosen behavior,
not the original engine's timing.
