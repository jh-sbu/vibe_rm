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
  Skyrim SE contradicts this (autotest21, `research/autotest21.md`,
  LooseObjects): a plugin bowl placed 150 units above the floor, without
  the "Don't Havok Settle" flag (`REFR` 0x20000000), already lay on the
  floor (z 8.6) when the test began, so it fell when the cell loaded. In
  Skyrim.esm 2% of loose items' references have the flag (724 of 33,570
  `MISC`, 321 of 2,597 `WEAP`). The likely rule is that references settle
  on load unless flagged; the Creation Kit stores settled positions, which
  is why vanilla clutter doesn't visibly drop. Ours still lies still
  (compared 2026-11-07); switching is open in `plans/ROADMAP.md`.
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
- **Angle limits' sign.** Havok's limits are taken as the angle of body A's
  frame turned from B's, the opposite of rapier's (B's from A's), so they
  are negated as joints are made. So read, every joint of the human
  skeleton rests within its limits in the bind pose (the ankles' 28 degrees
  within -34..4), knees and elbows bend the right way and hips let a leg
  swing 45 degrees outwards and 15 across; read as rapier's, none of that
  held and dead bodies' legs crossed. The cone of a ragdoll constraint is
  symmetric either way.
- **Constraint types.** Ball-and-socket constraints and socket chains (rope
  links: pivots in pairs, link `i` to link `i + 1`) are free ball joints; a
  stiff spring is a rope joint, so its bodies can come closer than its length
  (Havok holds them at it).
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
  caught in Forelhost's floor does this). In an exterior it must also be more
  than 256 units below the ground where it is now (`LOST_BELOW_GROUND`, no
  source): MQ101's carts are driven more than 4096 units down the mountain
  road, on the ground all the way (2026-11-08).
- **Cells unloading under them**: one now in another loaded exterior cell (a
  cart driven down the road) goes on in that cell, bodies and all, rather
  than unloading with the cell it was placed in (`hand_over_loose`). The
  vanilla opening needs it; see `cell-ownership.md`. Ones still in their own
  cell, or out past the loaded cells, unload with it as before.
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
  `defaultDisableHavokOnLoad` does when `havokOnZKey` is set, as on Fort
  Dunstad's hanging corpses); fixed ones can't be grabbed. The player's character
  controller passes through what is held. Grabbing owned things isn't a crime.
- **Blows**: the player's melee swing that lands on anything but a living
  actor pushes what moves of it along the view by 4 kg m/s (8 for a power
  attack; `BLOW_PUSH`, no source) and sends it `OnHit` (so
  `defaultDisableHavokOnLoad`'s `havokOnHit` lets it go), corpses included.
- **Corpses** lie where their ragdoll is: a dead actor's position follows its
  first ragdoll body (dragged or fallen), on the lowest of them.

## Open

- Throwing what is held: no public source says the game has a control for
  it (players drop things moving, or use Telekinesis); none is bound.
