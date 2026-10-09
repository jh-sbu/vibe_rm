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
  shadows, point lights) and glow maps added; normal maps, specular,
  parallax and the colour's alpha are unused.
- **Not done**: decals made at runtime (blood from hits, `ImpactDataSet`),
  skinned decals on actors.
