# Player controls disabled by scripts

Implemented in `src/player.rs` (`DisabledControls`), checked in
`Engine::update`, `Engine::activate`, the player's attacks
(`src/ai/combat.rs`, `src/ai/archery.rs`) and the window's input
(`src/app.rs`). Sources: the Creation Kit wiki pages for
`Game.DisablePlayerControls`, `Game.EnablePlayerControls` and the
`Is...ControlsEnabled` functions (argument order and defaults).

## Open

- **Arguments given false.** Here a false argument leaves that control as it
  was: `DisablePlayerControls(false, true)` doesn't re-enable movement. The wiki
  doesn't say whether the game does the same or sets every control from the
  arguments.
- **Several scripts at once.** One shared state: any script's
  `EnablePlayerControls` undoes every other script's disabling. Whether the
  game counts requests per caller isn't documented for Skyrim (Fallout 4's
  input enable layers do).
- **Fighting.** Disabling it stops attacks, bashes, blocking and shooting but
  doesn't put a drawn weapon away; whether the game sheathes it is unconfirmed.
- **`aiDisablePOVType` and camera switching** are stored but do nothing:
  there's no third-person camera yet. Journal disables opening the journal (J);
  it has no tabs to disable.
- **Menus** only covers the inventory (Tab); the console stays open to the
  developer.
