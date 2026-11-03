# First-person character (2026-11-03)

Evidence is from public documentation and installed Skyrim SE plugin, NIF and
Havok assets only. No engine binary or memory was examined. No external source
code was used. Reproduce the record/trigger survey with `vrm-tool first-person
"<Data>"`; use `hkb-vars`, `hkb-run` and `hkb-annotations` on
`meshes/actors/character/_1stperson/firstperson.hkx` for the graph.

## Records and models

The installed base masters plus Creation Club load order contains 1,134 ARMA:
259 with both MOD4/MOD5, six male-only, three female-only, 866 with neither.
The omitted slot masks include head/hair/circlets, amulets, rings, feet/calves,
tails and creature bodies, as well as some body/hand parts. The survey prints
every omitted addon and its mask, rather than assuming all are helmets.
3,300 WEAP have a non-null WNAM, all resolving to a STAT with an existing
MODL asset; 28 have no non-null WNAM.

No surveyed RACE names `_1stperson`. The matches in ARMA/STAT are model names,
not a race's rig/project selection. The fixed humanoid rig/project remains an
inference from the assets, not a documented record lookup rule.

[UESP ARMA, revision 3089062](https://en.uesp.net/w/index.php?oldid=3089062)
documents the model pairs and says MOD2/3 serve all viewpoints without MOD4.
This does not explain which parts the renderer suppresses near the eye.
[UESP Console, revision 3628513](https://en.uesp.net/w/index.php?oldid=3628513)
documents `s1st` displaying the separate first-person arms in third person.

## Graphs and triggers

`hkb-run` with `IsFirstPerson=1 i1stPerson=1 IsPlayer=1`, hand types 1/0,
`!weapEquip 2 !attackStart 1 !blockStart 1 !blockStop !Unequip 2` takes each
action. Draw raises WeapEquip_Out; the swing raises preHitFrame, weaponSwing,
HitFrame and AttackWinStart; blocking raises blockStartOut and attackStop.
With hand type 7, `!bowAttackStart 1 !attackRelease 2` takes draw and release.
The raw graph emits BowRelease; animation data supplies arrowRelease.
The cast sequence needs BeginCastRight before MRh_SpellAimedStart, as the
existing player casting driver sends it.

`meshes/animationdata/firstperson.txt` has 274 FootLeft, 279 FootRight,
98 FootSprintLeft and 92 FootSprintRight triggers (also scuffs), two
arrowRelease, twenty weaponDraw, eleven weaponSheathe, seven
MLh_SpellFire_Event and four MRh_SpellFire_Event. These are occurrences across
clip definitions, not the count in one action. The survey reports each clip,
event/payload and trigger time. The runtime already imports this data into
GraphAnim; `hkb-run` reports the raw authored behavior alone.

AimPitchCurrent and AimHeadingCurrent bind BSDirectAtModifier current-offset
outputs. BowAimOffsetPitch binds its offsetPitchDegrees; MagicAimOffsetPitch
is declared but has no direct binding in the inspected listing. Camera and
LookNode placement still need original-game comparison; see the decision note.
bowZoomAmt is computed from IsAttacking, iWantBlock, bowZoom, ZoomAcc and
TimeDelta. This does not establish the camera's FOV conversion or perk gating.

## Camera settings

[STEP's authored INI reference](https://stepmodifications.org/wiki/Guide:Skyrim_INI/Display#fDefault1stPersonFOV)
lists fDefault1stPersonFOV's default as 65 degrees, separately from world FOV.
It concerns original Skyrim: the SE angular convention and conversion to a
vertical projection remain unverified. Using 65 degrees vertically and a
one-unit near plane is recorded as an implementation choice, not measured SE
behavior. No source found here specifies a depth-buffer algorithm.

## Original-game probe

The `firstperson` test mod is to be run by the user in Skyrim SE. Its saved
reports, event routing, wheel observations, furniture view restoration,
shadows and helmet visibility have not yet been observed. These are open
verification tasks, not established results.
