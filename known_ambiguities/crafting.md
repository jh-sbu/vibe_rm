# Crafting choices

## Runtime form identity and lifetime (steps 2a–2c)

Research: UESP ALCH/ENCH format pages (permanent links in
`research/crafting/records.md`) describe effects but not deletion timing.
Public gameplay evidence is stacking of identical player-created potions.
Choice: intern generated forms by non-name fields and sorted effect pairs;
record IDs and FULL/EDID do not distinguish them. IDs share the created-reference
allocator and are never reused during a game.

Collect unreachable forms when serializing a snapshot and compact the live
runtime plugin only after the save has been written successfully. Collection
conservatively treats any matching four-byte handle in saved state as a root,
excluding FORM itself and thumbnails, and follows matching handles in retained
records. This covers VM arrays, story memory and future optional sections as
well as inventories without maintaining a second, easily incomplete root list.
Accidental byte matches can retain an otherwise unused form, but cannot delete
one still mentioned by a save. This is a deliberate conservative lifetime choice;
the original's collection timing and root algorithm are not established.

## Quality and health (step 3a)

Research: Smithing revision 3612986, lines 264–273 gives exact rating bonus
(3.6 * level - 1.6), halved outside chest armor, with rounding at display time.
This conflicts with some rounded entries in the earlier summary table. Use the
explicit formula for calculations, ceil armor display, round weapon display.
The local UESP index has no ExtraHealth article; GetItemHealthPercent appears
only in function lists. No permitted source established its encoding.

Provisional choice: store health as 1 + quality level / 6, the documented value
multiplier; untouched copies return 1. This is an internal encoding, not a claim
about Skyrim's ExtraHealth bytes. The manual plugin must check GetItemHealthPercent
before calling this native compatible for tempered copies. Quality ratings and
value multipliers do not depend on the unresolved external encoding.

Unspecified removals use unmodified copies before modified ones within each
ownership stack. Explicit row selection will select the copy state. Metadata
keeps its ownership; it must not migrate between stolen and owned stacks.

## Display value of player enchantments (step 3c)

[ARMO revision 2308925](https://en.uesp.net/w/index.php?oldid=2308925),
lines 91–93 documents base value plus rounded enchantment cost for armor.
Use the created enchantment's stored cost as the additive value for now, also
for weapons: the consulted ENCH/WEAP pages did not establish the player-created
weapon value rule. Weapon value remains a provisional choice to compare in the
manual enchanting test.

## Entering a station (step 4a)

The player's furniture animation isn't implemented. A supported WBDT station
opens its crafting menu immediately after normal OnActivate processing and
BlockActivation checks, and closes back to standing. This uses the authored
station type and keywords. Experiment types remain unsupported: the vanilla
survey found none, and their movie routing remains an open research question.

## First-use help (step 4d)

DOBJ supplies the nine HB* help messages, but the station survey and movie do
not establish repetition cadence. Choice: each help key is shown once per
character, for ten real seconds, with shown keys persisted in optional CHLP.
Missing default messages do not consume their shown flag. Cooking and ovens
share cooking help; unclassified Create Object stations use object-creation help.

## Recipe transactions (step 5b)

UESP's COBJ and Smithing pages establish recipes, materials and value-based XP,
but do not specify ownership order or stolen output propagation. Choice: consume
owned materials first, then stolen ownership stacks in their inventory order;
the output belongs to the player. Conditions and counts are rechecked when the
confirmation is accepted. For forge recipes XP uses the combined output value.
Cooking, smelting and tanning grant none. CRFT carries station R1, its current
location L1 and output F1, following the existing Story Manager argument mapping.
Create Object weapon/armor statistics count output items rather than button presses.

The extended skill threshold table also disagrees with the explicit formula
(e.g. quality 7 without a perk is listed at 202, while floor((skill+38)*3/103)
reaches 7 at 203). Use the explicit formula consistently, with the documented
minimum tempering threshold of 14, pending original-game boundary tests.

## Improving copies and XP (step 6c)

An explicit metadata row selects one copy in that row. When an identical copy
is equipped, improvement updates that equipped selection first; other copies
retain their state. This replaces the plan's form-only equipped/plain preference
now that rows identify metadata. Improving changes metadata in place and keeps
ownership, name, enchantment and charge. Ingredients use the same ownership
order as Create Object.

UESP Smithing revision 3612986 line 366 establishes value-added XP, while the
local index and additional public search did not establish its exact equation.
Provisional choice: (8 * valueAdded^0.6 + 25) * 1, using the four tempering
GMSTs. Verify against the manual plugin before calling the exact XP compatible.

Later research on the same page, line 447, explicitly confirms that stolen
components do not mark the result stolen, and improving/enchanting retains the
item's stolen status. Only material consumption order remains a choice.

## Alchemy experimentation (step 7b)

UESP Alchemy revision 3596102 lines 146–148 describes failed-combination greying,
not omniscient filtering by hidden shared effects. Only previously failed pairs
are greyed. Two or three distinct selected ingredients enable Craft even if no
shared effect exists, so experimentation can fail without revealing unknown
compatibility in advance. This corrects the plan's conflicting shared-effect
Craft gate versus failed-mix requirement. Highest source effect cost chooses an
ingredient's entire effect parameters; equal costs use deterministic form/order
ties pending original-game tests (Alchemy Effects revision 3536120 lines 9–14
says cost priority is believed, rather than verified).

## Potion naming, polarity and value (step 7c)

Use the highest-cost resulting effect's localized name with the authored
sCreatedPotionNamePrefix/sCreatedPoisonNamePrefix. Ties follow effect form-ID
order. Hostile dominant effects make poisons. Purity first filters using the
unscaled dominant effect; the scaled result then determines its final label and
polarity. The sources establish strongest-effect naming and Purity, but not this
tie rule or the precise mixed-effect order; these remain manual-test choices.

Magnitude and duration follow the documented power/no-value flags and round
separately. Positive hidden durations participate in value, even below ten
seconds. The final value floors the sum of exact costs, following ALCH format
notes rather than summing individually floored effect costs from the effect
page's illustrative calculation.

## Mixing and failed attempts (step 7d)

Mixing uses one of each selected ingredient, material ownership as above, and
teaches each retained shared effect on every selected ingredient carrying it.
Identical generated effect records reuse one runtime ALCH. Values supply the
Alchemy skill use input, so AVIF controls its XP multiplier.

UESP Alchemy revision 3596102 line 410 describes small failed-mix XP but does not
supply its exact use value. Choice: a failed attempt consumes the selected
ingredients and grants one base skill use. Failed pairs are recorded in ALFL;
a completely failed three-ingredient attempt marks all three pairs failed.
The exact failure use value and whether a triple teaches all pair failures
remain manual-test questions. Potions use DefaultPotion/DefaultPoison art,
keywords, pickup/drop sounds and consume sound, with weight 0.5.
