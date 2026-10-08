# Objects' behaviour graphs

Implemented in `src/world/animated.rs` (running them), `src/world/loader.rs`
(splitting the moving nodes out), `crates/nif` (`BSBehaviorGraphExtraData`),
`src/anim_events.rs` (the events they raise, for scripts).
1199 vanilla meshes carry the extra data (actors' skeletons among them).

## What is known

- `BSBehaviorGraphExtraData`: a name, the behaviour project file relative to
  `meshes` (`Traps\RiggedRockFall\RiggedRockFall01.hkx`), and whether it
  controls the base skeleton. The project has the same layout as an actor's:
  character, skeleton (`characterassets/skeleton.hkx`), behaviours and clips.
- The skeleton's bones are the model's nodes: the rigged rockfall's 21 bones
  are `Base01` and the 20 nodes carrying a `NiTransformController`.
- Scripts drive them with `PlayAnimation` events (`Down` / `Up` on pressure
  plates, `break` on a rigged rockfall, `open` / `close` on portcullises).
- The graphs raise events scripts wait for (`PlayAnimationAndWait("Single",
  "reset")` on a swinging blade; `open` / `opening` on a portcullis) or register
  for (`BladeTrapHit`: `Apex`, `reset`), and sounds as `SoundPlay.<SNDR editor
  id>` event names (`SoundPlay.TRPBladeSwingSwing`); actors' graphs give the
  sound as the payload of a `SoundPlay` event instead.

## Choices made without a source

- The nodes drawn apart and moved are those with a transform controller of
  their own in a model with a graph; the controllers' own keys aren't played.
- Collision on those nodes is moved as fixed colliders, set in place each
  frame (nothing is pushed by them; bodies resting on them fall when they
  move away).
- Graphs start in their start state when the cell loads; nothing about them
  is kept across loads (a broken rig is whole again, as its script's state
  is).
- `PlayAnimationAndWait` gives up after 10 s (`WAIT_LIMIT`) when its event
  doesn't come; the game's wait has no limit, but a graph event missing here
  would otherwise hang the script for good.
- `RegisterForAnimationEvent` fails (false, nothing registered) when the
  sender runs no graph (not loaded, or without one); registrations are kept
  until unregistered, also across the sender unloading.
- Graph events are matched to registrations and waits ignoring case (the blade
  raises `Reset`, its script waits for `reset`).

## Open

- `controls base skeleton` isn't read; graph variables scripts set
  (`SetAnimationVariable*`) are ignored.
