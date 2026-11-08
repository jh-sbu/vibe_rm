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
those heads. This table is checked against every NPC by
`vrm-tool facegen-check` (`plans/character-creation.md`, step 2).
