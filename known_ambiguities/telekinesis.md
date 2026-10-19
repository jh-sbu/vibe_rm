# Telekinesis

Implemented in `src/grab.rs` (`telekinesis_started` / `telekinesis_stopped`),
started and stopped by the effect's lifetime in `src/magic.rs`. Sources: the
records (`Telekinesis`: a self-delivered concentration spell, archetype 20,
magnitude 1; the `fMagicTelekinesisBaseDistance` 2100,
`fMagicTelekinesiDistanceMult` 1 and `fMagicTelekinesisDistanceMin` 64
settings, `sMagicTelekinesisNoRecast`, the `MAGAlterationTelekinesisGrab` /
`Throw` sounds), CommonLibSSE-NG's `TelekinesisEffect` (a grabbed object
handle and a move velocity). It uses the same holding as the player's Activate
grab: the object under the crosshair, carried in front of the eye, dropped
when the spell is let go (after the concentration effect's half-second
linger).

Unconfirmed, with what we chose:

- **Reach**: base distance plus the multiplier times the magnitude (2101 at
  the spell's magnitude 1). The settings' names don't say how the three
  combine; any form that multiplies the base by the multiplier gives about
  the same at the shipped values. The minimum is taken as how near the held
  object is kept.
- **Weight**: the held object is carried with Activate's force limit
  (`fZKeyMaxForceWeightHigh`); whether telekinesis lifts heavier things is
  unknown.
- **Throwing** the held object (the grab / throw sounds suggest one) isn't
  built.
- Holding something already when the spell takes hold shows
  `sMagicTelekinesisNoRecast`: taken to be what the setting is for.
- Only the player's casts do it (NPC casters, enchantments and the
  per-object move velocity aren't modelled).
