# Journal System page contract

Research date: 2026-10-28. Only installed game assets and public references
were examined; no engine binaries or memory. The project's MIT license and
CommonLibSSE-NG's MIT LICENSE were checked. Its default-object enum establishes
the name HelpManualPC; no implementation code was copied.

The installed quest_journal.swf was read using swfspike dis and fold.py.
The following is a description of its interface, not a copy of its bytecode.
Object and array arguments are live movie objects: mutation must finish before
the call returns, even when GameDelegate supplies no response callback.

### The page's states and the game calls it makes

`SystemPage` is a state machine. Its states are `MAIN_STATE` 0 through
`CONTROLS_CATEGORY_STATE` 18, with `TRANSITIONING` 15 while a panel plays its
`start` animation. Category indices are fixed in `onLoad` as Quicksave 0,
Save 1, Load 2, Installed Content 3, Settings 4, Controls 5, Help 6 and
Quit 7. Creations is spliced in at index 4 only when `SetShowMod(true)`.

| When | Menu calls (GameDelegate) | Game answers with |
| --- | --- | --- |
| page first shown | `SetVersionText(field)`, `ShouldShowKinectTunerOption` → `SetShouldShowKinectTunerOption(bool)`, then `UpdatePermissions` | sets `field`'s text; answers false |
| permissions | `SetSaveDisabled(quicksave, save, load, installed, controls, quit, bool)`, passing the category entries themselves | sets each entry's `disabled`, then the menu redraws |
| Quicksave | `QuickSave()` | saves, no answer |
| Save | `UseCurrentCharacterFilter()`, then `SAVE(entryList, batchSize)` | fills `entryList` and calls the `onSaveLoadBatchComplete(true, processed, total)` callback |
| Load | `LOAD(entryList, batchSize)` | as for Save |
| character filter (T) | `PopulateCharacterList(entryList, batchSize)` → `onFillCharacterListComplete(true)`, then `CharacterSelected(id, flags, saving, entryList, batchSize)` | fills the list with one entry per character (`text`, `id`, `flags`), then the chosen character's saves |
| highlight a save | `PrepSaveGameScreenshot(index, entry)` after 750 ms (index − 1 when saving: entry 0 is `[NEW SAVE]`) | `ScreenshotReady()` callback; the menu loads `img://BGSSaveLoadHeader_Screenshot` |
| confirm load | `IsOKtoLoad(index)` → `ConfirmOKToLoad()` callback, then a confirm box, then `LoadGame(index)` | loads |
| confirm save | a confirm box for an existing slot ("Save over this game?"), none for `[NEW SAVE]`, then `SaveGame(index)` after a 1 ms interval, then the menu's own `CloseMenu` | saves; index 0 is a new save |
| delete (X) | a confirm box, then `DeleteSave(index)` (index − 1 when saving) | deletes; the menu removes the row itself |
| leaving the list early | `ForceStopSaveListLoading()` if not all batches arrived | stops filling |
| Settings > Gameplay / Display / Audio | `RequestGameplayOptions(list)` / `RequestDisplayOptions(list)` / `RequestAudioOptions(list)` | fills each entry's `ID`, `value` and `defaultVal`; entries left without `ID` are removed by the menu; Audio may push more entries |
| an option changes | `OptionChange(ID, value)` | applies it live |
| Defaults (T) | `OptionChange(ID, defaultVal)` for each entry | as above |
| leaving Settings | after 1 s, `SaveSettings()` | writes the settings, then the `SettingsSaved()` callback |
| Controls | `RequestInputMappings(list)` (the PC form has no device index) | fills `{text, buttonName, buttonID, sortIndex}` per remappable event; the menu sorts by `sortIndex` |
| remap a row | after a 200 ms debounce, `StartRemapMode(eventText, list)` | waits for the next key or button, updates the row, then the `FinishRemapMode(success)` callback (false shows "That button is reserved.") |
| Defaults in Controls | `ResetControlsToDefaults(list)`, then `RequestInputMappings(list)` | resets the bindings |
| leaving Controls | after 1 s, `SaveControls()` | writes them, then `SettingsSaved()` |
| Help | `PopulateHelpTopics(list)` | pushes `{text, index}`; the menu sorts by `text` |
| a topic | `RequestHelpText(index, titleField, textField)` | sets both fields' text; the menu then converts button tags to art (`ButtonArtHolder.CreateButtonArt`) |
| Quit | `RequestIsOnPC()` → `populateQuitList(true)` (the PC list), then a confirm box | `QuitToMainMenu()` or `QuitToDesktop()` |

Option rows are typed by `movieType`: 0 is a slider (value 0..1 in twentieths:
the scroll bar's position / 20), 1 is a stepper (`options` labels, value =
index) and 2 is a checkbox (0 / 1). PC Gameplay rows: Invert Y, Look
Sensitivity, Vibration, 360 Controller, SaveGameMissingCreationsCheck, Survival
Mode, Difficulty (six labels from Very Easy to Legendary), Show Floating
Markers, Save on Rest, Save on Wait, Save on Travel, Save on Pause (5, 10, 15,
30, 45 or 60 minutes, or Disabled) and Use Kinect Commands. PC Display rows:
Brightness, HUD Opacity, the Actor, Item, Object, Grass, Shadow, Light,
Specularity and Tree LOD fades, Crosshair, Dialogue Subtitles, General
Subtitles and DDOF Intensity. Audio: `$Master`, then whatever the game pushes.

Save list entries (`JournalSaveLoadList`, `SaveLoadPanel.ShowScreenshot`) are
read for `text` (cut to 20 characters), `fileNum` (shown zero-padded to three
digits), `name`, `raceName`, `level`, `playTime`, `dateString`, `corrupt` and
`obsolete`. Character entries carry `text`, `id` (4294967295 when undefined)
and `flags`.

### Where the data comes from

- **Help topics.** CommonLibSSE's `BGSDefaultObjectManager` names default
  object `kHelpManualPC`. xEdit labels it `HMPC`, "Help Manual - PC". In the
  data it is `FLST 00000163 HelpManualPC` (last written by Dawnguard.esm), a
  list of `MESG` records (`HelpSprintLong` and the rest). A topic's title is
  the message's `FULL` and its text the `DESC` (UESP
  [Mod File Format/MESG](https://en.uesp.net/w/index.php?oldid=2308996)).
  `HelpManualInstalledContent*`, `HelpManualMods` and `HelpManualKinect` are
  Creation, mod and Kinect lists, so they are out.
- **Settings names and defaults.** The game's own `SkyrimPrefs.ini` (written by
  the launcher into the install folder) names every option. Examples:
  `bInvertYValues`, `fMouseHeadingSensitivity=0.0125`, `bGamePadRumble`,
  `bGamepadEnable`, `iDifficulty=2`, `bShowFloatingQuestMarkers`,
  `bSaveOnRest` / `Wait` / `Travel`, `bSaveOnPause`, `fAutosaveEveryXMins=15`,
  `fHUDOpacity`, `bCrosshairEnabled`, `bDialogueSubtitles`,
  `bGeneralSubtitles`, `fGamma`, `fLODFadeOutMultActors` / `Items` /
  `Objects`, `fGrassStartFadeDistance`, `fShadowDistance`,
  `fLightLODStartFade`, `fSpecularLODStartFade` and `fTreesMidLODSwitchDist`.
  Audio already uses `[AudioMenu]`'s layout (`src/audio/settings.rs`).
- **Controls.** `controlmap.txt`'s columns 5 to 7 flag whether an event can be
  remapped to a keyboard key, a mouse button or a gamepad button. The Main
  Gameplay context lists the events in a fixed order. `src/swf_ui.rs`'s
  `Buttons` already parses the file for button art, but the game's own keys
  are hard-coded in `src/app.rs`. Quicksave (F5 here, 0x3f) and Quickload
  (F9, 0x43) aren't bound at all.
- **Difficulty.** UESP [Skyrim:Damage, Difficulty
  Level](https://en.uesp.net/w/index.php?oldid=3485809), lines 106–128, gives
  NPC damage taken from ×2 (Novice) to ×0.25 (Legendary) and player damage
  taken from ×0.5 to ×3. It also says fall damage drops below Adept but doesn't
  rise above it, that Equilibrium's health cost changes, and that followers and
  summons count as NPCs. Of the `fDiffMult*` settings, Skyrim.esm writes only
  `fDiffMultHPToPCL = 3`, `fDiffMultXPH = 1.25` and `fDiffMultXPVH = 1.5`.
  The rest are engine defaults the data doesn't hold.
- **What the engine has.** Saves are `.vrs` files with named slots, an
  append-only `quicksave N` and one `autosave` (`src/save/mod.rs`); `HEAD`
  holds the plugins and where the player stands. There is no save metadata
  (name, level, play time, screenshot), no `img://` image source in
  `crates/ui-swf`, no settings file besides `audio.ini`, no input binding
  layer, no difficulty and no draw-distance fades. Subtitles and the crosshair
  are drawn by the HUD (`src/swf_ui.rs`) with no switch.


## States

| State | Number |
| --- | --- |
| Main | 0 |
| Save/load | 1 |
| Save/load confirmation | 2 |
| Settings categories | 3 |
| Options | 4 |
| Defaults confirmation | 5 |
| Input mapping | 6 |
| Quit confirmation | 7 |
| PC quit list | 8 |
| PC quit confirmation | 9 |
| Delete confirmation | 10 |
| Creations list / text | 11 / 12 |
| Help list / text | 13 / 14 |
| Transitioning | 15 |
| Character loading / selection | 16 / 17 |
| Controls category | 18 |

## Argument types and timing

All indices, IDs, counts, flags and values are AS2 Numbers; booleans are
Booleans. Entry lists are Arrays of Objects. Fields passed to SetVersionText
and RequestHelpText are TextField or CLIK text objects. SaveDisabled receives
six entry Objects and a Boolean. SAVE/LOAD/PopulateCharacterList receive an
Array and Number; CharacterSelected receives Number, Number, Boolean, Array,
Number. PrepSaveGameScreenshot receives Number and Object. No-argument calls
and each callback's arguments are listed in the protocol table above.
Request*Options, mappings, Help and permissions mutate synchronously; delayed
screenshot and save completion notifications use GameDelegate callbacks.

## Additional asset findings

The HUD exposes SetCrosshairEnabled(Boolean), which changes crosshair alpha
without hiding activation text. No HUD opacity setter was found; the root
clip's _alpha is suitable. FloatingQuestMarker_mc exists but the host does not
supply floating quest markers yet. SaveLoadPanel.onLoadInit sizes the loaded
bitmap to ScreenshotHolder.sizer.

The launcher preferences put opacity, crosshair, gamepad and autosave toggles
in MAIN, subtitles in Interface, difficulty and floating markers in GamePlay,
look controls in Controls, and the pause interval in SaveGame. The plan's
example sections are illustrative; implementation follows these asset sections.
The installed defaults have bSaveOnPause=0 and fAutosaveEveryXMins=15.
Audio GMST names include sFootstepsVolume (plural), sEffectsVolume,
sMusicVolume and sVoiceVolume. Read these from data rather than assumed names.

UESP was read through scripts/uesp.py's local index. Damage revision 3485809,
lines 106–128, supports the table and fall/follower exceptions. MESG revision
2308996 supports FULL titles and DESC bodies. These are documented claims,
not observations made in Skyrim SE during implementation. Slider ranges,
save labels/order, pause timing, remap conflicts and Help completeness still
need the observations listed in plans/system-page.md.

## Preference defaults used by the host

settings.rs SPECS records every key/section/default. All defaults except the
following were read from the installed launcher's Skyrim/SkyrimPrefs.ini:
Display/fLightLODStartFade=3500 and fTreesMidLODSwitchDist=9999999 come from
Skyrim_Default.ini (also present in preferences). fSpecularLODStartFade was
absent: use the default INI's fSpecularLODMaxStartFade=2000 as the implementation
choice recorded in known_ambiguities/system-page.md. Preferences are never
imported automatically. Audio retains its separate AudioMenu file.

## Tree LOD discovery

The local UESP index returned no BTT format matches. DynDOLOD's public
[Tree LOD documentation](https://dyndolod.info/Help/Tree-LOD) describes
Skyrim's dedicated tree LOD system: separate LST/BTT data files and one texture
atlas per worldspace (not NIF meshes like terrain or object LOD), with each
tree's billboard "shown on 2 double sided planes intersecting each other at a
90 degree angle". It also states that the system has no normal maps and
can show only upright trees. This is third-party format documentation, not an
observation made in Skyrim SE. The existing host reads BTR/BTO only. No SKSE
implementation was consulted.

Audio order: installed SkyrimPrefs.ini AudioMenu uID0–3 identify SFX, FST,
VOCGeneral, MUS. Use Effects, Footsteps, Voice, Music in that order after
Master, followed by additional in-menu categories in FormID order. The movie
itself supplies Master only.

## PC controls device boundary

The host reads all three controlmap columns and remapping flags. PC mappings
list events with keyboard or mouse remapping enabled; gamepad assignments and
flags survive custom-file writes and keyboard/mouse changes. Gamepad-only rows,
360 Controller and Vibration remain hidden until actual gamepad input exists.
The HUD refreshes activation art when the shared map generation changes.

HelpSprintLong's installed DESC contains [Sprint]. ButtonArtHolder scans
square brackets, calls GetButtonFromUserEvent synchronously, then builds
inline image markup using the returned art. Help therefore expands alias/global
tags while preserving bracket controls for the movie; FULL titles are literal
localized text without a prefixed $. HMPC supplies the list and the movie sorts
it alphabetically.

## Vanilla tree billboard format and validation

The local UESP index has no BTT article. Public primary source
[xEdit wbDataFormatMisc.pas, lines 102–133](https://github.com/TES5Edit/TES5Edit/blob/f2a2533c25aea4aeae903af86893bc64e81ccf4a/Core/wbDataFormatMisc.pas#L102-L133)
(dev-4.1.6 at f2a2533; `dfLODTreeLST` and `dfLODTreeBTT`) describes the LST/BTT
fields. Its MPL-2.0 license and this project's MIT license
were checked first: MPL permits a larger work under other terms while covered
files keep MPL obligations. No implementation or source expressions were copied;
the Rust parser independently reads the format facts and remains project code.
The source's license notice was retained in the ignored reference description.

LST is a u32 count and 32-byte rows: type, width/height, four atlas UV floats
and one unknown u32. BTT is a counted list of type/count groups, each reference
32 bytes: position xyz, rotation, scale, FormID and two unknown words. All
words/floats are little-endian. Installed Tamriel.lst is 1092 bytes for 34 types;
Tamriel.4.4.-12.btt parses to completion with positions inside that block's
world coordinates. Asset files were extracted only under /tmp, never refs or
tracked paths. Atlas path is textures/terrain/<world>/trees/<world>treelod.dds.
The host draws each billboard on the two double-sided planes at 90 degrees
that the DynDOLOD documentation above describes; this is not an SKSE extension.

Installed Tamriel atlas UVs include tiny overshoots (for example approximately
−0.00054 and 1.00098). The parser validates finite, ordered rectangles while
preserving these values; restricting them to [0,1] incorrectly rejects vanilla
assets. The synthetic parser test includes this border case.

## Difficulty attacker scope review (2026-10-28)

Local UESP was checked first: [Skyrim:Damage revision 3485809](https://en.uesp.net/w/index.php?oldid=3485809),
Difficulty Level, lines 106–128, supplies target-labelled multipliers and says
followers/summons follow the enemy rule, but does not specify who attacks them.
The [Damage talk page revision 2763142](https://en.uesp.net/w/index.php?oldid=2763142#Difficulty_Level_:_Followers),
lines 66–71, contains conflicting 2013 comments: one describes a player-only
effect; the reply describes all NPC targets receiving reduced damage regardless
of side. These are community discussion and a lead for testing, not verified
Skyrim SE behavior. No local index match for fDiffMultHP supplied an attacker
specification.

[Bethesda's Skyrim SE support article](https://help.bethesda.net/app/answers/detail/a_id/36397/)
describes difficulty changing the player's outgoing and incoming damage. It
neither confirms nor excludes NPC-to-NPC scaling. Inferred ByPC/ToPC setting
names alone cannot resolve that omission. The implementation retains its
target-based rule provisionally; known_ambiguities/system-page.md now records
that choice and plans/system-page.md asks for an SE observation comparing
player → NPC, NPC → player and NPC → NPC damage, including followers/summons.
No engine binaries, SKSE sources or SKSE behavior were examined.

External reference copies remain ignored: refs/system-page-damage.wiki,
refs/system-page-damage-talk.wiki and refs/system-page-difficulty-bethesda.json.
