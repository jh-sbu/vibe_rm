# TBC float and colour value curves

`NiFloatData` and `NiPosData` derive normalized Hermite tangents from their
serialized tension, bias and continuity parameters. Shader float, colour and
alpha-test channels already using `ValueKeys::sample` receive these values;
particle float-controller conversion now retains them too. Boolean curves
continue holding their preceding key.

## Public evidence

- The local UESP index returned no `NiFloatData` page. The public community
  [NifTools schema](https://github.com/niftools/nifxml/blob/develop/nif.xml#L1853-L1871)
  defines type-3 parameters in **tension, bias, continuity** order, after each
  scalar/vector key. The decoder follows this explicit schema order.
- [OpenMW's tangent generator](https://github.com/OpenMW/openmw/blob/master/components/nif/nifkey.hpp#L141-L187)
  supplies named-parameter coefficient math, one-sided endpoint tangents and
  interior time weights. Its incoming tangent uses the previous segment's
  fraction of the full neighbouring interval, and its outgoing tangent uses
  the following segment's fraction. Neutral parameters therefore preserve a
  linear ramp even with unequal segment durations. Tangents are generated at
  decode time and stored alongside quadratic tangents; raw parameters are
  not retained after decoding.
- [OpenMW's value evaluator](https://github.com/OpenMW/openmw/blob/master/components/nifosg/controller.hpp#L123-L150)
  uses those tangents in the same normalized cubic Hermite basis as quadratic
  keys. No further multiplication by segment duration is applied.

Only public community source and synthetic fixtures were used. No game assets,
engine executable/library binary data or leaked documents were examined or
added. This is not a claim of measured Skyrim SE runtime equivalence.

## Unresolved source disagreement and scope

- **Bias/continuity order remains disputed.** OpenMW's reader consumes tension,
  continuity, bias, whereas NifTools explicitly labels tension, bias, continuity.
  The names are not merely aliases: the two values enter different coefficients
  in OpenMW's tangent generator. Here schema field order and the generator's
  named-parameter math are combined. This is an explicit compatibility policy;
  it requires an original-game observation or stronger public evidence to
  establish Skyrim's interpretation when bias and continuity differ.
- [NifSkope's public float/vector evaluator](https://github.com/niftools/nifskope/blob/develop/src/gl/glcontroller.cpp#L347-L395)
  has no type-3 branch and falls back to linear interpolation, so it does not
  resolve that disagreement. Public schema blame/history endpoints could not
  be retrieved during this investigation; no historical-order conclusion is
  drawn from that failure.
- Sequence ownership/blending and particle material-animation routing are
  unchanged. This change only samples controller channels already supported.
- Endpoint holds, repeated timestamps and unsorted/nonfinite inputs retain
  existing policy. An interior zero neighbouring time span produces zero
  tangents, following OpenMW's generator. Nonfinite TBC parameters propagate
  through tangent generation; no malformed-input parity is established.
  Manually constructed tracks missing a complete tangent array retain the
  existing linear fallback; parsed tracks have complete arrays or fail parsing.

Synthetic tests cover scalar/vector decoder consumption and every truncated
payload, distinct bias/continuity, nonuniform time weights, one-sided endpoints,
neutral linear ramps, full-tension easing, empty/single-key tracks, zero time
spans, endpoint holds, backward seeks, NaN sample time and particle float
conversion while preserving boolean steps. No installed game or GPU is needed.
