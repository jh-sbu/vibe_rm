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
- **Not done:** quest targets (`QSTA`: compass and map markers), tracking
  quests (`Quest.SetActive`), the journal's stats and system tabs,
  `Message.Show`'s icon (`INAM`), and the `TNAM` display time of
  notifications.
