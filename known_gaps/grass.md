# Grass

Implemented in `src/world/grass.rs` (records, scattering) and the renderer's
grass batches (`GrassBatch`, `vs_grass` in `object.wgsl`). Sources: the
GRAS and LTEX record layouts (UESP; `vrm-tool grass <data>` prints every
grass's `DATA` and how many landscape textures list it), the Creation Kit
wiki's field names, the shipped `High.ini` / `Ultra.ini`
(`fGrassStartFadeDistance=7000`) and the vanilla grass meshes (`vrm-tool
nif-verts`). The game's own scattering isn't public.

## Settled

- **`DATA`**: density, min / max slope (degrees), units from water and its
  type, position / height / colour range, wave period, flags (vertex
  lighting, uniform scaling, fit to slope). Every vanilla grass has uniform
  scaling and fit to slope.
- **Vertex alpha** of grass meshes is 0 at the base and rises up the blade:
  how far each vertex sways, not opacity.

## Open (choices made here)

- **Grid.** Candidates every 20 units (`iMinGrassSize`, from community INI
  guides; the shipped INIs don't set it), each landscape texture's grasses
  tried independently at the candidate with their density's chance, so two
  grasses of one texture can share a point. Which texture picks: the one
  showing most at the nearest vertex (`Land::texture_at`).
- **Ranges.** Position range is a square offset of up to that many units;
  height range scales by 1 ± range; colour range darkens the landscape's
  vertex colour by up to that fraction.
- **Wave period** is read as the length of the waves (units) running over
  the grass; how fast they run and how far blades bend (10 units at full
  vertex alpha, more in strong wind) are this engine's.
- **Fade**: from 7000 units over another 1000 (no published fade range), by
  alpha test.
- **Not done**: vertex lighting flag, grass shadows, the grass cache
  (`bAllowLoadGrass`), grass under objects (vanilla doesn't cull it either
  except where the landscape texture lacks grass), interaction with actors.
