# Combat state for scripts

Implemented in `src/ai/combat.rs` (`CombatState`, `combat_state`,
`report_combat_states`, `send_hit_event`) with the natives in
`src/script/natives.rs`. Sources: the Creation Kit wiki pages for
`Actor.GetCombatState`, `OnCombatStateChanged`, `OnHit`, `OnEnterBleedout`,
`StartCombat` / `StopCombat`, and how the game's own scripts use them
(`activateSelfOnCombatBegin`, `defaultOnHitChangeAggression`).

## What is known

- States: 0 not in combat, 1 in combat, 2 searching.
- `OnCombatStateChanged(Actor akTarget, int aeCombatState)` goes to the actor
  (and the aliases it fills) when its state changes.
- `OnHit(ObjectReference akAggressor, Form akSource, Projectile akProjectile,
  bool abPowerAttack, bool abSneakAttack, bool abBashAttack, bool abHitBlocked)`.
- In the game (`test-mod autotest2`, 2026-10-13): `DamageActorValue` by script
  sends no `OnHit`; a fist's blows send the `Unarmed` weapon (000001F4) as
  the source, no projectile, `abPowerAttack` set for power attacks. A fighter
  still hostile was in combat again 0.43 s after `StopCombat` in both runs
  (its state 0 at once in one, still 1 in the other). A Firebolt cast by
  script (`Spell.Cast`) sent two `OnHit`s 0.1 s apart, the spell as the
  source and `FireboltProjectile01` as the projectile. An archer's hits sent
  the bow as the source and no projectile.

## Open

- **Searching.** The engine's searching is its own detection model (see
  `detection.md`): a fighter that loses its target searches (state 2); an actor
  alert to someone it hasn't found yet counts as not in combat. Whether the
  game reports the alert state as 2 is not documented.
- **Target changes.** Switching targets while fighting sends no event here; the
  wiki doesn't say whether the game sends one.
- **The player's state.** `Game.GetPlayer().GetCombatState()` is answered from
  the actors fighting (1) or searching for (2) the player; the player gets no
  `OnCombatStateChanged`. No source says how the game decides it.
- **`OnHit` sources.** Melee and arrow hits send it, with the weapon that
  struck as the source (the blow's hand's) and, for arrows, the ammo's
  projectile, though the game's archer's hits came with none (whether those
  were arrows wasn't seen); a spell sends its projectile (but once here,
  not twice as the game's Firebolt did). `abSneakAttack` is set for sneak
  attacks. The console's `damage` sends none, as scripts' don't. A
  fighter given an iron sword landed no blow on the target in 8 s in the
  game: a sword's hits are still to be seen.
- **`StopCombat`** ends the fight and any search; an actor still hostile
  finds its target again by detection, at the next detection update here
  (a quarter second).
- **`IsInCombat` (condition 289)** takes an integer parameter in SSE whose
  meaning isn't documented; it's ignored.
