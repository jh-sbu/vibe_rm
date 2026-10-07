# Trigger volumes

Implemented in `src/triggers.rs`; `vrm-tool triggers <data>` counts primitives,
their bases and scripts.

## What is known

- `XPRM` (UESP): half extents x, y, z, colour, a float, type (1 box, 2 sphere,
  3 portal box, 4 plane). Of 20077 references with a primitive, 5286 have an
  activator base; most of the rest are room bounds and occlusion (statics,
  portal boxes) with no script. Common scripts: `defaultsetStageTrigSCRIPT`
  (453), `WICommentTriggerScript`, `defaultAddMusicSCRIPT`, `WETriggerScript`,
  `defaultStartSceneTrigScript`, enable / disable linked ref triggers.
- Scripted box and sphere volumes send `OnTriggerEnter` / `OnTriggerLeave`
  (with the actor as `akActionRef`) to the reference's scripts and the
  aliases it fills; `GetTriggerObjectCount`.

## Choices made without a source

- Who sets them off: the player and loaded, living actors, tested at three
  heights over their feet with 20 units of reach (a capsule, roughly). Bodies,
  items and projectiles don't.
- Actors already inside when the cell loads enter on the first update.
- Disabled triggers send nothing (and forget who was inside); a script moving
  a trigger doesn't move its volume.

## Open questions

- Collision layers of trigger volumes (`XTRI`, `BGSCollisionLayer`): which
  objects each kind of trigger reacts to.
- Unscripted volumes with other uses: acoustic spaces (`ASPC`), sound markers,
  room bounds and portals (occlusion), water / lava damage volumes.
- `OnTrigger` (sent each frame something is inside) isn't sent.
