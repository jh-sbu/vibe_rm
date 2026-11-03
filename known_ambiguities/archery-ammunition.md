# Bow and crossbow ammunition: equipping and selection

What equips with a bow or crossbow, what the player's shot takes, and the
quiver. `src/ai/archery.rs` (`ammo_for`, `weakest_in`, `arrows_in`),
`src/items.rs` (`equip_in`), `src/world/actor.rs` (`dress`).

Most of the rules were observed in Skyrim SE with `vrm-tool test-mod ammo`
(run 2026-11-03; readings in `crates/tools/src/test_mods/ammo.rs`): the
weakest by damage, the first-added stack of equals, auto-equip only when
nothing the weapon shoots is worn, worn bolts giving way to arrows, nothing
put on by pickups or a removed stack, and the weakest left put on silently
when shots use up the worn stack. This file keeps only the choices that run
didn't settle.

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

- **NPCs** keep their rule: worn arrows, else their strongest, else iron
  arrows (they never run out). UESP's note and the test are about the
  player; NPCs' choice isn't observed.
- **List order** for ties is the order stacks were first added to the
  inventory (`Inventory::items`). The test added each stack once; whether a
  stack that ran out and came back keeps its old place isn't observed.
- **The quiver** is drawn for any actor with ammunition worn and carried:
  the shapes under the model's `ArrowQuiver` node, hung from its `Prn`
  bone. It doesn't change with the count left; that wasn't observed.

## Remaining uncertainty

- Crossbow firing and reloading aren't implemented; only the equipping of
  bolts is.
