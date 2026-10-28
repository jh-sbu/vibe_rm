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
- **Dialogue mode.** DOBJ names the two dialogue output models; the actual
  DOP2 model is 3D, DOP3 2D (opposite xEdit's labels). We choose DOP3 for the
  player's conversation, DOP2 for barks and scenes. Data names the models but
  does not specify the conversation-mode switch; listening remains useful.
  Descriptor-free player/menu sounds fall back to POPM/P3OM/IOPM. The generic
  point playback path recognizes the player at the player's position; callers
  with an explicit descriptor model always keep it.
- **Speaker scale and layouts.** The vanilla ONAM survey contains only
  0/50/75/100, so levels are interpreted as percent amplitude. Mono sources
  use the mono row; stereo sources use the two independent channel rows.
  Original channels are retained from RIFF fmt data, including xWMA; unknown
  non-RIFF formats default to stereo. cpal supplies a channel count, not
  speaker labels: we assume L R C LFE RL RR BL BR for eight outputs, standard
  quad/5.1 subsets otherwise, and use -3 dB center/surround downmix with LFE
  dropped on stereo/mono. This layout assumption is not established by assets.
- Rumble remains unused. Reverb send now reaches the network described in
  `reverb.md`.
