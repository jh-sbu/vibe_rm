# Soft effects

Implemented in `src/render/mod.rs` (the depth copy and the pass split in
`Renderer::render`), `src/render/model.rs` (`soft_depth`) and `soft_fade` in
`src/render/shaders/object.wgsl`. Sources: the `BSEffectShaderProperty`
fields in nif.xml and CommonLibSSE's `BSEffectShaderMaterial`
(`softFalloffDepth` at 0x68), the shader flag name `SLSF1_Soft_Effect`
(bit 30), and the vanilla data (`vrm-tool soft-effects <bsa>...`). The
game's shader source isn't public.

## Settled from the data

- **The flag gates the depth.** Effect shaders hold a soft falloff depth
  whether or not bit 30 of shader flags 1 is set: in the base game's
  meshes 3871 shaders carry the default 100 without the flag (and 135 a 5,
  164 a 2...), while the flagged ones carry authored values (64, 32, 100,
  5, 10, 8, 25, 50, 2048...). The depth is therefore read only with the flag.
- **Range.** Flagged depths run from 1 to 2048; the large ones belong to wall
  mist and big crystals (`fxdlc1scwallmistcorner02`: 256,
  `dlc01idealmastercrystal`: 2048), the small ones to fire, impacts and
  smoke (`fxfirewithemberslogs01`: 10, the shout words: 2), which reads as
  world units.

## Open (choices made here)

- **The formula.** An effect with the flag fades to nothing where it meets
  the opaque scene behind it: its opacity is multiplied by the gap along the
  view direction between it and the scene surface at that pixel, over the
  soft falloff depth, clamped to 0..1 (the usual soft particle fade, linear
  in view depth). Whether the game fades linearly, by distance along the
  ray or along the view axis, or with another curve isn't documented.
- **Which surfaces.** The scene is what the opaque pass (terrain, opaque
  objects, grass, skinned actors) and the decals drew; blended things, the
  water and other effects don't count, and an effect shows through the
  surfaces it doesn't touch as before. Effects drawn with the opaque pass
  (opaque blending, skinned effect models) aren't softened.
- **Particles** with a flagged shader use the same fade (their quads share
  the shader).
- **Where the sky is** (no opaque surface behind) the effect keeps its full
  opacity.
- **Without a depth test** a flagged effect behind a wall fades away
  instead of showing through (the gap is negative there).

`VRM_NO_SOFT_EFFECTS=1` turns the fade off.
