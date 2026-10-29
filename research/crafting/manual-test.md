# Manual crafting probes

Build with `cargo build -p vrm-tools`, then
`target/debug/vrm-tool test-mod crafting1 "<Skyrim SE>/Data" tests/assets/crafting1.esp`.
Generated ESP/PEX files go only to
ignored `tests/assets/`. The developer does not install them or run Skyrim.
Run in vanilla Skyrim SE on a disposable character/save, without SKSE or
unofficial patches. Remove crafting equipment, stones and sleep bonuses;
record any remaining bonuses. The probes replace test gear and change skills
and perks. Do not continue playing from the resulting save.

Use the quest console functions below (`cqf VrmCrafting1Quest ...`). Close
the console before crafting. The reports remain in the quest/player-alias
script variables and can be recovered with `vrm-tool test-results tests/results`
after copying the resulting save there. No native reads a player's created
enchantment or an item's display name; use screenshots for those observations.
`Form.GetGoldValue`, `Form.GetFormID`, `DropObject` and `GetItemHealthPercent`
are declared by the installed vanilla scripts.

## Tempering

Run `Temper15`, `Temper40`, `Temper57`, `Temper74`, `Temper100` in turn. Each
replaces the two swords and two cuirasses with fresh copies and sets Smithing.
Improve all four (iron/steel sword at a grindstone, iron/steel cuirass at a
workbench), close the menu, then run `ReadTemper`. If an improvement is refused,
leave that copy untouched; its health should remain 1. Each report is six lines.
Repeat with `Steel15`, `Steel40`, `Steel57`, `Steel74`, `Steel100`; these add
Steel Smithing. Finally `Fortify100` supplies a Fortify Smithing potion: drink
it, record the card's percentage and remaining duration, improve and read.

Compare displayed quality and reference health against the UESP tables and
`research/crafting/formulas.md`. These measurements settle the internal
health-to-quality mapping, table/formula rounding differences and Fortify input.
An item dropped by the probe is deleted after reading; screenshot its card first.

## Potions

Run `Potions 15 false`, mix Wheat + Blisterwort once, and screenshot its name,
effects and value. Repeat at 50 and 100, then with `true` for the same three
skills (Alchemist rank 5, Physician and Benefactor). The player alias records
each added potion's numeric form ID and gold value, capped at eighteen entries.
Keep the creation order with the screenshots. Record value again after reload
if it changes. Use a fresh character without other Alchemist ranks for the
`false` cases; the probe only removes the three perks it grants.

## Enchanting

Run `Enchanting`. Disenchant the fire sword provided, then enchant fresh iron
daggers with fire using petty, lesser, common, greater and grand souls, at
both ends of the strength slider. Screenshot each result's magnitude, value
and charge meter. Rename one, save/reload and screenshot it again. Repeat a
grand-soul creation with Azura's Star or the Black Star if available; record
whether the gem stays and becomes empty. Compare XP before/after one creation
and disenchantment in the skills screen. This probe does not claim those
unreadable quantities were measured by script.

## Smithing XP and saving

Use a separate fresh character for this page, with no progress already earned
toward Smithing 16 (setting its level cannot reset that progress through vanilla
natives). Run `SmithingXP`, then make iron daggers until Smithing first increases from
15. The alias counts OnItemAdded and reads base Smithing after each dagger;
the report contains the first eighteen readings and the final count. Compare
with the value-based formula and AVIF use values. Run `Finish` outside a menu
to request a save. Copy that save into `tests/results/` for extraction.

These probes have been generated and checked by the project tooling, but have
not yet been run in Skyrim SE. Preserve that distinction when recording results.
