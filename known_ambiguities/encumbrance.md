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
  whether the drawn value is used. Armor weighing nothing through Mod Armor
  Weight (Conditioning, Unhindered) doesn't slow; whether the game uses
  the perk-adjusted weight or a flag is unknown. Sprinting and sneaking
  take the same share. Manual test: time the player walking a fixed
  distance naked and in 50 / 115 / 150 weight of armor, sheathed and
  drawn.
- **The message** comes once, as the player becomes over-encumbered (not
  each time they try to run).
- **Weight compared**: over means more than `CarryWeight`, not equal.

## NPC worn armor

Loaded NPCs now use the same worn-armor multiplier as the player for package
walk / fast walk / jog / run, sneaking, follow / escort catch-up, combat
chasing (including ranged / magic positioning), and fleeing. The multiplier
is refreshed before each AI update from current equipment, Mod Armor Weight
perks, and the actor's weapon-drawn state. Authored movement-type and clip
speeds remain unchanged; displacement and the speed fed to animation are
scaled once. Footstep gait thresholds use the same multiplier so a running
actor remains classified as running in heavy armor. Furniture animation root
motion, the final seat alignment, and collision separation are not scaled.

Public evidence supports armor slowing movement and weight-removing perks
removing that penalty:

- [Prima's published Skyrim guide, Skill Constellations](https://primagames.com/eguides/elder-scrolls-v-skyrim-eguide/essentials/training/skill-constellations)
  explains Conditioning and Unhindered as removing the worn armor's weight
  and speed penalty. It does not give a numerical movement formula or an
  NPC-specific rule.
- [UESP's Oblivion Construction Set movement settings](https://cs.uesp.net/w/index.php?title=Movement_Game_Settings&oldid=26014)
  documents weight-based speed multipliers, including creature movement,
  the maximum weight cap, and drawn / sheathed settings. This is evidence
  for **Oblivion**, not confirmation of Skyrim's calculation.

**Unresolved Skyrim behavior:** extending the existing player formula to
NPCs is an implementation policy, not measured parity. No public source
consulted established whether Skyrim NPCs use that exact formula, whether
all locomotion modes share the multiplier, or whether creatures with worn
armor differ. The original engine's refresh timing after an equipment,
perk, or weapon-state change is also unverified. Weapons' weight remains
excluded, as in the existing player policy; the Oblivion reference includes
weapons, which is not sufficient evidence to change Skyrim behavior.

To resolve this without examining engine binaries: in an unmodified game,
time the same NPC following a fixed straight travel package with no armor
and synthetic 50 / 115 / 150-weight armor; repeat drawn / sheathed, sneaking
and running, then repeat with Conditioning / Unhindered. Compare distance
per second and verify when changes take effect. No game assets or engine
binary data were inspected or added for this implementation; the automated
tests use synthetic records, generated actor state, and an empty navigation
world.
