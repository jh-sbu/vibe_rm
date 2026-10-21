# Where unloaded actors appear

Implemented in `src/ai/schedule.rs` (`unloaded_arrival`: where a persistent
actor's package has it while its cell is unloaded) and `src/engine.rs`
(`spawn_actors`: onto the navmesh beneath, `NavWorld::floor_below`, and out
of other actors' way, `clear_of`).

## What is known

From `test-mod autotest6` and `autotest7` in Skyrim SE (2026-10-21, one run
each; their module docs have the readings): actors standing upstairs or on
the ground floor of the Riverwood Trader, their packages sending them to
markers on the ground floor, the trader unloaded.

- The game moves them before the cell loads, along their way at a speed
  (two actors had both gone 112.13 units when first read), within seconds
  of the save loading.
- Travel: on the straight line from where the actor stood to the target,
  height included, stopping 40 units short in XY with radius 0 or 32, and
  10 inside the radius with 128 (118 short). An actor already that near in
  XY doesn't move (25 and 30 off, and one upstairs 200 off with radius 300).
- Sit: on the straight line too, stopping 80 short of the chair in XY; in
  the chair as the cell loads.
- Sandbox near a reference: exactly on the reference, with its heading.
- Sandbox in a whole cell: stays where it stood while unloaded; as the cell
  loads it is at something to do somewhere in the cell (a sweep idle
  marker, the cooking pot), on either floor it started.
- As the cell loads, the others fall straight down onto the floor beneath
  (never the one above), keeping XY.
- Nothing spreads actors sent to one place. Two coming to one point: the
  later took it, the earlier was 10 further along its line; as they loaded
  one was set 38.25 from the other (the same 38.25 in both tests).

## Open questions

- **Upstairs over the target.** Twice, an actor upstairs 86 from its marker
  in XY (radius 128) was put exactly on the marker while unloaded. One 200
  off in XY with radius 300, also upstairs, wasn't moved while unloaded but
  was on the ground floor 164 from its marker as the cell loaded. What
  separates the two isn't known. The engine puts an actor that near in XY
  but farther in 3D on the target (right for the first, near the second's
  end), and one that near in 3D too where it stood (the actors 25 and 30
  off).
- **The stop distance's formula.** 40, and 118 with radius 128, fit "the
  radius less 10, at least 40", which the engine uses; radii between 50
  and 128, and over 300, aren't observed.
- **Other procedures.** Only Travel, Sit and Sandbox were observed. Sit
  ends in the chair, which the engine reaches by putting the actor at the
  chair and seating it on load; others with a location (Sleep, Eat,
  UseWeapon...) are put on their target as Sit is.
- **Coming from another space.** Only journeys within one cell were
  observed. An actor whose package is in another cell or worldspace is put
  on the target (the line from another space means nothing); a whole-cell
  package's actor arriving from elsewhere is put on a random navmesh
  triangle of the cell.
- **The whole-cell sandbox at load.** The game puts it at something to use
  in the cell; the engine leaves it where it stood and its sandboxing
  takes it from there (seating it at once only in furniture near it).
- **Which of two actors moves apart, and which way.** The engine steps the
  one spawned later 38.25 away from the other.
- **Time passing.** Waiting and sleeping (`jump_to_schedules`) put loaded
  actors on their package's target, now without spreading them; whether the
  game moves loaded actors there by the same rule as unloaded ones isn't
  observed.
