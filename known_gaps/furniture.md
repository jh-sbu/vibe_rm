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
