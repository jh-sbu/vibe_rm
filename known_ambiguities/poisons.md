# Poisons

Implemented in `src/poisons.rs`. Sources: the game's settings
(`sPoisonConfirmMessage`, `sPoisonBowConfirmMessage`,
`sPoisonNoWeaponMessage`, `sPoisonUnableToPoison`,
`sPoisonAlreadyPoisonedMessage`, `sPoisoned`), Concentrated Poison
(`00105F2F`: entry point 83, Mod Poison Dose Count, set to 2, so a dose
count of 1 without it), CommonLibSSE (`ExtraPoison` on one inventory entry
with a count, `ArrowProjectile::poison`, `PlayerCharacter::pendingPoison`,
the `PoisonedWeapon` event and its misc stat incrementer, Poisons Used),
UESP's Skyrim:Poisons (one hit per application; staffs can't be poisoned).

## Choices made without a source

- **Which hand**: the right hand's weapon, else the left's. With two
  weapons the game's "the current weapon" names one; which is unknown.
- **The question's wording**: the settings end before the name; the name
  and a `?` follow. For a bow it names the poison
  ("...your next shot with the <poison>?"), else the weapon. Whether the
  game adds more ("...with <poison>") is unknown.
- **Refusals** show as HUD notifications, not message boxes.
- **Bound weapons** take poisons like any other weapon; only staffs refuse.
- **Blocked blows** use a dose and apply the poison, as enchantments do
  here; whether the game spares the blocker is unknown.
- **Bows**: the dose leaves with the arrow as it is loosed and is lost if
  it misses (`ArrowProjectile::poison` suggests the arrow carries it).
  Crossbows the same.
- **One poisoned instance per weapon form**: two of the same sword carry
  at most one poison between them, and both hands' blows use it when a
  pair is held. The poisoned one goes last when some are taken away, but
  its poison is lost when it changes hands (dropped, stored, sold).
- **`sPoisoned`** ("You have been poisoned.") isn't shown: when the game
  shows it (struck by a poisoned weapon? a poison placed by a pickpocket?)
  is unknown.
- **`EquipItem` on a poison** (scripts, the console) does nothing; whether
  the game poisons the weapon, asks, or drinks it is unknown.
- **NPCs** never poison their own weapons; whether any do in combat is
  unknown.
- **The inventory** doesn't show which weapon is poisoned; the vanilla
  item card's strings name nothing of the kind, so whether the card or the
  name shows it is unknown.
- **The dose count's tabs**: Mod Poison Dose Count is given the weapon
  and the poison for its second and third tabs (Concentrated Poison has
  conditions on none).
