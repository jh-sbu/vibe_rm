# Furniture

Implemented in `src/ai/furniture.rs` and `src/ai/mod.rs`.

## Choices made without a source

- An NPC getting into furniture activates it as it begins its enter
  animation: the furniture's scripts hear `OnActivate` with the actor, and its
  activate children are activated. No public source says when (or whether)
  the game activates furniture for NPCs; the vanilla scripts imply it does:
  `CarryFurnitureScript` (wood piles, buckets, stone piles) registers for the
  user's `AddToInventory` / `RemoveFromInventory` animation events in
  `OnActivate`, and NPCs use wood piles.
- Actors placed in furniture as a cell loads don't activate it.
- Ways on are picked ahead of time without an actor. An idle whose own
  conditions count an item (`GetItemCount`: a wood pile's pick-up wants no
  firewood, its put-down some) makes its way only for actors without or with
  that item; the AI checks the actor's inventory when it reserves the marker.
- A one-shot use (the last clip plays once: picking up firewood) lasts as long
  as that clip, whatever the package says. Leaving it, the graph is sent
  `IdleForceDefaultState` and then the event the idle tree gives for
  `IsExitingInstant` (a wood pile: `OffsetCarryLogStart`). Nothing public says
  the engine leaves one-shot furniture this way.
- While carrying, the actor keeps the pick-up's anim objects, plays no standing
  idles, and on getting into furniture its graph is sent `OffsetStop` (the
  put-down's clip then puts the logs away with `AnimObjectUnequip` and leaves
  with `IdleFurnitureExit`). Furniture other than a put-down drops the objects
  when the actor stands up; the load stays in its inventory.
