# Impacts

Implemented in `src/impacts.rs`. Sources: the IPDS / IPCT / MATT layouts
(UESP, xEdit's definitions), the vanilla data (`vrm-tool esp-dump` of the
records named below) and what the game shows. How the engine picks and
places impacts isn't public.

## Settled

- **IPCT `DATA`**: effect duration, orientation (0 surface normal, 1
  projectile vector, 2 projectile reflection), angle threshold, placement
  radius (floats but the orientation), sound level, flags (0x01 no decal
  data), result. `WPNBlade1HandVsFleshImpact`: 0.11 s, reflection, 85
  degrees, 16 units. Its decal is `DNAM`'s texture set sized by the
  impact's own `DODT` (the texture set's when it has none).
- **Which set**: weapons carry their impact data set (`INAM`) and a bash
  one (`BIDS`); shields only `BIDS`. Races carry a material (`NAM4`:
  `MaterialSkin` for the playable races) and an unarmed impact data set
  (`NAM5`: `WPNzUnarmedImpactSet`). Shields and weapons carry the material
  a blow they block meets (`BAMT`). An IPDS lists material -> impact pairs;
  a material missing from it falls back on its parent material (`PNAM`), as
  footsteps do.
- **Effects stop by duration.** The blood sprays' emitters
  (`BSPSysMultiTargetEmitterCtlr`) loop with their visibility on all
  cycle, so the impact's duration must be what ends the emitting.

## Open (choices made here)

- **Where a blow lands**: on the target's side facing the attacker, at
  0.7 of its height (120 units for NPCs, scaled); the game uses the
  weapon's actual contact. Arrows land where they strike.
- **Blood decals**: from a wound, a ray goes on along the blow and down
  (0.6 to 2 times as much down as along, a little to either side) up to
  320 units, and the decal lands where it meets the world (actors ignored),
  within the placement radius of that point, turned at random about the
  surface, projected into it. A blocked blow, or an arrow in the world,
  marks the surface struck. Surfaces met at more than the angle threshold
  from head on take none. How the game throws blood (and whether the
  particle sprays leave decals where they land) is unknown.
- **Limits**: 64 runtime decals across the loaded cells, the oldest going
  first; they don't fade (the INI's decal lifetime and counts aren't in the
  data) and go with their cell when it unloads.
- **Effects** run only their particle systems (the models' meshes and
  keyframe animation aren't drawn), oriented with their +Z along the
  orientation (the reflection is about the surface normal: for a blow on
  an actor, back towards the attacker), turned at random about it, for
  their duration, then until their particles die (8 s at most).
- **Blood on bodies.** The game builds skinned decal geometry on the
  actor's meshes (its INI's skin decal counts aren't in the data). Here a
  wound is a decal box like the world's: from 10 units in front of where
  the blow lands along the blow, at least 24 units deep, held in the frame
  of the bone whose length passes nearest its middle (equipment, camera,
  look-at and pauldron nodes left out) and moved with the pose (and the
  ragdoll) each frame, drawn by the same screen-space pass. So whatever is
  in the box takes it, as the world's decals do: a blade passing through
  it, or a limb swinging into it, shows blood for that moment. Blocked
  blows leave none. At most 8 an actor and 48 in all, the oldest going
  first; they go when the actor unloads.
- **Not done**: hazards (`NAM2`), the second
  sound (`NAM1`), the impact result (bounce, impale, stick), sound levels
  for detection, spells' and explosions' impacts, footsteps' decals.
