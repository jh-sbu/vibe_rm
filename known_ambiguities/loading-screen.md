# Loading screen choices

Research: `research/ui/loading-screen.md` (2026-10-30). The observations that
would settle these are batched in `plans/loading-screen.md` ("SE observation
request"). Each choice stands until a source or an SE observation settles it.

## Fades and time on screen

The loading screen's own fades are the movie's (`loadingmenu.swf`: in over 19
frames, out over 10, at 30 fps). The rest is chosen:

- The view fades to black over 0.5 s before the screen (the sleep fade's
  `SLEEP_FADE_SECONDS`, also used by Quit to Main Menu) and back in over
  0.5 s after it. From the main menu, whose movie fades itself out first,
  the screen comes up at once. SE's durations aren't measured; the UESP talk
  page only says the screen "starts to darken before the game loads".
- The load runs once the screen has fully faded in, and the screen stays up
  at least 2 s from its fade in. Nothing public gives SE's minimum.
- The game is paused for the whole state, fades included (CommonLibSSE lists
  the Loading Menu as pausing the game). It runs again as the world fades
  back in.
- The load blocks the frame (asynchronous loading is ROADMAP item 10), so the
  screen and the movie stand still while it runs.

## The movie's answers

- `RequestPlayerInfo` is answered with the player's level and progress at the
  time the screen comes up, which for a save load is the game being left.
  From the main menu, with no player, the meter and level are hidden
  (`showMeter` false). The save's own level is in its header, but nothing
  says SE shows it there, and the progress isn't.
- The fades around the screen use the engine's image-space fade, not
  `fadermenu.swf`: the Fader Menu's `InitFade` fits a fade to black, but
  nothing in the assets or public sources ties it to loading.

## Which screen

UESP's article says a screen whose conditions pass "*will* appear" and that
otherwise one of the rest is picked at random; its talk pages leave the
details open (`research/ui/loading-screen.md`). Chosen, until
`test-mod loadscreens` is run in SE:

- The screens are chosen as the load starts, their conditions run on the
  player where they stand, so GetInCurrentLoc sees the location being left
  (a save load from the main menu has no location: no conditions run).
- In game: one of the screens whose conditions pass, uniformly, if any;
  otherwise one of the screens with no conditions. Screens whose
  conditions fail are never shown.
- From the main menu: one of the screens with the "Displays In Main Menu"
  flag, if any (none is installed), else one with no conditions.
- The first text is that screen's; the model is that screen's for the whole
  load. Each later text the movie asks for (every 15.5 s) is drawn from the
  passing and unconditioned screens together, not the one just shown. The
  talk archive's "after the first loading screen message is shown, it then
  moves on to a completely random loading screen" is one editor's belief.
- The draws come from a stream of the engine's own, seeded from the clock
  and never saved; GetRandomPercent in a screen's conditions draws from it,
  so choosing a screen never moves the streams a save restores.

## The scene

CommonLibSSE's Mist Menu names the parts (a mist model, the screen's model,
a camera path with its sequence, an image space, mouse buttons held); the
data gives the rest (`research/ui/loading-screen.md`). Chosen:

- **The mist** is `INTMenuFogParticles` (the only interface fog model, used by
  no record), at the origin, playing its `AutoPlay` sequence's emission,
  warmed up 3 s before the screen shows. Its colour comes out reddish here;
  SE's mist hasn't been compared.
- **The model** is the screen's `NNAM` `STAT` model at the origin: scaled by
  `SNAM`, turned by `RNAM` (degrees, applied as a reference's rotation) and
  moved by `XNAM`.
- **The camera** rides `CameraPathNode` along the `MOD2` model's `CameraPath`
  sequence, looping over its 60 s, facing +Y (toward the model; the node is
  unrotated and its `NiCamera` isn't read). A screen without a path uses
  `LSCameraPanZoomInSmall`. The field of view (50°) isn't in the data.
- **Light**: the three lights of the developer cell `LoadScreenTesting`
  (`LoadScreenLightMain`, `Back`, `Fill`, at their positions, radii and
  colours), a dim ambient (0.05), and `LoadScreenIS`'s saturation,
  brightness, contrast and tint. The test cell is a lead, not a source for
  what the game uses.
- **The fade**: the scene fades in and out with the movie (0.63 s / 0.33 s).
- **Turning**: with any mouse button held, horizontal mouse movement turns the
  model about Z, 0.4° a pixel, within `ONAM` (read as degrees). The header's
  held-button flags and the talk page's "spinning" are the evidence; the
  axis, rate and keys are guesses.
- No logo is drawn on its own: every installed screen has a model, and when
  the "default logo only" property applies is unknown.

## Sound

While the loading state lasts, the audio director's `loading` input is set:
`AudioCategoryPausedDuringMenuLoad` (`PDLC`) pauses, as its name says. The
state also counts as menu mode (the Loading Menu pauses the game), so the
`PausedDuringMenu` categories pause too; whether SE treats the Loading Menu
as a menu for those isn't known. Music isn't paused or restarted by the
state itself: it plays on, and the new cell's music type takes over once the
load is done, as after any cell change. SE's behaviour is in the observation
batch.
