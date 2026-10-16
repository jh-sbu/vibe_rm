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

The inventory's weapon card now shows the game's poison icon. The installed
`Interface/inventory components/ItemCard.swf` asset and the embedded card code
in `inventorymenu.swf`, `containermenu.swf`, `bartermenu.swf` and `giftmenu.swf`
all read the card object's `poisoned` boolean in `ItemCard.__set__itemInfo`
and select `PoisonInstance`'s `On` or `Off` frame. This is asset inspection,
not inspection of executable engine data. The shared list renderer
`InventoryScrollingList.SetEntryText` has no corresponding poison field;
the existing card icon is used without inventing a list marker. The public
[Poisoning Extended SE description](https://www.nexusmods.com/skyrimspecialedition/mods/13368?tab=description)
also describes the existing snake icon and adds the poison name beside it.
The vanilla field exposes only a boolean, so this implementation does not
add a poison name or remaining dose count to the card.

The card reads the row's source inventory (including merchant stock and
received gifts), and applying or exhausting poison refreshes it even when
the item's count and equipped hands remain unchanged.

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
- **Poison icons on grouped weapons**: the runtime's form-level poison
  metadata has no ownership or individual-instance identity. A row's icon
  currently means that at least one copy of that weapon form in its source
  inventory is poisoned; if owned and unowned stacks of the same form are
  listed separately, both get the icon. The assets expose the boolean but
  do not resolve how the original engine splits or selects those instances.
  Correct instance grouping and poison-preserving transfers remain open.
- **The dose count's tabs**: Mod Poison Dose Count is given the weapon
  and the poison for its second and third tabs (Concentrated Poison has
  conditions on none).
