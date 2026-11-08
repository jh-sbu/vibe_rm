# Character creation and heads built live

Implemented in `crates/facegen` (head parts, morphs, tints, texture sets,
hair colour), `crate::world::actor::{head_part_models, FaceTint}`, the
loader's `#facegen=` model variant and `crate::chargen` (the player's
appearance). Evidence in `research/facegen.md`; the plan is
`plans/character-creation.md`.

## Choices made

- **Hair colour without `HCLF`.** A face whose record has no hair colour
  (the player's, 00000007) takes its race's default for its sex (RACE
  `HCLF`, male then female) rather than the hair models' own colour. Every
  NPC with a pre-built head has `HCLF`, so no asset shows what the game
  does without one. The race's default exists to be used for something,
  and the race menu would start from it.
- **The 19th face slider.** The last `NAM9` float is 0 or `FLT_MAX` in
  vanilla records (UESP's NPC_ page: "only ever 00 00 00 00 or FF FF 7F
  7F"). Values outside −16..16 are taken as unset. Vampires' `VampireMorph`
  comes from their race's `Vampire` keyword instead, which fits every
  pre-built head (`research/facegen.md`).
- **Normals of morphed parts** are the model's own, not recomputed after
  the morphs. Whether the game recomputes them isn't settled; the pre-built
  heads' stored normals could be compared the way their positions were.
- **Tint resolution.** A tint built live is 512 × 512, as the pre-built
  ones are, and its masks are sampled nearest at that size.
