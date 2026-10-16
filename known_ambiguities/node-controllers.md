# Standalone NIF node controllers

Loaded placed objects now run `NiTransformController` and `NiVisController`
chains on their nodes and rigid geometry, independently of named sequences.
Effects continue to use their existing effect clock. No engine executable data
was examined and no game assets are stored in the repository.

## Public and asset evidence

- The community-maintained [Niftools schema](https://github.com/niftools/nifxml/blob/develop/nif.xml)
  defines the transform and visibility controller inheritance, interpolator
  references and controller chains. Its `TimeControllerFlags` names active,
  manager-controlled, cycle and animation-type bits, with `APP_TIME` and
  `APP_INIT` as the animation types. This establishes the serialized controls,
  but does not specify Skyrim's initialization and controller arbitration.
- The same schema defines `NiVisController` as targeting `NiAVObject`;
  `BSTriShape` inherits it directly, and `NiTriShape` / `NiTriStrips` through
  `NiGeometry`. Geometry therefore uses the existing transform and visibility
  evaluator; no engine binary inspection or game-derived fixtures were needed.
- Installed Skyrim SE assets, read through the repository's BSA/NIF readers:
  `meshes/architecture/solitude/farms/sfarmhousemillfan.nif` has one active
  looping transform controller, flags `0x48`, with a 13.333334 s cycle.
  Its animated node has rotation keys and an unset interpolator pose;
  the node's unkeyed translation must therefore remain at rest.
  `meshes/architecture/solitude/farms/ssawwaterwheel.nif` has one such
  controller with a 16.666668 s cycle. Neither model has a named sequence.
  `meshes/architecture/solitude/clutter/sropefestivalline01.nif` has nine
  active looping node transform controllers with staggered key ranges.
  These observations remain asset evidence; automated tests use synthetic
  models and do not require an installed game.

## Implemented behavior

- Only the `#world` model variant, selected by `Engine::add_objects`, splits
  standalone controller targets for placed objects. Default loads retain
  static geometry, so actor attachments and distant references keep their
  previous rendering. Effects also use block-identity splitting now.
- Targets are identified by NIF block index, including controlled root nodes;
  duplicate or empty names cannot select unrelated nodes. Empty parent parts
  remain in the flattened hierarchy. Children inherit their animated parent's
  transform and visibility. Scaling updates the render bounds.
- Rigid `BSTriShape`, `NiTriShape` and `NiTriStrips` can own controller chains.
  Each controlled shape becomes one part with vertices in its local space;
  its rest transform and ancestors apply once. Geometry filters still apply,
  and a same-named uncontrolled sibling remains static. Effects use this same
  conversion. Skinned shapes retain the existing bone-driven path.
- APP_TIME controllers sample the renderer's simulation clock; APP_INIT
  controllers sample time since the placed object loaded. Frequency, phase and
  cycle use the existing NIF timing evaluator. Boolean keys step at their key
  times. Inactive APP_TIME and manager-controlled channels do not run.
- Named sequences override their keyed transforms while playing and leave
  unrelated controllers running. Parts without an independent controller retain
  their terminal local poses afterward; standalone controllers keep updating.
  A behavior graph's transforms take final precedence. Visibility applies to
  either kind of pose, without changing the reference's separate hidden state:
  a controller cannot show an object hidden by Disable or MoveTo.
- Invisible parts are omitted from ordinary and shadow draws and the placed
  particle pass. Particle evaluation precedes object animation, so emission
  and particle visibility follow the previous frame's pose/visibility.

## Unresolved and unsupported

- **Initialization and persistence:** public flags do not establish the exact
  original clock origin, whether inactive APP_INIT controllers always start,
  or restart/catch-up behavior after Disable/Enable and cell unload/reload.
  Here APP_INIT starts with each loaded instance, pauses with simulation, and
  restarts on reload; it is not saved. This is an explicit approximation.
- **Competing drivers:** ordering between standalone controllers, named
  sequences and behavior poses needs a game-side comparison. The precedence
  above is this runtime's policy, not a claim about original engine order.
- **Target scope:** controllers directly attached to skinned geometry or
  particle systems, initially hidden NIF subtrees, controller chains interrupted
  by an unparsed controller, and standalone animation of distant references,
  actor attachments, loose physics objects and harvested variants remain
  unsupported. Rigid geometry
  uses the same render-only controller policy as nodes; collision is not moved.
- **Physics and particles:** standalone transforms move render parts, not
  their collision. Graph-driven collision retains its existing behavior.
  World-space particles in moving standalone parts do not yet receive the
  carry adjustment used by impact effects. Hidden placed emitters currently
  clear their particle state, rather than preserving live particles offscreen.
- **Timing and curves:** playback-backwards/forced-update/scaled-time flags
  beyond the existing evaluator and exact nonlinear key interpolation remain
  separate work. This change does not claim to resolve them.

Synthetic tests cover block identity, controlled roots, empty parents,
manager/inactive filtering, nested transforms/visibility, scaled bounds,
Disable preservation, APP_INIT timing and coexistence with a named sequence.
Synthetic models with one rotating node, one translating node and nine
translating nodes also check geometry retention, splitting without duplicate
meshes, and sampled transforms.
Synthetic geometry tests cover all three supported shape formats, local
vertices and inherited placement, directly controlled shape roots, nested
node/shape chains, stepped visibility, duplicate names, shape filtering,
effect conversion and skinned exclusion.

## Texture flips: evidence gap

`NiFlipController` remains unsupported. The public
[Niftools schema](https://github.com/niftools/nifxml/blob/develop/nif.xml)
describes texture-slot indices and `NiSourceTexture` references, with a float
interpolator in the supported NIF version. Its `Delta` field belongs only to
older versions. The schema does not establish a mapping onto Skyrim's
`BSShaderTextureSet` / `BSEffectShaderProperty` bindings. No public evidence
reviewed here establishes Skyrim's acceptance of those targets or frame-index
rounding and out-of-range behavior. This is an unresolved compatibility question,
not evidence that Skyrim rejects all flip controllers. Implementing legacy
`NiTexturingProperty` or inventing a BS shader slot mapping would exceed the
bounded geometry-controller work above.
