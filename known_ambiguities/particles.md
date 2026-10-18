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
  the instance's transform. A world-space system's particles are carried
  back to where they were when its instance moves (impact effects' moving
  parts).
- **Emission.** The emitter controller's interpolator is the birth rate
  (particles a second, fractions carried over), its visibility keys switch
  it, both on the controller's own clock (frequency, phase, cycle). Emitting
  stops at the data's maximum count. Box emitters spread over their width,
  height and depth along the emitter object's x, y, z; cylinders over their
  radius in x / y and their height along z; spheres within their radius.
  Rigid mesh emitters sample the referenced geometry; policies and remaining
  limits are described below.
- **Directions and variations.** Declination is the angle from the emitter
  object's +z, planar angle about it from +x; each varies by up to its
  variation either way. Speed, radius and life span vary by half their
  variation either way (made up).
- **Size.** A particle's quad reaches its radius from the centre (2 × radius
  across) times the scale modifier's curve over its life (values spread
  evenly), the system node's scale and the instance's. Aspect ratio is
  width over height. Aspect flag 1 stands the quad along the particle's
  motion on screen.
- **Forces.** Planar gravity adds its strength along its object's axis (or
  the world's when world aligned) a second; spherical gravity pulls towards
  its object. Drag damps the velocity component along its object's axis,
  within its range and falloff; the formula and evidence are below.
- **Rotation** spins quads in the screen plane at speed ± variation
  (random sign when asked) from angle ± variation.
- **Subtextures.** Without a subtexture modifier each particle shows one
  random cell. With one, it starts at the start frame plus up to the start
  fudge and flips on by (end - start) over its life.
- **Spawning on death** spawns min..max particles (by the percentage
  chance) at the dead one's place, along its velocity varied by the spawn's
  direction and speed variation, up to the generation count.
- **Colour.** Particles' colour and fade go in their vertex colour; the
  material uses vertex colours, and vertex alpha unless the palette gives
  the alpha (then the fade picks the palette's row only).
- **Running.** Systems run while their object is within 6000 units of the
  camera; one first seen runs 3 seconds ahead so fires are burning. Particles are sorted with the other blended draws by their
  system's centre, not one by one, and aren't lit.
- **Addon nodes.** A `BSValueNode`'s value is the index (ADDN `DATA`) of
  the addon node whose model goes there; 35 vanilla value nodes carry 99,
  which no ADDN has (bolts and spears), and are left empty. The addon
  models are `BSMasterParticleSystem`s whose emitters are
  `BSPSysMultiTargetEmitterCtlr`s: the game presumably runs one master
  system emitting at every node using it (and the ADDN's `DNAM` holds a
  master particle cap and flags). Here each node gets its own copy of the
  addon's systems, placed at the node's rest transform, so a chandelier's
  candles flicker independently.

## Directional drag

Public evidence:

- The community-maintained [Niftools schema](https://github.com/niftools/nifxml/blob/develop/nif.xml)
  defines the drag object as the position/orientation basis, the axis as a
  local direction, range as the fully affected distance, and range falloff
  as the distance where the effect stops. It does not define an update formula.
- The community [GECK NiPSysDragModifier documentation](https://geckwiki.com/index.php?title=NiPSysDragModifier)
  describes slowing along the object's local axis and interprets falloff as
  an additional distance beyond range. This is cross-game documentation,
  not a Skyrim SE runtime measurement. Its additive-distance interpretation
  is more specific than, and potentially conflicts with, the schema's wording.
- Installed Skyrim SE `Skyrim - Meshes0.bsa` and `Skyrim - Meshes1.bsa`, read
  only through the repository's BSA/NIF asset readers, contain 2245 parsed
  drag modifiers. None has zero range or falloff; 2233 have both at FLT_MAX.
  The remaining 12 have range/falloff 100/1000 (11) or 300/750 (1).
  `meshes/architecture/markarth/water/markarthwaterfallcliff04.nif` has separate
  X/Y/Z modifiers with percentages 0.05/0.01/0.03; this supports independent
  axis damping instead of applying all three percentages to every component.
  The finite values occur there, in `markarthwaterfallcliff03.nif`, and in
  `meshes/effects/fxtg09nocturnalbirds.nif`. Assets establish authored inputs,
  not the original runtime's formula. No assets were copied into this repo.

Implemented policy and unresolved behavior:

- Capture the drag object's rest transform when describing the system;
  normalize its transformed axis and scale range/falloff by its uniform
  scale. Animated drag objects are not followed. Missing objects use the
  existing particle-object fallback (the system transform). Exact scale and
  missing-object behavior require observation.
- Use radial distance from the object's centre, full strength up to range,
  then linear decay over an **additional** falloff distance. The decay curve,
  additive versus absolute endpoint interpretation, and possible special
  zero-range behavior remain unresolved. Here zero range affects the centre,
  and zero falloff cuts off immediately outside range; negative distances
  clamp to zero. The inspected assets do not resolve those boundary cases.
- Reduce only the parallel velocity component by
  `clamp(percentage * influence * dt, 0, 1)`, preserving the existing
  per-second Euler attenuation policy. The exact temporal formula is still
  unknown. Zero axes do nothing; nonpositive/nonfinite percentages do nothing;
  large percentages stop the parallel component without reversing it.
  Ordering against other force modifiers retains the existing renderer policy.

Synthetic tests exercise description wiring, translated/rotated/scaled drag
objects, positive and negative velocity components, range/falloff boundaries,
inactive modifiers, zero axes/distances and invalid/large percentages.

## Rigid mesh emission

Public evidence:

- The community-maintained [Niftools schema](https://github.com/niftools/nifxml/blob/develop/nif.xml)
  defines `NiPSysMeshEmitter` as emitting from its referenced meshes and
  identifies five placement modes: vertices, face centres, edge centres,
  face surfaces and edge surfaces. Its velocity modes identify mesh normals,
  random directions and an authored emission axis. The schema marks the
  surface interpretations and axis semantics as uncertain. The community
  [GECK block overview](https://geckwiki.com/index.php?title=NIF_Block_Types)
  independently describes mesh placement and selectable velocity computation,
  but supplies no selection or interpolation formula. This is community
  documentation, not original Skyrim SE runtime code or measurements.
- No game assets or engine binary data were examined for this change. All
  fixtures are synthetic geometry; no game assets were added to the repository.

Implemented policy and unresolved behavior:

- Resolve every supported rigid `BSTriShape`, `NiTriShape` or `NiTriStrips`
  reference, retaining each mesh's own rest transform in model space. Choose
  uniformly among meshes that have usable primitives. Mesh-reference
  weighting, repeated references, references to nodes instead of geometry,
  and the original runtime's missing-reference behavior remain unresolved.
  Skinned meshes and `BSDynamicTriShape` are skipped; animated rigid meshes
  use their rest transform. Live deformation and transform sampling are pending.
- Sample vertices, triangle centres or unique indexed-edge centres uniformly.
  Face surfaces use area-weighted triangles with uniform barycentric samples;
  edge surfaces use length-weighted unique edges with uniform segment samples.
  Weights are measured in mesh-local space. Exact runtime weighting, shared-edge
  deduplication and whether surface sampling follows these interpretations
  remain unresolved. Invalid indices, nonfinite vertices, degenerate triangles,
  singular transforms and unsupported mode values do not produce particles.
  Vertex mode can still sample finite vertices without valid triangles.
- Normal velocity interpolates normalized authored vertex normals; missing or
  invalid normals fall back to area-weighted adjacent triangle normals.
  Transform the result with the mesh's inverse transpose. A normal that remains
  zero produces zero velocity. Random velocity uses a uniform unit sphere;
  directed velocity uses the emission axis in the selected mesh's local space.
  Speed uses the existing emitter/controller and speed-variation path. Exact
  normal interpolation, axis space and interactions with declination/planar
  variation remain unresolved; those angular inputs are ignored for mesh
  velocity modes here. Existing instance/world-space particle behavior applies.

Synthetic tests cover NIF-description wiring for legacy and packed rigid
geometry, unsupported deformation, all five placement modes, all three velocity
modes, independently transformed meshes, surface sampling distribution, shared
edges, primitive weights, missing normals and malformed topology/references.
