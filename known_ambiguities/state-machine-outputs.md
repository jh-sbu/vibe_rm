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

Observed in Skyrim SE (`test-mod autotest17`, 2026-10-27, the player's
`1HM_Behavior` and `Blocking_Behavior`): a `blockStop` waiting for its initiate
interval during a block hit left `i1HMState` at the source's ID until the
interval opened; `iBlockState` kept its last value (1) after its machine
deactivated; and it read the start state (0) again once the machine activated.
Those three choices match the game. Still unconfirmed: whether the destination
ID is published when a blend begins or when it ends (one trial at Papyrus's
0.17 s polling resolution couldn't separate the player's 0.1 s blend),
publication order relative to modifiers within an update, and competing writers.

Synthetic tests cover nonsequential IDs, child start-state reads, pending
transitions and cross-fades, clip-triggered transitions, and local-to-project
variable mapping across referenced graphs. They validate our chosen behavior,
not the original engine's timing.
