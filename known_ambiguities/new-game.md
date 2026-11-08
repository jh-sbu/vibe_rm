# Starting a new game

Implemented in `src/new_game.rs` (2026-11-08): the main menu's New (and
`--new-game`) queues one load from the main menu, behind the loading screen.
The game's quests start, their startup stages run, and the player enters the
space a script moved them to. ROADMAP item 11; the main menu is
`plans/main-menu.md`.

## Evidence (game assets)

- `MQ101` (`Skyrim.esm` 0003372B, "Unbound") isn't start-game-enabled: its
  `DNAM` flags are 0x0100 (run once), without 0x0001. No vanilla script calls
  `Start` or `SetStage` on it to begin it: `vrm-tool pex-calls <Data> Start`
  and `SetStage` find only its own scripts and stages past the start. The
  `DOBJ` default objects don't name it. Yet every new game plays it (UESP,
  [Skyrim:Unbound](https://en.uesp.net/w/index.php?oldid=3575443): the game
  "always starts" on 17 Last Seed with this quest).
- Its stage 0 is flagged start-up (`INDX` flags 0x02). Its five log entries are
  conditioned on the `MQQuickstart` global (0004679E, 0 in the data) being 0
  to 4. Entry 0's fragment (`QF_MQ101_0003372B.Fragment_2`) sets `GameHour` to
  7 and sets stage 10.
- Stage 10 (`Fragment_4`) sets up the opening: `SetInChargen`,
  `DisablePlayerControls`, the player's inventory removed and the prisoner's
  clothes and cuffs equipped, the cast moved to their start markers, the player
  `MoveTo` the `PlayerStartMarker` alias (in Tamriel, a snowy mountain
  road), `SkyrimClear` forced, `FadeOutGame(false, true, 14, 15)`,
  `ShowTitleSequenceMenu`, the intro music and the cart sound. So the quest
  itself places the player. The data gives `PlayerRef` no placement.

## Choices made

- **The engine starts `MQ101`** on a new game, after the start-game-enabled
  quests. Nothing in the data starts it, so the engine must, but no public
  source says how or in what order. It is found by its `Skyrim.esm` form ID,
  not by its editor ID. Without `Skyrim.esm` there is no opening.
- **Before the world loads**, the startup stages run in one script update
  with no space loaded. The first space a script moves the player to is where
  the load goes. In vanilla that is `MQ101`'s `PlayerStartMarker`, after
  stage 0 and stage 10. When does SE run the stages relative to the load, and
  what is loaded before `MoveTo`? Unknown.
- **When nothing moves the player** (no opening, or an alternate start that
  moves the player later), they start where the developer launch puts them:
  Tamriel cell (4, -12), on the ground, facing north. SE's behaviour for a
  load order without `MQ101` isn't documented.
- **A new game that can't start** (no worldspace to enter) ends the session
  for the main menu with "The new game couldn't start." SE has no such
  message: the case can't happen with its data.
- **The main menu engine is the new game's.** No second teardown happens
  between the main menu and the new game: the main menu is already a fresh
  session (`into_main_menu`). The calendar is the data's (17 Last Seed,
  `GameDay` / `GameMonth`). UESP's note that the weekday carries over from a
  game loaded before isn't reproduced.
