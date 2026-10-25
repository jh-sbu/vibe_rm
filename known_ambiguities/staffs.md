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
- **Which staff's charge**: each depleted copy has its own sparse value in
  `Inventory::charges`; copies without one are full. Casting, combat,
  actor values and recharging select the first depleted copy. Full copies
  leave first, then depleted copies in reverse insertion order, keeping
  the selected copy until last. Values go with copies stored, sold,
  given, taken or dropped, including when another depleted copy of the
  same kind is already at the destination. Fully recharging the selected copy
  removes its sparse entry and selects the next depleted copy, if any.
  The game's instance-selection order, including how charge attaches to
  separately owned copies and to two identical weapons in different
  hands, isn't established. Here two same-base weapons in different hands
  share the selected copy's charge; ownership and charge are independent,
  so owner-filtered removal uses the same full-first charge ordering.
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

## NPCs

The records give each combat style a staff score multiplier (`CSGD` 0x20:
`csDragonPriest` 10, `csSpellsword` 6.8, `csFalmerMagic` 5.35,
`csVampireMagic` 2.15, most 1, some 0) beside its melee, magic and ranged
ones; warlock bosses carry staffs half the time (`LItemStaffFirebolt50`...),
Forsworn mages from `LootStaffsWeaponForsworn`, Hevnoraak and Maluril their
own (`vrm-tool item-users`). How the game scores staffs against weapons and
spells isn't public. Here:

- **Taking one up**: an NPC carrying an enchanted staff takes up the most
  valuable one in its right hand (the left free for spells) when its
  style's staff multiplier is at least its melee and ranged ones; else it
  picks weapons as before and leaves staffs alone.
- **Fighting with it**: anyone with a staff in the right hand fights with
  its enchantment (if it is a hostile aimed or target actor one) while it
  has the charge for a cast, as casters do with spells: before its spells
  when the style's staff multiplier is at least its magic one, else when
  it has no spell it can pay for. Spells then go in the left hand.
- **Charge**: NPCs' staffs spend charge as the player's do (their cost by
  their own skill), and run dry.

## Dropping and picking up

Implemented in `src/loose.rs`, `src/items.rs`, `src/world_state.rs` and
`src/world/inventory.rs`. A dropped reference keeps one value for each
depleted copy in its stack; full copies need no entry. Picking it up
restores these values without changing already-carried copies. The values
live outside loaded-cell state, so unloading the cell doesn't discard them,
and native saves keep them with the world state.

Public evidence for charge belonging to the instance, including a world
reference:

- [CommonLibSSE-NG's `ExtraCharge.h`](https://github.com/CharmedBaryon/CommonLibSSE-NG/blob/main/include/RE/E/ExtraCharge.h)
  identifies an instance extra-data entry with a floating-point charge.
- [Creation Kit wiki: `GetItemCharge`](https://ck.uesp.net/wiki/GetItemCharge_-_ObjectReference)
  documents the remaining charge on an ObjectReference.

These establish instance state; retaining that state across drop/pickup is
the implementation's inference. The public sources above don't establish
the engine's exact drop-stack splitting or copy-selection order. A drop
still creates one reference for the whole requested stack, now retaining
every depleted copy's value. This work doesn't add charge overrides read
from placed plugin references, or poisoned weapons'
and soul gems' other per-copy state to dropped references.

## Enchanted weapons

UESP's Skyrim:Generic Magic Weapons gives each weapon "Charge/Cost =
Uses" ("1000/18=55"), the cost lowered by the enchantment's school skill
("about 1.7 times the uses" at 100), as staffs'. The weapons with an
enchantment but no `EAMT` are the ones that never need recharging: bound
weapons, the Ebony Blade, Keening, Bloodskal, trap darts. Here:

- **A blow** that lands (blocked or not) pays the enchantment's `ENIT`
  cost by the attacker's skill in its school, as a staff cast; a bow pays
  for each arrow that strikes, not each shot loosed. With too little
  left, the blow lands without the enchantment, and nothing is shown.
- **No `EAMT`**: the enchantment always lands.
- **NPCs' weapons** run dry as the player's do. Whether the game spares
  NPCs isn't published.

## Recharging

Implemented in `src/recharge.rs`, from the inventory (`inventorymenu.swf`:
the card's `charge` under 100 offers Charge on the Wait key, T;
`AttemptChargeItem`, `ShowSoulGemList`, the card's `ICT_LIST` with
`listItems` / `currentCharge` / `chargeAdded`, `ItemCardListCallback`).
Sources: UESP's Skyrim:Soul Gems (charges 250 / 500 / 1000 / 2000 /
3000, gems consumed, Azura's Star and the Black Star not), Skyrim:Leveling
(0.05 .. 0.6 base Enchanting XP, which is the charge times
`fSkillUsageRechargeMult`), CommonLibSSE's `kKeywordReusableSoulGem`.

- **The list**: one row for each gem and soul carried ("Petty Soul Gem
  (Petty)"), smallest soul first; the game's labels and order aren't
  known. With no soul at all, `sNoChargeItems` is shown and nothing opens.
- **A soul bigger than what is missing** fills the weapon; the rest is
  lost (XP still by the soul's size).
- **Reusable gems** are left empty; one filled by its record (none in the
  vanilla data) can't be used.
- The card's meter takes `usedCharge` and `charge` both as the percent
  left; what the game puts in `usedCharge` isn't known.
