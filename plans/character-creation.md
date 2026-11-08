# Plan: character creation

**Status: in progress** (drafted 2026-11-08). ROADMAP item 6 (UI),
"Character creation: the race menu (`ShowRaceMenu`, `SetInChargen`) and the
player's face built live from FaceGen data (sliders, head parts, tints);
NPCs only use pre-baked heads", and item 5's "the head's morphs (`.tri`),
tint layers (`TINI` / `TINC`), hair colour (`HCLF`) and texture sets
(`HDPT` `TNAM`), with character creation".

Rules as for all work here (AGENTS.md): public sources and game assets only.
Describe `racesex_menu.swf`'s behaviour in prose and keep its disassembly
outside the repository (`$SPIKE_SWF_HOME/out`, `spike/swf/paths.sh`).
Choices made without a source go in a new `known_ambiguities/chargen.md`.
Open questions stay in this plan. Each numbered step is one commit or more;
run `cargo fmt` before each.

## Evidence

- `research/facegen.md`: how a head part's `.tri` morphs, the NPC's `NAM9`
  sliders and `NAMA` presets rebuild a pre-built FaceGen head (Alvor's to
  print precision).
- `racesex_menu.swf` (game asset; the spike's name popup already runs it):
  - The game fills it through `GameDelegate` callbacks:
    `SetCategoriesList(text, flag, ...)` (pairs);
    `SetRaceList(name, description, equipState, ...)` (triples; the entry
    with `equipState > 0` is the current race);
    `SetOptionSliders(text, filterFlag, callbackName, min, max, position,
    interval, sliderID, ...)` (eight per slider; the wide panel shows
    sliders whose `filterFlag` matches the selected category's flag);
    `SetNameText`, `SetRaceText`, `ShowTextEntry`, `ShowTextEntryField`,
    `HideLoadingIcon`, `FadeOut`, `moveCategoriesUp`.
  - The menu calls back: `ChangeRace(index, sliderID)`, about 0.6 s after
    the race list stops moving, with the loading icon shown until
    `HideLoadingIcon`; each slider's own `callbackName(position, sliderID)`
    on change; `ChangeName(name)` (or no argument on cancel);
    `ZoomPC(bool)` when the Head category is entered or left;
    `ConfirmDone()` from the Done button; `PlaySound(name)`;
    `SetAllowTextInput()`, `ShowVirtualKeyboard()`.
  - Categories 0, 1 and 2 are Race, Body and Head (`RACE_CATEGORY`,
    `BODY_CATEGORY`, `HEAD_CATEGORY`). A limited menu
    (`bLimitedMenu`, `ShowLimitedRaceMenu`) starts on Body and can't go
    back to Race.
- Scripts: `Game.ShowRaceMenu` (MQ101's fragments and
  `TriggerRaceMenuScript`), `Game.ShowLimitedRaceMenu` (Dawnguard's
  `DLC1SurgeryScript`), `Game.PrecacheCharGen` / `PrecacheCharGenClear`,
  `Game.SetInChargen` (stored already), `Actor.SetRace` (45 calls;
  `OnRaceSwitchComplete`).

## Steps

1. **A `.tri` reader** in the nif crate (`nif::tri`): positions, morph names,
   scale and deltas, from the diagnostic in `crates/tools/src/audio_formats.rs`
   (which then uses it). Tests on a built file.
2. **Heads rebuilt from a record.** `world::facegen`: from an NPC's race,
   sex, head parts, `NAM9` and `NAMA` (or the player's own appearance,
   step 5), the morphs each head part gets (`research/facegen.md`).
   `vrm-tool facegen-check <Data> [npc]`: rebuild every vanilla NPC's
   pre-built head shape by shape and report the worst and mean errors,
   which settles the slider table for every race (Argonian, Khajiit,
   elves, the elder and child heads).
3. **The player's head drawn morphed.** The loader builds a model variant
   (`#facegen=...`) whose shapes are the part's model with its morphs
   applied; the player's head parts use it in place of the bare models.
   Also the head's texture set (`FTST`, else the race's `DFTM` / `DFTF`)
   and its parts' (`HDPT` `TNAM`), and the hair colour (`HCLF` through
   CLFM) on hair-tinted shaders.
4. **The face's tints.** Composite the race's tint masks (`TINT` files) by
   the actor's layers (`TINI` / `TINC` / `TINV`) into the face tint
   texture the head shader's slot 6 reads, and the skin tone. Check it
   against the pre-built `facetint` textures, as step 2 does for shapes.
5. **The player's appearance as state.** Race, sex, weight, head parts,
   sliders, presets, tints, hair colour and texture set, held on the
   engine apart from the player's NPC record, used by steps 3 and 4,
   saved (a new save chunk), and changed by `Actor.SetRace` on the player.
6. **The race menu.** `Game.ShowRaceMenu` / `ShowLimitedRaceMenu` open
   `racesex_menu.swf` (`src/swf_ui/race_menu.rs`) over the game, filled
   from the playable races (`DATA` flag 0x1), the race presets and the
   sliders; its callbacks change the appearance (step 5) live, `ZoomPC`
   moves the camera to the face, the name popup (`ChangeName`) and
   `ConfirmDone` finish it. Close it as SE does, and send what MQ101 waits for.
7. **Starting a new game** with it: the main menu's New (`plans/main-menu.md`)
   and MQ101's flow, with ROADMAP item 11.

## Open questions

- Which sliders the game lists, under which category, with which labels,
  ranges and intervals: the menu receives them all from the game, and
  neither the movie nor `translate_english.txt` names them. Asset
  candidates: the 19 `NAM9` sliders, the three `NAMA` presets, the race's
  tint masks (`TINP` mask types: skin tone, war paint, dirt, ...), hair,
  hair colour, beard, scars, brows and eyes (`HDPT` types), and the race's
  presets. Labels without a source are a choice for
  `known_ambiguities/chargen.md`, or an SE observation (screenshots of the
  menu's sliders).
- How tint layers blend (order, the mask's channel, multiply or overlay),
  to be settled against the pre-built `facetint` textures (step 4).
- What the game does on `ConfirmDone` beyond closing (the "are you sure"
  message box, MQ101's stage progress, `OnRaceSwitchComplete`).
- `PrecacheCharGen`: likely only loading; a no-op is a candidate.
