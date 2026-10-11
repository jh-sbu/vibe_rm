# The game's HUD, message box and inventory (hudmenu.swf, messagebox.swf, inventorymenu.swf)

Implemented in `src/swf_ui.rs` and `src/swf_ui/item_menu.rs` (the engine's
side) and `crates/ui-swf` (the menus through the vendored Ruffle). Sources:
the movies themselves (`Interface/hudmenu.swf`, `inventorymenu.swf`...):
their ActionScript 2 (`swfspike dis`, see `spike/swf/README.md`) for the
functions, their parameters, the callbacks they register with
`GameDelegate`, the fields they read from the objects the game hands them,
the clips the HUD hands the game with `RegisterHUDComponents` and their
frame labels; `controlmap.txt` for the keys; the game settings for the sneak
texts (`sSneakHidden`, `sSneakCaution`, `sSneakDetected`), the quest
banners' statuses (`sQuestAddedText`...) and the inventory's category names
(`sFavorites`, `sAll`, `sWeapons`...). How the game itself calls the menus
isn't public; the choices below are what the movies' code allows.

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
- **Message boxes.** The box in front goes to `messagebox.swf` as
  `_root.MessageMenu.SetMessage(text, false)` (line breaks `\r\n` as `\n`)
  and `setButtons(true, labels...)`: buttons side by side, the first one
  focused for the keyboard. The message's title isn't shown apart from its
  text (the game's box has no title of its own); boxes can't be cancelled
  (`setIsCancellable` is never called). `buttonPress(position)` answers with
  that button's index for `Message.Show`.
- **Answers.** A call that wants an answer (`GameDelegate.call` with a
  callback) is answered during the call, as the menus need: `GameDelegate`
  forgets the call once `ExternalInterface.call` returns, and the HUD reads
  the key art on the next line. This much is the movies' code; that the game
  answers inside the call follows from it.
- **Input to the menus.** While a message box or the inventory is up it takes
  the keyboard and mouse (the console key aside). Keys go as the control
  map's menu events (`crates/ui-swf/src/input.rs`; its guesses are in
  `spike/swf/README.md`, "Input"), or as typed while an input field has the
  focus.

## Unconfirmed: the inventory

- **Opening.** `_root.InitExtensions()`, `_root.SetPlatform(0, false)`, the
  `SetCategoriesList` callback, then the items (the items list's
  `entryList` emptied and refilled, then `InvalidateListData`) and
  `UpdatePlayerInfo`; the list positions saved on closing (`SaveIndices`) go
  back through `RestoreIndices` on the next opening. The menu is opened
  afresh each time.
- **Categories.** Favorites, All, Weapons, Apparel, Potions, Scrolls, Food,
  Ingredients, Books, Keys, Misc, named by their game settings. The filter
  flags are ours (the menu only tests them against each other, and takes the
  first category's flag for favorites); only All is listed when empty
  (`bDontHide`). Ammunition is listed with weapons, soul gems and torches with
  misc, food is a potion with `ENIT` flag 0x2; gold isn't listed (the bottom
  bar shows it).
- **Entries.** `text`, `count`, `equipState` (a weapon held: 3, right hand;
  anything else worn: 1), `filterFlag` (its category, and the favorites' flag),
  `favorite`, `enabled` (always), and our own `vrmRow` to find the row again.
  The list is refilled when the items, what is worn or the favorites change.
- **The item card** (`RequestItemCardInfo`): `type` (`ICT_*`), `name`,
  `count`, `value`, `weight`, `stolen`; armor's rating (its piece of the
  player's rating, skills and perks applied) and `armorChange` (the player's
  rating with it put on, minus now); a weapon's or ammo's base damage and a
  weapon's `damageChange` against the one held; `effects`: each magic effect's
  description with `<mag>` and `<dur>` filled in, Survival Mode's `[SURV=...]`
  parts dropped; a book's `CNAM` description; an ingredient's effects all
  "unknown" (`numItemEffects`); `potionType` from a potion's first effect
  (magicka 1, stamina 2, else health 0) for the bottom bar's meter. Scrolls,
  soul gems and torches get the misc card.
- **The bottom bar** (`UpdatePlayerInfo`): `armor` (the player's rating),
  `damage` (the held weapon's base damage, 0 unarmed), `encumbrance` (the
  items' weight), `maxEncumbrance` (`CarryWeight`), `gold`, and health,
  magicka and stamina with their maximums; no colours (white).
- **Keys.** Accept (E, Enter) is the menu's own `ItemSelect`: weapons, ammo
  and apparel are equipped or taken off, potions, food and ingredients used,
  books read (the egui book view, back to the inventory after). The control
  map's item menu events: the Ready Weapon key (R) calls the menu's
  `DropItem` (then `ItemDrop(amount)`), Toggle POV (F) marks or unmarks a
  favorite, the attack buttons call `AttemptEquip(slot, true)`, slot 0 for
  Left Attack/Block (the right mouse button) and 1 for Right Attack/Block;
  the slot isn't used yet (one hand). Tab is the menu's own (it fades out,
  then calls `CloseMenu`); Esc does the same through `StartMenuFade`. Left
  on the categories and clicking outside (`ShowTweenMenu`) close the menu,
  as there is no tween menu yet. `CheckForMouseEquip(button)` (a click on
  the item's model) is answered with `(button, false)`.
