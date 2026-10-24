# Actor and reference natives without a better home

Vanilla call counts are `vrm-tool pex-natives` (calls are matched by name, so
names shared between classes share their count).

## `GetRace`

- `Actor.GetRace` and `ActorBase.GetRace` (90 calls) return the race the
  NPC's templates give (`RNAM` through template flag 0x1, Use Traits), the
  same lookup the renderer and impacts use; a form that is no NPC returns
  None.
- Unconfirmed: what `ActorBase.GetRace` returns for a base whose traits come
  from a leveled list (we follow the first pick, as for a reference with no
  seed), and whether it returns the base's own `RNAM` instead. `SetRace` is
  not supported, so the race never changes.

## `GetNthLinkedRef`

- `ObjectReference.GetNthLinkedRef(aiLinkedRef)` (66 calls) follows the
  reference's linked ref (`XLKR`, as `GetLinkedRef()` with no keyword reads
  it) that many times: 1 is the linked ref, 2 its linked ref, and so on;
  None once the chain ends. Vanilla scripts pair it with the non-native
  `CountLinkedRefChain` and pick 1..count (`CWCatapultStrikeSpawnerScript`,
  `DartTrap`, the DLC ambush scripts), which fixes 1 as the first link.
- Unconfirmed: 0 and negative numbers (we return the reference itself; no
  vanilla call passes them), and whether a keyworded link counts when the
  reference has no unkeyworded one (`GetLinkedRef()` here takes the first
  link of any keyword, so the chain does too).

## `StopCombatAlarm`

- `Actor.StopCombatAlarm()` (84 calls): the actor stops fighting and
  searching, so does every loaded actor fighting or searching for it, and
  the guards pursuing it give up (for the player: every pursuing guard).
  `StopCombat` stays the actor's own. Vanilla calls it on the player after
  a bribe or persuasion (`CrimeGuardsScript.BribeGuard` /
  `PersuadeGuard`, `DLC2CrimeGuardsScript`) and in quests ending fights
  (`QF_MQ303`, `CWFinaleScript`, `QF_MQSkyHavenSparring`), which is why the
  others' side is ended too. No public description was found.
- Unconfirmed: whether it also forgives resisted arrests
  (`Arrests::resisting`; we leave them, so guards whose arrest the player
  resisted attack again until the bounty is paid, as before), whether
  actors who would attack on sight start again at once (we do nothing to
  stop them: a bribe pays the bounty first), and whether unloaded actors'
  fights end too.

## `EnableAI`

- `Actor.EnableAI(abEnable)` (27 calls) turns one actor's AI off (false) or
  back on (true, also when the argument is missing). An actor with its AI off stands where it is, as everyone does when
  the console's `tai` turns all AI off: no packages, walking or fighting
  moves. Saved in optional `AIOF`; legacy saves have none.
- Unconfirmed: whether its animation graph freezes too (ours keeps
  playing), whether it still detects, starts combat, speaks and reacts to
  hits (we stop only its movement and actions: detection and combat state
  still change), and whether the flag survives the actor unloading in the
  original.
