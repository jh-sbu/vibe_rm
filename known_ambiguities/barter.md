# Bartering

Implemented in `src/barter.rs` (the menu is `bartermenu.swf`:
`known_ambiguities/swf-hud.md`). Sources: UESP's Skyrim:Speech page for the price
formula and the Haggling / Allure numbers, the game settings `fBarterMin` and
`fBarterMax`, the vendor factions in Skyrim.esm (`FACT` `VENC`, `VENV`,
`VEND`), the Haggling perk's own entry points (8 Mod Buy Prices, multiply
0.91; 60 Mod Sell Prices, multiply 1.1), the `OfferServicesTopic` fragments
calling `Actor.ShowBarterMenu`, Fortify Barter's effects (`AlchFortifyBarter`:
a peak value modifier on `SpeechcraftPowerMod`, which the player's
`AlchemySkillBoosts` perk turns into Mod Buy Prices x (1 - 0.01 x it) and
Mod Sell Prices x (1 + 0.01 x it), so it needs nothing of its own), UESP's Skyrim:Merchants page (merchants
restock every 48 hours) and the game setting `iDaysToRespawnVendor` (2). `VENV`'s
last two bytes are unused, as UESP's FACT page has them: fences with 0 and
1 in the third bought alike in the game (`test-mod venv`).

## Unconfirmed: choices made

- **What a merchant won't buy isn't listed** on the player's side (stolen
  goods unless the faction buys them, items without a keyword of the list, or
  with one when the list is inverted), rather than listed and refused.
- **The perks' tabs.** Mod Buy / Sell Prices are asked once per menu, with no
  item and the merchant as the third tab's subject (Allure's opposite sex).
- **Speech XP** is the gold that changed hands, through `use_skill` (the
  skill's use multiplier applied).
- **Stolen goods sold to a fence** keep their owner in the merchant chest,
  as moved items do; buying them back, they are still stolen.
- **A merchant short of gold** pays what they have, once the player accepts
  the warning.
- **The merchant's hours** (`VENV`) aren't checked on opening: the scripts
  that call `ShowBarterMenu` come from topics whose conditions check them
  (`GetOffersServicesNow`).
- **Restocking.** A merchant chest is restocked when the barter menu opens
  and `iDaysToRespawnVendor` days have passed since it was last stocked; a
  chest is first stocked when first traded with, not at the start of the
  game. Restocking replaces the chest's whole inventory with its container's
  items, leveled lists rolled again: what the player sold to it, and items
  scripts added to it, are gone, and its gold is back to the container's.
  Merchants without a chest (selling what they carry) aren't restocked.
- **Investor and Master Trader** work through the data: the perks' quest
  stage (Master Trader) and the Investor topics' fragments set globals that
  are the chance of none (`LVLG`) of gold lists in the merchant chests, so
  the gold comes with the chest's next stocking, not at once. Leveled lists
  read a global's value as the scripts have set it, else its record's.
