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
- Skyrim SE (autotest21, `research/autotest21.md`, FactionNatives): the
  player at rank 2, `SetPlayerExpelled(true)`: still in the faction, the
  rank reading -2, `IsPlayerExpelled` True. Ours keeps the membership and
  the rank, and reads it as -2 for the player while expelled
  (`GetFactionRank`, the native and condition 73).
- Unconfirmed: what else being expelled does, and whether clearing the
  flag gives the rank back (ours does: it was never changed). No vanilla
  script calls `SetPlayerExpelled` or `IsPlayerExpelled` and no vanilla
  condition uses 193 (`vrm-tool pex-calls`, `ctda-uses`).

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

## Who attacks the player by aggression

- Ours (`hostile_to`, `src/ai/combat.rs`): an aggressive actor (aggression 1)
  attacks its factions' enemies, and the player unless it keeps the law (a
  faction tracking crime); very aggressive (2) and frenzied (3) attack
  neutrals too; allies and friends are left alone below 3.
- Skyrim SE (autotest22, autotest23, `research/autotest23.md`,
  AttackOnSightAggression): an aggression 1 member of a plugin faction that
  tracks no crime and lists no relation to the player detected the player
  and didn't attack, before or after `SetAttackActorOnSight`. Ours would
  attack it: differs. Aggression 2 and 3 NPCs of no faction attacked the
  neutral player at once, as ours.
- Open: what makes bandits (aggression 1, `BanditFaction`, which lists no
  enemy relation to `PlayerFaction` and tracks no crime) attack the player
  in the game, if not our rule. Kept until that is known; dropping the rule
  would leave bandits peaceful.

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
- Skyrim SE (autotest22, autotest23): the marked player wasn't attacked by
  an aggression 1 NPC that wasn't already hostile. That NPC doesn't keep the
  law, so ours had it hostile before the mark (see above); the mark's effect
  on law-keeping aggressive actors, as ours uses it, wasn't read.
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
- Skyrim SE (autotest21, FactionNatives) agrees: a faction isn't in its
  own crime group unless listed, and the check is one way (a grouping
  faction lists Red and has it; Red doesn't have the grouping faction).
  The native has no vanilla callers (`vrm-tool pex-calls`).
