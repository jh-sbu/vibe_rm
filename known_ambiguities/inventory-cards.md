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
  established by a controlled game comparison.
- **Mixed souls and owners.** Crafting's copy rows now separate captured soul,
  authored/custom charge, poison, quality, enchantment and name within each
  ownership stack. Old saves lacked correlations between their sparse soul,
  charge and poison lists and ownership. Choice: materialize those legacy
  entries into player-owned copies first, then owned stacks in inventory order.
  New captures materialize immediately. Exact-row transfers, drops and pickup
  preserve this identity in both SWF and egui paths. Grand/black identity remains
  size-only as below.
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
  enchantments not).
- **Ties and singles.** Every item tied for the top is marked, and a lone item
  of its class is marked.
- **Scope.** The player's rows only (worn items included), in every item
  menu; the other side's rows are never marked.

## Ingredient cards' learned effects

The card's contract is in `ItemCard.swf` (`itemEffect0..3` set to the effect's
name when known, else `numItemEffects` slots read "$UNKNOWN"). Eating an
ingredient learns effects by the entry point `kModInitialIngredientEffectsLearned`
(72; Experimenter's ranks set it to 2, 3 and 4; the base is 1).

- **Unconfirmed: the label.** The effect's magic effect name alone (for
  example "Restore Health"), without magnitude or duration. Whether the card
  also shows those once learned is not in the SWF's inputs, which carry text only.
- **Unconfirmed: other ways to learn.** Besides eating and alchemy, whether
  anything teaches effects (some quest rewards, books) was not found in the
  data.

## Quantity selection in the egui fallback

The installed `spike/swf/data/interface/containermenu.swf` was inspected with
the permitted `swfspike dis` and `fold.py` tooling; output stayed in `/tmp` and
no asset or derived SWF code was added. Its `InventoryDefines` sets
`QUANTITY_MENU_COUNT_LIMIT` to 5. `StartItemTransfer` opens the movie's amount
slider above that limit, except that taking weightless items transfers all;
`onQuantityMenuSelect` passes the chosen amount through `ItemTransfer`. The
existing `src/swf_ui/item_menu.rs` honors that amount. This is asset behavior,
not a controlled comparison against a running Skyrim SE.

The egui fallback uses a modal chooser for stacks over five when moving,
pickpocketing or placing, and dropping. Shift still transfers or drops all.
The chooser keeps the source, destination, item and ownership row; its
confirmation re-reads that exact row, cancels if it is gone, and caps the
selected amount at what remains. Esc, Tab and Cancel dismiss only the chooser;
changing or closing the parent menu clears it. Pickpocket chance is refreshed
for the chosen amount. Unlike the SWF's weightless-taking shortcut, egui also
offers a chooser for those stacks. The original menu's modal appearance,
default slider position and exact keyboard focus behavior are not established.

Crafting adds separate rows for quality, player enchantment, remaining custom
charge, name and ownership. SWF transfer, gift, barter, drop and pickpocket paths
carry the selected copy state. Armor cards use the row's quality; weapon cards
include its exact quality bonus before rounding. The selected copy supplies charge
and poison to combat/recharge. One equipped-state slot per base form remains an
engine limitation for dual wielding differently modified copies of the same base;
that follow-up is recorded in ROADMAP rather than silently claiming per-hand identity.
