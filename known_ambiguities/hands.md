# What actors hold: equip slots and hands

Implemented in `src/world/inventory.rs` (`equip_slot`, `Inventory::hold` /
`release`: one record of the right hand, the left and the voice, holding
weapons, spells, a shield, a torch, a power or shout), `src/items.rs`
(`equip_in`), `src/spells.rs` (`equip_spell`) and `src/ai/equipment.rs`
(models, torches). Sources: Skyrim.esm's equip slots (`EQUP`: `RightHand`
00013F42 and `LeftHand` 00013F43 with no parents; `EitherHand` 00013F44 and
`BothHands` 00013F45 with both as parents, `BothHands` with `DATA` flag 0x1;
`Shield` 000141E8 with `LeftHand` its parent and flag 0x1; `Voice` 00025BEE;
`Potion` 00035698), and what points at them (`ETYP`, counted over the load
order: weapons 1694 `RightHand`, 77 `EitherHand`, 1552 `BothHands`; spells
1094 `EitherHand`, 149 `LeftHand`, 54 `RightHand`, 51 `BothHands`, 104
`Voice`; shields `Shield`); CommonLibSSE (`BGSEquipSlot`: `PNAM` parents,
`DATA` flags use-all-parents / parents-optional / item-slot;
`AIProcess::equippedObjects` left and right; `ExtraWorn` / `ExtraWornLeft`;
`Actor::GetEquippedObject(leftHand)`); the CK wiki for `GetEquippedItemType`
(9 a spell, 10 a shield, 11 a torch), `GetEquippedWeapon(abLeftHand)` and
`GetEquippedShield`; the vanilla behaviour graphs' own condition
`(iWantBlock == 0) || (iLeftHandType == 7) || (iLeftHandType == 12)`; the
vanilla skeleton's nodes (`WEAPON` and `SHIELD` in the hands; `WeaponSword`,
`WeaponDagger`, `WeaponAxe`, `WeaponMace`, `WeaponBack`, `WeaponBow`,
`QUIVER` on the body, none for a second weapon); `inventorymenu.swf`
(`InventoryDefines.ES_NONE` .. `ES_BOTH_EQUIPPED`, `AttemptEquip(aiSlot)`
calling `ItemSelect([aiSlot])`, Accept calling `ItemSelect([])`).

## Unconfirmed: choices made

- **One-handed weapons go in either hand.** Nearly all of them name only
  `RightHand`, yet the game dual-wields them, so a weapon goes in the hand
  asked for (an attack button in the menus, `equipitem <item> left`) whatever
  its slot, unless its slot takes all its hands. What `EitherHand` changes for
  weapons isn't known; it is treated as `RightHand` is. Spells keep to their
  slot: `IncinerateLeftHand` goes in the left whichever hand is asked for.
- **Where it goes unasked** (Accept, `EquipItem`, NPCs, bound weapons): the
  right hand if its slot allows it, else its first hand. Accept on something
  equipped takes it off wherever it is held; an attack button on something
  in its own hand takes it out of that hand.
- **Displacing.** What goes in a hand puts away what was there; something
  filling both hands (a two-handed weapon, a both-hands spell) leaves both
  when either is taken. The only one of an item moves from the other hand;
  with two, one goes in each, and when one of a pair leaves the inventory
  the left hand empties. Spells aren't counted: the same one can be in both
  hands.
- **Hand types.** A two-handed weapon is reported in both hands (the graphs'
  blocking condition expects a bow in the left). The graphs get a spell's
  hand as empty (0), as casting doesn't go through the graph yet;
  `GetEquippedItemType` and the condition get 9. `GetEquippedWeapon(true)`
  is `None` with a two-handed weapon.
- **Equip icons.** A weapon in the right hand is `ES_RIGHT_EQUIPPED`, in the
  left `ES_LEFT_EQUIPPED`, one of a pair in both `ES_BOTH_EQUIPPED`; a
  two-handed weapon shows as the right hand's (the game marks such an item
  worn, not worn left); shields and torches are `ES_EQUIPPED`.
- **A left-hand weapon is shown drawn only**, in the `SHIELD` node, the
  bows' hand. Sheathed it isn't shown: the skeleton has no node on the body
  for it.
- **NPCs' torches** take the left hand and set the shield aside as before,
  but an NPC with both hands on a weapon (a two-handed one, a bow) doesn't
  light one: the torch would take the weapon out of its hands, and whether
  the game puts it away for a torch and back for a fight isn't known.
- **The mouse in item menus.** The movies equip on an attack button only
  with the cursor over the list (`ShouldProcessItemsListInput(true)` calls
  Scaleform's `Mouse.getTopMostEntity()`). The vendored Ruffle's version
  returns the topmost clip, button or text field under the cursor (any, or
  with its argument false only those with mouse handlers); Scaleform's other
  forms (a mouse index, a point) aren't done.
