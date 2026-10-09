# Particle systems

Implemented in `crates/nif/src/psys.rs` (the blocks, after niftools'
nif.xml) and `src/render/particles.rs` (description, simulation, quads).
Sources: nif.xml's layouts and the few defaults and comments it has
(`BSPSysSimpleColorModifier` fades default to 0.1 / 0.9, `AspectFlags` bit 0
"Velocity Orientation"); the vanilla meshes (`vrm-tool nif-blocks <bsa>... --
<type>` lists a block type's values across every NIF, `vrm-tool nif-verify`
checks the layouts: every particle block that is parsed reads its declared
size exactly). Gamebryo's own particle code isn't public, so how the values
are used below is this engine's reading.

## Settled

- **Layouts** of `NiParticleSystem` / `BSStripParticleSystem`, `NiPSysData`
  (its vertex arrays are sized by a count SSE doesn't store: always empty;
  then atlas cells, aspect ratio and flags), the modifiers and emitters, the
  controllers and `NiBoolInterpolator` / `NiBoolData`.
- **Atlas cells** are (u, width, v, height): the vanilla data's cells step
  their first and third values by the second and fourth.
- **Simple colour fades**: `fade_out` is never below `fade_in` in the 1666
  vanilla modifiers and defaults to 0.9, so it's where the fade out starts
  (in fractions of the particle's life). The four colour percents are
  monotonic in 1496 of them and are read as keyframes: colour 0 until the
  first, blending to colour 1 by the second, held to the third, blending to
  colour 2 by the fourth.

## Open (choices made here)

- **Space.** Particles are simulated in the model's space and drawn through
  the instance's transform, `world_space` or not; nothing that carries
  particles moves yet.
- **Emission.** The emitter controller's interpolator is the birth rate
  (particles a second, fractions carried over), its visibility keys switch
  it, both on the controller's own clock (frequency, phase, cycle). Emitting
  stops at the data's maximum count. Box emitters spread over their width,
  height and depth along the emitter object's x, y, z; cylinders over their
  radius in x / y and their height along z; spheres within their radius.
  Mesh emitters emit at their first mesh's origin (the mesh's vertices and
  faces aren't used yet).
- **Directions and variations.** Declination is the angle from the emitter
  object's +z, planar angle about it from +x; each varies by up to its
  variation either way. Speed, radius and life span vary by half their
  variation either way (made up).
- **Size.** A particle's quad reaches its radius from the centre (2 × radius
  across) times the scale modifier's curve over its life (values spread
  evenly), the system node's scale and the instance's. Aspect ratio is
  width over height. Aspect flag 1 stands the quad along the particle's
  motion on screen; speed to aspect (flag 0x100) isn't used.
- **Forces.** Planar gravity adds its strength along its object's axis (or
  the world's when world aligned) a second; spherical gravity pulls towards
  its object. Decay and turbulence aren't used. Drag takes its percentage of
  the velocity a second; its object, axis and range aren't used.
- **Rotation** spins quads in the screen plane at speed ± variation
  (random sign when asked) from angle ± variation; the axis isn't used.
- **Subtextures.** Without a subtexture modifier each particle shows one
  random cell. With one, it starts at the start frame plus up to the start
  fudge and flips on by (end - start) over its life; loop start and frame
  count aren't used.
- **Spawning on death** spawns min..max particles (by the percentage
  chance) at the dead one's place, along its velocity varied by the spawn's
  direction and speed variation, up to the generation count.
- **Colour.** Particles' colour and fade go in their vertex colour; the
  material uses vertex colours, and vertex alpha unless the palette gives
  the alpha (then the fade picks the palette's row only).
- **Running.** Systems run while their object is within 6000 units of the
  camera; one first seen runs 3 seconds ahead so fires are burning. LOD
  modifiers, colliders, bombs, inherited velocity, recycle bounds, strip
  systems (trails) and mesh particle systems are not used. Particles are sorted with the other blended draws by their
  system's centre, not one by one, and aren't lit.
- **Addon nodes.** A `BSValueNode`'s value is the index (ADDN `DATA`) of
  the addon node whose model goes there; 35 vanilla value nodes carry 99,
  which no ADDN has (bolts and spears), and are left empty. The addon
  models are `BSMasterParticleSystem`s whose emitters are
  `BSPSysMultiTargetEmitterCtlr`s: the game presumably runs one master
  system emitting at every node using it (and the ADDN's `DNAM` holds a
  master particle cap and flags). Here each node gets its own copy of the
  addon's systems, placed at the node's rest transform, so a chandelier's
  candles flicker independently; caps, the `DNAM` flags, the nodes'
  keyframe animation (the blood spray's spinning nodes) and the value
  node's flags aren't used.
