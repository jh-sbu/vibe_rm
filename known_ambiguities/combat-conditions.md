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

## Not implemented

- `GetAttackState` (574): the game's graph phases (draw, swing, hit, next attack,
  follow-through, bash). The engine tracks only the swing timer and whether a
  blow landed. Vanilla idles also compare it against values 7 to 12, which no
  source documents (`ROADMAP.md` Next, game logic).
- `GetShouldAttack` (66): the engine's attack-on-sight decision (`hostile_to`)
  needs `&mut` for its faction cache. The Grand Healing talk (`Skyrim
  talk:Grand Healing`, oldid 1330973) says it checks hostility without
  `IsHostileTo`, which doesn't settle its exact rule.
