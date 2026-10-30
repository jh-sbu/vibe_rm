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
