# Control tags in player-facing text

Step 0 of `plans/control-text-tags.md`, researched 2026-11-03. **Original-game
screenshots are pending.** Movie behavior below is asset evidence, not an
observation of native text replacement or of the native binding callback.

## Sources and method

Local UESP was searched first with `scripts/uesp.py search` for “control text
button tags”, “Control” (Skyrim Mod titles), and “ShowAsHelpMessage” (all
namespaces). No specification was found; the last search returned a user-talk
script fragment, not authoritative behavior. No public source code was imported
or used as a behavior specification (including no SKSE source).

The installed external Data directory was
`/home/frieren/.local/share/Steam/steamapps/common/Skyrim Special Edition/Data`.
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

## Manual Skyrim SE observation (user action pending)

Generated with `cargo build -p vrm-tools` **before** running:

```sh
target/debug/vrm-tool test-mod controltags "<Skyrim SE>/Data" tests/assets/VrmControlTags.esp
```

Outputs are the generated plugin and `tests/assets/Scripts/VrmControlTagsScript.pex`.
They are authored test artifacts, not copied game assets. The generator does
not install anything or run the game. Only vanilla Papyrus natives are used;
there is no rendered-text readback and no claimed automated visual result.

Use a disposable save and keyboard/mouse mode. The user installs the generated
files by their normal test procedure. Start the quest if necessary:

```text
startquest VrmControlTagsQuest
cqf VrmControlTagsQuest Probe
```

Close the console immediately. After five seconds a **MESG message box** appears;
screenshot it, then dismiss it. Two seconds later a **MESG notification** appears;
screenshot promptly. Eight seconds later a **ShowAsHelpMessage** hint appears for
20 seconds; screenshot it. Each surface contains exactly these same six tags:
`[Activate] [Sprint] [Right Attack/Block] [Favorites] [Mouse1] [XButton]`.
The probe is manual and repeatable, not an automatic startup interruption.

After all hints expire, rebind Activate to one unused, chord-free keyboard key
and remove Sprint's keyboard binding, leaving its keyboard mapping unbound
rather than merely moving it. Record the actual final mappings and how they
were obtained. If vanilla's controls UI cannot make a binding unbound, report
that limitation; **do not use SKSE** and do not substitute a different experiment
without recording it. Run the same `cqf ... Probe` command again and close the
console. The help-message event is reset on every invocation so its maximum
count does not suppress the second run. Do not overlap invocations.

For each before/after screenshot, record each token's visible text/art or
unchanged brackets, clipping, the selected input device, and the new Activate
key / Sprint state. In particular: does the box show labels or art; does the
notification contain images; does the hint draw inline art; what does unbound
Sprint show; does XButton resolve to the keyboard Ready Weapon control? Restore
the user's controls afterwards. If a hint is suppressed by game settings or
another tutorial, report that rather than treating absence as evidence about
its tags. Screenshots can be placed in `tests/results/`; this visual-only probe
has no numeric Papyrus report for `test-results` to extract.

Unresolved pending observation: native plain-text labels, unknown/unbound native
callback results, engine-side notification preprocessing, and chord labels. The
six-tag chord-free probe does not settle chords or localization. No engine
behavior choice has been made on those questions here.

## Validation

- `cargo test -p vrm-tools`: 53 passed, three existing asset-dependent tests ignored.
- Tools-local tests cover context order/event precedence, references retained in
  classification, literal keys, unknown names and bracket extraction.
- The probe PEX builds, parses and is checked against the existing vanilla native
  signature table; the plugin was successfully generated after the required build.
- The survey completed against the external installation; missing arguments
  return a usage error rather than indexing past the argument list.
