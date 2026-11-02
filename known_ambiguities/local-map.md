# Local map

## Area bounds

The exact local-map area selection is not documented in the research collected
in `plans/local-map.md`. We use the transformed OBND boxes of the interior
cell's references, excluding markers and records flagged Not On Local Map.
For exteriors, we use the square of cells loaded around the player. The
interior rule, in particular, may include references the game does not use or
miss geometry outside an authored OBND.
