# Quadratic float and colour value curves

`NiFloatData` and `NiPosData` now retain their quadratic tangent payloads and
evaluate a cubic Hermite segment instead of a linear approximation. Existing
standalone and named-sequence shader / alpha-test channels receive these
values. Particle float-controller conversion retains the same evaluator;
boolean curves continue to hold their preceding key.

## Public evidence

- The community [NifTools Key schema](https://github.com/niftools/nifxml/blob/develop/nif.xml#L1859-L1871)
  places same-type forward then backward tangent values after each quadratic
  key. This means vector keys carry vector tangents, rather than scalar slopes.
- [OpenMW's quadratic key reader](https://github.com/OpenMW/openmw/blob/master/components/nif/nifkey.hpp#L122-L138)
  reads the first tangent as incoming and the second as outgoing. Its
  [value evaluator](https://github.com/OpenMW/openmw/blob/master/components/nifosg/controller.hpp#L123-L150)
  combines the lower key's outgoing tangent with the upper key's incoming
  tangent, component by component, using normalized segment time. Tangents
  enter the Hermite basis directly, without multiplication by elapsed seconds.
- [NifSkope's evaluator](https://github.com/niftools/nifskope/blob/develop/src/gl/glcontroller.cpp#L361-L380)
  independently uses the lower backward and upper forward tangent in the same
  Hermite basis without duration scaling. Its generic implementation fetches
  tangents as scalars, so vector handling here follows OpenMW's typed reader
  and evaluator and the schema.

These public community implementations support the chosen behavior; they do
not establish original Skyrim SE parity. The local UESP index had no
`NiFlipController` result. No engine executable or library data, leaked
documents, or game assets were examined or added for this work.

## Limits and unresolved behavior

- TBC value curves now derive Hermite tangents; see
  [tbc-value-interpolation.md](tbc-value-interpolation.md) for source-order
  disagreement and limits. Transform key groups still discard interpolation metadata;
  their quadratic rotation / translation / scale evaluation is separate work.
- Endpoint holds and repeated-timestamp selection retain existing policy;
  Skyrim's duplicate / unsorted key handling remains unverified. A NaN sample
  time returns no value, allowing material channels to retain their base value.
- Synthetic callers may construct a quadratic curve without a complete tangent
  array; those curves retain the linear fallback. Parsed quadratic tracks always
  have one tangent pair per key or fail parsing.
- This does not resolve [texture-flip-controllers.md](texture-flip-controllers.md),
  sequence ownership / blending, controller clock initialization or persistence.

Synthetic tests cover scalar / vector payload consumption, asymmetric nonzero
tangents, four-second spacing (to catch duration scaling), zero-tangent easing,
endpoint holds, backward seeks and NaN time. Renderer tests check standalone
and named-sequence UV, colour and alpha-test uniforms, and imported particle
float curves while retaining boolean steps. No GPU or installed game is needed.
