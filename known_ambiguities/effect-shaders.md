# Effect shader rendering decisions

Research attempted: local UESP [EFSH revision 3089075](https://en.uesp.net/w/index.php?oldid=3089075)
(the DATA struct is explicitly undocumented there), xEdit dev-4.1.6's public
serialized definitions, MIT CommonLibSSE-NG's TESEffectShader declaration, and
installed EFSH records. Public source copies, licenses and hashes are retained
in ignored `refs/spell-visuals/`; no implementation code was copied from them.
D3D9 enum values were checked against Microsoft's public documentation for
[blend factors](https://learn.microsoft.com/en-us/windows/win32/direct3d9/d3dblend),
[blend operations](https://learn.microsoft.com/en-us/windows/win32/direct3d9/d3dblendop)
and [comparisons](https://learn.microsoft.com/en-us/windows/win32/direct3d9/d3dcmpfunc).
No game-engine binary was examined.

The layouts establish inputs, not shader equations. These are the chosen
approximations where public information and asset inspection did not settle them:

- **Membrane equation.** Linear fade-in, full-alpha and fade-to-persistent stages;
  stopping fades from the current envelope value. Pulse multiplies that value
  with a sine. Fill RGB keys and scales interpolate over elapsed seconds; global
  Color Scale multiplies them. The edge uses a power of one minus absolute
  view/normal dot product, with inversion when requested; fill and edge colors
  combine by opacity. Holes use the authored threshold/time range with a smooth
  edge. The exact pulse phase, key-time units, edge/fill compositing and palette
  row convention are not specified publicly. Compare timed captures of Oakflesh's
  actual ShieldSpellFXS, Soul Trap, fire burns and an RFCT holes dissolve in SE.
- **Depth and surface placement.** Reuse the same posed mesh with zero normal
  inflation and zero depth bias. An offscreen check exposed that a nearer bias
  incorrectly rejected Oakflesh's authored GreaterEqual comparison after depth
  reversal; identical geometry retains depth equality. The original extrusion
  amount remains unknown. Compare close silhouette/occlusion captures in SE.
- **Which shapes.** Affect Skin Only recognizes FaceGen/skin-tint materials;
  No Weapons excludes weapon-path attachments. This does not establish the
  original classification of unusual modded equipment. Compare head, body,
  clothing, armour, a shield and weapons separately.
- **Stacking.** All distinct shader owners draw, in deterministic target/form/id
  order, without a shader-count cap. Replaying a scripted form on one target
  refreshes that form; RFCT and EffectShader callers have independent keys even
  if they refer to the same EFSH. Order/cap are not recoverable from record layouts.
  Compare two noncommuting colored shaders applied in opposite orders in SE.
- **Particles.** Birth ramps interpolate between full ratio and persistent count;
  count is interpreted as particles per second. Surface samples are uniform by
  triangle index, with uniform barycentric points; only sampled vertices are
  skinned using the current pose and weight morph. Normal speed/acceleration,
  world velocity/acceleration, lifetime variation, size/color keys and rotation
  use their authored values. Frame interpolation traverses start/end over the
  particle's normalized life. Exact density/area normalization, vector frame,
  frame-count/variation/loop semantics and palette timing remain unestablished.
  A 4096-particle safety cap bounds each shader instance. Compare on differently
  sized actors, during a pose change, and against a shader with animated frames.
- **Addon models.** DEBR addon fade/scale values are parsed and retained, but
  addon rendering is omitted as the plan specifies; public field names do not
  establish spawn placement/timing. Compare an EFSH that actually uses DEBR to
  establish those policies before implementing it.

Offscreen engine captures were inspected for a membrane on the player, shader
particles and Flames hand art/lights. They verify that the rendering paths run,
not visual parity with SE. The SE comparisons above have not been performed.
