# Plan: the main menu's self-contained leftovers

**Status: drafted** (2026-10-30). Follows `plans/main-menu.md` ("Where it
stands", Remaining). Covers the leftovers that don't wait on other ROADMAP
items: the save list's keyboard selection, the credits' logos and a stale
entry. New (item 11), saves made with other plugins (item 9's changed load
orders) and the backdrop (research-gated) stay where they are. Each numbered
step is one commit; run `cargo fmt` before each. Changes to `vendor/ruffle`
are their own commits, one change each, as `vendor/ruffle/VENDOR.md` asks.

## 1. The stale entry

`plans/main-menu.md` still lists "A loading screen for loads from the main
menu". `plans/loading-screen.md` ("After the observations") records that
this works and was captured offscreen ("a save load from the main menu").

- **Step 1.** Remove the line from `main-menu.md`'s Remaining, pointing to
  the loading-screen plan, and correct ROADMAP item 11's main-menu entry if
  it repeats it. A docs-only commit.

## 2. The save list starts with nothing selected

In the main menu's Load (and the journal's), the character list and the save
list open with no row selected, so Enter does nothing until an arrow key
picks a row. Both lists come from the shared save-list code
(`src/swf_ui/journal_system.rs`: `PopulateCharacterList`, the batch rows,
then `onFillCharacterListComplete` / `onSaveLoadBatchComplete`). The Quit
list sets `selectedIndex` itself (`RequestIsOnPC`). The save lists don't,
and leave it to the movie.

- **Step 2a (research).** In `quest_journal.swf` and `startmenu.swf`, read
  what `onFillCharacterListComplete` and `onSaveLoadBatchComplete` do with
  the list's `selectedIndex` and focus on PC (platform 0) against a gamepad.
  Also read what the list does on a key press with `selectedIndex` −1. The
  likely cases:
  - the movie sets a selection itself, which the vendored Ruffle loses
    somewhere (focus, `Selection.disableFocusKeys`, an `onSetFocus`
    handler): fix in `vendor/ruffle` or `crates/ui-swf`;
  - the movie leaves it unset on PC and the first arrow key selects. Then
    this matches the game, and the entry becomes a known behaviour rather
    than a bug, recorded in `known_ambiguities/main-menu.md` (or checked by
    a direct SE observation: open Load and press Enter).

  Write the finding into `research/ui/main-menu.md` (prose, no copied
  bytecode).
- **Step 2b (fix, if 2a finds a bug).** The fix goes where the fault is. A
  Ruffle change is its own `vendor/ruffle:` commit with a test in
  `crates/ui-swf` if one fits. A host-side change (setting `selectedIndex`
  as the movie expects the game to) goes in `journal_system.rs`, so both
  menus get it. Offscreen check: `--main-menu`, open Load, press Enter
  with no arrow key. The character list moves to the saves, and Enter
  there loads.

## 3. The credits' logos

`creditsmenu.swf` gets `interface/credits.txt` as HTML. Line 1 is `<img
src='SkyrimLogo' height="244" width="129">`, and lines 3191 and 3193 are
`<img src='MiddlewareLogos'>` and `<img src='CreationEngineLogo'>`, all
library bitmaps of the movie. `credits_text` (`src/swf_ui/main_menu.rs`)
drops these lines, because the vendored Ruffle lays out the lines after an
inline image on top of one another. The inline-image support is
`vendor/ruffle` commit `1d11662` (`LayoutContext::append_image` in
`core/src/html/layout.rs`).

- **Step 3a (reproduce).** Add a layout test in `crates/ui-swf` (or in the
  vendored crate's `html` tests, if those build here): an HTML field holding
  `<img src='X' height='244' width='129'>\n\nTITLE\nName`, with a library
  bitmap `X`. Check each line's top `y`. Things to check:
  - whether `newspan` resets `max_ascent` to the font's ascent when the text
    after the image starts a new span, dropping the image's height before
    `newline` advances the cursor (`max_ascent + max_descent`);
  - whether the image box's bounds (placed at `cursor - height`, then
    shifted by `baseline_adjustment` in `fixup_line`) are counted in the
    line's height;
  - what the swapped-looking `height="244" width="129"` does: Scaleform
    reads `width` and `height` as written (Flash doesn't scale inline
    images that way). Compare the bitmap's own size with the attributes, and
    check against an SE screenshot of the credits if one can be had.
- **Step 3b (fix).** One `vendor/ruffle:` commit that makes the line holding
  an image as tall as the image, and the lines after it start below it.
  Run the test from 3a, plus the existing inline button-art case (HUD help
  text with `[Activate]` art), so text-height images don't change.
- **Step 3c (use it).** `credits_text` stops dropping the `<img>` lines.
  Update its test (`credits_text_drops_the_logos` becomes a test that keeps
  them). Offscreen capture of the credits' start (the Skyrim logo) and end
  (the two logos), kept under ignored `tests/captures/`. Update
  `research/ui/main-menu.md` and remove the Remaining line in
  `plans/main-menu.md`.

## Verification

- `cargo test` (ui-swf layout test, main_menu tests).
- `cargo test --bin vibe_rm -- --include-ignored` for the GPU-gated main-menu
  tests.
- The offscreen runs in 2b and 3c.

## Open questions

- Whether SE selects the first row of the save lists on PC (2a may settle it
  from the movie; otherwise a direct observation).
- How SE sizes the Skyrim logo from those attributes.
