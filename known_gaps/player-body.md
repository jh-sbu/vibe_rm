# The player's body

Implemented in `src/player_body.rs` (assembly and animation) and
`crate::world::actor::{describe_player, head_part_models}`.
Sources: UESP's Skyrim Mod pages for `NPC_` (`PNAM` head parts), `RACE`
(`HEAD` default head parts after `NAM0` / `MNAM` / `FNAM`) and `HDPT` (`PNAM`
part type, `HNAM` extra parts).

The body is the player's NPC record (`0x7`): its race's skeleton, skin and
behaviour project for its sex, what the player has equipped (rebuilt when that
changes) and a head built from head parts. It runs its graph as NPCs do,
driven by the player's movement (`Speed`, `TurnDelta`, `moveStart` /
`moveStop`, `SneakStart` / `SneakStop`, the movement type's `iState`), with
`IsNPC` 0.

## Open

- **The head.** No FaceGen head is pre-built for the player, so it is put
  together from the race's default parts and the NPC's own, with no morphs
  (`.tri`: race, chargen sliders, expressions), no tint layers (`TINI` /
  `TINC`: skin tone, war paint), no hair colour (`HCLF`) and no texture set
  overrides (`HDPT` `TNAM`). Which part types the NPC's own parts replace,
  rather than add to, is taken to be every type; UESP doesn't say.
- **First person** draws nothing of the player: the game has a separate
  first-person skeleton (`_1stperson/skeleton.nif`), arms and `1stperson*`
  armour and weapon models and its own behaviour graph. The third-person body
  casts its shadow in first person here; whether the game does is unconfirmed.
- **Graph inputs still missing**: `Direction` (strafing and walking backwards:
  in third person the body turns to face where it goes instead), sprinting
  (`SprintStart`), jumping and falling, weapon drawing, attacks, blocking, bow
  draws and casting, so the body plays none of these. Its footstep sounds still
  come from the first-person stride, not the graph's footstep events. Foot IK
  and head tracking are off.
- **Dying** leaves the body standing; it has no ragdoll.
- **Nothing shows it yet**: there's no third-person camera, so the body only
  casts its shadow.
