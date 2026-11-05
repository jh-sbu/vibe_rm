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

## Paired furniture graph events (2026-11-05)

Asset evidence in `research/carry-sawmills.md` connects the worker's
MillLogPileLoadStart notification to the sawmill furniture graph's matching
transition and ResourceObjectSawMillScript's self-registration. Previously
only the worker's scripts received its raised notification, leaving the
separate log graph idle.

Choice: relay payload-free notifications from a furniture user to that exact
furniture's graph. If the graph accepts the event in its current state, deliver
the paired notification to the furniture's registered scripts as well. Rejected
events are ignored. This does not echo ordinary PlayAnimation inputs and does
not forward payload-bearing internal notifications; autotest19's existing
notification policy is retained. Exact SE event-routing scope, ordering and
duplicate rules for paired furniture remain unmeasured. A probe should record
both actor and furniture registrations through loading and cutting a log.

Wait's StopMovement input is interpreted as interrupting an existing walk when
true; false lets that path continue while the Wait timer runs. A seated wait
retains its furniture either way, as StayAtCurrentLocation's branch requires.
The input does not displace a simultaneous moving procedure's control of the
body. These are interpretations of the authored input names; native movement
and seated timing still need comparison. CarryAndDropItem's unused wait inputs
are not inserted into its tree; CarryAndKeepItem's authored Wait reads them.
