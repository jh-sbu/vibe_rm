# Line-of-sight and tracked-stats registrations

Natives (on `Form`, `Alias` and `ActiveMagicEffect`): `RegisterForLOS`,
`RegisterForSingleLOSGain`, `RegisterForSingleLOSLost`, `UnregisterForLOS`,
`RegisterForTrackedStatsEvent`, `UnregisterForTrackedStatsEvent`. Implemented in
`src/registrations.rs`.

## Sources

The Papyrus Index (papyrus.bellcube.dev, consulted 2026-11-09), descriptions
attributed to the Creation Kit wiki (CC BY-SA 4.0):

- `RegisterForLOS(akViewer, akTarget)`: line-of-sight gain and loss events
  between viewer and target go "only [to] the specific form, alias, or magic
  effect that registered", not relayed to aliases or effects. A gain or loss is
  sent at once, by whether the viewer already sees the target. A non-player
  viewer's target must be an actor. Non-player actors make "a single check
  using the detection system"; the player makes three picks clipped to the
  camera. Picks are throttled to about one a frame. Aliases and quests
  unregister when the quest stops, effects when removed.
- `OnGainLOS(akViewer, akTarget)` / `OnLostLOS`: when the viewer goes from
  not seeing to seeing (or back).
- `RegisterForTrackedStatsEvent()`: `OnTrackedStatsEvent(arStatName,
  aiStatValue)` when a tracked stat updates, to the registered object only. The
  `OnTrackedStatsEvent` page lists the stats that send it: Locations
  Discovered, Dungeons Cleared, Most Gold Carried, Skill Increases, Skill Books
  Read, Quests Completed, Misc Objectives Completed, Dragon Souls Collected,
  Words Of Power Learned, Shouts Learned, Shouts Mastered, Magic Items Made,
  Weapons Made, Armor Made, Locks Picked, Pockets Picked, the ten holds' and
  Tribal Orcs' bounties, and Level Increases.

## Choices

- The player sees a target in the camera's view (its vertical field of view and
  a 16:9 width) with a clear line from the camera to the target's position
  raised 60 units; an NPC by `GetLineOfSight`'s view cone and clear line. Four
  registrations are looked at a frame, in turn.
- A single registration ends once its event is sent. `RegisterForLOS` again
  for the same viewer and target replaces the earlier registration.
- Bounty stats don't send the event yet: bounties are kept by crime faction,
  not as counted stats.
- Registrations are saved (`LOSR`, `TSTL`), with whether the viewer last saw
  the target, so a load doesn't repeat a gain.
