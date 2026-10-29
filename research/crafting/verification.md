# Crafting verification (2026-10-29)

These runs use this project's engine with public/game-asset evidence, not the
original engine. No game executable, library or memory was inspected. Captures,
generated plugins and compiled probe scripts remain ignored and uncommitted.

## Automated checks

Current review run (2026-10-29), with `WGPU_BACKEND=vulkan` and access to a
headless GPU adapter:

- `cargo test --workspace -j 2`: **785 passed, 372 ignored**, no failures.
- `cargo test --bin vibe_rm save::tests:: -- --include-ignored`: 165 passed.
- `cargo test --bin vibe_rm crafting_ -- --include-ignored`: 27 passed.
- `cargo fmt --check` and `git diff --check`: clean.

Logs are retained under ignored `tests/captures/crafting/review/`. The obsolete
collection unit test was replaced by an ignored GPU regression covering saves
with a runtime-only projectile root, so one passing test moved into the ignored
count. Earlier workspace totals reflected earlier revisions and are superseded
by this run. The sandbox exposes no Vulkan adapter; these tests ran with adapter
access. Earlier OpenGL attempts hit the renderer's 16-texture adapter limit.

Other checks recorded during implementation, before the review corrections:

- `cargo test --bin vibe_rm staffs:: -- --include-ignored`: 3 passed.
- `cargo test --bin vibe_rm ui::quantity_tests -- --include-ignored`: 8 passed,
  including stale/cancelled quantities and ownership/pickpocket paths.
- The crafting1 probe's typed vanilla-call and bounded-page dry run passed in
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

These corrections passed the workspace, crafting, save and staff suites with
`WGPU_BACKEND=vulkan` and a headless GPU adapter, plus formatting and diff checks.
These remain project-engine checks; the original-game probe has not been run.

## Exit-key correction (2026-10-29)

Offscreen input checks confirm Tab calls `CloseMenu` and exits forge,
grindstone, armor workbench, alchemy and enchanting frames. With an ingredient
or enchanting item selected, Tab opens the existing quit confirmation: No
keeps the selection/menu, another Tab asks again, and Yes exits. These checks
use the authored movie and the rebuilt VibeRM client; captures and callback
logs stay under `tests/captures/crafting/exit/`. The host also accepts the configured
Tween Menu key through the same exit handler. All 27 crafting tests pass.

## Tutorial hint height (2026-10-29)

The workbench hint reproduced the reported clipping in an isolated HUD render:
field height 41.25 pixels versus text height 56 pixels. Enabling vertical growth
through the field's standard `autoSize` preserves its word-wrap width and font.
The field then measures 60.9 pixels high, with maximum scroll 1 (all lines fit).
Forge, armor workbench, alchemy and enchanting hints fit when changed from short
to long and back, including hiding and showing the hint again.

End-to-end renders in the rebuilt VibeRM client open the Riverwood workbench,
exit with Tab, and show the complete final line at both 1280x720 and 800x600.
Captures and measurements remain under `tests/captures/crafting/help-layout/`;
the initial isolated reproduction is `help-layout/hint-before.png` there.
`cargo build --features remote-console --bin vibe_rm`, `cargo fmt --check` and
`git diff --check` pass. The existing running client's remote console was queried
without changing its gameplay state; its process must restart to load the code
correction. These are VibeRM/asset checks, not original-engine observations.

## Player locomotion verification (2026-10-29)

The installed-assets player-body regression first failed because ActionMoveStart
resolved to no event. With the explicit unmounted answer, it enters locomotion at
walk/run speeds and returns to standing on stop.
All 9 player-body tests and 31 status-condition tests pass with ignored fixtures
enabled; the workspace suite passed at that stage. Current workspace totals
are recorded in the automated checks above. Offscreen checks in VibeRM's
WhiterunBanneredMare confirm rendered walk/run strides and separate
locomotion/standing graph states. Captures and logs remain under
`tests/captures/player-animation/`; Skyrim SE itself was not run.

## Review corrections (2026-10-29)

Saving now retains every created ALCH/ENCH record and leaves the live runtime
plugin and magic cache untouched. Capture no longer serializes state to scan for
handles, so the save and thumbnail are encoded once. The regression exercises a
poison referenced only by an unsaved projectile, an otherwise unused enchantment,
repeat saves, failed writes preserving the previous save and live records,
reload, interning and monotonic ID allocation. Reclamation remains deferred as
recorded in the plan and runtime-lifetime decision.

The old step 10 commit was split into persistence/migration, selected-copy
recharge, fallback inventory actions, authored menu integration, shield quality,
and documentation commits. The crafting1 probe now precedes its verification
documentation. Temporary captures and logs were copied to the ignored capture
paths above; no captures, game assets or external references were committed.
