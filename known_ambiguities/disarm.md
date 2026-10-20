# Disarm

Implemented in `src/disarm.rs` (effect archetype 9, `VoiceDisarmEffect01` to
`03` of the Disarm shout, the draugr's too). Sources:
[UESP's Skyrim:Disarm](https://en.uesp.net/w/index.php?oldid=3246655)
(revision 3246655), [Skyrim:Statistics](https://en.uesp.net/w/index.php?oldid=3569833)
(revision 3569833: "Weapons Disarmed", "the number of NPCs you have
successfully disarmed"), CommonLibSSE's `EffectArchetypes` (kDisarm = 9), and
xEdit's `WEAP` `DNAM` definition (flags2 0x2000 "Bound Weapon", checked on
`BoundSword`'s weapon).

## What the sources say, and is implemented

- The weapon is pulled out of the target's hand; bows, crossbows and staves
  too; bound weapons are not affected; the left-hand weapon of a dual-wielder
  is "placed in the inventory instead of being dropped"; a target that
  sheathed in time (the effect finds no weapon in hand) keeps its weapons.
- The magnitude (12 / 20 / 30 in the three words' spells) is the highest
  level it reaches: "Disarms enemies up to level 12", and "Draugr stop using
  this shout after you reach level 31, as you are immune to it". It uses the
  same level cap as calm, fear and frenzy.
- The player is disarmed the same way (the drawn weapon is dropped); the
  player's count goes up for each actor they disarm.

## Unconfirmed choices

- **Where the weapon lands.** `Engine::drop_item`: in front of the owner, falling
  from chest height. The game probably throws it away from the caster; the
  force isn't documented.
- **Weapons that can't be dropped** (the `DNAM` "Can't Drop" flag) are taken out
  of the hand and stay in the inventory; UESP doesn't say.
- **Two-handed weapons and bows** are dropped as the right hand's weapon.
- **"Drawn" as the test.** UESP's note about sheathing just as the draugr
  stand up says the shout then does nothing; we test the weapon-drawn state
  when the effect lands, ignoring the half-second windup.
- **The player with no body graph** (console-only runs) never counts as having
  a weapon drawn, so is not disarmed.
- **What follows.** A disarmed NPC fights with what it still carries (its
  other weapons or its fists), and picking the dropped weapon up again
  ("Taking an enemy's weapon if you have lost yours", Salma in Ironbind
  Barrow) isn't built.
- No hit reaction (the target is not staggered), no `OnHit` and no sound or art.
