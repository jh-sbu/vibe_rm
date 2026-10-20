# Sound output models' attenuation curves

Implemented in `src/world/sound.rs` (`parse_output_model`, `voice_falloff`)
and the mixer's `Falloff` (`src/audio/mod.rs`). Source: UESP's [SOPM
page](https://en.uesp.net/w/index.php?oldid=3089107): `NAM1` flags (bit 1
"Attenuates With Distance", bit 2 "Allows Rumble") and reverb send percent,
`MNAM` type (0 HRTF, 1 defined speaker output), `ANAM` (four unknown bytes,
min and max distance, `uint8[5]` "Curve"); and the base game's 86 output
models (80 have an `ANAM`).

## Settled from the data

- **The curve's points.** Of the 80 with a curve, 42 hold `100, 50, 20, 5, 0`; 20 are
  `100, 75, 50, 25, 0` (the "Linear" and dragon ones), a few in
  between (`100, 60, 30, 10, 0`, `100, 85, 65, 40, 0`...), and a handful of
  odd ones (`0, 50, 100, 50, 0` for the "distant" sounds, `100, 63, 79, 64,
  100` for `SOMStereoRad12000MS06`). `100, 50, 20, 5, 0` is about the square
  of the remaining distance at 0, 25, 50, 75 and 100% of the way from the
  minimum to the maximum distance (100, 56, 25, 6, 0), which is what the
  mixer used before: the five points are taken at those fractions.
- **2D models.** `SOMDialogue2D`, `SOMDialogue2D_verb`, `SOMStereo`,
  `SOMStereo_verb`, `SOMUIDefault` and `SOMMono01400Player1st` have no `ANAM`
  and no `NAM1` distance flag: sounds using them aren't positioned.

## Open (choices made here)

- **The curve is a gain.** The points are percentages of full volume,
  linearly interpolated between them (the CK draws a five-point curve; the
  interpolation isn't documented), and within the minimum distance a sound
  has the first point's gain (the `0, 50, 100, 50, 0` models are silent up
  close).
- **Voices** have no sound descriptor, so no output model of their own: they
  take `SOMDialogue3DDefault`'s distances and curve (70 to 3000 units,
  `100, 50, 20, 5, 0`), replacing a hard-coded 400 to 3000 that made speech
  near the speaker quieter. Which model the engine uses for them isn't known.
- **Not used.** The type (HRTF or speaker output: `Stereo Rad` models'
  stereo radius), the reverb send, rumble and the three channels of speaker
  output values (`ONAM`): the mixer pans mono by the listener's right vector
  as before. Descriptors without an output model keep a made-up 200 to 2000
  with the default curve.
