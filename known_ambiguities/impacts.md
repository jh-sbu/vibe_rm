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
- **Two sound references.** IPCT `SNAM` is Sound 1 and `NAM1` Sound 2,
  each optionally referencing SNDR, SOUN or null, according to
  [xEdit's public TES5 definitions](https://github.com/TES5Edit/TES5Edit/blob/dev-4.1.5/Core/wbDefinitionsTES5.pas#L8307).
  Both references are resolved in the impact's plugin; absent, null or
  truncated references are ignored independently. This applies to weapon,
  projectile and footstep impacts.

## Open (choices made here)

- **Sound playback.** Both populated slots are submitted at the impact
  position through the existing sound-descriptor pipeline, including for
  footsteps. This follows the Creation Kit ImpactData description's plural
  impact sounds ([public wiki mirror](https://tesck.ru/index.php/ImpactData))
  and a [mod author's report of two conditional footstep descriptors](https://forums.nexusmods.com/topic/919650-footstep-impact-sounds/).
  The format establishes two slots, but does not establish exact timing,
  duplicate-reference handling, or whether every impact trigger plays both.
  Here they start together, and equal references are submitted twice.
  Physics collisions may use the slots as soft/hard alternatives; that path
  is not implemented or inferred here. The existing sound pipeline's
  descriptor-condition support also limits fidelity for conditional slots.
- **Hazards (`NAM2`).** xEdit names IPCT `NAM2` the impact's hazard
  ([TES5 definitions](https://github.com/TES5Edit/TES5Edit/blob/dev-4.1.5/Core/wbDefinitionsTES5.pas));
  nothing public says how the game places it. Here the hazard (its spell, on
  its interval, within its radius) is left at the impact point when the
  impact lands, lasting its record's lifetime (`DATA` 0x08; 10 s when that is
  not over 0). At its limit (`DATA` 0x00, when over 0) the oldest of its
  record goes. Not done: its model, light, sound and impact data set, and
  its flags (drop to ground, align to the surface, the spell's duration
  or radius); the hazard stays where it landed whatever cell it is in, and
  is not saved.
- **Sound-only records without the no-decal flag.** The existing full
  impact loader requires `DNAM` when `DATA` does not set No Decal Data;
  a missing texture set therefore prevents weapon/projectile sound playback
  too. Footsteps read the sound slots without loading decals. Whether
  unflagged records without `DNAM` should still sound needs confirmation;
  the second-sound change preserves this existing loader behavior.
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
- **Effects** are oriented with their +Z along the orientation (the
  reflection is about the surface normal: for a blow on an actor, back
  towards the attacker), turned at random about it. The blood sprays'
  emitters and the shock disc (surface normal) fit +Z; the shout push's
  dust (`PushDirtDust02`, projectile vector) is a ring about +Y, the way
  projectiles' models point, and the ice shards (`IcicleImpact01`,
  projectile vector) fly out along +Z, into what was struck. Which axis
  each orientation takes is unknown.
- **Effects' clock.** An effect plays from its text keys' `start` (0.83 s
  in for the ice shards; else 0): its meshes are drawn for the impact's
  duration, its particles emit for it and live on (8 s at most in all).
  The `end` key isn't used. All of the model's own controllers run from
  then, active or not: in the vanilla effect models nearly all are inactive
  `APP_INIT` (flags 0x45), which the engine is taken to start with the
  effect; the active ones (0x48, 0x4C) run from then too rather than on the
  application's clock. Manager-controlled ones (sequences') don't run.
- **Parts.** The nodes with transform or visibility controllers of their
  own are drawn apart and posed by them (after the part they lie in);
  addon nodes and billboards among them take their models along. A hidden
  part stops its particles emitting. Particles of world-space systems
  (the sprays') stay where they were emitted as their part moves; others
  move with it.
- **Blood on bodies.** The game builds skinned decal geometry on the
  actor's meshes (its INI's skin decal counts aren't in the data). Here a
  wound is a decal box like the world's: from 10 units in front of where
  the blow lands along the blow, at least 24 units deep, held in the frame
  of the bone whose length passes nearest its middle (the skeleton's
  root, equipment, camera, look-at and pauldron nodes, fingers and toes
  left out) and moved with the pose (and the
  ragdoll) each frame, drawn by the same screen-space pass. So whatever is
  in the box takes it, as the world's decals do: a blade passing through
  it, or a limb swinging into it, shows blood for that moment. Blocked
  blows leave none. At most 8 an actor and 48 in all, the oldest going
  first; they go when the actor unloads.
