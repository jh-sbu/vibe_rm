# Follow-on: loading screens

**Status: implemented; the SE observation batch is pending.** Drafted
2026-10-28 as a sketch; audited and expanded 2026-10-30, and steps 0–6 done
the same day (`research/ui/loading-screen.md`,
`known_ambiguities/loading-screen.md`, `src/loading.rs`,
`src/load_screens.rs`, `src/loading_view.rs`,
`src/swf_ui/loading_menu.rs`). What remains is under "After the
observations" at the end. Separate from the completed journal System page (its step 11).
ROADMAP Next item 11 (New game and the opening) lists it; it also closes
`plans/audio-unblocked.md`'s "loading-menu pause reporting" item and the
"loading-screen presentation" part of `known_ambiguities/fast-travel.md`.

This plan shows the game's loading screen while the player changes cells by a
load door, fast travel, a script or a native save load. The loading screen has
two layers: the `loadingmenu.swf` overlay (the hint text and the level meter)
and a 3D scene behind it (an `LSCR` model in fog, moved along a camera path),
which is not part of the movie (see "What the first look found").

Rules as for all work here (AGENTS.md): public sources and game assets only, no
engine binaries; describe the movies' contracts in prose and tables, and don't
copy their disassembly into the repository. Don't infer the original selection
weights, timing or progress behaviour from engine binaries. Cite UESP pages by
their permanent revision links, and keep source claims apart from inference
and from behaviour observed in SE. Write the research up as
`research/ui/loading-screen.md` in step 0. Choices made without a source go in
`known_ambiguities/loading-screen.md`; open questions stay here or in ROADMAP.
Nothing goes in `deviations/` unless the user declares it. Run `cargo fmt`
before each commit. Each numbered sub-step is one commit.

## What the first look found (2026-10-30)

Sources: `interface/loadingmenu.swf`, `fadermenu.swf` and `loadwaitspinner.swf`
(`swfspike dis`), the GMSTs (`vrm-tool gmst`), the load order's `LSCR` records
(`vrm-tool esp-list` / `ctda-stats`), the Meshes BSA listing, UESP
[Skyrim:Loading Screens](https://en.uesp.net/w/index.php?oldid=3122269) and
[Skyrim Mod:Mod File Format/LSCR](https://en.uesp.net/w/index.php?oldid=2384541),
xEdit's `wbDefinitionsTES5.pas` (MPL-2.0) and CommonLibSSE-NG headers (MIT):
`RE/L/LoadingMenu.h`, `RE/M/MistMenu.h`, `RE/T/TESLoadScreen.h`,
`RE/I/InterfaceStrings.h` and `RE/B/BGSDefaultObjectManager.h`. Step 0 checks
this section and finishes it.

### The movie (`loadingmenu.swf`, menu name "Loading Menu")

It has **no load-progress bar**. Its meter is the player's progress towards
the next level.

| When | Movie calls (GameDelegate) | Game answers with |
| --- | --- | --- |
| the game calls `FadeInMenu()` | `RequestPlayerInfo()`, then requests the text | `SetLevelProgress(level, progress, showMeter)`: the level label, the meter's percentage, and whether the meter shows |
| each time the text fader's `fadeIn` label plays | `RequestLoadingText()` | `SetLoadingText(text)`; undefined shows a blank |
| the game calls `FadeOutMenu()` | nothing | the movie plays its parent's `fadeOut` |

Because the text fader requests new text on its own timeline, the movie paces
how often the hint changes. UESP's "multiple messages may be displayed"
matches this. Step 0 measures the timeline's frame count and frame rate, so
that a dwell time can come from the asset instead of a guess. The movie
locks the level meter to the top right and the text to the bottom right of
the safe rect.

### Other movies and menus

- **The Mist Menu** (CommonLibSSE `MistMenu`, menu depth 8, below the Loading
  Menu's 9) holds the 3D side: a mist model, the `LSCR` model, a camera path
  with its controller sequence, six ambient colours, the image space it
  replaced, a camera FOV and a "default logo only" shader property. Which
  meshes it uses for the mist and the logo is open (step 0).
- `BGSDefaultObjectManager` names `kImagespaceLoadscreen`, the image space for
  that scene.
- `fadermenu.swf` ("Fader Menu"): `InitFade` with start/end alpha, a colour, a
  duration and a minimum time; it reports `FadeDone`. This is probably the
  fade to black around a load. Quit to Main Menu currently uses the sleep
  image-space fade instead (`known_ambiguities/main-menu.md`).
- `loadwaitspinner.swf` ("LoadWaitSpinner"): a spinner. When SE shows it is
  open.
- CommonLibSSE lists the Loading Menu as pausing the game, always open and
  allowing saving, with a current `BGSLocation` and a list of
  `TESLoadScreen`s. The list suggests that the game picks its candidates
  when the menu opens. `InterfaceStrings` also names "LoadingMenuData" and
  "CancelLoading". Header names are leads, not behaviour.

### `LSCR` records

- Fields (xEdit, UESP): `DESC` text, conditions, `NNAM` model (a `STAT`),
  `SNAM` initial scale, `RNAM` initial rotation (3 × int16), `ONAM` rotation
  offset constraints (min, max), `XNAM` translation, `MOD2` camera path. Record
  flag bit 10 is "Displays In Main Menu".
- The load order has 371. Condition functions by use (`ctda-stats`):
  GetInCurrentLoc 191, GetRandomPercent 133, GetStage 16, GetGlobalValue 14,
  GetStageDone 11, GetQuestCompleted 8, GetVMQuestVariable (629) 6, HasKeyword 3
  and GetDead 1 on a reference, GetQuestRunning 1.
- Camera paths: `meshes/cameras/lscamera*.nif` (pan, zoom) and
  `testloadscreencamerapan.nif`. Models include `meshes/dlc01/loadscreenart/`.
- UESP (oldid 3122269): "If a condition is satisfied, that image *will*
  appear ... If no conditions are satisfied, one of the remaining load
  screens is selected at random", and most conditioned screens also carry a
  GetRandomPercent chance. This is a wiki claim, not a verified rule.

### Text (GMSTs)

`sLoading` "Loading...", `sLoadingArea` "Loading area...", `sQuickLoading`
"Quickloading...", `sAutoLoading` "Autoloading...", `sLoadingLOD` and
`sMiscPlayerDeadMessage` "Reloading the most recent save game", with
`fPlayerDeathReloadTime` 5. Where each one shows (the HUD, the loading text,
a notification) is open.

### What already exists

- Cell changes are synchronous and run in the frame that asks for them:
  `Engine::enter_interior` / `enter_exterior` (`src/engine.rs`) and
  `teleport_through`. The callers are load doors (`activate`), fast travel
  (`src/fast_travel.rs`), arrest (`src/arrest.rs`), Papyrus `MoveTo` into
  another interior (`pending_moveto`), `center_on_cell` (console) and the
  launch (`src/app.rs`).
- Save loads: `pending_save_load` (journal Load, main-menu Continue/Load,
  Quickload) runs in `apply_pending_save_load` before the frame advances.
  `load_game_from` reads and parses the whole file before anything changes,
  but `Snapshot::apply` then changes the engine and can still fail in its
  final `enter_*` call. A failure in that call leaves a half-applied game.
  Failures show a debug message box.
- Exterior streaming loads two pending cells per frame. SE shows no loading
  screen while walking, so this stays as it is.
- The audio director already has a `loading` input that pauses `PDLC`
  (`src/audio/director.rs`). It is always false.
- `GetRandomPercent` reads the gameplay RNG (`peek_rand`), and save loads
  restore saved RNG streams. Choosing a loading screen must not draw from those
  streams.
- `level_progress` (`src/skills.rs`) can answer
  `SetLevelProgress`.
- Quit to Main Menu shows no loading screen in SE (observed,
  `research/ui/main-menu.md`). Keep it that way.

## Steps

0. **Research** (`research/ui/loading-screen.md`, no code). *Done
   2026-10-30; the observation batch is below under "SE observation
   request".* Finish the
   section above, then settle what the assets and public sources can:
   - the text fader's and the menu's fade timelines (frames, fps), and which
     frame label `FadeInMenu` and `FadeOutMenu` play;
   - `fadermenu.swf`'s `InitFade` arguments and when it reports `FadeDone`;
   - the mist and logo meshes, the camera-path NIFs' controller sequences
     (`nif-tree`), and what `ONAM` constrains (player-controlled rotation?);
   - condition function 629; and `kImagespaceLoadscreen`'s record in
     Skyrim.esm;
   - what UESP's talk pages and other public sources say about selection
     (the departure or the destination location for GetInCurrentLoc, and
     whether unconditioned screens stay eligible when a conditioned one
     passes), the main-menu flag, and the death reload.

   List what only SE can answer and batch it into one observation request for
   the user. The batch should include what can be tested with a test plugin
   (`vrm-tool test-mod`): `LSCR` records whose text names the location or
   condition they test. Examples: one conditioned on the departure location
   and one on the destination; one with no conditions next to an
   always-passing conditioned one. The user reads which text appears through
   a door and on a save load. Other questions are direct observations: fade
   lengths, whether a quick load shows a screen, the minimum time on screen,
   and the death reload.

1. **A loading state in the app.**
   - 1a. Defer every cell change and save load behind one request
     (`LoadRequest`: door, fast travel, script move, arrest, save load). The
     current callers queue it instead of calling `enter_*` directly. Refusals
     and destination checks still run before queueing, so a refused travel
     never shows a screen. No visible change yet; existing tests keep
     passing.
   - 1b. The state machine: fade out → present the loading frame → run the
     synchronous load → hold for the minimum time (if any) → fade in. The
     world is paused throughout (Loading Menu pauses the game). Async loading
     remains ROADMAP item 10. Until then, the movie and the scene freeze
     during the blocking load itself; record that in
     `known_ambiguities/loading-screen.md` only if SE evidence shows it
     differs.
   - 1c. Save-load failure: make `Snapshot::apply` fail before it changes
     anything wherever possible (resolve the destination cell first). If it
     still fails after changes, go to the main menu (or the last good state)
     and don't leave a half-applied game. Show the error in the game's own
     message box (`sSaveGameCorruptMenuMessage` where it fits) instead of a
     debug box.

2. **The movie.** Open `loadingmenu.swf` over the frame during the state.
   Answer `RequestPlayerInfo` from `level_progress`, and `RequestLoadingText`
   from the chosen screens. Call `FadeInMenu` / `FadeOutMenu` at the state's
   edges. Use `fadermenu.swf` for the fades if step 0 confirms its role;
   otherwise keep the image-space fade and record that choice.

3. **Selection.** Build the candidate list when the state starts. Evaluate
   each `LSCR`'s conditions against the subject and location that step 0
   settles. Draw random numbers from a separate stream that never touches the
   saved gameplay streams. Apply the main-menu flag for loads from the main
   menu. Record every rule the research left open in
   `known_ambiguities/loading-screen.md`, with its sources. Unit-test it with
   synthetic records: always-true, random, location, quest stage, none
   eligible.

4. **The 3D scene.** Render the `NNAM` model with `SNAM` / `RNAM` / `XNAM` in
   the mist, lit by the loading image space, along the `MOD2` camera path or
   a default one. Draw the logo when no screen applies, if step 0 shows that
   SE does this. Add input rotation within `ONAM` if step 0 shows that SE
   allows it.

5. **Audio.** Set the director's `loading` input for the whole state,
   including failure and cancellation. Check how music behaves across the
   load against SE's observation.

6. **Verification.** Run the synthetic selection tests, the engine fixture
   tests (`--include-ignored`), and offscreen captures (screenshot and burst
   hooks) for: a native save load from the journal and from the main menu, a
   quick load, an interior door, an exterior door, fast travel, a script
   `MoveTo` and a failed load. Keep captures and test plugins under ignored
   `tests/assets/`, and results under `tests/results/`. Update ROADMAP item 11,
   `plans/audio-unblocked.md`, `known_ambiguities/fast-travel.md` and this
   plan's status.

## SE observation request (step 0's batch)

Build the plugin with `cargo build -p vrm-tools` and
`vrm-tool test-mod loadscreens "<Skyrim SE>/Data" tests/assets/VrmLoadScreens.esp`,
enable it in SE, and note the **first** hint text of each loading screen:

1. Through the Sleeping Giant Inn's front door, into the inn and out again,
   five times each way.
2. Through some door far from Riverwood, five times.
3. Load a save made outside Riverwood once in game and once from the main
   menu.

The module doc (`crates/tools/src/test_mods/loadscreens.rs`) has what each
reading predicts. By eye, during the same run:

4. How long the fade to black before the screen and the fade back take
   (roughly: under half a second, about a second, longer).
5. Whether a very quick load (a small interior) still shows the screen for a
   while, and roughly how long.
6. A quick load (F9) and the death reload: is it the full loading screen?
   Where do "Quickloading..." and "Reloading the most recent save game"
   appear, if at all?
7. Whether holding a mouse button and moving the mouse turns the model.
8. Whether the music keeps playing across the loading screen.

## Open questions

- Is GetInCurrentLoc evaluated at the departure or the destination? (The
  Loading Menu stores a current location.)
- Selection when several screens pass, and whether unconditioned screens are
  eligible then.
- What the "Displays In Main Menu" flag restricts.
- Fade lengths, any minimum time on screen, and the text's dwell time (the
  movie's timeline may answer this one).
- Whether quick loads, autoloads and the death reload show the full screen,
  and where the `sLoading*` / `sQuickLoading` / `sAutoLoading` text goes.
- Whether the player can rotate the model, and what `ONAM` limits.
- The mist and logo meshes, and what `loadwaitspinner.swf` is for.
- Autosave on travel: its timing relative to the screen
  (`known_ambiguities/fast-travel.md`).

Out of scope here: asynchronous streaming (ROADMAP item 10), the new-game
loading screen (item 11's new game), and SKSE or mod behaviour.

## After the observations (2026-10-30)

Verified here: the selection, state, failure and director tests
(`cargo test --bin vibe_rm -- --include-ignored`), and offscreen captures (kept
under ignored `tests/captures/loading/`) of an interior door, an exterior door,
a script `MoveTo` into an interior, a save load in game, a save load from the
main menu and a failed load. Fast travel and the quick-load key are covered by
the tests only (fast travel has no console path; the quick-load key queues the
same request as the journal). Still open:

- Settle the selection rules, the fades, the minimum time, quick-load and
  death-reload presentation, model turning and music with the observation
  batch above, then update `known_ambiguities/loading-screen.md` and the code.
- The half-applied save path (`HalfApplied`) has no test: after the early
  destination check nothing known reaches it.
- The mist's colour against SE's, and the camera's field of view.
- `loadwaitspinner.swf` and `fadermenu.swf` stay unused (no source ties them
  to loads); the "default logo only" case isn't drawn.
- The launch (`--cell`, `--load`) and the console's `coc` still change cells
  without the screen.
- A door or travel target that fails to load after `enter_*` has unloaded
  the old cells leaves the player in no cell, as before; only save loads
  check their destination first.

