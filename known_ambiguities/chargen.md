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

## The race menu (provisional, chosen by the user's call to build before an SE observation)

`racesex_menu.swf` receives its whole content from the game, and no public
source or asset lists what the game sends. The user chose (2026-11-08) to
build it with provisional content first and correct it from screenshots of
SE's menu later. Implemented in `crate::chargen` (`CATEGORIES`, `sliders`,
`apply_slider`) and `src/swf_ui/race_menu.rs`. Every point below is a
choice:

- **Categories** past the movie's fixed Race, Body and Head: Face, Eyes,
  Brow, Mouth and Makeup, each a filter bit (the movie's filter is bitwise).
  The labels are plain English, not `$` translation keys.
- **Sliders**, labels and order:
  - Body: Sex, Presets (the race's `RPRM` / `RPRF` NPCs; 0 is the first),
    Skin Tone (the skin-tone mask's colour presets), Weight (0–100),
    Complexion (`FTSM` / `FTSF`), Dirt (none or one of the dirt masks).
  - Head: War Paint (none or one of the paint masks), War Paint Color, Hair,
    Beard, Face Scars, Hair Color (`AHCM` / `AHCF`).
  - Eyes: Eyes, Eye Shape, and the eye face sliders. Brow: Brow Type and
    the brow face sliders. Mouth: Mouth Shape and the mouth face sliders.
  - Face: Nose Type and the other face sliders.
  - Makeup: each other tint mask type's colour preset and strength.
- **Ranges**: face sliders −1..1 in steps of 0.1; tint strengths 0..1 in
  0.05; everything else whole steps over its list. Nose, eye and mouth
  shapes run to the highest `NoseType<n>` ... the face part's chargen
  `.tri` has.
- **Choosable head parts**: HDPT flagged playable and for the sex, not
  extra parts, whose valid races (`RNAM` form list) include the race, in
  record order.
- **The races**: playable (`DATA` flags 0x1), the vampire races left out,
  ordered by name.
- **Changing race, sex or preset** starts over from that race and sex's
  preset NPC (the first for race and sex), keeping the weight.
- **Keys**: R is Done, as the movie's own bottom-bar button art shows it.
  A and D move between categories. The movie registers `moveCategoriesUp`
  for the game to call, but nothing says which key the game uses.
- **Done** brings up the name popup (`ShowTextEntry(true)`,
  `ShowTextEntryField`), which the movie's own name entry was built for.
  A name fades the menu out (`FadeOut`, 0.6 s) and closes it. Cancelling
  the popup leaves the menu open. There's no confirmation message box.
- **The camera** stands in front of the player, level: 170 units from the
  body, 45 from the face on `ZoomPC`. The player sits right of centre and
  the panels on the left. CommonLibSSE names a `RaceSexCamera`, but
  nothing public describes it. There's no race-menu light.
- `Game.PrecacheCharGen` / `PrecacheCharGenClear` do nothing (loading ahead).
- `Actor.SetRace` changes only the player's race, keeping their face, and
  sends `OnRaceSwitchComplete` to their scripts whether or not it changed.
