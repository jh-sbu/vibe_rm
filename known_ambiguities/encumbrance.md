# Encumbrance

Implemented in `src/encumbrance.rs`. Sources: UESP's Skyrim:Carry Weight
(300 to start, +5 for each stamina level up, over it "limited to walking,
unable to run or sprint", no fast travel), the Conditioning and Unhindered
perks (entry point 32, Mod Armor Weight, set to 0 for worn armor of their
kind), `sOverEncumbered`.

## Choices made without a source

- **Over-encumbered speed**: the player's walk. UESP's page says both
  "limited to walking" and, elsewhere, "an almost unbearably slow crawl";
  worn armor slowing the wearer isn't modelled yet, which may be the crawl.
- **The message** comes once, as the player becomes over-encumbered (not
  each time they try to run).
- **Weight compared**: over means more than `CarryWeight`, not equal.
