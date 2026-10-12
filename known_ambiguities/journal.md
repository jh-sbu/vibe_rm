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

## Confirmed in the game (by the user, 2026-10-11)

- The game starts on a Sundas (17th of Last Seed, 4E 201).
- A quest's description is its latest log entry only, with the objectives
  listed apart (the miscellaneous entry has objectives only).
- New quests aren't tracked: The Golden Claw is untracked once picked up.
  The main quest is tracked because its stage fragments call `SetActive` on
  the next quest (`QF_MQ102` does `MQ103.SetActive(true)`), so the engine
  leaves tracking to the scripts and to Toggle Active.
- The journal key always opens the Quests tab; Esc opens the journal on the
  tab it was last closed on.

## Unconfirmed: choices made

- **Opening.** The game's side is `InitExtensions` and `SetPlatform(0,
  false)` on `_root.QuestJournalFader.Menu_mc`, then the
  `RestoreSavedSettings(tab, false)` callback: 0 for the journal key, the
  tab the menu last closed on (`CloseMenu`'s first argument) for Esc. The
  selected quest is kept either way.
- **The quest first selected.** `onQuestsDataComplete` selects the entry
  whose `formID` and `instance` match the first two values; before the
  journal was ever closed we send the latest quest still running (else 0,
  the miscellaneous entry). Undefined would select the completed quests'
  divider, which has no `formID`.
- **Order.** Quests are listed by when their journal last changed, latest
  first (the menu then puts the completed ones after the divider, keeping
  that order). A quest's objectives are listed by index, highest first.
- **The objectives' states.** An objective is `active` (its frame with the
  marker) while it is displayed and not completed or failed, and its quest is
  tracked and not finished. The miscellaneous entry's objectives are those of
  running miscellaneous quests (type 6) not yet completed or failed, each
  `active` as its quest is tracked.
- **Tracking.** The miscellaneous objectives are shown until toggled off.
- **Closing.** Esc and the journal key make the game call `StartCloseMenu`,
  which runs the menu's own close (`CloseMenu`: its sound, then the game's
  `CloseMenu`); Tab is the menu's own. The menu closes at once, with no fade.
- **The date** on the bottom bar is "Sundas, 8:15 PM, 17th of Last Seed, 4E
  201": the `GameDay` / `GameMonth` / `GameYear` globals' starting values
  moved on by the days passed, with the months' lengths of the real calendar.
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
- **What the engine counts** (the game's scripts count Horses Owned,
  Persuasions, Bribes and the rest they call `IncrementStat` for; when the
  game counts these isn't published):
  - kills by the player, by the first of the victim's actor type keywords
    in the order Dragon, NPC, Daedra, Dwarven, Undead, Animal, Creature
    (so a vampire is one of the People Killed); hares (`HareRace`) count in
    Bunnies Slaughtered too. Sneak Attacks count each of the player's.
  - Assaults, Murders and Trespasses count once when reported (a witness
    told a crime faction), Items Stolen every stolen item taken however
    unseen. Pockets Picked counts each successful theft from pockets,
    Items Pickpocketed the items in it (pickpocketed items aren't in Items
    Stolen). Fines Paid is the gold paid, Days Jailed the days served;
    Times Jailed and Jail Escapes count each.
  - Total Lifetime Bounty is all the bounty ever added (the infamy, with
    every faction); Largest Bounty the highest bounty with one faction at
    any time; the holds' Bounty statistics the bounty with each hold's crime
    faction now.
  - Gold Found is gold picked up or taken from containers and bodies (not
    the living, bought, given or a script's); Chests Looted counts each
    container (`CONT`) the first time anything is taken from it; Most Gold
    Carried is the most gold the player has held.
  - Books Read counts each book the first time it is read, Skill Books Read
    each skill book that taught; Skill Increases each skill point however
    gained; Food Eaten, Ingredients Eaten and Potions Used each one used;
    Barters each purchase or sale.
- **The System page**: the Creations entry is left out (`ShouldShowMod`
  false). With the game's menus, Esc no longer lets go of the mouse or
  quits: it opens the journal (egui's HUD keeps the old Esc).
