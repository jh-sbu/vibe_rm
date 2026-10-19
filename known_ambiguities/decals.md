# Projected decals

Implemented in `src/world/decal.rs` (decal data, placement) and the
renderer's decal pass (`src/render/decal.rs`, `vs_decal` / `fs_decal` in
`object.wgsl`). Sources: the TXST `DODT` layout (UESP: min / max width and
height, depth, shininess, parallax scale and passes, flags, colour) and the
vanilla placements (`vrm-tool decals <data> [texture set]`: 59 texture sets
with decal data, 3210 references placing them). How the game projects them
isn't public.

## Settled

- **Axis.** A placed decal projects along its reference's +Y: 2460 of the
  2713 vanilla ground blood and burn decals point it down.
- **Flags** (`DODT` byte 29): 0x02 alpha blending, 0x04 alpha testing,
  0x08 no subtextures (the puddles' "LargeSmallGroup" set it and use the
  whole texture; the single-puddle sets don't).

## Open (choices made here)

- **Reach.** The references aren't on their surfaces: wall paint in
  Sleeping Tree Cave sits 150-270 units in front of the wall, beyond its
  48-unit half depth. Decals are read as rays: from the reference along
  +Y, up to 1024 units, against the cell's collision (actors ignored); the
  box is centred on the hit. Those that hit nothing aren't drawn. The real
  reach and whether collision or render geometry is used are unknown.
- **Size.** Width and height are picked per reference between the minimum
  and maximum (hashed from its FormID), times its scale.
- **Subtextures**: a 2x2 atlas, one quarter picked per reference.
- **Screen-space projection.** Decals are drawn as boxes over the opaque
  scene's depth, so anything inside a box takes them (the game bakes them
  into geometry); surfaces facing away from the projection fade out, which
  spares most passers-by. Grass in a box takes them too.
- **Shading**: lit with the surface's normal from depth (ambient, sun with
  shadows, point lights) and glow maps added. The texture set's normal map
  bends that normal in the decal's frame (u along the box's X, the image's
  top along its Z, flattened onto the surface) and its alpha masks a white
  Blinn-Phong specular whose exponent is the `DODT` shininess (capped at 1024:
  vanilla runs from 0, which gives none, to 40000). Whether the game's decal
  specular is Blinn-Phong with that exponent is not public.
- **Parallax** (`DODT` flag 0x01, the scale and the passes byte; xEdit: at
  most 30). No vanilla decal sets the flag (vanilla scales are 1, and 0.175 on
  the dragon's landing mark), so nothing is seen in the base game. A decal with
  the flag and a height map (slot 4) is marched in that many steps along the
  view ray; a scale of 1 shifts the texture by 0.04 of it at grazing angles.
  That constant, and whether the height map is read as depth from the top, are
  guesses.
- **Alpha blending flag** (0x02). Decals always blend, except that an alpha
  tested one without the flag (vanilla: giant paint, Riekling paint, the Soul
  Cairn wall) is cut out hard-edged (opaque past the 0.5 test). Those with
  neither flag (blood hits on actors, 0x08) still blend. What the game does with
  no blending and no testing isn't known.
- **Colour alpha.** The `DODT` colour's fourth byte is 0 in all 59 vanilla
  sets, so it can't be an opacity (everything would vanish); it's ignored and
  the colour's RGB tints the decal.
- Decals made at runtime by impacts, on the world and on actors' bodies:
  `known_ambiguities/impacts.md`.
