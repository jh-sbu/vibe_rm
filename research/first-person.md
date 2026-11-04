# First-person character (2026-11-03)

Evidence is from public documentation and installed Skyrim SE plugin, NIF and
Havok assets only. No engine binary or memory was examined. No external source code was incorporated. The existing xEdit format
labels are MPL-2.0; no implementation or declarations were copied into this
MIT project. Reproduce the record/trigger survey with `vrm-tool first-person
"<Data>"`; use `hkb-vars`, `hkb-run` and `hkb-annotations` on
`meshes/actors/character/_1stperson/firstperson.hkx` for the graph.

## Records and models

The installed base masters plus Creation Club load order contains 1,134 ARMA:
259 with both MOD4/MOD5, six male-only, three female-only, 866 with neither.
The initially omitted slot masks include head/hair/circlets, amulets, rings, feet/calves,
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

## Implementation follow-up

The record survey identifies DOBJ FFFP as keyword 000A56D7,
FurnitureForce1stPerson, and FFTP as 000A56D8, FurnitureForce3rdPerson.
FFFP is a default-object key, not a KYWD subrecord. The probe chair now uses
Skyrim.esm's actual FFFP keyword. The local
[KYWD page, revision 3089088](https://en.uesp.net/w/index.php?oldid=3089088)
explains KWDA keyword membership, but does not describe these default keys.

IronShieldAA (00012EB8) has MOD2 only; many other vanilla shields likewise
lack MOD4/5. Shield addons therefore use the documented shared-model
fallback, while explicit first-person pairs still take precedence. Other
omissions remain a choice pending the probe.

The character asset names characterassets/skeletonfirst.hkx; there is no
skeleton.hkx adjacent to _1stperson/skeleton.nif. Animation loading now accepts
that explicit Havok rig path. This matters for clip binding, not just graph
state transitions: using an adjacent filename leaves the arms in bind pose.

Engine-only offscreen captures use `scripts/first-person-captures.sh` and
remain under /tmp. They are smoke tests, not vanilla SE observations.

The full-draw bow uses a one-element mask that excludes its root; unlisted
bones must retain weight one. Treating them as zero dropped the arms to bind
pose even though the graph advanced normally. This is also described by the
[Souls Modding author's Havok behavior research](https://www.soulsmodding.com/doku.php?id=topic:havok_behavior_editing)
(section on half blends). That is cross-game community evidence, corroborated
by the Skyrim first-person assets and the drawn-bow regression/capture, not
an original Skyrim SE runtime measurement. Sampling and coverage diagnostics
now agree on the implicit-one tail.

### Bone-weight array survey (`vrm-tool hkb-masks "<Data>"`)

The convention applies to every graph, not only the first-person one, so all
installed behavior projects were surveyed against their own Havok skeletons:

- Blender children with their own arrays: 507 full length, 631 shorter than
  the skeleton, 9 longer. None of the 631 short arrays ends in exactly 1:
  591 end in 0 and the rest in 0.0001–0.7. That is the shape expected if
  trailing ones are left out, and not if trailing zeros are.
- 101 short arrays are all zeros. Under an implicit-zero tail each would be a
  no-op. Their lengths stop just before meaningful bones instead: 98/99 before
  `Camera Control` (30), 97/99 before `Camera3rd`/`Camera1st`, 1/99 after the
  root (42), 4/99 before `NPC COM` (9).
- Example: `sprintbehavior.hkx` `1HM_Torch_SprintForwardSword[0]` (82/99)
  weights the right hand, forearm and twists at 1 and the left arm and all
  left fingers at 0, then stops before `NPC R Finger00`. Only the implicit-one
  tail keeps the right fingers with the right hand. 129 arrays stop at that
  bone.
- Every BSBoneSwitchGenerator child in the vanilla assets is bound to a
  character property, and every property that switches use is full length.
  The short properties (`SneakMagic{Upper,Lower}Body` 85/99, dragon priest
  ward weights 80/82, troll `H2HTurnWeights_*` 54/56) are unused or bound to
  blenders only, and the last two omit only magic nodes. The runtime's switch
  default mask (`1 − max` of the children, treating a missing child entry as
  0) therefore never meets a short switch array in vanilla data. Its tail
  convention is untested by these assets.

These are inferences from authored data, not SE runtime measurements, but
they are Skyrim's own assets rather than cross-game evidence.

The dual-rig bow regression also found that changing the third-person rig's
`i1stPerson` with the view resets its release branch before arrowRelease.
Keeping its authored identity zero, with IsFirstPerson following the view,
passes a full draw/release/view-switch fixture and consumes exactly one arrow.
The first-person rig keeps identity one. This is an implementation choice
pending the SE probe's variable readings, not a claimed vanilla observation.
