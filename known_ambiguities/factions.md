# Factions' runtime standing

## Public sources

- Vanilla `Faction.pex` (the game's `Scripts`, read with `vrm-tool pex-dump`):
  native signatures `SetEnemy(Faction akOther, Bool abSelfIsNeutralToOther,
  Bool abOtherIsNeutralToSelf)`, `SetAlly(Faction akOther, Bool
  abSelfIsFriendToOther, Bool abOtherIsFriendToSelf)`, `SetReaction(Faction,
  Int aiNewValue)`, `ModReaction(Faction, Int aiAmount)`, `GetReaction`,
  `SetPlayerEnemy(Bool abIsEnemy)`, `SetPlayerExpelled(Bool abIsExpelled)`,
  `IsPlayerExpelled()` and `IsFactionInCrimeGroup(Faction akOther)`. There is
  no getter for the player-enemy flag.
- [Papyrus Faction API](https://papyrus.bellcube.dev/skyrimse/script/faction/)
  (one-line descriptions, no parameter notes): SetEnemy "sets the two
  factions to be neutral or enemies of each other"; SetAlly "friend or allies
  of eachother"; Set/Mod/GetReaction "the faction's reaction towards the other
  one"; SetPlayerEnemy / SetPlayerExpelled "sets or clears the enemy /
  expelled flag from this faction for the player"; IsFactionInCrimeGroup
  "checks to see if the passed in faction is in this faction's crime group".
- CommonLibSSE `TESReactionForm.h`: each `XNAM` is a `GROUP_REACTION` of
  form, `int32 reaction` and `FIGHT_REACTION fightReaction` (0 neutral,
  1 enemy, 2 ally, 3 friend). `TESFaction.h`: `SetAlly` / `SetEnemy` take the
  same two booleans as Papyrus, and a separate `SetFactionFightReaction`.

## Combat reactions (`SetEnemy`, `SetAlly`)

- Each call sets both factions' group combat reaction towards the other:
  enemy (1) or, where that side's boolean is true, neutral (0) for
  `SetEnemy`; ally (2) or friend (3) for `SetAlly`. This is read from the
  parameter names and the one-line descriptions. The reaction modifier is
  left as it was.
- Changes are kept per (faction, towards) pair over the plugin's `XNAM`
  (added where the plugin listed nothing) and saved in optional `FRCT`;
  legacy saves have none. Combat's hostility checks, `GetFactionCombatReaction`
  and `GetFactionRelation` all read them.
- Unconfirmed: whether fights under way end when the factions stop being
  enemies. Vanilla resets such as `DA06LargashburHostileResetScript`
  (`SetEnemy(PlayerFaction, true, true)` with no `StopCombat`) suggest they
  might. We leave fighters fighting until they lose their target; searchers
  give up because they no longer would attack. Also unconfirmed: whether a
  faction listing another more than once has every entry changed (we answer
  with the change alone).

## Reaction modifiers (`SetReaction`, `ModReaction`, `GetReaction`)

- These read and change the `XNAM` reaction modifier (the `int32` CommonLibSSE
  calls `reaction`) of this faction towards the other, one way only. They
  leave the group combat reaction alone, and nothing else here reads the
  modifier. Saved with the combat reactions in `FRCT`.
- Unconfirmed: which field "reaction" means. CommonLibSSE's names favour the
  modifier (the combat value is `fightReaction`, with its own
  `SetFactionFightReaction`). Against that, the only vanilla call,
  `QF_ccBGSSSE001_Crab_MQ4_050009E9`, sets the guards' reaction towards the
  flaming crabs to 1, which reads like "enemy" if it is the combat
  reaction. `ModReaction` and `GetReaction` have no vanilla callers.

## The player's flags (`SetPlayerExpelled`, `SetPlayerEnemy`)

- Each faction's flags start as its `DATA` bits 2 (expelled) and 3 (enemy),
  as CommonLibSSE `FACTION_DATA` names them. `SetPlayerExpelled` and
  `SetPlayerEnemy` set or clear one flag, true if the argument is missing.
  `IsPlayerExpelled`, `GetPCExpelled` (193) and `GetPCEnemyofFaction` (197)
  read them, the conditions whoever their subject is. Changes are saved in
  optional `FPLR`; legacy saves have none, so factions are as their plugins
  say. Calls on forms that aren't factions do nothing.
- Unconfirmed: what being expelled does besides these reads. No vanilla
  script calls `SetPlayerExpelled` or `IsPlayerExpelled` and no vanilla
  condition uses 193 (`vrm-tool pex-calls`, `ctda-uses`), so we change
  nothing else. Rank, membership and services stay as they were.

## Members of factions the player is an enemy of

- An actor attacks the player on sight when one of its factions, or its
  crime faction, has the player-enemy flag. Actors that are unaggressive
  (aggression 0), whose factions are allied or friendly to the player's, or
  that are teammates are left out. `GetActorCrimePlayerEnemy` (458) reads the
  subject's crime faction alone; the player has none, so it is false for
  them.
- Evidence: vanilla scripts set the flag on the hold crime factions while
  the player is a werewolf (`PlayerWerewolfChangeScript`) or vampire lord,
  on the Dark Brotherhood and hold factions (`QF_DB09_0001EA58`,
  `BlackDoorScript`), and more. UESP
  [Skyrim:Lycanthropy, revision 3637628](https://en.uesp.net/w/index.php?oldid=3637628)
  says "most will attack you on sight", except for followers, the Dark
  Brotherhood, the Circle, the Thieves Guild and others. That page describes
  the werewolf as a whole, which also sets `SetAttackActorOnSight`.
- Unconfirmed: whether the flag alone makes members attack, or only marks
  them (combat could come from `SetAttackActorOnSight` or the crime system).
  Also whether aggression 0 members flee, which is what the lycanthropy page's
  "feared" suggests (we do nothing), and whether faction allies are exempt or
  only teammates. The exceptions the page lists suggest allies are.

## Actors attacked on sight (`SetAttackActorOnSight`)

- `Actor.SetAttackActorOnSight(abAttackOnSight)` (vanilla `Actor.pex`; the
  werewolf, vampire lord and `C03RampageQuest` scripts set it and later clear
  it) marks an actor, the player included. Anyone with aggression 1 or more
  who doesn't like the marked actor (its factions aren't allied or friendly
  towards the actor's) attacks it on sight. Teammates don't attack the
  player. The mark is one way: the marked actor attacks no one new. It is
  saved in optional `AOSG`; legacy saves have no marks.
- The meaning is read from the name and from the werewolf's "most will
  attack you on sight" (UESP Skyrim:Lycanthropy, revision 3637628). No
  public description of the native was found.
- Unconfirmed: whether unaggressive actors flee from a marked actor (we
  leave them alone), whether faction allies of the marked actor are exempt
  (we exempt them, matching the page's exceptions), and whether the mark
  survives in the original's saves.

## Crime groups (`IsFactionInCrimeGroup`)

- `IsFactionInCrimeGroup(akOther)` holds when `akOther` is in the form list
  this faction names in `CRGR` ("the passed in faction is in this faction's
  crime group"). It looks one way, and a faction is in its own group only if
  the list names it. Sharing crimes between witnesses
  (`in_shared_crime_faction`) still checks both lists and equal factions.
- Unconfirmed: whether a faction counts as in its own crime group without
  being listed, and whether the game checks the other faction's list too.
  The native has no vanilla callers (`vrm-tool pex-calls`).
