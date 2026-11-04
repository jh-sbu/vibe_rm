# Bone-weight array tails

An authored `hkbBoneWeightArray` may be shorter than the character's Havok
skeleton. The runtime (`havok::behavior::runtime::BoneMask`) gives every bone
past the authored entries a fixed weight, and that weight depends on what
uses the array:

- **Blender children** (`hkbBlenderGeneratorChild.boneWeights`, including
  character-property bindings): weight **1**.
- **Bone-switch children** (`BSBoneSwitchGeneratorBoneData`, including
  `spBoneWeight` property bindings): weight **0**. The child takes only the
  bones it lists.
- **A bone switch's default child** takes what no child does,
  `1 − max(children)` per bone, including past every child's entries (weight
  1 there, since the children's tails are 0).
- Nested masks multiply per bone, tails included, so a switch inside a masked
  blender keeps the blender's trailing ones and the switch's own complement.
- A switch child with no weights at all takes no bones, and the default
  stays unmasked.

## Supporting research

`vrm-tool hkb-masks "<Data>"` surveys every array in the installed Skyrim SE
behavior projects against the project's own Havok skeleton (game assets read
through the repository's VFS; nothing copied, no engine binaries examined).
Details are in [research/first-person.md](../research/first-person.md)
("Bone-weight array survey").

- **Blenders, tail 1:** none of the 631 short blender arrays ends in exactly
  1, which is the shape of arrays saved without trailing ones. 101 are all
  zeros and would be no-ops with a zero tail. Their lengths instead stop just
  before meaningful bones (`Camera Control`, the cameras, everything after
  the root). The first-person drawn-bow mask and
  `1HM_Torch_SprintForwardSword` (right fingers stay with the right hand)
  only work with tail 1. Cross-game corroboration: the
  [Souls Modding Havok behavior notes](https://www.soulsmodding.com/doku.php?id=topic:havok_behavior_editing).
- **Bone switches, tail 0:** vanilla has exactly one short switch array. The
  werewolf's `BSBoneSwitchGenerator` binds its child to the character property
  `cameraOut`, which the werewolf character lacks, so the authored fallback
  `[1.0]` applies (1 of 88 bones, `NPC Root`). The child is the
  `WW_IdleBase` clip and the default is the whole combat/locomotion behavior.
  With tail 1 the idle clip would take every bone and the werewolf could
  never move; with tail 0 it drives only the root. The earlier
  implementation (tail 1 everywhere, a zero-filled default) blended the two
  about 50/50 on every other bone.

## Remaining uncertainty

- This is inferred from authored data, not measured in Skyrim SE. A probe
  plugin can't settle it, since it would need a hand-built behavior file.
- The switch convention rests on one vanilla array, which is also a fallback
  that the original game might never use (if it resolves `cameraOut`
  differently). Mods with short switch arrays follow this choice.
- Overlapping switch children (two children weighting the same bone) add up
  and are normalized per bone with the default, so they share it. Whether
  Havok instead lets one child win is unknown; no vanilla switch has
  overlapping short children.
- Whether an empty switch child array (unresolved property, no fallback)
  means "no bones" is unverified; vanilla has none.
