# Navigation choices (AI depth step 12)

Research is in `research/offscreen-navigation.md`. Public NAVI/NAVM layouts
provide topology and geometry, not off-screen scheduling or steering rules.
Autotest13/14 show unreachable-height actors staying put; autotest16 measures
walking and explicit package jumps. The following choices implement the plan
where those sources do not determine details.

- Merge incremental NAVI entries, then search NAVM connections and XTEL door
  pairs by centre-distance cost (zero-heuristic A*, with preferred cost from
  `escort.md`). Refine every leg with the existing local triangle/funnel path.
  NVPP groups remain hints; they do not create nonadjacent connections.
- Walk that geometry at the existing buffered/coarse speeds and cadence.
  Crossing an XTEL edge consumes no fictitious distance between coordinate
  spaces; it unlocks permitted door halves and emits location changes.
  No route means no movement, except the already measured explicit jumps.
  Cached paths are disposable and rebuilt from saved execution positions.
- Actor-specific locked portals reject actors without ownership or a key on
  either side. Animated door obstacles may be crossed by permitted actors;
  normal door opening happens ahead of contact and unlocks the door. A second
  overlapping obstacle still blocks the route. This applies the plan's access
  policy to walking; measured explicit package jumps remain possible even when
  the path cannot be used (see `locks.md` on packages crossing locked doors).
- Local clutter steering looks ahead 90 units, expands dynamic-object bounds
  by 24 units and tries a slow diagonal sidestep (35% forward, 65% sideways),
  staying on a floor within 30 vertical units. With neither side free it waits.
  Objects below 0.2 kg or 12 units high retain ordinary capsule contact rather
  than avoidance. These are chosen steering/push thresholds, not measured SE
  constants. Only loose objects enter this pass; actors use their existing
  spacing and no force is deliberately applied to avoided clutter.

Queued SE probe: compare a traveller's timing and positions on a long bent
route (around terrain and through an interior), with locked/unlocked doors,
and a loaded walk through large loose clutter and tiny dropped objects.
Record both door permissions, route length, strides, contacts and events.
Synthetic tests verify this implementation, not the original game's choices.
