# Objects' behaviour graphs

Implemented in `src/world/animated.rs` (running them), `src/world/loader.rs`
(splitting the moving nodes out), `crates/nif` (`BSBehaviorGraphExtraData`),
`src/anim_events.rs` (the events they raise, for scripts).
1199 vanilla meshes carry the extra data (actors' skeletons among them).

## What is known

- `BSBehaviorGraphExtraData`: a name, the behaviour project file relative to
  `meshes` (`Traps\RiggedRockFall\RiggedRockFall01.hkx`), and whether it
  controls the base skeleton (set on 4 of the 1199 vanilla meshes: the
  Argonian and Khajiit tails, `AuxBones\Tail\TailProject.hkx`, whose five
  `TailBone` bones are in the beast skeleton too; every other one, actors'
  skeletons included, is 0). The project has the same layout as an actor's:
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

- Clip generators' modes (`hkbClipGenerator.mode`, byte at 0x72): 0 single
  play, 1 looping, 2 user controlled (the time is `userControlledTimeFraction`
  of the length, 0x6C, or the variable bound to it), 3 ping-pong. A
  sarcophagus lid's start state holds its trigger clip user controlled at 0.

## Choices made without a source

- The nodes drawn apart and moved are those with a transform controller of
  their own in a model with a graph; the controllers' own keys aren't played.
- Collision on those nodes is moved as fixed colliders, set in place each
  frame (nothing is pushed by them; bodies resting on them fall when they
  move away).
- Graphs are kept as they were when their cell unloads and run on from there
  when it loads again (a broken rig stays broken); time doesn't pass for them
  meanwhile. References' scripts stay attached across unloads too, keeping
  their variables and state (`OnInit` runs once; `OnLoad` / `OnCellAttach`
  again). `OnCellLoad` goes to every scripted reference of a cell each time it
  loads, after all of them have their scripts (so a puzzle pillar's, which
  counts itself into its lever's script, finds that script attached).
  Unconfirmed: whether the game sends it again on a reload (the pillars guard
  with a `doOnce`; `barredDoor`, `defaultEnableEncLinkedRef` and about 90
  other vanilla scripts have the event) and how it orders it against
  `OnLoad`.
- `GetOpenState` (Papyrus and condition 157, CommonLibSSE's
  `BGSOpenCloseForm::OPEN_STATE`: 0 none, 1 open, 2 opening, 3 closed, 4
  closing): animated doors from their `Open` / `Close` sequences; objects
  with a graph from its events, `Open` / `Close` taken putting them opening /
  closing and the `Opening` / `Closing` they raise when done (the large
  Nordic portcullis's graph: states `Closed` -> `Open` -> `Opened` and
  back) open / closed, `Opened` / `Closed` sent snapping there; other doors,
  activators and containers closed (a door the player left open, open),
  anything else none. Unconfirmed: CommonLibSSE gives activators the
  open/close form too but not how the game decides their state. Kept in
  memory with the graphs, not in saves (neither are the graphs).
- `GetOpenState` answers at the end of the frame it is called in, once the
  scripts it set going have run (the caller resumes the next frame). The
  pillar puzzles' lever (`defaultPillarPuzzleLever`) activates its gate and
  asks the gate's `GetOpenState` at once; only opening or open sets its
  `doorOpened`, which lets a pillar turned off its answer later close the
  gate again. In SSE (Bleak Falls Barrow, tested by hand) the gate does
  close on such a turn, and solving again and pulling the lever reopens it,
  so the game's answer comes after the gate's `OnActivate` has begun
  opening it. Answering at once, the lever saw the gate closed (its
  `OnActivate` runs a pass later). Unconfirmed: how long the game's wait
  is, and whether other natives are held the same way.
- `PlayAnimationAndWait` gives up after 10 s (`WAIT_LIMIT`) when its event
  doesn't come; the game's wait has no limit, but a graph event missing here
  would otherwise hang the script for good.
- `RegisterForAnimationEvent` fails (false, nothing registered) when the
  sender runs no graph (not loaded, or without one: a placed coin, enabled or
  disabled, gave false in the game, `test-mod autotest2`); registrations are
  kept until unregistered, also across the sender unloading (unchecked).
- Graph events are matched to registrations and waits ignoring case (the blade
  raises `Reset`, its script waits for `reset`). In the game a registration
  for `WEAPONDRAW` got the player's `weaponDraw` (`test-mod autotest2`).
- Script-set graph variables (`SetAnimationVariable*`) hold against the
  engine's own writes (an actor's `Speed`, `iState`, `bHeadTracking`,
  `IsBlocking`...) while the engine keeps writing the value it had before the
  script's; once the engine's own value changes (another weapon equipped, a
  new movement state) the engine's wins again. Unconfirmed: public sources
  don't say whether the game's own per-frame writes win over a script's, or
  when it stops a held value. Chosen so a script's value sticks without
  freezing the variable for good: `DLC2BookDungeonControllerScript` sets the
  player's `iLeftHandType` / `iRightHandType` to 0, which would otherwise
  outlast every later change of weapons. Continuously varying values (`Speed`
  while moving) go back to the engine's within a frame. A graph's own
  modifiers (expression and variable outputs) still write them.
- A graph that controls the base skeleton (the tails') runs beside the actor's
  own and poses only its skeleton's bones (`TailBone01..05`) over the actor
  graph's. Unconfirmed: how the game feeds it. We copy every variable it shares
  by name from the actor's graph each update and pass it the actor's events
  and the events the actor's graph raises (`tailMTLocomotion`, `tailSprint`,
  `tailCombatIdle`... are among 39 `tail*` events in the character graphs; the
  aux graph's other triggers, `staggerStart` and the jumps, are events the AI
  sends the actor's graph). It is set up when the actor spawns from the first
  worn model with such a graph, and not again when it changes what it wears.
