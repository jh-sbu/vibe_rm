# Crafting movie contract

Evidence: vanilla `interface/craftingmenu.swf`, examined using the local
`swfspike dis` and `fold.py` tools, 2026-10-28. The asset and its bytecode are
not part of these notes. This describes the interface, not engine behavior.


The movie's root `Menu` is the `CraftingMenu` clip. Its timeline has one frame
per sub-menu: `ConstructibleObject` 1, `Smithing` 2, `EnchantConstruct` 3,
`EnchantDestruct` 4 and `Alchemy` 5. A frame either has an `ItemListTweener`,
a single list (`MT_SINGLE_PANEL`), or `InventoryLists`, categories and items
(`MT_DOUBLE_PANEL`, the inventory's own class, with the same
`SetCategoriesList` / `InvalidateListData` callbacks that
`src/swf_ui/item_menu.rs` already answers).

| Menu calls (GameDelegate) | Meaning |
| --- | --- |
| `SetSelectedCategory(i)`, `SetSelectedItem(i)` | the highlighted category and row |
| `ChooseItem(i)` | a row pressed in the two-panel menus (select an ingredient, choose an item or enchantment or gem) |
| `CraftSelectedItem(i)` | a row pressed in the one-panel menu |
| `CraftButtonPress()` | the Craft button (R on PC), only while the game has set `bCanCraft` |
| `AuxButtonPress()` | the Aux button (F on PC: rename while enchanting) |
| `CloseMenu()` | Tab / Exit |
| `CanFadeItemInfo()` → `SetCanFadeItemInfo(bool)` | whether the card may fade when nothing is selected |
| `ShowItem3D(bool)`, `StartMouseRotation()`, `StopMouseRotation()` | the 3D item (out of scope) |
| `EndItemRename(useNewName, newName)` | the rename text field closed |
| `CalculateCharge(sliderValue)` → `SetChargeValues(...)` | the enchanting slider moved |
| `SliderClose(accepted, value)` | the slider's sub-menu closed |

| The game calls (on `Menu`) | Meaning |
| --- | --- |
| `Initialize()`, `SetPlatform(0, false)` | setup, as the other menus |
| `bCanCraft = bool`, `UpdateButtonText()` | whether Craft is offered; the bottom bar's labels (`ButtonText` per button) |
| `UpdateItemList(fullRebuild)`, `UpdateItemDisplay()` | redraw after the game changes the entries |
| `PreRebuildList()` / `PostRebuildList(restore)` | keep the category scroll and selection across a rebuild |
| `UpdateIngredients(title, [{Name, RequiredCount, PlayerCount}], showCount)` | the "requires" line under the card (grey when short) |
| `SetPartitionedFilterMode(bool)` | the items list's partitioned filter (enchanting's categories) |
| `EditItemName(text, maxChars)` | opens the rename field |
| `ShowSlider(max, min, current, snap)`, `SetSliderValue(v)` | the enchanting charge / magnitude slider |
| `MenuName`, `MenuDescription`, `AdditionalDescription` text | the header and help lines |
| `ItemInfo` (the `ItemCard`, as in the item menus) | the card |

CommonLibSSE's `CraftingSubMenus` headers name the same split and point at
what each menu keeps:

- `ConstructibleObjectMenu`: recipes with a material filter flag. The material
  keywords (Wood, Hide, Draugr, Iron, ... Stalhrim) map to the flags, plus
  eight `GenericCraftableKeyword` flags, which are Hearthfire's categories,
  named by GMSTs `sGenericCraftKeywordName01`.. "HOUSE", "CONTAINERS", ...
- `SmithingMenu`: improvable items with their recipe, and the bench's form
  type (weapon or armor).
- `EnchantConstructMenu`: category entries (Disenchant 0, Soul Gem 1, Item 2,
  Enchantment 3) with filter flags (EnchantWeapon 1, DisenchantWeapon 2,
  EnchantArmor 4, DisenchantArmor 8, EffectWeapon 0x10, EffectArmor 0x20,
  SoulGem 0x40). Enchantment entries carry a magnitude and a maximum
  magnitude.
- `AlchemyMenu`: ingredient entries with four effect filter ids, selected and
  greyed flags, up to three selected indexes, the usable effect maps, the
  result potion, an "unknown potion" for unknown effects, a new-recipe format
  string and whether the player has the Purity perk.

Message boxes go through `messagebox.swf` with GMST texts:
`sConstructibleMenuConfirm`, `sSmithingConfirm`, `sEnchantItem`,
`sConfirmDisenchant`, `sQuitAlchemy`, `sQuitEnchanting`,
`sEnchantMustChooseItems`, `sEnchantInsufficientCharge`,
`sEnchantArmorIncompatible`, `sEnchantmentKnown`, `sEnchantmentsLearned` and
`sPotionCreationFailed`. Headers and descriptions: `sConstructibleMenuDescription`,
`sSmithingMenuDescription`, `sEnchantMenuDescription`,
`sEnchantDeconstructMenuDescription` and `sAlchemyMenuDescription`. Categories:
`sWeaponEnchantments`, `sArmorEnchantments`, `sIngredients`, ...


## Argument types and clips

Selection indexes and slider values are numbers; craft, auxiliary, close and
rotation requests take no arguments. ShowItem3D and CanFadeItemInfo use booleans.
EndItemRename takes a boolean and string. SliderClose takes a boolean and number.
UpdateIngredients takes a string, an array of objects containing string Name and
numeric RequiredCount/PlayerCount, and a boolean. EditItemName takes string and
numeric limit. ButtonText is a four-string array (select, exit, auxiliary, craft).

The root is `_root.Menu`. Initialize binds ItemInfoHolder.ItemInfo,
BottomBarInfo, MenuDescriptionHolder.MenuDescription and InventoryLists.
Single-panel frames bind ItemListTweener.List_mc; double-panel frames bind
InventoryLists.ItemsList and its categories. SetChargeValues is the delegate
reply registered for CalculateCharge; its exact payload still needs a viewer
check. The card's unknown ingredient effect label is the translated `$UNKNOWN`.

## Station mapping

The vanilla survey counts WBDT types {1: 39, 2: 1, 3: 4, 5: 2, 7: 1}.
There are no Experiment (4/6) records. Type 3 includes the normal enchanter
variants, DisenchantmentFont01 and DLC2ApocryphaFountain, the latter two with
skill FF and no keywords. Records alone cannot prove which runtime path opens
the standalone EnchantDestruct frame. Keep this an open question; do not invent
a type-4 mapping from its label. Normal enchanters expose disenchanting in their
EnchantConstruct category list.

Station scripts found: BYOHHouseTrophyBaseScript (OnActivate, OnUpdate),
BYOHWorkbenchScript (OnActivate), BlacksmithForge01 (OnTriggerEnter, onActivate),
DLC1LD_AetheriumForge (OnActivate), defaultBlockFollowerActivation (OnLoad).
None defines OnSit/OnGetUp. Papyrus inheritance may add handlers, so this survey
is evidence about directly attached scripts, not proof about every event path.

## Activation events (step 4c)

The station survey found no directly attached OnSit or OnGetUp handlers.
Use the normal activation dispatcher, which sends OnActivate to reference and
alias scripts and checks BlockActivation before opening the crafting menu.
Scripted Activate as the player reaches the same station-opening helper after
activation-parent dispatch. No synthetic sit/get-up events are introduced here:
there is no physical player furniture transition in this implementation.
Inherited or quest-driven sit/get-up behavior remains a test-plugin question;
add dispatch only when authored scripts or original-game evidence require it.

Alchemy uses the movie's partitioned filter mode: each ingredient packs its
four known effect category IDs into four bytes of filterFlag. Category IDs are
1..255; the all-ingredients sentinel is FFFFFFFF. This comes directly from
ListFilterer.EntryMatchesPartitionedFilter in the authored movie. Keep effect
categories explicitly visible because the shared category visibility scan uses
ordinary bit masks rather than partitioned IDs.
