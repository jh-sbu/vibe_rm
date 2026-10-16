# Alpha-test controllers

`BSNiAlphaPropertyTestRefController` is parsed by `crates/nif/src/anim.rs`.
`src/render/model.rs` merges its alpha property's own controller chain with
the shader property's animation; `src/render/mod.rs` updates the material's
test reference on the existing controller clock. The object and cutout-shadow
passes use that same uniform. Alpha testing must already be enabled on the
property; the controller does not enable it.

## Public evidence and assets

- [niftools' nif.xml](https://github.com/niftools/nifxml/blob/develop/nif.xml)
  declares this controller as a `NiFloatInterpController` without additional
  fields. Its inherited SSE payload is the time controller followed by one
  interpolator reference. It identifies time-controller bit 5 as Manager
  Controlled and the property's threshold as a byte.
- [NifSkope's AlphaController](https://github.com/niftools/nifskope/blob/develop/src/gl/controllers.cpp)
  samples the float keys and divides their value by 255 before setting the
  alpha-test reference.
- [NifSkope's AlphaProperty](https://github.com/niftools/nifskope/blob/develop/src/gl/glproperty.cpp)
  attaches this controller to `NiAlphaProperty` and keeps test enablement
  separate from its reference.
- Installed base-game mesh assets corroborate the byte range: Alduin's
  `SkinFadeOut` keys run from 5 to 255, `SkinFadeIn` from 255 to 5. The DLC2
  lurker's projectile-explosion mesh has keys from 255 through 118 and back
  to 255. Only NIF assets were inspected; no executable or engine binary
  data was used. No asset data is included in the repository.
- Verification of `Skyrim - Meshes0.bsa` and `Skyrim - Meshes1.bsa` reads
  22,047 NIFs without failures. All 767 alpha-test controller blocks consume
  their declared size exactly: 637 are manager controlled and 130 are
  standalone (51 active clamped, 44 active looping, 35 APP_INIT clamped).

## Remaining limits and choices

- **Sequences.** Manager-controlled controllers are parsed but excluded from
  standalone playback. Alduin and the inspected lurker projectile explosion
  bind their float interpolators through `NiControllerSequence` and
  `NiBlendFloatInterpolator`; binding and blending those shader / alpha
  channels during graph sequence playback remains unimplemented. This change
  supports property-owned standalone controllers, including mod content.
- **Quantization.** References are sampled continuously and divided by 255,
  following NifSkope. This renderer clamps the result to 0..1. Whether Skyrim
  rounds or truncates to a byte at intermediate key values, or clamps
  out-of-range keys, is unresolved.
  The lurker actor's short standalone curve even ends at -26. NifSkope's
  division supports the units, but does not establish Skyrim's conversion or
  clamping. No public source found here settles those behaviors.
- **Interpolation and clocks.** This uses the renderer's existing float-key
  interpolation and active / APP_INIT clock handling. Quadratic / TBC
  tangents, backwards playback, sequence transitions and exact initialization
  ordering retain the existing animation limitations. A valid constant
  `NiFloatInterpolator` pose is supported when key data is absent; unset
  poses and nonfinite samples leave the material unchanged.
- **Test functions.** The renderer's existing lower-bound cutout test remains
  in use; the other `NiAlphaProperty` comparison functions are a separate gap.
- **Particle materials.** Particle descriptions currently discard material
  animation, so this does not add alpha-test controller playback to particles.
