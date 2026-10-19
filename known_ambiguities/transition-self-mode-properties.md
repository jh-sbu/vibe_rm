# Transition-effect self-transition mode inputs

## Evidence

The public, community-authored [Skyrim Behavior Editor transition-effect reader](https://github.com/BrannigansLaw/Skyrim-Behavior-Editor-/blob/7b7167cb18e28f435a64069acbf61eca49a07be3/src/hkxclasses/behavior/hkbblendingtransitioneffect.cpp)
retains `selfTransitionMode` and lists its four enum values in order: continue
if cyclic / blend if acyclic, continue, reset, and blend. The local checkout in
`refs/Skyrim-Behavior-Editor-/` was consulted. The local UESP title search for
`hkbTransitionEffect` in Skyrim Mod returned no match.

The public [Souls Modding binding guide](https://www.soulsmodding.com/doku.php?id=topic%3Ahavok_behavior_editing#hkbvariablebindingsets)
describes variables and constant character properties as inputs to members
through binding sets. That guide concerns another game's Havok version. These
sources support extending the existing input mechanism, not Skyrim's runtime
sampling schedule or a vanilla use of property-bound `selfTransitionMode`.

The pre-existing asset observations in [state-machine-events.md](state-machine-events.md)
establish the authored byte at effect offset `0x48` and all four values in the
humanoid project. The shared binding layout was already established in
[character-properties.md](character-properties.md). No additional game assets
were inspected for this extension. No SDK mirrors, leaked documents or engine
binary data were used, and no game assets were added to the repository.

## Implemented scope and unresolved behavior

`hkbBlendingTransitionEffect.selfTransitionMode` now retains whole-member
graph-variable and scalar character-property inputs. The runtime resolves them
when a transition commits, including after an initiate-interval wait. It then
uses the existing self-transition algorithm: continue leaves the active state
alone, reset restarts without a fade, blend restarts with the fade, and mode 0
uses the existing cyclic-generator check. This does not change those algorithms
or establish their original-engine parity; see the existing notes above.

Character-property names resolve case-insensitively and remain read-only.
Graph variables take precedence over duplicate properties. Missing characters,
missing names, non-scalar properties and values other than exactly 0, 1, 2 or 3
preserve the authored mode. An invalid variable value or missing variable index
does not fall through to a duplicate property. Single-bit bindings are excluded.
These are implementation policies; exact original-engine handling of invalid, conflicting or bit-specific
bindings remains unresolved.

The commit-time read follows the runtime's existing duration read. Whether the
original engine samples this member on effect activation, on transition request,
on commit or at another boundary needs public evidence or observable game-side
tests. Changes after commit do not revise a restart already chosen. Nested-state
target handling, event modes, other transition-effect members, runtime property
overrides and static event-to-clip planning are unchanged.

Synthetic tests cover parser retention, exclusion of single-bit bindings,
per-character modes, all four enums, cyclic versus acyclic input, read-only
properties, absent/non-scalar/invalid fallbacks, and variable precedence after a
pending transition's input changes. They validate this implementation, not the
unresolved original-engine behavior.
