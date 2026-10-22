# Platform, race, item and ownership status conditions

Implemented in `src/condition/mod.rs` (`function_value`); tests in
`src/condition/status_tests.rs`.

## Platform and gore

`IsWin32` (521), `IsXBox` (309), `IsPS3` (520) and `IsGoreDisabled` (547).

### What is known

- UESP Skyrim:Console (oldid 3628513, https://en.uesp.net/w/index.php?oldid=3628513):
  `IsWin32` "Returns true if the game is running on a Windows environment",
  `IsXBox` and `IsPS3` the same for their platforms.

### Unconfirmed choices

- This is a PC build, so `IsWin32` holds and the console checks fail. Whether
  Skyrim SE on Windows reports `IsWin32` true is documented only by the
  console wiki's description of the function, not observed.
- `IsGoreDisabled` has no description on UESP. Nothing here sets a gore
  setting, so it is always false (gore shown).

## GetIsPlayableRace

`GetIsPlayableRace` (254) is true when the subject's race has the `RACE` `DATA`
flag 0x1 ("Playable" in xEdit's `wbRACE`).

### What is known

- UESP Skyrim Mod:Mod File Format/Raw Function Data (oldid 3411473) gives
  `Ref.GetIsPlayableRace()`; no page describes its result.

### Unconfirmed choices

- The race is the one templates give (`race_of`, as `SameRace` reads it), not
  the base record's own `RNAM` as `GetIsRace` does. They differ only for
  template-inheriting NPCs.
- A subject with no race (not an actor) fails the condition.

## GetKeywordItemCount

`GetKeywordItemCount` (465) totals the counts of every carried item whose base
record lists the keyword (`KWDA`).

### What is known

- UESP Skyrim Mod:Mod File Format/Raw Function Data (oldid 3411473) gives
  `Ref.GetKeywordItemCount(Keyword)`; its result is not described.

### Unconfirmed choices

- It counts stacks, not stack kinds: two keyworded swords and three keyworded
  arrows are 5. The name and `GetItemCount` suggest this, nothing confirms it.
- Equipped items are carried items and count; so do items in a reference's
  starting inventory before it changes, as `GetItemCount` reads them.
- Whether a keyword list, rather than one keyword, is accepted is unknown; the
  parameter is a single keyword.

## HasTwoHandedWeaponEquipped and HasBoundWeaponEquipped

`HasTwoHandedWeaponEquipped` (636) and `HasBoundWeaponEquipped` (706, one
casting-source parameter).

### What is known

- UESP Skyrim:Console (oldid 3628513) lists both without descriptions; the
  raw function data (oldid 3411473) gives `Ref.HasTwoHandedWeaponEquipped()`.
- The parameter of `HasBoundWeaponEquipped` is xEdit's `ptCastingSource`
  (0 left hand, 1 right hand, 2 voice, as CommonLibSSE's `CastingSource`).
- Bound weapons are `WEAP` `DNAM` flags2 bit 0x2000 (xEdit's "Bound Weapon").

### Unconfirmed choices

- "Two-handed" is a weapon in the right hand whose animation type is
  greatsword or battleaxe / warhammer (5, 6). Whether bows, crossbows or
  staves count is unknown; they are left out because they are not melee
  two-handers and the animation types are named separately.
- A two-handed weapon is the right-hand item (`weapon_in_hand`, where the
  game places it), whether drawn or sheathed: "equipped" is not "drawn".
- `HasBoundWeaponEquipped` fails for the voice source and for any value other
  than 0-2. The hand is the engine's held slot, so a bound weapon the Bound
  archetype is still conjuring counts once it is in the hand.
