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
  are solid).
- **Several bodies.** A model's bodies are joined as its constraints join them
  (`bhkHingeConstraint`, `bhkLimitedHingeConstraint`, `bhkRagdollConstraint`,
  as rapier joints whose bodies don't collide): a hand cart's frame and wheels,
  a sign's fixed bracket, chain rings and board. A model qualifies when every
  body is simulated, or some are and constraints join them; one with fixed and
  unjoined simulated bodies stays fixed. Each body's node is drawn on its own.
  Only the reference's place (its root body's) is remembered: on reload the
  other bodies start where the model puts them.
- **Constraint pivots** are taken in the body's node frame (`bhkRigidBody`) or
  the `bhkRigidBodyT` frame, as the cart's, signs' and breakable boards' data
  fit.
- **Constraint types.** Ball-and-socket constraints and socket chains (rope
  links: pivots in pairs, link `i` to link `i + 1`) are free ball joints; a
  stiff spring is a rope joint, so its bodies can come closer than its length
  (Havok holds them at it). Prismatic and malleable constraints, wrapped or
  not, aren't read (none in the vanilla meshes).
- **Motors** are read (type, forces, tau, damping, target) but not simulated:
  every vanilla limited hinge and ragdoll constraint has none, and Papyrus
  can't drive them. Mapping a position motor's tau / damping / recovery
  velocities onto rapier's stiffness and damping would need a source.
- **Breaking.** A `bhkBreakableConstraint`'s threshold is compared with the
  joint's linear impulse in the last step (locked axes and linear limits),
  taken as kg m/s (Havok's documentation calls it an impulse threshold; which
  impulse the game sums isn't public). Broken joints are removed whether or not
  "remove when broken" is set, and stay broken across loads. The beehive's
  spring (threshold 20) breaks under a hard push; an arrow (`ARROW_MASS`)
  doesn't knock it down, and its hit lands before its script makes it dynamic.
  The breakable boards' joints (threshold 10000) won't break by weight here.
- **`SetMotionType`** leaves the bodies the NIF fixes in place (a beehive's
  mount, a sign's bracket, a trip wire's pegs) as they are.
- **Masses** are the NIF's, as given (a kettle weighs 68, a wooden plate 10,
  a hand cart 70 with 15 for each wheel); pushes on several bodies are shared
  by mass.
  The player pushes with 80 kg (`PLAYER_MASS`), arrows with 0.1 kg
  (`ARROW_MASS`); `ApplyHavokImpulse`'s magnitude is taken as kg m/s.
- **Damping** (linear 0.1, angular 0.5) keeps things from rolling forever.
- **Where dropped items appear**: 50 units ahead of the one dropping them, 70
  up (`DROP_AHEAD`, `DROP_HEIGHT`), as one reference for the stack.
- **Falling out of the world**: 4096 units below where it started, it is put
  back (`LOST_DEPTH`); falling out again, it is held there fixed (a fragment
  caught in Forelhost's floor does this).
- **What is remembered**: where a moved object comes to rest (or is when its
  cell unloads), as a scripted move would be.

- **Grabbing** (`src/grab.rs`): holding Activate (E) for 0.3 s
  (`GRAB_HOLD`) grabs; a shorter press activates. The point under the
  crosshair is held where it was grabbed (at least 50 units ahead,
  `HOLD_NEAREST`), pulled by a velocity-matching impulse each step through
  that point (so things hang and swing from it) with their spin damped,
  carrying the weight of the body and of the bodies joined to it (a whole
  ragdoll). The force is capped at `fZKeyMaxForceWeightHigh` (150) times
  gravity, so heavier things drag; `fZKeyMaxForce` (100) is in the data but
  what it limits isn't public, so it is unused. Only `TESGrabReleaseEvent`
  and the `IsPlayerGrabbedRef` / `GetPlayerGrabbedRef` condition numbers come
  from CommonLibSSE; the rest is chosen. What is held drops (`OnRelease`) when
  it gets 150 units from where it is pulled (`LET_GO`), when a menu or
  conversation opens, the player dies or activation is disabled; it leaves
  the hand at no more than 700 units / s (`DROP_SPEED`). Keyframed bodies are
  grabbed but don't move (their `OnGrab` may make them dynamic, as
  `defaultDisableHavokOnLoad` does when `havokOnZKey` is set: no vanilla
  reference sets it); fixed ones can't be grabbed. The player's character
  controller passes through what is held. Grabbing owned things isn't a crime.
- **Corpses** lie where their ragdoll is: a dead actor's position follows its
  first ragdoll body (dragged or fallen), on the lowest of them.

## Open

- `SetMotionType(Motion_Dynamic)` on an object whose model is fixed (trap
  rubble, scripted collapses) does nothing: its collision is static.
- Arrows striking a loose object fall instead of sticking in it.
- Melee blows, spells, explosions and shouts don't push objects.
- Throwing what is held: no public source says the game has a control for
  it (players drop things moving, or use Telekinesis); none is bound.
- Display cases' lids keep their collision when opened, so what lies in
  them can't be grabbed (Whiterun's barracks).
- Dropped items aren't owned or stolen; the Story Manager's remove-item event
  isn't sent.
- Actors don't step around clutter (navmeshes don't know of it); they shove it.
