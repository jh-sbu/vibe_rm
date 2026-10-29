# Plan: crafting

**Status: implemented** (2026-10-29); original-game comparison awaits the user-run
`crafting1` probe. Crafting connects ROADMAP's other systems: the Story Manager's craft event (item 2), the crafting perks' entry
points (item 5, Perks), alchemy teaching the effects two ingredients share and
the crafting statistics (item 6), and the Smithing, Alchemy and Enchanting skill
uses (item 5, Skills). This plan builds the game's five crafting menus at the
stations that open them:

| Station (`FURN` `WBDT` bench type) | Menu (`craftingmenu.swf` frame) | What it does |
| --- | --- | --- |
| Forge, tanning rack, smelter, cooking pot, oven, Dragonborn's staff enchanter, Hearthfire's building benches (1, Create Object) | `ConstructibleObject` | makes items from recipes (`COBJ`) |
| Grindstone and workbench (2 / 7, Smithing Weapon / Armor) | `Smithing` | improves (tempers) weapons and armor |
| Arcane enchanter (3, Enchanting) | `EnchantConstruct` | enchants items with known enchantments and a filled soul gem; destroys items to learn their enchantments |
| Alchemy lab (5, Alchemy) | `Alchemy` | mixes ingredients into potions and poisons |

`EnchantDestruct` is the movie's fourth frame. Which station opens it (if any
does in vanilla) is part of step 0.

What this plan leaves out, and why:

- **Getting into the station** (the player's sit/use animation and the
  furniture's idle). It waits on the player using furniture, which the player's
  body doesn't do yet (ROADMAP item 5, "The player's body"). Here the menu
  opens on activation and closes back to standing (step 4, recorded).
- **The item's 3D model beside the card** (`ShowItem3D`,
  `StartMouseRotation` / `StopMouseRotation`). It shares the inventory's
  `UpdateItem3D` work (ROADMAP item 6). Logged and answered as a no-op until
  that lands.
- **NPCs crafting.** NPCs only play the stations' idles (done, see ROADMAP
  Done). Nothing in vanilla has an NPC make an item.
- **Survival's and Creations' recipes.** They are Creation Club plugins, not
  vanilla. Their records load like any others, but nothing here is tested
  against them.

Rules as for all work here (AGENTS.md): public sources and game assets only, no
engine binaries; questions the running game can answer go to a test plugin
(`vrm-tool test-mod`), which the user runs. These need a manual one, since the
menus stop unattended scripts. Choices made without a source go in
`known_ambiguities/crafting.md`, open questions here or in ROADMAP. Run `cargo
fmt` before each commit. Each numbered sub-step is one commit.

## What the research found (2026-10-28)

Sources: the movie's ActionScript 2 (`swfspike dis` on
`spike/swf/data/interface/craftingmenu.swf`, folded with `fold.py`), the data
(`vrm-tool esp-list` / `esp-dump` / `gmst`), UESP (permanent links below),
CommonLibSSE-NG headers (MIT; their structure names and enums, used as
pointers, not code) and the engine's current code. Write this up as
`research/crafting/` in step 0, describing the movie's contract in prose and
tables rather than copying the disassembly.

### The menu's contract

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

### Records

- **`COBJ`** (UESP [Mod File Format/COBJ](https://en.uesp.net/w/index.php?oldid=2645219)):
  `COCT` + `CNTO` (item, count) inputs, `CTDA` conditions (perks, skill level,
  quest stage), `CNAM` output, `BNAM` bench keyword and `NAM1` count. The same
  record type serves recipes and improvements; the bench keyword decides which.
  UESP notes COBJ are keyed by editor id, not form id.
- **Bench keywords** (data): `CraftingSmithingForge` 88105,
  `CraftingSmithingSharpeningWheel` 88108, `CraftingSmithingArmorTable` ADB78,
  `CraftingSmelter` A5CCE, `CraftingTanningRack` 7866A, `CraftingCookpot`
  A5CB3, `CraftingSmithingSkyforge` F46CE, Dawnguard's `DLC1CraftingDawnguard`
  and `DLC1LD_CraftingForgeAetherium`, Hearthfire's `BYOHCraftingOven` and
  `BYOHHouseCraftingCategory*`, and Dragonborn's staff enchanter.
- **`FURN` `WBDT`** (UESP [Mod File Format/FURN](https://en.uesp.net/w/index.php?oldid=3098570),
  line 49): bench type 0..7 (None, Create Object, Smithing Weapon,
  Enchanting, Enchanting Experiment, Alchemy, Alchemy Experiment, Smithing
  Armor) and the skill it uses (an actor value, or 0xFF).
- **Help on first use**: default objects "Help - Basic Forging / Smelting /
  Tanning / Cooking / Object Creation / Smithing Weapon / Smithing Armor /
  Enchanting / Alchemy" (`HBFG`, `HBML`, `HBTA`, `HBCO`, `HBOC`, `HBSM`,
  `HBSA`, `HBEC`, `HBAL`; xEdit names, CommonLibSSE `kHelpBasic*`).

### Formulas (UESP)

- **Item quality** (tempering): [Skyrim:Smithing, Item
  Quality](https://en.uesp.net/w/index.php?oldid=3612986), lines 148–200.
  The quality levels are Fine through Legendary, with the skill each needs with
  and without the material's perk and the rating bonus for chest armor and for
  the rest. Higher levels continue past Legendary. Fortify Smithing applies to
  tempering directly. Arcane Blacksmith is needed to improve enchanted items
  (`Skyrim:Smithing#Arcane_Blacksmith`).
- **Smithing XP**: same page, lines 374–378: crafting gives 3 × value^0.65 +
  25, matching `fConstructibleSkillUseMult` 3, `fConstructibleSkilluseExp`
  0.65 and `fConstructibleSkillUseConst` 25. Improving goes by the value added
  (`fTemperingSkillUseItemValMult` 8, `...Exp` 0.6, `...Const` 25,
  `fTemperingSkillUseMult` 1). Smelting and tanning give none.
- **Potion strength**: [Skyrim:Alchemy, Formula](https://en.uesp.net/w/index.php?oldid=3596102),
  lines 152–160. Result = `fAlchemyIngredientInitMult` (4) × base magnitude ×
  (1 + (`fAlchemySkillFactor` 1.5 − 1) × skill / 100) × the Alchemist,
  Benefactor, Physician and Poisoner perks × Fortify Alchemy (enchantments) ×
  Seeker of Shadows. A negative result falls back to the base magnitude.
- **Enchantment strength and charge**: [Skyrim:Enchanting Effects, Effect
  Charges and Strength](https://en.uesp.net/w/index.php?oldid=3595887),
  lines 698–790, with `fEnchantingSkillFactor` 1.25,
  `fEnchantmentPointsMult` 0.12, `fEnchantmentEffectPointsMult` 8 and the
  ENCH cost formula it links. Disenchanting XP: `fDisenchantSkillUse`.
- **Enchanting rules**: [Skyrim:Enchanting, Skill
  Usage](https://en.uesp.net/w/index.php?oldid=3538992), lines 127–147.
  Disenchanting destroys the item and teaches its enchantment; items that
  can't be disenchanted (staves, artifacts, most unique items: keyword
  `MagicDisallowEnchanting`) don't appear. Enchanting needs an item, a known
  enchantment and a filled gem. Weapons get a charge/strength slider. Items
  can be renamed (32 characters on PC).

### Starting point before implementation

- Ingredient effects the player learned (`src/knowledge.rs`, saved in the
  world state); `Ingredient.LearnEffect` and its siblings.
- Soul gems with captured souls kept apart in inventories, weapon charges per
  copy, recharging (`src/recharge.rs`), enchanted weapon and armor effects
  from `EITM` (`src/spell_fx/enchant.rs`, `src/magic.rs`), potions and poisons
  used from the inventory, perks with entry points (`src/perks.rs`), skill uses
  (`src/skills.rs`), the Story Manager's `CRFT` event mapped to
  `OnStoryCraftItem` (`src/story.rs`), the crafting statistics' names
  (`src/stats.rs`), stations' idles for NPCs (`src/ai/furniture.rs`) and the
  item menus and item card (`src/swf_ui/item_menu.rs`).
- **Previously missing**: forms made while playing (the load order is mmapped plugins
  only, so a new potion or enchantment has nowhere to live); per-copy item
  quality, player enchantments and custom names; the player using a station;
  every crafting record and formula.

## Steps

### 0. Research and a survey

Steps 0–10 were implemented in the numbered commit order below, with each
lettered substep committed separately. See `research/crafting/` and the
verification record at the end. Open research: standalone EnchantDestruct
routing, the new-recipe translation string, original-game prompt and formula
rounding checks. DefaultPotion FULL supplies the unknown-potion name.

- `research/crafting/menu.md` (the contract above, with argument types and
  each frame's clips), `research/crafting/records.md` (`COBJ`, `FURN` `WBDT`,
  the bench keywords, the fields of `INGR` / `ALCH` / `ENCH` / `MGEF` used
  below) and `research/crafting/formulas.md` (each formula with its UESP
  permanent link and lines, its GMSTs from the data, and what it leaves open).
- `vrm-tool crafting-survey <data>`: stations by `WBDT` type and keywords; the
  number of `COBJ`s by bench keyword; the condition functions they use (should
  be `HasPerk`, `GetItemCount`, `GetGlobalValue`, `GetStageDone` and similar,
  all existing); and the scripts attached to stations, with the events those
  scripts handle (`OnActivate`, `OnSit`, `OnGetUp`, `OnItemAdded`). Settle here
  which station opens `EnchantDestruct`, and whether any vanilla station uses
  types 4 or 6 (Experiment).
- Find the GMSTs this plan still guesses: the quality names
  ("Fine".."Legendary"), the potion and poison naming, the default potion
  name for unknown effects, and the new-recipe format string.

### 1. Records

1a. **`COBJ`** in `src/world/crafting.rs` (`Recipe { id, inputs, conditions,
    output, count, bench }`), indexed by bench keyword at load; conditions run
    through `src/condition` with the player as subject.
1b. **Stations**: `FURN` `WBDT` (type, skill) and keywords, through the
    existing furniture records.
1c. **The fields crafting reads**: `INGR` effects (`EFID`/`EFIT`, the
    ingredient's value and flags), `ALCH` `ENIT` (flags: food, poison, ...),
    `ENCH` `ENIT` (cost, flags, type, charge, base enchantment, worn
    restrictions form list `WNAM`), `MGEF` flags (hostile, detrimental, power
    affects magnitude / duration, no-magnitude / no-duration), `WEAP` / `ARMO`
    `EITM` and enchantment amount, and template items (`CNAM`/`TNAM`) where
    the item card already resolves them. Reuse what `src/spells.rs` and
    `src/magic.rs` parse; add only what's missing.

Tests on synthetic records (`esp::RecordBuilder`) for each parser, and a survey
assertion on the real data (GPU-ignored fixture, as other data tests are).

### 2. Forms made while playing

Potions and enchantments the player makes are new base forms in the original
(form ids `FF......`). The engine needs them too, or every item path that
reads `lo.get(form)` would need a second lookup.

2a. **A runtime plugin in `esp::LoadOrder`.** Add a last, in-memory plugin
    (owned bytes in place of an mmap; `Plugin` gets an enum backing). It
    takes `RecordBuilder` records whose ids are allocated from the `FF`
    range, shared with `src/created.rs`'s counter so references and forms
    never collide. `get`, `tag_of`, `ids_of_type` and the strings lookup (its
    names are plain strings, not localized ids) see it like any plugin.
2b. **Saved** as an optional `FORM` section: the records' bytes in id order.
    Loading re-inserts them before anything that reads inventories. A save
    without `FORM` loads with none.
2c. **Sharing and cleanup.** Making the same potion again (same effects,
    magnitudes and durations) reuses its form, as the original does (players
    see crafted potions stack). Enchantments are shared the same way. A
    created form nothing holds any more stays until saving, then is dropped.
    Whether and when the original deletes them isn't public; record the choice
    in `known_ambiguities/crafting.md`.

Tests: insert, look up, save, load, reuse; an id never handed out twice across
references and forms; `FORM` missing.

### 3. Per-copy item state

Inventories keep counts by base form, with per-copy state kept apart (souls,
poisons, charges). Crafting adds three kinds, following the same pattern,
travelling with transfers and drops (`RemovedItems`) and carried by dropped
references (`src/ref_state.rs`):

3a. **Quality** (the original's `ExtraHealth`): `(item, quality, count)`, with
    quality 1.0 meaning untempered. Armor rating and weapon damage use it. The
    rating bonus by quality level comes from UESP's table: chest armor and the
    rest differ, and weapons use the "other" column. The value multiplier
    comes from the same table's value column. Damage and armor code in
    `src/ai/combat.rs` and the armor-rating path in `src/perks.rs` read it.
    `ObjectReference.GetItemHealthPercent` (a vanilla native) returns it for
    references.
3b. **Player enchantments and names** (`ExtraEnchantment`,
    `ExtraTextDisplayData`): `(item, enchantment, charge, name, count)`. Worn
    or wielded, the enchantment works as an `EITM` does today
    (`src/spell_fx/enchant.rs`), with its charge in the existing charge
    tracking.
3c. **The item card and lists.** A tempered item's name gets its quality
    ("Iron Sword (Fine)", the GMST names from step 0), and renamed items show
    their name. Values follow the quality and the enchantment. Inventory rows
    group by base form plus per-copy state, so a tempered sword and a plain
    one are two rows. This is the same grouping question as
    `known_ambiguities/inventory-cards.md`'s souls, solved here for all
    per-copy state.
3d. **Saved** in the inventory sections (optional fields or a new optional
    section). Older saves load with none.

Tests: transfer, drop and pick up keep each kind; armor rating and damage at
each quality level against UESP's table; card name and value; save round trip.

### 4. The menu shell

4a. **Opening.** Activating a `FURN` with a crafting `WBDT` type opens
    `Menu::Crafting { station, kind }` (`src/activation.rs`), before the
    "Sit" path. The prompt's verb (now "Sit") becomes the station's: `sCraft`
    "Craft" is the candidate GMST; confirm it in step 0 against what the
    game shows. The menu pauses the game like the other item menus.
4b. **`src/swf_ui/crafting_menu.rs`.** Open `craftingmenu.swf`,
    `Menu.gotoAndStop(<frame label>)`, then `Initialize` and `SetPlatform`.
    Answer `CloseMenu`, `CanFadeItemInfo`, the selection calls and the shared
    category/list calls (reusing `item_menu.rs`'s list and card code). Set
    `MenuName` / `MenuDescription` / `ButtonText` from the GMSTs. `ShowItem3D`
    and the rotation calls are logged no-ops. Confirmations go through the
    existing message box path and answer back into the menu.
4c. **Events.** `OnActivate` on the station's scripts (already sent on
    activation), and `OnSit` / `OnGetUp`, if step 0 finds station scripts that
    need them, sent as the menu opens and closes, since the player doesn't
    actually sit (recorded).
4d. **First-use help.** The station's help message (`HB*` default object) is
    shown the first time each station type opens, then marked shown (saved).
    How the original paces these (once ever, or once per game) is recorded
    as a choice.

Offscreen check: `--cell Riverwood --console "activate <forge ref>"` (Alvor's
forge; find the ref with `cell-refs`) and screenshots of the menu.

### 5. Making items (Create Object)

5a. **The list.** Recipes whose `BNAM` is one of the station's keywords and
    whose conditions pass, grouped by material category (the output's
    material keyword mapped to CommonLibSSE's filter flags, named by the
    output's keyword or, for Hearthfire, the `sGenericCraftKeywordName`
    GMSTs). The card shows the output, and `UpdateIngredients` shows the
    inputs with the player's counts. A recipe the player can't afford is
    listed and greyed (as the movie's colouring implies), with Craft off.
5b. **Crafting.** Confirm (`sConstructibleMenuConfirm`), then remove the inputs
    and add `count` × output. Stolen inputs are used first or last: a choice to
    record, since crafting with stolen goods doesn't make the output stolen in
    any source found. Then send `OnItemAdded` / `OnItemRemoved` (existing
    inventory events), the Story Manager's `CRFT` event (the station, its
    location and the item, per `src/story.rs`'s mapping), and the statistics
    (Weapons Made, Armor Made). Grant Smithing XP by UESP's formula with the
    output's value, at forges only (none for smelting and tanning). Food from
    a cooking pot grants none (no source gives cooking XP). Rebuild the list.
5c. **The staff enchanter** (Dragonborn) and **Hearthfire's benches**. These
    are Create Object menus over their own keywords. Hearthfire's building
    benches output `MISC` items whose Form-derived script handles
    `OnContainerChanged` and calls the building quest. The player alias also
    updates objective/log state on `OnItemAdded`. Check that chain with the
    survey's script list and an offscreen run at a house plot. Fix what the
    chain needs from natives and events, not by special-casing it.

Tests: recipe filtering by keyword and conditions (a perk-gated recipe); input
removal with counts and stolen copies; output count; XP by formula; the craft
event's arguments; the statistic.

### 6. Improving items (Smithing)

6a. **The list.** The player's weapons (grindstone) or armor (workbench) that
    have a recipe there: a `COBJ` whose `CNAM` is the item and whose `BNAM` is
    the bench's keyword, with conditions passing. Enchanted items are listed
    only with Arcane Blacksmith. Each row shows the quality it would reach.
    Use `SetPartitionedFilterMode` as step 0 finds the movie needs.
6b. **Quality.** The skill (with Fortify Smithing: the effect's magnitude
    applies to tempering directly, per UESP) picks the quality level from
    UESP's table, by whether the player has the material's perk (the recipe's
    `HasPerk` conditions name it). Above Legendary, levels continue at the
    table's steps. Improving never lowers an item. An item already at the
    reachable level isn't offered (Craft off).
6c. **Improving.** Confirm (`sSmithingConfirm`) and use one set of inputs.
    Change one copy's quality (3a), choosing the equipped copy first if the
    player holds one, else an untempered one: a choice to record. Grant XP by
    the value added, update the statistics (Weapons Improved, Armor Improved),
    and refresh the equipped item's effects.

Tests: quality level by skill, perk and Fortify Smithing at the table's
boundaries; Arcane Blacksmith gating; XP from value added; the copy chosen.

### 7. Alchemy

7a. **The list.** The player's ingredients, in effect categories: each known
    effect is a category (`MenuIngredientEntry`'s four effect filter ids), plus
    all ingredients. The card shows the ingredient's known effects and "?" for
    unknown ones.
7b. **Selecting.** `ChooseItem` toggles up to three ingredients. Ingredients
    sharing no effect with those selected are greyed (the usable-effect maps).
    The result is every effect shared by at least two selected ingredients.
    With Purity, it drops the negative effects of a potion and the positive
    ones of a poison. The card shows the result: known effects named,
    unknown ones as the "unknown potion". `bCanCraft` is set while the result
    has an effect.
7c. **Strength.** Per effect: UESP's formula, with the perks through their
    entry points (step 9), not hard-coded, and Fortify Alchemy. Magnitude vs.
    duration follows the effect's "power affects" flags. The strongest effect
    names the potion and decides potion or poison: a hostile strongest effect
    makes a poison. The value follows the effects' costs. Each piece of this
    with no source is recorded.
7d. **Mixing.** Remove one of each selected ingredient and add the potion: a
    created `ALCH` (step 2), shared when identical. Learn every effect in the
    result for each ingredient that has it (ROADMAP's "alchemy teaching the
    effects two ingredients share"). Grant Alchemy XP by the potion's value
    (source in step 0), and update the statistics (Potions Mixed, Poisons
    Mixed). A selection with no shared effect fails (`sPotionCreationFailed`)
    and the ingredients are lost: a choice to check against UESP's Alchemy page
    in step 0.
7e. **Leaving** with ingredients selected asks `sQuitAlchemy`.

Tests: shared-effect results for two and three ingredients; Purity; strength
at skill 15, 50 and 100 with and without perks against UESP's worked examples;
naming and poison choice; effects learned; identical potions share a form.

### 8. Enchanting

8a. **Disenchanting.** The Disenchant category lists the player's enchanted
    weapons and armor whose base enchantment isn't known, and isn't on an
    item with `MagicDisallowEnchanting`. Choosing one asks
    `sConfirmDisenchant`, destroys it, learns the enchantment (a saved set of
    known enchantments) and shows `sEnchantmentsLearned`. It also grants XP
    (`fDisenchantSkillUse` × the enchantment's cost, per step 0's source).
    Known ones show `sEnchantmentKnown`.
8b. **The categories.** Item (unenchanted, enchantable weapons and armor), the
    enchantments known, filtered to the chosen item's type (EffectWeapon /
    EffectArmor) and its worn restrictions (`ENCH` `WNAM`), and Soul Gem
    (filled gems, Azura's Star and the Black Star). Use
    `SetPartitionedFilterMode` and the filter flags as CommonLibSSE names
    them. Extra Effect allows a second enchantment (an entry point, step 9).
    Incompatible picks show `sEnchantArmorIncompatible`.
8c. **Strength and charge.** UESP's formulas. Armor enchantments get magnitude
    from skill, perks (Enchanter, Insightful, Corpus) and Fortify Enchanting.
    Weapon enchantments get charge from the gem's soul size, then use
    `ShowSlider` / `CalculateCharge` / `SetChargeValues` to trade magnitude
    against charges, and `SliderClose` keeps the value. The perks' parts go
    through entry points (step 9).
8d. **Naming.** `AuxButtonPress` → `EditItemName(current name, 32)`, then
    `EndItemRename(use, name)` stores the name, using the text input mode
    already used for the name popup.
8e. **Enchanting.** Confirm (`sEnchantItem`), consume the item copy and the gem
    (reusable gems are emptied, not consumed), and make a created `ENCH`
    (step 2) with the effects at the chosen magnitudes. Add the item with the
    enchantment, charge and name (3b). Grant XP (source in step 0) and update
    the statistics (Magic Items Made, Soul Gems Used). Leaving with choices
    made asks `sQuitEnchanting`. Missing parts: `sEnchantMustChooseItems`.

Tests: disenchant learning and refusal; category filtering by item type and
worn restrictions; magnitude at several skills and perks against UESP's
examples; weapon charge from each soul size and the slider's trade; gem
consumed vs. emptied; rename kept through save/load.

### 9. Crafting perks and effects

- From `vrm-tool perks`, list the vanilla Smithing, Alchemy and Enchanting
  perks and their entry points (CommonLibSSE `BGSEntryPoint::ENTRY_POINTS`:
  the potion magnitude / duration modifiers, enchantment power, soul gem
  charge, extra effect count, the Purity flag, Arcane Blacksmith).
  Implement each at the step that uses it, through `perk_entry_point` as the
  existing ones are, and add their numbers to `perks::ep`.
- The Fortify Smithing, Alchemy and Enchanting effects (actor values the
  formulas read) and Seeker of Shadows / Sorcery (Dragonborn) as the
  formulas specify.
- Update `known_ambiguities/perks.md`'s list of entry points waiting on
  crafting.

### 10. Saves, natives and the rest

- **Saved**: created forms (`FORM`, 2b), per-copy state (3d), known
  enchantments (8a) and first-use help shown (4d). The game is never saved
  while a crafting menu is open (`saving_disabled`).
- **Natives**: `ObjectReference.GetItemHealthPercent` (3a). `pex-natives`
  lists no other vanilla crafting native; check again in step 0.
- **Statistics**: the Crafting category's entries this plan fills in (Weapons
  Made / Improved, Armor Made / Improved, Potions / Poisons Mixed, Magic Items
  Made, Soul Gems Used). Update `known_ambiguities/journal.md`'s list of what
  the engine counts.
- **Story Manager**: the `CRFT` event (5b) also comes from the Smithing,
  Alchemy and Enchanting results if step 0's survey shows quests waiting on
  them. Otherwise only Create Object sends it, recorded.

## Test mod (manual, in Skyrim SE)

Crafting menus stop unattended scripts, but a manual test plugin can set up
the conditions and read the results afterwards with vanilla natives. Batch
these into one `test-mod crafting1`, with its report pages kept to about 20
lines each:

1. **Tempering.** The player at a grindstone and a workbench with fixed
   Smithing (15, 40, 57, 74, 100), with and without the Steel Smithing perk,
   and with a Fortify Smithing potion. The user improves an iron and a steel
   sword and armor at each. Then the script drops them (`DropObject`) and
   reports `GetItemHealthPercent` on each reference against UESP's table.
2. **Potions.** Fixed Alchemy and perks, given ingredients. The user mixes the
   listed pairs. A player alias's `OnItemAdded` reports the created potion's
   `GetGoldValue` and `GetFormID`; screenshots supply its name for comparison
   with 7c (`GetName` is absent from vanilla scripts).
3. **Enchanting.** Fixed Enchanting, a known enchantment and gems of each
   size. The user enchants an iron dagger at each gem and slider end and
   screenshots each item card (magnitude, charge meter). No vanilla native
   reads a player enchantment's magnitude or charge, so the cards are the
   evidence.
4. **Smithing XP.** From a fixed Smithing level, the number of iron daggers
   crafted before the skill rises (the script counts `OnItemAdded` and polls
   `GetBaseActorValue("Smithing")`), against UESP's formula and the skill's
   `AVIF` use values.

## Verification

- Unit tests per step, as listed; formula tests against UESP's worked numbers.
- Offscreen runs that open each station in Riverwood (Alvor's forge,
  grindstone and workbench, Riverwood's outdoor cooking pot), Whiterun
  (Arcadia's alchemy lab, Farengar's arcane enchanter) and a Hearthfire plot,
  with screenshots of each menu before and after crafting.
- `cargo test`, including the GPU-ignored tests here with `--include-ignored`.

## Documents

- New: `research/crafting/` (step 0) and `known_ambiguities/crafting.md` (no
  sit animation, created-form lifetime, which copy is improved, stolen inputs,
  potion naming and poison choice, failure cost, first-use help pacing, cooking
  XP, the `CRFT` sources).
- Updated: `known_ambiguities/perks.md`, `known_ambiguities/journal.md`
  (statistics), `known_ambiguities/inventory-cards.md` (per-copy grouping) and
  `known_ambiguities/saves.md` (`FORM` and the new state).
- ROADMAP: move each finished step to Done; remove "waits on crafting" from
  items 2, 5 and 6 as each is satisfied; add the sit animation and the 3D
  item preview where they belong (items 5 and 6) if not already there.


## Implementation and verification record (2026-10-29)

The numbered commit schedule is complete. The manual probe is generated at
`tests/assets/crafting1.esp` with its two scripts; instructions are in
`research/crafting/manual-test.md`. It has **not** been installed or run in
Skyrim SE. All comparisons reported here run this project's engine.

- Workspace tests with Vulkan: 786 passed, 366 ignored. The sandbox's default
  OpenGL adapter hits an existing 16-texture limit; the Vulkan rerun passes.
- Crafting integration/formula/save tests, including the installed vanilla perk
  audit and GPU-ignored crafting cases: 24 passed. Related charge/quantity
  checks and the manual probe's typed-call/dry-run test are included in the
  final verification notes in research/crafting/verification.md.
- Offscreen captures under ignored `tests/captures/crafting/` cover forge
  creation, weapon/armor tempering, cooking, alchemy at Arcadia's lab and the
  Sleeping Giant, and disenchanting/enchanting at Farengar's table. Hearthfire's
  controlled Lakeview setup supplies the house layout and required materials;
  foundation creation advances BYOHHouseFalkreath through stages 120/130 and
  completes its objective, then removes that recipe. This bypasses land
  purchase; it verifies the crafting script chain, not house acquisition. No visual comparison with the original game is
  implied.
- The movie adapter initializes its frame's clips before binding them, uses
  ItemCard.itemInfo, invalidates the tempering list, supplies skill/player
  bottom bars and follows live crafting key bindings. Recipe conditions are
  rechecked after activation scripts change quest variables.
- Per-copy rows include quality, enchantment, charge/capacity, name, poison,
  captured soul and ownership. Legacy sparse metadata is materialized in a
  deterministic ownership order; COPY v3 reads v1/v2. The engine's one selected
  equipped state per base form still limits independent dual-wield identity
  for differently modified copies of the same weapon; that broader follow-up
  is in ROADMAP.

The remaining original-game questions and the working decisions are separated
between this plan and known_ambiguities/crafting.md. Station use animations,
3D previews and Creation Club behavior remain outside this plan's scope.
