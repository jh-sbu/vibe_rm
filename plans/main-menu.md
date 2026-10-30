# Follow-on: the main menu

**Status: planned** (drafted 2026-10-28, expanded 2026-10-30). Separate from the
journal System page; ROADMAP Next item 11 (New game and the opening).

This plan builds the game's own main menu (`startmenu.swf`, menu name "Main
Menu") with the PC entries Continue, New, Load, Credits and Quit, the Credits
menu it opens (`creditsmenu.swf`), and Quit to Main Menu from the journal. It
also connects New to a new-game path, which is researched separately.

The movie's entries and states that this plan leaves out, and why:

- **Bethesda.net, Creations and mods**: the login screen
  (`BethesdaNetLogin.swf`, which the movie loads into itself), `$CREATIONS`,
  `$MOD MANAGER`, the message of the day, the 10th Anniversary upsell and
  Download All. These aren't vanilla content (as on the System page). The
  game's answer to `sendMenuProperties` leaves these entries out, and the
  login screen is skipped (see the contract below).
- **Console-only states**: press start, the change-user and gamer-tag widgets,
  the Joy-Con instructions, the PS5 and Switch data transfers, the Xbox
  Marketplace, `ORBISDeleteSave`, the Sony save-data events and the
  `iPlatform`-specific branches. PC is platform 0.
- **Downloadable Content (`$DOWNLOADABLE CONTENT`, shown as "ADD-ONS") and
  Help (`$HELP`)**: both are optional entries. UESP's
  [Skyrim:Controls](https://en.uesp.net/w/index.php?oldid=3637273) lists
  Downloadable Content among the main menu's entries, but doesn't say which
  edition. Whether vanilla SE on PC shows either entry, and what Help does
  there, is open (step 0). Until that is settled both stay out.

Rules as for all work here (AGENTS.md): public sources and game assets only, no
engine binaries; describe the movie's contract in prose and tables, and don't
copy its disassembly into the repository. The main menu runs before any game
is loaded, so a test plugin can't observe it. Questions only the running game
can answer go to the user as a direct observation in SE. Write the research up
as `research/ui/main-menu.md` in step 0. Choices made without a source go in
`known_ambiguities/main-menu.md`, open questions here or in ROADMAP. Run
`cargo fmt` before each commit. Each numbered sub-step is one commit.

## What the first look found (2026-10-30)

Sources: `startmenu.swf` and `creditsmenu.swf` (`swfspike dis`, folded with
`fold.py`; classes `StartMenu`, `SaveLoadPanel`, `CreditsMenu`),
`interface/translate_english.txt`, the GMSTs (`vrm-tool gmst`), Skyrim.esm
(`vrm-tool esp-list` / `esp-dump`) and CommonLibSSE-NG headers (MIT):
`RE/M/MainMenu.h` and `RE/B/BGSDefaultObjectManager.h`. Step 0 checks
this section and finishes it.

### The calls the movie makes and the answers it expects

| When | Menu calls (GameDelegate) | Game answers with |
| --- | --- | --- |
| set-up | expects the `sendMenuProperties(...)` callback | builds the entry list from 15 positional arguments (below) |
| state changes | `StartState(name)`, `currentState(name)` | nothing (notifications) |
| Continue | `CONTINUE()` | `ConfirmContinue(text)` → confirm panel; on accept the menu fades out and calls `ContinueLastSavedGame()` |
| New | `NEW()` | `ConfirmNewGame(text)` → confirm panel; on accept, the `UIStartNewGame` sound, a fade-out, then `StartNewGame()` |
| Load (enabled) | `PopulateCharacterList(list, batch)` → `onFillCharacterListComplete(true)`, then `CharacterSelected(id, flags, false, list, batch)` → `onSaveLoadBatchComplete` | as on the System page; there's no `LOAD` call, so the character list always comes first |
| Load (disabled) | `OnDisabledLoadPress()` | not known yet (a message?) |
| highlight a save | `PrepSaveGameScreenshot(index, entry)` | `ScreenshotReady()` |
| confirm load | `IsOKtoLoad(index)` → `ConfirmOKToLoad()`, a "$Load this game?" panel, then a fade-out and `LoadGame(index)` | loads |
| delete (X) | a "$Delete this save?" panel, then `DeleteSave(index)` (no −1 offset: the main menu never saves). If no saves are left, `DoDeleteSaveUISanityCheck()` | `OnDeleteSaveUISanityCheck(hasRecentSave, canLoad)`: removes Continue and/or disables Load |
| leaving the list early | `ForceStopSaveListLoading()` | stops filling |
| Credits | the menu's own fade-out, then `OpenCreditsMenu()` | opens `creditsmenu.swf`; the game sends `FadeInMenu()` afterwards (inferred) |
| Quit | the movie's own "$Quit to desktop?  Any unsaved progress will be lost." panel, then `QuitToDesktop()` | quits |
| any fade-out | `fadeOutStarted()`, then the stored call when the `fadeOut` animation ends | the game can also start a fade itself with `FadeOutMenu()` (and fade back in with `FadeInMenu()`) |
| sounds | `PlaySound(name)`: `UIMenuOK`, `UIMenuCancel`, `UIMenuFocus`, `UIStartNewGame` | plays them |

`sendMenuProperties` arguments by position:

| # | Meaning | PC vanilla value |
| --- | --- | --- |
| 0 | adds `$QUIT` | true |
| 1 | there are saves: adds `$CONTINUE` (selected by default) and enables `$LOAD` | from the save list |
| 2 | version text (`"v " + value`; blank when undefined) | the build's version |
| 3 | starts in the press-start state | false |
| 4 | 10th Anniversary upsell banner | false |
| 5 | adds `$DOWNLOADABLE CONTENT` | open (step 0) |
| 6 | adds `$HELP` | open (step 0) |
| 7 | adds `$MOD MANAGER` | false |
| 8 | adds `$CREATIONS` | false |
| 9 | already logged in: true skips the Bethesda.net login state | true |
| 10 | read, no visible effect | false |
| 11, 14 | PS5 / Switch data transfer | false |
| 12, 13 | Creations' icon and access | false |

The order is fixed: Continue, New, Load, then the optional entries, Credits,
Quit and Help. The movie sends `sendMenuProperties` again to rebuild the list
(for example after deleting the last save), and keeps the entry that was
selected.

Text the game passes: `sConfirmContinue` is "Continue from your last saved
game?" and `sLoadFromMainMenu` is "Are you sure you want to load this game?"
(GMSTs). No GMST names a new-game confirmation, so the `ConfirmNewGame` text
is open. The movie's own panels use the translation strings above. The
translations file also has "$Quit to main menu?  Any unsaved progress will be
lost." for the journal.

Continue: whether the game always answers `CONTINUE` with `ConfirmContinue`,
or only sometimes (for example only when a game is running), needs an SE
observation. Its alternative is to fade out and load without a panel, which
the movie supports through `FadeOutMenu`. UESP says Continue resumes "your
most recent saved game". Whether that means the newest save by time or by
save number, across all characters, and what happens if that save is
obsolete or corrupt, is open.

### Credits (`creditsmenu.swf`)

The Credits menu talks to the game through a code object (`CodeObj`), not
GameDelegate. When the object is installed (`onCodeObjectInit`), the menu calls
`CodeObj.requestCredits(field1, field2)` and `CodeObj.getScrollSpeed()`.
It calls `CodeObj.closeMenu()` when the text has scrolled off, or on Tab, Esc
or a mouse click. Up, Down, Page Up, Page Down and the mouse wheel scroll it.
`crates/ui-swf` has no code-object support yet. Lead: the GMST
`fCreditsScrollSpeed` = 2 is probably the scroll speed. Which of
`interface/credits.txt` and `credits_plru.txt` the game uses, and how it splits
the text between the two fields, is open.

### Behind the menu

- CommonLibSSE's `MainMenu.h` gives the menu's flags: depth 0, pauses the
  game, disables the pause menu, requires update, update uses cursor, and
  "application menu".
- `BGSDefaultObjectManager` names a default object `kMainMenuCell`. Skyrim.esm
  has an interior `CELL 0006F3EB MainMenuCell`. The Meshes BSAs hold
  `meshes/interface/logo/logo.nif` (and `logo01ae*.nif`). Probably the 3D
  smoke-and-logo background is that cell, with or showing the logo model.
  Step 0 confirms this from the default-object record and the cell's
  references.
- Music: no default object names main-menu music, and no `MUSC` record is
  named for the main menu. `MUSSpecialIntro` (`0008DC7C`) is a lead only.
  Open.
- Loading-screen records have a "displays in main menu" flag
  (`TESLoadScreen::kDisplaysInMainMenu` in CommonLibSSE). This belongs to the
  loading-screen plan (`plans/loading-screen.md`), but a load from this menu
  goes through that plan's loading state.

### What already exists

- `src/swf_ui/journal_system.rs` and `journal_menu.rs` already answer the save
  list protocol: `PopulateCharacterList`, `CharacterSelected`, batching,
  `PrepSaveGameScreenshot`, `IsOKtoLoad` (with the corrupt and obsolete
  messages), `DeleteSave` and `LoadGame` through `engine.pending_save_load`.
  The main menu should share this code, not copy it.
- `Engine::load_game` / `apply_pending_save_load` (`src/save/mod.rs`)
  already unloads everything (`Engine::unload_all`) and replaces the engine's
  state and the VM's. Quit to Main Menu's teardown is close to that path
  without a destination save.
- `--load <name>` and the developer cell/world launch (`src/main.rs`).
- The journal's `QuitToMainMenu` only logs a warning
  (`src/swf_ui/journal_menu.rs`). The System page's step 10 left the main-menu
  entry out of its quit list.

## Steps

0. **Research.** Check and finish the contract above, and write it up as
   `research/ui/main-menu.md`. Settle from assets or public sources, or list
   for an SE observation:
   - which optional entries vanilla PC SE shows (arguments 5 and 6), and what
     `HELP` and `OnDisabledLoadPress` do;
   - whether Continue confirms, which save it picks, and what it does with an
     obsolete or corrupt newest save;
   - the `ConfirmNewGame` text;
   - what draws behind the menu (MainMenuCell, the logo model, the camera)
     and which music plays;
   - the credits' text source and scroll speed;
   - whether the main menu's version text shows on PC;
   - what the game does at quit-to-main-menu (a fade, a loading screen?).

   Batch the questions only SE can answer into one manual observation list
   for the user.
1. **The movie and its state.**
   - 1a. An application state for the main menu (wherever the launch modes
     live). The developer launch stays the default (the user's decision,
     2026-10-30); a `--main-menu` option launches to the main menu instead.
     `--load` and the cell/world arguments keep working as now.
   - 1b. `src/swf_ui/main_menu.rs`: open `startmenu.swf` with platform 0, send
     `sendMenuProperties` with the PC values above, answer the state
     notifications and sounds, and make sure the login movie's `loadClip`
     fails or loads quietly with no code object behind it.
   - 1c. The background (the cell or model step 0 names) and music, or a
     recorded fallback if step 0 can't settle them.
2. **Load and Continue.**
   - 2a. Move the save list handlers into code shared by the journal and the
     main menu, without changing the journal's behaviour (its tests must pass
     as they are).
   - 2b. Load: the character list, saves, screenshots, delete and
     `DoDeleteSaveUISanityCheck` / `OnDeleteSaveUISanityCheck`, and `LoadGame`
     after the movie's fade. A failed load comes back to the main menu with a
     message instead of exiting.
   - 2c. Continue: `CONTINUE`, the confirmation step 0 settles, and
     `ContinueLastSavedGame`. Whatever is chosen for unreadable newest saves
     goes in `known_ambiguities/main-menu.md`.
3. **Credits.** Code-object support in `crates/ui-swf` (a host object passed
   to `onCodeObjectInit`, with its methods answered by the game), then
   `creditsmenu.swf` with `requestCredits`, `getScrollSpeed` and
   `closeMenu`, and `FadeInMenu` on the main menu when it closes.
4. **Quit.**
   - 4a. `QuitToDesktop` from the main menu.
   - 4b. Quit to Main Menu from the journal: tear down the whole session
     (pending saves, the VM, actors and cells, audio, open menus and their
     callbacks, the away actors and story manager queues) and keep user
     settings and control bindings. Build it on the load path's teardown.
     Then add the main-menu entry back into the System page's quit list
     (`PCQuitList`, filled in the `RequestIsOnPC` answer in
     `src/swf_ui/journal_system.rs`) and answer `QuitToMainMenu`.
5. **New.** `NEW` → `ConfirmNewGame` → `StartNewGame`. It connects to the
   new-game initialisation and placement researched under ROADMAP item 11
   (how MQ101 starts and where the player is placed are open there). The
   character's identity is created once and kept through saves. Don't treat the
   developer launch as evidence for MQ101 or Helgen. Until item 11's research
   lands, New answers with a message that a new game isn't available and
   starts nothing. Record that fallback in `known_ambiguities/main-menu.md`.
6. **Verification.**
   - Synthetic state tests: Continue with no saves, an obsolete newest save and
     a corrupt newest save; failed loads; deleting the last save (Continue
     removed, Load disabled); `sendMenuProperties` rebuilt with the selection
     kept.
   - Repeated game → main menu → game cycles, checking for leaked state (VM
     stacks, actors, audio voices, registrations).
   - Credits scrolling to the end and closing early.
   - Quit from both menus, and a new game's own identity.
   - Offscreen captures (`--menu-key`) of the main menu, the load list, a
     confirm panel and the credits, kept under ignored `tests/assets/` or
     `/tmp`.

## Open questions

- Everything step 0 lists, until it's settled.
