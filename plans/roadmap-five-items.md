# Five roadmap implementation candidates

1. **Numbered quicksaves** (`plans/ROADMAP.md`, Saves): keep each script-requested
   quicksave as a distinct numbered native `.vrs` save while manual names and the
   autosave slot retain their current behavior.
2. **Detection blindness**: inspect the remaining gap in `known_ambiguities/detection.md`
   and implement only an uncovered player-facing behavior.
3. **Lockpicking key-reward perk**: check the existing perk/effect handling and
   implement the missing reward when a lock is picked.
4. **Container transfer quantity selection**: check the SWF callbacks and current
   egui fallback, then implement a quantity choice where the engine UI lacks one.
5. **Journal Favorite Weapon statistic**: check `stats.rs` and the game's statistic
   mapping, then add the missing favorite-weapon usage count.

This is a candidate list for independent roadmap work. Items 2–5 still need a
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
pass.
