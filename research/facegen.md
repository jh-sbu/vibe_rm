# FaceGen heads: how a record becomes a head

Research for `plans/character-creation.md`. Sources: game assets (Skyrim.esm,
`Skyrim - Meshes0.bsa`), xEdit's `wbDefinitionsTES5.pas` (record layouts) and
UESP's [NPC_ format](https://en.uesp.net/w/index.php?oldid=3650076) and
[RACE format](https://en.uesp.net/w/index.php?oldid=3650080). Assets were read
with `vrm-tool esp-dump`, `tri-dump`, `nif-verts`, `nif-dump` and a scratch
least-squares fit, outside the repository.

## The data

- **HDPT `NAM0` / `NAM1`** (xEdit "Parts"): each head part names up to three
  `.tri` files by type: 0 "Race Morph", 1 "Tri" (expressions and phonemes),
  2 "Chargen Morph". `MaleHeadNord` (0005162F): `MaleHeadRaces.tri`,
  `MaleHead.tri`, `MaleHeadCustomizations.tri`. `MaleEyesHumanLightBlue`:
  `EyesMale.tri`, `EyesMaleChargen.tri`. Parts without a chargen file
  (hair) have only type 1.
- **The `.tri` files** (layout in `research/audio/lip-format.md`) have the
  same vertices as the part's model, in order: `MaleHead.tri` and
  `MaleHead.nif` both have 898, and their base positions agree to 0.058
  units.
  - The race morph file has one morph per race, named after the race's
    editor ID (`NordRace`, `HighElfRace`, ... `DremoraRace`).
  - The chargen file has one morph per direction of each face slider
    (`NoseLong` / `NoseShort`, `JawWide` / `JawNarrow`, ..., `VampireMorph`)
    and one per preset (`NoseType1..30`, `EyesType2..37`, `LipType1..30`;
    Argonian and Khajiit heads have five of each).
- **NPC_ `NAM9`**: 19 floats, the face sliders, each −1..1, in xEdit's and
  UESP's order. **`NAMA`**: four int32s, the nose, an unused one (−1 on most
  humanoids), the eyes and the mouth presets.
- **NPC_ tints** (`TINI` index, `TINC` RGBA, `TINV` value / 100, `TIAS`
  preset), `QNAM` (skin tone, floats), `HCLF` (hair colour, CLFM), `FTST`
  (face texture set). **RACE head data, per sex** (after `NAM0` and `MNAM`
  or `FNAM`): default parts (`INDX` / `HEAD`), available morphs
  (`MPAI` / `MPAV`), preset NPCs (`RPRM` / `RPRF`), hair colours
  (`AHCM` / `AHCF`), face detail texture sets (`FTSM` / `FTSF`), the default
  face texture (`DFTM` / `DFTF`) and tint masks (`TINI`, `TINT` file, `TINP`
  mask type, `TIND` default colour, then presets `TINC` colour, `TINV`
  default value, `TIRS` index).

## How the pre-built heads are made (verified on assets)

A pre-built FaceGen head (`facegeom/<plugin>/<id>.nif`) holds one
`BSDynamicTriShape` per head part, named after the part's editor ID, with the
part's own vertex count. Alvor's (00013475, `MaleHeadNord` shape) was
rebuilt from `MaleHead.nif` and his record as follows, and matched his
pre-built head to 0.047 units at worst and 0.0025 on average. `nif-verts`
prints two decimals, so this is the print precision:

    head = base
         + race morph named after the NPC's race (weight 1)
         + SkinnyMorph × (1 − NAM7 / 100) from the expression file
         + for each NAM9 slider v: the positive morph × v if v > 0,
           the negative one × −v if v < 0
         + NoseType<NAMA.nose> + EyesType<NAMA.eyes> + LipType<NAMA.mouth>
           (weight 1)

A free least-squares fit over all race and chargen morphs gave the same
weights (`NordRace` 1.001, `NoseType5` 1.000, `EyesType14` 0.987,
`NoseShort` 0.999, `JawNarrow` 1.000, `EyesMoveIn` 1.017, ...).

Slider directions, as confirmed by Alvor's head (negative → positive):

| NAM9 | − | + |
|---|---|---|
| 0 Nose Long/Short | NoseShort | NoseLong |
| 1 Nose Up/Down | NoseDown | NoseUp |
| 2 Jaw Up/Down | JawUp | JawDown |
| 3 Jaw Narrow/Wide | JawNarrow | JawWide |
| 4 Jaw Forward/Back | JawBack | JawForward |
| 5 Cheeks Up/Down | CheeksDown | CheeksUp |
| 6 Cheeks Forward/Back | CheeksIn | CheeksOut |
| 7 Eyes Up/Down | EyesMoveDown | EyesMoveUp |
| 8 Eyes In/Out | EyesMoveIn | EyesMoveOut |
| 9 Brows Up/Down | BrowDown | BrowUp |
| 10 Brows In/Out | BrowIn | BrowOut |
| 11 Brows Forward/Back | BrowBack | BrowForward |
| 12 Lips Up/Down | LipMoveDown | LipMoveUp |
| 13 Lips In/Out | LipMoveIn | LipMoveOut |
| 14 Chin Narrow/Wide | ChinThin | ChinWide |
| 15 Chin Up/Down | ChinMoveUp | ChinMoveDown |
| 16 Chin Underbite/Overbite | Overbite | Underbite |
| 17 Eyes Forward/Back | EyesBack | EyesForward |
| 18 VampireMorph | — | VampireMorph |

UESP's NPC_ page gives each direction (for example "−1 Short to 1 Long").
The asset agrees with it everywhere except row 17: UESP has "−1 Forward to
1 Back", but Alvor's −0.6 is matched by `EyesBack` × 0.6. Human heads have
no `JawBack` morph (Argonian heads do), so a negative jaw value moves only
those heads. 
## Checked on every vanilla NPC

`vrm-tool facegen-check <Data> [npc]` (`crates/facegen`) rebuilds each NPC
that has a pre-built head (3214, the base game, the DLCs and the Creation
Club plugins on this install) part by part, from the NPC's own record, and
compares each part with the pre-built shape of its name. With an NPC, it
prints each part's morphs, and for parts still off, the one extra morph
that would explain what's left. Rules the survey added to Alvor's:

- **Weight.** Every part with an expression `.tri` (`NAM0` 1) takes its
  `SkinnyMorph` × (1 − `NAM7` / 100): heads, mouths, eyes, brows and hair
  alike. Erandur (weight 0) was off by exactly `MaleHead.tri`'s
  `SkinnyMorph` peak, 1.294, without it.
- **The race morph** is named after the race's morph race (RACE `NAM8`)
  when it has one: the vampire races name their living race, and
  `DremoraRace` names `DarkElfRace`. Without it, vampires and Dremora
  were off by the living race's morph peak (`WoodElfRace` 3.999,
  `OrcRace` 3.319, `DarkElfRace` 3.128).
- **Vampires** (races with the `Vampire` keyword, 000A82BB) take the whole
  `VampireMorph`: from the chargen file, or the expression file of parts
  without one (Babette's `ChildMouth`). The last `NAM9` float is not it.
  It is 0 or `FLT_MAX` (UESP: "only ever 00 00 00 00 or FF FF 7F 7F")
  regardless of race, and taken as unset.
- **The base** is the part's model's vertices, not the `.tri` file's:
  starting from the `.tri` vertices put 1900 race-morph-file parts over
  0.05 units.

Results (worst and mean per-vertex error, largest axis, game units):

| Part type | Shapes | Worst | Mean | Over 0.05 |
|---|---|---|---|---|
| 0 misc (mouths, `_1bit` beards) | 6594 | 0.677 | 0.00045 | 18 |
| 1 face | 3214 | 0.124 | 0.00073 | 48 |
| 2 eyes | 3206 | 0.007 | 0.00000 | 0 |
| 3 hair | 3091 | 0.010 | 0.00003 | 0 |
| 4 facial hair | 1453 | 0.739 | 0.00499 | 701 |
| 5 scar | 747 | 0.065 | 0.00164 | 1 |
| 6 eyebrows | 2659 | 0.012 | 0.00052 | 0 |

Every part off by more than 0.124 is a Khajiit beard: their expression
files (`khajiitbeard*.tri`) are among the nine with header fields the
reader refuses (`research/audio/lip-format.md`), so they get no
`SkinnyMorph`. Human beards are off by about 0.1 at worst (mean 0.004),
which no single morph of their files explains. Heads off by more than
0.05 are off by at most 0.124 (Khajiit women), likely rounding in the
pre-built heads. 2579 parts had no shape of their name: all of them are
the "none" parts (`MarksMaleHumanoid00NoScar`, `HumanBeard00NoBeard`,
`BrowsMaleHumanoid12NoBrow`, ...), which have no shape to draw.
