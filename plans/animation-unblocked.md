# Animation: the unblocked items

**Status: planned** (2026-10-27). Covers what can be built today from ROADMAP
Next item 1 (Animation), as sorted on 2026-10-27, plus what the state-output
probes (`test-mod autotest17`, `autotest18`) turned up. The items blocked on
evidence (twist modes, selectors' blends and choosers, character properties,
delayed state changes, change notifications) and on other systems (keyframe
bones, combat / first-person engine variables, buckets, DLC2 stone piles) are
out of scope; ROADMAP keeps them.

Rules as for all work here (AGENTS.md): public sources and game assets only, no
engine binaries; questions a plugin can settle go to a test mod the user runs;
choices made without a source go in `known_ambiguities/`, open questions here
or in ROADMAP. `cargo fmt` before each commit; one commit per step or
sub-step as marked.

## What the survey changed

- **Footstep splashes and decals: dropped.** No vanilla footstep impact has a
  usable visual. All 161 footstep impacts (`FST*`, `NPC*Foot*`) that name a
  model name `Effects\ImpactEffects\FootImpactDust01.nif`, which isn't in the
  SE data (`vrm-tool impacts`). None has a decal texture set (`esp-has <Data>
  IPCT TNAM` lists no footstep impact; `FSTWaterPuddleImpact` has `DODT` but
  no `TNAM`). Water splashes come from the water surface's own ripples, which
  is rendering work. ROADMAP's "Footsteps: ... splashes, decals" is reworded
  to say so (step 9).
- **The state-output confirmations unblock no code by themselves.** Nothing
  in the engine reads `i1HMState`, `iBlockState`, `iEquippedItemState` or
  `currentDefaultState`; the graphs' own conditions read them, and the
  runtime already publishes them as SE does. What the probes did surface:
  - In SE the script heard **no** `InitiateWinBegin` or `blockHitStop` from
    the player's graph (`autotest17`). vibe_rm sends scripts every event a
    graph raises (`src/anim_events.rs`). If SE forwards only some events,
    scripts here hear events the game's never do. The probe didn't record
    the registrations' return values, so this needs one more probe (step 0)
    before it becomes a fix (step 8).
  - An unalerted commoner given `DrawWeapon` by Papyrus **didn't draw**
    (`autotest17`; it also had `SetDontMove`). vibe_rm's native draws for any
    actor (`src/script/natives.rs`, `actor.drawweapon`). Probed in step 0,
    fixed in step 8 if confirmed.
  - `Debug.SendAnimationEvent` drives the player's graph without the engine
    taking it back, and polling one variable reads at about 0.12 s. Both are
    harness facts the probes below rely on.

## Order

```
0 probe (autotest19) ─────────────────────────────┐
1 action dispatcher ─┬─ 2 sprint + strafe ─────────┤ (2, 3 use 0's answers)
                     └─ 3 swimming ────────────────┘
4 turning (rotate modifier)       independent
5 foot IK follow-ups              independent
6 look-at: creatures, listeners   independent
7 NPC conversations               after 6 (large; scope decided at its step 7a)
8 probe follow-ups                after 0's results
9 roadmap and docs                last
```

Steps 1, 4, 5 and 6 can start while the user runs step 0. Steps 2 and 3
implement their mechanics first and take their thresholds from step 0's
results when they arrive. Until then they use the placeholders named below,
each recorded as a choice in `known_ambiguities/`.

## 0. Probe: `test-mod autotest19` (fully automatic)

One plugin, built as autotest17 / 18 were (`crates/tools/src/test_mods/`,
dry-run test, pages of 20 lines at most, reports in `Result*` variables, a
save requested). Its pages:

1. **Events to scripts.** Register the player for `weaponDraw` (known to
   arrive, autotest2), `InitiateWinBegin`, `blockHitStop`, `FootLeft`,
   `SoundPlay` and `HitFrame`; log each `RegisterForAnimationEvent` return
   value. Draw, block, send `blockHitStart`, then `attackStart` (a swing with
   `HitFrame`); log what arrives, with times. The test NPC walking a Travel
   package gives `FootLeft` / `FootRight` from a moving actor. Readings: which
   kinds of graph event (clip triggers, notify events, state enter / exit
   events) reach `OnAnimationEvent`.
2. **NPC draw.** Four NPCs, same template, no `SetDontMove`: unalerted;
   `SetAlert(true)` first; a guard (a vanilla guard NPC's template); and one
   in combat with a test target. Each gets `DrawWeapon`; `IsWeaponDrawn` is
   read at 1, 3 and 10 s.
3. **Swimming depth.** A spot by vanilla water with a sloping bed (picked with
   `places.rs` / the terrain heights; the plugin writes `XMarkerHeading`s at
   water depths 40, 60, 80, 100, 120, 140 and 180 under the surface). The
   player, then an NPC, is moved to each. Read 1 s later: `IsSwimming` through
   a test ability whose effect is conditioned on `IsSwimming == 1`
   (`HasMagicEffect`), the actor's Z against the surface (floating height),
   and the graph's `SwimDirection`. The same with the player in Waterbreathing,
   to see whether it changes the state.
4. **Sprinting.** Two test NPCs set to fight, 1500 units apart (as earlier
   combat autotests did). `IsSprinting`, `IsRunning` and the distance are
   sampled every 0.25 s while the attacker closes; a cowardly NPC set to flee
   is sampled the same way.
5. **Strafing.** Two melee NPCs (a dueling combat style, then a flanking one)
   fighting for 20 s: `Direction`, `Speed` and the distance between them
   sampled every 0.25 s. A sustained nonzero `Direction` at fighting distance
   is circling; values near 0.5 mean backing off.

The module doc gets its table of predictions (what each reading would mean for
steps 2, 3 and 8) before it's handed over, as the earlier autotests' did.

**Written** (2026-10-27, `crates/tools/src/test_mods/autotest19.rs`; waiting on
a run in SE). Where it differs from the sketch above:

- Page 1 also registers `FootRight` and `attackStart` (an event the script
  sends in), and counts each event per source rather than logging every
  occurrence. The walker runs a copy of a vanilla Travel package, to near the
  player.
- The combat target on page 2 and the sprint target are held by
  `SetRestrained`. Page 4's coward and page 5's fighters are separate pages
  (Sprint, Flee, Duel, Flank), each under 20 lines. Duel is
  `csHumanMeleeLvl6`, Flank `csMercerFreyMelee` (vanilla's only humanoid
  flanking melee style).
- Papyrus reads take about 0.12 s each, so samples come about 0.6 s apart
  (Sprint, Flee) and a Duel / Flank line (four `Direction` reads) about 2 s.
  The report prints the times.
- Swimming: `places::shore` picks the lake from `LAND` heights and the cell's
  water (Tamriel (-3, -15), surface 600, west of Riverwood). The swim pages
  run last and move the player back at the end. `IsSwimming` is read at 1 and
  3 s, because ability conditions are rechecked about once a second.

## 1. Action trees as the source of AI graph events

Today the AI sends hard-coded events (`moveStart`, `moveStop`, `turnLeft`,
`turnRight`, `turnStop`, `SneakStart`, `SneakStop`, `SprintStop`) from
`Actor::animate_graph` (`src/ai/mod.rs`). The game routes each through an
action (`AACT`) whose idle tree picks the event per graph, under conditions:
`ActionTurnLeft` gives `turnLeft` for most creatures but `SwimLeft` for fish;
`ActionSprintStart` requires not swimming, not casting, not attacking, not
sneaking, not blocking (conditions 185, 632, 672, 286, 569); `ActionSneak`'s
`SneakStart` requires not sneaking and not already in the state.
`IdleIndex::select_for` (`src/ai/idles.rs`) already walks a tree for an
actor's project with conditions (`ActionIdle`, `ActionTalking`,
`ActionActivate`).

- **1a. Dispatcher.** `Engine::action(actor, "ActionX") -> Option<String>`:
  find the action's root, `select_for` with the actor as subject, cached per
  (project, action) where the branch has no conditions. Return the event, or
  `None` when no branch applies (the action is refused). Unit tests on
  synthetic idle trees: a per-project event, a conditioned branch, refusal.
- **1b. Movement through it.** Replace the hard-coded sends with
  `ActionMoveStart` / `ActionMoveStop`, `ActionTurnLeft` / `Right` / `Stop`
  and `ActionSneak`. The humanoid branches give the same events as now
  (`CharacterMoveStart` -> `moveStart`), so humanoids shouldn't change; fish
  now get `SwimLeft`. The fixture tests that drive humanoid and creature
  locomotion (`engine-fixture-tests` memory: run with `--include-ignored`)
  must pass unchanged.
- **1c. The player's body.** `player_body.rs` sends `SprintStart` /
  `SprintStop` / `moveStart` itself (`Told::events`). Route sprint and sneak
  through the dispatcher too, so the conditions refuse a sprint while blocking
  or sneaking (today the sprint is just sent). Keep the order rules the
  comment there documents.
- Commits: 1a, then 1b + 1c.

## 2. NPCs sprinting and strafing

The graph has everything (`iState_NPCSprinting`, the locomotion blend by
`Direction`); the player's body already uses both. What's missing is the AI
asking for them.

- **2a. Direction for NPCs.** Set `Direction` from the angle between the
  actor's heading and its velocity (0 forward, 0.25 right, 0.5 back, 0.75
  left, as the player's body computes it in `player_body.rs`). This
  immediately fixes archers backing off (`src/ai/archery.rs`, the `CSCR`
  fallback), who today walk backwards facing forward. Fixture test: an
  archer backing off reads `Direction` near 0.5.
- **2b. Combat circling and fallback.** Use the combat style's `CSCR`
  (circle, fallback, flank distance, flanking stalk time) and `CSLR` strafe
  multipliers and its dueling / flanking flag (UESP *Mod File Format/CSTY*:
  `python3 scripts/uesp.py read "Skyrim Mod:Mod File Format/CSTY"`). Within
  reach, a dueling fighter steps sideways around its target at a rate scaled
  by the circle multiplier, facing it; flanking fighters move to the
  target's side. The rates come from step 0's page 5; until then a placeholder
  (circle 20% of the time at walk speed) is recorded in
  `known_ambiguities/combat-movement.md` (new). Keep `keep_apart` and navmesh
  limits. Headless test: a dueling pair keeps its distance and its relative
  angle changes; Direction stays consistent with the motion.
- **2c. Sprinting.** `ActionSprintStart` / `ActionSprintStop` through the
  dispatcher, and `iState` from the movement type's sprint state (`MOVT`
  `NPC_Sprinting_MT`, `MNAM` `NPCSprinting`; vanilla also has
  `AIControlledNPC_Sprinting_MT`, whose use 2c's research settles). When: from step 0 page 4 (charging from far, fleeing);
  placeholder: a fighter more than 1000 units from its target, and fleeing
  actors, sprint. Stamina drain uses the existing `fSprintStaminaDrainMult`
  path (`src/ai/combat.rs`, used today for the player). Sprinting power attacks
  (`attackStartSprint`) were left out of the attack data: include them once
  NPCs sprint (`combat.rs` "Attacks for situations not handled yet").
- Commits: 2a; 2b; 2c.

## 3. Swimming

The humanoid root graph's `Swim_State` is entered by `SwimStart` (and
`SwimStartFromRagdoll`) and left by `SwimStop`, with swim idle / locomotion by
`moveStart` / `moveStop` and `SwimDirection`; creatures have their own swim
events (`ActionSwimStateChange`: `swimStart`, `MountedSwimStart`,
`HorkerSwimStart`...). Movement types exist for swimming (`NPC_Swimming_MT`,
`Bear_Swimming_MT`, `Horse_Swim_MT`...). The game settings in the data are
`fActorSwimBreathDamage` 0.08, `fActorSwimBreathMult` 0.2, `fJumpSwimmingMult`
2 (`vrm-tool gmst`); others the engine uses have no record and keep defaults
the data doesn't show.

- **3a. Depth and state.** `water.rs` gives the surface height. An actor whose
  feet are deeper than the swim depth swims (placeholder: `WADE_DEPTH` 100,
  already used by `src/footsteps.rs` to stop footsteps, until step 0 page 3
  says otherwise). Hysteresis of a few units, so an actor at the threshold
  doesn't flicker; recorded as a choice. Condition 185 `IsSwimming` and the
  console's `isswimming` answer from it (`src/condition/mod.rs`; today always
  false).
- **3b. Graph and movement.** Entering / leaving sends
  `ActionSwimStateChange`'s event through the dispatcher; locomotion uses the
  swim movement type's speeds (`world/movement.rs` already skips "swim" names
  when choosing default states: give them their own state). Swimmers hold at
  the floating height (from step 0; placeholder: eyes at the surface) instead
  of standing on the bed: `stand_on_ground` / `find_ground` in `src/ai/mod.rs`
  skip while swimming, and pathing keeps to the navmesh (water navmesh exists
  in vanilla). Weapons: the graph's `swimForceEquip` path sheathes; the AI
  doesn't draw while swimming (`ActionDraw` conditions).
- **3c. The player.** `player.rs` movement in water: no gravity below the
  surface, swim speed from the movement type, the camera above the surface
  (no diving controls in this step; diving needs underwater rendering, noted
  as follow-up). The player's body sends the swim events (step 1c path).
- **3d. Footsteps and foot IK.** Footstep sets' swim group (`world/footsteps.rs`
  stores it; `footsteps.rs` drops steps past `WADE_DEPTH`): swimming plays the
  swim footsteps the graph's events name instead of none. Foot IK already
  turns off through the swim behaviour's `bHumanoidFootIKDisable`; check
  creatures' swim states do the same (`hkb-vars <project> FootIK`).
- **3e. Breath.** Under water (head below the surface) breath runs out and
  health drains; Waterbreathing (`MGEF` archetype / `WaterBreathing` effect)
  stops it. The formula's inputs `fActorSwimBreath*` are in the data, the
  base breath time isn't: research UESP (*Skyrim:Waterbreathing (effect)*,
  *Skyrim:Argonian*) first; if it stays open, record the chosen base in
  `known_ambiguities/water.md` and add it to a later probe. Breath meter in
  the HUD SWF if the HUD exposes it (check the SWF's functions as for the
  other meters). This is the only gameplay part of the step; it can be split
  off if it grows.
- Tests: synthetic water cell (as `water.rs`'s tests): wading below the
  depth, swimming past it, hysteresis; `IsSwimming` in a condition; a fixture
  test entering `Swim_State` on a humanoid graph.
- Commits: 3a; 3b; 3c; 3d; 3e.

## 4. Turning: one owner for the yaw

`Default_RotateModifier` (90 deg/s bound to `TurnDelta`) runs in the standing
equip / unequip states (`1HM_Unequip_ModifierGenerator` and its siblings in
`0_master.hkx`). The AI turns the actor itself and reports that as `TurnDelta`
(`animate_graph`), and `Instance::character_turn` already computes the
modifier's yaw but only furniture uses it (`known_ambiguities/rotate-character.md`).

- When the graph has an active rotate modifier (`character_rotations()`
  non-empty), the AI stops turning the heading directly. It sets `TurnDelta`
  to the turn it wants (clamped to the movement type's rate), and the heading
  follows `character_turn(dt)`. When none is active, nothing changes. This
  removes the double-turn concern in the note, because only one side turns at
  a time.
- The player's body the same way (`player_body.rs` sets `TurnDelta` too).
- Record the choice (the modifier owns the yaw while active; units of
  `TurnDelta` as degrees per second) in `rotate-character.md`, replacing the
  "deliberately not fed" paragraph.
- Test: a fixture with a humanoid drawing while turning towards a target
  reaches the target heading at the modifier's rate and never overshoots.
- Commit: one.

## 5. Foot IK follow-ups

Gains already parsed (`FootIkGains` in `crates/havok/src/behavior/mod.rs`);
placement in `src/world/footik.rs`. Humanoids author foot unlock 1.0,
align world-from-model 0, and no legs in their controls modifier (`hkb-obj
<Data> meshes/actors/character 0_master.hkx hkbFootIkControlsModifier 1`);
horses author align 0.15.

- **5a. Survey.** Extend `vrm-tool hkb-run` / a small `hkb-footik` listing:
  every project's controls modifiers with their non-zero unlock, align, hip
  and knee gains, and legs with `ungroundedEvent`s. The result decides how
  much of 5c is real work, and goes in `foot-ik.md`.
- **5b. Planted-foot locking.** While a foot is planted (the existing
  planted / raised blend says so), pin its world position from the frame it
  planted, so feet don't slide when the body's root motion and the animation
  disagree (turning in place, stopping). The foot unlock gain eases the pin
  off as the animation lifts the foot. That's the gain's name, not a
  documented formula, so it's a recorded choice. Check with an offscreen
  capture burst (`--burst-from`) of an NPC turning in place, before and after.
- **5c. Align world-from-model.** Pitch / roll the body to the plane of the
  feet's ground hits, eased by the gain (horses 0.15), within the
  driver's limits. Humanoids keep 0, so they don't change.
- **5d. Ungrounded events.** Where a leg's ray finds no ground for its whole
  reach, raise its `ungroundedEvent` into the graph once. Only if 5a finds
  vanilla projects that author one; else note "unused in vanilla" and skip.
- Tests: synthetic slope / step scenes as the existing foot IK tests have.
- Commits: 5a; 5b; 5c; 5d.

## 6. Look-at for creatures, and listener idles

- **6a. Creatures.** Creature projects run look-at modifiers that nothing
  binds (`BSLookAtModifier` / `hkbLookAtModifier`); the engine must aim them.
  How many projects, and what enables each, is the step's first survey. Give creatures the same targets as humanoids: the player
  close by, their combat target. `animate_graph` currently filters
  `look_at` to `project().humanoid()`; lift that for projects whose graphs have
  a look-at modifier, setting whatever enables theirs (`hkb-vars <project>
  LookAt` per project). Record the target rules as a choice in
  `known_ambiguities/head-tracking.md` (new).
- **6b. Listener idles.** One vanilla INFO has a listener idle
  (`IdleEatSoup`, `vrm-tool info-idles`). Play `LNAM` on the listener when
  the line starts, as speaker idles (`SNAM`) are. `src/dialogue/mod.rs` already
  reads it (`listener_idle`). Small; also covers scene lines with a listener.
- Commits: 6a; 6b.

## 7. NPCs talking to each other (outside scenes)

Scene dialogue already turns heads (`scene_headtracking`), and barks face the
player. What's missing is the conversations NPCs start among themselves:
idle chatter and hellos aimed at another NPC. Story Manager lists "hellos
between NPCs" under the same gap. This is a dialogue system, not just
animation.

- **7a. Research and scope.** UESP (`python3 scripts/uesp.py search "NPC
  conversation"`, the *Mod File Format/DIAL* and *INFO* pages, talk pages as
  leads): how a conversation partner is chosen (topic conditions with
  `GetIsID` / `GetIsRace` on the target, the run-on target), how often, at what
  range, and how long the exchange runs. Write the findings and the chosen
  scope into this section before any code. If the research leaves the pacing
  open, the pacing is a probe candidate, not something to invent.
- **7b. Implementation** (after 7a): an NPC with an `IDLE` / `HELO` line whose
  conditions pass for a nearby NPC target says it to that NPC. The target may
  answer through the topic's response chain. Both look at each other
  (`look_at` to the other's head), and the talking gestures apply (existing
  `ActionTalking`). Reuse the bark scheduler (one speaker at a time).
- Commits: 7a (plan text only); 7b split as its scope dictates.

## 8. Probe follow-ups (after step 0's results)

- **8a. Events to scripts.** If page 1 shows clip triggers or other kinds of
  event don't reach `OnAnimationEvent`, filter them in `src/anim_events.rs`
  by the kind the runtime raised them as. That needs the raise to carry its
  kind (`Raised` in the havok runtime). `PlayAnimationAndWait` uses the same
  path: keep its matching consistent with the filter. Update
  `known_ambiguities/object-graphs.md`.
- **8b. Papyrus `DrawWeapon`.** If page 2 confirms unalerted actors don't
  draw, gate `actor.drawweapon` on whatever made the others draw (alerted, a
  guard, in combat). Update the console path only if the game's console
  differs, which needs its own observation, so leave it otherwise.
- **8c.** Feed page 3–5 answers into steps 2 and 3: replace the placeholders,
  and remove each settled choice from `known_ambiguities/`.

## 9. Roadmap and docs

- Move each finished step to ROADMAP Done. Reword Next item 1's remaining
  lines: footsteps keep "swimming" only until step 3 is done, and "splashes,
  decals" become a note that vanilla footstep impacts carry no usable visuals.
  Rendering's water ripples are listed under item 8 if not already there.
- Every choice made above goes in `known_ambiguities/` with its evidence and
  remaining uncertainty, and nothing else does.

## Open questions

1. Which graph event kinds reach `OnAnimationEvent` in SE (step 0, page 1).
2. Whether unalerted NPCs draw on Papyrus `DrawWeapon` (step 0, page 2).
3. The swim depth, floating height and Waterbreathing's effect on the state
   (step 0, page 3).
4. When NPCs sprint (step 0, page 4) and how much they circle (page 5).
5. The base breath time under water (step 3e research).
6. How NPC-to-NPC conversations are paced (step 7a).
