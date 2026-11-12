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
- **Who it reaches.** Living actors whose feet are within the radius
  (Skyrim SE's falloff fits the distance to the feet: `test-mod
  explosions`, `research/test-mod-explosions.md`), and loaded references
  other than actors whose position is: destructible ones and those with a
  script handling `OnHit`.
  Not the reference that went off or set it off. The radius is the record's
  as it is, not scaled by anything.
- **Line of sight** (UESP's Elemental Burst: shaded targets take nothing):
  a ray from 8 units above the centre to the middle of an actor's body (60
  above its feet, by scale; the player's capsule centre) or an object's
  moving centre, passing through the source (or the owner); the target's
  own body ending the ray counts as reached. "Ignore LOS Check" skips the
  ray. What is out of sight takes nothing but still hears `OnHit` (SE: an
  explosion on the floor hurt nobody yet all three NPCs heard it). How SE
  tests sight is open: from a point on the floor it seems to fail even in
  the open; ours lifts it 8 units.
- **Damage.** The `DATA` damage times 1 − (d / radius)² and through armor
  as a blow's (both Skyrim SE's readings), taken as health damage as from a
  blow by the owner (a fight and an assault), scaled by difficulty; not
  changed by resistances or perks (no reading). Destructible references
  within take the same falloff as a blow (stages that ignore external
  damage take none).
- **The enchantment** lands on every actor reached, cast by the owner (no
  caster without one), quietly as far as the magic goes: the explosion's
  own `OnHit` stands for it, and a hostile enchantment with an owner starts
  the fight a hostile spell would.
- **`OnHit`**: to each actor and reference within the radius, the owner
  the aggressor (`None` without one), the weapon behind it, else the
  explosion, the source, the projectile behind it the projectile, all four
  flags false. One it hurt first hears the `Unarmed` weapon (`000001F4`,
  no projectile) as SE sends (about 0.03 s before, here at once). The
  owner of a placed explosion is none, as SE gave. An explosion set off by a spell landing sends none: the spell's own
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
- **Pushing what moves.** Dynamic bodies (loose objects, corpses' ragdolls;
  living actors only through knockdowns) whose centre of mass is within the radius and in
  sight get an impulse away from the centre: the force times
  `fExplosionForceMultLinear` read as a Havok impulse (kg m/s), falling off
  linearly to nothing at the radius (the mass illusion spells' explosions
  reach 5333 units with force 6: at full strength they would scatter every
  cup around the caster), but never more than sends the body off at 700 units a
  second (10 m/s; no source: the catapult's 240 would throw a 1 kg cup at
  240 m/s). The vertical offset mult is added to the push's upward part
  (values 0 to 500 in the records; read as a lift, so 500 is straight up).
  `fExplosionForceMultAngular` (1) and any spin are left out.
- **"Push Explosion Source Ref Only"** pushes only its source, with the
  force times `fExplosionSourceRefMult` (3), with no line of sight test.
- **The model** plays once where it goes off, for its sequence's length
  (`CWCatapultExplosion.nif`: `SpecialIdle_AreaEffect`, text keys `start`,
  `Hit` at 0.033 s, `end` at 3.5 s). Explosion models name their one
  sequence for its use, so the first is played when there's no `Idle`.
  Its +Z faces back the way what set it off came (a placed one's points
  up); "Always Uses World Orientation" keeps the world's axes. The `Hit`
  key isn't waited for (see When).
- **The light** (`DATA` light) is at the centre, 8 units up, fading
  linearly to nothing over the model's length (at least 0.5 s).
- **The image space modifier** is applied at full strength when the player
  is within the IS radius as it goes off; nothing scales it by distance.
  "Ignore Image Space Swap" and `fExplosionImageSpaceSwapPower` are unused.
- **Sounds 1 and 2** both play at the centre.
- **The impact data set** lands once, on the surface it went off on, along
  the way what set it off was going (straight down for a placed one).
- **Explosion references.** Every explosion has a reference while it lasts
  (the model's length, at least 0.5 s), then it is deleted. `PlaceAtMe` of
  an `EXPL` returns it; others get one made in the place of their source
  (or owner, or the player). A `PlaceAtMe` explosion spares nobody (the
  reference it is placed at is often the actor it is meant for:
  `magicPlaceExplosionScript` places it on the effect's target) and is
  blamed on that reference's actor cause only, never on the reference
  itself. One placed initially disabled doesn't go off.
- **The placed object** goes where the explosion went off: a hazard as an
  impact's would (`spawn_hazard`), an explosion going off next frame (blamed
  on the same owner), anything else created at the explosion's reference.
- **What sets them off.** Where a spell lands (each landing: a
  concentration spell's every 0.25 s tick too, as its impacts): a dual
  cast's `DUAL` explosion, else its projectile's (`PROJ` `DATA` 0x24), else
  the first of its effects' (`MGEF` `DATA` 0x4C). One explosion, not one for
  each. A self-delivered spell's spares its caster; an aimed one's doesn't
  (UESP's Flaming Familiar: "the explosion does damage to ... the caster").
  Arrows and bolts with one set it off where they stop (in an actor, the
  world or a loose object; not when they run out of time), blamed on the
  shooter, sparing a shooter that isn't an actor (traps), with the
  shooter's weapon as the `OnHit` source. Destruction stages set off the
  explosion of the stage entered (only that one, if a blow crosses
  several), at the object's moving centre or its position, blamed on its
  actor cause, sparing the object.
- **Runes** (UESP *Skyrim:Rune (effect)*, revision 3157529: enemies
  within melee range of the circle set it off; the caster, their followers
  and summons don't; one rune at a time; any projectile spell sets it
  off). A spell whose projectile has "Alt. Trigger" (`PROJ` flag 0x4: the
  runes, proximity 150, timer 0) lies where its projectile stops, unless it
  struck an actor (then it lands at once, as a rune cast under an enemy's
  feet does). It goes off, as the spell landing there, when a living actor
  within the proximity (its feet) fights the caster, is fought by them, or
  either would attack the other on sight; when its timer, if any, runs out;
  or when an aimed or target location spell lands within the proximity.
  "One rune at a time" is read as one for each caster, whatever the
  spell: a new one ends the last. A rune whose cell unloads is gone. The
  Light Foot perk, the hidden caster's crime rule and the re-targeting the
  page describes are left out; the rune's decal isn't drawn (only its
  projectile's model, if it has one).
