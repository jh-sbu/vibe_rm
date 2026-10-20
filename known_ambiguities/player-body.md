# The player's body

Implemented in `src/player_body.rs` (assembly, animation and the third-person
camera) and `crate::world::actor::{describe_player, head_part_models}`.
Sources: UESP's Skyrim Mod pages for `NPC_` (`PNAM` head parts), `RACE`
(`HEAD` default head parts after `NAM0` / `MNAM` / `FNAM`) and `HDPT` (`PNAM`
part type, `HNAM` extra parts); the Creation Kit wiki for
`Game.ForceFirstPerson` / `Game.ForceThirdPerson`; the behaviour graphs
themselves (`vrm-tool hkb-tree`, `hkb-vars`, `hkb-run`) for event and
variable names; the race's attack data (`ATKE`) for attack events.

## Confirmed in the game (`test-mod autotest2`, 2026-10-13)

- **`IsNPC`** is 0 (false) for the player's graph, `IsPlayer` 1. The run read
  the rest of the graph's variables too: `bHumanoidFootIKEnable` and
  `bHeadTrackSpine` 1; `iState` 0 sheathed and 9 drawn with spells in both
  hands; `iEquippedItemState`, `iLeftHandType` and `iRightHandType` 9 (spells);
  `iWantBlock`, `Speed`, `Direction` 0 standing; `IsFirstPerson` following the
  view.

## Unconfirmed: choices made

- **Head parts.** The NPC's own parts (`PNAM`) replace the race's default
  part of the same type rather than adding to it, for every type; UESP
  doesn't say which types replace.
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
- **Equipment changes and drawn state (2026-10-20).** Rebuilding the player's
  meshes now retains the body and running graph when the cached skeleton,
  project, rig path and sex still match. Draw intent, physical weapon placement,
  draw readiness, blocking, locomotion and in-progress draw/sheath transitions
  survive. Hand variables and rigid attachments refresh from the new inventory;
  attachments use the retained `weapon_out` state. The same wearer graph also
  survives, so changing unrelated armor does not restart the tail. A different
  skeleton/project/rig/sex still creates a fresh, sheathed body.
  This fixes a repository lifecycle bug identified by the roadmap; it does not
  establish Skyrim's exact equipment-transition policy. The local UESP searches
  for `equip weapon drawn` and `EquipItem` did not settle whether weapon-type
  switches replay an equip transition, whether blocking survives, or when
  attack/cast interruptions occur. Here bow draws/releases and pending melee
  swings are cancelled on equipment rebuild; a retained graph is asked for
  `attackStop` when needed. Cast policy is unchanged. Synthetic state-machine
  tests cover draw/readiness/sheathing notifications at zero time, compatibility
  gates, and shot cancellation without losing drawn state. No engine binaries
  or new game assets were examined for this change.
  VibeRM smoke tests using installed SE assets in WhiterunBanneredMare passed:
  equipping iron armor one frame after `pdraw` kept `Actor.IsWeaponDrawn` true;
  swapping an iron sword for an iron dagger while drawn kept it true and the
  next attack reached `AttackRight_1HMDagger`; removing armor after sheathing
  kept it false. These validate this implementation, not original Skyrim SE
  behavior. Logs and screenshots stayed under `/tmp`, outside the repository.
- **Power attack by movement.** The event is chosen from the race's attack
  names: `attackPowerStartInPlace` standing, `..._Sprint` sprinting, else
  `...Forward` / `Right` / `Backward` / `Left` by `Direction` (falling back to
  the standing one, then any); the left hand's add `LeftHand`
  (`attackPowerStartBackLeftHand` going back). The names suggest this; no
  source states it. The hands' attacks: `known_ambiguities/hands.md`.
- **Blows at `HitFrame`**, timed by the third-person graph in first person
  too (the game runs a first-person graph there). A swing whose `HitFrame`
  hasn't come within 1.5 s (of its start, or its last blow) is dropped; the
  limit is chosen. It is over at `attackStop` once it has landed, and the
  next attack waits 0.4 s (the graphs' blend out of an attack): the clip
  blending out raises `attackStop` again, which would end an attack
  started meanwhile. How the game gates it is unknown.
- **Bow release timing.** The player's button release now asks the body's
  graph for `attackRelease`; only an accepted draw followed by an accepted
  release can produce an arrow at `arrowRelease`. Ammo consumption and aim
  are evaluated at that notification, while draw strength keeps the button's
  hold duration. PolarRobotFrog's public
  [first-person animation modding guide](https://www.nexusmods.com/skyrimspecialedition/mods/97555)
  identifies `arrowRelease` as the annotation that fires the arrow/bolt. The
  third-person graph still supplies this timing in both views until a separate
  first-person body exists. Installed `Skyrim - Animations.bsa`'s
  `meshes/animationdata/defaultmale.txt` confirms `arrowRelease` in `Bow_Release`
  at 0.0333333 s, and `SneakBow_Release` at 0.0666667 s. The project's `hkb-run`
  tool also confirms accepted `bowAttackStart` / `attackRelease` transitions
  and duplicate upper/lower-body release clips; that tool omits the animation
  data triggers that `GraphLibrary` adds at runtime. Only asset metadata was
  inspected; no engine binaries or downloadable tool bundles were examined,
  and no assets were added to the repository.
- **Interrupted or duplicate bow releases.** A missing `arrowRelease` cancels
  the pending shot after 1.5 seconds, without spending ammo or firing on a
  timer. Sheathing, body rebuilding (including equipment changes), death,
  conversation and disabling fighting cancel it too (including when the graph
  is retained across an equipment rebuild); rejected graph events
  cannot queue a shot. Interrupted draws and refused releases ask the graph
  to lower the bow with `attackStop`, as for NPCs. Another draw waits until
  no `arrowRelease` has arrived for 0.4 seconds, so outgoing layered/blending
  clips' duplicate events do not
  consume another arrow or a newly queued shot. Events during drawing are
  ignored; a button release during that quiet interval cancels the draw.
  Both limits and these cancellation policies are chosen; public sources do
  not specify the game's policies, exact blend completion gates, or which
  equipment/aim/ammo it snapshots before release. A notification received in
  the same update as the timeout wins, since its time within that update is
  not exposed to the player event consumer. The console's direct `pshoot`
  and a player without a body graph retain immediate firing.
- **The third-person camera's starting distance** is 200 units, chosen by
  eye: the `[Camera]` defaults (STEP's default INI listing) give the zoom
  limits (`fVanityModeMinDist` 155, `fVanityModeMaxDist` 600) and the
  over-the-shoulder offsets, but no starting distance.
- **Where the shoulder offsets apply.** `fOverShoulderPosX` / `PosZ` (30,
  -10) and the drawn `fOverShoulderCombatPosX` / `PosZ` (0, 20) are taken as
  right and up from the eye (right level with the ground, up the world's),
  then the camera goes back by the distance. `fOverShoulderCombatAddY`
  (-100) is taken as forward along the view, so drawn the camera sits 100
  farther back. The game's pivot (the eye, a camera node, the body) and its
  axes aren't published. The move between the two places takes 0.4 s,
  chosen.
- **Aim in third person.** Activation, attacks, shots, casting and grabbing
  aim from the eye at the first thing the view's centre meets (looked for
  from level with the eye), so they meet the crosshair; the game's way of
  matching them isn't published.
- **Disabled camera switching** (`DisablePlayerControls`) stops the wheel's
  zoom as well as F; whether the game's does is unconfirmed.
