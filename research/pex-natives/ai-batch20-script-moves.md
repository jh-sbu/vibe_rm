# Scripted movement and small actor states (batch 20)

Natives: `Actor.PathToReference`, `KeepOffsetFromActor`,
`ClearKeepOffsetFromActor`, `SetHeadTracking`, `SetPlayerControls`,
`GetPlayerControls`; `ObjectReference.SplineTranslateToRefNode`,
`RemoveAllStolenItems`, `SetContainerAllowStolenItems`.

## Sources

The Papyrus Index (papyrus.bellcube.dev, consulted 2026-11-09), descriptions
attributed to the Creation Kit wiki (CC BY-SA 4.0):

- `PathToReference(aTarget, afWalkRunPercent)`: the actor paths to the target
  at a speed between walking (0) and running (1); latent, returning when the
  goal is reached, pathing fails or another request interrupts it.
- `KeepOffsetFromActor(arTarget, x, y, z, angleX, angleY, angleZ,
  afCatchUpRadius = 20, afFollowRadius = 5)`: keeps an offset in the target's
  body space (x right, y ahead, z up); beyond the catch-up radius it runs,
  within the follow radius it stays put.
- `SetHeadTracking(abEnable = true)`: head tracking on or off.
- `SetPlayerControls(abControls)`: whether player input goes to the actor;
  an NPC's AI is disabled. Not for the player (use `DisablePlayerControls`).
- `RemoveAllStolenItems(akTransferTo = None)`: removes all stolen items,
  moving them to the other object if given.
- `SetContainerAllowStolenItems(bool)`: containers only; whether stolen items
  show in the container's menu.

## Game-asset observations

`PathToReference` is called by the test rig only; `SetHeadTracking` by a few
scene and quest scripts; the rest have no vanilla caller (`pex-natives`).

## Choices

- PathToReference picks a gait from the share (under 0.25 walk, 0.5 fast
  walk, 0.75 jog, else run), pins the actor's goal, and returns True within
  96 units of the target, False if the actor dies, unloads or is sent
  elsewhere, or after 300 s. Neither waits nor offsets are saved.
- KeepOffsetFromActor travels to the offset spot when farther than the follow
  radius (running past the catch-up radius), else stands turned to the
  target's heading plus the Z angle. The X and Y angles are ignored.
- SetHeadTracking(false) leaves the head facing where the body faces.
- SetPlayerControls(true) on an NPC turns its AI off and `GetPlayerControls`
  reads it (always True for the player); player input isn't routed to an NPC
  yet (open in `plans/ROADMAP.md`).
- RemoveAllStolenItems moves the stolen stacks (`Inventory::owned`) with
  their owners; their copies' souls and charge are dropped.
- SetContainerAllowStolenItems is recorded only; what "shown in the menu"
  hides (the player's stolen items when storing?) isn't described.
