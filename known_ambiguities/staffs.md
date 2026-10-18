# Staffs

Implemented in `src/staffs.rs` (the casting itself is `src/casting.rs`'s).
Sources: UESP's Skyrim:Staves (no magicka used, the staff's charge
depleted and the staff unusable when empty, the charge cost lower with
skill in the school, "about 1.7 times the uses" at 100, no skill gained,
one-handed and dual-wielded), the records (`WEAP` `DNAM` animation type 8,
`EITM`, `EAMT`: Staff of Flames 500, Firebolts 1000; `ENCH` `ENIT`: cost,
cast type, delivery, type 12 for staffs, charge time), the game's settings
(`sMagicCastInsufficientItemCharge`), `magicbehavior.hkx` (the hands'
release clips for hand type 8: `StfMagicRight_Release`, `SpellFire` at
0.34 s) and CommonLibSSE's `kRightItemCharge` / `kLeftItemCharge`.

## Choices made without a source

- **The cost** is the enchantment's `ENIT` cost (by the second for
  concentration staffs) times the spells' skill factor
  (`1 - (fMagicCasterPCSkillCostBase x skill) ^ fMagicPCSkillCostScale`:
  0.59 at 100, which gives UESP's 1.7 times the uses). Spell cost perks
  (`ModSpellCost`, the school's Novice .. Master perks) aren't applied;
  whether they are isn't published.
- **Too little charge**: a cast needs its whole cost (a concentration
  staff any charge at all, and stops when a frame's share can't be paid),
  and "Your item has insufficient charge" shows; the magicka meter doesn't
  blink.
- **Which staff's charge**: one of the carried staffs of a kind carries
  the charge spent (`Inventory::charges`); the others are full. Charge
  isn't carried through transfers yet: a staff dropped, stored, sold or
  given away is full again in its new inventory.
- **Magicka** keeps coming back while only staffs are being cast.
- **An empty hand beside a staff** is a staff's (8) to the graph, as with
  spells (`casting.md`): otherwise a staff in the right hand alone leaves
  the graph in `1HM_H2H_Readied`, outside the magic behaviour. Whether the
  game sets 8 or 9 (or 0) for the empty hand isn't known.
- **`RightItemCharge` / `LeftItemCharge`** are the weapon's in that hand:
  its charge left (its `EAMT` the most); `DamageActorValue` /
  `RestoreActorValue` on them spends or restores it.
- Staffs aren't dual cast (dual casting is for spells), and give no skill
  XP, Story Manager cast event or spell statistics.

## Still open

- NPCs fighting with staffs (Forsworn briarhearts, Falmer shamans, dragon
  priests); recharging with soul gems; the HUD's charge meters.
- Enchanted weapons' charge used by blows (their `EAMT` is read the same
  way, but blows don't spend it yet).
