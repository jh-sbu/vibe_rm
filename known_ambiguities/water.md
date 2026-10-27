# Water contact and dangerous water

Implemented in `src/water.rs`: loaded water surfaces retain the resolved WATR
form used by the renderer. Condition 332 (`IsInDangerousWater`) tests actors'
contact with a surface whose WATR FNAM has bit 0 set. The console's
`[ref.]isindangerouswater` exposes the same query.

## Public and asset evidence

- [xEdit's Skyrim record definitions](https://github.com/TES5Edit/TES5Edit/blob/dev-4.1.5/Core/wbDefinitionsTES5.pas)
  name FNAM bit 0 "Causes Damage", XNAM "Spell", CELL XCWT "Water", and
  WRLD NAM2 "Water". Importantly, WATR DATA is **unused in SSE**; the
  "Damage Per Second" label is for the older format. Do not introduce
  automatic SSE water damage based on that older label.
- [A mod author's explanation on Water Douses Fire](https://www.nexusmods.com/skyrimspecialedition/mods/34344?tab=posts)
  describes using dangerous-water flags to detect water even when too
  shallow to swim. The [Realistic Needs and Diseases author's troubleshooting](https://www.nexusmods.com/skyrim/mods/26228?tab=posts)
  likewise says standing in patched water must make the condition true.
  These establish that swimming is not required, but not the exact depth.
- Installed Dragonborn.esm, inspected only as plugin assets with
  `vrm-tool esp-dump`: DLC2Book01Dungeon is an interior with water at height
  0 and XCWT pointing to DLC2ApocryphaWater; that water has FNAM 1,
  DATA 0 and no XNAM. DLC2abApoWaterDamage has alternatives conditioned
  on IsSwimming (185) and IsInDangerousWater (332).
  DLC2abApoWaterDamageActual has a 70-magnitude effect conditioned on
  swimming and a 25-magnitude effect conditioned on dangerous water.
  The asset conditions should decide which effects apply; the flag alone
  is not an instruction to inflict a hardcoded damage amount.

## Unresolved choices and limitations

- Contact uses the actor's feet strictly below the cell water surface.
  The exact immersion threshold, collision-shape/race/scale dependence,
  equality behavior and any hysteresis are not established by these sources.
  Swimming now has a separate state and condition 185 reads it.
- Queries use loaded cell surfaces. Unloaded actors return false; processing
  tier behavior and updates without attached 3D need observable game tests.
  Objects that are not actors return false. Deceased actors are not explicitly
  excluded; the original condition's behavior on them is unresolved.
- Surfaces have the same horizontal bounds as the loader's rendered planes
  (interiors use its large square). Exterior edges belong to the cell to
  the north/east, matching grid lookup. A loaded actor crossing an edge uses
  its position's cell, even before its runtime ownership changes. Geometry
  masking water or water meshes separate from cell planes is not considered.
- Overlapping planes select the highest surface, as the previous water-height
  query did. Authored overlapping water volumes and equal-height precedence
  need evidence before extending that approximation.
- XNAM contact-spell application, removal, refresh timing and caster identity
  remain unimplemented. No vanilla contact spell was identified in the earlier
  WATR survey. These need public evidence or a purpose-built plugin observed
  in Skyrim SE. No engine binaries or SDK source mirrors were inspected.

Synthetic tests cover adjacent cells with different types/heights (including
negative coordinates), overlap selection, and shallow/deep/dry feet. They
validate the chosen geometric rules, not the original engine's threshold.
A synthetic plugin tests dangerous and ordinary flags, unrelated flag bits,
wet/dry positions, absent/empty flag fields, and invalid water forms without
an installed game. Earlier installed-asset checks passed with Dragonborn
loaded; the automated test now checks the query independently of those assets.

Hardware-rendered checks (2026-10-15) passed outside the sandbox using
Vulkan on AMD Radeon Graphics (RADV RENOIR), Mesa 26.2.3:

- DLC2Book01Dungeon (water height 0): player feet at -1 returned 1, feet at
  0 and +1 returned 0. Console placement adds two units of clearance, so
  the corresponding `player.setpos` Z arguments were -3, -2 and -1.
- Riverwood: below the ordinary cell water returned 0.
- Both runs loaded their cells, rendered and wrote screenshots under `/tmp`;
  no assets or captures were added to the repository.

The earlier sandboxed attempt could only access software OpenGL, whose
16-fragment-texture limit is insufficient for the terrain pipeline's 17.
These hardware checks validate this engine's integration; they do not
confirm the original game's immersion threshold.

## Swimming depth (autotest20 deferred)

Autotest19 observed feet on the bed at depth 80 and floating at depths
100–140, about 92 units below the surface. Its HasMagicEffect swim detector
was inconclusive. Chosen entry depth is strictly over 100, with four units
of exit hysteresis measured against the bed, so floating at the observed
92 does not toggle the state. A swimmer lifted clear of that float band
also leaves the state. The bed comes from static-world ground rays.
Scale/race-dependent depth and Waterbreathing's influence on state remain
unmeasured; use the same threshold for now. Autotest20 is deferred by the user.
