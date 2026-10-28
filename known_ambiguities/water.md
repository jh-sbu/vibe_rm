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
unmeasured; use the same threshold for land-capable swimmers for now.
Autotest20 is deferred by the user.

The local UESP [RACE revision 3650080](https://en.uesp.net/w/index.php?oldid=3650080)
documents Swims (0x40) and Walks (0x100). Chosen scope guard: exclude only
races with Swims but without Walks from surface swimming. Swim-only races such
as SlaughterfishRace retain their underwater position instead of being raised
to the humanoid float height. Other races keep the existing depth-based state;
this is not evidence for the original game's aquatic movement controller.

Vanilla asset evidence supplied in review: HorseRace has flags 0x00508908
(Walks, without Swims), yet has Horse_Swim_MT (`MNAM` HorseSwim) and
`iState_HorseSwim` / `bInSwimState` in horsebehavior.hkx. Requiring Swims
therefore incorrectly prevented horses from entering their authored swim state.
Absence of Swims is not treated as proof that a race cannot surface-swim.
The precise meaning of that flag across races remains uncertain.

NPC swim locomotion selects graph-named swim MOVT states and uses water
navmesh paths. Swimmers float at the observed surface-minus-92 feet height;
land collision standing is suspended. No diving or race-specific float height
is inferred. Swimming fighters pursue without attempting attacks or drawing.
ActionDraw's vanilla tree, contrary to the planning sketch, has no general
IsSwimming condition: avoiding a draw while swimming is an engine policy here.
The humanoid swim blend actually binds `Direction`; `SwimDirection` is declared
but has no binding in the inspected graph. Both are supplied the motion fraction.

The humanoid graph declares no NPC swim iState constant (its HorseSwim
constant is a mounted subgraph's). Humanoids therefore use NPCSwimming's
speeds while retaining their default iState; creatures use their own authored
swim constants. This avoids accidentally selecting mounted swim speeds.

Player movement holds the same floating feet height using collision movement,
with no swim gravity or dive controls. The capsule's 64-unit half-extent and
56-unit eye offset leave the eye 28 above the surface at that height. Swim
speed uses NPCSwimming walk/run, scaled by the existing armor movement rule.
Diving controls await underwater rendering; breath still applies to actors
whose head is actually submerged (including externally placed actors).

## Breath and drowning

The local UESP [Waterbreathing revision 3380569](https://en.uesp.net/w/index.php?oldid=3380569)
and [Argonian revision 3571436](https://en.uesp.net/w/index.php?oldid=3571436)
establish that Waterbreathing prevents drowning, including Argonians' ability.
Neither gives an unprotected breath timer. Public mod authors disagree:
[Dynamic Breath](https://www.nexusmods.com/skyrimspecialedition/mods/100466)
describes twenty seconds; [Nikolaf's More Realistic Underwater Breathing](https://www.nexusmods.com/skyrimspecialedition/mods/47886)
describes thirty. These are author observations, not verified here.

Installed WaterbreathingFFSelf is archetype 34 (Peak Value Modifier) of
WaterBreathing (AV 57), already handled by the effect runtime. No new native
is needed to apply that effect. The vanilla GMSTs are
fActorSwimBreathMult=0.2 and fActorSwimBreathDamage=0.08; their names and values
do not establish a formula. Chosen policy: capacity = 100 * breath multiplier
seconds (20 by default), then damage = maximum health * damage multiplier
per second. Only the portion of an update after exhaustion does damage.
Surfacing or gaining WaterBreathing resets elapsed time. No stamina term is
introduced. The hundred-unit capacity scale, recovery/reset policy and max
health basis need an original-game probe; keep these as follow-up questions
in the animation plan, without generating autotest20 now.

Humanoid NPC head immersion uses 90% of scaled skeleton height; player immersion
uses the eye. Chosen interim scope: breath damage only runs for loaded humanoid
graphs and the player. The human float depth submerges dogs' and mudcrabs' heads;
using it with the generic skeleton-height estimate falsely kills ordinary
swimmers. The available float-height probe measured a humanoid and does not
settle creature float heights or breathing points. Creature breath stays disabled
until those are resolved (animation plan question 8).
Transient breath state clears on unloading. Diving controls remain future work.
The inspected vanilla HUDMenu ActionScript (existing ignored
spike/swf/out/hudmenu.dis) exports health, magicka, stamina, shout and charge
meter setters, with no breath/oxygen symbol or setter. No custom meter is added.
