# The player's body

Implemented in `src/player_body.rs` (assembly, animation and the third-person
camera) and `crate::world::actor::{describe_player, head_part_models}`.
Sources: UESP's Skyrim Mod pages for `NPC_` (`PNAM` head parts), `RACE`
(`HEAD` default head parts after `NAM0` / `MNAM` / `FNAM`) and `HDPT` (`PNAM`
part type, `HNAM` extra parts); the Creation Kit wiki for
`Game.ForceFirstPerson` / `Game.ForceThirdPerson`.

The body is the player's NPC record (`0x7`): its race's skeleton, skin and
behaviour project for its sex, what the player has equipped (rebuilt when that
changes) and a head built from head parts. It runs its graph as NPCs do,
driven by the player's movement (`Speed`, `TurnDelta`, `moveStart` /
`moveStop`, `SneakStart` / `SneakStop`, the movement type's `iState`), with
`IsNPC` 0, and the player's own: `Direction` (where they go against where
they face, a fraction of a turn clockwise: strafing and walking backwards),
`SprintStart` / `SprintStop`, `JumpStandingStart` / `JumpDirectionalStart`
(standing or on the move), `JumpFall` when off the ground 0.3 s without
jumping, and `JumpLand` / `JumpLandDirectional`, after which `moveStart` is
sent again (the graph stands after landing).

The locomotion blends take `Direction` through their
`BSCyclicBlendTransitionGenerator`'s `fBlendParameter`, the blender's own
parameter being unbound; the runtime passes it on (as the member names
suggest), not from a description of the class.

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
- **Facing.** In first person the body faces the view and strafes; in third
  person it turns to face where it goes (`Direction` then near 0). Whether
  the game's third-person body walks backwards or turns round with the
  weapon sheathed is unconfirmed.
- **Graph inputs still missing**: weapon drawing, attacks, blocking, bow
  draws and casting, so the body plays none of these. Its footstep sounds still
  come from the first-person stride, not the graph's footstep events. Foot IK
  and head tracking are off.
- **Dying** leaves the body standing; it has no ragdoll.
- **The third-person camera** sits straight behind the eye, its distance
  (60 to 600, 200 at first) chosen by eye: the game's come from INI settings
  (over-the-shoulder offsets, zoom limits) not checked against a source. It
  doesn't swing round the body on its own, there is no vanity camera, and the
  activation and attack rays still go from the eye (straight ahead of the
  camera, as it sits behind it). It stops short of the world but not of actors.
- **`aiDisablePOVType`** (`DisablePlayerControls`) is still ignored; disabled
  camera switching stops F and the wheel.
