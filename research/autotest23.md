# autotest23 in Skyrim SE (run 2026-11-10)

`test-mod autotest23` (`crates/tools/src/test_mods/autotest23.rs`): what
autotest21 and autotest22 left open, and audit items sharing their setups.
The user ran it on a throwaway save (a level 6 Nord, game day 1, started
near Riverwood). The first attempt crashed to desktop with no save or log.
The rebuilt plugin saves after each test. Test 9 (TimeDead) was the crash,
and the run went on with `set VrmAt23SkipTest to 9`. Our test cell's
location was Whiterun Hold this time.

*Settled* means the reading answers the question; *Partial* answers part
of it; *Fault* means the test didn't show what it was meant to. "Ours" says
how the engine compares where that was checked while recording; "compare"
leaves it for when the results are applied.

## Crash and faults

- **TimeDead** crashed the game every time. autotest24 crashed on its first
  part alone (the kill and `IsKillerObject` run on the dead victim; see
  `research/autotest24.md`), so the likely cause is that probe, not the
  timescale of 1000 that came after. `GetTimeDead`'s unit stays open.
- **ViewCone**: most of the spots were off our cell's floor (it spans ±1536;
  the observer stood at (-1300, -1300)). The player fell, and the game put
  them back in the middle of the room, about 1840 units in front of the
  observer. Only 90° stood on the floor: seen at 2 s and 5 s. The 99° to
  135° readings are *Fault*. So was autotest22's 135° (y -1600, off the
  floor). Its 180° spot was on the floor (not seen). The edge stays between
  90° and 180°. Next time, put the observer in the middle.
- **AttackOnSightCrime**: our guard isn't a guard by `IsGuard` (see IsGuard:
  that needs `IsGuardFaction`), so its never attacking at 999 or 1000 says
  nothing about Attack on Sight.
- **Holds**: Calm and Fury sent no effect event and changed nothing. The
  script casts from the player at the warehouse entry, about 550 units off,
  didn't land. Fear did (below).
- **RelationshipHelp**, the last case (helps friends and allies, rank 4):
  the attacker never fought (the victim's state 0), so nothing to read.

## Natives

**GameSettings** (as the game has them; 0 may mean the setting doesn't
exist): fFleeIsSafeTimer 30, fCombatFleeDistanceInterior 2048,
fCombatFleeDistanceExterior 4096, fCombatDialogueFleeMinElapsedTime 3,
fCombatDialogueFleeMaxElapsedTime 5, fCombatDialogueFleeDistanceMult 1,
fCombatLowFleeingTargetHitPercent 0.25,
fCombatDetectionFleeingLostRemoveTime 30, fCombatDetectionLostRemoveTime
30, fCombatConfidenceModifierMin -0.25 / Max 1, fCombatStrengthUpdateTime
1, fCombatThreatRatioUpdateTime 5, fCombatGroupCombatStrengthUpdateTime 2,
fCombatRestoreHealthPercentMin 0 / Max 0.5,
fCombatRestoreHealthRestrictTime 15, fCombatRestoreStopCastThreshold 0.25,
fCombatItemRestoreTimer 3, iAICombatRestoreHealthPercentage 60,
iAICombatRestoreMagickaPercentage 30, iFriendHitCombatAllowed 4. Zero:
fFleeDistance, fFleeDoneDistance, fCombatFleeDistance,
fCombatFleeWaitTime, fCombatFleeBoostConfidence and its radius,
fCombatHealthRegenRateMult. *Settled* as values. Ours: compare.

**GameTimeToString** (GameDaysPassed 0.552, so the save was still in its
first day): 0 and 1 both `07/16/0201 00:00`; 1.5 `07/16/0201 12:00`; 31
`08/15/0201 00:00`; 365 `07/15/0202 00:00`; now (0.552) `07/16/0201 13:14`;
now + 1 (1.552) the same `07/16/0201 13:14`. The hour is the fraction of
the argument. The date is the start date for any value below 2, and after
that moves on by the whole days less one (31 → 30 days on, 365 → 364).
*Settled* as readings; a save past its first days still wasn't tried.
Ours: compare.

**ParentNone**: `HasParentRelationship(None)` False for Gerdur, Frodnar,
Hod and a plain NPC. `HasFamilyRelationship(None)` True for the three with
family, False for the plain NPC. *Settled*: with no actor, family means "has
any family relationship", and parent is never True. Ours: compare.

**IsGuard**:
- A Whiterun guard (vanilla base) and a plain copy of it: True.
- The copy with its `CRIF` removed: True. But it still reports Whiterun's
  crime faction: its template supplies it, so the removal didn't take
  (*Fault* for this case).
- Guard class (flagged Guard) with no crime faction, or with Whiterun's:
  False.
- `IsGuardFaction` with no crime faction: False. With Whiterun's: True.
- Our guard-class NPCs, members of our own crime factions (Arrest, with
  and without Attack on Sight): False.
- *Settled*: `IsGuard` is `IsGuardFaction` membership together with a crime
  faction. The class's Guard flag doesn't count. Ours (faction or class)
  **differs**.

**DeleteHeld**: a created barrel deleted while a script holds it: IsDeleted
True, not disabled, 3D unloaded, x reads 0. `MoveTo` sets x to 100 but it
stays unloaded; `Enable` doesn't bring it back; `AddItem` still works
(5 gold counted). A plugin barrel: before Delete enabled and loaded; after,
IsDeleted True, not disabled, unloaded, x still 900; `Enable` doesn't bring
it back either. *Settled*: Delete removes the reference from the world at
once (disabled stays False); calls on a held deleted reference still change
its data. Ours: compare.

**SpellsLearned**: 0, then 1 after `AddSpell` of a vanilla spell, 2 after
a plugin spell, and still 2 when both were removed. *Settled*: scripted
`AddSpell` counts, removal doesn't take it back. Ours: compare.

**CellLocation** (our cell, `XLCN` Whiterun Hold):
- The player: current location Whiterun Hold, editor location None.
- A created marker, `CellCenter` (a plugin marker) and Alpha (a plugin
  actor): current and editor location Whiterun Hold.
- A created sword on the floor: Whiterun Hold both. After
  `Player.AddItem(ref)`: both still Whiterun Hold.
- `IsLoaded`: Whiterun Hold True, Whiterun False, Bleak Falls False.
- `GetCrimeGold` without a faction on the player, with bounties of 7 in
  Whiterun and 3 in the Rift: `== 0` yes, 3, 7 and 10 no.
- *Settled*: the player's editor location is None; a created reference's is
  where it was made (see Outdoors). A reference taken into an inventory keeps
  the location it had. Without a faction, `GetCrimeGold` reads 0 for the
  player even inside a hold with a bounty there (outdoors in Whiterun Hold
  too, below). Ours: compare.

## Conditions

**XPNext** (level 6, 32 skill increases): `GetXPForNextLevel` in [-0.02,
0.10), so 0. The same after `IncrementSkill(Alteration)` (15 → 16, 33
increases) and after `AdvanceSkill(Alteration, 5)`. With autotest22 (level
1: `>= 0` yes, `>= 25` no), *Settled*: the condition reads 0 for the player.
Ours (the level's whole cost) **differs**.

**RelAngle**, `GetRelativeAngle(Beta, Z)` on Alpha, bisected to 1.6°:

| Alpha's angle | Beta's place | Beta's angle | reading |
| ---: | --- | ---: | ---: |
| 0 | east | 90 | 270 |
| 0 | east | 180 | 180 |
| 0 | north | 90 | 270 |
| 0 | north | 270 | 90 |
| 30 | east | 90 | 300 |

*Settled*: the subject's angle less the reference's, wrapped into [0, 360);
where Beta stands doesn't matter. Ours (the reference's less the subject's)
**differs** in sign. The kill moves' `>= 90` and `<= 270` test is symmetric,
so they aren't affected.

**PlayerDisease**: `GetDisease` no before, with Ataxia added by `AddSpell`
(HasSpell True) and after removal. *Settled* with autotest22's NPC: a
disease added by `AddSpell` doesn't count, for the player either. What does
count (a disease caught by contact?) is open. Ours (any disease effect)
**differs**.

**DrawnHands**, an NPC fighting (state 1, weapon drawn), the left hand's
type 11 (torch) or 10 (shield): `IsTorchOut` yes, `IsActorUsingATorch` no,
`IsShieldOut` yes. *Settled*: a torch drawn in combat is out but still not
"used" (as sheathed in autotest22). Ours: compare.

## Story Manager and magic

**EssentialKill**: an essential NPC killed by `Kill(None)` and by
`Kill(player)`: bleedout, not dead, no `KILL`. A plain NPC `Kill(None)`:
dead, `KILL` with killer None. *Settled*: no `KILL` for an essential actor.
So autotest22's `KILL` came from something else. Ours (none) **agrees**.
Also heard, from earlier tests: one `SKIL Alteration` (`IncrementSkill`),
and two `ASSU` events between the DrawnHands fighters and their held
targets, whose crime member read as large garbage values
(-2024643328, -2048921600).

**EffectsOnDead**: Invisibility (1) and a flame cloak on an NPC.
Killed: Invisibility 0 a second later; `HasMagicEffectWithKeyword(MagicCloak)`
still True at 1 s and 4 s. *Partial*: Invisibility's value goes at death.
The cloak may stay, or may be reported as `HasMagicEffect` reports failing
conditions (autotest21). Ours: compare.

**FailingCondition**: our 10-point fire spell with a condition that fails
(`IsWin32 == 0`): no damage (50 → 50), yet the effect event, `OnHit` and
combat came. The same spell without the condition: 50 → 39.025 (the taper).
*Settled*: a fire-and-forget effect whose condition fails does nothing, but
the spell still hits. Ours (it doesn't land) agrees on the damage; the
events: compare.

**SlowTime**: the world's share (from a bowl's fall from rest over 0.4 s,
which goes with t², so the share is the square root of the rate's ratio)
and the game clock's (over 0.5 s, against its 1.04 before):

| word | magnitude | world | clock | lasted (real s) |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 0.3 | 0.315 | 0.298 | about 8 |
| 2 | 0.2 | 0.210 | 0.199 | about 12 |
| 3 | 0.1 | 0.106 | 0.103 | about 16 |

The first sample (from 0.05 s after the cast) read about a third of normal
speed for all three words, and the one straddling the end read in between.
The next sample was back to normal each time. *Settled*: the world and the
game clock run at the magnitude, for the effect's duration in real time.
autotest21's 0.5 and 0.35, and its "still slow after", were samples that
straddled the start and the end. *Partial*: the change seems to take part
of a second at either end, but this sampling can't give its shape. Ours
(the magnitude, sudden, ending with the effect) **agrees** apart from that.

**Disarm** (NPCs fighting a held target, each carrying a dagger too):
- The shout's own spell (`VoiceDisarm1`), cast by script from 150 units:
  nothing, no event. *Settled*: a script cast of the shout spell does
  nothing, close or far.
- Our target-actor copy at 12, level 1 NPC: effect, `OnHit`, unequip. The
  sword was on the floor 159 units off, at hand height above the feet
  (dz 67.5). 4 s later the NPC held nothing, its "weapon" still drawn, still
  fighting, and it hadn't picked the sword up or drawn the dagger.
- The same at 12 on a level 10 NPC: the effect event, but it kept its sword.
- On a level 20 NPC with a greatsword, at 12 and at 30: kept it (no
  logger, so no events).
- *Partial*: Disarm hits (`OnHit`) and the weapon drops a little way off;
  the disarmed NPC fights on unarmed for at least 4 s. Magnitude as a plain
  level cap doesn't fit (level 10 kept its sword at 12). What decides it is
  open. Ours: compare.

**CreateLevel** (player level 6, five picks each of `LCharBanditMelee1H`):
create level easy: four level 1 and one level 5; very hard: three level 5
and two level 1. *Partial*: leans to harder picks at very hard; too few to
say more.

## Fights (in `WarehouseFurniture`, location None)

**EnchantedBlows** (a sword of burning, `EnchWeaponFireDamage01`: cost 4,
magnitude 5): the charge fell 1.919 for each enchantment effect the target
received, at Enchanting 15 and at 100 alike (9 effects: 500 → 482.728).
*Settled*: 1.919 per enchanted blow, whatever the Enchanting skill. Ours
(about 3.6): **differs**.

**NpcStaffs** (the spell-less warlock):
- The staff always went to the left hand (type 8). The right held the iron
  dagger it carries (type 2). With nothing else carried, the right hand's
  type read 0. *Settled*: the empty hand beside a staff reads 0.
- Sparks, Destruction 39: about 5.8 charge a second while casting (500 →
  477.3 in 4 s, then it went to melee). DestructionMod 50: the same rate
  (500 → 462.8 over 10 s). Destruction 100: about half (500 → 485.7 over
  10 s, three casts). *Settled*: the Destruction skill lowers staff charge
  use, `DestructionMod` doesn't.
- Firebolt (1000 charge): 11.557 a cast (five casts: 57.78), drawn down in
  steps through each cast.
- Ours: compare (`known_ambiguities/staffs.md`).

**Holds**: Fear on a level 8 NPC fighting a held target, magic
resistance 0 and 90: the effect event, but confidence stayed 3,
`IsFleeing` no and it kept fighting. Calm and Fury: *Fault* (above).
*Partial*: Fear landed on a level 8 NPC without making it flee or lowering
its confidence within 3.5 s. Why is open: the level cap, a delay, or the
script cast.

**Healer** (the Vigilant with 300 magicka): at 70% it cast once (magicka
300 → 268, four effect events) without its health rising. At 45% and 20%
nothing in 8 s each. *Partial*: no heal below 60% even with magicka. What
it cast at 70% is open. Ours (heals below 60%) **differs**.

**Wards** (the Vigilant with 300 magicka, a fire warlock 700 off):
WardPower 0 throughout; one cast near the end. *Settled* for this NPC: it
doesn't ward. Ours: compare.

**Flee** (3000 health, set to 18% against a level 20 fighter, 20 s): the
Average NPC kept fighting, `IsFleeing` no. The Coward stayed in combat
state 1 with `IsFleeing` yes. *Settled*: Average confidence doesn't flee at
18%; a fleeing actor's combat state stays 1. Ours (by threat ratio, the
Average one likely flees) **differs**.

**AttackOnSightAggression**: aggression 1: not hostile before or 6 s after
`Player.SetAttackActorOnSight(true)`. Aggression 2 and 3 (no faction):
hostile, fighting the player before it was called. *Settled*: very
aggressive and frenzied NPCs attack a neutral player anyway, and
aggression 1 ignores the flag (as autotest22). Ours: aggression 2's attack on
a neutral: compare; the flag at aggression 1 **differs** (as recorded in
autotest22).

**Ethereal**: a fighter on an ethereal NPC: health 50 throughout, no `OnHit`,
only the combat event. A fighter set on the ethereal player stayed in
combat 130 to 255 units off; the player's health didn't move. *Settled*:
blows on an ethereal actor do nothing and send no `OnHit`; NPCs keep
fighting an ethereal player. Ours: compare.

**RelationshipHelp** (a helper of no faction, 650 units from a victim
attacked by a Red fighter, 8 s):

| helper | rank 1 | rank 2 | rank 3 | rank 4 |
| --- | --- | --- | --- | --- |
| helps allies | no | joined | joined | joined |
| helps friends and allies | joined | joined | | (fault) |

*Settled*: rank 2 (Confidant) and up count as allies, rank 1 (Friend) as a
friend. Ours (allies from rank 3) **differs** at rank 2.

**FollowerHelp**: a `CurrentFollowerFaction` member (rank 0, not a teammate)
joined against a fighter attacking the player, helping friends and allies
or allies only. *Settled*: the faction alone brings help. Ours **agrees**.

## Outdoors (from the start, near Riverwood)

- The player: current location `RiverwoodLocation`, editor None; in
  Whiterun Hold.
- The created start marker and a sword created at the player: current
  Riverwood, editor `BleakwindBasinLocation`. The sword kept both after
  `AddItem` to the player.
- Whiterun's and Solitude's map markers (far, unloaded): current and editor
  their cities.
- `IsLoaded`: Riverwood True, Whiterun Hold True, Whiterun False, Bleak
  Falls False.
- `GetCrimeGold` without a faction, bounties 7 and 3: `== 0` yes, 3 and 7
  no.
- *Settled* as readings: a created reference's editor location isn't the
  current location here. Bleakwind Basin is likely the exterior cell's own
  location. Ours: compare.

## Faults and open items, for a follow-up batch

1. `GetTimeDead`'s unit, without the timescale of 1000.
2. The view cone's edge between 90° and 180°, with the observer in the
   middle of the floor.
3. Attack on Sight with a guard that is `IsGuard` (in `IsGuardFaction`).
4. Calm and Fury from close by; Fear's flee (a longer read, a lower
   level).
5. Disarm: what decides it beyond the level.
6. `GetDisease`: what makes it true.
7. What the Vigilant cast at 70%, and why it doesn't heal lower.

## Compared with ours (2026-11-10)

- **Changed to agree**: `IsGuard` (`IsGuardFaction` with a crime faction);
  `GetRelativeAngle`'s sign; `GetXPForNextLevel` 0; relationship rank 2 as
  an ally for helpers; NPCs' staffs without the school reduction;
  `OnHit` and `OnMagicEffectApply` for a failing fire-and-forget effect;
  `GameTimeToString` from the start date; the player's editor location
  None; `GetDisease` 0; `IsActorUsingATorch` 0 for NPCs.
- **Agree already**: Slow Time (apart from the easing); `KILL` not sent
  for an essential actor; `HasParentRelationship(None)` and
  `HasFamilyRelationship(None)`; Spells Learned; `GetCrimeGold` without a
  faction (0 for the player); the game settings our code falls back to;
  ethereal targets; a follower-faction member helping; the view cone.
- **Differ, left open** (each recorded in its `known_ambiguities/` file):
  NPCs' staff and weapon charge per use (`staffs.md`), NPC staffs in the
  left hand (`staffs.md`), the Vigilant's healing (`npc-healing.md`) and
  warding (`wards.md`), the Average NPC fleeing (`combat-fleeing.md`), an
  aggression 1 NPC attacking the neutral player (`factions.md`), `Delete`
  on plugin and held references (`created-refs.md`), a created
  reference's editor location outdoors and items taken into an inventory
  (`query-natives.md`), and a disarmed NPC taking up what it carries
  (`disarm.md`).
