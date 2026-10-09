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
- **Not drawn or heard:** rain and snow (precipitation particles, MNAM),
  lightning and thunder, weather sounds (SNAM: precipitation, wind,
  thunder), auroras, sky statics, the weather's volumetric lighting, sun
  glare and damage, cloud speeds (RNAM / QNAM) and wind direction.
- **Not in saves**: the weather state lives only in memory.
