# Plan: control tags in text through the live input map

**Status: done, corrected by Skyrim SE observations** (2026-11-03). The
user's two probe rounds (`research/ui/control-tags.md`, "Skyrim SE
observations") showed that SE resolves tags only in the help hint, as button
art. `0b4bc53` follows that: plain text keeps tags as written, message boxes
drop the title, art uses the key tables' names, later contexts are tried,
and a hint already up isn't redrawn. The design below is kept as it was
planned; where it says "label", SE has none.

Message, help, loading and quest text names controls in square brackets
(`[Activate]`, `[Right Attack/Block]`). Previously this engine replaced them
with fixed labels from `messages::control_key`, so a rebound key still showed
its default name, and the HUD got plain text where its movie would draw button
art. The implementation resolves recognized tags through the live input map
(`Engine::input_map`), keeps tags in text until it is shown, and leaves them
for the movies that draw art themselves.

Rules as for all work here (AGENTS.md): public sources and game assets only.
Describe the movies' behaviour in prose; don't copy their bytecode into the
repository. Put choices made without a source in
`known_ambiguities/messages.md` (it already has "Controls in text"), and keep
open questions in this plan.

## What exists (2026-10-30)

- `src/messages.rs`: `control_key` maps 17 lower-cased event names to fixed
  labels ("E", "Left Mouse Button"...). `Engine::replace_text_tags` replaces
  `<Alias=...>` and `<Global=...>`, and `[Control]` with `control_key`.
  `replace_text_tags_preserving_controls` leaves the brackets alone.
- Callers that replace controls now: message boxes (text, title and buttons)
  and notifications (`messages.rs`, `show_message`), help messages
  (`show_help_message`), journal quest names, objectives and log entries
  (`journal.rs`, `swf_ui/journal_menu.rs`) and resets (`resets.rs`).
  Callers that keep them: the journal's Help topics (`system_help.rs`) and
  loading screen text (`loading.rs`, `next_loading_text`).
- `src/input_map.rs`: `InputMap` parses the game's controlmap columns plus the
  custom file, and `generation` goes up on every remap and reset.
  `bindings` / `first` only resolve events in context 0 (Main Gameplay),
  following `!ctx,Event` references into other contexts.
- `src/swf_ui.rs`: `Buttons` answers the movies' `GetButtonFromUserEvent`
  with art names (`E`, `L-Shift`, `Mouse1`, `UnknownKey`) from the live map,
  keyboard first and then the mouse. The HUD resends its Activate art when
  `generation` changes (`input_generation`).

## What the first look found (2026-10-30)

- **Movies that draw button art.** Of the interface movies in
  `Skyrim - Interface.bsa`, only `hudmenu.swf`, `quest_journal.swf` and
  `tutorialmenu.swf` contain `CreateButtonArt` / `GetButtonFromUserEvent`.
  `messagebox.swf` and `loadingmenu.swf` don't, so their text has to arrive
  already resolved. In `hudmenu.swf`, `CreateButtonArt`'s second occurrence
  sits just after `ShowTutorialHintText`. That suggests help messages are
  drawn with art, which `known_ambiguities/swf-hud.md` assumes too. Step 0
  confirms it. Help messages are currently resolved to text before the HUD
  sees them, so the HUD never draws that art.
- **Tags in the data.** `vrm-tool messages <Data>` counts the bracket tags
  in `MESG` text. There are four kinds:
  - Main Gameplay events (`Activate` 50, `Right Attack/Block` 47,
    `Left Attack/Block` 38, `Ready Weapon` 9, `Shout` 9, `Jump` 7, `Sneak`,
    `Sprint`...).
  - Events in other controlmap contexts (`Favorites`, `LocalMap`,
    `PlacePlayerMarker`, `Zoom In`, `RotatePick`, `RotateLock`,
    `ChargeItem`, `LeftEquip`, `RightEquip`, `NextPage`...).
  - Key names (`[Mouse1]`, `[F]`, `[C]`, `[R]`, `[1]`, `[2]`, `[8]`).
  - Gamepad names (`[XButton]` 7, `[YButton]` 6, `[Left Thumbstick]`,
    `[Right Thumbstick]`).
- **Key names.** `interface/controls/pc/keyboard_english.txt` and
  `mouse.txt` list each key's name, DirectInput code and Flash key code
  (`L-Shift 0x2a 16`, `Space 0x39 32`, `Mouse1 0x0 0`). These are the same
  names as the button art. The file's own comment says the name field "is
  used to ID events in the code", so whether the game shows these names as
  text is open.
- **Already-resolved text** sits in `Messages::help` and
  `Messages::boxes`, so a rebind doesn't reach it.

## Design

- **One resolver.** `InputMap::control(tag) -> Option<Control>` takes the
  bracketed name. An event in any context resolves to its bindings: the
  first context naming it wins, as `Buttons` does, and `!ctx,Event`
  references are followed. Otherwise a name in the key-name tables resolves
  to that key. Otherwise the result is `None` and the plain-text tag stays as
  written, which keeps unknown gamepad names until a gamepad exists. The art
  callback returns an empty result for unknown tags so the movie retains them.
- **Two renderings.** Movies that draw art get the tag unchanged; the art
  comes from `GetButtonFromUserEvent`, which already reads the live map.
  Plain-text movies get a text label: keyboard first, else mouse, from the
  key-name tables. Steps 0 and 1 settle the label for a chord or an unbound
  event, and the text for an unbound event's art (`UnknownKey`, as the movies
  already get).
- **Where each kind of text goes.**

  | Text | Shown by | Tags |
  | --- | --- | --- |
  | Help messages (`ShowAsHelpMessage`) | `hudmenu.swf` tutorial hint | kept, art (if step 0 confirms) |
  | Journal Help topics | `quest_journal.swf` | kept (as now) |
  | Tutorial menu text | `tutorialmenu.swf` | kept |
  | Message boxes | `messagebox.swf` | label, at display |
  | Notifications | `hudmenu.swf` `ShowMessage` | label at display, unless step 0 finds art |
  | Loading screen text | `loadingmenu.swf` | label (now kept: wrong) |
  | Quest names, objectives, log | `quest_journal.swf` lists | label at display, unless step 0 finds art |

- **Keep tags until display.** Alias and global tags are still replaced
  when the text is made, since aliases can change later. Control tags are
  stored as written and resolved where the text goes out, so a rebind or
  reset shows up in text that is already up.

## Steps

All implementation steps are complete (2026-11-03). Commit schedule:

| Step | Commit |
| --- | --- |
| 0: research, survey and manual probe | `6dae9fc` |
| 1a: cross-context event lookup | `e8e5225` |
| 1b: key names and resolver | `8e42960` |
| 2: text through the resolver | `4ae3768` |
| 3a: help tags for art | `b104bf6` |
| 3b: message boxes and notifications at display | `b15647d` |
| 3c: loading and journal text at display | `f4c6de1` |
| 4: generation-driven refresh | `b4bb90c` |
| Follow-up review fix | `ab41fc1` |
| SE observations applied | `0b4bc53` |
| Art from exported bitmaps; tab-separated key tables | `df8fa0f` |

The follow-up returns an empty result for unknown art tags (so the movie
retains their brackets) and unifies HUD Activate with the control-art resolver.
`df8fa0f` keeps labels from the key tables but takes art from the movies'
exported bitmap names, and reads the tables by tab.
Step 5 is this documentation finalization; the user-run SE observation is still
pending, not a prerequisite silently treated as completed. The original work
breakdown follows for traceability.

0. **Research and a survey tool.**
   - In `hudmenu.swf`, find what calls `CreateButtonArt`: the tutorial hint,
     notifications (`ShowMessage`) or anything else. Also find what
     `ButtonArtHolder` does with a tag it gets `UnknownKey` or an empty
     string for. Check the same for `quest_journal.swf`'s objective and log
     fields, not only its Help topics, and for `tutorialmenu.swf`.
   - Add `vrm-tool control-tags <Data>`, which goes over every record type
     with player-facing text: `MESG` `DESC` / `FULL` / `ITXT`, `LSCR`
     `DESC`, `QUST` objectives and log entries, `BOOK` `DESC`, and help
     topics. For each tag it prints the count, the record types, and what
     the tag resolves to with the shipped controlmap: an event (which
     context), a key name, or nothing.
   - Write it up as `research/ui/control-tags.md`.
   - Prepare a manual SE observation for the user, through
     `vrm-tool test-mod controltags`. Papyrus can't read rendered text, so
     the user watches and takes screenshots. The plugin shows one message
     box, one notification and one help message, each naming `[Activate]`,
     `[Sprint]`, `[Right Attack/Block]`, `[Favorites]`, `[Mouse1]` and
     `[XButton]`. It shows them again after the user rebinds Activate to a
     chord-free key and unbinds Sprint's keyboard key. The questions: what
     text a message box shows (key name, art, the tag?), whether
     notifications draw art, and what an unbound event shows.
   - Commit: the tool, the research note and the test plugin.
1. **The resolver** (`src/input_map.rs`).
   - 1a. Event lookup in any context: `bindings_of(event, device)` searches
     the contexts in order and resolves references, as `resolve` does.
     `first` and `bindings` keep their context-0 meaning for input
     dispatch. Unit tests: a menu-context event, a reference, an event
     missing from every context.
   - 1b. Key names: load `keyboard_english.txt` and `mouse.txt` from the
     VFS beside `controlmap.txt` (a code-to-name and name-to-code table),
     with a fallback to the art names `swf_ui::keyboard_art` /
     `mouse_art` already know. Add `InputMap::control(tag)` and
     `Control::label()`. Unit tests on synthetic tables: remapped,
     chord, unbound, key name, gamepad name.
2. **Text through the resolver.** Remove `messages::control_key`.
   `Engine::replace_tags` resolves `[...]` with `InputMap::control`, and
   tags it can't resolve stay as written. Replace the `control_key`
   assertions in `messages.rs` tests with map-driven ones: the same tag
   before and after `remap`, and after `reset`. Record the label choices
   (chords, unbound, gamepad) in `known_ambiguities/messages.md`.
3. **Keep tags until display.**
   - 3a. Help messages: `show_help_message` keeps control tags, using
     `replace_text_tags_preserving_controls`. The HUD's tutorial hint gets
     the tags and draws art through `GetButtonFromUserEvent`. If step 0
     finds the hint draws no art, the HUD resolves labels instead (the
     same call as 3b).
   - 3b. Message boxes and notifications: `show_message` keeps control
     tags in `MessageBox` text, title and buttons and in the notification
     queue. The places that hand them to `messagebox.swf` and the HUD
     resolve them through one `Engine::resolve_controls(&str) -> String`.
     Saved message boxes (`restore_message_boxes`) keep tags as well, so
     older saves with resolved text still load as they are.
   - 3c. Loading text: `next_loading_text` resolves labels, because the
     loading movie draws no art. Journal quest names, objectives and log:
     resolve at display, or keep for art if step 0 finds the journal
     lists draw it.
4. **Refresh after a rebind.** The HUD resends the current help message
   when `input_map.generation` changes, beside its Activate art (today
   `sent.help` compares only the text). Any other text the HUD keeps
   (an open notification) is left as it is, because the game's notification
   timeline has no refresh call. The journal's lists are filled when they
   open, so they need nothing more. Engine test: show a help message, call
   `remap` and check the text the HUD gets next.
5. **Docs — complete (2026-11-03).** Updated the choices and implementation
   evidence in the ambiguity/research notes and moved the ROADMAP item to Done.
   The SE observation remains pending; add user screenshots and findings when
   supplied, without treating implementation captures as original-game evidence.

## Verification

Completed 2026-11-03 (this engine only):

- `cargo test --workspace` passed: core 616 passed / 412 ignored; tools
  53 passed / 3 ignored. Targeted ignored tests also passed: messages 9,
  loading 3, resets 4, save 1, HUD 1.
- Actual offscreen runs succeeded using external assets and a custom
  `VRM_CONTROLS` map rebinding Activate from E to J (`0x24`). The plain
  box changed E to J; other labels stayed L-Alt, Mouse1, Q, Mouse1, R. The
  help hint's Activate art also changed E to J. The captures were scratch
  output and are not kept. No game assets were copied and no original-game
  runs were performed.
- Before/after shipped-map surveys were identical: 4,889 records, 43 HMPC
  help forms, 130 candidates (45 events, 7 keys, 78 unmatched). Expected
  exceptions include `Mouse Move`, `ShowOnMap`, `Modded`, thumbstick names,
  `[Optional]`, book markup such as `[pagebreak]`, and editorial prose.
  Not every bracket candidate is a control; “all tags resolve except
  gamepad” is not the verification criterion. See the research survey.
- User Skyrim SE screenshots remain pending under the manual protocol in
  `research/ui/control-tags.md` (“Manual Skyrim SE observation”).

## Open questions

- What the hint draws for an event bound to a key without art: the key's
  name (the movie keeps the tag) or `UnknownKey`. A probe hint with Activate
  on NumPad Enter, NumLock or Apps would show it (NumPad 1–8 can't be bound).
- Whether a later context's keyboard binding outranks an earlier context's
  mouse binding.
- Loading screens weren't probed.
- Localised key names for other languages (`keyboard_french.txt`...) when
  the game language isn't English: follow the strings' language setting
  once there is one.
