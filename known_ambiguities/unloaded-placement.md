# Where unloaded actors appear

Implemented in `src/ai/schedule.rs` (`unloaded_arrival`: where a persistent
actor's package has it while its cell is unloaded) and `src/engine.rs`
(`spawn_actors`: onto the navmesh beneath, `NavWorld::floor_below`, and out
of other actors' way, `clear_of`).

## What is known

From `test-mod autotest6` in Skyrim SE (2026-10-21, one run; its module doc
has the readings): actors standing upstairs in the Riverwood Trader, their
packages sending them to markers on the ground floor, the trader unloaded.

- The game moves them before the cell loads: within seconds of loading the
  save, they were already where they ended up.
- Travel: on the straight line from where the actor stood to the target,
  height included, stopped exactly 40 units short in XY (five actors,
  radius 0, 32 and three sharing a marker). They don't walk the last 40.
- Sandbox near a reference: exactly on the reference, with its heading.
- Sandbox in a whole cell: stays where it stood while unloaded.
- As the cell loads they fall straight down onto the floor beneath (the
  ground floor, never the one above), keeping XY.
- Nothing spreads actors sent to one place: two came to rest 5 units apart;
  as they loaded one was pushed away (to 62 from the other).

## Open questions

- **Starting inside the radius.** A Travel actor 86 units away in XY with
  radius 128 was put exactly on its target (keeping its own heading), not
  40 short nor left where it was. The engine puts any Travel actor starting
  within its radius (in XY) on the target, and one within 40 but outside
  the radius where it stood; neither rule is confirmed beyond that one
  reading.
- **Height in the radius check.** The one inside its radius was 86 away in
  XY but 256 in 3D, so the radius is taken in XY; unconfirmed.
- **Other procedures.** Only Travel and Sandbox were observed. Others with a
  location (Sit, Sleep, Eat, UseWeapon...) are put on their target, as
  Sandbox is, and seated by the AI as before.
- **Coming from another space.** Only journeys within one cell were
  observed. An actor whose package is in another cell or worldspace is put
  on the target (the line from another space means nothing); a whole-cell
  package's actor arriving from elsewhere is put on a random navmesh
  triangle of the cell, as before.
- **The whole-cell sandbox at load.** It stood upstairs while unloaded, yet
  0.2 s after the player came in it was on the ground floor by an idle
  marker: placed there at load, or a move too quick to see. The engine
  leaves it where it stood and lets its sandboxing take it from there.
- **How far apart.** The game's separation of overlapping actors isn't
  measured beyond one push; the engine steps a new actor 40 units (two
  capsule radii) out of each one it lands on, on the same floor.
- **Time passing.** Waiting and sleeping (`jump_to_schedules`) put loaded
  actors on their package's target, now without spreading them; whether the
  game moves loaded actors there by the same rule as unloaded ones isn't
  observed.
