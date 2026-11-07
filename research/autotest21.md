# autotest21 in Skyrim SE (run 2026-11-01)

`test-mod autotest21` (`crates/tools/src/test_mods/autotest21/`), run by the
user in Skyrim SE on a throwaway save of Testnord (level 1, outdoors), plugins
`VrmAutoTest21.esp` plus the Creation Club's Survival Mode, Curios and
`_ResourcePack`. One save:
`tests/results/Save5_823C2F21_1_546573746E6F7264_Tamriel_000052_20261101191106_1_1.ess`
(read with `vrm-tool test-results tests/results`). Every test ran to the end;
the reports below are the game's readings, with what they settle.

Verdicts:

- **Settled**: the reading answers the audit item.
- **Partial**: some of the item is answered, the rest is still open.
- **Fault**: the test didn't measure what it meant to. The item is still
  open and goes to a later batch (see "Faults" at the end).

"Ours" says whether the reading agrees with the choice recorded in
`known_ambiguities/`, where that was checked while recording. "Compare"
means the comparison is left for when the results are applied.

## The condition probes: all void

Every condition read through a probe (`Probe` in `base.psc`) came back
`(has TRUE, +0)`, both when the condition had to hold and when it couldn't
(`IsPC1stPerson` in first and third person, `GetPlayerControlsDisabled`
with controls on and off). Two faults stack:

1. The probe effect is a copy of `AlchFortifyOneHanded`, which raises the
   skill's damage modifier (actor value 135), not `OneHanded`. autotest4
   found this and autotest5 fixed it by setting the copy's primary actor
   value (`MGEF` `DATA` 0x44) to 6. autotest21's copy keeps 135, so the
   gain is always 0.
2. `HasMagicEffect` reports an ability's effect whose condition fails. This
   matches autotest19, whose `IsSwimming` detector read True everywhere. In
   **Recheck**, the probe on `GetHealthPercentage >= 0.9` stayed present for
   the whole 10 s window after each halving (the `gone after 10.0xx s`
   readings are the loop's timeout) and was "back" at the first poll. The
   readings agree on this in three tests: Recheck, the probes, and
   autotest19. So conditions can't be read through `HasMagicEffect`.

So these items stay open: the condition probes in ActorConditions,
FactionConditions, Geometry, ItemConditions, PlatformConditions,
DeathConditions, InCombatParam, the `== value` scans, and magic's Recheck
interval. The fix is to read the actor value gain only, from an effect on
`OneHanded`. The natives those tests also read are good and are listed
below.

## Natives

**QueryNatives**
- IsUnique: false for all three bases, including the one flagged unique.
  But that "plain unique" base has the commoner as a traits template (all
  our NPCs do), so the test can't separate inheritance from the flag.
  *Fault* (needs a base without a template).
- GetClass of a base whose stats template is a leveled list
  (`LCharBanditMelee1H`): `EncClassDremoraMelee` (0x17008). That is neither
  the base's own `Citizen` nor a bandit class. The placed actor's leveled
  base has `EncClassBanditMelee`. *Settled*: an unresolved leveled template
  gives an unrelated class. Ours resolves with seed 0: compare.
- GetEditorLocation of a created marker: None; of the player: None. A
  created marker in our location-less cell: GetCurrentLocation None.
  *Settled* for created refs and the player. Compare.
- Location population: Bleak Falls Barrow has Boss alive 1. Whiterun Hold
  (its ancestor) has 0, and `HasRefType(Boss)` is False on the hold, True
  on Bleak Falls. *Settled*: no recursion into child locations. Ours
  recurses: **differs**.
- HasCommonParent(Whiterun, hold) and (Bleak Falls, Whiterun): both True.
  These cases share Skyrim either way, so it's not discriminating.
  `WhiterunLocation.IsChild(WhiterunHoldLocation)` False, though the hold is
  Whiterun's `PNAM`. So `a.IsChild(b)` means "b is a child of a", as ours
  has it: **agrees**.

**MathNatives**: `asin(2)` 0, `acos(-2)` 0, `sqrt(-1)` NaN (prints
`-nan(ind)`), `tan(90)` -22877332, `tan(270)` -83858280, `pow(0, -1)` 0,
`pow(-8, 0.3333)` NaN, `Floor(-0.5)` -1, `Ceiling(-0.5)` 0. `Floor(2.9999999)`
gives 3, because the literal is already 3.0 as an f32. Float to String uses
six decimals (`0.333333`, `90.000000`). 4e9 cast to Int gives -2147483648.
*Settled*. Ours returns NaN for inverse trig out of [-1, 1] (**differs**:
the game clamps to 0). Pow, the poles and the string format: compare.

**ContainerCounts**: an empty chest gives `IsContainerEmpty` True and count
0. With 10 gold the count is 1; with 3 swords and 2 ingots added it is 3.
*Settled*: GetAllItemsCount counts distinct forms. Ours sums copies:
**differs**.

**WornKeyword**: a drawn iron sword gives `WornHasKeyword(WeapTypeSword)`
True. An NPC at carry weight 0 holding 39 gives IsOverEncumbered False.
*Settled* (hand items count; ours **agrees**).

**EncumbranceEdge**: an NPC carrying 9 with CarryWeight 9, 8.9 and 9.1:
IsOverEncumbered False each time. With WornKeyword's 39-over-0, the native
looks False for NPCs whatever they carry. *Settled* for the native
(encumbrance's "> vs =" item is not answered: the native doesn't show
NPCs over at all). Ours uses the capacity calculation: **differs**.

**Dimensions**: barrel 54 × 54 × 80 at scale 1. After `SetScale(2)` the
same 54 / 80, and turned 45° also unchanged. The player has scale 1.030,
height 128. An NPC has scale 1, height 128; after `SetScale(2)`, scale 2
and height still 128. *Settled*: bounds unscaled and rotation-free; actor
height 128 whatever the scale. Ours multiplies by scale: **differs**.

**DeadCount**: 0, then 1 when killed. Resurrect leaves 1 (IsDead False),
killed again gives 2, deleted stays 2. *Settled*: a cumulative count. Ours
counts actors currently dead: **differs**. Gold values: Restore Health
potion 17, torch 2, sweet roll 2 (compare).

**QuestStates**: Start returns True, running, not starting. Right after
Stop: not running, not stopping, stopped (the stage stays 10). Reset on a
running quest gives not running, stage 0, stage 10 not done. *Settled*,
**agrees** (synchronous stop; Reset stops and clears).

**3DLoaded**: a PlaceAtMe'd barrel is Is3DLoaded at once. Disable gives
False at once. Delete gives IsDeleted True at once, and the form id is
still readable. Placed initially disabled: not loaded. *Settled*. Compare.

**SoundIds**: 240, 241, then 242 after both stopped. *Settled*: ids aren't
reused; ours **agrees**.

**ReferenceSetters**
- Door: open state 3 → `SetOpen(true)` → 1 with `OnOpen` → `SetOpen(false)`
  → 3 with `OnClose`. *Settled*. Ours raises none: **differs**.
- Spider web, `SetDestroyed(true)`: stage stays -1, not disabled, no stage
  event, but `OnCellAttach`, `OnLoad` and `OnCellLoad` (the 3D was rebuilt).
  `SetDestroyed(false)`: nothing. *Settled* (no stage, no stage event).
  Ours takes the first destroying stage: **differs**.
- `CalculateEncounterLevel(0..4)`: 1 1 1 1 1 at player level 1. *Fault*:
  level 1 can't show the multipliers.

**Destruction** (a second web, five-point hits every 0.5 s): stage -1, then
1, 1, 2, 2, 2, 2. Events: `OnDestructionStageChanged(0, 1)` at the first hit,
then `(1, 2)`, with an `OnLoad` before the second change. So
`GetCurrentDestructionStage` is -1 undamaged while the event calls that
stage 0. `ClearDestruction`: stage -1, an `OnLoad`, no stage event. Then
1000 damage at once: stage 1 only, not disabled, one `(0, 1)` event. One
blow goes up one stage, however large. *Settled*: stage numbering, events,
ClearDestruction, overkill. The web's thresholds need its `DEST` read
alongside: compare.

**ActorState**
- Killed wearing a cuirass (added and equipped by script): still equipped.
  After Resurrect: alive, not equipped, count 0. The resurrected NPC is back
  to its base inventory. *Settled*. Ours keeps what it died with:
  **differs**.
- `EquipSpell(Flames, 1)` then an iron sword: the sword took the right hand,
  so the spell was gone before UnequipAll. The spell question is *open*.
  `UnequipAll` unequips the sword (and the cuirass).
- `UnequipItemSlot(32)` unequips the cuirass (and `EquipItem` re-added the
  cuirass the NPC no longer had); `(2)` leaves it on. *Settled*: biped slot
  numbers 30..61, as ours.
- `SetDoingFavor(true)` then `IsDoingFavor`: False. The flag doesn't read
  back on a plain NPC. *Partial*.

**Hostility**: Red (unaggressive) and Green, whose factions list each other
as enemies: IsHostileToActor False both ways. `GetFactionReaction` gives
1 / 0: Red lists Green, Green lists nobody. Green dead: False. *Fault* for
"dead": it was False alive too. `GetFactionReaction` reads the one-way
combat reaction.

**Reset** (a plugin barrel with 3 ingots in `CNTO`): the first read showed
ingots 0, with the barrel unopened and its inventory not yet made. After
the changes (moved, gold added, owner set, activation blocked), and once
the inventory existed: ingots 3. After `Reset`: ingots 3, gold 0, back at
x 600, still locked, owner still `VrmAt21Naked`, activation still
blocked. Disabled then Reset: still disabled. *Settled*: Reset restores
position and inventory, not owner, blocked activation or enable state.
Ours re-enables: **differs**. The lock: the plugin barrel reads locked
before any change, so the lock isn't tested.

**Finders**: `FindClosestActorFromRef` returns a dead NPC 141 off before a
living one 849 off. *Settled*, **agrees** (dead included). Sun position
0, 0, 0 at 12 and 18 in our interior cell: *Fault* (needs outdoors).
`GameTimeToString(0)` gives `07/16/0201 00:00` and `(1.5)` gives
`07/16/0201 12:00`, so the year is zero-padded (**agrees**). The day didn't
advance for 1.5: compare. A missing string setting gives `""` (**agrees**).

**ActorNatives**
- `GetRace` of a base whose traits template is a leveled list (its own
  `RNAM` Nord): `FoxRace` (0x109C7C). The placed actor is Nord. *Settled*:
  an unrelated race, as with GetClass.
- `GetNthLinkedRef` on A → B → C (unkeyworded): 1 B, 2 C, 3 C, 0 A, -1 A.
  Past the end it stays on the last; 0 and negatives give self. *Settled*.
- A reference linked only with a keyword: `GetLinkedRef()` None,
  `GetNthLinkedRef(1)` itself, with the keyword B. *Settled*.

**EnableAI**: with AI off, `StartCombat(player)` gives combat state 1 and
IsInCombat True. A fire spell lands (effect start and `OnHit` reach the
script), the actor doesn't move (0.012), and it doesn't detect the player.
*Settled* for detection, combat state and hit events. The graph freeze and
speech: open.

**Delete**
- A created barrel held in a variable, after Delete: IsDeleted True, not
  3D loaded, not disabled, x 0. A plugin reference: IsDeleted True, not
  disabled, not loaded. *Settled*.
- A created sword reference given to a chest by `AddItem(ref)`: count 1,
  the reference's x reads 0. Delete on it afterwards: the chest's count
  stays 1. *Settled*.

**Outfits**: an NPC wearing outfit A (iron cuirass) and given a second iron
cuirass has 2, worn. `Actor.SetOutfit(B)` leaves iron 0 (the given one went
too), leather 1 worn. The other NPC of the same base is unchanged. Back to
A: iron 1, leather 0. *Settled*: SetOutfit is per reference (ours
**agrees**). The removal took the non-outfit copy too: compare
(ours removes only outfit items?).

**FormLists**: the plugin's [iron sword, steel sword] has size 2.
AddForm(gold) appends it at index 2 (`Find` 2). AddForm of the iron sword
again, and of gold again: size stays 3. `RemoveAddedForm(steel sword)` (a
plugin form): still there, size 3. AddForm(ingot) goes to index 3. Revert
gives size 2. *Settled*, **agrees** with ours on every point.

**FactionNatives**
- Red → Green `GetReaction` 0 / 0, though Red's `XNAM` lists Green as an
  enemy. `SetReaction(1)`: GetReaction 1, still not hostile, not fighting.
  `SetEnemy`: GetReaction 0. *Settled*: Set/GetReaction is the modifier,
  not the combat reaction (ours **agrees**).
- The player at rank 2 in Green, `SetPlayerExpelled(true)`: still in the
  faction, rank -2, IsPlayerExpelled True. *Settled*: expulsion makes the
  rank read -2 while membership stays. Compare.
- Crime groups: Red isn't in its own (unlisted). Grouped lists Red and has
  it; Red doesn't have Grouped; Grouped isn't in its own. *Settled*: one
  way only, never self unless listed. Compare with ours (`factions.md`
  line 114).

**Protection**
- `ProtectBase.SetEssential(true)` on a loaded actor: IsEssential True;
  Kill puts it in bleedout, not dead. *Settled*: loaded actors follow
  the base flag.
- Ghost: DamageActorValue(10) takes 10; a fire spell takes 0. *Settled*.
- Invulnerable base: DamageActorValue takes 10 health and 10 stamina;
  ForceActorValue(1) works; a fire spell takes 0. *Settled*.

**AliasProtection**: not essential before. In an essential alias:
essential. Alias cleared, quest running: not essential. Refilled, quest
stopped: not essential. *Settled*: the flag holds while filled.

**FactionRanks** (an NPC, never in Green): rank -2, not in.
ModFactionRank(+2) while out: no change. AddToFaction: rank 0. At rank 3,
AddToFaction keeps 3. ModFactionRank(-5): rank 0, still in.
RemoveFromFaction: rank -1, out. SetFactionRank(1) while out: in at 1.
SetFactionRank(-1): rank -1, out. *Settled*, **agrees** with ours (and -2
is "never a member"). `IsGuard`: False for the Guard class alone and for
an `IsGuardFaction` member alone. *Settled*: compare.

**Infamy** (crime faction): gold 0, infamy 0. +100 non-violent and +40
violent: gold 140 (violent 40), infamy 140 (violent 40). Paid: gold 0,
infamy 140, 140 gold taken. `SetCrimeGold(50)`: infamy 190. *Settled*:
GetCrimeGold includes the violent part; paying leaves infamy (ours
**agrees**); SetCrimeGold raises infamy by what it adds.

**Relationships** (vanilla RELA bases placed):
- Gerdur-Frodnar (mother, son): HasParentRelationship True / False;
  HasFamilyRelationship True / True; HasAssociation(ParentChild)
  True / True; rank 3 / 3.
- Gerdur-Hod (spouses): HasParentRelationship True(!), family True, spouse
  both ways, rank 3. Hod-Frodnar: True / False. Alvor-Dorthe: True / False.
  Alvor-Gerdur (no RELA): family False, rank 0.
- Frodnar killed: Gerdur's family and parent relationship to him stay True.
- *Settled*: direction (the parent's side is True), dead family still
  counts. Gerdur → Hod being True is unexplained: compare against RELA
  records before applying.

**ActorKeywords**: wearing an iron cuirass, `HasKeyword(ArmorHeavy)` is
False and WornHasKeyword True; `HasKeyword(ActorTypeNPC)` True. Under Flame
Cloak, `HasMagicEffectWithKeyword(MagicCloak)` True and `HasKeyword` False.
*Settled*: HasKeyword sees neither worn items nor effects.

## Magic

**AbsorbMagicka**: a 20-point absorb, the caster down 40: target -20,
caster +20. *Settled*.

**ResistanceCap**: a 10-point fire spell from the player at an NPC took
10.975 at fire resistance 0, 1.646 at 85, 0 at 100 and 150, and 16.463 at
-50. *Settled*: NPCs aren't capped at 85; 100 stops it all (ours, a cap of
100: **agrees**). The 1.0975 factor at 0 is unexplained (the player's
skill or difficulty?): compare.

**Holds**: Calm, Fury and Fear moved neither `Aggression` nor `Confidence`
as `GetActorValue` reads them (0 and 3 throughout). *Fault*: these
archetypes don't work through the actor values, so the level cap and
resistance questions are still open. (Fear on a level 8 target was above
the spell's cap anyway.)

**Cloak**: Flame Cloak cast by script on the player, four NPCs held
100–500 off: no damage in 6 s, none in combat. Compare: no hostility
(friendly NPCs spared?) or the script cast. *Partial*.

**Ethereal**: Become Ethereal on an NPC. A 10-point fire spell took 0 and
sent no `OnHit` and no effect event. DamageActorValue health 10 and
magicka 10 both took 10. *Settled*.

**SlowTime** (word 1, cast by script on the player): a bowl falling from
rest covered 233.9 units per real second over 0.4 s before, 58.5 under
Slow Time. A fall from rest goes with t², so the world ran at 0.5×.
`Utility.Wait(1.0)` took 1.04 real s. The effect lasted 7.7 real s, during
which 54.1 game seconds passed (timescale 20). Afterwards the fall rate was
still 57.4: the slowdown outlasted `HasMagicEffect`. *Partial*: the world
share is settled, and Wait and effect durations count real time. When
time comes back: compare.

**SoulTrap** (empty petty, grand and black gems held):
- A wolf: TrapSoul True, yet empty petty 1, filled petty 0, empty grand 1,
  empty black 1. No gem counted as filled.
- An NPC: black filled (empty black 0, filled black 1).
- Another NPC, no black gem left: False; the grand gem stays empty (no
  black souls in white gems).
- Another wolf with only the grand empty: True, the grand still counted
  empty.
- *Settled*: the black soul rule, and no NPC souls in grand gems. *Open*:
  where the wolves' souls went. Likely the soul is kept on the empty gem's
  form (extra data) rather than swapped to the filled form, which
  `GetItemCount` can't tell apart.

**WornEnchantment**: a gold ring with `EnchArmorFortifyHealth01`: max
health 50 → 70, taken off 50. *Settled*.

**HostileSpell**: the player's fire spell cast by script at a lawful NPC:
effect and `OnHit`, then combat state 2 (searching), and no crime gold.
*Settled*: no assault bounty from a script cast; the target searches.

## Items, statistics, loose objects

**NpcStats** (`vrm-tool npc-stats` gives ours):
- Dervenin: OneHanded 17, Marksman 27, Block 17, Enchanting 18, Alteration
  18, Alchemy 23, level 6. All equal `esp::auto_calc`'s. *Settled*: the
  tie order (higher index first) **agrees**.
- Khayla (not auto-calc): base health 50, level 10. Her `ACBS` health
  offset is 0, so this is race plus offset, as ours (**agrees**). The audit
  and `actor-values.md` had her offset as 105, which was wrong.
- Deer: Sneak 85 (15 + the offset of 70). `MGArnielSummon`: Alteration 115.
  *Settled*: offsets are added past 100 (**agrees**).
- A PC-level-mult copy of `EncDremoraMelee06` (calc min 46): level 46,
  OneHanded 100, TwoHanded 100, HeavyArmor 84, Block 84, Destruction 15,
  health 545. *Settled*: held at 100, and the others are the stored
  `DNAM`'s (Block and HeavyArmor 84), not ours (80, 81). **Differs**;
  applied (2026-11-07): a rule fitted to the stored `DNAM`s gives these
  values (`known_ambiguities/actor-values.md`, "Skills past 100").
- Class with the stats template: see QueryNatives (the placed actor's
  leveled base has the bandit class). *Settled*.

**KillStats** (`Kill(player)`): wolf → Animals. Hare → Animals and Bunnies.
Mudcrab → Animals. Skeleton → Undead. Flame atronach → Daedra. Dwarven
spider → Automatons. Vampire → People and Undead. Commoner → People.
Creatures never moved. *Settled*.

**UseStats**: Skill Increases +1 for `IncrementSkill`; `AdvanceSkill`
(100000) +39. `EquipItem` of a sweet roll, wheat and a potion: Food Eaten,
Ingredients Eaten and Potions Used +1 each. *Settled*.

**BoundWeapon**: Bound Sword on the player, then
`RemoveItem(bound sword, chest)`: the player has 0, the chest 0; dispelled:
0 / 0. *Settled*: the bound weapon is destroyed, not moved.

**TakeUpArmor**: a leather cuirass given to a plain NPC isn't worn after 3 s
or 15 s; a teammate wears it at once. An iron cuirass given too: the plain
NPC wears neither; the teammate keeps the leather. *Settled*: only
teammates take up given armor, and not better armor over what they wear.

**Disarm**: Disarm cast by script at an NPC with a drawn iron sword: still
held, count 1, no events. *Fault*: the shout spell did nothing, so the
landing, two-hander and event items are open. (`FindClosestReference...`
then found a sword at a garbage distance.)

**LooseObjects**
- The plugin bowl placed 150 above the floor already lay at z 8.6 when the
  test began. It had fallen when the cell loaded, so "lying still until
  disturbed" is **not** what SE does for this bowl. *Settled* (compare
  against the bowl's flags).
- A PlaceAtMe'd bowl at 150: on the floor (1.9) 2 s later. `GetMass` 70.
- ApplyHavokImpulse +x on a bowl: magnitude 1 moved it 0, 10 moved it
  0.585, 50 moved it 15.7. All of it within 0.5 s, nothing more by 3 s.
  *Settled* (scale, strong damping).
- The player facing north drops a sword: it appears 140.5 ahead, 107.8 up,
  and lies 5.7 up 2 s later. *Settled*.
- A corpse pushed (`PushActorAway` 5): moved -9.9 in x. *Partial*.

**RememberBefore / RememberAfter**: the plugin bowl moved and knocked to
(501.8, 499.9, 0.4) was at the same place after the cell unloaded and
loaded. *Settled*: the resting place is remembered.

## Detection

**ViewCone**: an observer facing north; the player, sneaking, 400 off,
was detected at every bearing 0..180 (behind too). Blindness 100: still
detected at 400 and 150. Light level 31.9. At 1500 ahead not detected,
3000 ahead detected, 1500 behind not. *Fault*: our cell's floor doesn't
reach 1500–3000, and readings after `MoveTo` may be stale, so the cone,
distance and blindness items stay open. (Detected all round at 400 in a
lit room is a reading to keep.)

**AlertFlag**: `SetAlert(true)`: alerted, combat state 0, weapon drawn
(unarmed). `SetAlert(false)`: not alerted. An unalerted NPC told to
DrawWeapon sheathed 0.62 s later, before the second DrawWeapon (due at
1 s) was sent. *Settled*: sheathes in about 0.6 s. *Open*: whether a
repeated DrawWeapon restarts it.

**SoundLevels**: iSoundLevelSilent 10, Normal 50, Loud 100, VeryLoud 200.
fCombatDetectionFleeingLostRemoveTime 30, fCombatLowFleeingTargetHitPercent
0.25, fCombatDetectionLostRemoveTime 30, fCombatDialogueFleeMin/MaxElapsedTime
3 / 5, iFriendHitCombatAllowed 4, fMagicWardPowerMaxBase 1.3,
iAICombatRestoreHealthPercentage 60. *Settled* (the game's defaults).

## Story Manager and aliases

**StoryItems**: events left from earlier tests show `KILL` for script
kills (killer None for `Kill(None)`), `ASSU` for the player's script spells,
and one `SKIL Alchemy` for UseStats' increases. Then:
- `AddItem` to the player: no `AIPL`.
- `DropObject`: no `REMP`.
- Picked up with `Activate(player)`: `AIPL`, owner None, container None,
  type 4.
- `RemoveItem` into a chest, or to an NPC: no `REMP`.
- An NPC-owned ingot picked up, and an ingot taken from the chest by
  `RemoveItem`: no `AIPL`. The AIPL quest might still have been stopping
  from the first event, so these two are *Fault*.
- *Settled*: script `AddItem` / `RemoveItem` / `DropObject` send nothing;
  pickup by activation sends `AIPL` with no owner for an unowned item.

**StoryCombat**:
- The player's first spell at an NPC: `ASSU`. The second: none (first
  blow only).
- An NPC's spell at another: none.
- `Kill(player)`: `KILL`, crime 0.
- A lawful NPC killed by Alpha: `KILL` crime 1, but it arrived more than
  1.5 s late (logged in the next step).
- `IncrementSkill(Sneak)`: no `SKIL` within 1.5 s.
- `SetRelationshipRank(Alpha, Beta, 2)`: `CHRR` 0 → 2.
- The `ASSU` crime member read as garbage (1076888576, -2019674368).
  Check the event's parameter types.
- *Settled*: ASSU on the first blow only, NPC kills' crime status, CHRR.
  *Open*: the skill increase event (one SKIL appeared for UseStats'
  calls, none here).

**CreatedRefs**: every created alias reference read None. **AliasMatch**'s
leveled bandit created at levels 0 and 3: None. *Fault*: our
`ALCO`/`ALCA`/`ALCL` aliases created nothing. Compare with a vanilla quest's
create alias.

**AliasMatch**: three NPCs match `GetIsID`: a plugin one in the cell, one
in the warehouse, and one placed by script near the player. All three
quests (no flags; in loaded area; in loaded area and closest) took the
script-placed one. *Partial*: without Closest the fill still took the
nearest (or newest), so the two can't be told apart here.

## Hands and odds

**HandTypes**: Flames in the right hand only: `GetEquippedItemType` left 0,
right 9; graph `iLeftHandType` 0, `iRightHandType` 9, the same drawn.
*Settled*: the empty hand beside a spell is 0. Ours gives it the spell's 9
(`casting.md`): **differs**. A dagger (speed 1.3) drawn: `weaponSpeedMult` 1.3,
`leftWeaponSpeedMult` 1. `WeaponSpeedMult` 2: 2.6 (the product).
*Settled*.

**StolenDrop**: an ingot owned by the crime faction taken by
`Activate(player)` with nobody about: bounty 0. Dropped and picked up, then
a bounty paid with "remove stolen goods": the ingot stays. *Partial*: the
pay didn't take it, but there's no control with an undropped stolen item.

**InvisiblePotion**: Invisibility on the player (value 1); a potion drunk
by `EquipItem` gives 0. *Settled*: drinking breaks it. Food and the locked
chest came after it was already broken: *Fault*.

## Combat (in `WarehouseFurniture`)

**Assistance**: Red attacks a Teal NPC. The victim fights (1); Teal allies
500 and 1200 off never join; a factionless helper at RELA rank 3 joins at
once and targets the attacker. *Settled*: the relationship rank brings
help; same-faction members with "helps allies" didn't come within 10 s.
Compare.

**FriendlyHits**: one Teal member's fire spell at another turned it on the
first hit (health 39). *Settled*: spells between friends aren't tolerated
like blows (`iFriendHitCombatAllowed` 4 doesn't cover them).

**Flee**: the Average NPCs' combat states fell to 0 within 1–4 s, likely
killed by the level 50 brute (no health read). The Cowardly NPC ran to
about 900 units in 3 s and stayed there in combat state 1 for 12 s.
*Partial*: the coward flees at once and stays in combat. The Average
cases are *Fault*.

**PlayerEnemy**: SetPlayerEnemy on Red: the aggressive member stays at 0
(not targeting the player) for 4 s at 400 units. Nothing else happened
either. *Fault?* Compare: we may need the detection and aggression set up
differently (the Red fighter did fight when told to).

**Teammate**: an NPC attacks the player: the player's GetCombatState is 1,
and the teammate joins (1 each second, targeting the foe). The teammate
kills it: Murders 0, `GetKiller` the player. *Settled*: teammates join;
their kills are credited to the player, not as murder. With DeathConditions
(`GetKiller` is the player for a teammate's kill): settled.

**Casters**: a mage with Firebolt and `csHumanMagic` on a held target 900
off walked in to 70 units, never readied a spell (both hands None), and
sent only the combat event. **NpcCostReduction**: the same mage spent 0
magicka at DestructionMod 0, 50 and -50. **Healer**: no heal at 70%, 45%
or 20%. **NpcStaff**: charge 500 throughout, the target unhurt. **Wards**:
WardPower 0, while the vanilla warlock spent its magicka casting. *Fault*:
our caster NPCs (level 10, 50 magicka, skills 15) never cast; the vanilla
warlock did.

**InvisibleAttacker**: an NPC under Invisibility set on a target 150 off
was visible after 4.5 s of fighting. *Partial*: an attack breaks it, but
the timing against swing vs hit is not read.

**EnchantedBlows**: an NPC with an iron sword of burning: charge 500 →
496.16 after 10 s. The target saw two enchantment effects among four hits.
*Settled*: NPCs use charge, about 1.9 per enchanted blow. Compare with
the enchantment's cost.

**AttackOnSight**: a guard of the crime faction (Guard class, in the
faction) 500 off: combat state 0 at bounty 0, 999, 1000 and violent 1000.
*Fault?* Like PlayerEnemy, nothing turned on the player in the warehouse.

**FightAfterAlly**: Red and Green fighters, told to fight, were at 0 / 0
after 3 s (one likely died: no health set, unlike StopCombat). *Fault*.

**StopCombat**: StopCombat on a fighting Red: state 0 at once, fighting
again 0.39 s later. StartCombat on another: the target switches, no
events. *Settled*.

## Outdoors

**BackOutside**: the test cell (interior) had sky mode 1 and reported the
outside weather (0x12F89) as current; outdoors the same weather, sky mode
3. *Settled*: indoors keeps the outdoor weather state.

**Weather** (timescale 20): forcing one weather and setting another active
gave these full transitions:

| delta | accelerate | half at | full at |
| ---: | --- | ---: | ---: |
| 82 | no | 7.6 s | 15.2 s (5.28 game min) |
| 61 | no | 6.1 s | 11.9 s |
| 125 | no | 11.3 s | 22.2 s |
| 61 | yes | 6.1 s | 11.7 s |

The transition is linear in time and about 0.18 real s per delta unit.
Accelerate made no difference. `FindWeather(0)` returned the current
pleasant weather (0x12F89) six times; classes 1–3 found none. *Settled*:
the speed law, abAccelerate, FindWeather. Compare.

**FastTravel**: `EnableFastTravel(false)`: disabled. After a script
`MoveTo` (with the cell change): enabled again. *Settled*. Game.FastTravel
40261 units took 2.713 game hours; 140452 units took 9.546. That's about
14,800 units per game hour, linear. *Settled*.

**MapMarkers**: a hidden marker stays hidden at 8000 and 4000, and becomes
visible and travellable at 2000, with Locations Discovered +1. Another,
`AddToMap(true)`: visible and travellable, no discovery, and none when the
player then came within 200. *Settled*: reveal between 2000 and 4000; an
AddToMap'd marker is never counted as discovered.

## Faults, for a follow-up batch (`test-mod autotest22`)

1. **Probes**: the effect's actor value set to `OneHanded` (6), and the
   gain read alone. That reruns every condition item, Recheck, and the
   `== value` scans.
2. **Created aliases**: find why `ALCO` created nothing (compare a vanilla
   create alias's fields and flags).
3. **Casters**: use vanilla casters (the warlock cast) or give ours their
   magicka and skills, for Casters, NpcCostReduction, Healer, Wards and
   NpcStaff.
4. **Fights that ended early**: health set high for FightAfterAlly and the
   Average fleers. Check why SetPlayerEnemy, SetAttackActorOnSight and
   the 1000-bounty guard did nothing.
5. **IsUnique** without a traits template; **Holds** read by combat
   state and fleeing, not actor values; **Disarm** with an NPC's shout
   or the effect's conditions checked; **ViewCone** inside the floor,
   with a re-read; **sun position** outdoors; **CalculateEncounterLevel**
   at a higher level; **InvisiblePotion**'s food and activation each
   alone; **StoryItems**' later AIPLs with time for the quest to stop.
