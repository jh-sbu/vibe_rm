# Character properties in behavior modifiers and generators

Implemented in `crates/havok/src/behavior/{mod.rs,runtime.rs}` and
`src/world/behavior.rs`: read-only scalar inputs to supported modifiers and
generator members already handled by the runtime, scalar blender-child weights,
transition-effect duration, and bone-weight array inputs on blender children and
bone switches.

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
- Transition effects resolve the scalar `duration` property at the existing
  transition-commit read site (see below); other effect members remain unsupported.
  Generator members not listed above are retained but not evaluated, including
  forwarding `BSCyclicBlendTransitionGenerator`'s `fBlendParameter` property to
  its child. The inspected humanoid graphs did not show scalar uses there.
  Blender-child bone-weight properties are supported as described below. The static
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

## Blender-child bone-weight properties

The runtime now retains `hkbBlenderGeneratorChild`'s character-property binding
for `boneWeights` and resolves it against the character's existing
`hkbBoneWeightArray` property table. This extends the same property-binding
mechanism described by the community guide above; it does not treat a pointer
property as a scalar or a writable graph variable.

An in-memory audit of installed Skyrim SE game assets found 32 such bindings
across the 17 graphs reachable from the humanoid `0_master.hkx`. They reference
`LeftArm`, `RightArm`, `UpperBody`, `Arms`, `LeftForearm`, `LeftHand`,
`MountedCombatBowBlend` and `SaddleOffsetBoneWeights`. DefaultMale supplies a
99-bone weight array for each of these names. Examples include `blockbehavior.hkx`
(`UpperBody`), `magicbehavior.hkx` (left/right arms), `1hm_behavior.hkx` (arms and
hands), `weapequip.hkx`, and `horsebehavior.hkx` (mounted combat/saddle masks).
The child uses the same binding-set pointer at 0x10, type-1 binding records and
graph-local property-name table already established above. These are observations
of authored assets, not a claim about every game version or mod.

Resolved arrays replace the child's authored `boneWeights` array. Missing
characters, names, or non-array properties retain that authored fallback. Names
resolve case-insensitively, and resolved masks belong to the shared runtime for
that character, so another character using the same graph can supply another
array. Existing mask composition is preserved: parent and child weights multiply
per bone; scalar child-weight normalization and additive handling do not change.

Original-engine handling of missing/mistyped bindings, empty arrays, mismatched
skeleton lengths and per-bone normalization is still unverified. This change
keeps the existing mask policies for those cases: empty arrays add no mask, and
composition treats missing entries as 1. Runtime property overrides remain
unsupported, as do graph-variable pointer bindings on `boneWeights`. Synthetic
tests cover graph-name retention, case-insensitive
resolution, distinct characters, replacing authored masks, nested mask
composition, unchanged scalar weights and missing/non-array fallback. They
validate this supported input path, not those unresolved parity questions.

Only public community documentation and game behavior/character assets were
examined. No engine binary data or SDK mirrors were used, and no game assets
or asset bytes were copied into the repository.

## Blender-child scalar weight properties

`hkbBlenderGeneratorChild.weight` now retains its type-1 character-property
binding and resolves a scalar from the character by graph-local property name.
The public [Souls Modding behavior reference](https://www.soulsmodding.com/doku.php?id=common-refmat%3Ahavok_behavior_reference#hkbblendergeneratorchild)
documents the child's binding set and scalar weight, including its role as a
position relative to a parametric blender's parameter. Combined with the
guide's description of constant character-property inputs, this supports
extending the existing input path. Both documents concern another game's
Havok version. No Skyrim asset using a scalar child-weight property binding
was established for this extension; serialized bindings use the same layout
as the existing asset-backed child bone-weight bindings above.

Variable bindings take precedence over duplicate property bindings; missing
characters, missing names and non-scalar properties preserve the authored
weight. Names resolve case-insensitively. Properties remain read-only and
separate from graph variables. Resolved values enter the existing blender
algorithm: ordinary weights normalize, additive weights keep their existing
clamp, and parametric children use weights as positions. Synchronization uses
the same resolved weights for cycle lengths and trigger-master selection.

Original-engine policies for missing or duplicate bindings, invalid/non-finite
values, and binding update/lifecycle timing remain unverified. This extension
preserves the existing runtime's policies rather than asserting parity. It
does not add `worldFromModelWeight`, other transition-effect inputs, property
overrides, or property evaluation to the static event-to-clip resolver.

Synthetic tests cover parser retention alongside a duplicate variable input,
case-insensitive per-character scalar values, read-only handling, ordinary
normalization, additive clamp and negative-weight handling, authored fallback
for absent or non-scalar properties, live variable precedence, and parametric weights used
by synchronized clips. They verify this input path, not original-engine parity.
Only public community documentation and synthetic fixtures were used for this
extension; no engine binary data was examined and no assets were added.

## Transition-effect duration properties

`hkbBlendingTransitionEffect.duration` now retains its type-1 character-property
binding and resolves the scalar by graph-local property name when the transition
commits. This uses the same binding-set layout and the existing duration read
path for graph variables; variable bindings retain precedence over properties.
Case-insensitive property lookup and authored fallback for missing characters,
missing names and non-scalar values follow the other supported scalar inputs.
Properties remain read-only and outside the animation-variable namespace.

The public [Souls Modding behavior guide](https://www.soulsmodding.com/doku.php?id=topic%3Ahavok_behavior_editing#hkbvariablebindingsets)
describes character properties and variables sharing binding sets that target
object members. This supports extending the shared input mechanism, but concerns
another game's Havok version and does not establish Skyrim transition-effect
lifecycle behavior. An in-memory audit of the installed Skyrim SE behavior
assets found 230 `hkbBlendingTransitionEffect` objects across the 17 graphs
reachable from the humanoid `0_master.hkx`; none bound `duration` to a character
property. This extension therefore does not establish a vanilla Skyrim use of
property-bound duration.

The runtime samples duration when a transition commits, including transitions
waiting for their initiate interval, and keeps that duration while the blend
plays. Its existing cross-fade curve, uninterruptibility and non-positive-duration
handling are unchanged. Exact original-engine sampling and update timing,
missing/duplicate-binding policy, and invalid or non-finite duration handling
remain unverified. Other transition-effect members, runtime property overrides,
and the static event-to-clip resolver's property evaluation remain unsupported.

Synthetic tests cover parser retention of a graph-local property name alongside
a duplicate variable input, distinct characters, case-insensitive scalar lookup,
read-only handling, authored fallback for missing or non-scalar values, and
variable precedence sampled when a pending transition commits. They validate
this implementation's input path, not original-engine parity. Only the public
community guide and game behavior assets were examined for this extension; no
SDK source or engine binary data was examined, and no game assets or asset bytes
were copied into the repository.
