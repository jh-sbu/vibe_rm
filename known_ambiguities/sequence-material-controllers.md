# Material controllers in named NIF sequences

Placed rigid objects can now select the shader and alpha-test interpolators in
their named NIF sequences through `PlayGamebryoAnimation`, doors' Open / Close,
and automatic Idle playback. This uses the existing shader-variable evaluator
and cutout uniform. It does not add another material pass or enable alpha
testing on a property that has it disabled.

## Public evidence

- The community [Niftools schema](https://github.com/niftools/nifxml/blob/develop/nif.xml)
  declares each controlled block's interpolator and controller references,
  node / property identifiers, sequence frequency and cycle, and the manager
  controlled flag. The shader controller supplies the controlled variable;
  the alpha-test controller is a float interpolator controller.
- [NifSkope's controller implementation](https://github.com/niftools/nifskope/blob/develop/src/gl/controllers.cpp)
  selects shader and alpha controllers through their property and exact
  controller reference, then binds the controlled block's interpolator.
  Its shader / alpha branch differs from its generic controller branch:
  the former does not copy sequence timing fields. Its alpha controller
  divides the sampled test reference by 255. This supports the binding and
  units, but does not establish Skyrim's sequence-clock arbitration.

Only these public community sources and synthetic models were used for this
change. No game engine binary or executable data was examined and no game
assets or game-derived test fixtures are included.

## Supported behavior

- A material binds only controlled blocks whose controller reference is
  actually in that shader or alpha property's controller chain. The sequence's
  interpolator supplies the float / colour keys or valid constant pose, even
  when the controller's own interpolator is a manager blend interpolator.
  Duplicate geometry names do not select unrelated materials. Controller
  cycles are bounded by block identity.
- Manager-controlled shader channels, like alpha-test channels, do not run as
  standalone animations. Other property-owned channels retain their existing
  clock. A selected sequence overrides only its material channels after those
  standalone values have been evaluated.
- Sequence-only materials on unskinned `BSTriShape`, `NiTriShape` and
  `NiTriStrips` create render parts in the placed-object conversion. Nested
  empty parents and part relationships are retained. Default conversions,
  including actor attachments, keep their previous geometry organization.
- Each placed reference has separate material uniforms and selection state;
  its geometry and immutable animation descriptions remain shared. Starting
  or advancing one copy therefore cannot animate another copy's materials.
  Both the ordinary and cutout-shadow passes use the resulting uniform.
- The runtime uses the selected sequence's key time for both transforms and
  material keys: its frequency advances the key clock once, its loop / clamp
  handling maps that time, and the property's standalone frequency, phase and
  range are not applied again. This is an explicit runtime policy pending an
  original-game comparison, not a claim established by NifSkope.
- A completed clamped sequence retains its terminal material sample. A new
  selection restarts its key clock and replaces the old selection. Materials
  not keyed by that new sequence return to their base plus standalone values.
  This follows the runtime's single-sequence policy; the original engine's
  retention and competing-driver behavior remain unresolved.

## Remaining ambiguities and scope

- The exact relationship between controller and sequence timing, priorities,
  weights, blending and transitions remains unresolved. Multiple manager
  scopes with duplicate sequence names and simultaneous sequence ownership
  need separate handling. No claim is made that these runtime policies match
  the original engine's arbitration.
- Reverse cycle mode retains the existing placed-sequence limitation (it
  reaches and holds the stop pose rather than ping-pong playback). Negative
  frequency / backwards playback, nonlinear float or colour interpolation,
  initialization ordering and save / reload clock persistence remain separate
  work. Positive sequence frequency is now respected by transforms as well
  as material channels.
- Controlled blocks without an actual controller link are not resolved by
  node name, property type or controller/interpolator IDs. External KF binding,
  chains interrupted by unknown controller types and initially hidden shapes
  remain unsupported. No guessed name mapping is substituted.
- This wires named material selection only for placed rigid objects. Actor
  skinned geometry, actor graph sequences, effect graph / projectile sequences,
  particles and addon model sequences are not included. The effect converter
  may retain material-controlled rigid parts, but effect playback still uses
  its existing standalone controller clock.
- Alpha-test byte quantization and comparison-function limitations remain in
  [alpha-test-controllers.md](alpha-test-controllers.md).

Synthetic tests cover material-only conversion in all three rigid formats,
duplicate names and exact controller identity, sequence interpolator binding,
float / colour channels, nonunit sequence frequency without a second controller
clock mapping, independent material cursors, terminal retention and reselection,
and coexistence with standalone channels. These tests do not require a GPU or
an installed game; visual parity and original-engine timing remain unverified.
