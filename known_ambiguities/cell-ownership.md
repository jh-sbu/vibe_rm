# Which exterior cell a loaded actor belongs to

Implemented in `Engine::rehome_actor` (`src/ai/schedule.rs`, 2026-11-08), with
loose objects in `hand_over_loose` (`src/loose.rs`; `loose-objects.md`).

## Evidence

- Vanilla `MQ101` drives the player, the two prisoner carts, their horses
  and Hadvar on his horse from cell (5, -24) to beyond (5, -20). That is more
  than four cells, past where cell (5, -24) unloads (radius plus one). In
  SE the cast doesn't vanish on the way, so whatever SE does, the actors and
  carts it drives are not unloaded with the cells they started in.
- Before this, actors stayed with the cell they spawned in, except commanded
  actors (`update_commanded`, which respawns them in the new cell). Horse 2
  and Hadvar's horse unloaded beside the player at about 122 s into the
  ride, and came back where the schedule last had them.
- No public source describes when SE moves an actor to another cell (the
  UESP dump has no `GetParentCell` page for Skyrim; the CK wiki is out of
  reach for agents).

## Choices made

- **Every frame**, a loaded actor in an exterior cell whose position is in
  another loaded exterior cell moves to that cell's lists as it is (its
  runtime, render instance and capsule): no respawn, its walk or ride goes
  on. Walkers, riders and their mounts, and cart passengers alike.
- **Out past the loaded cells**, it stays with the cell it is kept in, and
  goes away when that unloads, as before.
- **Interiors** are one cell; nothing changes there.
- `GetParentCell`, cell attach/detach events and `OnCellLoad` timing for an
  actor that changed cell this way haven't been checked against SE.
