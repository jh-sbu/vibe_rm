# Game queries and flags

Public pages consulted 2026-10-24 (Papyrus Index / community CK wiki text,
summarized), https://papyrus.bellcube.dev/skyrimse/script/game/function/
`findclosestactor` (the player is an actor and can be returned),
`findrandomactor`, `findclosestreferenceoftype`, `findrandomreferenceoftype`,
`findclosestreferenceofanytypeinlist` / `findrandomreferenceofanytypeinlist` (the
list page notes it may use editor positions; more testing needed), `getsunposition`
x / y / z, `isplayersungazing` (looking directly at the sun), `setplayerreportcrime`
(enable or disable crime reporting on the player), `clearprison` (clears the
prison variables so the game knows the player is out), `setinchargen` (disable
saving, waiting; show a controls-disabled message), `calculatefavorcost`.
Vanilla use (`vrm-tool pex-calls`): `DLC1MagicCastFromSunScript` puts an effect at
the player plus the sun's position times a scale and clamps its Z at 0.25 before
taking arctangents, so the position is a direction of about unit length;
`SetInChargen` 33 calls (chargen quests); `critterFish` / `critterMoth` find random
plants within their leash.

Implementation (`src/game_queries.rs`; inference where marked). Saved as `GAMF`:
- Finders search the loaded cells' references (actors: the loaded ones and the
  player), within a sphere, none disabled. Dead actors are included (the sources
  don't exclude them). Closest breaks ties by form id; random picks uniformly
  from the set in form id order with the engine RNG.
- `GetSunPosition*`: the unit direction from the world to the sun
  (`scene.env.sun_dir`).
- `IsPlayerSungazing`: outdoors, the sun above the horizon, the camera within about
  12 degrees of it and nothing blocking the ray (the angle is ours).
- `SetPlayerReportCrime(false)`: `commit_crime` reports nothing.
- `ClearPrison`: forgets the sentence, with none of the events or the return of
  belongings that serving or escaping bring.
- `SetInChargen`: saving (script requests and named saves) and waiting (the sleep /
  wait menu) are refused while their flags are on. The controls-disabled message
  isn't shown: the string it would use is unknown (it is stored).
- `CalculateFavorCost`: no source gives the formula and nothing calls it; it
  returns the price.

Tests: synthetic references; payload and legacy-load tests for `GAMF`.
