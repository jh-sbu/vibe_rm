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
