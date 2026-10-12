# Sneak attacks

Implemented in `src/ai/combat.rs` (`Engine::sneak_attack_mult`, `Engine::hit`).
Sources: the game settings named below (Skyrim.esm), UESP's Skyrim:Sneak page
and the perk descriptions.

## Settled

- **What counts**: the attacker is sneaking and the struck actor doesn't
  detect it (detection value at or below zero, `crate::detection`), the
  `[HIDDEN]` eye.
- **Melee multipliers** by the weapon's animation type: `fCombatSneak1HSwordMult`,
  `1HDagger`, `1HAxe`, `1HMace` (3), `2HSword`, `2HAxe` (2), `fCombatSneakHandMult`
  (2) unarmed.
- **Bows: 2x**, 3x with Deadly Aim (UESP's Skyrim:Sneak table and
  Skyrim:Deadly_Aim). No game setting holds it.
- **Message**: `sSuccessfulSneakAttackMain` + the multiplier to one decimal +
  `sSuccessfulSneakAttackEnd` ("Sneak attack for 3.0X damage!").

## Open (choices made here)

- **Crossbows: 2x, as bows.** UESP's Sneak and Deadly Aim pages speak only
  of bows; whether crossbows share the multiplier (and Deadly Aim) isn't
  published.
- **Staves** strike without a sneak bonus (no setting for them).
- **Before armor**: the multiplier applies to the blow's damage before the
  target's armor takes its share. The order isn't published.
- **Bashes** are never sneak attacks.
- **The player as a target**: NPCs never sneak attack the player, since the
  player has no detection of actors to test.
- **Settings left unused**: `fCombatSneakAttackBonusMult` (100),
  `fCombatDamageBonusSneakingMult` (2), `fDamageSneakAttackMult` (1). What they
  scale isn't public.
- **Perks**: the multiplier goes through the attacker's "Mod Sneak Attack
  Mult" entries (Backstab 6x, Assassin's Blade 15x with daggers, Deadly Aim
  3x): `known_ambiguities/perks.md`. Sneak skill gain: `known_ambiguities/skills.md`.
