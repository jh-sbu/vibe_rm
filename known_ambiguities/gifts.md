# Gifts (Actor.ShowGiftMenu, giftmenu.swf)

Implemented in `src/gift.rs` (the engine's side) and
`src/swf_ui/item_menu.rs` (the menu). Sources: the CK wiki for
`Actor.ShowGiftMenu(abGivingGift, apFilterList, abShowStolenItems,
abUseFavorPoints)` and `ActorBase.GetGiftFilter`; the game's scripts
(`FavorDialogueScript.GiveGift` caps the returned favor points at 100 and
turns them into Speech XP); the NPC records' `GNAM` (a form list of
keywords such as `VendorItemWeapon`, or items); the movie's ActionScript
(`GiftMenu` extends the inventory's `ItemMenu`: `SetMenuInfo(giving,
useFavorPoints)`, `UpdatePlayerInfo(favorPoints)`, `ItemSelect(amount)`).

## Unconfirmed: choices made

- **Favor points.** A gift to the actor earns its value (the item's gold
  value times the count given), when `abUseFavorPoints` is set. How the game
  counts them isn't published; the script caps the total at 100.
- **What is listed.** The giver's items (the player's when giving, the
  actor's when taking) that are in the filter list or have one of its
  keywords; all of them when there is no filter. Gold is never listed;
  stolen items only with `abShowStolenItems`.
- **Opening.** `InitExtensions` and `SetPlatform(0, false)` on
  `_root.Menu_mc` (the movie's root has no functions of its own, unlike the
  other item menus), the categories as the inventory's, then `SetMenuInfo`.
- **The bottom bar** gets the favor points so far through
  `UpdatePlayerInfo`; the movie only shows its gift frame with them.
