# Combat-state condition functions

Implemented in `src/condition/mod.rs` on the engine's state: `GetCombatState`
(323), `IsCombatTarget` (513), `IsPowerAttacking` (673), `IsShieldOut` (103) and
`IsTorchOut` (102). Engine helpers: `combat_state` and `combat_target`
(`src/ai/combat.rs`), `power_attacking` (`src/ai/combat.rs`), `shield_out` and
`torch_out` (`src/ai/equipment.rs`).

## What is known

- `GetCombatState` values: 0 none, 1 fighting, 2 searching. Sources:
  `Skyrim talk:Console/Archive 1` (oldid 2308857) and the vanilla uses, which
  compare against 0, 1 and 2 (`vrm-tool ctda-uses`, spell and idle conditions).
- The UESP function indices name every function here, but describe only
  `GetCombatState`'s values. Sources: `Skyrim Mod:Function Indices` (oldid
  3566882), `Skyrim:Console` (oldid 3628513).
- `IsPowerAttacking` is used by a Block perk talk (`Skyrim talk:Block/Archive 3`,
  oldid 1562595), as the enemy's state while it swings.

## Unconfirmed choices

- `IsCombatTarget` and `GetCombatState` read the same target as the engine's
  `GetCombatTarget` native: a searching actor's target is the one it lost.
  Whether the game clears the combat target while the actor searches is not
  documented.
- `IsPowerAttacking` is true while the attack the actor is making is a power
  attack, from the swing's start to its end (NPCs), or until the player's
  blows land (the player's swing is tracked only then). Whether a power bash
  counts is not known; this follows the engine's `power()` flag, which a power
  bash also carries.
- `IsShieldOut` is a shield in the left hand while weapons are drawn; a
  sheathed shield is put away. The UESP pages give the function's name only.
- `IsTorchOut` is a torch held in the left hand, drawn or not. The engine has
  no separate lit state, so a torch counts whether or not it is lit.

## Combat groups, lethal intent and assistance

Implemented in `src/condition/mod.rs` (`function_value`) on the engine's
combat state: `GetCombatGroupMemberCount` (709), `GetGroupTargetCount` (416),
`IsLastHostileActor` (674), `GetCombatTargetHasKeyword` (707),
`ShouldAttackKill` (678), `GetShouldHelp` (705), `IsProtected` (476) and
`GetActorAggroRadiusViolated` (118). Engine helpers: `combat_group`
(`src/ai/threat.rs`, the sides threat ratios already use), `spared_by`,
`should_help`, `is_protected` and `aggro_radius_violated`. Tests in
`src/ai/combat_condition_tests.rs`.

### What is known

- UESP Skyrim:Console (oldid 3628513) lists the names and describes only
  `IsProtected` ("Check whether the target actor has ..."); the raw function
  data (oldid 3411473) gives `GetShouldHelp(Actor)`, `ShouldAttackKill(Actor)`
  and `GetCombatTargetHasKeyword(Keyword)`.
- Vanilla uses (`vrm-tool ctda-uses`): `ShouldAttackKill` is 1 on the subject
  of every killmove and werewolf paired idle (null actor); `IsLastHostileActor`
  is 1 on the *target* of the killmove roots; `GetCombatTargetHasKeyword`
  asks for `PlayerKeyword` in dialogue (the combat target is the player);
  `IsProtected` is asked once, by a quest (`CR08`). The group counts,
  `GetShouldHelp` and `GetActorAggroRadiusViolated` are never used.
- The combat group of CommonLibSSE's `CombatGroup` has members and targets;
  the engine's threat code already sorts fighters into two sides.
- Essential actors, and protected ones but to the player, bleed out instead
  of dying (`known_ambiguities/death.md`; `ACBS` 0x2 and 0x800).

### Unconfirmed choices

- The combat group is the subject's side of its fight: the fighters
  (including the player while anyone fights them) who fight its targets or
  anyone fighting its side, itself included; its targets are the other side.
  Only loaded, living, standing fighters count, and an actor not fighting has
  no group (both counts 0). The game's own grouping rules aren't documented.
- `IsLastHostileActor` is true when the subject is alone on its side of a
  fight. The killmove idles ask it of the victim, so this reads as "no one
  else is still fighting the attacker".
- `ShouldAttackKill` is true unless the actor (the parameter; null is the
  subject's combat target, else the context's target, which is the victim in
  an idle) is one the subject can't kill: essential, or protected and the
  subject not the player. This is inferred from the killmove idles; whether
  hostility, yielding or non-lethal fights also make it false is unknown, and
  hostility isn't checked.
- `GetShouldHelp` follows the assistance rule combat uses to join a fight:
  an ally (reaction 2) when assistance is 1 or 2, a friend (3) when it is 2,
  and a teammate for the player; the strongest reaction among the two
  actors' factions, as `GetFactionRelation`. Only loaded helpers answer.
- `IsProtected` is the loaded actor's flag, else its base's `ACBS` flag.
- `GetActorAggroRadiusViolated` is true for a living loaded actor with an
  attack aggro radius (`AIDT`) while the player is within it. The warn
  radius, and whether it stays set after the player leaves, are not modelled.
- `GetCombatTargetHasKeyword` reads the keywords of the combat target as
  `HasKeyword` does (an actor's own and its race's).

## Unsettled, so left unsupported

- `GetAttackState` (574): vanilla idles compare it against values 7 to 12,
  which no source documents (the graph phases it reports are in `ROADMAP.md`
  Next, game logic).
- `IsIgnoringCombat` (710) and `GetActorsInHigh` (554): UESP lists the names
  only and nothing in vanilla calls either; no source says what state they
  report (the engine has no ignore-combat flag).
- `GetShouldAttack` (66): the Grand Healing talk (`Skyrim talk:Grand Healing`,
  oldid 1330973) says it checks hostility without `IsHostileTo`, which doesn't
  settle its exact rule.
