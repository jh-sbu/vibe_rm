# Scalar character properties in behavior modifiers and generators

Implemented in `crates/havok/src/behavior/{mod.rs,runtime.rs}` and
`src/world/behavior.rs`: read-only scalar inputs to supported modifiers and
generator members already handled by the runtime.

## Public and asset evidence

- The community-authored [Souls Modding behavior guide](https://www.soulsmodding.com/doku.php?id=topic%3Ahavok_behavior_editing)
  describes character properties as constant inputs to variable binding sets.
  It explicitly describes these bindings as setting behavior-object members,
  alongside variable bindings, rather than being specific to modifiers.
  That guide concerns another game's Havok version; the Skyrim layouts and
  specific uses below come from Skyrim SE assets, not that guide's schema.
- Inspected only the installed game's `Data` behavior and character assets,
  through this repository's packfile/VFS readers. In DefaultMale's character,
  `hkbCharacterData` has property type information at 0x60 and its value set
  at 0x80; the value set's word array is at 0x10. Property names come from
  `hkbCharacterStringData` at 0x50. Types match the existing graph-variable
  schema: 0 bool, 1..3 signed integers, 4 real, 5 pointer. DefaultMale contains
  `IsMale = true`, `IsThirdPerson = true`, byte `cCharacterSelector = 0`, and
  real `Blend_AttackToBlock = 0.1667`.
- Across DefaultMale's 17 reachable graphs, `mt_behavior.hkx` binds an
  `hkbEvaluateExpressionModifier`'s `enable` to character property `IsMale`;
  `1hm_behavior.hkx` binds an `hkbTwistModifier`'s `enable` to `IsThirdPerson`.
  Binding type 1 identifies properties; the binding's index addresses the
  graph's property-name table, independently of the character's property order.
  Previously these bindings were discarded, so those modifiers ignored their
  character-specific gates.
- No SDK source, engine executable/DLL data, disassembly, or decompilation was
  examined. No asset bytes were copied into this repository.

## Supported subset and remaining questions

- Boolean words become 0 or 1; signed integer words convert to the runtime's
  existing f32 representation; real words are interpreted as IEEE f32 bits.
  Pointer/vector/unknown types are excluded from scalar lookup. Missing names
  and unsupported types leave the existing authored-member fallback in place.
  Integer precision above f32's exact range remains a runtime limitation.
- Modifier update gates and exposed pose settings use property inputs alongside
  graph-variable inputs. Names resolve case-insensitively. Variable bindings
  take precedence if a malformed asset binds both sources to one member.
  Properties stay outside `SetAnimationVariable*` and modifier-output writes;
  runtime property overrides and output-to-property semantics are unresolved.
- Generator registrations retain type-1 bindings using the same graph-local
  property-name table and bindable-object layout as modifiers. The runtime
  resolves scalar properties for `playbackSpeed`, `startTime`,
  `userControlledTimeFraction`, `selectedGeneratorIndex`, `blendParameter`, and
  `startStateId` at their existing input-read sites. Clip start and state-machine
  start are read on activation; speed, user-controlled time, selector index and
  blend parameter follow their existing update/sample paths. Synchronized clips'
  cycle lengths also use property-bound speed. Variable bindings take precedence.
  Missing names or excluded non-scalar values preserve the authored fallback.
  `currentStateId` output writes still target graph variables only: properties
  cannot become animation variables or be overwritten by a state transition.
  This extends input resolution without changing selector switching or other
  activation/update algorithms; their parity limits remain in the respective
  ambiguity notes. The public guide establishes the shared binding concept;
  the existing Skyrim asset observations above establish the serialized layout.
  No additional Skyrim generator-specific use or original-engine lifecycle
  behavior was established in this extension.
- Shared runtime caching includes the character-file identity. DefaultMale and
  DefaultFemale can use the same root graph while retaining their own property
  values and character driver settings.
- Scalar bindings on blender children and transition effects remain unsupported.
  Generator members not listed above are retained but not evaluated, including
  forwarding `BSCyclicBlendTransitionGenerator`'s `fBlendParameter` property to
  its child. The inspected humanoid graphs did not show scalar uses there.
  Character-property bone-weight bindings on blender children also remain
  outside this change (bone switches already support them). The static
  event-to-clip planning resolver still uses its existing variable-default and
  authored-member paths; it does not resolve scalar generator properties.
- Activation/deactivation callbacks and event-driven modifiers retain their
  existing lifecycle behavior; only input resolution at established read sites
  is extended here. Exact original-engine enable handling at those lifecycle
  boundaries still needs public evidence or an observable game-side test.

Synthetic tests cover typed values, unsupported types, invalid binding indices,
graph-name resolution, separate character values, modifier update/pose gates,
read-only properties, and precedence over duplicate property bindings. Generator
tests cover retaining and memoizing bindings, reverse playback with a property
start/speed, user-controlled time, selector inputs and live variable precedence,
state selection and read-only outputs, synchronized parametric blending and
missing/non-scalar fallback. These invented graphs test this implementation's
input handling, not original-engine lifecycle parity.
