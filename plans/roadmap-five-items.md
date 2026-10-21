# Five roadmap implementations

1. **Numbered quicksaves** (Done; `plans/ROADMAP.md`, Saves): each script-requested
   quicksave is a distinct numbered native `.vrs` save; manual names and the
   autosave slot retain their existing behavior.
2. **Detection blindness** (Done; `plans/ROADMAP.md`, Detection): the observer's
   Blindness actor value scales the visual detection term.
3. **Lockpicking key-reward perk** (Done): successful lockpicking evaluates the
   key reward entry point and grants a key not already carried.
4. **Egui fallback quantity selection** (Done): the SWF already handles
   quantities; the fallback now has the missing chooser for large stacks.
5. **Journal Favorite Weapon statistic** (Done): accepted player melee attacks
   and actual bow releases contribute usage; native saves retain it.

This list records the scope and outcome of the five independent roadmap items.
Each item was planned before implementation, implemented by a Luna 6 High
subagent, then reviewed and validated by the parent. Only public sources and
game assets were used; no engine binaries were examined or game assets committed.
NPC fall damage was rejected during initial scoping because its existing
documented blocker is the absence of airborne NPC motion and landing events;
it is not counted among these five completed items.

## Item 3 plan

Implement the Wax Key `Mod Lockpicking Key Reward Chance` (entry point 90) at
the successful completion of the player's lockpick, using the lock reference
as the entry-point context. Award the reference's valid `KEYM` once, only when
the player does not already carry it, through the normal inventory mutation
path so item events fire. Keep ordinary key use, script unlocks, cancelled
attempts, and failed attempts outside this reward path. Test these cases with
the synthetic lock Engine fixture, including a conditionally configured perk
entry. The installed `Skyrim.esm` record `WaxKey` sets entry point 90 (function
1, two tabs) to `100.0`; with the community enum name
`kModLockpickingKeyRewardChance`, use a 0–100 chance and seed evaluation at
0. Do not simulate a first-ever pick if there is no durable key/lock history
to support it. Record any unresolved relock/dropped-key differences from the
game's "first time" wording.

## Item 1 outcome

Implemented and documented as native history behavior. Local UESP GMST defaults
revision 3098585 gives `sMenuDisplayQuicksaveName` the label "Quicksave"; the
public [Bethesda Kinect support article](https://help.bethesda.net/app/answers/detail/a_id/16611/~/why-am-i-unable-to-create-more-than-one-quick-save-for-the-elder-scrolls-v:)
says that interface has one quicksave slot. Neither source establishes the
`Game.RequestSave` filename behavior for Skyrim SE. Each flushed native request
now chooses the next unused `quicksave N.vrs`; existing numbered files remain,
gaps are preserved, failures do not consume a number, and request deduplication
within one update remains. Autosave still replaces `autosave.vrs`, and manual
names remain replace-in-place. This is not a claim of Skyrim SE parity.

`cargo test --bin vibe_rm quicksave -- --nocapture`: 3 passed, including malformed
and legacy names, overflow, unreadable directory handling and failed-write slot
reuse; the GPU-backed flush test compiled but is ignored by default. Attempting
it with `--ignored` failed during existing Engine setup because the selected
wgpu adapter rejects the terrain pipeline (17 sampled textures; limit 16),
before save behavior runs. Rustfmt on the two Rust files and `git diff --check`
pass. Follow-up validation after item 1 was committed: the parent ran all 16
save tests and one GPU-backed save integration under external Vulkan; all passed.

## Item 2 plan

Add the documented Blindness actor value (84) to the observer's visual
detection factor. UESP's [Skyrim:Sneak revision 3575824](https://en.uesp.net/w/index.php?oldid=3575824)
provides the detection formula and visual/sound separation; [Actor Value
Indices revision 3596002](https://en.uesp.net/w/index.php?oldid=3596002) identifies
Blindness as AV 84, a percentage that reduces spotting while the target sneaks
and outside combat. The public [CK Detection category](https://ck.uesp.net/wiki/Category:Detection)
gives `Blindness=(100-blindness)/100` for vision. Integrate the live observer AV
only into visual detection, retain sound and skill contributions at 100
Blindness, and keep `sees` as geometric visibility (line of sight/view/sleep/
invisibility) rather than a detection result. Clamp finite AV values to 0–100;
treat non-finite values as 0 blindness. Only apply the AV when the target is
sneaking and the observer has `CombatState::None`; the engine's `Searching`
state bypasses the AV. Mapping those states and the behavior of out-of-range AV
values are engine policies because the public descriptions do not specify those
runtime edges.

## Item 2 outcome

Implemented in `src/detection.rs` with actor value 84 named in
`crates/esp/src/actor_value.rs`. The observer's live AV scales vision only when
the target is sneaking and the observer has `CombatState::None`. Full blindness
leaves sound and observer skill in the formula; `sees` remains geometric
visibility. The remaining Skyrim-SE uncertainty is recorded in
`known_ambiguities/detection.md`, including the UESP AV row's sound-only summary
at maximum blindness versus the CK formula's visual-only multiplier.

Parent validation outside the sandbox:
`cargo test --bin vibe_rm blindness_tests -- --include-ignored --nocapture`:
all 3 tests passed, including the actual `detection_between` integration test.
The sandbox's fallback graphics adapter cannot construct the terrain pipeline;
the external Vulkan adapter can. Modified Rust files were formatted;
`git diff --check` passes.

## Item 3 outcome

The successful lockpick path evaluates entry point 90 after changing the lock
to unlocked. A positive result awards one `KEYM` through the regular inventory
mutation path, provided the key is not already carried. Missing/invalid keys,
failed or cancelled attempts, ordinary key use and script unlocks do not call
the reward hook. Local UESP describes the Wax Key reward and its “first time”
wording; installed `Skyrim.esm` confirms the vanilla entry point is Set 100. The
0–100 percent interpretation, roll resolution for custom fractional values,
and post-unlock condition timing are implementation policies. Since native
state has no history of dropped keys or past picks, repeats after a dropped key
or relock can award again; this limit is recorded in `known_ambiguities/perks.md`.

The parent ran `cargo test --bin vibe_rm locks::tests -- --include-ignored
--nocapture` outside the sandbox with Vulkan: all 6 tests passed (3 new Wax Key
tests and 3 existing lock tests). `cargo fmt --check` and `git diff --check`
pass. Item 3 is complete within the documented first-pick/history limitation.

## Item 4 plan

The SWF container menu already supplies quantities: the permitted
`containermenu.swf` inspection shows that stacks over five open its quantity
menu, except weightless items taken through its auto-all path, and
`src/swf_ui/item_menu.rs` already honors the callback amount. Implement the
quantity chooser only in the egui fallback (`src/ui.rs`) for taking/storing,
pickpocketing/placing, and dropping stacks over five. Keep Shift-all as an
immediate shortcut. Save the parent menu plus exact source, destination, item,
and ownership row in a pending action; while it is open, suppress the parent
inventory controls. Escape, Tab, or Cancel clears only the pending action.
Clear it if the displayed menu changes. On confirmation, find the same current
row, cancel if it disappeared, and clamp the selected amount to its current
positive count before calling existing transfer, drop, or pickpocket APIs. Show
the pickpocket chance for the chosen amount. Test the production confirmation
helper for exact ownership-row movement, clamping/stale rows, cancellation and
the pickpocket count path using synthetic Engine records. Do not alter or copy
game assets or SWF-derived code.

## Item 4 outcome

The permitted `containermenu.swf` inspection established a quantity-menu limit
of five, with the movie already passing its chosen amount through the SWF item
transfer bridge. The egui fallback now opens its own chooser for stacks over
five when moving, pickpocketing/placing or dropping; Shift still takes all.
Actions preserve the parent menu, source, destination, item and selected
ownership row. Confirmation rechecks the row and caps the requested amount at
the current count; missing rows cancel. Esc, Tab and Cancel leave the parent
menu open, and external menu changes clear the pending chooser. Drop actions
also remove only the selected ownership row.

The parent ran `cargo test --bin vibe_rm ui::quantity_tests --
--include-ignored --nocapture` outside the sandbox: all 6 tests passed,
including actual `Ui::build` Escape/Tab handling and Engine-backed transfer,
pickpocket take/place, stale-row, cancellation and ownership-specific drop
cases. `cargo fmt --check` and `git diff --check` pass. The chooser's appearance
and exact slider defaults are egui policies, not claims of original-menu
parity.

## Item 5 plan and outcome

The implementation plan was written before coding in
[`favorite-weapon-statistic.md`](favorite-weapon-statistic.md). Local UESP's
[Statistics revision 3569833](https://en.uesp.net/w/index.php?oldid=3569833)
describes the most-used melee weapon or bow, but leaves the usage event and
tie rules unspecified. Accepted player melee attacks and actual bow releases
now count by weapon base form; ties retain the incumbent. Hit notifications,
bashes, unarmed attacks, staves and failed actions do not add uses. The journal
shows the normal localized weapon name, independently of manual favorites.

Optional native-save section `WUSE` retains sorted usage counts and the winner.
Old saves replace this state with empty counts; malformed data is refused, and
invalid weapon forms are rejected before mutating the engine. The chosen usage
metric and ties remain implementation policies, recorded in
`known_ambiguities/journal.md` and `known_ambiguities/saves.md`.

Parent validation: both `stats::action_tests` passed with graphics access,
including accepted/rejected attacks, bow release, save restoration and old-save
replacement. Both pure weapon-usage tests passed. The broader engine suite
passed 294 tests (67 ignored by default), `vrm-esp` passed 7, and all 24 native
save tests passed with graphics access. All new GPU-dependent tests were run
explicitly; synthetic tests establish implementation behavior, not Skyrim SE
parity. All five items are complete within their documented scope.
