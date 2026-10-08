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

## Choices made without a source

- "Touching" is within 2 units (`TOUCH_MARGIN`); sleeping bodies don't hit.
- What the player holds doesn't hit the player.
- Which objects are traps: those whose scripts (or the scripts they extend)
  define any of the three events. The engine's own rule isn't public.

## Open

- `ProcessTrapHit`'s pushback isn't applied (actors have no body to push),
  and it sends no `OnHit`.
- Traps moved by behaviour graphs (swinging maces and blades, rigged
  rockfalls' supports, dart and flame traps' parts) don't move: objects don't
  run graphs, so `PlayAnimation` does nothing. Ansilvund's rockfall fires
  but its rocks stay on the rig's unbroken supports.
- NIF animated collision (keyframed controllers) doesn't hit.
- Disarming (`trapDisarmed`), trip wires' own scripts (`Tripwire` makes its
  wire dynamic) and the Light Foot perk (`HasPerk` is unimplemented).
- Who gets a trap kill: no one.
