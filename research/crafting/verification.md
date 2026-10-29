# Crafting verification (2026-10-29)

These runs use this project's engine with public/game-asset evidence, not the
original engine. No game executable, library or memory was inspected. Captures,
generated plugins and compiled probe scripts remain ignored and uncommitted.

## Automated checks

- `cargo fmt --check` and `git diff --check`: clean.
- `WGPU_BACKEND=vulkan cargo test --workspace -j 2`: 786 passed, 366 ignored.
  A first sandbox run hit the default OpenGL adapter's 16-texture limit in
  existing renderer tests. The unsandboxed Vulkan rerun passes.
- `WGPU_BACKEND=vulkan cargo test --bin vibe_rm crafting_ -- --include-ignored`:
  24 passed, including installed vanilla/DLC perks, transactions, scripted
  output events, created forms, copy identity, legacy saves, active armor
  enchantment reload, stars becoming refillable, and drop/pickup metadata.
- `... cargo test --bin vibe_rm staffs:: -- --include-ignored`: 3 passed.
- `... cargo test --bin vibe_rm ui::quantity_tests -- --include-ignored`:
  8 passed, including stale/cancelled quantities and ownership/pickpocket paths.
- The crafting1 probe's typed vanilla-call and bounded-page dry run passes in
  the tools suite. Rebuilt `vrm-tool` before generating its plugin/scripts.

## Offscreen authored-menu checks

Captures are under `tests/captures/crafting/`. Run with Vulkan and the installed
Data directory, `--screenshot`, `--wait 240`, and a sequence of `--menu-key`
values. `ArrowRight` expands categories, E selects, R crafts, Enter confirms.
These are interactive movie operations, not direct backend calls.

- Alvor's forge `000D6987`: an iron sword is created after confirmation;
  materials decrease, the card/skill bar refresh and CRFT is emitted.
- Grindstone `0001C785`: one iron sword becomes Fine, damage/value and the
  required ingot count refresh. Armor workbench `0003550D` similarly improves
  an iron cuirass. Captures `forge-after`, `grindstone-after`, `armor-after`.
- Riverwood outdoor cooking pot `001054A1`: FoodChicken plus SaltPile creates
  grilled chicken. The ordinary carry/gold bar has no placeholder stats. CRFT
  starts AchievementsHardWorkerFoodQuest. This is the outdoor pot, correcting
  the plan's original assumption about a pot in the Sleeping Giant Inn.
- Sleeping Giant lab `00026D7B` and Arcadia's lab `000C6E45`: select Blisterwort
  and Wheat, mix, see both counts decrease and Restore Health become a known
  effect category. The result card shows Potion of Restore Health/value 14 at
  the test character's skill 15. Captures `alchemy-after`, `arcadia-after`.
- Farengar's enchanter `000C6E44`: disenchant a fire sword and learn Fire Damage;
  select a grand soul, dagger and effect; the strength/uses slider appears,
  accepts a changed value, and enchanting confirmation spends the gem and
  refreshes the list/skill. F opens the native rename field. Captures
  `enchant-learned`, `enchant-preview`, `enchant-after`, `enchant-rename`.
- Lakeview carpenter's workbench `03003066`, at (-8861,-70795,2390): the initial
  unowned setup has no eligible recipes. For a controlled crafting check,
  enable its three required log-pile aliases (`0300ED28`, `03010FBF`,
  `03010FC0`), start BYOHHouseBuilding and supply small-house layout
  `03003060`, sawn logs `0300300E` and quarried stone `0300306C`. The authored
  recipe conditions then expose wall framing/foundation. This setup bypasses
  land purchase and is not a test of buying or managing a house. Creating the
  foundation through E/Enter removes one sawn log and ten stone, advances
  BYOHHouseFalkreath through stages 120/130, completes the foundation objective
  and removes the foundation recipe. Captures `hearthfire-ready` and
  `hearthfire-after` verify the real building-item script chain.

UI findings fixed during these runs: initialize after timeline children load;
write ItemCard.itemInfo rather than invoking a nonexistent SetItemInfo;
invalidate the tempering list; supply actual skill/player bottom bars; use
crafting cards for known enchantments; refresh recipe visibility after
activation scripts change quest variables. Craft and Rename use live bindings.

## Remaining original-game evidence

`research/crafting/manual-test.md` describes the generated `crafting1` probe.
It has not been installed or run in Skyrim SE. Exact ExtraHealth mapping,
formula rounding, several XP inputs, and transient enchantment pricing remain
working choices in known_ambiguities/crafting.md. The native engine's separate
per-hand identity for differently modified copies of one base weapon remains
a broader engine follow-up in ROADMAP. This verification does not claim original
game parity for those choices or the excluded station animation/3D preview.

## Audit corrections (2026-10-29)

- Shared created ENCH records now have separate active effects per worn piece.
  Two +25 Health pieces grant +50; removing either leaves +25. Save/load keeps
  effect IDs, equipment sources and held values. Legacy source-less effects
  acquire their worn source without restarting. Invalid equipment sources and
  malformed extended AEFF payloads are rejected.
- Custom Muffle and Waterbreathing retain magnitude 1 at skills 15/100, all
  five soul sizes and with Enchanter bonuses. Tests cover previews, created
  records and worn actor values; the sources are in formulas.md.
- First depletion of a tempered/named authored enchanted weapon keeps its
  selected metadata. Regression checks cover repeat depletion, inventory
  listing, untouched sibling copies, save/load and legacy charge migration.
- Broader save checks exposed crafting sections preceding META/HEAD. They now
  follow the headers, preserving journal listing, previews and save numbering.
  Prefix-reader tests cover generated forms with and without META.

Final checks with `WGPU_BACKEND=vulkan` and a headless GPU adapter:
`cargo test --workspace -j 2` (786 passed, 369 ignored), plus
`cargo test --bin vibe_rm crafting_ -- --include-ignored` (27 passed),
`cargo test --bin vibe_rm save::tests:: -- --include-ignored` (165 passed), and
`cargo test --bin vibe_rm staffs:: -- --include-ignored` (3 passed).
`cargo fmt --check` and `git diff --check` are clean. These remain project-engine
checks; the original-game probe has not been run.

## Exit-key correction (2026-10-29)

Offscreen input checks confirm Tab calls `CloseMenu` and exits forge,
grindstone, armor workbench, alchemy and enchanting frames. With an ingredient
or enchanting item selected, Tab opens the existing quit confirmation: No
keeps the selection/menu, another Tab asks again, and Yes exits. These checks
use the authored movie and the rebuilt VibeRM client; captures and callback
logs stay under `/tmp/vibe-crafting-exit/`. The host also accepts the configured
Tween Menu key through the same exit handler. All 27 crafting tests pass.
