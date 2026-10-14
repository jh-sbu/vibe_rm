# Soul trap

Implemented in `src/soul_trap.rs`. Sources: UESP's Skyrim:Soul Trap and
Skyrim:Soul Gem (sizes by level, black souls for people, the smallest
gem that can hold it), the game's settings (`i<Size>SoulActorLevel` 4, 16,
28, 38; `sSoulCaptured`, `sSoulGemTooSmall`), the soul gem records (`SOUL`,
`SLCP`, `NAM0` the filled form, record flag 0x20000 on the black ones),
`magicSoulTrapFXScript` (calls `Caster.TrapSoul(Victim)` as the effect
finishes on a body, then plays the visuals if it returned true).

## Choices made without a source

- **Black souls**: actors with `ActorTypeNPC` (theirs or their race's);
  `NoSoulTrap` ones give none. Dremora and other `ActorTypeNPC` creatures
  count as people.
- **White souls in black gems**: allowed, but only when no other empty gem
  holds the soul.
- **Filling**: a gem filled to its size becomes its linked filled form
  (`NAM0`: the common ones, the black one); a smaller soul, or a gem with no
  linked form (petty, Azura's Star), keeps the soul with the empty gem
  (the game's per-item soul). Partly filled gems aren't refilled.
- **"No soul gem large enough."** shows when the caster has empty gems but
  none takes the soul; with no empty gem at all, nothing shows. A soul not
  trapped can still be taken by a later trap on the same body.
