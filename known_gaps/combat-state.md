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
- **`OnHit` sources.** Melee and arrow hits send it, with the attacker's
  equipped weapon as the source (none unarmed) and, for arrows, the ammo's
  projectile. Sneak attacks don't exist yet, so `abSneakAttack` is false.
  Damage from scripts and the console (`DamageActorValue`, `damage`) sends no
  `OnHit`; whether the game sends one for those isn't confirmed.
- **`StopCombat`** ends the fight and any search; an actor still hostile
  finds its target again by detection. How soon the game lets it re-engage
  isn't documented.
- **`IsInCombat` (condition 289)** takes an integer parameter in SSE whose
  meaning isn't documented; it's ignored.
