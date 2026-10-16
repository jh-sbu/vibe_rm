# hkbTwistModifier

Implemented in `crates/havok/src/behavior/{mod.rs,runtime.rs}` and
`src/world/twist.rs`, applied by `src/world/behavior.rs`.

## Public and asset evidence

- The community [Skyrim Behavior Editor's twist class](https://github.com/BrannigansLaw/Skyrim-Behavior-Editor-/blob/master/src/hkxclasses/behavior/modifiers/hkbtwistmodifier.cpp)
  lists `LINEAR`, `RAMPED` in that order, and model then local axis coordinates.
  Its XML reader/writer establishes member order and signed bone indices. The
  numeric enum interpretation uses that ordering; no SDK sources were used.
- [Staff of Flames's Proper Spell Cast Direction](https://www.nexusmods.com/skyrim/mods/68794)
  documents additive model-axis twists driven by `Pitch`, with spine endpoints
  24 and 26. This supports the purpose of the angle and model-axis setting, but
  does not specify the angle's unit or distribution formula.
- The [Fallout 4 community Havok guide](https://forums.nexusmods.com/topic/9329348-fallout-4-havok-guide/)
  includes a single-bone twist example and reports it can manipulate one bone.
  This establishes single-bone use, not identical behavior between game versions.
- Installed Skyrim SE **assets only**: `vrm-tool hkb-obj <Data>
  meshes/actors/character 0_master.hkx hkbTwistModifier 10` shows axis at 0x50,
  angle at 0x60, signed endpoints at 0x64/0x66, method at 0x68, coordinates at
  0x69, additive at 0x6A, and authored enable at 0x48. The two modifiers have
  matching endpoints (1 or 3), method 0, coordinates 0, additive true.
  `hkb-vars ... Pitch` shows the first bound to `PitchOffset` and enable bound
  to `m_bEnablePitchTwistModifier`. `1hm_behavior.hkx` contains a further
  single-bone twist and `BowAttackSpineTwistModifier` spanning 24..26, bound
  to `Pitch`. No asset bytes were added to the repository; no engine executable
  data was examined.

## Supported subset and unresolved behavior

- **Angle units:** interpreted as degrees, inferred from the documented binding
  to Skyrim's `Pitch` aiming parameter. None of the sources above explicitly states
  this member's unit. A game-side pose experiment remains necessary to confirm it.
- **Single-bone additive LINEAR twists:** apply the full signed angle once to
  the sampled bone orientation. Local-axis turns multiply on the right;
  model-axis turns multiply on the left after conversion through the parent's
  orientation. Translation and scale stay unchanged. Static enable and bound
  enable/angle are honored, along with event-driven activation.
- **Chains and other modes:** LINEAR's exact multi-bone distribution, whether
  endpoints describe a parent chain or a contiguous index interval, RAMPED's
  formula, and non-additive replacement semantics are unresolved. These modes
  remain parsed but unapplied. In particular the vanilla three-bone bow twist
  is still unsupported; no equal-angle distribution is invented.
- **Other bindings:** vector axis bindings and bindings to indices or mode fields
  are not resolved; such modifiers are unapplied instead of using authored
  defaults that might disagree with the binding.
- **Pose order and blends:** twists run as a pass after the final clip/additive
  blend and before foot IK/look-at, in the existing active-modifier traversal
  order. This is an approximation: original child-pose modifier ordering,
  per-branch weights/masks, and transition fading are not established by these
  sources. They require a pose-tree evaluator or further public/game-side
  evidence; this implementation does not claim fidelity for those cases.

Synthetic tests cover field offsets, bound angle/enable, event gates, mapped
indices, signed angles, rotated parents/bones in both coordinate spaces, and
invalid inputs/unsupported modes. They establish implementation consistency,
not agreement with the original engine's unresolved behavior.
