# The game's HUD (hudmenu.swf)

Implemented in `src/swf_ui.rs` (the engine's side) and `crates/ui-swf` (the
menus through the vendored Ruffle). Sources: the HUD movie itself
(`Interface/hudmenu.swf`): its ActionScript 2 (`swfspike dis`, see
`spike/swf/README.md`) for the functions, their parameters, the callbacks it
registers with `GameDelegate`, the clips it hands the game with
`RegisterHUDComponents` and their frame labels; the game settings for the
sneak texts (`sSneakHidden`, `sSneakCaution`, `sSneakDetected`) and the quest
banners' statuses (`sQuestAddedText`...). How the game itself calls the HUD
isn't public; the choices below are what the movie's code allows.

## Unconfirmed: choices made

- **The crosshair's text.** `SetCrosshairTarget`'s rollover text is the verb,
  a line break, then the name (`Talk<br>Lydia`); the activate key's art goes
  left of the first line, which is why the verb comes first. A crime's verb
  is red (`#E64640`, egui's colour before). Weight, value and the damage /
  armor field are left undefined, so the item line stays hidden.
- **Meters.** A changed health, magicka or stamina percent is sent without
  `abForce`, so the meter fades in, moves and fades out again, and stays up
  while the value keeps changing (regenerating). The first value sent is
  forced (no fade) only when full.
- **The compass.** `SetCompassAngle` gets the camera's heading (degrees
  clockwise from north) as both the player's and the compass's angle. No
  markers yet.
- **Notifications** (`Debug.Notification`) go to `ShowMessage`, each once.
  **Help messages** (`Message.ShowAsHelpMessage`) go to
  `ShowTutorialHintText(text, true)`, and `("", false)` when they go.
- **Quest banners** go to `QuestUpdateBaseInstance.ShowNotification` as a
  `QUEST_UPDATE` (type 0) with the quest's name as the text and the status
  ("Quest added") as the status, no sound, objectives or level, one at a
  time once `CanShowNotification()` says so.
- **The sneak eye.** `StealthMeterInstance` plays `FadeIn` when sneaking
  starts and `FadeOut` when it stops; `SneakAnimInstance` goes to frame
  `1 + 99 x` the eye's openness (frames 1 "invisible" to 100 "visible";
  what its frames 100 to 120 are for is unknown); the text is
  `sSneakHidden` while the eye is shut, `sSneakDetected` when fully open and
  `sSneakCaution` between.
- **Enemy health.** Shown (`EnemyHealth_mc._alpha` 100) while the actor the
  crosshair is on is hurt and alive, hidden (0) otherwise; a new target's
  percent goes to `EnemyHealthMeter.SetPercent`, a change to
  `SetTargetPercent`, the name to `BracketsInstance.RolloverNameInstance`.
  The brackets' width (frames up to "longest name") isn't set.
- **Modes.** `ShowElements("All", true)` at start, `"DialogueMode"` on and off
  with conversations; the HUD isn't drawn while the inventory, journal or
  skills menu is up. `SetPlatform(0, false)` (PC).
- **Key art.** `RefreshActivateButtonArt` gets the Activate event's key and
  `GetButtonFromUserEvent` (a help message's `[Event]`) is answered with the
  event's key, both as the art the menus export (`E`, `L-Shift`), from
  `Interface/Controls/PC/controlmap.txt`: the keyboard column (a
  DirectInput scan code; `!0,Other` refers to another event's; a
  combination shows its last key), else the mouse column, read as buttons
  0 to 7 (`Mouse1` to `Mouse8`), 8 and 9 the wheel and 0xA movement (a
  guess), else `UnknownKey`. The first context naming an event wins.
- **Sounds.** A menu's `PlaySound` plays the sound record of that editor ID
  at the camera.
