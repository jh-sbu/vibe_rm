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
- **Message**: `sSuccessfulSneakAttackMain` + the multiplier to one decimal +
  `sSuccessfulSneakAttackEnd` ("Sneak attack for 3.0X damage!").

## Open (choices made here)

- **Bows and crossbows: 2x.** No game setting holds it; UESP gives double
  damage before Deadly Aim (3x).
- **Staves** strike without a sneak bonus (no setting for them).
- **Before armor**: the multiplier applies to the blow's damage before the
  target's armor takes its share. The order isn't published.
- **Bashes** are never sneak attacks.
- **The player as a target**: NPCs never sneak attack the player, since the
  player has no detection of actors to test.
- **Settings left unused**: `fCombatSneakAttackBonusMult` (100),
  `fCombatDamageBonusSneakingMult` (2), `fDamageSneakAttackMult` (1). What they
  scale isn't public.
- **Not done**: perks' "Mod Sneak Attack Mult" entry point (Backstab 6x,
  Assassin's Blade 15x with daggers, Deadly Aim 3x), waiting on the perk
  system; sneak skill gain (`fSneakAttackSkillUsageMelee`), waiting on skill
  advancement.
