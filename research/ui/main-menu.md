# Main menu and credits contract

Research date: 2026-10-30. Only installed game assets and public references
were examined; no engine binaries or memory. Sources:

- `interface/startmenu.swf` and `interface/creditsmenu.swf`, read with
  `swfspike dis` and `fold.py` (classes `StartMenu`, `SaveLoadPanel`,
  `CreditsMenu`), and run in the spike harness (`swfspike run`). What follows
  describes their interface; it isn't a copy of their bytecode.
- `interface/translate_english.txt`, `interface/credits.txt`.
- Skyrim.esm: GMSTs (`vrm-tool gmst`), `DOBJ 00000031`'s `DNAM`, the cell
  `MainMenuCell` (`vrm-tool cell-refs`), `meshes/interface/logo/logo.nif`
  (`vrm-tool nif-dump`).
- CommonLibSSE-NG (MIT; license checked, no code copied): `RE/M/MainMenu.h`,
  `RE/B/BGSDefaultObjectManager.h`, `RE/T/TESLoadScreen.h`.
- xEdit's record definitions (`wbDefinitionsTES5.pas`, MPL-2.0; read for
  labels only): the default object labels `MMCL` and `MMSD`.
- UESP [Skyrim:Controls](https://en.uesp.net/w/index.php?oldid=3637273),
  "Menus": "Continue: Continue playing from your most recent saved game."

## The movie

The `StartMenu` clip is `_root.MenuHolder.Menu_mc`; its parent plays the
`fadeIn` / `fadeOut` labels. The game starts it with `InitExtensions()` and
`SetPlatform(0, false)` (PC), then the `sendMenuProperties` callback. Running
these three in the spike harness draws CONTINUE, NEW, LOAD, CREDITS and QUIT at
the bottom right, Continue selected, and the version text at the bottom
corner (`"v " + argument 2`).

The movie also loads `BethesdaNetLogin.swf` into `LoginHolder_mc` and hands it
a code object that the game would fill. The login only shows in
`LOGIN_STATE`, which `setupMainMenu` enters when argument 9 is false. With
argument 9 true it goes straight to `Main`; the login clip stays hidden and
its code-object calls go to an empty object (no-ops in ActionScript 2).

### `sendMenuProperties` arguments

| # | Effect in `setupMainMenu` |
| --- | --- |
| 0 | push `$QUIT` |
| 1 | push `$CONTINUE` (and select it); `$LOAD` enabled, else pushed disabled |
| 2 | version text, `"v " + value`; blank when undefined |
| 3 | start in the press-start state |
| 4 | show the 10th Anniversary upsell banner |
| 5 | push `$DOWNLOADABLE CONTENT` ("ADD-ONS") |
| 6 | push `$HELP` (after Quit) |
| 7 | push `$MOD MANAGER` (not on Switch) |
| 8 | push `$CREATIONS`; 12 is its icon, 13 whether it is enabled |
| 9 | logged in: false enters `LOGIN_STATE` first |
| 10 | read, nothing visible follows |
| 11, 14 | PS5 / Switch data transfer entries |

Entry order: Continue, New, Load, [PS5 / Switch transfer], [DLC], [Creations],
[Mod Manager], Credits, [Quit], [Help]. Sending it again rebuilds the list and
keeps the selected entry's index; from the save-list and confirm states it
returns to `Main`.

### States and calls

States are strings: `Main`, `MainConfirm`, `CharacterLoad`,
`CharacterSelection`, `SaveLoad`, `SaveLoadConfirm`, `DeleteSaveConfirm`, `DLC`,
`Login`, `PressStart` and others. `StartState(name)` and `currentState(name)`
report changes; `StartState` plays `<state>StartAnim` and the `EndAnim`
returns.

| Entry or key | The menu |
| --- | --- |
| Continue | calls `CONTINUE()`; nothing else until the game answers `ConfirmContinue(text)` (or `ConfirmNewGame`, both bound to `ShowConfirmScreen`) |
| accept in `MainConfirm` with Continue selected | `PlaySound("UIMenuOK")`, fade out, then `ContinueLastSavedGame()` |
| New | `NEW()`; the game's `ConfirmNewGame(text)` opens the confirm panel; accepting plays `UIStartNewGame`, fades out, then `StartNewGame()` |
| Load (enabled) | `PopulateCharacterList(list, batch)`, state `CharacterLoad`; the panel's `onFillCharacterListComplete(true)` moves to `CharacterSelection`; choosing a character calls `CharacterSelected(id, flags, false, list, batch)`; the saves arrive with `onSaveLoadBatchComplete` |
| Load (disabled) | `OnDisabledLoadPress()` |
| a save | `PrepSaveGameScreenshot(index, entry)` → `ScreenshotReady()` |
| accept a save | `IsOKtoLoad(index)` → the game's `ConfirmOKToLoad()`, the panel "$Load this game?", accept: fade out, then `LoadGame(index)` |
| X on a save | "$Delete this save?", accept: `DeleteSave(index)`; with none left, `DoDeleteSaveUISanityCheck()`, answered by `OnDeleteSaveUISanityCheck(hasRecentSave, canLoad)`, which removes Continue and disables Load |
| T in the save list | back to the character list |
| Credits | fade out, then `OpenCreditsMenu()` |
| Quit | its own panel "$Quit to desktop?  Any unsaved progress will be lost.", accept: `QuitToDesktop()` |
| Help | `HELP()` |
| any fade | `fadeOutStarted()`; the game may also fade with the `FadeOutMenu` callback and return with `FadeInMenu` |
| sounds | `PlaySound` with `UIMenuOK`, `UIMenuCancel`, `UIMenuFocus`, `UIStartNewGame` |

The confirm panel's text is whatever the game passes. GMSTs give
`sConfirmContinue` "Continue from your last saved game?" and
`sLoadFromMainMenu` "Are you sure you want to load this game?". No GMST or
translation string reads as a PC new-game confirmation (`$NX_ConfirmNewMods`
is the Switch's).

`OnDisabledLoadPress` and `HELP` aren't otherwise described by the assets.

## Credits (`creditsmenu.swf`)

The `CreditsMenu` clip is `_root.Menu_mc`. The root timeline's
`onCodeObjectInit()` calls the clip's, which:

1. clears `textField` and `textField2`;
2. calls `_root.CodeObj.requestCredits(textField, textField2)`;
3. sets the scroll rate (pixels per frame) to `_root.CodeObj.getScrollSpeed()`;
4. notes whether `textField2` has text.

Each frame both fields move up by the rate. The second follows once the first
has left the stage. When the last one's bottom passes y = −100, or on Tab, Esc
or a mouse click, the menu calls `_root.CodeObj.closeMenu()`. Up and Down move
the text 40 px, Page Up and Page Down 80 px, the wheel 20 px per notch, and
moving up pauses the scrolling for a second.

`credits.txt` (UTF-8, CRLF) is HTML text beginning with
`<img src='SkyrimLogo' ...>`; `credits_plru.txt` is its Polish / Russian
counterpart. The GMST `fCreditsScrollSpeed` is 2. Inference: the English
credits fill `textField` (HTML) and the scroll speed is that GMST. How the
game splits text into `textField2` is unknown; one field holds the whole file.

## What is behind the menu

- CommonLibSSE: the menu is "Main Menu", depth 0, flags pause game, disable
  pause menu, requires update, update uses cursor, application menu.
- `DOBJ`'s `MMCL` (xEdit: "Main Menu Cell") is `CELL 0006F3EB MainMenuCell`, an
  interior with dark lighting (`XCLL`) and one reference, a
  `COCMarkerHeading` at (−113, −193, 64), heading 268.3°.
- `meshes/interface/logo/logo.nif` is a `BSFadeNode` "Logo" with the
  `GenericBehaviors\Autoplay.hkx` graph and one shape (`actualLogoMirror01`,
  `textures\interface\objects\Logo02_d.dds`). `logo01ae.nif` and
  `logo01ae_animated.nif` are the Anniversary Edition's.
- `MMSD` is the map menu's looping sound (xEdit), not the main menu's.
- No default object or `MUSC` record names main-menu music; the cell has no
  music type (`XCMO`).

Not established: how the game places the logo and camera in the cell (the
marker is the only reference), what draws the smoke, and what music plays.

## Loading screens

`TESLoadScreen` has a "displays in main menu" flag (bit 10). That belongs to
`plans/loading-screen.md`.

## Open after this research

For an observation in SE (the main menu can't be scripted):

1. Do the PC SE main menu's entries include Help and/or Add-Ons? What does
   Help do?
2. Does Continue ask "Continue from your last saved game?" before loading,
   always, never, or only in some cases (after Quit to Main Menu)?
3. Does New ask anything before starting?
4. Which save does Continue load when the newest one is from a different
   plugin list?
5. What does quitting to the main menu show (a fade, a loading screen)?
6. What music plays at the main menu?
