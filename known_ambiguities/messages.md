# Message boxes, help messages and the quest journal

Implemented in `src/messages.rs` (message boxes, help messages, text tags,
banners) and `src/journal.rs` (log entries, objectives, the journal's quests);
drawn in `src/ui.rs`. Sources: the Creation Kit wiki pages for `Message.Show`,
`Message.ShowAsHelpMessage`, `Message.ResetHelpMessage`,
`Quest.SetObjectiveDisplayed` / `Completed` / `Failed` and Text Replacement;
UESP's MESG and QUST record layouts; the game settings (`sOk`,
`sQuestAddedText`, `sQuestCompletedText`, `sQuestFailed`, `sHUDCompleted`,
`sMiscQuestName`); and the game's data (`vrm-tool messages`, `vrm-tool
quest-log`, console `sqs`).

## Notification delivery

Notifications have sequence numbers independent of their display timestamps.
This prevents this engine's HUD from losing a notification at game time zero
or one queued after the preceding HUD update without the clock advancing
(for example, while paused). Retention and the egui fade still use game time.
This is an internal delivery fix; the menu-pause timing question below remains
open.

`Message.Show` now reads notification display time from MESG `TNAM`, an
unsigned 32-bit integer in seconds. The notification queue retains each entry
for its own display time, and the fallback egui HUD fades it out at that expiry.
`Debug.Notification` and notifications without a complete `TNAM` keep this
engine's existing six-second default. Message boxes still wait for a button;
their `TNAM` does not close them.

Format source: [xEdit's public TES5 MESG definition](https://github.com/TES5Edit/TES5Edit/blob/dev-4.1.5/Core/wbDefinitionsTES5.pas)
(`wbRecord(MESG)`, `TNAM` is `itU32`, `INAM` is `Icon (unused)`). The editor
describes Display Time as seconds for a non-box message; see
[Creation Kit Message](https://ck.uesp.net/wiki/Message).
`INAM` is an unused leftover in Skyrim, so no icon lookup is implemented.

The game's HUD movie still controls its own notification animations. Inspection
of the locally extracted **game asset** `Interface/hudmenu.swf` (the existing
`spike/swf/out/hudmenu.dis`) shows that `HUDMovieBase.ShowMessage(asMessage)`
takes only text and appends it to `MessagesInstance.MessageArray`.
`Messages.Update` creates `MessageText` clips and starts their `FadeOut`
timeline; it does not consume a per-message duration. No duration argument is
invented for that call. Expired notifications are skipped before delivery, but
once delivered to the SWF, an authored `TNAM` cannot yet change its lifetime.
Resolving how the original game reconciles TNAM with this asset's fixed
animation requires public evidence or further asset work; no engine binary
inspection was performed. No game assets or asset disassemblies are added by
this change.

Further asset inspection (2026-10-20) narrows this blocker: the `Messages`
class has `MAX_SHOWN = 4` and `END_ANIM_FRAME = 80`. Its `Update` method
attaches a `MessageText` clip, shifts one text string out of `MessageArray`
into the clip's text field, and stores the clip in `ShownMessageArray`.
The `MessageText` timeline labels `FadeIn` at frame 1 and `FadeOut` at
frame 15. `Update` starts the oldest clip's `FadeOut` when no message is
queued and it is not already animating; its spacing callback can also
start that fade when all four slots are occupied. Clips reaching frame
80 are removed. Thus this asset's queue and timeline, including the
arrival of other messages, participate in expiry; this is not simply a
missing duration argument on `ShowMessage`.

These findings come from the existing local asset disassembly's
`__Packages.Messages`, `MessageText`, and `HUDMovieBase.ShowMessage`
sections (`spike/swf/out/hudmenu.dis`, produced by the documented
`spike/swf` tooling). They establish the movie's behavior, not how Skyrim's
native HUD code applies `TNAM`. Neither the public record schema nor this
asset establishes that native-to-movie mechanism. Holding clips on a frame,
rewriting queue entries, or adding a duration parameter would be a new
compatibility policy rather than a verified implementation. The SWF
display-time roadmap item remains research-gated; no engine binary was
examined and no asset or disassembly is committed.

## Settled from the data

- **Every passing log entry applies, not only the first.** Stages put an
  unconditional entry first (completing the quest, with a fragment) and
  conditional entries after it holding the journal text and more fragments
  (`MQ102` stage 160: entry 0 completes the quest, entries 1 and 2 run the
  Hadvar / Ralof fragments, entries 3 to 5 hold the summary for each case). With
  "first passing entry wins" none of those could ever run, so each entry whose
  conditions pass adds its text, applies its flags and runs its fragment, in
  order. Before this, every fragment of a stage ran whatever its entry's
  conditions.

## Open

- **Display-time edge cases.** The original game's handling of missing,
  truncated, zero or extremely large `TNAM` values, exact fade timing, and
  whether queueing delays count toward the authored display time are not
  documented by the format schema. Here zero expires immediately, malformed
  data uses the existing default, timing starts when queued, and the fallback
  HUD keeps its existing 1.5-second fade (shortened for shorter durations).
- **Help messages "done".** Here an input event counts only while its help
  message is up (shown or waiting to come back); after that the event's help
  doesn't show again until `ResetHelpMessage`. The wiki doesn't say whether the
  game also counts the event when the player did it before any help was shown.
  Fishing (`ccBGSSSE001_FishingSystemScript`) resets `Activate` before each of
  its prompts, which fits either reading.
- **Only one help message at a time?** The latest due one shows; several
  events can wait at once.
- **Quest types.** Quests without a type stay out of the journal and get no
  banners, but their objectives still show in the HUD. Whether the game shows
  them there is unconfirmed.
- **The journal's log.** The latest entry shows first, the earlier ones greyed
  below. Entries are written as "the story so far", so the game may show only
  the latest.
- **Banners.** "Quest added" shows the first time a quest with a type gets a
  log entry or a displayed objective; completing and failing show their
  banners. When exactly the game shows them, and its objective HUD wording
  (here `Completed: <objective>`), aren't documented.
- **Text kept as it was.** Log entries and objectives keep their alias names as
  they were when added (the INDX "keep instance data" flag isn't read).
  `<Global=...>` works in any quest's text, not only for the globals the quest
  lists (`QTGL`).
- **Menu mode** (settled: every menu here but dialogue pauses the game, per
  the `kPausesGame` flags in CommonLibSSE's menu headers: `MessageBoxMenu`,
  `JournalMenu`, `InventoryMenu`, `ContainerMenu`, `BookMenu`,
  `LockpickingMenu`, `Console`; `DialogueMenu` doesn't). In menu mode the world,
  the game clock and the real-time clock timers and `Wait` count on stand
  still; scripts and events run on, `WaitMenuMode` counts real time, and
  `GetCurrentRealTime` / `GetRealHoursSpent` include menus (the CK wiki: `Wait`
  doesn't return in menu mode, `WaitMenuMode` does). Still open: whether world
  sounds already playing pause with the game (here they play on, as music
  does), and notifications and help messages, whose times stand still in menus
  here.
- **Offscreen runs answer message boxes.** Nobody can press a button in a
  `--screenshot --wait` run and the boxes pause the world (the Survival Mode
  prompt comes up at the start), so a box left up two seconds gets its last
  button pressed unless `--hold-boxes`. Not the game's behaviour; a test hook.
- **Controls in text.** `[Activate]` and the rest show this engine's keys
  (`messages::control_key`); gamepad-only names (`[XButton]`...) stay as
  written.
