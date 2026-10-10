# The player's body

Implemented in `src/player_body.rs` (assembly, animation and the third-person
camera) and `crate::world::actor::{describe_player, head_part_models}`.
Sources: UESP's Skyrim Mod pages for `NPC_` (`PNAM` head parts), `RACE`
(`HEAD` default head parts after `NAM0` / `MNAM` / `FNAM`) and `HDPT` (`PNAM`
part type, `HNAM` extra parts); the Creation Kit wiki for
`Game.ForceFirstPerson` / `Game.ForceThirdPerson`; the behaviour graphs
themselves (`vrm-tool hkb-tree`, `hkb-vars`, `hkb-run`) for event and
variable names; the race's attack data (`ATKE`) for attack events.

## Unconfirmed: choices made

- **Head parts.** The NPC's own parts (`PNAM`) replace the race's default
  part of the same type rather than adding to it, for every type; UESP
  doesn't say which types replace.
- **`IsNPC`** is set to 0 for the player's graph. No source gives the game's
  value; the graph takes locomotion, jumping and combat events with it.
- **`Direction` through `BSCyclicBlendTransitionGenerator`.** The locomotion
  blends' `Direction` is bound to the wrapper's `fBlendParameter` while the
  blender inside has no binding of its own; the runtime gives the blender the
  wrapper's binding, going by the member names (no description of the class
  was found).
- **Facing.** In first person, and with the weapon out, the body faces the
  view and strafes; with the weapon sheathed in third person it turns to face
  where it goes. Whether the game's sheathed third-person body backpedals and
  strafes instead, or turns round only for some directions, is unconfirmed.
- **Falling** is sent after 0.3 s off the ground without jumping, so steps
  and slopes don't set it off; the game's threshold is unknown.
- **Shadow in first person.** The third-person body casts its shadow while
  the view is first person; whether the game does is unconfirmed.
- **Drawing to attack.** Attacking with the weapon put away draws it instead
  of swinging; from memory of the game, not a source.
- **When the graph can swing.** Swings are sent only after `WeapEquip_Out`
  (the end of the draw): before it the graph takes `attackStart` and plays
  nothing. How the game gates attacks during the draw is unknown.
- **Power attack by movement.** The event is chosen from the race's attack
  names: `attackPowerStartInPlace` standing, `..._Sprint` sprinting, else
  `...Forward` / `Right` / `Backward` / `Left` by `Direction` (falling back to
  the standing one, then any). The names suggest this; no source states it.
- **Blows at `HitFrame`**, timed by the third-person graph in first person
  too (the game runs a first-person graph there). A swing whose `HitFrame`
  hasn't come within 1.5 s is dropped; the limit is chosen.
- **The third-person camera** sits straight behind the eye at 60 to 600
  units (200 at first), chosen by eye: the game's come from INI settings
  (over-the-shoulder offsets, zoom limits) not checked against a source.
  Activation and attack rays still go from the eye, which lines up with the
  crosshair only because the camera has no sideways offset.
- **Disabled camera switching** (`DisablePlayerControls`) stops the wheel's
  zoom as well as F; whether the game's does is unconfirmed.
