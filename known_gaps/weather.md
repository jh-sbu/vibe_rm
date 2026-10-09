# Weather changes, regions' weathers and scripts setting them

Implemented in `src/weather.rs` (the weather over time) on top of
`src/world/weather.rs` (WTHR / CLMT records, the sky at an hour, two skies
mixed). Sources: CommonLibSSE's `Sky.h` / `Sky.cpp` (`currentWeather`,
`lastWeather`, `overrideWeather`, `currentWeatherPct`, the sky modes and
`IsRaining` / `IsSnowing` in full), `TESWeather.h` (the DATA layout and flags,
IMSP), `TESRegionDataWeather.h` / `WeatherType.h` (weather, chance, global);
the Creation Kit wiki's `Weather` script page; the game's data (`vrm-tool
esp-dump` of regions and weathers, `vrm-tool pex-calls` of the scripts using
`SetActive` (87 calls), `ForceActive` (18), `ReleaseOverride`, `FindWeather`).

## Settled

- **Two weathers and a transition.** The incoming weather, the outgoing one
  and how far the change has come (0..1). Sky colours, fog, light,
  directional ambient and the image space mix by it; the outgoing weather's
  cloud layers fade out under the incoming one's.
- **Precipitation** (`IsRaining`, `IsSnowing`) follows `Sky::IsRaining`: the
  incoming weather's once the transition passes its DATA begin fade in
  (byte 6, /255), the outgoing one's until it passes its end fade out
  (byte 7, /255, + 0.001).
- **Classification** is the first of the DATA flags pleasant, cloudy, rainy,
  snow (byte 11): `GetClassification` 0..3, -1 without one.
- **Regions.** A cell lists its regions (XCLR); weather data is RDAT type 3
  (byte 5 the priority) followed by RDWT entries of (weather, chance,
  global). Riverwood is in `WeatherPineForest`: cloudy 35, clear 35, fog 10,
  overcast rain 5, storm 5, two `_A` variants 5 each.
- **Override.** `ForceActive` / `SetActive` with `abOverride` hold the
  weather against regional changes until `ReleaseOverride`; `SetActive`
  without it does nothing while overridden.

## Open

- **Transition speed.** The DATA "trans delta" (byte 3, 125 in most
  weathers) is taken as thousandths of the transition per game minute (8 game
  minutes at 125, 24 seconds at timescale 20), accelerated by
  `fWeatherTransAccel` (16, from the data). A trans delta of 0 changes at
  once. The real units aren't public.
- **How often the weather changes.** Every 2 to 6 game hours (made up) the
  region's list is rolled by chance; the same weather may come up again. The
  climate's TNAM volatility (byte 4: 50 for `SkyrimClimate`) isn't used.
- **Global chances.** An RDWT entry with a global takes its chance from the
  global's value; whether the game adds it to the chance instead isn't known.
- **Region priority and the override flag.** The highest priority region with
  weather data wins; RDAT's override flag (byte 4) isn't read. Without
  regional weathers the climate's WLST is used.
- **Leaving a region** whose list lacks the current weather brings one of the
  new region's in by the usual transition (not accelerated).
- **Indoors** the weather keeps its state and its rolls but shows nothing; a
  game started indoors has no weather until it goes out
  (`GetCurrentWeather` returns None). Sky mode is 1 indoors, 3 outdoors; no
  worldspace is treated as sky dome only.
- **Weather sounds.** SNAM entries are (sound, type): default (29 in the
  vanilla weathers), precipitation (9), wind (4), thunder (7). Outdoors,
  every sound but thunder loops at its weather's share of the transition
  (the rain loop's own condition is `IsInInterior == 0`; descriptors'
  conditions aren't evaluated). Thunder plays one of the weather's thunder
  sounds, not positioned, while it's on: the incoming weather's once the
  transition passes the DATA lightning begin fade in (byte 8), the outgoing
  one's until its end fade out (byte 9). Its frequency (byte 10) is taken as
  the time between rolls, 5 seconds at 0 to 30 at 255, each between half and
  one and a half times that (made up: the storms have 246, Storm Call's
  `FXMagicStormRain` 15, so lower is taken as more often). The first roll
  after thunder comes on waits.
- **Precipitation.** A weather's MNAM names its shader particle geometry
  (SPGD). DATA is read in UESP's layout: gravity velocity, rotation
  velocity, particle size X / Y, center offset min / max, initial rotation
  range, subtextures X / Y, type (0 rain, 1 snow), box size, density
  (`DustParticles` and `FogParticles` stop at 40 bytes and aren't drawn).
  What's settled is only the layout; how the game uses the values isn't
  public:
  - Gravity velocity is taken as units a second (rain's 674 is about the
    terminal velocity of a drop).
  - Snow's rotation velocity (100) is taken as degrees a second each flake
    circles its falling centre, at a radius between the center offsets
    (50..185), starting at an angle in the initial rotation range (360).
    Whether it instead spins the sprite isn't known.
  - Particle size is multiplied by 10 into units (made up: rain 3.5 x 20, a
    streak about the length a drop falls in a frame; snow flakes 11.5).
  - Density is multiplied by 2500 into a particle count (made up), capped
    at 32768.
  - Rain's texture (`FXRaindrops.dds`) is dark and faint (alpha under 0.4
    in streaks a texel or two wide). Drawn like snow it couldn't be seen,
    so it is drawn in the weather's effect lighting colour, its alpha
    doubled and its mip 0 sampled. Snow is its texture times the effect
    lighting. Both are alpha blended and fogged.
  - The amount falling follows the precipitation fades: the incoming
    weather's rises from its begin fade in to the end of the transition,
    the outgoing one's falls to nothing at its end fade out. Any weather
    with an MNAM draws it, whatever its flags (`SovngardeClear`'s stardust).
  - Particles hang in a box the SPGD's size about the camera, faded out
    towards its edges. Nothing keeps rain off under roofs or overhangs.
- **Lightning.** Each thunder roll flashes at once in the weather's DATA
  lightning colour (bytes 12..14, UESP; the storms' 219, 220, 238), at its
  share of the transition: a flicker, a dim gap and a fading second stroke
  over 0.6 seconds (made up), adding to the sky colours, the clouds, the
  ambient and directional ambient and the precipitation's light. The flash
  and the sound come together (no delay for the strike's distance); no bolt
  is drawn. Thunder now rolls without an audio device as well.
- **Cloud speeds.** QNAM (x) and RNAM (y) give each of the 32 layers a byte,
  read as xEdit does: (value - 127) / 1270, so 127 is still and the range
  -0.1..0.1. Each layer drifts at its own speed times 0.1 texture widths a
  second, in real time (the scale is made up; the storm's fastest layer,
  178, drifts across a texture in about four minutes). The layers' texture
  scales over the sky (1, 0.8, 1.3, 0.6) are this engine's own.
- **Sky statics.** A weather's TNAM entries are statics (STAT), 48 to 144
  per weather: the big cloud meshes (`Sky\CloudDistant01.nif`,
  `Sky\CloudShape06_O.nif`...) placed about the worldspaces, 1060 references
  of 161 bases in the vanilla data (`vrm-tool weathers`, `vrm-tool
  model-users <data> "sky\"`). A reference whose base any weather lists is
  shown only while a weather listing it is on, at that weather's share of
  the transition, drawn in the weather's sky statics colour (NAM0 colour 13:
  bright by day, grey in storms, dark blue at night). That they are hidden
  in the other weathers rather than always drawn, and that the colour
  multiplies them, is inferred from the field's name and values; no public
  source describes it. The references are persistent and flagged 0x10000
  (full LOD?) but load with their cells like any other here, so the far ones
  don't show.
- **Wind.** DATA bytes 17 and 18 are the wind direction and its range
  (CommonLibSSE's `TESWeather::Data`; UESP's layout puts them at 16 and 17).
  They're read as 256ths of 360 and 180 degrees (`GetWindDirection` gives
  0..360). Every vanilla weather has 43 and 43 (60 and 30 degrees) or 0 and
  0. The direction is taken as a heading (clockwise from north, +y) the wind
  blows toward; whether it's where the wind comes from isn't known. It sways
  across its range over 30 seconds (made up). Rain and snow drift with it at
  the wind speed (0..1) times 1000 units a second (made up: a storm's 0.2
  slants falling rain by about 17 degrees), the rain's streaks slanted along
  their fall. Nothing else feels the wind yet (trees, grass, clouds).
- **Not drawn:** auroras, the weather's volumetric lighting, sun glare and
  damage.
- **Not in saves**: the weather state lives only in memory.
