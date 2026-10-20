# Float and colour constant interpolation

`NiFloatData` and `NiPosData` now preserve their NIF `KeyType`. Shader float,
colour and alpha-test channels use that metadata in both standalone and named
sequence playback. Particle float-controller curves also preserve it when
converting their keys. Linear keys retain their previous interpolation.

## Public evidence and policy

- The community [NifTools schema](https://github.com/niftools/nifxml/blob/develop/nif.xml#L376-L383)
  identifies type 5 (`CONST_KEY`) as a step function. A nonempty `KeyGroup`
  carries an interpolation type; an empty group carries only its count.
- [NifSkope's float/vector evaluator](https://github.com/niftools/nifskope/blob/develop/src/gl/glcontroller.cpp#L347-L395)
  chooses the first endpoint below the segment midpoint and the second at or
  above it for type 5. [OpenMW's value evaluator](https://github.com/OpenMW/openmw/blob/master/components/nifosg/controller.hpp#L123-L150)
  likewise selects the nearest endpoint, but chooses the first at the exact
  midpoint. These are public community implementations, not evidence of the
  original Skyrim engine's precise tie behavior.
- The runtime follows NifSkope's midpoint tie policy. Thus values at times 2
  and 6 switch at time 4, selecting the time-6 value at that instant. Exact
  key times select their own value; samples before / after the track hold
  the first / last value.
- [NifSkope's boolean evaluator](https://github.com/niftools/nifskope/blob/develop/src/gl/glcontroller.cpp#L419-L433)
  uses the preceding key instead. Node visibility and particle boolean curves
  retain their existing hold-until-next-key behavior; float constant keys
  must not be converted to that boolean mode.

## Remaining ambiguities and limits

- Public implementations disagree at exact midpoints. Skyrim's tie policy
  and behavior for repeated timestamps remain unverified. Existing duplicate
  timestamp selection is retained; no new engine-parity claim is made.
- Quadratic value curves now preserve and evaluate their tangents; see
  [quadratic-value-interpolation.md](quadratic-value-interpolation.md). TBC
  value curves derive Hermite tangents; source-order disagreement and limits:
  [tbc-value-interpolation.md](tbc-value-interpolation.md). Transform key groups
  still discard interpolation metadata and are outside these changes.
- This corrects sampling for material channels that already reach the
  renderer. It does not extend actor / effect sequence ownership, particle
  material animation, initialization ordering or blending.

Synthetic tests cover scalar and vector parsing, empty groups without a type
field, single-key tracks, endpoints, either side of midpoints, the chosen tie,
unchanged linear sampling, nonlinear payload consumption, shader UV / colour /
cutout uniforms, named sequence clocks, and particle float versus boolean
curves. Only public sources and synthetic data were used; no game assets were
added and no engine executable or library binary data was examined.
