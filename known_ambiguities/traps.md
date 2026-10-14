# Traps

Implemented in `src/traps.rs` (hits), `src/activation.rs` (activate parents),
`src/triggers.rs` (phantom triggers) and `src/grab.rs` / `src/loose.rs` (the
bodies). The game's trap logic is in its scripts (`TrapBase`,
`TrapTriggerBase`, `PressurePlate`, `PhysicsTrap`, `TrapHitBase`,
`PhysicsTrapHit`...); the engine supplies triggers, activation, motion types
and hit events. Sources: the CK wiki pages for `OnTrapHit`, `OnTrapHitStart`
and `ProcessTrapHit` (Fallout 4's), UESP for `XAPR` / `XAPD`, and the scripts
themselves.

## What runs

- A pressure plate's phantom is a trigger; stepping on it runs
  `PressurePlate`, which activates itself; activation passes to its activate
  children (a `TrapLinker`), and from the linker to the traps (`PhysicsTrap`
  rocks, made dynamic and pushed down; a rigged rockfall's `break`).
- A loose object whose scripts handle trap events, awake and touching a
  living actor's capsule or the player's, sends `OnTrapHitStart` when the
  touch begins, `OnTrapHit` then and every 0.25 s while it lasts
  (`HIT_INTERVAL`), and `OnTrapHitStop` when it ends. Arguments: the target,
  the trap's velocity at the contact point in m/s (Havok units:
  `PhysicsTrapHit` compares its squared length with
  `damageVelocityThreshold` 6.0, which fits m/s and not game units), the
  contact point in game units, the trap collider's Havok material,
  `abInitialHit` (the first touch of that trap and target ever), and the
  motion type (1 dynamic, 4 keyframed).
- `ProcessTrapHit` takes the damage from the target's health (no attacker:
  no fight, no crime) and staggers by `afStagger`.

## Unconfirmed: choices made

- "Touching" is within 2 units (`TOUCH_MARGIN`); sleeping bodies don't hit,
  nor do graph-moved parts standing still.
- A reference's copy of a script its base also has keeps the base's
  properties it doesn't set (`src/engine.rs`, `attach_cell_scripts`): the
  reference's VMAD holds only what it overrides (Ragnvald's swinging blades
  set `TrapLevel`; their base sets the damage per level and the sounds).
  No source states the rule; the data only makes sense with it.
- What the player holds doesn't hit the player.
- Which objects are traps: those whose scripts (or the scripts they extend)
  define any of the three events. The engine's own rule isn't public.
  Not yet seen in the game: `autotest4`'s scripted bowl, dropped onto and
  pushed at a held NPC, sent no trap event; `autotest5`'s, dropped from
  250 units above one (its script's `OnLoad` ran), came to rest beside it,
  104 units off, again with no event and no harm. The game's falling rock
  (`TrapFallingRockSm01_Rock`) placed 300 units up and given
  `SetMotionType` dynamic didn't fall. A plugin with a copy of that rock
  under our own id crashes the game while it loads plugins, with or
  without a script of ours and its alternate textures (`MODS`).

- Parts moved by objects' behaviour graphs (swinging maces and blades, a
  rig's supports) hit like keyframed traps (motion type 4), with the
  velocity from their last two poses.
- Who gets a trap kill: no one.
