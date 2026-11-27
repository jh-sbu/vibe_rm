# Actors held in place: `SetDontMove` and `SetRestrained`

Source: the Creation Kit wiki's pages "SetDontMove - Actor" and
"SetRestrained - Actor" (text pasted by the user from a browser,
2026-11-10; the wiki blocks agents; copies under ignored
`refs/ck-wiki/`). What they say:

- `SetDontMove(abDontMove = true)`: "Set to limit the actors movement.
  Don't let this actor be pushed around by another actor." Its example
  keeps a dragon "to stay put and not be pushed around". On the player it
  "disables player movement, camera switching, and camera rotating, but
  keeps the full HUD displayed (unlike Game.DisablePlayerControls())".
- `SetRestrained(abRestrained = true)`: "Restrained actors will not move
  from their current position, but will continue to 'think' (pick
  packages, run detection so they could yell alarms) and go into
  dialogue." They "can still be moved with functions that invoke
  movement, such as KeepOffsetFromActor()". Restrained and unconscious
  exclude each other: setting one clears the other. The same note on the
  player as `SetDontMove`.

What ours does (`world_state.dont_move` / `restrained`,
`Engine::held_in_place`, `ActorRuntime::step_in_place`, `player_held`):

- Both hold an NPC where it stands: a walk its package starts is halted
  without turning it, and no step or root motion moves it (it can still
  fall). It keeps evaluating packages, detecting, idling and talking, and
  turns for other reasons. A `SetDontMove` actor fights in place; a
  restrained one doesn't start a fight (`Engine::start_combat` refuses it),
  and one restrained while fighting stays in combat, drawn, without
  attacking or casting (`Engine::update_actors`). A restrained actor with
  an active `KeepOffsetFromActor` isn't held. `GetRestrained` (condition
  244) reads the restrained flag.
- On the player, either stops movement input, camera switching (the POV
  toggle and zoom into first person) and mouse look; the HUD is left as
  it is.

Choices the pages leave open:

- "Limit the actors movement" is taken as holding it like a restrained
  actor, as Skyrim SE showed: autotest24's ViewCone observer, held by
  `SetDontMove`, read 300 units off at every spot, as placed, for over a
  minute (`research/autotest24.md`). Before this, ours ignored the call
  and the observer's sandbox idles' root motion moved it 5 to 15 units.
  Whether package travel is stopped outright or only slowed is untested.
- Whether `KeepOffsetFromActor` and other script movement move a
  `SetDontMove` actor (said only of restrained ones): ours holds it.
- Whether a restrained actor fights, settled in Skyrim SE by autotest27
  (`research/autotest27.md`, RestrainedFights). A fighter 100 units from
  an enemy, both given `StartCombat` on each other: restrained, it stayed
  in combat state 0 for 12 s, never swung or hurt the enemy, and took 55
  damage. Held by `SetDontMove`, it fought in place (5 swings, moved 0).
  Free, it fought and moved. autotest28 (`research/autotest28.md`,
  RestrainedMidFight) restrained a fighter already engaged: combat state 1
  and drawn for 10 s, 0 swings, 0 damage dealt, 36 taken, moved 75 units.
  Ours keeps it in combat without attacking. Unread: how the restrained
  fighter moves while struck (blows' pushes, perhaps; ours holds it), and
  whether it turns to face its target.
- Whether a held actor turns, sits or gives ground in combat; whether
  `IsMovementControlsEnabled` and `IsLookingControlsEnabled` read false
  for a held player (ours: they read only `DisablePlayerControls`);
  whether either flag lasts in a save (ours: not saved).
