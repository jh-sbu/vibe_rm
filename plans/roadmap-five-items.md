# Five roadmap implementation candidates

1. **Numbered quicksaves** (`plans/ROADMAP.md`, Saves): keep each script-requested
   quicksave as a distinct numbered native `.vrs` save while manual names and the
   autosave slot retain their current behavior.
2. **Detection blindness** (`plans/ROADMAP.md`, Detection): apply the observer's
   Blindness actor value to the visual detection term.
3. **Lockpicking key-reward perk**: check the existing perk/effect handling and
   implement the missing reward when a lock is picked.
4. **Container transfer quantity selection**: check the SWF callbacks and current
   egui fallback, then implement a quantity choice where the engine UI lacks one.
5. **Journal Favorite Weapon statistic**: check `stats.rs` and the game's statistic
   mapping, then add the missing favorite-weapon usage count.

This is a candidate list for independent roadmap work. Items 3–5 still need a
scope check against current code before implementation.

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
