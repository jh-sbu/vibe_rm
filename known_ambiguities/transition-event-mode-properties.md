# Transition-effect event-mode inputs

## Public evidence

The community-authored [Skyrim Behavior Editor transition-effect reader](https://github.com/BrannigansLaw/Skyrim-Behavior-Editor-/blob/7b7167cb18e28f435a64069acbf61eca49a07be3/src/hkxclasses/behavior/hkbblendingtransitioneffect.cpp)
reads and writes `eventMode` and lists four enumerators in order: default,
process all, ignore from generator, and ignore to generator. The local public
checkout under `refs/Skyrim-Behavior-Editor-/` was consulted. A local UESP title
search for `hkbTransitionEffect` in Skyrim Mod returned no match.

The public [Souls Modding binding guide](https://www.soulsmodding.com/doku.php?id=topic%3Ahavok_behavior_editing#hkbvariablebindingsets)
describes variables and constant character properties as binding inputs to
object members. That guide concerns another game's Havok version; it does not
establish Skyrim's transition-effect lifecycle or event routing.

Existing asset observations in [state-machine-events.md](state-machine-events.md)
establish the authored event-mode byte at `0x49` and modes 0..2 in the humanoid
project; no effect in that audit uses mode 3. The binding-set layout was already
established in [character-properties.md](character-properties.md). No additional
assets were inspected for this extension. No engine binary data, SDK mirrors or
leaked documents were examined, and no game assets were added to the repository.

## Implemented scope and unresolved behavior

`hkbBlendingTransitionEffect.eventMode` now retains whole-member graph-variable
and scalar character-property inputs. They resolve when a transition commits,
including after an initiate-interval wait. The selected mode remains fixed for
that fade even if its variable changes. Properties resolve case-insensitively
and stay read-only, outside the animation-variable namespace.

Variables take precedence over duplicate properties. Missing characters, names,
non-scalar properties, missing variable indices and values other than exactly
0, 1, 2 or 3 preserve the authored mode. Invalid variables do not fall through
to a duplicate property. No float-to-integer cast is used for bound enums, so
fractions, infinities and NaNs cannot accidentally select a valid mode.
Single-bit bindings are excluded by the existing binding reader.

The extension uses the existing routing approximation: `PROCESS_ALL` offers
events to the fading source as well as the destination; `DEFAULT` and
`IGNORE_FROM_GENERATOR` offer them only to the destination. The valid enum
`IGNORE_TO_GENERATOR` is retained and selected by bindings but still uses that
same existing destination-only approximation. It is **not** rejected as an
invalid enum and does **not** fall back to the authored mode. Its intended
source-only routing remains unimplemented. The enum names alone do not resolve
which external, generated or lifecycle events should be suppressed, their
interaction with nested machines/modifiers, or interruption ordering.

Original-engine sampling timing, default-mode semantics, invalid/duplicate
binding policy and exact event delivery remain unresolved. Runtime property
overrides, other effect members and static event-to-clip planning are unchanged.
Synthetic tests cover parser retention and bit-binding exclusion, per-character
inputs, read-only properties, absent/non-scalar/invalid fallbacks, variable
precedence, pending-commit sampling, fixed routing during a fade and removal of
the source after the fade. Mode 3's test checks the documented approximation,
not original-engine parity.
