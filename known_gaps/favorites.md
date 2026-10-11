# The favorites menu (favoritesmenu.swf) and hotkeys

Implemented in `src/swf_ui/favorites_menu.rs` (the menu) and `src/items.rs`
(`toggle_favorite`, `use_hotkey`). Sources: the movie's ActionScript
(`FavoritesMenu`: the `PopulateItems`, `SetSelectedItem` and `StartFadeOut`
callbacks, the `ItemSelect` and `FadeDone(index)` calls, Tab fading it out;
`FavoritesCenteredList`: entries' `text`, `hotkey` (0 to 7 shown as "1." to
"8."), `equipState`, sorted by name); the control map (`Favorites` on Q,
`Hotkey1`..`Hotkey8` on 1 to 8, the favorites menu's Cancel on Favorites,
Tween Menu and Pause).

## Unconfirmed: choices made

- **Filling the list.** `PopulateItems` names a function the movie doesn't
  have, so the game must fill the list itself: the entries go into
  `List_mc.entryList` and `List_mc.InvalidateData()` shows them. An entry's
  `index` is its place in the engine's rows.
- **What is listed.** The favorite items the player carries (a count over
  one after the name, "Potion of Healing (3)") and the favorite spells and
  powers they know. An item that leaves the inventory stays a favorite and
  shows again when it comes back.
- **Accept** does what the right attack button does. The attack buttons
  (the control map's) equip in their hand: spells in that hand, powers as
  the voice, items as the inventory's Accept does (one hand: weapons in the
  right). Equipping again takes it off or puts the spell away. The mouse
  buttons equip the selected entry wherever the cursor is.
- **The menu stays up** after equipping; Tab, Esc and Q fade it out. It
  opens on the entry selected when it last closed.
- **Hotkeys.** 1 to 8 in the menu put the selected favorite on that key,
  taking it off any other key; unmarking a favorite drops its hotkey. Only
  the first key of each `HotkeyN` is read, not the numpad's. In the world a
  hotkey does what Accept would (so pressing it again takes the item off),
  a spell going to the right hand; in dialogue the number keys pick topics
  instead.
- **Sounds.** Only the movie's own (moving through the list); none on
  opening, closing or equipping.
- egui (`--egui-hud`) has no favorites menu.
