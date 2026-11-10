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
  0.59 at 100, which gives UESP's 1.7 times the uses), times the caster's
  reduction of the school's costs: `max(0, 1 - <School>Mod / 100)`
  (`AlterationMod` .. `RestorationMod`, actor values 108 .. 112, which worn
  `Fortify <School>` enchantments raise; `magic-menu.md`). The same applies
  to enchanted weapons' blows, crafted copies' own use cost included.
  UESP's [Skyrim:Enchanting](https://en.uesp.net/w/index.php?oldid=3538992)
  says items reducing a school's magicka cost "also affect charge levels on
  weapons associated with that school of magic" (four 25% pieces give
  Chillrend unlimited uses), and
  [Skyrim:Fortify Destruction](https://en.uesp.net/w/index.php?oldid=3429893)
  that it lowers "the use cost of Destruction-based enchantments" and
  "also works with staves". Neither says how: the reduction is applied as
  for spells (UESP's Skyrim:Magic Overview, equipment multiplier 1 less the
  sum of the reductions), from the actor value rather than through
  `PerkSkillBoosts`' entries. The school is the costliest effect's, as for
  the skill factor.
- **Not the whole `ModSpellCost` entry point**: spell cost perks (the
  school's Novice .. Master perks and the rest below) aren't applied to
  enchantments; whether they are isn't published. Spells keep the school
  reduction through `PerkSkillBoosts`, so it isn't counted twice. An SE
  test could settle it (a plugin perk with an unconditioned `ModSpellCost`
  entry, `RightItemCharge` read before and after one staff cast).
- **NPCs** get no reduction. Skyrim SE (autotest23,
  `research/autotest23.md`, NpcStaffs): a warlock's staff of sparks spent
  charge at the same rate with `DestructionMod` 50 as with 0 (about 5.8 a
  second). For spells the reduction comes from `PerkSkillBoosts`, which
  only the player owns, so this fits. Enchanted weapons' blows are taken to
  go the same way (not read).
- **Spell cost perks on enchantments**: `vrm-tool cost-entries <Data>`
  classifies the 43 vanilla `ModSpellCost` entries by whether their spell
  conditions could match an enchantment. 31 can't: the school perks'
  `SpellHasCastingPerk` (an `ENCH` has no `SPIT` casting perk) and
  `DLC1UnearthlyWill`'s `GetIsID` (vampire lord spells only). 11 could:
  `PerkSkillBoosts`' five `EPMagic_SpellHasSkill` entries (the school
  reduction, `AlterationMod`..`RestorationMod`),
  `DLC2BlackBookSeekerPerks` and `DLC2StandingStoneTreePerk` (any school),
  `StormMastery` / `FrostMastery` / `FireMastery` (`MagicDamage*`
  keywords, which staff and weapon effects carry) and
  `DLC2dunKolbjornHelmPerk` (summon and rune keywords). One is
  unconditioned (`DLC2BlackBookNoMagickaPerk`). Nothing public says
  whether the game runs the entry point for enchantments, so it isn't run.
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
- **Skyrim SE (autotest22, `research/autotest22.md`, NpcStaff)**: a warlock
  copy without spells given a staff of sparks by `EquipItem` held it in
  its left hand (a dagger in the right, `IsEquipped` False) and fought with
  it, its charge 500 to 457.5 over 10.5 s (about 8.7 each 1.5 s while
  casting, two Cast events). autotest23 (`research/autotest23.md`,
  NpcStaffs) again: the staff in the left hand, the dagger in the right
  (type 2), and with nothing else carried an empty right hand (type 0).
  Ours fights with a staff only from the right hand: differs, open (the
  cast's hand would need carrying to the graph and the effects).
- **NPCs' charge per use** (autotest23): at Destruction 39, `StaffEnchSparks`
  (`ENIT` 19) about 5.8 a second and `StaffEnchFirebolt` (41) 11.557 a
  cast, about 0.31 and 0.28 of the cost; at Destruction 100, sparks about
  3 a second (rough: three casts with gaps). Ours (the player's skill
  factor, 0.78 at 39) spends about 2.7 times as much. No formula from the
  game settings gives these (`fMagicCasterSkillCostMult` 0.5 is the only
  NPC-looking one in Skyrim.esm). Kept as ours until more readings fix one.
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
- **NPCs' weapons** run dry as the player's do. Skyrim SE (autotest21,
  `research/autotest21.md`, EnchantedBlows): an NPC with an iron sword of
  burning (`EnchWeaponFireDamage01`: `ENIT` cost 4, auto-calculated; one
  5-point `EnchFireDamageFFContact`, base cost 0.9) went from 500 to 496.16
  charge in 10 s of fighting, the target seeing two enchantment effects
  among four hits: NPCs do pay, about 1.92 an enchanted blow (or 0.96 a
  hit). autotest23 (`research/autotest23.md`, EnchantedBlows) read the
  charge after each hit: 1.919 for each enchantment effect the target
  received, at Enchanting 15 and 100 alike (the attacker's Destruction,
  not printed, is taken as its record's 15). So Enchanting doesn't count;
  1.919 is 0.48 of the cost. Ours pays the `ENIT` 4 by the attacker's
  Destruction (about 3.5 at skill 15): differs, open with the staffs'
  charge above.

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
