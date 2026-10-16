# hkbRotateCharacterModifier

The reader now retains the authored character-rotation settings, and
`Instance::character_rotations` exposes running modifiers with supported scalar
inputs resolved. **No rotation is applied to actors yet.** The roadmap's actual
character-turning integration remains pending.

## Public and asset evidence

- The community-authored [Skyrim Behavior Editor class](https://github.com/BrannigansLaw/Skyrim-Behavior-Editor-/blob/master/src/hkxclasses/behavior/modifiers/hkbrotatecharactermodifier.cpp)
  reads and writes `enable`, `degreesPerSecond`, `speedMultiplier` and
  `axisOfRotation`. Its [editing UI](https://github.com/BrannigansLaw/Skyrim-Behavior-Editor-/blob/master/src/ui/hkxclassesui/behaviorui/modifiers/rotatecharactermodifierui.cpp)
  identifies the first three as bool/real/real and the axis as a vector4, and
  permits variable or character-property bindings on each. These establish the
  authoring vocabulary and types, not the original engine's update algorithm.
- Read-only inspection of installed Skyrim SE **game assets only**, using
  `vrm-tool hkb-obj <Data> meshes/actors/character 0_master.hkx
  hkbRotateCharacterModifier 10`, found `Default_RotateModifier` with enable at
  0x48, rate at 0x50, multiplier at 0x54, and aligned vector at 0x60. The
  separation between the two scalar fields and vector agrees with their public
  types. The reader uses those observed offsets and retains all four vector
  components. The object's trailing runtime-sized storage is not interpreted.
- `vrm-tool hkb-vars <Data> meshes/actors/character turn` identified a
  `degreesPerSecond` binding to `TurnDelta` in `0_master.hkx`,
  `blockbehavior.hkx`, and `mt_behavior.hkx`. Thus authored constant rates alone
  would lose an installed asset's actual input. The meaning of `TurnDelta` as
  supplied by the original engine is not established by this binding alone.

No original game engine binaries, SDK source mirrors, or leaked documents were
examined. No game assets or copies of asset data were added to the repository.
Tests use invented packfile contents and graphs.

## Supported subset and unresolved behavior

- Active settings follow the existing runtime's modifier traversal and
  event-driven activation/deactivation. Authored enable, scalar variable and
  character-property bindings to enable/rate/multiplier are resolved. A binding
  to an unsupported member (including the vector axis), or an unavailable scalar
  input, omits the modifier rather than presenting stale authored input. The
  active-settings accessor neither multiplies rates nor normalizes/interprets
  the axis.
- `degreesPerSecond` names a rate with a degree unit, but the exact rotation step,
  multiplier application, axis coordinate space, sign relative to actor yaw,
  normalization requirements and vector W semantics still need public
  documentation or controlled game-side observation. No formula is implemented
  from the field names alone.
- Composition with clip root motion, actor-facing controls, generator transforms,
  and ordering when multiple rotation modifiers run remain unresolved. The
  current active-modifier traversal does not establish branch weights, masks,
  outgoing transition participation, or original enable-gate lifecycle behavior.
  Integrating this accessor into actor movement before resolving these questions
  could introduce double turns or apply a branch's rotation at full strength.
- The AI and player body already supply `TurnDelta`, but its original-engine
  units and meaning relative to this modifier remain unverified. Turning through
  action-tree events also remains pending. Parsing and exposing these settings
  does not complete the roadmap's NPC turning behavior.

Synthetic tests cover alignment-sensitive field reads, all four axis components,
signed scalar values, authored disable, live scalar bindings, event gates,
character-property inputs and variable precedence, unavailable scalar properties,
and vector variable/property omission. They verify this implementation's data
handling, not original-engine rotation parity.
