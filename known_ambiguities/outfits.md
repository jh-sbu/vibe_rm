# Outfits changed by scripts

## What we do

- `Actor.SetOutfit(akOutfit, abSleepOutfit)` (vanilla `Actor.pex`; 82
  calls, nearly all on actors: followers' and quest NPCs' armor, the Civil
  War prisoners, `defaultAliasChangeOutfit`) gives the reference a new
  default outfit. The armor the old outfit gives (leveled lists resolved
  with the reference's seed, as when it was first equipped) is taken out of
  its inventory one each, unless the new outfit gives it too, and the new
  outfit's armor is added where missing and worn. The choice is the
  reference's, saved in optional `OUTF`; legacy saves have none, so actors
  wear their NPC's `DOFT`.
- A sleep outfit (`abSleepOutfit` true; `BYOHRelationshipAdoptionChildScript`
  only) does nothing: actors don't change for bed yet.
- `ActorBase.SetOutfit` sets the NPC record's outfit (saved in optional `BOFT`):
  actors of it with no reference-level outfit swap armor as above, and actors made
  later start with it; a reference-level outfit wins. The player is left alone.

## Unconfirmed

- Whether the game marks outfit items so that only those are taken away (a
  cuirass the player gave the actor that matches the old outfit's could go
  here), and whether the old outfit's items vanish or stay unequipped in
  the inventory (we take them away).
- Whether `Actor.SetOutfit` changes the reference only or its base (and so
  every actor sharing it); we change the reference. `ActorBase.SetOutfit` changes
  the record's, as its name says.
- Whether a new outfit's leveled lists are resolved again each time it is
  equipped (cell resets, respawns) or kept.
