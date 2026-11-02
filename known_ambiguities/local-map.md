# Local map

## Area bounds

The exact local-map area selection is not documented in the research collected
in `plans/local-map.md`. We use the transformed OBND boxes of the interior
cell's references, excluding markers and records flagged Not On Local Map.
For exteriors, we use the square of cells loaded around the player. The
interior rule, in particular, may include references the game does not use or
miss geometry outside an authored OBND.

## Appearance and visible geometry

The movie requests an engine-rendered image, and the public camera layouts
listed in `plans/local-map.md` identify an image-space pass without describing
its appearance. We use fixed neutral lighting and a grey image, and draw the
static environment, terrain and water without actors, effects, weather or fog.
We cull back faces even on double-sided objects, inferring that this removes
many downward-facing ceilings. It is unknown whether the game uses this
method or exactly which objects and materials it includes. The user's vanilla
observation establishes that higher levels obscure lower levels in the fixed
projection.

## Fit

The movie specifies an 800 by 450 image but not the camera padding. We fit
all rotated bounds to that aspect ratio and add five percent of the larger
side on each edge. The actual game's margin and minimum view size remain
unknown.

## Map motion

The movie exposes an image and marker fractions but does not document how the
engine pans or zooms its camera. We change the orthographic box and render it
again at most four times per second during motion and on drag release. Render
cost is logged at info level. This keeps the image and marker projection in
step; the game's actual update frequency is unknown.

A Bleak Falls Barrow capture exposed effects with full-range signed 16-bit
OBND values (for example, `FXDrips01tall` and `FXAmbBlowingFog01`). Those
boxes expand far beyond the dungeon, so the map omits objects with that exact
sentinel box when fitting and drawing. This is a practical filter based on the
installed assets; other effects may use different bounds.

## Exploration and reveal

UESP's [map bugs](https://en.uesp.net/w/index.php?oldid=3651114) describe
unmapping in rectangular pieces but do not specify the grid or reveal rule.
We use 256-unit squares in 16-by-16 pages, with separate grids per interior
and worldspace. Every half second the player reveals square centres within
2,560 units (twenty 128-unit standing body lengths); exteriors are limited to
loaded cells. The user observed roughly twenty body lengths and provisionally
the same distance indoors and outdoors; the exact range remains unverified.
We do not check line of sight, lights, or height, and do not know the game's
exact rule for those factors.

Unrevealed pixels are black in the generated image; markers remain a separate
movie layer. The user observed that doors and quest targets remain visible in
unexplored areas. The game's exact fog colour and opacity are not established.
