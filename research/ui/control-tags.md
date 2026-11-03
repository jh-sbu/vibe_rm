# Control tags in player-facing text

Research for `plans/control-text-tags.md`, 2026-11-03. Movie behaviour in
"Movie paths" is asset evidence; what Skyrim SE actually shows is under
"Skyrim SE observations" (the user's screenshots, two rounds), and where the
two differ the observations win.

## Sources and method

Local UESP was searched first with `scripts/uesp.py search` for “control text
button tags”, “Control” (Skyrim Mod titles), and “ShowAsHelpMessage” (all
namespaces). No specification was found; the last search returned a user-talk
script fragment, not authoritative behavior. No public source code was imported
or used as a behavior specification (including no SKSE source).

The installed Skyrim SE Data directory (`<Skyrim SE>/Data`, outside the
repository) was used.
Only game assets were examined: Interface archive movies and control tables,
plugin records and localized English strings. No engine executable, library or
memory was inspected. Existing `spike/swf/swfspike` disassembly and `fold.py`
were used on movies already extracted outside the repository under
`~/.cache/vibe_rm/swf/interface/`. Derived disassembly/folded output remains
outside the repository at `~/.cache/vibe_rm/swf/out/control-tags/`. A small
external read-only SWF tag walk checked bitmap exports and dimensions. No game
asset or asset symlink was added to the repository. This note describes behavior
in prose rather than copying ActionScript.

## Movie paths

| Surface | Asset path / methods | Finding |
| --- | --- | --- |
| HUD help | `hudmenu.swf`, `HUDMenu.ShowTutorialHintText`, `Shared.ButtonTextArtHolder.CreateButtonArt` | The hint's text is scanned for bracket tags; the result is assigned as HTML with inline button images. Keep controls for this path. |
| Notifications | `hudmenu.swf`, `HUDMenu.ShowMessage`, `Messages.onEnterFrame` | Text is queued, then assigned directly to the message clip's HTML field. There is no bracket scanner or button-art callback along this path. Native code might still transform the incoming string; screenshots must settle that. |
| Journal Help | `quest_journal.swf`, `SystemPage`, Help button holder | The selected Help body goes through `CreateButtonArt` and becomes HTML. The Creations text uses a separate holder too. |
| Quest title/objectives/log description | `quest_journal.swf`, `QuestsPage.onQuestHighlight`, `SetDescriptionText`, `ObjectiveScrollingList.SetEntry`, inherited `BSScrollingList.SetEntryText` | Title and description use ordinary `SetText`; objectives call the list's ordinary text setter. No call to `CreateButtonArt` occurs on these paths. Quest list text is not the Help body. |
| Tutorial menu | `tutorialmenu.swf`, `TutorialMenu.ApplyButtonArt` | Calls the holder on `HelpScrollingText.textField` and assigns the returned HTML. |
| Message box | `messagebox.swf`, `MessageBox.SetMessage` | Ordinary text setter, no bracket scanner / `GetButtonFromUserEvent`. |
| Loading text | `loadingmenu.swf`, `LoadingMenu.SetLoadingText` | Ordinary text setter, no bracket scanner / `GetButtonFromUserEvent`. |

The HUD's only invocation of `CreateButtonArt` outside its definition is the
tutorial hint. A similarly named `ButtonArtHolder` instance should not be
confused with the journal controls remapping list: that list has a separate
`UnknownKey` movie clip fallback which writes a hex code or `???`.

### Missing names and UnknownKey

The three text-art holders take the substring between brackets and call
`GetButtonFromUserEvent`, receiving a name through `SetButtonName`. On PC the
callback stores that name. The holder loads the named `.png` bitmap; a valid
positive-height bitmap produces an inline image (normally height 26). If the
callback name is undefined or the bitmap is absent/invalid, the original
bracketed substring is appended unchanged. An empty string therefore attempts
`.png`, which is not exported by these movies, and retains the tag. There is no
special case that deletes unknown bracket text.

All three movies export `UnknownKey.png` as an 88 by 64 bitmap. Returning the
literal name `UnknownKey` therefore takes the image branch; it does **not**
invoke the controls-list fallback or convert to `???`. The actual native result
for an unbound event, its visible art, and any engine-side preprocessing remain
unverified. The movie alone cannot determine how a chord is named or which
context the original callback chooses. The installed movies contain newer
console-platform branches; findings above describe their PC branch, not every
historical SE release.

## Survey

Run `vrm-tool control-tags "<Skyrim SE>/Data"`. It reads winning records in the
default load order, loads English strings through the VFS, and counts every
bracket candidate in:

- `MESG`: `DESC`, `FULL`, all repeated `ITXT` fields;
- `LSCR`: `DESC`;
- `QUST`: `FULL`, all objective `NNAM` and stage-log `CNAM` fields;
- `BOOK`: `DESC`, `FULL`.

Help topics are the `MESG` forms listed by default object `HMPC`; their fields
are marked `MESG(help)` without counting them twice. Counts are occurrences,
not distinct records. This is the plan's record-text scope, not all possible
text in scripts, translations or movies. The survey deliberately retains prose
brackets and book markup rather than assuming every bracket is a control.

Classification uses **the Interface BSA directly**, never a loose/custom map:
first event context (zero-based, file order), all other contexts with that name,
and its raw keyboard/mouse/gamepad columns; otherwise keyboard/mouse key-name
tables; otherwise `nothing`. References and chords remain explicit raw columns,
not invented labels. Matching is ASCII case-insensitive. This small tools-local
classifier neither imports nor changes `src/input_map.rs`; a future engine
resolver can replace it if sharing is useful.

### Installed-data results

The default list contained Skyrim, Update, Dawnguard, HearthFires, Dragonborn,
Fishing, Survival, Rare Curios, Saints and Seducers, and `_ResourcePack`.
There were **4,889 records**, **43 HMPC help forms**, and **130 distinct bracket
candidates**: 45 event names, seven keyboard/mouse names, 78 unmatched candidates.
No bracket candidates appeared in LSCR text, QUST titles/log entries, MESG titles
or button labels in this installation; those fields are still surveyed.

Important corrections to the preliminary plan:

- `Favorites` is already context **0**, keyboard `0x10` (Q).
- `XButton` and `YButton` are **events**, not unresolved gamepad-only names.
  First context **3** (Item Menus), also context 12; keyboard references lead
  to `Ready Weapon` and `Toggle POV` respectively.
- `Accept`/`Cancel` first occur in context 1; `ChargeItem` in 4; `NextPage` in
  5 and 10; `LocalMap` in 7; `RotatePick`/`RotateLock` in 15.
- `[Activate]` occurs 50 times, `[Right Attack/Block]` 47, `[Left Attack/Block]`
  38; `[Mouse1]` occurs twice in Help. Literal key candidates include F, C, R,
  1, 2, 8. Book footnote numbers also contribute to 1 and 2.
- Unmatched MESG candidates are `Left Thumbstick`, `Right Thumbstick`,
  `Mouse Move`, `ShowOnMap`, and `Modded`. `Mouse Move` does not exactly match
  the shipped key name `MouseMove`; do not silently invent an alias.
- `[Optional]` occurs in one objective; `[pagebreak]` occurs 1,533 times in
  books. Most other unmatched candidates are editorial/book prose. Therefore
  “all tags resolve except gamepad names” is not an appropriate verification
  criterion for an unfiltered record survey.

## Manual Skyrim SE observation

Build with `cargo build -p vrm-tools`, then generate:

```sh
target/debug/vrm-tool test-mod controltags "<Skyrim SE>/Data" tests/assets/VrmControlTags.esp
```

This writes the plugin and `tests/assets/Scripts/VrmControlTagsScript.pex`
(source: `crates/tools/src/test_mods/controltags/controltags.psc`). They are
authored test files, not copied game assets, and use only vanilla Papyrus
natives. The generator installs nothing and doesn't run the game. Papyrus
can't read rendered text, so the results are screenshots.

### What it shows

Every token is numbered, so a screenshot can be read even where art replaces
the tag. Set A is ordinary bindings; set B is edge cases that the shipped map
already has, so no control map needs editing.

- **A:** `1[Activate] 2[activate] 3[Sprint] 4[Right Attack/Block] 5[Favorites] 6[Mouse1] 7[XButton]`
- **B:** `8[Zoom In] 9[Multi-Screenshot] 10[Move] 11[Optional] 12[Mouse Move] 13[Left Thumbstick]`

| # | why it's there |
| --- | --- |
| 1, 2 | an event; 2 tests case-insensitive matching |
| 3, 5 | keyboard events |
| 4 | a mouse-only event |
| 6 | a key name, not an event |
| 7 | an Item Menus event (`!0,Ready Weapon`) |
| 8 | mouse wheel only |
| 9 | a chord (L-Ctrl + Print Screen, which has no art) |
| 10 | gamepad only in Main Gameplay (a later context has the mouse) |
| 11–13 | not events or key names |

The probe was written when this engine predicted other results (key names in
plain text, `Wheel` and `UnknownKey` art). What SE showed is under "Skyrim SE
observations", and the engine now follows it (`0b4bc53`).

The surfaces are:

- a message box: the title (`1`, `9`), the text (A and B) and the button (`OK 1`);
- notifications A and B;
- the quest objective (`Obj 1 8 9 11`), shown as the HUD's objective banner and
  in the journal;
- in the journal (the quest is a side quest named `Quest 1 9 11`), its log
  entry with A and B;
- help messages A and B, shown as the HUD's tutorial hint.

### Protocol

Use a throwaway save, keyboard and mouse, and default controls. Make sure
tutorials/help messages are on in the game's settings. If the quest isn't
running, start it with `startquest VrmControlTagsQuest`. Vanilla's console
can't call a script function (`cqf` is SKSE's), so the probe is started with
the console's `set` command on a global, which the quest checks once a second
(UESP, [Skyrim:Console](https://en.uesp.net/w/index.php?oldid=3628513),
`set <global> to <value>`). After each `set` below, close the console right
away, and don't start another run until the last one ends.

**Round 1: default controls.** Run `set VrmControlTagsRun to 1`.
1. After 5 s, a message box: screenshot it, then press its button.
2. After 2 s, notification A, then 3 s later notification B: screenshot each.
3. After 8 s, the objective banner: screenshot it.
4. After 8 s, help A for 15 s, then help B for 15 s: screenshot each.
5. Open the journal on the quest: screenshot its name, objective and log
   entry.

**Round 2: rebinding.** Run `set VrmControlTagsRun to 2`. Help A comes up
for 90 s.
1. Screenshot help A.
2. While it is still up, open the Controls settings and rebind Activate to
   NumPad5. If NumPad5 can't be bound, use another numpad key (1–8) and
   write down which one.
3. Back in the game: screenshot help A again, if it's still up. Does 1's
   art change, or does it keep E until the hint is shown again?
4. Open the journal on the quest: screenshot it again. Does text written
   under E now show NumPad5?
5. Run `set VrmControlTagsRun to 1` again and screenshot every surface as
   in round 1. The objective banner shows again; the log entry is written
   only once.

Afterwards, restore Activate to E.

**For each screenshot, note:** each number's text, art, or unchanged brackets,
and any clipping. In particular:
- Do boxes and notifications show words or art?
- What words: the key table's names (`L-Alt`, `MouseWheelUp`) or others?
- What do 8, 9 and 10 show?
- Do 11–13 keep their brackets?
- Does 2 match 1?
- Does 7 show R?

If a hint doesn't appear (tutorial setting, another hint up), say so. Missing
text isn't evidence about the tags. Screenshots can go in `tests/results/`;
there's nothing for `test-results` to read.

The probe doesn't cover loading-screen text or other languages' key names.

## Skyrim SE observations

### Round 1 (2026-11-03, default controls, keyboard and mouse)

Screenshots by the user, English, the probe as of `04ce1ae`. Notification B
was not captured.

| Surface | What SE showed |
| --- | --- |
| Message box | The text exactly as written, every tag in brackets (`1[Activate]`...). The `FULL` title (`Title 1[Activate] 9[Multi-Screenshot]`) was **not shown**: the box began with the text's first line. The button read `OK 1[Activate]`. |
| Notification A | As written, all brackets. |
| Objective banner (HUD) | As written: `Obj 1[Activate] 8[Zoom In] 9[Multi-Screenshot] 11[Optional]`. |
| Journal | Quest name (upper-cased by the movie), log entry and objective all as written. |
| Help A | Inline art for every token: 1 E, 2 E, 3 Alt (the `L-Alt` art), 4 M1 (`Mouse1`), 5 Q, 6 M1, 7 R. |
| Help B | 8 `[Zoom In]` and 9 `[Multi-Screenshot]` kept as written; 10 `[Move]` drew the **Mouse** art (`MouseMove`); 11–13 kept as written. |

What this shows:

- **No plain-text resolution.** Message boxes (text and buttons),
  notifications, the objective banner and the journal show control tags
  exactly as written. Only the help hint (the movie's own art holder)
  replaces them. This engine's plain-text labels are wrong for all of these
  surfaces. Loading screens weren't covered.
- **Message boxes don't show `MESG` `FULL`.** This engine has prepended the
  title since `b15647d`.
- **Art matching.** Lower case (2) matches. A key name (6) draws its art. An
  Item Menus event (7) follows `!0,Ready Weapon` to R. Sprint's `L-Alt` art is
  drawn as "Alt".
- **The wheel (8) gets no art.** This engine sends `Wheel` (since `df8fa0f`),
  which would draw art. SE's result fits the callback answering with the key
  table's name (`MouseWheelUp`, for which there is no bitmap, so the movie
  keeps the tag), or with nothing. It doesn't fit `Wheel`.
- **The chord (9) gets no art.** That fits PrintSrc's table name (no bitmap)
  or nothing. It doesn't fit this engine's `UnknownKey`.
- **Move (10) isn't unbound.** In Main Gameplay, Move has no keyboard or mouse
  binding. A later context binds it to mouse 0xA (`MouseMove`), and SE drew
  that art. So the lookup doesn't stop at the first context naming the event
  when it is unbound there. This engine sends `UnknownKey`.

### Round 2 and the rest of round 1 (2026-11-03)

- **Notification B** (round 1 rerun, Activate on E): as written, all brackets.
- **Rebinding.** NumPad 1–8 can't be bound (the game reserves them), so the
  user bound Activate to **NumPad9**. Help A had been showing E art for 1 and
  2. After the rebind it **did not change at all** while it stayed up: SE
  doesn't redraw a hint already on screen.
- **Journal after the rebind:** unchanged, every tag still as written.
- Not observed: a help message shown fresh after the rebind. NumPad9 has
  exported art anyway, so it can't tell `UnknownKey` from the key table's
  name for a key without art. A key with no art that can be bound (NumPad
  Enter, NumLock or Apps) would.

## Validation

These checks predate the SE observations: the box and plain-text results
below (Activate E to J) are from before `0b4bc53`, which shows tags as
written there instead.

- `cargo test --workspace` passed: core 616 passed / 412 ignored; tools
  53 passed / 3 ignored. Targeted ignored tests passed separately: messages 9,
  loading 3, resets 4, save 1, HUD 1.
- Tools-local tests cover context order/event precedence, references retained in
  classification, literal keys, unknown names and bracket extraction.
- The probe's Papyrus source compiles against the vanilla native declarations
  and its calls are checked against them; the plugin was generated after
  rebuilding `vrm-tool` (revised 2026-11-03: journal text, edge-case events,
  unknown tags and the rebind round).
- The survey completed against the external installation; missing arguments
  return a usage error rather than indexing past the argument list. Before/after
  surveys were identical: 4,889 records, 43 HMPC help forms, 130 candidates
  (45 events, 7 keys, 78 unmatched). The exceptions documented above, including
  `Mouse Move`, `ShowOnMap`, `Modded` and book markup, remain expected.
- Actual offscreen runs succeeded using external assets and a custom
  `VRM_CONTROLS` map changing Activate E to J (`0x24`). The plain box's
  Activate changed E to J; the other labels stayed L-Alt, Mouse1, Q, Mouse1,
  R. The help hint's Activate art changed E to J too. The captures were
  scratch output and are not kept; no game assets were copied and no original
  game runs were performed.
- Follow-up review fix `ab41fc1` returns an empty result for unknown art tags,
  allowing the movie to preserve the original brackets, and unifies HUD Activate
  with the control-art resolver. This implementation policy is not evidence of
  the native callback result. The full step/commit schedule is in the plan.
- Follow-up fix `df8fa0f`: button art uses the names the movies export
  (`Wheel`, `Enter`, `UnknownKey` for keys without art) rather than the key
  tables' names (`MouseWheelUp`, `NumPadEnter`), which have no bitmap and made
  the movie keep the tag. All three art movies export `Wheel.png` and none
  exports `MouseWheelUp`, `MouseWheelDown` or `NumPadEnter`. The tables are
  read by tab, so names with spaces (`Prev Track`) load.
- The manual Skyrim SE protocol above remains pending user screenshots. Native
  plain labels, unbound/unknown callback behavior, notification preprocessing,
  chords and localization remain unanswered by these implementation checks.
