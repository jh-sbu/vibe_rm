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
