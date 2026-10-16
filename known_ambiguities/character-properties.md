# Scalar character properties in behavior modifiers

Implemented in `crates/havok/src/behavior/{mod.rs,runtime.rs}` and
`src/world/behavior.rs`: read-only scalar inputs to supported modifiers.

## Public and asset evidence

- The community-authored [Souls Modding behavior guide](https://www.soulsmodding.com/doku.php?id=topic%3Ahavok_behavior_editing)
  describes character properties as constant inputs to variable binding sets.
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
- Shared runtime caching includes the character-file identity. DefaultMale and
  DefaultFemale can use the same root graph while retaining their own property
  values and character driver settings.
- Scalar bindings on generators, blender children and transition effects are
  not implemented in this subset. The inspected humanoid graphs did not show
  scalar uses there. Character-property bone-weight bindings on blender children
  also remain outside this change (bone switches already support them).
- Activation/deactivation callbacks and event-driven modifiers retain their
  existing lifecycle behavior; only the established update and pose input paths
  are extended here. Exact original-engine enable handling at those lifecycle
  boundaries still needs public evidence or an observable game-side test.

Synthetic tests cover typed values, unsupported types, invalid binding indices,
graph-name resolution, separate character values, modifier update/pose gates,
read-only properties, and precedence over duplicate property bindings.
