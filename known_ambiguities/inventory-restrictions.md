# Weapon inventory restrictions

Implemented in `src/world/inventory.rs`, `src/items.rs` and `src/loose.rs`.
Sources: [xEdit's public TES5 record definitions](https://github.com/TES5Edit/TES5Edit/blob/dev-4.1.6/Core/wbDefinitionsTES5.pas)
(`WEAP` `DNAM`) and [CommonLibSSE-NG's public weapon header](https://github.com/CharmedBaryon/CommonLibSSE-NG/blob/main/include/RE/T/TESObjectWEAP.h).
[First-hand CK modding discussion](https://forums.nexusmods.com/topic/1000400-bound-weapons-do-not-unsummon-on-sheath/)
describes clearing Can't Drop and Bound Weapon and setting Playable on a
bound bow to make it behave as a normal inventory weapon.

## Settled from the records

- `DNAM`'s 16-bit flags at byte 12 include Can't Drop (0x08) and
  Non-playable (0x80). The weapon record header also has Non-playable (0x04).
- Vanilla `BoundWeaponSword` sets both Can't Drop and Non-playable (0x88).
  Inventory lists now omit non-playable weapons, including NPC weapons;
  these lists serve inventory, containers, barter, gifts and pickpocketing.
  Menu transfers also reject non-playable weapons. Inventory counts,
  equipping, combat and effect cleanup still use the carried items.
- Dropping checks Can't Drop before removing anything or creating a world
  reference. The flag is checked independently of Non-playable: a mod can
  author a visible weapon that cannot be dropped.

## Verification in this engine (2026-10-15)

An offscreen run using the installed game records cast Bound Sword: its
weapon was equipped and `GetItemCount` returned 1, but it was absent from
the inventory list. Dropping it left the count at 1; dispelling removed
it, returning 0. An ordinary Iron Sword dropped successfully and its count
became 0. The run also finished rendering in a debug build after fixing
the egui painter's cleanup of applied texture deltas.

## Open

- Script `RemoveItem` and `RemoveAllItems` still operate on these weapons,
  including transfers requested explicitly by scripts. Whether Skyrim
  restricts these operations for bound weapons needs a public source or
  an in-game comparison. Effect expiry must remain able to remove them.
- `DropObject` and the developer console's `drop` share the drop check here;
  whether the original console bypasses Can't Drop is unconfirmed.
- A mod's playable bound weapon is governed by its authored flags; no
  extra transfer restriction is inferred just from the Bound Weapon flag
  or an active bound-weapon effect. Feedback for trying to drop a visible
  Can't Drop weapon also needs an in-game comparison.

No engine binary was examined and no game assets were added to the repo.
