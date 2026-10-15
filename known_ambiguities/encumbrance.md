# Encumbrance

Implemented in `src/encumbrance.rs`. Sources: UESP's Skyrim:Carry Weight
(300 to start, +5 for each stamina level up, over it "limited to walking,
unable to run or sprint", no fast travel), the Conditioning and Unhindered
perks (entry point 32, Mod Armor Weight, set to 0 for worn armor of their
kind), `sOverEncumbered`.

## Choices made without a source

- **Over-encumbered speed**: the player's walk. UESP's page says both
  "limited to walking" and, elsewhere, "an almost unbearably slow crawl";
  the crawl may be worn armor's slowing on top of the walk.
- **Worn armor's slowing** follows Oblivion's formula (the Oblivion CS
  wiki's Movement Game Settings; Skyrim keeps the settings, with
  `fMoveWeightMin` gone, so taken as 0): speed x (1 - effect x worn
  weight / `fMoveWeightMax` (115)), weight past the max counting as the
  max; `fMoveEncumEffect` (0.3) with a weapon drawn, else
  `fMoveEncumEffectNoWeapon` (0.15). Oblivion's talk page disputes
  whether the drawn value is used. Only the player is slowed: NPCs' speed
  comes from their animations. Armor weighing nothing through Mod Armor
  Weight (Conditioning, Unhindered) doesn't slow; whether the game uses
  the perk-adjusted weight or a flag is unknown. Sprinting and sneaking
  take the same share. Manual test: time the player walking a fixed
  distance naked and in 50 / 115 / 150 weight of armor, sheathed and
  drawn.
- **The message** comes once, as the player becomes over-encumbered (not
  each time they try to run).
- **Weight compared**: over means more than `CarryWeight`, not equal.
