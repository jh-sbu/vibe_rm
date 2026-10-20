# Integer generator inputs with bit selectors

The runtime's `Binding` and `PropertyBinding` retain a member name and an input
index/name, but not `hkbVariableBindingSet`'s `bitIndex`. Previously a binding to
one bit of a clip's `mode`, for example, could replace the complete playback
mode. A bit-bound input of zero changed an authored ping-pong clip into a
single-play clip. That behavior did not implement a bit assignment.

The reader now excludes bindings with a bit selector other than 255 (serialized
`-1`) for these additional integer inputs:

- Clip `mode`.
- State-machine `startStateMode`, `syncVariableIndex` and `startStateId`.
- Selector `selectedGeneratorIndex`.

Both graph-variable and character-property bindings use this policy. The
previous whole-member restrictions for `flags`, `selfTransitionMode` and
`eventMode` now share the same predicate, including the direct character-property
lookup path. Excluding an unsupported binding leaves the authored input in
effect unless another supported whole-member input exists. Whole-member
bindings retain their existing precedence and runtime evaluation.

## Public evidence and policy limits

The community-authored [Skyrim Behavior Editor binding header](https://github.com/BrannigansLaw/Skyrim-Behavior-Editor-/blob/master/src/hkxclasses/behavior/hkbvariablebindingset.h)
represents `memberPath`, `variableIndex`, `bitIndex` and `bindingType` separately,
defaults `bitIndex` to `-1`, and distinguishes variable (0) from character
property (1) bindings. Its [reader/writer](https://github.com/BrannigansLaw/Skyrim-Behavior-Editor-/blob/master/src/hkxclasses/behavior/hkbvariablebindingset.cpp)
preserves the bit selector. These sources were read from the public source
checkout under ignored `refs/`. The local UESP title search for
`hkbVariableBindingSet` found no additional binding documentation.

The exclusion is this implementation's conservative unsupported-input policy;
it is **not** a claim that Skyrim ignores bit bindings. The original engine's
bit assignment/conversion rules, evaluation order, duplicate-binding behavior,
and interaction with enum validation remain unresolved. Supporting these
bindings requires retaining their selectors and public documentation or asset
evidence sufficient to establish the operation. The change does not implement
bit bindings for other members or alter their existing parsing.

## Deferred sync-master investigation

The community editor's [blender implementation](https://github.com/BrannigansLaw/Skyrim-Behavior-Editor-/blob/master/src/hkxclasses/behavior/generators/hkbblendergenerator.cpp)
names `indexOfSyncMasterChild`, initializes it to `-1`, and resets an index beyond
the children array to `-1`. Its [UI](https://github.com/BrannigansLaw/Skyrim-Behavior-Editor-/blob/master/src/ui/hkxclassesui/behaviorui/generators/blendergeneratorui.cpp)
exposes variable and character-property bindings for it. This supports the
member's existence, but does not settle whether an explicit master selects
trigger delivery, cycle duration, synchronization markers, or a combination;
nor does it establish runtime sampling or the 64-bit serialized layout. An
explicit-master runtime implementation is therefore deferred. The existing
heaviest-child trigger selection and blended cycle-length policy are unchanged.

Synthetic tests cover both input kinds, whole-member retention, selectors
0/1/7/15/31/254, all three reader lookup paths, and a parsed clip run through
runtime playback. The latter verifies ping-pong fallback for a bit input and
single-play behavior for the corresponding whole-member input. No game assets,
engine binaries, leaked documents or SDK mirrors were examined or added.
