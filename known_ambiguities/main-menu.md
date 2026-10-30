# Main menu choices

Research: `research/ui/main-menu.md` (2026-10-30), including what the user
observed in Skyrim SE. Each choice below stands until a source or an SE
observation settles it. Deliberate deviations are in `deviations/main-menu.md`.

## Continue's save

Continue loads the newest save by write time across all characters, passing
over corrupt saves and saves made with other plugins, as the Quickload key
already does. UESP says only "your most recent saved game". With saves but
none loadable, Continue shows "No save can be loaded." and stays at the menu.
(SE loads saves made with fewer plugins and asks about missing ones; that
needs loading across load orders, `plans/main-menu.md`.)

## New

There is no new-game path yet (ROADMAP item 11). After SE's "Start a new
game?" confirmation and the menu's fade, `StartNewGame` shows "Starting a new
game isn't available yet." and the menu fades back in.

## Title music and backdrop

The title track (`music/special/mus_maintheme.xwm`) plays looping with the
default music category's parameters (`MDSC`) from the time the main menu opens
until a game loads. Whether SE loops it, and through which category, is
unknown. The frame behind the menu is black: the data names a main menu cell
(`MMCL`, `MainMenuCell`) holding only a heading marker, and the installed logo
model, but not how the game places the logo or camera, or what draws the
smoke.

## Credits

`requestCredits` puts the whole of `interface/credits.txt` (CRLF made LF) in
the first field as HTML and leaves the second empty. `getScrollSpeed` answers
the GMST `fCreditsScrollSpeed` (2), read as pixels per movie frame as the movie
uses it. Both are inferences from the names. Closing the credits fades the
main menu back in with `FadeInMenu`.

## Quit to Main Menu

SE fades quickly to black, then shows the main menu with no loading screen.
Here the journal closes, the HUD hides, and the world fades to black over the
sleep fade's 0.5 s (`SLEEP_FADE_SECONDS`); SE's duration isn't measured.
Requested saves are written, then a new engine replaces the old one. The load
order (runtime records cleared), renderer, audio device (all voices stopped),
settings, control bindings and save folder carry over; every other piece of
state starts as at launch. The HUD and message box movies are reopened and
the other menus closed. A load from the main menu that fails shows the error
and returns to the main menu, title track included.
