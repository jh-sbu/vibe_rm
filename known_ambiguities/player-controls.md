# Player controls disabled by scripts

Implemented in `src/player.rs` (`DisabledControls`), checked in
`Engine::update`, `Engine::activate`, the player's attacks
(`src/ai/combat.rs`, `src/ai/archery.rs`) and the window's input
(`src/app.rs`). Sources: the Creation Kit wiki pages for
`Game.DisablePlayerControls`, `Game.EnablePlayerControls` and the
`Is...ControlsEnabled` functions (argument order and defaults).

## Settled in the game (`test-mod autotest`, 2026-10-12)

- **Arguments given false** leave that control as it was:
  `DisablePlayerControls(false, true)` doesn't re-enable movement, and
  `EnablePlayerControls(false, true)` leaves it disabled.
- **Several scripts at once** share one state: any script's
  `EnablePlayerControls` undoes every other script's disabling.
- **Fighting** disabled sheathes a drawn weapon.

## Open (choices made here)

- **Journal** disables opening the journal (J); it has no tabs to disable.
- **Menus** covers the tween menu (Tab), the inventory (I), the magic menu
  (P), the favorites menu (Q) and the skills (K); the console stays open to
  the developer.
