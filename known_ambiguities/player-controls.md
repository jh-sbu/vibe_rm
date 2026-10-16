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
  This observation did not test different `aiDisablePOVType` values.
- **Fighting** disabled sheathes a drawn weapon.

## POV ownership (public documentation and community observation)

The ninth argument, `aiDisablePOVType`, selects Script (0, the default)
or Werewolf (1), according to the Creation Kit documentation mirrored by
[Papyrus Index: disable](https://papyrus.bellcube.dev/skyrimse/script/game/function/disableplayercontrols/)
and [enable](https://papyrus.bellcube.dev/skyrimse/script/game/function/enableplayercontrols/).
A [firsthand console experiment](https://www.gamesas.com/help-fix-the-dawnguard-3rd-person-bug-t290187.html)
reports independently toggling the Script and Werewolf POV states and a
combination that prevents camera switching, though the author could not
recall which pair of values blocked switching. Together these support keeping
separate camera locks: enabling one owner leaves the other owner's lock
in place. This is an inference from public observations, not an in-game
verification here. Other selected controls remain shared across owners.

Both F and wheel camera input, the control description and
`IsCamSwitchControlsEnabled` use the combined camera lock. `ForceFirstPerson`
and `ForceThirdPerson` remain script commands independent of user input.

## Open (choices made here)

- **POV query**: public function documentation does not specify whether
  `IsCamSwitchControlsEnabled` reports both owners or Script alone. It
  reports whether user camera switching is available here (both owners).
- **Unknown POV types**: values outside 0 and 1 have no documented
  behavior. Here they leave camera locks unchanged; other selected controls
  still apply. No binary examination was used to fill this gap.
- **POV overlap**: mixed-owner enable/disable calls need a game test to
  confirm the inferred independent-lock behavior, including whether
  repeated calls count or simply overwrite an owner's state. Here each
  owner has one boolean, matching the observed shared state for scripts.

- **Journal** disables opening the journal (J); it has no tabs to disable.
- **Menus** covers the tween menu (Tab), the inventory (I), the magic menu
  (P), the favorites menu (Q) and the skills (K); the console stays open to
  the developer.

## HUD cart mode

`Game.SetHudCartMode(abSetCartMode = true)` now selects the HUD's cart
visibility group; false restores the ordinary group. This changes visibility,
not movement or other control flags. The requested state survives disabled
movement: `MovementDisabled` temporarily replaces `CartMode`, then cart mode
returns when movement is enabled. The default argument and this interaction
are documented by the [Creation Kit documentation mirrored in Papyrus
Index](https://papyrus.bellcube.dev/skyrimse/script/game/function/sethudcartmode/).

The game's UI asset code is publicly available in the [HUDMenu asset
reconstruction](https://github.com/Mardoxx/skyrimui/blob/master/src/hudmenu/HUDMenu.as),
`SetModes` and `ShowElements`. Cart mode retains notifications, subtitles and
help; movement-disabled mode also retains quest banners. Both hide the
crosshair, activation text, meters, sneak indicator and compass. The SWF path
uses these visibility groups; the egui fallback applies the same exclusions.
No asset code or game assets were added to the repository.

Unresolved: public documentation does not establish cart mode's precedence
against conversations or whether mode changes within one simulation frame
should be observed individually. Here disabled movement takes precedence,
then cart mode, then dialogue; the HUD sees the final state each frame. The
fallback conversation UI retains its existing separate presentation. Changes
to quest banners' expiration while hidden remain outside this implementation.
No engine executable or binary data was inspected.

## Player AI-driven mode remains pending

[SetPlayerAIDriven's public
contract](https://papyrus.bellcube.dev/skyrimse/script/game/function/setplayeraidriven/)
(default true) permits the player to run AI packages. The current package
scheduler operates on NPC actor runtimes; the player has a separate movement
and body controller. This needs player package execution, including scene and
quest-alias packages, before the native can be meaningfully supported. It has
not been replaced with a movement-disable toggle or an inert success stub.
Exact arbitration between user input and package motion remains unverified;
public information reviewed here does not settle it.
