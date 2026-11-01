# autotest22 in Skyrim SE (run 2026-11-01)

`test-mod autotest22` (`crates/tools/src/test_mods/autotest22.rs`) reruns
autotest21's faults (`research/autotest21.md`, "Faults"). The user ran it in
Skyrim SE on the same throwaway save of Testnord (level 1, outdoors), with
`VrmAutoTest22.esp` plus the Creation Club's Survival Mode, Curios and
`_ResourcePack`. One save:
`tests/results/Save2_823C2F21_1_546573746E6F7264_Tamriel_000045_20261101213924_1_1.ess`
(read with `vrm-tool test-results tests/results`; `vrm-tool save-info` gives
49/100 XP and 0.852 game days passed at the end). Every test ran to the end.

Verdicts as in autotest21's note: **Settled**, **Partial**, **Fault**.
"Ours" compares the reading with the engine as it stood at a9450df.

## The probes work now

The probe reads only the +3 `OneHanded` its ability gives while the
condition holds. The controls behave: `IsWin32` yes, `IsPC1stPerson` yes in
first person and no in third, `IsGoreDisabled` no. So every probe reading
below is a condition's real answer. **Recheck**: the `GetHealthPercentage >=
0.9` probe was gone 0.48 to 0.82 s after each halving and back 0.928 s after
each restore, all three times. So abilities' conditions are rechecked about
once a second. *Settled*; ours (`SYNC_INTERVAL` 1 s) **agrees**.

## Status conditions

**PlatformConditions**
- `GetXPForNextLevel` is not the whole cost (`fXPLevelUpBase` +
  level × `fXPLevelUpMult` = 100): `== 100` failed, and the scan gave `>= 0`
  yes, `>= 25` no. At that point the player had about 33 XP (two Illusion
  increases, to 16 and 17), so the value isn't the XP missing (about 67) or
  held (33) either. *Partial*: ours (the whole cost) **differs**, and what the
  value is stays open (a finer scan, before and after a skill increase).
- `GetPlayerControlsDisabled` with movement disabled: `(0,0)` yes, `(1,0)`
  no, `(0,1)` yes; with everything enabled: `(0,0)` yes, `(1,0)` yes. The one
  rule fitting all five readings: true unless a selected control is disabled,
  and true with none selected. Despite the name, it reads as "the selected
  controls are enabled". *Settled* for movement (fighting disabled wasn't
  tried). Ours (all selected disabled; none selected: any disabled)
  **differs**.
- `GetCrimeGold` with no faction, on the player: `== 0` with no bounty. With
  70 in our crime faction, `== 70` failed, on the player and on an NPC. The
  player was in our location-less cell. *Settled*: without a faction it
  doesn't add up the player's bounties. Ours sums them: **differs**. What it
  reads (the current hold's faction?) is open.

**ItemConditions**
- Three iron swords, one equipped: `GetKeywordItemCount(WeapTypeSword) == 3`
  (copies count). `GetWeaponAnimType` is 1 for the sword and 5 for the
  greatsword. `HasTwoHandedWeaponEquipped` no, then yes. *Settled*, ours
  **agrees**.
- A torch equipped, weapons sheathed: `IsTorchOut` yes, `IsActorUsingATorch`
  no. *Settled*: a torch held sheathed is out, but not "used". Ours gives
  both the torch test: `IsActorUsingATorch` **differs**.
- The "drawn" readings (torch and shield after `DrawWeapon`) are *Fault*.
  autotest21's AlertFlag found that an unalerted NPC sheathes about 0.6 s
  after `DrawWeapon`, and the probes came 2 s later.
- Iron cuirass (125) and iron shield (60) worn: `GetClothingValue >= 0` yes,
  `>= 5` no. *Settled*: armor doesn't count. Ours (clothing-type armor
  only) **agrees**.
- `IsShieldOut` sheathed: no (ours **agrees**).

**ActorConditions**
- `GetIsCreature`: an NPC no, a wolf no. `WolfRace` carries
  `ActorTypeCreature` (0x13795) in its `KWDA`. *Settled*: not the keyword.
  It is probably always false in Skyrim (Oblivion's creature records are
  gone). Ours (the keyword) **differs**.
- `SetScale(2)`: `GetScale >=` held through 1.9 and failed at the scan's
  2.0000x (float steps). *Settled*, ours **agrees**.
- An NPC given the Ataxia disease by `AddSpell`: `GetDisease` no.
  *Settled* for NPCs. Ours (any disease effect) **differs**. All the vanilla
  uses ask about the player, which this doesn't test.
- `SetUnconscious(true)`: `GetUnconscious` yes. An essential NPC in
  bleedout: no. *Settled*: the flag `SetUnconscious` sets, not bleedout.
  Ours reads bleedout and has no `SetUnconscious`: **differs**.
- Stamina at 0.401: `GetStaminaPercentage >= 50` no, `>= 0.3` yes. *Settled*:
  a fraction, ours **agrees**. (`>= 0.5` yes came 5 s later, after stamina
  had regenerated.)
- An NPC over its carry weight: `IsOverEncumbered` no (ours **agrees**).
- `ActorBase.SetProtected(true)`: `IsProtected` yes, cleared no (ours
  **agrees**).

**FactionConditions** (Alpha rank 0 and Beta rank -1 in Teal)
- `SameFaction(Beta)` no: rank -1 isn't membership. `SameRace(Gamma)` no:
  Gamma is a Nord with an Orc traits template. *Settled*, ours **agrees**.
- `GetFactionCombatReaction`: (Red, Green) 1, (Green, Red) 0, (Teal, Teal) 0.
  *Settled*: one way, and a faction isn't its own ally unless it lists
  itself. Ours gives 2 for a faction towards itself: **differs**.
- `GetFactionRelation(Beta)` on Alpha: 0 (ours **agrees**: no shared
  membership at rank -1).
- `GetFactionRankDifference(Teal, Beta)` on Alpha: 0 (only 0 matched in
  -3..3). Alpha's rank is 0 and Beta's -1. *Settled* for this case. Ours
  (0 − (−1) = 1) **differs**. Either a rank below 0 counts as 0, or the
  function always gives 0.
- `GetShouldHelp(Beta)` no (ours **agrees**).
- `GetInCellParam(our cell, Beta)` yes (ours **agrees**).
- `GetScriptVariable(Alpha, "Marker") == 7`: no, run on Beta and on Alpha.
  Alpha's Papyrus script has `Int Marker = 7`. A missing variable `== 0`:
  yes. *Settled*: the condition doesn't read Papyrus variables. Ours does:
  **differs**.

## Geometry

Alpha faces north, Beta faces east.
- `GetHeadingAngle(Beta)`: in [90, 135) with Beta east, [180, 225) south,
  [-135, -90) west and [0, 45) north. The natives gave 90, 180, -90 and 0.
  *Settled*: the condition is the native's value. Ours **agrees**.
- `GetRelativeAngle(Beta)`: in [0, 45) for all four. *Fault*: the probe gave
  no axis (its second parameter was 0).
- `GetWithinDistance(Beta, 300)`: yes at 300, exactly 300, 299, and 300
  straight up. *Settled*: inclusive, 3D. Ours **agrees**.
- `IsCloserToAThanB(Beta, Gamma)`: no at a tie (200 / 200), yes at 200 /
  250. *Settled*, ours **agrees**.
- `GetLineOfSight(Beta)` 200 ahead in the open: yes (HasLOS True). Beta 200
  behind Alpha: no (HasLOS False). *Settled*: an actor's line of sight
  takes its facing into account. Ours (a clear line from the eyes)
  **differs**.
- A created actor placed at the cell's centre marker (`PlaceActorAtMe`),
  moved to x 123 and then to -400: `GetStartingPos(X)` equals the marker's x.
  *Settled*: where it was created. Ours **agrees**.

## Death

Victim (a plugin NPC) killed by `Victim.Kill(Alpha)`; `GetKiller` gives
Alpha.
- `IsKiller(Victim)` on Alpha: no. `IsKiller(Alpha)` run on Victim: yes.
  *Settled*: the condition runs on the victim and names the killer. Ours has
  it the other way: **differs**.
- `IsKillerObject` on Alpha, with a list of Victim's base and with a list of
  Victim's reference: no both. *Settled* that it isn't "the subject killed
  one of these". By analogy with `IsKiller`, it is likely "the subject's
  killer is in the list". Ours **differs**.
- `GetCauseofDeath` on Victim: -1 (from -2..12). *Settled* for a script
  `Kill`. Ours leaves it unsupported.
- `GetTimeDead` 70.7 s after the kill (0.41 game hours at timescale 20): `>=
  1` yes, `>= 10` no (the probes ran up to about 85 s, 0.47 game hours). Not
  game hours, nor game minutes or real seconds. Real minutes, or a tenth of
  a game hour, fit. *Partial*: ours (game hours) **differs**, and the unit
  is open.
- A teammate's kill: `GetKiller` is the player (as autotest21).

## Combat state

**InCombatParam**: fighting (state 1), `IsInCombat(0)`, `(1)` and `(2)` all
yes. After `StopCombat`, with the foe gone, `(0)` and `(1)` no. *Settled*:
the parameter makes no difference here. Ours ignores it: **agrees**.

## Natives

**UniquePlain**: a base flagged unique with no template: `IsUnique` True.
The same flag on a base under a non-unique traits template: False. Gerdur:
True. *Settled*: `IsUnique` follows the traits template. Ours reads the
base's own flag (the native and condition 641): **differs**.

**Disarm**: a level 1 and a level 10 NPC, sword drawn and fighting: still
holding it after the Disarm spell, no events. *Fault* again. The spell
cast by script does nothing; check the shout effect's conditions and
archetype before a rerun.

**InvisibleFood**: Invisibility on the player (1); a sweet roll eaten by
`EquipItem`: 0. *Settled*: eating ends it, as drinking did in autotest21.
Ours doesn't interrupt on consumption: **differs**.

**InvisibleActivate**: `Activate(player)` on a locked key-only chest: still
invisible. *Settled* for a locked target. Ours interrupts on any script
activation as the player: **differs**.

## Story Manager

**StoryPickups**, 4 s between events:
- An unowned ingot picked up (`Activate(player)`): `AIPL`, owner None,
  container None, type 4 (pick up).
- An ingot owned by an NPC base, and one owned by a faction: `AIPL`, owner
  None, type 1 (steal).
- `RemoveItem` from a chest to the player, and back: none.
- *Settled*: stolen pickups send `AIPL` as type 1. The owner member is None
  for a base owner and a faction owner alike. Ours: compare.
- Left from earlier tests: two `SKIL Illusion` events 41 and 45 s into the
  run, when the script cast Invisibility on the player. So script casts with
  the player as caster give skill use. A `KILL` with killer None at 480 s
  came from ActorConditions, whose only kill was `Kill(None)` on an
  essential NPC (which went into bleedout). So that `KILL` was probably sent
  for an essential actor that didn't die. Compare, and confirm before
  relying on it.

**CreatedRefs** (optional aliases created at the player): both made, an
actor and an iron sword in the world. After the aliases were cleared, the
quest stopped, started again (new references), completed (`CompleteQuest`
left it running) and stopped: every created reference stayed, not disabled,
not deleted, its 3D loaded, 2 s after each step. *Settled*: neither Clear,
Stop nor CompleteQuest removes created references at once. Ours disables
objects created in the world when the quest stops: **differs**. Whether
they go later (unloaded, or a cell reset) is open.

**CreateLevel** (a leveled bandit at create levels easy and very hard,
player level 1): level 1 both times, with different bases (0x39CFA and
0x1BCD8) and races. *Partial*: one pick each can't separate the create
level from chance.

## Magic and fights (in `WarehouseFurniture`)

**Casters**: a copy of the vanilla fire warlock (level 12, Destruction 39,
magicka 132) on a held target 900 off stopped at about 830 to 850 and cast
from there. Firebolt was in both hands, with casts about every 2 s
(Cast events at 110.21, 110.25... s: 2.3, 1.8, 2.0, 2.4 apart) and an
unequip/equip between. *Settled*: vanilla casters keep their range and
cast. Ours: compare.

**NpcCostReduction**: the warlock at DestructionMod 0 spent 32.36 per
Firebolt (three casts). At 50 it spent 31.27 and 32.36 on Firebolts. At -50
it switched to what looks like Flames, at about 16 a second. The first
reading was 130.435 in all three runs, whatever the start. *Partial*:
DestructionMod 50 didn't lower Firebolt's 32.36 for this NPC. The
concentration spending mixes in, so compare with our cost formula before
deciding.

**Healer**: a Vigilant (level 5, Restoration 20) fighting, set to 70%, 45%
and 20% health: no heal in 8 s each, its magicka rising (21 to 43).
*Partial*: no heal below `iAICombatRestoreHealthPercentage` (60). Its
magicka may have been too low for its spell: compare with the spell's cost.

**Wards**: a Vigilant held 700 from a held fire warlock: WardPower 0
throughout and health falling from the 8th second. The warlock cast nine
times. *Partial*: this Vigilant didn't ward.

**NpcStaff**: a warlock copy without spells, given a staff of sparks:
`IsEquipped` False, yet the left hand reports the staff and the right an
iron dagger. Left charge 500 → 457.5 over 10.5 s (about 8.7 every 1.5 s
while casting), with two Cast events. *Settled*: NPCs use a staff from the
left hand and spend its charge, about 5.8 a second for sparks. Ours:
compare.

**FightAfterAlly**: Red and Green fighting, then made allies: both still
fighting 8 s later. *Settled*, ours **agrees** (fights go on).

**Flee**: an Average NPC unharmed, and another at 40%, set on a level 20
fighter: neither fled in 12 s, health falling to 0.72 and 0.18, staying
within 60–240 units in state 1. *Settled*: Average confidence doesn't flee
at 18% health within 12 s. Ours: compare (`known_ambiguities/combat-fleeing.md`).

**PlayerEnemy**: before, the aggressive Red fighter detects the player and
isn't hostile. After `SetPlayerEnemy`: within 4 s it is hostile, in combat
and targets the player. The unaggressive Red member isn't hostile, doesn't
detect, and moves away (498 → 883 units). *Settled*. Ours **agrees**
(unaggressive members left alone; whether they flee is open).
`Player.SetAttackActorOnSight(true)`: neither Red member turned hostile in
6 s, the aggressive one detecting the player. *Settled* for an aggressive,
non-enemy actor. Ours (anyone with aggression 1+ who doesn't like the marked
actor attacks it) **differs**. Left for a follow-up before changing (try
aggression 2 and 3).

**AttackOnSight**: a guard of our crime faction 500 off, at bounty 0, 1000,
and violent 1000: never hostile or fighting. *Fault*: our faction had Arrest
without Attack on Sight (`CRVA`), so attack wasn't expected. (Ours wouldn't
attack either; arrest approaches aren't read.)

**Holds**: Fear on a level 8 NPC fighting a held target, at magic
resistance 0, 50 and 90: distances 196 → 150, 168 → 245, 155 → 107, combat 1
each. Calm on a level 1 fighter: combat 1 throughout. Fury on an
unaggressive NPC: no combat, no target. *Fault*: distance during a fight
doesn't show fleeing, and nothing shows the spells landed. Next time, probe
`IsFleeing` and log effect starts.

## Detection

**ViewCone**: an observer facing north-east, the player sneaking at light
31.9, read at 2 s and 5 s (always the same):
- At 300: detected at 0°, 45° and 90° from its facing; not at 135° or 180°.
- Ahead: detected at 600, not at 1200 or 1800.
- Blindness 100: not detected at 600 or at 150.
- *Settled*. Ours: the view half-angle (95°) **agrees**. A rough hand
  computation of our detection value is above 0 at 600 and below at 1200
  with Blindness 0, and below at 150 with Blindness 100: **agrees**. Check it
  in a test.

## Outdoors

**SunPosition** (at the run's start, Tamriel cell (4, -12)):

| game hour | X | Y | Z |
| ---: | ---: | ---: | ---: |
| 6 | 308.058 | 40 | 91.931 |
| 9 | 290.477 | 40 | 109.536 |
| 12 | 72.295 | 40 | 327.718 |
| 18 | -364.071 | 40 | 35.915 |

*Settled* as readings: not a unit vector (lengths 324 to 368), Y constant at
40, east (+X) in the morning and west in the evening. Ours returns a unit
direction: **differs** in scale. The law is open (each read came 1 s after
setting `GameHour`).

## Faults and open items, for a follow-up batch

1. `GetXPForNextLevel`: a finer scan, before and after a skill increase.
2. `GetTimeDead`: read at two widely separated times (real and game time
   apart, e.g. a `Wait` with a raised timescale).
3. `GetRelativeAngle` with an axis.
4. Torch and shield "drawn" readings: draw in combat, or keep the NPC alert.
5. `GetCrimeGold` without a faction inside a hold, and `GetDisease` on the
   diseased player.
6. `SetAttackActorOnSight` with aggression 2 and 3; a crime faction with
   Attack on Sight.
7. Disarm, Holds (probe `IsFleeing`, log effect starts), Healer (a healer
   with magicka enough), Wards.
8. CreateLevel at a higher player level, several picks each.
