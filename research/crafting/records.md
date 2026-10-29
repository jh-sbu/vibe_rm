# Crafting records and survey

Sources, read through the local UESP SQLite index:

- [COBJ revision 2645219](https://en.uesp.net/w/index.php?oldid=2645219),
  lines 2–38: CNTO input (form,u32 count), CTDA conditions, CNAM output,
  BNAM bench keyword, NAM1 u16 output count. The page says records appear to
  be keyed by editor ID; this is a source claim, not verified override behavior.
- [FURN revision 3098570](https://en.uesp.net/w/index.php?oldid=3098570):
  WBDT is two bytes, bench type and skill (FF for no skill). Keywords are KWDA.
- [ENCH revision 2652910](https://en.uesp.net/w/index.php?oldid=2652910),
  lines 28–57: ENIT has cost@0, flags@4, casting@8, charge@12, delivery@16,
  type@20, charge time@24, base enchantment@28, worn restriction FLST@32.
  The plan's WNAM label is inaccurate for disk ENCH: read ENIT offset 32.
- [ALCH revision 2308919](https://en.uesp.net/w/index.php?oldid=2308919),
  lines 35–61: DATA weight; ENIT value@0, flags@4 (manual 1, food 2,
  medicine 10000, poison 20000), addiction and sound. Auto value is the floor
  of summed effect costs. Effects are EFID followed by EFIT (f32 magnitude,
  u32 area, u32 duration), with any CTDA belonging to that effect.

INGR uses the existing magic-item effect parser and inventory value/weight.
MGEF DATA flags drive hostile/detrimental, no magnitude/duration and whether
power changes magnitude/duration. WEAP/ARMO EITM and EAMT plus existing template
resolution supply authored enchantments. Reuse the existing parsers.

## Reproducing the survey

`cargo build -p vrm-tools`, then `vrm-tool crafting-survey <Data>`.
It explicitly loads only the five base masters, excluding Creations. It prints
station keywords, recipe counts, condition function counts and attached script
events, including scripts on placed station references. PEX files are read in
memory through VFS and are never copied into this repository.

2026-10-28 results: forge 242 recipes, Skyforge 9, Dawnguard forge 10,
Aetherium forge 3, grindstone 204, workbench 319, smelter 16, tanning 17,
cookpot 23, oven 12, staff enchanter 43; Hearthfire has many additional
building keywords (including interior-part-specific benches).

Conditions: GetItemCount 1005, GetStageDone 30, GetGlobalValue 67, HasSpell 43,
GetInCurrentLoc 3, HasPerk 728, GetQuestCompleted 8, HasKeyword 1,
GetVMQuestVariable 38, EPTemperingItemIsEnchanted 523. The last is absent from
the current evaluator and must receive the item under consideration.

CommonLibSSE-NG public headers were consulted only for names/enums, after
checking refs/CommonLibSSE-NG/LICENSE (MIT, compatible with this project's MIT).
No implementation code or SKSE behavior is used as a specification.
`vrm-tool pex-natives <Data>` confirms GetItemHealthPercent and the existing
Ingredient learning natives; there is no vanilla native that performs crafting.

## Text settings

Quality: sHealthDataPrefixWeap1..6 and sHealthDataPrefixArmo1..6.
Potion/poison prefixes: sCreatedPotionNamePrefix / sCreatedPoisonNamePrefix.
Prompt: sCraft is present and equals Craft; original-game prompt observation
is still pending. No unknown-potion or new-recipe GMST was found by searching
all GMST names/values; translation strings or other records may supply them.
Keep those questions in the plan until resolved.

Seven vanilla placeholder recipes have no CNAM output and are excluded from the
usable recipe index: RecipeArmorSteelPlateShield, TemperArmorSteelPlateShield,
TemperArmorDA03Masque, and four TemperArmorGildedElven* records. Thus usable
forge/workbench counts are 241/313; the survey's raw counts are 242/319.

## Hearthfire's actual building-token chain (step 5c)

The authored assets refine the plan's hypothesis: BYOHHouseBuildingPlayerScript's
OnItemAdded calls UpdateLogCount (materials display), rather than consuming a
house token itself. Example COBJ BYOHHouseRecipe187Room11ARoof_3 outputs
BYOHHousePart187Room11ARoof. That MISC has BYOHBuildingObjectScript, which extends
Form and handles OnContainerChanged. Entering the player inventory from no
container calls BYOHhousebuildingscript.BuildHousePart using the current house
location and the token's authored IDs. The house quest script then dispatches
the building stage/room work to its selected house quest.

BYOHWorkbenchScript.OnActivate sets the active house location and calls its
linked BYOHHouseCraftingTriggerScript.StartCrafting. The trigger returns hidden
building items from a holding chest and manages stages. These are authored
scripts; support their ordinary events and natives instead of detecting house
recipe names in crafting code. Form-derived item scripts must be attached to
the base-form VM object before OnContainerChanged delivery. Reference-derived
scripts retain the existing placed/created-reference path.

Dragonborn's staff enchanter is WBDT Create Object with DLC2StaffEnchanter and 43
recipes gated by HasSpell. The same station-keyword/condition transaction path
handles it without staff-specific output rules.

A PEX event survey found no override of OnStoryCraftItem beyond the native Quest
base declaration. This does not establish which original-game actions emit CRFT;
keep that question for the manual test rather than infer absence from consumers.

Enchanting restrictions were checked against the authored records:
EnchArmorFortifyHealthBase ENIT offset 32 points to EnchantmentHealth, whose LNAM
members are keywords (e.g. ArmorShield and ClothingNecklace), not equip-slot
records. A matching item keyword permits that enchantment. Constant-effect
ENCHs are apparel; fire-and-forget ENCHs are weapons; staff type 12 is excluded.
The movie's enchanting categories use ordinary bit filters (Item 1/4,
Disenchant 2/8, Effect 16/32, Soul Gem 64), so partitioned filter mode is disabled
for enchanting and enabled for alchemy's packed effect IDs.


The offscreen Lakeview check on 2026-10-29 supplied the drafted small-house
layout (03003060), sawn logs and stone after initializing the building quest.
Creating foundation 03003014 called the real building-item script, advanced
BYOHHouseFalkreath through stages 120/130, completed its foundation objective
and hid that recipe. This verifies event delivery through the project's VM;
it is not observation in Skyrim SE. The house-purchase path was bypassed.

## Shared enchantments and worn effects

[Fortify Health revision 3494104](https://en.uesp.net/w/index.php?oldid=3494104),
line 58, describes multiple enchanted pieces contributing to total health
(with additional NPC combat behavior outside this correction). ENCH sharing
is base-form identity; it must not merge the active contributions from different
worn pieces. The engine now keys constant enchantment applications by both ENCH
and equipment base form, and saves that source with each effect. Unequipping
one piece removes its contribution while the other continues. This is the
project's internal representation, not a claim about original-engine storage.
