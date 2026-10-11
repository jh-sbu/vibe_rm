# The tween menu (tweenmenu.swf)

Implemented in `src/swf_ui/tween_menu.rs`. Sources: the movie's
ActionScript (`TweenMenu`: the `StartOpenMenuAnim(levelUp, date, level,
percent)`, `StartCloseMenuAnim`, `ShowMenu`, `HideMenu` and
`ResetStatsButton` callbacks; the `HighlightMenu(n)`,
`OpenHighlightedMenu(n)`, `OpenAnimFinished`, `StartCloseMenu` and
`CloseMenu` calls; `FrameToLabelMap` None, Skills, Magic, Inventory, Map);
the control map (`Tween Menu` on Tab, `Quick Inventory` on I); the item and
magic menus' `ShowTweenMenu` / `CloseTweenMenu`.

## Unconfirmed: choices made

- **Opening.** Tab opens it from the world; I opens the inventory straight
  away. Every opening plays the movie's unfolding (`StartOpenMenuAnim`),
  coming back from a menu too; `ShowMenu` / `HideMenu` aren't used.
- **The bottom bar** is the journal's: the date, the level (with a level up
  waiting counted in) and the progress to the next. A level up waiting turns
  Skills into Level Up.
- **Choosing.** Skills opens the skills view (egui's, with the perk trees),
  Magic the magic menu, Items the inventory. The tween menu goes away while
  they are up. Map does nothing: there is no map yet.
- **Coming back.** The inventory's and magic menu's `ShowTweenMenu` (Left on
  the categories, clicking outside, Right in the magic menu) fade them out
  and open the tween menu again; their `CloseTweenMenu` (Tab) goes back to
  the world. The skills view goes straight back to the world.
- **Closing.** Tab (the movie's own) and Esc fold it away
  (`StartCloseMenuAnim`), then `CloseMenu`.
- **Sounds.** None: the game plays its own for opening, closing and
  highlighting, by names no public source gives.
- egui (`--egui-hud`) has no tween menu: Tab opens the inventory there.
