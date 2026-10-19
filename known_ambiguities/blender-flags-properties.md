# Blender flag inputs

`hkbBlenderGenerator.flags` now resolves graph variables and read-only scalar
character properties through the existing member-binding path. The supported
bits remain synchronization (1), parametric blending (16), and cyclic parametric
blending (32). Pose weighting, synchronized cycle lengths, nested blends and
trigger-master selection share the resolved flags and weights. Loop detection
still comes from the active clips. Additive handling retains its existing
normalization and clamp rules.

## Public evidence

The community-authored [Skyrim Behavior Editor blender implementation](https://github.com/BrannigansLaw/Skyrim-Behavior-Editor-/blob/master/src/hkxclasses/behavior/generators/hkbblendergenerator.cpp)
reads and writes the named `flags` member alongside a binding set. Its
[blender header](https://github.com/BrannigansLaw/Skyrim-Behavior-Editor-/blob/master/src/hkxclasses/behavior/generators/hkbblendergenerator.h)
identifies the three supported flag values above, as well as other flags whose
algorithms this runtime does not implement.
The public [Souls Modding binding guide](https://www.soulsmodding.com/doku.php?id=topic%3Ahavok_behavior_editing#hkbvariablebindingsets)
describes variables and constant character properties as inputs to object
members, and shows whole-member bindings using `bitIndex` 255. That guide concerns
another game's Havok version. These sources support extending the existing
binding mechanism; they do not establish Skyrim's precise evaluation timing or
a vanilla Skyrim use of bound blender flags. No additional game assets were
examined for this change.

## Supported policy and unresolved questions

- Scalar properties resolve by case-insensitive character name. Graph variables
  take precedence over duplicate properties. Missing characters, names, and
  non-scalar values preserve the authored flags. Properties remain outside the
  animation-variable namespace and cannot be written through its setters.
- Finite integral inputs in 0..65535 are accepted without a narrowing or
  saturating cast changing their value. Negative, fractional, out-of-range and
  non-finite inputs preserve the authored flags; an invalid variable does not
  fall through to a duplicate property. This validation is an implementation
  safeguard, not an established original-engine conversion rule. Unsupported
  bits retain the existing policy of having no effect.
- Only whole-member `flags` bindings (`bitIndex` 255 / -1) are evaluated. Other
  bit indices are excluded from the parsed variable/property inputs, preserving
  authored flags when no whole-member input exists. The binding representation
  does not retain bit selectors; their conversion, combination and precedence
  require further public or asset evidence. This exclusion is specific to
  `flags`; other members' existing parsing is unchanged.
- Inputs are read at the existing weight and update sites. Original-engine
  activation/update sampling, lifecycle gates and conflicting-binding behavior
  remain unverified. Changing synchronization while active retains this runtime's
  existing blend phase: independent playback does not advance that phase, so
  turning synchronization on can reposition clips. Resynchronization policy
  needs separate evidence; this change does not infer a new phase algorithm.
- Smoothing, zero-weight-child deactivation, dense-pose flags, explicit sync
  master selection, runtime property overrides and property evaluation in the
  static event-to-clip resolver remain unsupported.

Synthetic tests cover parser retention and single-bit exclusion, separate
character values, case-insensitive/read-only properties, authored fallbacks,
invalid values, live variable precedence, ordinary/parametric/cyclic/synchronized
weights and times, wrapped clips, trigger masters, nested synchronized blends
and additive normalization. They verify these implementation policies rather
than original-engine parity. Only public community documentation/source and
invented fixtures were used; no leaked documents, SDK mirrors, game-engine
binary data or game assets were examined or added to the repository.
