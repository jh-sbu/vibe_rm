# Living ragdolls: evidence and asset census

2026-11-09. Target: vanilla SE and its official masters, with assets read at
`/home/frieren/.local/share/Steam/steamapps/common/Skyrim Special Edition/Data`.
No engine executable, library or memory was examined. Reproduce the survey with
`vrm-tool ragdoll-census <Data>` and `hkb-obj <Data> meshes/actors/character
0_master.hkx <class>`. Derived census output stays in ignored
`refs/ragdoll-census.txt`; assets are not extracted into this repository.

## Public information and licenses

The local UESP index was consulted first:
[Unrelenting Force, revision 3603032](https://en.uesp.net/w/index.php?oldid=3603032)
notes 66–71 document delayed fall damage before recovery, immunity, jumping during
recovery, and wards preventing a draugr shout's ragdoll effect.
[Paralyze, revision 3648133](https://en.uesp.net/w/index.php?oldid=3648133)
notes 60–66 document falling over, delayed recovery, immunity to falling damage
and pickpocket vulnerability while getting up.

Member names/order were checked against
[Skyrim Behavior Editor](https://github.com/BrannigansLaw/Skyrim-Behavior-Editor-),
MIT, copyright 2016 Wayne K, and knock-state names/numbers against
[CommonLibSSE-NG ActorState.h](https://github.com/CharmedBaryon/CommonLibSSE-NG/blob/main/include/RE/A/ActorState.h),
MIT, copyright 2018 Ryan-rsm-McKenzie. These licenses permit use in this MIT
project. Only format facts and enum values are used, with independently written
parsing and simulation. xEdit's `wbDefinitionsTES5.pas` is MPL 2.0; it was read
for record-format facts, without incorporating its implementation. The license
and source references remain under `refs/`.

## Findings from the assets

- RACE DATA flags are at byte 32: 0x400000 No Knockdowns, 0x40000 Allow
  Ragdoll Collision. These are separate from shout immunity keywords.
- The nonzero race ATKD knockdown entries in the installed official masters are
  werewolf/werebear sprinting left/right attacks, all 1. Their actual trigger rule
  still needs SE observation; do not silently apply a guessed probability.
- EXPL DATA flags at byte 44 use bits 2 and 3 for Always and By Formula.
  These are future consumers of the knockdown API; explosions remain separate.
- VoicePushEffectScript properties include force 15 on Unrelenting Force's third
  word, 10 on werewolf/shield charge effects, and values 2–15 on DLC effects.
- The humanoid selector's children are reanimate, ordinary get-up, swimming,
  in that order. Creature selectors also put reanimate before ordinary recovery;
  many omit swimming. State labels often contain spaces, so capability checks
  must inspect states/events rather than require the humanoid spelling.
- FullyRagdoll's no-matching powered modifier has maxForce **0**, tau 0.8,
  damping 1, recovery velocities 2 and 1. Matching has maxForce **200**, the same
  other settings, and different bone weights. A zero maxForce must stay limp.
- GetUpMod: normal +Z, duration 1 second, ground alignment 0.25 seconds.
  Humanoid root/other/another bones are 0/11/10 in Havok skeleton order.
- Rigid-body controls: hierarchy 0.17, velocity damping 0, acceleration 1,
  velocity 0.6, position 0.05, position limits 1.4/1.8, snap gain 0.1,
  snap velocity limits 0.3/0.3, snap distance limits 0.03/0.1, blend 0.5.
- Serialized 64-bit offsets (relative to object): pose matcher rotation 0xa0,
  blend/speed/switch timings 0xb0–0xbc, play/match events 0xc0/0xc4,
  four signed bone indices 0xc8–0xce, mode 0xd0. Get-up normal 0x50,
  times 0x60/0x64, three signed bone indices 0x68–0x6c. Powered controls
  scalars 0x50–0x60, bones pointer 0x70, three indices 0x78–0x7c,
  mode 0x7e, weights pointer 0x80. Rigid controls scalars 0x50–0x80,
  bones pointer 0x90. Keyframe info array 0x50, bone-list pointer 0x60.
  Contact-listener event id 0x58, payload pointer 0x60, bones pointer 0x68.

The plan's furniture/mount behavior, force units, area threshold, recovery timing,
constraint release timing and living ForceAdd/Remove semantics cannot be settled
from these assets. Choices belong in `known_ambiguities/ragdolls.md`; the supplied
SE probe is needed to replace them with observations. No game results are claimed.
