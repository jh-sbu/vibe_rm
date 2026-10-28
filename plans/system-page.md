# Plan: the journal's System page

**Status: planned** (2026-10-28). Covers everything on the journal's System
page (`quest_journal.swf`, ROADMAP Next item 6, "The journal's rest") that can
be built today: Quicksave, Save, Load (with the character filter, screenshots
and deleting), Settings (Gameplay, Display, Audio), Controls (key remapping),
Help and Quit. It also covers the work under those entries that ROADMAP lists
elsewhere: the save / load menu (item 9), the Audio sliders' UI (item 7) and
the autosaves that fast travel, waiting and sleeping are missing.

The page's entries and options that this plan leaves out, and why:

- **Installed Content / Creations** (`$INSTALLED CONTENT`, `$CREATIONS`,
  `ModManager`, `PopulateCreationClubTopics`). These are Creation Club and
  Bethesda.net features, not vanilla content. `ShouldShowMod` stays false.
  The Installed Content entry at index 3 is disabled through
  `SetSaveDisabled` (step 3).
- **Console- and Kinect-only options.** These are the `NX_*` entries, `$Use
  Kinect Commands`, the Kinect tuner (`ShouldShowKinectTunerOption`,
  `OpenKinectTuner`) and `$VisualFidelity_Title`. The movie only adds most of
  them on other platforms. On PC the game leaves an entry out by giving it no
  `ID` (see the contract below).
- **`$Survival Mode`** belongs to the Survival Creation
  (`ccQDRSSE001-SurvivalMode`), and **`$SaveGameMissingCreationsCheck`** to
  Creations. Both are left out.
- **`$360 Controller` and `$Vibration`** wait on gamepad input (gilrs, ROADMAP
  item 6). The settings file stores them from the start (step 5). Their
  entries get an `ID` once a gamepad can be read.
- **`$DDOF Intensity`** waits on depth of field (ROADMAP item 8, Rendering).
  The value is stored, and the entry is listed once DOF draws.
- **Quit to Main Menu** waits on a main menu (`startmenu.swf`). Step 10 removes
  the entry until one exists. Step 11 sketches the main menu and the loading
  screen as follow-on work.

Rules as for all work here (AGENTS.md): public sources and game assets only, no
engine binaries; questions only the running game can answer go to the user (a
test plugin, or a direct observation where a menu is involved, since menus stop
unattended scripts); choices made without a source go in
`known_ambiguities/system-page.md`, open questions here or in ROADMAP. Run
`cargo fmt` before each commit. Each numbered sub-step is one commit.

## What the research found (2026-10-28)

Sources: the movie's ActionScript 2 (`swfspike dis` on
`spike/swf/data/interface/quest_journal.swf`, folded with `fold.py`), classes
`SystemPage`, `SaveLoadPanel`, `JournalSaveLoadList`, `OptionsList`,
`SettingsOptionItem` and `InputMappingList`. Also the game's data
(`vrm-tool esp-list` / `esp-dump`), `interface/controls/pc/controlmap.txt`,
the game's own INI files (`Skyrim_Default.ini` and the launcher-written
`Skyrim/SkyrimPrefs.ini` in the install folder) for setting names and
defaults, CommonLibSSE-NG headers (MIT) for default-object names, and UESP
pages cited per step. Write this section up as `research/ui/system-page.md`
in step 0. Describe the contract in prose and tables, and don't copy the
disassembly.

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

## Steps

### 0. Research note and a driver

- `research/ui/system-page.md`: the contract above with each call's argument
  types, the state numbers, the option row types and the entry fields. Include
  the sources and what each one establishes.
- A `swfspike view quest_journal` driver (`spike/swf`) that answers the System
  page's calls with fixed data: a few saves, the Gameplay rows and a help
  topic. It lets the page be clicked through before the engine side exists, as
  the HUD and message box drivers do.

### 1. Save metadata and the save list

1a. **A `META` section.** It is written first in every save: format version, a
    character id, the player's name, race name, level, location name, game
    date text (the bottom bar's `date_text`), play time in seconds, the time
    written (seconds since the epoch) and the plugin list's hash. `META` is
    optional: older saves list with what `HEAD` gives, plus "?" fields.
1b. **Reading only the header.** `SaveFile::read_prefix` stops after `META`,
    checking that section's CRC, so listing a hundred saves doesn't decode
    them. `list_saves_in` becomes `save_list_in() -> Vec<SaveListing>` with
    the file name, `fileNum`, the metadata, `corrupt` (the container or `META`
    doesn't read) and `obsolete` (the plugin list differs, or `HEAD` refuses
    the current load order).
1c. **Numbering and names.** A save's `fileNum` is the next unused number when
    it is written, kept in `META` and in the file name (`Save 7 - Prisoner
    Riverwood`). The display `text` follows the original's visible pattern:
    the name, location and in-game date, as the user sees it in SE. The exact
    format is a choice to record (see Observations below). Quicksaves and
    autosaves keep their names and are listed with their type's label
    (`sMenuDisplayQuicksaveName` "[Quicksave]" and the autosave GMSTs).
1d. **Characters.** A character id is chosen at new game and kept in every save
    of that game, through `ScriptState` and saved in `META`. The developer
    launch assigns one per launch without `--load`. Saves without `META` group
    under their player name.
1e. **Deleting** (`Engine::delete_save(name)`): remove the file. Refuse names
    outside the save directory, as `save_path_in` does.

Tests: `META` round trip; a save without `META` lists; a damaged `META` marks
the listing corrupt without failing the list; numbering skips used numbers;
`obsolete` with a changed plugin list.

### 2. Screenshots

2a. **Capture.** When the journal opens, keep the last world frame (before the
    menu is drawn over it) as a downscaled RGBA thumbnail (e.g. 384×216). When
    a save is written from the menu, `QuickSave` or an autosave, the thumbnail
    goes into `META` (raw or PNG, whichever the workspace's existing
    dependencies allow).
2b. **`img://` images in `crates/ui-swf`.** A host image source keyed by name
    (`BGSSaveLoadHeader_Screenshot` now, `Local_Map` for the map later). In
    `vendor/ruffle` (one vendor commit), `MovieClipLoader.loadClip` resolves an
    `img://` URL through the host into a bitmap clip, then fires `onLoadInit`,
    which sizes it to the holder.
2c. **The handshake.** `PrepSaveGameScreenshot(index, entry)` reads that save's
    thumbnail, hands it to the image source and calls `ScreenshotReady`. When
    saving, entry 0 is the current game's live thumbnail.

Tests: thumbnail encode / decode; a `swfspike` run that loads an `img://`
bitmap into a clip (its size after `onLoadInit`).

### 3. Saving, loading and deleting from the page

In `src/swf_ui/journal_menu.rs`, answering the calls in the table:

3a. **The lists.** `SAVE` / `LOAD` push every listing into `entryList`, then
    call `onSaveLoadBatchComplete(true, n, n)`. The list's batch size is only a
    hint, so one batch is enough. `UseCurrentCharacterFilter` filters Save's
    list to the current character. `PopulateCharacterList` and
    `CharacterSelected` provide the T filter. `ForceStopSaveListLoading` has
    nothing to stop. The list is ordered newest first (as the original seems
    to be; see Observations).
3b. **`SetSaveDisabled`.** Set `disabled` on Quicksave and Save when
    `saving_disabled()` or the player is dead. Set it on Installed Content
    always. Load, Controls and Quit stay enabled. The game's other refusals
    (combat, scripted) stay with ROADMAP item 9.
3c. **Saving.** `SaveGame(0)` is a new save and `SaveGame(n)` overwrites
    listing n − 1, both with the thumbnail. The menu closes itself after it.
    `QuickSave` saves `quicksave N` and leaves the menu open. Notify
    `sAutoSaving` / the quicksave GMSTs as `flush_save_requests` does.
3d. **Loading.** `IsOKtoLoad` answers `ConfirmOKToLoad` unless the listing is
    corrupt or obsolete. Those get a message box (`sLoadingObsoleteSave`-style
    GMSTs, to be found in step 0), and no load. `LoadGame(index)` closes the
    journal and asks the app to load at the next frame boundary, not inside
    the responder, since loading unloads cells and reopens nothing. A failed
    load shows a message box and stays in the game.
3e. **Deleting.** `DeleteSave(index)` → `delete_save`.
3f. **Keys.** Bind Quicksave (F5) and Quickload (F9, the newest save, as
    `Quickload` does in the original) in `src/app.rs`. Until step 8 they are
    hard-coded, like the others.

Offscreen check: `--console "save one" --console "save two"`, then Esc, the
System tab, Load, and a screenshot of the list with its thumbnail. Tests for
the responder's answers against a fake movie where the existing menu tests
allow it.

### 4. Autosaves

4a. **Save on Rest / Wait / Travel** (`bSaveOnRest` / `bSaveOnWait` /
    `bSaveOnTravel`, default on). Autosave after sleeping, after waiting
    (`src/sleep.rs`) and before fast travel's arrival (`src/fast_travel.rs`).
    Fast travel's autosave timing is already an open entry in
    `known_ambiguities/fast-travel.md`: choose there and refer to it.
4b. **Save on Pause** (`bSaveOnPause`, with `fAutosaveEveryXMins` from the
    stepper: 5 / 10 / 15 / 30 / 45 / 60 / Disabled). Autosave when the journal
    or the tween menu opens and that many real minutes have passed since the
    last save of any kind.
4c. **Rotation.** The original keeps several autosaves (players see
    `Autosave1` to `Autosave3`). The count is a choice: default 3, cycling
    oldest first, recorded with the evidence found.

Tests: each trigger requests an autosave when its setting is on and not when
it's off; rotation replaces the oldest; disabled saving notifies.

### 5. The settings file and the Gameplay options

5a. **`src/settings.rs`.** `Settings` mirrors the original's keys and sections
    (`[Controls] bInvertYValues`, `[Gameplay] iDifficulty`, `[Interface]
    fHUDOpacity`, ...) in `$XDG_CONFIG_HOME/vibe_rm/settings.ini`, next to
    `audio.ini`. It is written atomically like `audio.ini`, with `VRM_SETTINGS`
    overriding the path. Defaults come from `Skyrim_Default.ini`'s values
    where it has them, else the launcher's `SkyrimPrefs.ini`, recorded per
    key. Importing a player's `SkyrimPrefs.ini` is optional and off by
    default, as audio's is. Settings are separate from saves, as in the
    original.
5b. **The protocol.** `Request*Options(list)` sets `ID`, `value` and
    `defaultVal` on each row the engine supports, by its `text`. `OptionChange`
    applies it live and marks the file dirty. `SaveSettings` writes it, then
    calls `SettingsSaved`. Unsupported rows get no `ID`, so the movie drops
    them. A console command (`setting <key> [value]`) uses the same API.
5c. **Gameplay rows.**
    - Invert Y: flips the look pitch in `src/app.rs`.
    - Look Sensitivity: slider → `fMouseHeadingSensitivity`. How the 0..1
      slider maps onto the setting isn't public; see Observations. Until then,
      use linear 0..1 onto [0.0, 0.025], centred on the default 0.0125, and
      record the choice.
    - Show Floating Markers: whether quest targets draw as in-world markers.
      First check `hudmenu.swf` for the floating marker clips and whether the
      HUD draws them. If not, the option is stored and ROADMAP gets the
      markers.
    - Save on Rest / Wait / Travel and Save on Pause: step 4.
    - Difficulty: 5d.
5d. **Difficulty** (`crate::difficulty`). Applies UESP's damage table to damage
    taken by the player and by everyone else, followers and summons included.
    It covers weapons, bashes, spells, falls (reduced below Adept only) and
    hazards. Damage that isn't "damage" keeps its value (UESP Ravage Health:
    Absorb/Ravage aren't multiplied). Equilibrium's health cost follows the
    same table. Each multiplier reads its `fDiffMult*` GMST when the data
    holds it (`fDiffMultHPToPCL = 3` does) and UESP's value otherwise.
    Recorded: which damage paths count as damage, and the missing settings'
    names and values. `fDiffMultXPH` / `fDiffMultXPVH` are skill experience
    multipliers at Hard and Very Hard. Wire them through `src/skills.rs`,
    which already notes the Legendary-difficulty skill rules, once a source
    says what they multiply. Otherwise list them as an open question here.

Tests: settings round trip; unknown keys kept; an unsupported row left without
an `ID`; difficulty multipliers at each level for player-taken and NPC-taken
damage; falls at Novice and Legendary; Ravage unchanged.

### 6. Display options

6a. **HUD Opacity** (`fHUDOpacity`): the HUD movie's alpha. Use the HUD
    movie's own alpha entry point if `hudmenu.swf` has one (check its
    `HUDMovieBaseInstance` functions in step 0), else the compositor's layer
    alpha.
6b. **Crosshair** (`bCrosshairEnabled`): hide the crosshair clip, keeping the
    activation prompt, as the original does.
6c. **Dialogue / General Subtitles.** Dialogue subtitles are the lines of
    conversations the player is in (the dialogue menu's). General subtitles
    are everyone else's barks and scene lines (the HUD's `ShowSubtitle`).
    Gate each path. Both default to the launcher's values (off), but the
    developer launch keeps them on through the environment override, since
    offscreen tests read them.
6d. **Brightness** (`fGamma`): a gamma term in `src/render/post.rs` after tone
    mapping. Slider mapping: see Observations; until then linear over [0.5,
    1.5] around 1.0.
6e. **The fades** (Actor, Item, Object, Grass, Shadow, Light, Specularity, Tree
    LOD): distance fading by reference class in the renderer. Actors, items
    (loose objects) and objects (statics and the rest) fade out by
    `fLODFadeOutMult*` × the object's bound radius. Grass fades from
    `fGrassStartFadeDistance`, shadows reach `fShadowDistance`, lights fade
    from `fLightLODStartFade`, specular from `fSpecularLODStartFade`, and the
    Tree LOD switch uses `fTreesMidLODSwitchDist`. This is the largest Display
    item and needs renderer support that doesn't exist (per-instance fade
    alpha and distance culling). Split it into its own sub-steps when started,
    one class per commit. Each slider's mapping onto its setting: see
    Observations.

Tests: settings reach their consumer (HUD alpha set, crosshair hidden,
subtitle paths gated, gamma uniform); fade distances by class in the
renderer's culling tests.

### 7. Audio options

`RequestAudioOptions(list)`: Master gets an `ID`. Then push one slider per
`audio_preferences.menus` category, named by the category's `FULL`, or the
GMST the original uses for it (`sVoiceVolume` "Voice", `sEffectsVolume`,
`sFootstepVolume`, `sMusicVolume`: confirm the names in step 0). Changes go
through `Engine::set_audio_volume`. `SaveSettings` writes `audio.ini` with
the rest. The order is the category tree's or the original's, whichever step
0 establishes. This closes `plans/audio-unblocked.md`'s "slider UI" item.

Tests: the list has Master plus the menu categories; `OptionChange` changes the
category's gain and the file.

### 8. Controls: an input binding layer and remapping

8a. **`src/input_map.rs`.** User events (controlmap's Main Gameplay context)
    bound to keyboard keys and mouse buttons. The defaults come from
    `controlmap.txt` and the user's changes from `controlmap_custom.txt` in
    the config directory, written in the same column format. Map scan codes to
    winit `KeyCode`s with a table. `src/app.rs` asks "is `Activate` pressed"
    rather than "is E pressed", for every gameplay key it reads today:
    movement, attacks, Activate, Ready Weapon, Tween Menu, Toggle POV, Jump,
    Sprint, Shout, Sneak, Run, Toggle Always Run, Auto-Move, Favorites,
    Hotkeys, Quicksave, Quickload, Wait, Journal, Pause and the Quick menus.
    The developer keys (F3, the console's backquote, the debug letters) stay
    hard-coded and are reserved. `Buttons` in `src/swf_ui.rs` reads the same
    map, so button art follows a rebinding.
8b. **The protocol.** `RequestInputMappings(list)` pushes the remappable events
    (keyboard or mouse flag set) as `{text: event, buttonName: art,
    buttonID: code, sortIndex: controlmap order}`. `StartRemapMode(event,
    list)` puts the app in capture mode: the next key or mouse button goes
    to the map, not the game or the menu. A reserved key (Esc, the console,
    the developer keys) answers `FinishRemapMode(false)`. Otherwise update the
    row and answer `FinishRemapMode(true)`. A key already bound to another
    remappable event is a conflict. The original's handling of conflicts isn't
    public: unbind the other event (it shows "???"), see Observations.
    `ResetControlsToDefaults` and `SaveControls` → `SettingsSaved`.
8c. **Gamepad.** The gamepad column is read and kept but not remappable until
    gilrs input exists. `RequestInputMappings` lists only keyboard and mouse
    on PC.

Tests: controlmap parse (alternatives, `!0,` references, chords); custom file
round trip; capture of a key, a reserved key and a conflict; `app.rs` reacting
to a rebound Activate (an engine-level test of the event lookup).

### 9. Help

`PopulateHelpTopics(list)`: for each `MESG` in the `HMPC` default object's
form list, push `{text: FULL, index}`. Leave out messages with no title and
duplicates, and send `text` with a `$` only if the movie expects it (`SetEntry`
prints it as given; check in the driver). `RequestHelpText(index, title,
text)`: set the title to `FULL` and the text to `DESC`, through
`replace_text_tags` (aliases, globals) and the button-tag form the movie's
`ButtonArtHolder` reads. Check in step 0 which tag syntax `DESC` uses and that
`CreateButtonArt` turns it into art.

Tests: the list is the form list's titled messages; a topic's text has its
tags replaced.

### 10. Quit and the version text

- `SetVersionText(field)`: set the field to the engine's name and version
  (`CARGO_PKG_VERSION`).
- `RequestIsOnPC` answers true. Before answering, remove `$Main Menu` from
  `PCQuitPanel.List_mc`'s entries (no main menu yet), so the list holds
  Desktop only. `QuitToMainMenu` stays unanswered, logged.
- `QuitToDesktop`: already sets `quit_requested`.

Record the Main Menu removal in `known_ambiguities/journal.md` (replacing the
note there that says the System page isn't built).

### 11. Follow-on (separate plans, not in this one)

- **The loading screen** (`loadingmenu.swf`, `LSCR` records with their
  conditions and models) while a save loads or a load door is crossed. It also
  closes `plans/audio-unblocked.md`'s "loading-menu pause reporting" item.
- **The main menu** (`startmenu.swf`: Continue, New, Load, Credits, Quit). It
  enables Quit to Main Menu and belongs with ROADMAP item 11 (New game).

## Observations for the user (in Skyrim SE)

Menus stop unattended test scripts, so these are direct observations. Ask for
them in one batch, once the steps that use them are near:

1. **Slider mappings.** Set Look Sensitivity, Brightness, HUD Opacity and each
   fade slider to the far left, the middle and the far right in turn. After
   each, send the values `SkyrimPrefs.ini` holds for `fMouseHeadingSensitivity`,
   `fGamma`, `fHUDOpacity`, `fLODFadeOutMult*`, `fGrassStartFadeDistance`,
   `fShadowDistance`, `fLightLODStartFade`, `fSpecularLODStartFade` and
   `fTreesMidLODSwitchDist`. The file is written when the Settings panel is
   left.
2. **The save list.** A screenshot of Load with a few saves, quicksaves and
   autosaves (the text format and the order). How many autosaves the game
   keeps, and their names in the save folder.
3. **Save on Pause.** Whether opening the journal or the tween menu autosaves
   after the interval.
4. **Remapping conflicts.** Bind Jump to the key Sneak uses: is Sneak unbound,
   swapped, or refused?
5. **Help.** A screenshot of the Help list (order, and whether all of
   `HelpManualPC`'s messages appear).

## Verification

- Unit tests per step, as listed.
- Offscreen runs (`--menu-key`) that open the journal (Esc), click the System
  tab (`click:x,y`, as the Stats tab test does) and walk each entry, with
  screenshots of Save, Load with a thumbnail, Settings > Gameplay, Controls
  mid-remap, a Help topic and Quit.
- `cargo test`, including the GPU-ignored tests here with `--include-ignored`.

## Documents

- New: `research/ui/system-page.md` (step 0), `known_ambiguities/system-page.md`
  (the slider mappings, the save text format, autosave rotation, the remap
  conflict rule, Save on Pause's trigger, the difficulty damage paths, the
  defaults' sources).
- Updated: `known_ambiguities/journal.md` (the System page entries),
  `known_ambiguities/saves.md` (`META`, numbering, characters, thumbnails),
  `known_ambiguities/fast-travel.md` (its autosave), and
  `plans/audio-unblocked.md` (the slider UI done).
- ROADMAP: move each finished step to Done; reword Next item 6's "The
  journal's rest", item 9's "a save / load menu" and item 7's "Audio menu
  sliders' UI"; add what this plan leaves out (gamepad options, DDOF, Creations
  and the main menu) where it belongs.
