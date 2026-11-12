# `test-mod explosions`: Skyrim SE readings

Run by the user in Skyrim SE on 2026-11-12 (round 1; the batch is
`crates/tools/src/test_mods/explosions.rs`). Explosions placed with
`PlaceAtMe` at a marker on the test cell's floor, 600 north of its centre;
NPCs held with `SetDontMove`, given 1000 more health.

## Readings

- **PlacedRef** (`TrapBallistaBoltExplosion01`): `PlaceAtMe` gave a
  reference (created, `FF...`) whose base is the explosion. At once and at
  1 s: not deleted, enabled, 3D loaded, at the spot. At 6 s: deleted (still
  "enabled", "3D loaded").
- **Falloff** (same explosion: damage 80, radius 320, ignores line of
  sight), health lost by distance north of the spot, all at floor level:
  40: 78.75; 120: 68.75; 200: 48.75; 280: 18.75; 360: 0. Each actor hit
  logged two `OnHit`s about 0.03 s apart, no aggressor: one with the
  `Unarmed` weapon (`000001F4`) as source, one with the explosion; usually
  the `Unarmed` one first. The one at 360 got none. An NPC 200 south in an
  iron cuirass and shield (`DamageResist` 56) lost 42.549.
- **Shade** (`dunHighGateFlamingThrallFireBallExp01`: damage 60, a fire
  enchantment, radius 320, line of sight checked): the NPC 90 north, the
  one 200 north behind it and the one 200 east in the open each lost 0
  health in 4 s, and each logged one `OnHit` with the explosion as source.
- **Knockdowns**: `GetKnockedStateEnum` read no knockdown for any case,
  "Knock Down - Always" included, and no NPC moved.

## What they settle

- **Falloff**: damage × (1 − (d / radius)²) fits all four to the hundredth
  (78.75 = 80 × (1 − 1/64); 68.75 = 80 × (1 − 9/64); 48.75 = 80 × (1 −
  25/64); 18.75 = 80 × (1 − 49/64)), with d the horizontal distance to the
  actor's feet (with the middle of the body 60 up, 40 would give 75.95).
- **Armor**: 42.549 / 48.75 = 1 − 0.1272: our blows' reduction for armor 56
  in two pieces (56 × `fArmorScalingFactor` 0.12 + 2 × `fArmorBaseFactor`
  3 = 12.72 %). Explosion damage goes through armor as a blow's.
- **`OnHit`**: a hurt actor hears the `Unarmed` weapon and the explosion;
  one not hurt (Shade) hears only the explosion. No aggressor for a placed
  explosion.

## Open

- **Shade**: nobody was hurt, the NPC in the open included, though all
  heard it. The likeliest reading is that the explosion's line of sight from
  a point on the floor was blocked for all three; it settles that `OnHit`
  doesn't need line of sight, not how shading works. A second round should
  set the explosion off above the floor.
- **Knockdowns**: inconclusive. Nobody moved, not even for "Always": the NPCs
  were held with `SetDontMove`, which may keep them standing. A second round
  should use NPCs left free.
- **3D loaded**: SE reports the explosion's reference's 3D loaded; ours
  says not.
