# Clip playback-mode binding inputs

`crates/havok/src/behavior/runtime.rs` resolves `hkbClipGenerator.mode` scalar
character-property and graph-variable bindings when a clip activates. Values
0, 1, 2 and 3 select the runtime's existing single-play, looping,
user-controlled and ping-pong paths. Graph variables take precedence over a
duplicate property binding; property names resolve case-insensitively, and
properties remain read-only and outside the animation-variable table.

## Public evidence and provenance

The community-authored [Skyrim Behavior Editor clip class](https://github.com/BrannigansLaw/Skyrim-Behavior-Editor-/blob/master/src/hkxclasses/behavior/generators/hkbclipgenerator.cpp)
lists those four playback modes in order, reads/writes the `mode` member,
and retains a variable binding set on the clip. The community
[Souls Modding behavior guide](https://www.soulsmodding.com/doku.php?id=topic%3Ahavok_behavior_editing#hkbvariablebindingsets)
describes properties and variables as member inputs through binding sets;
the guide concerns another game's Havok version. Together these support
extending the existing input path, but do not establish Skyrim's original
mode-binding lifecycle or a vanilla use of a property-bound mode.

The repository already parses authored modes at node + 0x72 and retains
arbitrary generator binding names. This change introduces no new serialized
layout. Only public community material and invented test data were used.
No game assets were examined or added to the repository, and no SDK sources,
engine executable/DLL data, disassembly or other engine binaries were examined.

## Supported behavior and unresolved questions

The resolved mode initializes the same stored clip-state flags as an authored
mode. Samples, trigger advancement and synchronized blenders' existing loop
detection therefore share the activation's mode. This does not change the
existing ping-pong algorithm, trigger-coordinate policy, synchronization phase
algorithm, crop handling or user-controlled sampling rules. In particular,
user-controlled and ping-pong children inside synchronized blends retain the
existing synchronization behavior; this extension does not establish its parity.

Mode stays fixed for an activation. Changed variables take effect on the next
activation. Original-engine support for changing mode on a running clip, and
whether doing so resets time/direction or changes trigger delivery, remain
unresolved. No mutable character-property API is introduced. The static
event-to-clip planner continues to use authored modes.

Missing names, unavailable characters and non-scalar properties retain the
authored mode. Fractional, non-finite and out-of-range bound values also retain
the authored mode, including when an invalid variable overrides a valid property.
These are local robustness policies: original-engine enum conversion, missing
or duplicate binding handling and invalid-value behavior remain unverified.

Synthetic tests cover named-mode binding retention, authored-mode preservation,
all four mode paths, sample/wrap/end-trigger behavior, distinct property values,
read-only and case-insensitive lookup, absent/non-scalar inputs, invalid enum
values, variable precedence, activation-time sampling and synchronized loop
detection. They verify this implementation's supported path, not engine parity.
