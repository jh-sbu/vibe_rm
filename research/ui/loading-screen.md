# Loading screen contract

Research date: 2026-10-30. Only installed game assets and public references
were examined; no engine binaries or memory. Sources:

- `interface/loadingmenu.swf`, `fadermenu.swf` and `loadwaitspinner.swf`,
  read with `swfspike dis` and `fold.py`, plus a small tag walker for their
  timelines (frame counts, labels, the colour transforms of each frame). What
  follows describes their interface and timing; it isn't a copy of their
  bytecode.
- Skyrim.esm and the installed masters: GMSTs (`vrm-tool gmst`), the `LSCR`
  records (all 374 in the installed masters and Creation Club files, read
  field by field), `DOBJ 00000031`'s `DNAM`, the image space `LoadScreenIS`,
  the cell `LoadScreenTesting` (`vrm-tool cell-refs`), the `STAT`s
  `INTMenuFogParticles` and `LoadScreenLogo`, and the camera NIFs
  (`vrm-tool nif-dump`, `nif-tree`).
- CommonLibSSE-NG (MIT; license checked, no code copied): `RE/L/LoadingMenu.h`,
  `RE/M/MistMenu.h`, `RE/F/FaderMenu.h`, `RE/L/LoadWaitSpinner.h`,
  `RE/T/TESLoadScreen.h`, `RE/I/InterfaceStrings.h`,
  `RE/B/BGSDefaultObjectManager.h`. Header names are leads, not behaviour.
- xEdit's `wbDefinitionsTES5.pas` (MPL-2.0; read for labels only): `LSCR`,
  the default object label `LSIS`, condition function 629.
- UESP: [Skyrim:Loading Screens](https://en.uesp.net/w/index.php?oldid=3122269),
  [Skyrim talk:Loading Screens](https://en.uesp.net/w/index.php?oldid=2198446),
  [its Archive 1](https://en.uesp.net/w/index.php?oldid=2224154) and
  [Skyrim Mod:Mod File Format/LSCR](https://en.uesp.net/w/index.php?oldid=2384541).

## The movie (`loadingmenu.swf`)

CommonLibSSE names the menu "Loading Menu" and its root `Menu_mc`; the movie
places its `LoadingMenu` clip as `_root.Menu_mc`, and the clip's parent (the
root) plays `fadeIn` / `fadeOut`. The movie runs at 30 fps.

| The game calls | The movie |
| --- | --- |
| `InitExtensions()` | locks `LevelMeterRect` top right and `LoadingTextFader` bottom right of the safe rect; the text shrinks to fit, bottom aligned; the meter is a `Components.Meter` over `LevelProgressBar` |
| `FadeInMenu()` (once until `FadeOutMenu`) | calls `RequestPlayerInfo` (answered `SetLevelProgress(level, percent, showMeter)`), then `RequestLoadingText` (answered `SetLoadingText(text)`), and plays the root's `fadeIn` |
| `FadeOutMenu()` | plays the root's `fadeOut` |

`SetLevelProgress` sets `LevelNumberLabel` to the level, the meter to the
percentage (0–100, clamped; frame `Full` is 1, `Empty` 140) and the meter
rectangle's visibility to the third argument. `SetLoadingText(undefined)`
shows a blank.

**There is no load-progress bar**: the meter is the player's progress towards
the next level (the talk page calls it "the level-meter which shows in the
upper right corner").

Timelines (frames at 30 fps; alphas from the frames' colour transforms):

| Timeline | Frames | Alpha |
| --- | --- | --- |
| root `fadeIn` | 2–20, stops on 20 | 0 → 100 % over 19 frames (0.63 s) |
| root `fadeOut` | 21–30, stops on 30 | 100 → 0 % over 10 frames (0.33 s) |
| text fader `fadeIn` | 2–466 | 0 % to frame 22, 0 → 100 % over frames 23–41, held to 434, 100 → 0 % over 435–466 |

On frame 466 the text fader calls the menu's `refreshLoadingText`, which
requests new text and plays `fadeIn` again. So the movie, not the game, paces
the hints: a new text every 465 frames (15.5 s), each blank for 0.7 s, fading
in over 0.63 s, held 13.1 s and fading out over 1.07 s. UESP's "depending on
how long the game takes to load, multiple messages may be displayed" agrees.
The level meter is requested once per `FadeInMenu`.

## The other movies

- **`fadermenu.swf`** ("Fader Menu"; CommonLibSSE: depth 3, always open,
  custom rendering). The game registers nothing; the movie registers the
  callbacks `InitFade` and `UpdateFade`.
  `InitFade(fadeIn, black, duration, minSeconds)`: `fadeIn` true fades the
  rectangle from opaque to clear, false from clear to opaque; `black` false
  makes it white; a fade started while the rectangle is part way resumes from
  its alpha. `UpdateFade(elapsed)` advances it; nothing moves until the
  total elapsed time reaches `minSeconds`; when the fade's elapsed time
  reaches `duration` the movie calls `FadeDone`. Which game events start a
  fade isn't in the asset.
- **`loadwaitspinner.swf`** ("LoadWaitSpinner"): only `FadeInMenu` /
  `FadeOutMenu` over a spinner locked bottom right (fades 19 / 10 frames as
  the loading menu's). CommonLibSSE lists it as listening to system, gamer
  profile and save/load manager events: a lead that it belongs to saving or
  profile work, not to loading screens.

## The 3D scene ("Mist Menu")

CommonLibSSE's `MistMenu` (depth 8, under the loading menu's 9) holds a mist
model, the load screen's model (a `BSFadeNode`), a camera path node with its
controller sequence, six ambient colours (−X, +X, −Y, +Y, −Z, +Z), the image
space it replaced, a camera FOV, an angle about Z with a camera rotation,
flags to show the mist and the load screen, a "default logo only" shader
property, and whether the left and right mouse buttons are held. Inference:
the player turns the model with the mouse; the talk page mentions "spinning,
say, the dwarven head".

- **The image space**: the default object `LSIS` (xEdit "Imagespace: Load
  screen", CommonLibSSE `kImagespaceLoadscreen`) is `IMGS 000FF9DB
  LoadScreenIS`.
- **The mist**: no record names it. `STAT 001080E1 INTMenuFogParticles`
  (`interface\intmenufogparticles.nif`, a particle system emitting
  `FXSteamThinAnim.dds` over a 830 × 690 unit area below its origin) has no
  placed references and is the only interface fog model. Inference, not
  established: it is the mist. UESP's format page: "All loading screens have
  the foggy effect and black background".
- **The logo**: `STAT 00109515 LoadScreenLogo`
  (`LoadScreenArt\LoadScreenLogo.nif`) is the model of several ordinary
  `LSCR`s (UESP lists it on the levelling and quest tips). Every installed
  `LSCR` has a model, so when the "default logo only" property applies is
  unknown.
- **Lighting**: the developer cell `LoadScreenTesting` (UESP Test Cells; the
  talk page's guess at "the room") has a black ambient (`XCLL`), a
  `LoadScreenBronzeDragon` at (0, 0, −140) and three lights:
  `LoadScreenLightMain` at (160, −160, 128), radius 1500, colour
  (247, 245, 210); `LoadScreenLightBack` at (−160, 96, 160), 1024,
  (179, 208, 204); `LoadScreenLightFill` at (−128, −160, −96), 1024, white.
  Whether the game lights the screen with them is not established.
- **Camera paths** (`MOD2`): `meshes/cameras/lscamera*.nif`. Each is a
  `BSFadeNode` with one `NiControllerSequence` `CameraPath` (0–60 s, cycle 0,
  text keys `start` / `end`) moving `CameraPathNode`, which holds an
  `NiCamera`. The paths start 250–300 units in front of the origin on −Y
  (`PanZoomInSmall` at (0, −300, 16), `PanLFTZoomINBig` (10, −250, 0),
  `PanRGTZoomINBig` (−10, −250, −5), `PanUPZoomINBig` (0, −250, −15)) and close
  in on it; `PanDown01` starts at (0, −25, 35); `PanUPLargeWeap` moves only
  on Z (−60 → 0 → −60). Use: 208 `PanZoomInSmall`, 74 `PanLFTZoomINBig`, 53
  `PanDown01`, 30 `PanRGTZoomINBig`, 8 `PanUPZoomINBig`, one with none.

## `LSCR` records

Fields (xEdit, UESP, CommonLibSSE): `DESC` text (lstring), conditions, `NNAM`
model (a `STAT`), `SNAM` initial scale, `RNAM` initial rotation (int16 ×3,
degrees by their values), `ONAM` rotation offset constraints (int16 min,
max), `XNAM` initial translation offset (float ×3), `MOD2` camera path. Record
flag bit 10 is "Displays In Main Menu" (bit 12 CommonLibSSE calls
"Ignored").

In the installed data: 374 records (Skyrim.esm 298, Dragonborn 30, Dawnguard
19, Survival Mode 13, Fishing 5, Hearthfire 4, Update 3,
`ccBGSSSE025-AdvDSGS` 2). None has the main-menu flag; every one has a model. `ONAM`
is (−180, 180) on 353 of them, otherwise narrower ((−25, 25) ×9, (−5, 5) ×4,
...): read as the range the model may be turned through, in degrees
(inference). `SNAM` runs 1.0–2.5. 238 have no conditions.

Condition functions by use: GetInCurrentLoc 191, GetRandomPercent 133,
GetStage 16, GetGlobalValue 14, GetStageDone 11, GetQuestCompleted 8,
**629 = GetVMQuestVariable** (xEdit) 6 (the werewolf screens'
`C00::PlayerHasBeastBlood_var`, per the talk archive), HasKeyword 3, GetDead 1
on a reference, GetQuestRunning 1. The engine already evaluates all of these.

Selection, as public sources describe it:

- UESP's article: "If a condition is satisfied, that image *will* appear
  ... If no conditions are satisfied, one of the remaining load screens is
  selected at random"; most conditioned screens add a GetRandomPercent
  chance.
- Talk archive (2012): the conditions are checked and a passing screen is
  shown; "I *believe* that after the first loading screen message is shown,
  it then moves on to a completely random loading screen".
- Talk page (2013): a player who set GetRandomPercent below 0 on screens
  found they "didn't ... not appear": a lead that screens whose conditions
  fail are still drawn at random, or that the random draw ignores them. Not
  settled.
- GetRandomPercent gives 0–99 (talk archive), as the engine has it.

Nothing public says whether GetInCurrentLoc sees the departure or the
destination. The loading menu storing a "current location" is a lead
either way.

## Text

GMSTs: `sLoading` "Loading...", `sLoadingArea` "Loading area...",
`sQuickLoading` "Quickloading...", `sAutoLoading` "Autoloading...",
`sLoadingLOD`, `sMiscPlayerDeadMessage` "Reloading the most recent save game",
`fPlayerDeathReloadTime` 5. The movie has no field for these besides the hint
text; where they show is open.

## Left for Skyrim SE

Batched for the user (`plans/loading-screen.md`, step 0):

1. With `test-mod loadscreens` (`crates/tools/src/test_mods/loadscreens.rs`):
   which test texts appear through the Sleeping Giant Inn's door each way,
   on a save load in game and from the main menu (departure or destination;
   conditioned against unconditioned; the main-menu flag).
2. By eye: how long the fade to black before the screen and the fade back
   take; whether the screen stays up a minimum time when the load is quick;
   whether a quick load and the death reload show the full screen and where
   "Quickloading..." / "Reloading the most recent save game" appear; whether
   the mouse turns the model; whether the mist and level meter show on every
   load.
