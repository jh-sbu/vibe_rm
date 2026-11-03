# Bow and crossbow ammunition: equipping and selection

What equips with a bow or crossbow, what the player's shot takes, and the
quiver. `src/ai/archery.rs` (`ammo_for`, `weakest_in`, `arrows_in`),
`src/items.rs` (`equip_in`), `src/world/actor.rs` (`dress`).

The SE observation `vrm-tool test-mod ammo` (`crates/tools/src/test_mods/ammo.rs`)
was built to settle the choices below; until its readings come in, each is
the choice recorded here.

## Public evidence

- Local UESP [Skyrim:Archery, revision 3575828](https://en.uesp.net/w/index.php?oldid=3575828),
  Notes: equipping a bow equips the weakest arrows in the inventory by
  default, and stronger ones can be chosen; crossbows do the same with bolts;
  arrows stay equipped (and on the character's back) when the bow is
  unequipped.
- Local UESP [Skyrim:Ammunition, revision 3607741](https://en.uesp.net/w/index.php?oldid=3607741):
  bows take arrows and crossbows bolts.
- Local UESP [Skyrim Mod:Mod File Format/AMMO, revision 2645195](https://en.uesp.net/w/index.php?oldid=2645195),
  `DATA`: projectile, flags (0x2 Non-Playable, 0x4 Non-Bolt), damage, value.
- Local UESP [Skyrim Mod:Mod File Format/WEAP, revision 3089110](https://en.uesp.net/w/index.php?oldid=3089110),
  `DNAM` animation type: 7 Bow, 9 Crossbow.
- Game setting `sNoArrows`: "No arrows equipped."
- Game assets: arrow and bolt models (`Weapons\Iron\IronArrow.nif`,
  `DLC01\Weapons\Crossbow\SteelBolt.nif` and others) have root `Prn` =
  `QUIVER`, an `ArrowQuiver` node (the quiver and its arrows) and a loose
  `Arrow:0` shape; bolts nest both under an `Arrow` node.

## Choices made

- **Weakest** is the lowest `DATA` damage among carried, playable ammunition
  the weapon shoots. Value isn't considered. Of equal damage, the first the
  inventory lists (the order stacks were first added) wins. The test's
  `Pricey` and `Tie` readings check both.
- **When.** Only the player's bow or crossbow auto-equips, by any route
  (menus, hotkeys, `EquipItem`, console), and only when no ammunition it
  shoots is worn: a stack the player chose stays, and worn bolts give way to
  arrows when a bow is equipped (`Chosen`, `Switch`, `Incompatible`).
  Unequipping the weapon leaves the ammunition worn (UESP).
- **Arrows picked up** while the bow is held don't equip themselves
  (`Pickup`).
- **Shots.** The player's shot takes only worn ammunition; with none worn
  it shows `sNoArrows` and nothing flies, as the setting's wording suggests.
  When the worn stack is used up, it comes off and no other stack is worn
  in its place, so the next shot shows `sNoArrows` (`RunOut`, `RemoveHeld`).
- **NPCs** keep their rule: worn arrows, else their strongest, else iron
  arrows (they never run out). UESP's note is about the player; NPCs' choice
  isn't observed and isn't part of the test.
- **The quiver** is drawn for any actor with ammunition worn and carried:
  the shapes under the model's `ArrowQuiver` node, hung from its `Prn`
  bone. It doesn't change with the count left.

## Remaining uncertainty

- All the choices above until the `ammo` readings come in.
- Crossbow firing and reloading aren't implemented; only the equipping of
  bolts is.
