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

## Choices

- A held scroll is an item (equipped and counted like a weapon), not a spell:
  `Actor.GetEquippedSpell` doesn't report it, and the "Favorite Spell" and
  "Favorite School" statistics don't count it (they count spells).
- The cast is the charge-and-release of a spell, with the scroll's own charge
  time; dual casting isn't offered for scrolls.
- The Scroll of Blizzard gives no skill experience: the page gives no amount.
- The inventory marks a held scroll with the plain "equipped" icon, not a
  hand's.

## Movement while casting

[UESP's Scrolls page](https://en.uesp.net/w/index.php?oldid=3543120)
says walking and running are unavailable during casting, but does not describe
other controls or exact animation boundaries. The local format reference and
record survey (`research/magic/scroll-casting-hold.md`) do not settle those
questions either. These are implementation choices pending manual SE observation:

- Hold forward/sideways movement, sprinting and new jumps from a successful
  cast start through charging and the wait for the animation's SpellFire event.
  Allow movement once the spell fires, without waiting for the release animation
  to finish. Early release or cancellation frees movement immediately after the
  casting state clears. Either hand can hold movement; concentration scrolls
  hold it while channeling.
- Turning and aiming remain available. Sneaking can still be toggled, but cannot
  produce movement. Clear sprint input before sprint processing so an attempted
  sprint neither drains stamina nor cancels sneaking during the hold.
- Clear movement input rather than freeze position or velocity: existing falls
  and other physics continue, and swimming receives the same input restriction.

These boundaries have not been verified in Skyrim SE. Jumping is blocked as a
conservative extension of the walking/running hold; preserving aim permits the
player to target the scroll while charging. No change to scroll consumption or
interruption by blows/shouts is part of this movement restriction.
