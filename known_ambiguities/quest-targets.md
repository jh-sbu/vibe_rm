# Quest targets on the compass

Implemented in `src/quest_targets.rs`, with the compass in `src/swf_ui.rs`.
Sources: the data (`QOBJ`, `QSTA` (alias, flags) and the `CTDA` after each
target; load doors' `XTEL`); the CK wiki (quest objectives and targets,
`SetActive`); `hudmenu.swf` (the Quest and QuestDoor frames of the compass
marker, quest markers off the compass held at its nearer edge).

## Unconfirmed: choices made

- **Which targets.** Those of the displayed objectives not completed or
  failed, of running quests the journal tracks (`SetActive`), whose
  conditions pass with the target as the subject and the quest as the
  quest. Targets that are location aliases, or aliases left empty, show
  nothing; nor do disabled references, the player, or what the player
  carries. `QSTA`'s flag 1 (the marker ignoring locks) isn't used: the way
  through doors doesn't look at locks.
- **Where a target is.** A loaded actor where it stands, an actor not
  loaded where its schedule has it, what's in a container where the
  container is, anything else where it was placed or moved to.
- **The way there.** A target in the player's interior or worldspace is
  marked where it is. Elsewhere, the marker is the load door the way there
  starts at: the fewest load doors between interiors and worldspaces (all
  the data's, disabled ones aside), the nearest of the first doors as short.
  The game's own choice (it may weigh distances, or the navmeshes' door
  links) isn't published.
- **Every quest marker** is at full alpha and scale 100, at any distance;
  targets sharing a spot (or a door) get one marker.
- **Enemies.** Loaded actors fighting the player (not fleeing, dying or
  dead) are marked with the Enemy frame, at any distance.
