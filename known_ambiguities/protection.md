# Essential, protected, invulnerable and ghost actors

## Public sources

- UESP [Skyrim Mod:Mod File Format/QUST, revision 3507775](https://en.uesp.net/w/index.php?oldid=3507775),
  Aliases: reference alias `FNAM` 0x40 "Essential (makes the alias target
  essential for the duration of the quest)" and 0x400 "Protected".
- UESP [Skyrim Mod:Mod File Format/NPC, revision 3650076](https://en.uesp.net/w/index.php?oldid=3650076):
  `ACBS` flags 0x02 Essential, 0x800 Protected, 0x20000000
  "Ghost/non-interactable (Ghosts, Nocturnal)", 0x80000000 Invulnerable.

## Quest aliases' flags

- A reference filling a reference alias with `FNAM` 0x40 (0x400) of a
  running quest is essential (protected), whatever its base says; clearing
  the alias or stopping the quest ends it. Nothing is stored: the flags are
  read from the current fills, as alias factions are. `IsEssential`,
  `IsProtected`, `Kill`, damage and bleeding out all read the same answer.
- The player-only rule for the protected is the base flag's: the player can
  kill them, others only bring them down.
- Unconfirmed: whether "for the duration of the quest" means while the
  alias is filled (our reading) or until the quest stops even if the alias
  is cleared, and whether an alias flag counts for a quest that has started
  but whose start-up stage hasn't run. Whether a flagged alias pointing at a
  base that is already essential changes anything (we answer no).
