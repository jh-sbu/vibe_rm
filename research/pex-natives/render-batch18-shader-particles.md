# ShaderParticleGeometry.Apply and Remove

## Sources

- The Papyrus Index (papyrus.bellcube.dev): `Apply(afFadeInTime = 0)` and
  `Remove(afFadeOutTime = 0)` on a `ShaderParticleGeometry` (`SPGD`) apply and
  remove the particle geometry, fading over the time given. (The pages carry
  no further notes.)

## Game-asset observations

- `pex-natives` counts 118 / 84 calls to `Apply` / `Remove`, but those names are
  shared with `ImageSpaceModifier`; matching receivers' types
  (`vrm-tool pex-dump`), the vanilla SPGD callers are
  `ShaderParticleGeometryScript` (a magic effect: apply on start, remove on
  finish), `MG02QuestScript`, `MG06VisionTriggerScript` and
  `QF_MG04MonkSceneQuest_00076F39` (the College's visions, indoors).
- Skyrim.esm's SPGDs include weather precipitation and others meant for
  scripts (`SovngardeStardust`, `MAGBlizzardParticles`, `DragonRainParticles`).

## Choices

- One script geometry falls at a time, in a third precipitation layer drawn
  like a weather's (round the camera), outdoors and indoors. Indoors nothing
  shelters it (there is no occlusion map); outdoors the weather's occlusion
  map applies. A second `Apply` replaces the first.
- The fades are linear over the seconds given; 0 is at once. It isn't saved.
- Whether SE layers several applied geometries, or shelters indoor ones,
  is unknown.
