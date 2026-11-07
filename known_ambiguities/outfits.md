# Outfits changed by scripts

## What we do

- `Actor.SetOutfit(akOutfit, abSleepOutfit)` (vanilla `Actor.pex`; 82
  calls, nearly all on actors: followers' and quest NPCs' armor, the Civil
  War prisoners, `defaultAliasChangeOutfit`) gives the reference a new
  default outfit. The armor the old outfit gives (leveled lists resolved
  with the reference's seed, as when it was first equipped) is taken out of
  its inventory, every copy, unless the new outfit gives it too, and the new
  outfit's armor is added where missing and worn. The choice is the
  reference's, saved in optional `OUTF`; legacy saves have none, so actors
  wear their NPC's `DOFT`.
- `ActorBase.SetOutfit` sets the NPC record's outfit (saved in optional `BOFT`):
  actors of it with no reference-level outfit swap armor as above, and actors made
  later start with it; a reference-level outfit wins. The player is left alone.

## Unconfirmed

- Skyrim SE (autotest21, `research/autotest21.md`, Outfits): an NPC in
  outfit A (an iron cuirass) given a second iron cuirass had 2, worn;
  `Actor.SetOutfit(B)` left none and a leather cuirass worn, and back to A
  gave one iron cuirass. So given copies of the old outfit's armor go too
  (no outfit marking), and they vanish rather than stay unequipped. The
  other NPC of the same base was unchanged: `Actor.SetOutfit` is per
  reference, as ours. Unobserved: whether armor of the old outfit that the
  NPC no longer wears, or holds but doesn't wear, goes the same way.
- Whether a new outfit's leveled lists are resolved again each time it is
  equipped (cell resets, respawns) or kept.
