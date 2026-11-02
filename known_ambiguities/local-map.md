# Local map

## Area bounds

The exact local-map area selection is not documented in the research collected
in `plans/local-map.md`. We use the transformed OBND boxes of the interior
cell's references, excluding markers and records flagged Not On Local Map.
For exteriors, we use the square of cells loaded around the player. The
interior rule, in particular, may include references the game does not use or
miss geometry outside an authored OBND.

If an interior has no usable reference bounds, we centre a 512-unit fallback
box on the player so its local map can still open. The game's behavior for
such a cell is unknown.

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

A Bleak Falls Barrow capture exposed effects with full-range signed 16-bit
OBND values (for example, `FXDrips01tall` and `FXAmbBlowingFog01`). Those
boxes expand far beyond the dungeon, so the map omits objects with that exact
sentinel box when fitting and drawing. This is a practical filter based on the
installed assets; other effects may use different bounds.

## North and interior exit

The [UESP map export tutorial](https://en.uesp.net/w/index.php?oldid=2395286)
identifies NorthMarker's Z angle as the rotation for the local map export.
Our camera uses the same clockwise heading as the reference, putting the
marker facing up; exteriors use true north as observed by the user. If an
interior has no NorthMarker we assume zero rotation; that fallback is not
verified. When the world map is shown from an interior, we place the player
at the exterior door reached by a shortest load-door route, preferring the
nearest first door. The user observed the local map opens first, but the
world-map marker position remains unverified.

## Fit

The movie specifies an 800 by 450 image but not the camera padding. We fit
all rotated bounds to that aspect ratio and add five percent of the larger
side on each edge. The actual game's margin and minimum view size remain
unknown.

## Map motion

The movie exposes an image and marker fractions but does not document how the
engine pans or zooms its camera. We change the orthographic box and render
each changed view in the same UI update, before projecting its markers.
The original 250 ms redraw throttle was removed at the user's request after
profiling the CPU mask and readback costs (`research/ui/local-map.md`). Render
cost is logged at info level; the game's camera update method and frequency
remain unknown.

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
unexplored areas. We hide the custom destination in unexplored squares; its
vanilla visibility there, and the exact fog colour and opacity, are unverified.
