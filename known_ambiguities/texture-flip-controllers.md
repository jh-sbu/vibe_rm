# NiFlipController compatibility

Texture flip controllers remain deferred. Investigation exposed a bounded
rendering issue that could be resolved using public evidence: float / colour
`CONST_KEY` interpolation was discarded and linearly blended. Its correction
is described in [value-key-interpolation.md](value-key-interpolation.md).
This does not implement `NiFlipController` or establish its Skyrim support.

## Public evidence

The community [NifTools schema](https://github.com/niftools/nifxml/blob/develop/nif.xml#L3410-L3422)
describes `NiFlipController` as changing a `TexDesc` image using a float
interpolator and `NiSourceTexture` references. Its `TexType` slot enumeration
uses base = 0 and glow = 4; those are legacy texturing-property slots, not
Skyrim shader texture-set indices. The `Delta` / accumulated-time fields only
exist through version 10.1.0.103, before the versions this NIF reader targets.
Adding a delta-based payload decoder for Skyrim would therefore be incorrect.

[OpenMW's flip controller](https://github.com/OpenMW/openmw/blob/master/components/nifosg/controller.cpp#L498-L532)
shows public support for legacy delta-driven and interpolator-driven flips.
Its interpolator constructor always targets diffuse, while its alternate
constructor accepts an explicit texture unit. This does not resolve target
semantics for Skyrim's `BSLightingShaderProperty` / `BSEffectShaderProperty`.

## Unresolved scope

- This renderer handles Skyrim shader properties, not the legacy
  `NiTexturingProperty` / `TexDesc` pipeline. Public sources inspected here
  do not establish how, or whether, Skyrim applies `NiFlipController` to its
  shader properties. No mapping from `TexType` to shader slots is guessed.
- A representative asset with a supported Skyrim shader target is needed
  before choosing runtime attachment and frame binding behavior. No game
  assets were inspected for this investigation, so it establishes neither
  presence nor absence of vanilla uses.
- Frame-index rounding, negative / out-of-range index handling, missing source
  textures, manager ownership and transition behavior need evidence before
  a compatibility implementation can claim original-game behavior.

Only public community source was examined. No engine executable / library
binary data or leaked documents were used, and no game assets are included.
