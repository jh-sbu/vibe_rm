# Loose objects

Implemented in `src/loose.rs` (bodies following and remembering), with the
bodies in `src/physics/mod.rs` (`add_loose`, `release`, `move_player`'s
pushes, `wake_touching`) and the shapes in `src/physics/shapes.rs`. Sources:
the NIF's `bhkRigidBody` (motion system, mass, friction, restitution, layer),
the Creation Kit wiki for `ApplyHavokImpulse`, `SetMotionType` (motion type
values) and `DropObject`, and how the game's own scripts call them (the weapon
racks set their weapons to `Motion_Keyframed`). The game's Havok behaviour is
otherwise undocumented; rapier stands in for it.

## Choices made without a source

- **Lying still until disturbed.** Placed objects don't settle on load: they
  stay exactly where the plugin puts them (floating ones too) until pushed,
  struck or moved by a script. This follows what players see in the game
  (misplaced clutter floats until touched). The physics engine wakes new bodies
  as their colliders go in, so each body's axes are locked for its first
  steps and it is put to sleep when they are unlocked (`SETTLE_STEPS`). The
  "Don't Havok Settle" reference flag isn't read; with nothing settling it has
  no effect to give.
- **Created objects fall** at once (dropped items, `PlaceAtMe`).
- **Shapes.** Triangle meshes on a moving body become their convex hull (bowls
  are solid). One rigid body per model only: models with several bodies or
  constraints stay fixed.
- **Masses** are the NIF's, as given (a kettle weighs 68, a wooden plate 10).
  The player pushes with 80 kg (`PLAYER_MASS`), arrows with 0.1 kg
  (`ARROW_MASS`); `ApplyHavokImpulse`'s magnitude is taken as kg m/s.
- **Damping** (linear 0.1, angular 0.5) keeps things from rolling forever.
- **Where dropped items appear**: 50 units ahead of the one dropping them, 70
  up (`DROP_AHEAD`, `DROP_HEIGHT`), as one reference for the stack.
- **Falling out of the world**: 4096 units below where it started, it is put
  back (`LOST_DEPTH`).
- **What is remembered**: where a moved object comes to rest (or is when its
  cell unloads), as a scripted move would be.

## Open

- `SetMotionType(Motion_Dynamic)` on an object whose model is fixed (trap
  rubble, scripted collapses) does nothing: its collision is static.
- Arrows striking a loose object fall instead of sticking in it.
- Melee blows, spells, explosions and shouts don't push objects; the player
  can't grab and carry them.
- Dropped items aren't owned or stolen; the Story Manager's remove-item event
  isn't sent.
- Actors don't step around clutter (navmeshes don't know of it); they shove it.
