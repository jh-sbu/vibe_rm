# Environment maps

Implemented in `src/render/model.rs` (`material`: slots 4 and 5, the scale)
and `src/render/shaders/object.wgsl` (the reflection). Sources: nif.xml
(`BSLightingShaderProperty`'s per-type fields, the shader flags, the
controlled-float variable 8 "Environment Map Scale"), the Creation Kit
wiki's texture set slots (4 environment / cube map, 5 environment mask), and
the vanilla data (`vrm-tool envmap-shapes <bsa>...`: 6725 shapes of type 1
and 3246 eyes of type 16 in the base game's meshes; 2706 and 3146 of them
name a mask). The game's shader source isn't public.

## Settled

- **Cube map axes.** The vanilla maps are authored with the world's axes:
  `quicksky_e.dds` is brightest on +Z and darkest on -Z, its side faces
  graded towards +Z, so the reflected vector samples them unswizzled.
- **Which shapes.** Environment (`sf1` bit 7) or eye environment (bit 17)
  mapping on shader types 1 (environment map), 11 (multi-layer parallax:
  its env map strength) and 16 (eye: its cube map scale).

## Open (choices made here)

- **Combining.** The reflection is added on top of the lit surface,
  multiplied by the light reaching it (ambient, sun, point lights; not the
  emissive) but not by the diffuse colour, since the vanilla cube maps carry
  their material's tint (`bronze_e`, `ore_gold_e`...). The exact vanilla
  terms aren't published.
- **Mask.** Without an environment mask the normal map's alpha (the specular
  mask) masks the reflection; without either there is none. Model-space
  normal maps have no specular alpha, so those shapes reflect only with a
  mask.
- **Eyes** reflect about the surface normal: the per-eye centres type 16
  stores (a sphere for each eye) are skipped.
- **Effect shaders' environment maps** (`BSEffectShaderProperty`'s env map
  and mask textures) aren't read.
- **`ENVMAP_LIGHT_FADE`** (`sf2` bit 15) isn't used.
