# Scroll and soul gem inventory cards

Implemented in `src/swf_ui/item_menu.rs`. No engine executable, engine binary
data, leaked documents, or SDK-source mirrors were examined. No game assets
or copies of their disassembly were added to the repository.

## Asset evidence

Read the installed game's `Interface/Inventory Components/ItemCard.swf`
using the repository's `swfspike dis` and `fold.py` tools. The extracted SWF
already existed under ignored `spike/swf/data`; inspection output was written
only to `/tmp`. Its `InventoryDefines` assigns `ICT_SPELL` 7 and
`ICT_SOUL_GEMS` 12. `ItemCard.itemInfo` selects the spell layouts using
`castTime == 0` (the per-second layout), fills the casting rank from
`castLevel`, the effect description from `effects`, and the cost from
`spellCost`. `ICT_SOUL_GEMS` selects `SoulGem` and copies `soulLVL` directly
to its soul text field; a number alone would therefore display a number,
not a localized soul name.

The inventory movie's `UpdatePerItemInfo` selects its school meter for
`ICT_SPELL` and reads `magicSchoolName`, `magicSchoolLevel`, and
`magicSchoolPct`. These are supplied using the existing spell-school
helpers; schoolless scrolls clear the fields explicitly.

Inspected only the `GMST` group of installed `Skyrim.esm` to confirm setting
names: `sEmpty` and `sSoulLevelNamePetty`, `sSoulLevelNameLesser`,
`sSoulLevelNameCommon`, `sSoulLevelNameGreater`, `sSoulLevelNameGrand`.
Soul values follow the already implemented `SLGM.SOUL` and captured-soul
metadata representation: 0 empty, 1 petty through 5 grand. The card reads
the row's actual source inventory, including a merchant's stock chest and
gifts received from an actor, rather than always the player's inventory.

## Unresolved behavior and conservative choices

- **Exact native scroll payload.** The SWF establishes the fields and
  layouts, but does not prove which card type the original game assigns to
  scrolls or what it puts into each field. The roadmap's intended
  `ICT_SPELL` path is implemented. Scrolls use the existing effect text,
  spell rank, school, and concentration classification. `castTime` is a
  layout selector (0 concentration, 1 otherwise), not `SPIT` charge time.
  Their cost is shown as 0: [public player accounts describe scrolls as
  consuming the item instead of magicka](https://www.reddit.com/r/skyrim/comments/sutjpe/),
  but the original card's exact appearance and cost label have not been
  established by a controlled game comparison. Equipping/casting scrolls
  is a separate unfinished roadmap item.
- **Mixed souls and owners.** Inventory rows still group by base form and
  ownership; captured souls aggregate by base form and soul size without
  ownership identity. A whole form's homogeneous soul state is safe to show
  on each ownership row. When that form mixes empty and filled gems or
  several soul sizes, the card shows an em dash instead of asserting that
  the selected ownership row is empty or has a particular soul. Invalid
  levels also show the dash. This placeholder is an implementation choice,
  not an original-game string. Splitting soul-size rows requires instance
  identity in selection and transfer operations; it is not implemented.
- **Black soul labels.** The current soul metadata stores size only;
  grand white and black souls both use value 5. This card therefore shows
  the grand label for either. Whether the original card distinguishes them
  by a separate label remains unconfirmed; gem capacity and its ability to
  hold black souls are not substituted for the captured soul's size.
- **School meter refresh.** Soul-state changes participate in the list's
  refresh snapshot, so filling a gem in place updates its card. Scroll
  school level/progress refreshes with ordinary list invalidation, as the
  existing magic-menu card does; script-driven skill changes while the
  inventory remains open have not been compared against the original game.

Five synthetic tests cover scroll payload metadata and concentration layout,
zero magicka cost despite an authored magic-item cost, localized soul labels,
base and captured souls, mixed/invalid state, every inventory source, and
empty-to-mixed-to-filled refresh snapshots with unchanged row identity/count.
They validate the implementation's contract, not the original game's UI
appearance.

## Best-in-class mark

Implemented in `src/swf_ui/item_menu.rs` (`best_in_class`). The game's
`InventoryScrollingList.SetEntryText` (read in the Interface SWFs' AS2, shared
by the inventory, container, barter and gift menus) appends
`<img src='BestIcon.png' vspace='2'>` to the text of an entry whose
`bestInClass` is `true`; the menus' own art draws it, so the entry only
carries the flag. The icon is a hollow triangle/diamond, not a star.

Public community accounts (the Elder Scrolls fandom wiki's menu page; a Nexus
mod description of the vanilla "BestInClass" feature; a forum post) say it
marks, among the player's items, the best weapon (not staves) or apparel (not
jewelry) of its type by armor rating or damage, with roughly ten classes in
vanilla, and ignores enchantments and perks. No source gives the classes, the
exact number compared or the tie rule, so:

- **Classes.** A weapon's animation type (sword, dagger, axe, mace, two-hand
  sword, two-hand axe/hammer, bow, crossbow), an armor's slot (body, hands,
  feet, shield, head). That is 13 classes, not ~10; light and heavy armor
  are compared together (the mod above says vanilla does and splits them in
  its 24). Jewelry, cloaks, circlets and the rest, staves, fists and ammo are
  never marked.
- **What is compared.** The card's own numbers: base weapon damage; an
  armor's rating through `protection_of` (skill and armor perks included,
  enchantments not). Tempering and poisons are not tracked on items yet.
- **Ties and singles.** Every item tied for the top is marked, and a lone item
  of its class is marked.
- **Scope.** The player's rows only (worn items included), in every item
  menu; the other side's rows are never marked.
