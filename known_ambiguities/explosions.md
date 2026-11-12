# Explosions

Choices made to get explosions working where the sources
(`research/explosions.md`) are silent. `src/explosions.rs`;
`plans/explosions.md`.

- **The short `DATA` layouts.** The 48-byte record
  (`TrapPoisonGasExplosion01`) is read without the vertical offset mult,
  since that reading gives the flags and sound level of its siblings. The
  44- and 40-byte records (six) are read for their model and enchantment
  only: no force, damage, radius or flags. Their fields' order is unknown;
  `fakeForceBall1024` (used by two projectiles and a script) and
  `fakeForceBallTiny` (fireflies' and bees' death push) therefore push
  nothing.
- **Who it reaches.** Living actors whose body's middle (60 units above
  the feet, by scale; the player's capsule centre) is within the radius,
  and loaded references other than actors whose position (a moving body's
  centre) is: destructible ones and those with a script handling `OnHit`.
  Not the reference that went off or set it off. The radius is the record's
  as it is, not scaled by anything.
- **Line of sight** (UESP's Elemental Burst: shaded targets take nothing):
  a ray from 8 units above the centre to what it reaches, passing through
  the source (or the owner); the target's own body ending the ray counts as
  reached. "Ignore LOS Check" skips the ray.
- **Damage.** The `DATA` damage, the same at any distance within the
  radius, taken as health damage as from a blow by the owner (a fight and an
  assault), scaled by difficulty but not reduced by armor, resistances or
  perks. Destructible references within take it as a blow (stages that
  ignore external damage take none).
- **The enchantment** lands on every actor reached, cast by the owner (no
  caster without one), quietly as far as the magic goes: the explosion's
  own `OnHit` stands for it, and a hostile enchantment with an owner starts
  the fight a hostile spell would.
- **`OnHit`**: to each actor and reference reached, the owner the
  aggressor (`None` without one), the weapon behind it, else the explosion,
  the source, the projectile behind it the projectile, all four flags
  false. An explosion set off by a spell landing sends none: the spell's own
  two `OnHit`s (`test-mod autotest4`) stand for it.
- **The owner** is the actor it is blamed on, through their actor cause
  (`SetActorCause`). An explosion with no actor owner hurts without
  starting fights.
- **Noise**: the owner's, of the record's sound level, at the centre.
  Nobody's explosion makes no noise, as for other noises
  (`known_ambiguities/detection.md`).
- **When.** All of it at once when it goes off; nothing waits for the
  model's hit key time. An explosion set off while another goes off waits
  for the next frame (at most 32 a frame), so chains of destructible
  objects can't recurse.
- **Knockdowns.** "Knock Down - Always" knocks down every living actor
  reached; "By Formula" those left at or below
  `fKnockdownCurrentHealthThreshold` (50) percent of their health once its
  damage is taken. The setting's name is the only lead to the formula. The
  push is `crate::knock`'s (as `PushActorAway`'s) with the explosion's
  force. Explosions without either flag don't move living actors or make
  them stagger.
- **Pushing what moves.** Dynamic bodies (loose objects, corpses' and the
  knocked down's ragdolls) whose centre of mass is within the radius and in
  sight get an impulse away from the centre: the force times
  `fExplosionForceMultLinear` read as a Havok impulse (kg m/s), the same at
  any distance, but never more than sends the body off at 700 units a
  second (10 m/s; no source: the catapult's 240 would throw a 1 kg cup at
  240 m/s). The vertical offset mult is added to the push's upward part
  (values 0 to 500 in the records; read as a lift, so 500 is straight up).
  `fExplosionForceMultAngular` (1) and any spin are left out.
- **"Push Explosion Source Ref Only"** pushes only its source, with the
  force times `fExplosionSourceRefMult` (3), with no line of sight test.
