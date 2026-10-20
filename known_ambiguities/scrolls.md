# Scrolls

Implemented in `src/items.rs` (using a scroll from the inventory or favorites
puts it in the hand asked for, or away), `src/casting.rs` (`use_up_scroll`,
no magicka cost) and `src/spells.rs` (`equipped_spell` counts a held scroll).
Source: [UESP's Skyrim:Scrolls](https://en.uesp.net/w/index.php?oldid=3543120)
(revision 3543120).

## From the source

- Anyone can use a scroll whatever their skill; casting costs no magicka; each
  scroll is used once, and one cut short before the cast is not used and has to
  be started again; selecting it in the inventory (or favorites) assigns it to
  a hand, and some need both hands (taken from the scroll's equip slot, `ETYP`,
  as for spells); casting one gives no magic skill experience, apart from
  the Scroll of Blizzard.

## Choices and gaps

- A held scroll is an item (equipped and counted like a weapon), not a spell:
  `Actor.GetEquippedSpell` doesn't report it, and the "Favorite Spell" and
  "Favorite School" statistics don't count it (they count spells).
- The cast is the charge-and-release of a spell, with the scroll's own charge
  time; dual casting isn't offered for scrolls.
- **Not built**: the player not being able to walk or run while casting a
  scroll (UESP says the casting animation holds them; the engine lets them
  move as with spells); the Scroll of Blizzard's skill experience (the page
  gives no amount); NPCs casting scrolls; what interrupts a charge (this engine's
  spells aren't interrupted by blows or shouts either, so scrolls aren't).
- The inventory marks a held scroll with the plain "equipped" icon, not a
  hand's.
- Torches (`LIGH`) can't be equipped from the inventory yet: the player's
  body doesn't show a carried light (see the roadmap).
