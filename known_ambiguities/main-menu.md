# Main menu choices

Research: `research/ui/main-menu.md` (2026-10-30). Each choice below stands
until the SE observations listed there settle it.

## Entries

The PC `sendMenuProperties` arguments show Continue (when there are saves),
New, Load, Credits and Quit. Add-Ons (argument 5) and Help (argument 6) are
left out: UESP's [Skyrim:Controls](https://en.uesp.net/w/index.php?oldid=3637273)
lists Downloadable Content without saying which edition, and nothing in the
assets says what `HELP` does. `OnDisabledLoadPress` (Load pressed with no
saves) does nothing. The version text is the engine's crate version.

## Continue

`CONTINUE` is always answered with `ConfirmContinue` and the GMST
`sConfirmContinue` ("Continue from your last saved game?"), the only text the
data has for it; whether SE skips the question at the main menu is unknown.
The save it loads is the newest by write time across all characters, passing
over corrupt saves and saves made with other plugins, as the Quickload key
already does. UESP says only "your most recent saved game". With saves but
none loadable, Continue shows "No save can be loaded." and stays at the menu.

## New

There is no new-game path yet (ROADMAP item 11). `NEW` shows the message "Starting
a new game isn't available yet." and sends no `ConfirmNewGame`, so the menu
never calls `StartNewGame`.

## Behind the menu

The frame behind the menu is black and no music plays. The data names a main
menu cell (`MMCL`, `MainMenuCell`) holding only a heading marker, and the
installed logo model, but not how the game places the logo or camera, what
draws the smoke, or the music.

## Credits

`requestCredits` puts the whole of `interface/credits.txt` (CRLF made LF) in
the first field as HTML and leaves the second empty. `getScrollSpeed` answers
the GMST `fCreditsScrollSpeed` (2), read as pixels per movie frame as the movie
uses it. Both are inferences from the names. Closing the credits fades the
main menu back in with `FadeInMenu`.

## Quit to Main Menu

Confirmed in the journal, it happens at once, with no fade or loading screen.
Requested saves are written first. Then a new engine replaces the old one: the
load order (runtime records cleared), renderer, audio device (all voices
stopped), settings, control bindings and save folder carry over; every other
piece of state starts as at launch. The HUD and message box movies are
reopened and the other menus closed. A load from the main menu that fails
shows the error and returns to the main menu.
