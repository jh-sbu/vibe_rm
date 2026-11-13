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

## Swimming depth

Measured in SE (`research/autotest25.md`, an authored cell of placed still
water with floors of exact depth): swimming begins with the feet about 90
under the surface, the same for the player and an NPC (dry at 89.7,
swimming at 90.2 and 94.7). Moving out continuously, swimming lasted
until the feet were between 55 and 68 under (swimming at 67.7, dry by
about 55, the detector's lag allowed for). Moved by `MoveTo`, the state
is judged afresh. A still swimmer placed deeper than about 92 kept its
depth for 20 s. Placed shallower it came up to 92 to 94 under.
autotest19 and autotest20's lake readings (entry between bed depths 100.4
and 105.1) are superseded: the feet there stood 24 to 37 above the
terrain's heights.

Chosen: start past 90 under (`SWIM_DEPTH`), and stay swimming while the
feet are 60 or more under (`SWIM_EXIT_DEPTH`, inside the measured 55 to
68). The feet decide, not the bed: on autotest25's ramp feet and bed were
3 apart, so the two weren't told apart. A `MoveTo` of an NPC clears its
state (the player is placed afresh anyway). A translation carries it
over. Swimmers keep their depth but rise no higher than 92 under
(`FLOAT_DEPTH`). The player's slight rise after a move (about 10 in
autotest25; NPCs none) isn't modelled. Waterbreathing doesn't change the
state, and NPCs share the player's depth (autotest20, autotest25).
Scale- and race-dependent depths remain unmeasured.

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
navmesh paths. Swimmers keep their depth, no higher than surface-minus-92
(see Swimming depth); land collision standing is suspended. No diving or race-specific float height
is inferred. Swimming fighters pursue without attempting attacks or drawing.
ActionDraw's vanilla tree, contrary to the planning sketch, has no general
IsSwimming condition: avoiding a draw while swimming is an engine policy here.
The humanoid swim blend actually binds `Direction`; `SwimDirection` is declared
but has no binding in the inspected graph. Both are supplied the motion fraction.

The humanoid graph declares no NPC swim iState constant (its HorseSwim
constant is a mounted subgraph's). Humanoids therefore use NPCSwimming's
speeds while retaining their default iState; creatures use their own authored
swim constants. This avoids accidentally selecting mounted swim speeds.

Player movement keeps the same depth rule using collision movement,
with no swim gravity or dive controls. The capsule's 64-unit half-extent and
56-unit eye offset leave the eye 28 above the surface at that height. Swim
speed uses NPCSwimming walk/run, scaled by the existing armor movement rule.
Diving controls await underwater rendering; breath still applies to actors
whose head is actually submerged (including externally placed actors).

## Interior water levels

An interior cell can carry the has-water flag (`DATA` 0x2), a water level
(`XCLW`) and a water type (`XCWT`). In SE (`research/autotest25.md`) an
authored interior with the flag, level 0 and
`DefaultIntWaterWaistHeightStill` had no water: none to see, and nobody
swam at any depth. Vanilla interiors' visible water is placed (activators
with a water type). Chosen: interiors get no water from their level.
Uncertain: 19 vanilla interiors have the flag and a finite level with no
placed water (`BronzeWaterCave`, `RiftenRatway01`, `AnisesCabin01`,
`Mzulft01`, ...). Whether SE shows water at the level in any of them, for
example with another water type, wasn't checked. Before this change ours
gave them a water plane at that level.

## Placed water

Vanilla places much of its water as activators with a water type (`WNAM`):
62 kinds in Skyrim.esm, in 128 interiors and along outdoor rivers. In SE
(`research/autotest25.md`) six `Water2048DefaultIntWaistStill` gave a
swimmable body at their height, down to a floor 300 under. Chosen: each
such reference is water over its base's bounds (`OBND` x and y, as
placed: position, rotation about all axes, scale), at the height of the
bounds' middle, reaching down without limit, of its water type.
Uncertain: how deep a placed water really reaches (only 300 was tried),
whether its mesh's own shape matters where it isn't a rectangle
(`MrkWaterSystemWarrens01`), and placed water that is enabled, disabled or
moved after its cell loads (`TG08RisingWaterScript` translates one): ours
keeps what was there at load. Its surface is drawn as a flat rectangle of
its water type, like cell water.

## Currents

Measured in SE at one marker (`research/autotest26.md`): Update.esm's
`CellWaterCurrentMarker` `01002B8A` in a mountain lake, in a cell whose
water was `DefaultWater` while the test plugin overrode it. Still swimmers,
player and NPC alike, drifted along the marker's heading at about 71 units
a second. The speed was the same at every free point to 1024 along and 512
across, and at 92 and 122 under. Rocks turned them off their line, and a
higher strip of bed stopped them. The marker holds only a position and a
heading, and nothing published gives a speed or a reach.

Chosen: a cell whose water level is its own (exterior cell water) and
which holds a current marker flows along the marker's heading at 71 units
a second (`CURRENT_SPEED`), over the whole cell, whatever its water type.
Without a marker there is no current. Only swimmers are carried. A
swimmer's drift stops where something stands in the way at its waist or
the bed ahead rises to its feet (NPCs; the player's own collision handles
its drift). Uncertain: the speed elsewhere (one site; Update.esm's flowing
water types carry no value near 71), whether the reach is the cell (every
point measured lay in the marker's cell), cells with several markers (the
first is taken), wading actors, and placed water: no interior has a
marker, and placed rivers (`Water1024RiverFlowSE`, ...) get no current
here.

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
in the animation plan; autotest20 doesn't measure them.

Humanoid NPC head immersion uses 90% of scaled skeleton height; player immersion
uses the eye. Chosen interim scope: breath damage only runs for loaded humanoid
graphs and the player. The human float depth submerges dogs' and mudcrabs' heads;
using it with the generic skeleton-height estimate falsely kills ordinary
swimmers. The available float-height probe measured a humanoid and does not
settle creature float heights or breathing points. Creature breath stays disabled
until those are resolved (animation plan question 8).
Transient breath state clears on unloading. Diving controls remain future work.
The inspected vanilla HUDMenu ActionScript (disassembled to
`$SPIKE_SWF_HOME/out/hudmenu.dis`, outside the repository) exports health, magicka, stamina, shout and charge
meter setters, with no breath/oxygen symbol or setter. No custom meter is added.

## Submerged audio

The camera uses the same water surface query as swimming. SNCT FNAM bit 0
controls muting and DOBJ UWLS names a 2D underwater loop. Public documentation
does not give a transition time: we choose 150 ms amplitude ramps. No extra
hysteresis is used without observed flicker. Underwater REVB is selected by
editor ID in step 5; no record links it to submersion, so that override is an
inference from its name and parameters, not verified original-game behavior.
