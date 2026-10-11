# Harvesting

Implemented in `src/harvest.rs`, with the pick shapes in `src/physics/mod.rs`
and the harvested models in `src/world/loader.rs`. Sources: the data
(`FLOR` and `TREE`: `PFIG` the produce, `SNAM` the sound, `RNAM` the verb,
`PFPC`); the models (plants' shapes on Havok layer 15, non-collidable; a
`NiSwitchNode` with the harvested look as its second child, as in the
cabbage and the Reach's mushroom stumps); the game settings `sHarvest`,
`iHoursToRespawnCell`; the game's scripts (`NirnrootACTIVATORScript` counts
Nirnroots Found itself, `critter` Wings Plucked).

## Unconfirmed: choices made

- **Picking.** The activation ray finds plants by their non-collidable
  (layer 15) shapes, which nothing else collides with; the nearer of that
  and what the ray hits otherwise is the target. Other non-collidable
  shapes aren't picked.
- **The produce** is always given: `PFPC` (four bytes, taken to be chances
  by season) is all 0 on the cabbage, which the game lets the player
  harvest, so it isn't used. One of `PFIG` is given (a leveled list
  resolved); Green Thumb and other perks on the count aren't applied.
  "<item> added" shows; Ingredients Harvested counts ingredients.
- **Owned plants** (by the reference or its cell) are stolen from: the
  prompt shows red and the produce is marked as the owner's, a crime if
  seen, counted in Items Stolen.
- **Harvested** plants show their harvested look (switch nodes on their
  second child; plants without one look the same) and offer no prompt. They
  grow back after `iHoursToRespawnCell` (240) game hours, in place of the
  cell respawning the game does.
