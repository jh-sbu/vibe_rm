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

## Choices not resolved by those sources

- Publish the actual state's ID on activation and when a transition begins,
  before activating the new state's child. Child selectors and nested start
  states can therefore use the value immediately. Publish the ID actually
  selected when an invalid start ID falls back to the first state.
- A pending transition retains the source ID until its initiate interval opens.
  Once a cross-fade begins, publish the destination ID even while the source
  pose still contributes. Do not republish the fading source's ID each frame.
- Leave the last published value on deactivation; do not reset it or overwrite
  script / expression writes on frames without a state change.

Public declarations and assets do not settle publication order relative to
modifiers, per-frame output refresh, deactivation resets, or competing writers
(especially during blends). These need observable tests in Skyrim SE before
the choices above can be treated as confirmed compatibility behavior.

Synthetic tests cover nonsequential IDs, child start-state reads, pending
transitions and cross-fades, clip-triggered transitions, and local-to-project
variable mapping across referenced graphs. They validate our chosen behavior,
not the original engine's timing.
