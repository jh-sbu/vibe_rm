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

## Skyrim SE readings (autotest23, autotest24)

`research/autotest23.md` and `research/autotest24.md`, Disarm. Our
target-actor copy of the effect, cast by script from the player at NPCs
fighting a held target:

- A disarm sends `OnHit` with the unequip, and the weapon lands about 160
  units off at hand height (dz 67.5 above the feet). Each cast that left the
  weapon sent only `OnMagicEffectApply`. Ours sends `OnHit` with each
  disarm and none otherwise (`Engine::disarm`).
- Every disarm was within the cap (levels 1 and 10 at magnitude 12, 10 at
  30), but some casts within it left the weapon (levels 1 and 10). The cap
  stays as UESP has it. What else stops it is open: perhaps the target's
  action as it lands (CommonLibSSE-NG declares `DisarmEffect` a
  `StaggerEffect`). Ours always disarms within the cap.
- A disarmed level 1 NPC carrying a dagger fought on with nothing in hand
  for at least 4 s, not drawing the dagger or picking its sword up. Ours
  takes up what it still carries: differs, open (how long before it does).
- The shout's own spell (`VoiceDisarm1`) cast by script from 150 units did
  nothing at all.

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
  other weapons or its fists).
- No hit reaction (the target is not staggered) and no sound or art.
