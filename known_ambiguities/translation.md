# Translations (`TranslateTo` and its kin)

Implemented in `src/translation.rs`. Sources: the Papyrus index
(papyrus.bellcube.dev, the `TranslateTo`, `SplineTranslateTo`,
`StopTranslation` and `OnTranslationAlmostComplete` pages, quoting the CK
wiki): speed in units a second, rotation speed in degrees a second;
`afMaxRotationSpeed` 0 means "the object will reach the target rotation when
it reaches the target position", and no cap can make it turn faster than
that; `OnTranslationComplete` when both position and rotation are reached;
`OnTranslationAlmostComplete` at a game setting's share, "default is 90% of
the way" (Skyrim.esm overrides no setting of that name).

Vanilla callers (`vrm-tool pex-calls`): `TranslateTo` 85, mostly puzzles,
the Civil War catapults, rising water and critters; `SplineTranslateTo` 18,
all critters.

## Choices made without a source

- **Who can move**: any reference in a loaded cell, drawn or not (markers,
  water), and loaded actors. One that isn't gets `OnTranslationFailed` at
  once, and one whose cell unloads on the way gets it then. When the game
  sends `OnTranslationFailed` isn't described.
- **`StopTranslation`** leaves the reference where it is and sends no event.
- **The rotation** turns each angle the short way round, at the share of the
  way gone; a cap holds each axis to that many degrees a second.
- **The spline** (`afTangentMagnitude`, "magnitude of the spline tangents")
  is a cubic Hermite curve whose tangents are the start's and the end's
  facings (+Y turned by their angles) times the magnitude. The speed is kept
  along the curve's length, measured in 32 steps.
- **Not saved**: a save or load mid-way leaves the reference where it was,
  without the events.
- `SplineTranslateToRefNode` (no vanilla callers) isn't implemented.
