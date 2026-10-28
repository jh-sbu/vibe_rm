# Sound descriptors' frequency and dB variance

Implemented in `src/world/sound.rs` (`SoundDesc::variation`), the mixer's
per-voice rate (`Audio::play_pitched`) and the engine's playing of
descriptors (`Engine::play_sound`, looping sounds of references, thunder,
weather loops). Source: UESP's [SNDR
page](https://en.uesp.net/w/index.php?oldid=2309034): `BNAM` is int8 "%
Frequency Shift", uint8 "% Frequency Variance", uint8 priority, uint8 "dB
Variance", uint16 static attenuation (100 times the dB). The data
(`vrm-tool sndr-stats <data>`): of 3186 base game descriptors 96 have a
nonzero shift (-20% to 50%), 1656 a frequency variance (up to 54%) and 1335 a
dB variance (1 to 16, mostly 2 to 6: footsteps, impacts, voices' effects).

## Open (choices made here)

- **Units.** The shift and variance are percent of the playback rate (a
  rate of 1 + percent / 100), the dB variance decibels of gain. The page
  names the units; that the rate rather than the pitch in semitones moves
  by the percent isn't stated.
- **Spread.** Each playing draws two numbers: the rate is 1 plus the shift
  plus a uniform draw between minus and plus the variance; the gain is a
  uniform draw between minus and plus the dB variance. Whether the game's
  variances are symmetric, or one-sided (variation only ever quieter, as
  attenuation), or drawn some other way isn't public.
- **Where.** One-shots (`play_sound`: footsteps, impacts, the sounds that
  scripts and events play), thunder, and the loops of references (a fixed
  draw per reference) and of the weather (a rate drawn when the loop
  starts; the weather's own volume follows the transition, so no gain
  variance there). The mixer resamples without interpolation, so a rate
  well off 1 is rougher than the game's.
- **Not used.** The priority (`BNAM` byte 2: which voices to drop when the
  game runs out of them) and the rumble send (`LNAM`).

## Mixer smoothing (audio plan 0a)

The data and public SNDR/SNCT documentation specify gains, not their sample-level
interpolation. Gain changes use linear amplitude ramps; ordinary volume and stop
changes take 5 ms. Category pause holds the playback cursor only after fading
to zero. This is a click suppression choice, not measured Skyrim behavior.
