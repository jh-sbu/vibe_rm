# Image space modifiers, screen fades and camera shakes

Implemented in `src/imagespace.rs` (IMAD records, modifiers on the view,
`Game.FadeOutGame`, `Game.ShakeCamera`) and `src/render/post.rs` /
`shaders/post.wgsl` (the pass drawing the frame through the effects).
Sources: CommonLibSSE's `TESImageSpaceModifier.h` (`ImageSpaceModifierData`,
the DNAM layout, and which `*IAD` subrecord drives which value) and
`ImageSpaceData.h`; the Creation Kit wiki pages for `ImageSpaceModifier`
(`Apply`, `ApplyCrossFade`, `PopTo`, `Remove`, `RemoveCrossFade`),
`Game.FadeOutGame` and `Game.ShakeCamera`; the game's data (`vrm-tool
esp-dump` of IMADs, `vrm-tool pex-calls` / `pex-dump` of the scripts using them).

## Settled from the data

- **Curves.** The multiplier of value `i` is subrecord `<i>IAD` (byte `i`,
  then `IAD`), its addend `<0x40 + i>IAD`; keys are (time, value) with time
  0..1 over the duration. Saturation is 17, brightness 18, contrast 19
  (`defaultDesaturateImod`: `\x11IAD` 0.7, `\x12IAD` 0.6, `\x13IAD` 0.9).
  Colour curves (`TNAM` tint, `NAM3` fade) are (time, r, g, b, amount).
- **Static modifiers hold their first keys.** Non-animatable modifiers
  (DNAM byte 0 clear) have the effect at time 0 and neutral values at time 1
  (`ISMDinCloudBlurStatic`: blur 0.5 then 0), so they show their first keys
  and stay until removed. `FadeToBlackHoldImod` (static, black throughout)
  has to outlast its 3 second duration: the carriage script holds it across a
  fast travel before `PopTo`ing `FadeToBlackBackImod`.
- **Where the base image space comes from.** Interiors name theirs with
  XCIM (728 of 761 do); the rest get `DefaultImageSpaceInterior` (0x160).
  Every weather has IMSP: four image spaces for sunrise, day, sunset and night
  (CommonLibSSE's `TESWeather::imageSpaces[ColorTime::kTotal]`), blended over
  the day with the same weights as its colours. Worldspaces have none
  (`vrm-tool image-spaces` surveys the records and their users).
- **Modifiers work on the base.** Each modifier value is the base's times the
  multiplier plus the addend (`defaultDesaturateImod` outdoors at noon:
  saturation 1.6 x 0.7).
- **Animatable modifiers end.** They play their curves over the duration and
  come off at its end (`FadeToBlackImod` is black from 2/3 of its 3 seconds;
  the carriage script `PopTo`s the hold modifier at 2).

## Open

- **No HDR.** The base image space's cinematic values (CNAM) and tint (TNAM)
  are drawn, but the frame is rendered straight to display range: there is no
  eye adaptation, bloom or tone mapping, so the HDR values (HNAM: eye adapt
  speed and strength, bloom radius / threshold / scale, receive bloom
  threshold, white, sunlight scale, sky scale) and modifiers driving them do
  nothing. Some HNAM values can't be plain multipliers (sky scale runs
  -0.15 .. 1, 0.05 at clear noon), so their meaning needs a source too. The
  depth of field values (DNAM) aren't drawn either.
- **How the effects are drawn.** Saturation by luminance (Rec. 709 weights),
  tint as a blend toward luminance times the colour, brightness as a
  multiplier, contrast about the frame's average luminance (a mip chain of
  the frame, no adaptation over time), then the fade colour, all on display
  values. The game's own shader math isn't public. The game works on its
  tone mapped HDR frame: contrast about mid grey on ours crushed the shadows
  (clear noon has contrast 1.4, cloudy 1.5), as the lighting here was tuned
  without an image space; about the average it keeps the frame's exposure.
- **Blended tints** mix as colour times amount, so an image space without a
  tint fades another's out across a time-of-day blend.
- **Blur and double vision units.** Blur is taken as a radius in pixels at 720
  lines (a 13-tap disc), double vision as 8 such pixels per unit of strength.
  Both are guesses: `ISMDinCloudBlurStatic`'s blur 0.5 hardly shows.
- **Not drawn:** radial blur, depth of field, motion blur.
- **Several modifiers at once** multiply their cinematic values in turn, lay
  their tints and fades over each other in the order applied, and take the
  largest blur. `Apply` on a modifier already on restarts it.
- **Strength.** `Apply`'s strength scales each multiplier's distance from 1,
  each addend, and the tint and fade amounts.
- **Cross-fades.** Only one cross-fade modifier: `ApplyCrossFade` fades the
  previous one out over the same time as the new one fades in.
- **Modifiers aren't tied to a place or actor** (`TriggerIfNotActive`'s
  target), and spells and magic effects don't apply them yet (no magic).
- **Camera shakes.** Without a duration a shake lasts 1 second (the game's
  default isn't in its data); one from a source fades out with the player's
  distance from it, to nothing at 4096 units (made up). The shake is up to
  about 3 degrees of turn at full strength, dying down linearly. Controller
  rumble (`ShakeController`) is not done.
- **Fades and menus.** Modifiers, fades and shakes stop with the world in menu
  mode. Whether the game's fades (a menu of their own) go on in menus isn't
  known. The HUD and menus draw over the effects.
