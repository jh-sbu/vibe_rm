# The journal (quest_journal.swf), quest tracking and statistics

Implemented in `src/swf_ui/journal_menu.rs` (the menu), `src/journal.rs`
(quests, tracking) and `src/stats.rs` (statistics, the calendar). Sources:
the movie's ActionScript 2 (`swfspike dis` folded with `fold.py`, see
`spike/swf/README.md`): `Quest_Journal`, `QuestsPage`, `StatsPage`,
`JournalBottomBar`, the callbacks they register and the fields they read
from the entries; the interface translations (`translate_english.txt`) for
the statistics' names; the CK wiki for `Quest.SetActive` / `IsActive` and
`Game.IncrementStat` / `QueryStat`. How the game calls the menu and builds
its data isn't public; the choices below are what the movie's code allows.

## Unconfirmed: choices made

- **Opening.** The game's side is `InitExtensions` and `SetPlatform(0,
  false)` on `_root.QuestJournalFader.Menu_mc`, then the
  `RestoreSavedSettings(tab, false)` callback with the tab the menu last
  closed on (`CloseMenu`'s first argument), as the item menus are opened.
- **The quest first selected.** `onQuestsDataComplete` selects the entry
  whose `formID` and `instance` match the first two values; before the
  journal was ever closed we send the latest quest still running (else 0,
  the miscellaneous entry). Undefined would select the completed quests'
  divider, which has no `formID`.
- **The description** is the quest's latest log entry only; earlier entries
  aren't shown.
- **Order.** Quests are listed by when their journal last changed, latest
  first (the menu then puts the completed ones after the divider, keeping
  that order). A quest's objectives are listed by index, highest first.
- **The objectives' states.** An objective is `active` (its frame with the
  marker) while it is displayed and not completed or failed, and its quest is
  tracked and not finished. The miscellaneous entry's objectives are those of
  running miscellaneous quests (type 6) not yet completed or failed, each
  `active` as its quest is tracked.
- **Tracking.** A quest is active (tracked) once it is added to the journal
  (its first log entry or objective); the miscellaneous objectives are shown
  until toggled off. Nothing marks tracked objectives yet (no compass or map
  markers).
- **Show on Map** is not offered (`abAllowShowOnMap` false): there is no map.
- **Closing.** Esc and the journal key make the game call `StartCloseMenu`,
  which runs the menu's own close (`CloseMenu`: its sound, then the game's
  `CloseMenu`); Tab is the menu's own. The menu closes at once, with no fade.
- **The date** on the bottom bar is "Sundas, 8:15 PM, 17th of Last Seed, 4E
  201": the `GameDay` / `GameMonth` / `GameYear` globals' starting values
  moved on by the days passed, with the months' lengths of the real calendar.
  Which weekday the game starts on isn't known: the first day is a Sundas.
  The month and weekday names are English (no game setting found for them).
- **The level meter** is the progress to the next level the player can take
  (pending level ups count as taken).
- **Statistics.** The page's categories (General, Quest, Combat, Magic,
  Crafting, Crime) are fixed by the movie; which statistics each lists and
  their order are as remembered from the game, not confirmed. Each is sent as
  (name, value, category, 0); the fourth value isn't read by the movie.
  Completing a quest counts in Quests Completed and in its type's statistic
  (main, side, the guilds, Daedric, Civil War, Dawnguard, Dragonborn);
  miscellaneous quests count each completed objective in Misc Objectives
  Completed instead. Questlines Completed is left to scripts.
- **The System page** shows, but saving, loading, settings, controls, help
  and quitting do nothing; the Creations entry is left out (`ShouldShowMod`
  false).
