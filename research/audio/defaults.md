# Vanilla audio defaults (2026-10-27)

Evidence: read-only `vrm-tool audio-defaults`, `sndr-stats` and `esp-dump` on
installed Skyrim SE game data, plus `Skyrim_Default.ini`'s Audio section. No
engine executables, libraries or process memory were inspected. Asset bytes
and external references are not committed.

Public references read through the local UESP database:

- [DOBJ, revision 3089074](https://en.uesp.net/w/index.php?oldid=3089074):
  DNAM is a sequence of four-byte keys and four-byte form IDs. Contrary to the
  planning survey's summary, UESP does document this record.
- [SNCT, revision 3508410](https://en.uesp.net/w/index.php?oldid=3508410):
  parent, volume, menu defaults and submerged/menu flags.
- [SOPM, revision 3089107](https://en.uesp.net/w/index.php?oldid=3089107):
  attenuation, reverb send, output type and three eight-speaker rows.
- [MUSC, revision 2384538](https://en.uesp.net/w/index.php?oldid=2384538) and
  [MUST, revision 3089093](https://en.uesp.net/w/index.php?oldid=3089093):
  music flags, ducking, fades, cue points, loops, silence and palettes.
- [ASPC, revision 3089064](https://en.uesp.net/w/index.php?oldid=3089064),
  [REVB, revision 3089102](https://en.uesp.net/w/index.php?oldid=3089102),
  [REGN, revision 2309021](https://en.uesp.net/w/index.php?oldid=2309021).

The public [xEdit dev-4.1.6 definitions](https://github.com/TES5Edit/TES5Edit/blob/dev-4.1.6/Core/wbDefinitionsTES5.pas)
were checked for DOBJ labels. Its file header licenses it under MPL 2.0;
this project is MIT. Only factual field names/layouts are used; no xEdit code
is incorporated or translated. The downloaded reference stays in ignored refs/.

The installed winning DOBJ (00000031, SurvivalMode.esl) is sparse: its DNAM
contains zero-key slots and only Survival-named entries. Thus the plan's
assertion that it copies the base defaults is incorrect for these assets.
Resolve the last named entry per key in load order; an explicit named null
clears an entry. See known_ambiguities/audio-defaults.md for the inference.
The assignments and category tree are listed in plans/audio-unblocked.md and
are reproducible with `audio-defaults`. In particular DDSC is VOC, NDSC NonVOC,
MDSC MUS, SFDC SFXFadeDuringDialogue, PIMC PausedDuringMenu,
PDMC PausedDuringMenuFade, PDLC PausedDuringMenuLoad, SMSC AMBr and SSSC
MUSC `_NONE`. UWLS supplies the underwater loop and RVBT the fallback reverb.
DFMS/BTMS/DTMS/SCMS/LUMS/DCMS supply exploration/combat/death/success/level-up/
dungeon-cleared music.

xEdit labels DOP2 "Dialogue Output Model (2D)" and DOP3 "(3D)". The game data
points DOP2 to SOMDialogue3DDefault and DOP3 to SOMDialogue2D. Resolve the
actual model rather than inferring its behavior from xEdit's label.

The game's default INI gives music ducking/unducking 6/8 seconds and menu
fade out/in 3/1 seconds. These become named constants, not a committed INI.
SkyrimPrefs.ini's AudioMenu layout is master plus decimal uIDn/fValn pairs;
SNCT UNAM provides slider defaults. These are data observations; smoothing,
selection and trigger interpretations are documented separately as choices.

## Completion trigger survey

`pex-calls <Data> CompleteQuest` finds vanilla callers including
QF_dunBleakFallsBarrowQST_000354CC, QF_dunGauldursonQST_000E4D31,
CompanionsStoryQuest and dunExploreCompleteSCRIPT. `quest-log` exposes
completion flags on journal entries in addition to direct native calls.
These are game-asset observations, not proof of when music plays. Both
routes already converge in Engine::complete_quest; the provisional success
trigger is documented in known_ambiguities/music.md.
