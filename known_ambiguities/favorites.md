# The favorites menu (favoritesmenu.swf) and hotkeys

Implemented in `src/swf_ui/favorites_menu.rs` (the menu) and `src/items.rs`
(`toggle_favorite`, `use_hotkey`). Sources: the movie's ActionScript
(`FavoritesMenu`: the `PopulateItems`, `SetSelectedItem` and `StartFadeOut`
callbacks, the `ItemSelect` and `FadeDone(index)` calls, Tab fading it out;
`FavoritesCenteredList`: entries' `text`, `hotkey` (0 to 7 shown as "1." to
"8."), `equipState`, sorted by name); the control map (`Favorites` on Q,
`Hotkey1`..`Hotkey8` on 1 to 8, the favorites menu's Cancel on Favorites,
Tween Menu and Pause).

## Settled from the data

- **Keyboard alternatives.** The installed `Skyrim - Interface.bsa` asset
  `interface/controls/pc/controlmap.txt` gives `Hotkey1` through `Hotkey8`
  two comma-separated keyboard scan codes each: the number row and the
  corresponding numpad key. Both are accepted for assigning favorites and
  using them in the world. The first remains the key shown by button art.
  Custom control maps supply these bindings in both places, instead of the
  world always using the number row. `--menu-key Numpad1` through `Numpad8`
  are available for offscreen checks. No control map asset is stored here.

## Unconfirmed: choices made

- **Filling the list.** `PopulateItems` names a function the movie doesn't
  have, so the game must fill the list itself: the entries go into
  `List_mc.entryList` and `List_mc.InvalidateData()` shows them. An entry's
  `index` is its place in the engine's rows.
- **What is listed.** The favorite items the player carries and the
  favorite spells and powers they know. An item that leaves the inventory stays a favorite and
  shows again when it comes back.
- **Accept** readies a spell in the right hand and uses an item as the
  inventory's Accept does. The attack buttons (the control map's) equip in
  their hand: spells and weapons in that hand, powers as the voice, other
  items as Accept does (`known_ambiguities/hands.md`). Equipping again takes
  it off or puts the spell away. The mouse buttons equip the selected entry
  wherever the cursor is.
- **The menu stays up** after equipping; Tab, Esc and Q fade it out. It
  opens on the entry selected when it last closed.
- **Hotkeys.** 1 to 8 in the menu put the selected favorite on that key,
  taking it off any other key; unmarking a favorite drops its hotkey. In the world a
  hotkey does what Accept would (so pressing it again takes the item off),
  a spell going to the right hand; in dialogue the number keys pick topics
  instead.
- **Sounds.** Only the movie's own (moving through the list); none on
  opening, closing or equipping.
