# Plan: living actors' ragdolls and getting up

**Status: implemented, SE calibration pending** (2026-11-09).

The census, physics controls, living knock state, graph matching/recovery,
natives/conditions/constraints, paralysis, ragdolled fall damage, native saves,
player handling and vanilla probe generator are implemented. Research:
[research/living-ragdolls.md](../research/living-ragdolls.md). Implementation
policies and approximations:
[known_ambiguities/ragdolls.md](../known_ambiguities/ragdolls.md).

Validation includes physics and synthetic graph tests, GPU engine fixtures,
vanilla VoicePushEffectScript execution, immunity and ward routing, and an
engine offscreen capture of Sinmir recovering automatically in the Bannered
Mare. These are engine checks, not vanilla SE observations. The generated
`tests/assets/VrmRagdolls.esp` and its script await the user's SE run; instructions
and the pending-results record are in
[research/ragdolls-se.md](../research/ragdolls-se.md).

Remaining work without a decision: SE measurements of force/area/timing and
persistence; explode/out knock states; EXPL "By Formula"; and the ATKD trigger
rule (the census found nonzero values on werewolf/werebear sprint attacks, but
no verified application rule). Ordinary airborne NPC falls remain separate.
The original design below records the starting point and intended behavior;
implemented approximations, including keeping bodies until recovery ends and
restarting saved recovery clips from the physical pose, are documented above.

In Skyrim SE a living actor can be knocked off its feet: the last word of
Unrelenting Force, a werewolf's or a shield charge's knockback, a giant's
or a dragon's landing, a collapsing hall, paralysis. Its body goes limp as a
ragdoll, comes to rest, then gets up through its behaviour graph and carries
on. At the start of this work only the dead had ragdolls. The natives that knocked actors down did
nothing, paralysed NPCs kept walking, and `AddHavokBallAndSocketConstraint` on a
living actor failed.

## The evidence

### Public sources

- UESP [Skyrim:Unrelenting Force](https://en.uesp.net/w/index.php?oldid=3603032)
  (revision 3603032): the weak words stagger; "the strongest version will
  throw objects a great distance and turn enemies into a ragdoll, briefly
  rendering them helpless". The table gives Dah "1.5 + ragdoll", and its
  note 2 says "Creatures immune to ragdolling are staggered with a magnitude
  of 1.5". Dragons, dwarven automatons (not spiders), mammoths, atronachs,
  dragon priests and some named NPCs can't be ragdolled. Wards stop a
  draugr's shout from ragdolling the player. Enemies thrown off cliffs take
  fall damage. When the player is ragdolled they can be mistaken for dead,
  and they can jump while getting up.
- UESP [Skyrim:Paralyze (effect)](https://en.uesp.net/w/index.php?oldid=3648133)
  (revision 3648133): "Enemies fall over when paralyzed and take a moment to
  get back up". A one-second paralysis can disable for several seconds. A
  paralysed body takes no fall damage. An NPC getting up can be pickpocketed
  freely (out of combat, unaware).
- UESP [Skyrim:Reanimate](https://en.uesp.net/w/index.php?oldid=3575808)
  (revision 3575808): a reanimated thrall turned into a ragdoll "should reset
  their behavior once they get up".
- The Papyrus Index (papyrus.bellcube.dev; descriptions attributed to the
  Creation Kit wiki, CC BY-SA 4.0), already summarised in
  `research/pex-natives/combat-batch16-fire-impacts.md` and
  `cell-batch17-fog-constraints.md`:
  - `KnockAreaEffect(afMagnitude, afRadius)` knocks back actors in the
    radius.
  - `PushActorAway(akActorToPush, aiKnockbackForce)` pushes the actor away
    as an explosion at the reference would; a negative force pulls.
  - `AddHavokBallAndSocketConstraint` works only on dead actors or
    ragdolling living ones, and a living actor getting up breaks it.
- xEdit `wbDefinitionsTES5.pas` (refs/xedit):
  - `RACE` `DATA` flags `0x00040000` "Allow Ragdoll Collision" and
    `0x00400000` "No Knockdowns".
  - `EXPL` flags "Knock Down - Always" and "Knock Down - By Formula".
  - Attack data (`ATKD`) has a "Knockdown" float beside "Stagger".
  - Actions `AKDN` "Action - Knockdown" and `AAGU` "Action - Get Up".
  - Conditions 107 `GetKnockedState` and 725 `GetKnockedStateEnum`.
- CommonLibSSE-NG (MIT; refs/CommonLibSSE-NG, `include/RE/A/ActorState.h`)
  names the actor's knock states: `kNormal` 0, `kExplode` 1,
  `kExplodeLeadIn` 2, `kOut` 3, `kOutLeadIn` 4, `kQueued` 5, `kGetUp` 6,
  `kDown` 7, `kWaitForTaskQueue` 8. These are community research names and
  values only; when each state is entered and left is not described.
- Skyrim Behavior Editor (MIT; refs/Skyrim-Behavior-Editor-) gives the
  members of `hkbPoseMatchingGenerator`, `hkbGetUpModifier`,
  `hkbPoweredRagdollControlsModifier`, `hkbRigidBodyRagdollControlsModifier`,
  `hkbKeyframeBonesModifier` and `BSRagdollContactListenerModifier`, in
  order. We use them to read the fields from the game's graphs.

### Game assets (examined 2026-11-09)

- **The humanoid graph** (`meshes/actors/character/behaviors/0_master.hkx`,
  `vrm-tool hkb-tree` / `hkb-obj` / `hkb-events`), `Master_Behavior`:
  - Wildcards `* --Ragdoll--> FullyRagdoll` (only `if i1stPerson == 0`)
    and `* --RagdollInstant--> FullyRagdoll`.
  - `FullyRagdoll --GetUpStart--> GetUpFromRagdoll --GetUpExit--> Root`.
    `AnimateToRagdoll` (death animations, `Dismount_ATR`) goes to
    `FullyRagdoll` on `Ragdoll`.
  - In `FullyRagdoll` (`RagdollModList`), the event-driven modifier
    `TurnOffFullRagdoll` keeps `PoweredRagdollNoMatching` on until
    `GetUpBegin` (event 810). `TurnOnMatchingRagdoll` switches on
    `MatchAndSendGetup` at `GetUpBegin`. That list holds a powered-ragdoll
    modifier and `GetUpTimerMod`, a 0.5 s timer that sends `GetUpStart`
    (event 177). So the engine sends `GetUpBegin`, and the graph itself
    sends `GetUpStart` half a second later.
  - Both states play `PoseMatcherSelectorGen`, selected by `iGetUpType`
    (default 0). Its children are, in order:
    - `ReanimatePoseMatcher` (`Reanimate.hkx`, `ReanimateFaceDown.hkx`);
    - `GetUpPoseMatcher` (`MT_GetUpFromFront.hkx`, `Mt_GetUpFromBack.hkx`);
    - `GetUpPoseMatcher_Swimming` (`SwimIdle.hkx`).
    So 0 is reanimating, 1 an ordinary get-up and 2 in water. This is
    inference from the order; step 0 checks it.
  - The get-up clips raise `AddCharacterControllerToWorld` (the back clip at
    0.46 s, the front one at 0 s), then `Getup`, `GetUpEnd` and
    `GetUpExit` near their ends. The swimming one raises
    `SwimStartFromRagdoll`.
  - `GetUpFromRagdollModList` runs `GetUpMod` (`hkbGetUpModifier`: ground
    normal +Z, duration 1.0 s, align with ground 0.25 s) with a
    keyframe-bones modifier and `DriveRagdollRB`
    (`hkbRigidBodyRagdollControlsModifier`: hierarchy gain 0.17, velocity
    gain 0.6, position gain 0.05, snap gain 0.1, blend 0.5 s; first
    reading, step 0 decodes all of them).
  - `PoweredRagdollNoMatching` reads tau 0.8, damping 1.0, proportional
    recovery velocity 2.0, constant 1.0, with a bone-weight array (first
    reading; its max force reads 0 and may be bound to a variable).
  - `AnimToRagdollModList` runs `CollisionListener`
    (`BSRagdollContactListenerModifier`) with its contact event.
- **Creatures:** every creature project has `FullyRagdoll` and
  `GetUpFromRagdoll` except the dragon (`ST_FullyRagdoll`, no get-up) and
  the witchlight (neither): atronachs, bear, dogs, wolves, chaurus, cow,
  deer, draugr, dwarven spider and centurion, falmer, frostbite spider,
  giant, goat, hagraven, horker, horse, ice wraith, mammoth, mudcrab, sabre
  cat, skeever, slaughterfish, troll, werewolf, wisp.
- **Vanilla callers** (`vrm-tool pex-calls`):
  - `PushActorAway` (9): `VoicePushEffectScript`, `werewolfPushEffectScript`,
    `MQ101VoicePushEffectScript`, `SpringTrap`, `DLC2CycloneEffectScript`,
    `DLC2LevitateScript`, `DLC2WhirlwindDamageScript`,
    `dunKatariahScimitarScript`.
  - `KnockAreaEffect` (27): dragon landings (`dragonActorSCRIPT`),
    `giantActorSCRIPT` (0.5 on its stomp's animation event), Alduin,
    Hrothgar's pushback (1.0), collapsing halls, boss triggers (0.5), the
    Throat of the World hazard (1.0); magnitudes run from 0.1 to 1.0.
  - `ForceAddRagdollToWorld` (2): `DefaultRagdollAttachPointSCRIPT` and
    `DefaultWerewolfAttachPointSCRIPT`, on load, before hanging a body's
    hands and feet from attach points. `ForceRemoveRagdollFromWorld`: no
    callers.
- **Unrelenting Force:** `VoiceUnrelentingForce3`'s ragdoll comes from a
  script, not from the stagger. Its effects are:
  - `VoiceUnrelentingForceEffect03`: Stagger archetype, magnitude 1.5,
    scripted, only on targets without keyword `000172AC`
    `ImmuneStrongUnrelentingForce`;
  - `VoiceUnrelentingForceEffect02`: Stagger 1.5 with no script, only on
    targets with that keyword.

  Effect 03's script is `VoicePushEffectScript` with `PushForce` 15, so
  `PushActorAway(Target, 15)`. `DragonVoiceUnrelentingForceEffect` uses 15,
  `werewolfKnockbackEffect` and `shieldChargeKnockbackConcAimed` 10.
- **Game settings** that look relevant (meaning unconfirmed):
  `fKnockbackForceMax` 128, `fKnockbackTime` 0.3, `fKnockbackDamageMult` 5,
  `fKnockdownCurrentHealthThreshold` 50, `fProjectileKnockMultBiped` 0.25,
  `fExplosionForceMultLinear` / `Angular` 1, `fGameplayImpulseMultBiped`
  0.04, `fWaterKnockdownSize*` and `fWaterKnockdownVelocity` 5, the
  `fDeathForce*` family.
- **autotest21:** a corpse pushed with `PushActorAway` 5 moved −9.9 in x
  (*Partial*). No living actor was pushed.

## What exists (2026-11-09)

- `src/world/ragdoll.rs`: `RagdollDesc` (capsule bodies and limited / ball /
  rope joints from the skeleton NIF) and `RagdollPose`, which maps bodies
  back to bones and keeps undriven bones under their parents.
- `src/physics/mod.rs`:
  - `spawn_ragdoll` makes dynamic bodies with joints, lifted out of the
    ground; `ragdoll_bodies` and `remove_ragdoll`. `RagdollHooks` keeps one
    ragdoll's bodies from colliding with each other.
  - `RAGDOLL_GROUP` collides with the world and other ragdolls, not with
    capsules.
  - Actor capsules are kinematic colliders (`add_actor_capsule`,
    `move_actor_capsule`, `slide_actor`), switched off by `set_enabled`.
    NPCs follow navmesh height (`stand_on_ground`) and have no airborne
    movement.
- `kill_actor_lying` (`src/ai/combat.rs`) disables the capsule, spawns the
  ragdoll from the pose with the walking velocity, and stores it on
  `ActorRuntime::ragdoll` (only while `dead`). The AI loop poses dead
  actors from their bodies and moves `exec.pos` to them.
  `WorldState::body_poses` keeps bodies across unloads, and `Resurrect`
  removes the ragdoll (`src/actor_flags.rs`).
- `src/havok_constraints.rs` resolves a dead actor's node to its ragdoll
  body; living actors fail.
- The graph runtime (`crates/havok/src/behavior`) runs `Timer` and
  `EventDriven` modifiers, so `GetUpBegin` → `GetUpStart` would already run.
  It reads `hkbPoseMatchingGenerator` as a plain blender, which would blend
  the front and back get-ups together. It doesn't parse the get-up,
  keyframe-bones, powered or rigid-body ragdoll modifiers (`Other`).
- Stagger (`staggerStart`, `staggerMagnitude`) exists for blows, wards and
  the Stagger archetype (`src/magic.rs`). Paralysis only holds the
  `Paralysis` actor value; nothing reads it for NPCs.
- `falling.rs` handles the player's fall damage only. NPCs' falls wait on
  airborne movement (ROADMAP).

## Design

### The knock state

`ActorRuntime` gains `knock: Option<Knock>`:

```rust
pub(crate) struct Knock {
    pub state: KnockState,           // Down, GetUp (also Queued for one frame)
    pub ragdoll: physics::Ragdoll,   // living bodies; handed to the corpse on death
    pub mapping: RagdollPose,
    pub cause: KnockCause,           // Push, Area, Paralysis, Script, Fall
    pub down_for: f32,               // seconds since it went down
    pub rest_for: f32,               // seconds the bodies have been still
    pub hold_until: Option<f64>,     // paralysis: not before its effect ends
    pub peak_z: f32,                 // highest pelvis point, for fall damage
}
```

`KnockState` uses CommonLibSSE-NG's numbering so that `GetKnockedStateEnum`
can return it directly: `Normal` 0, `Queued` 5, `GetUp` 6, `Down` 7. We have
no evidence for the explode or "out" states (1–4) or `WaitForTaskQueue` (8),
so we don't use them; `known_ambiguities/ragdolls.md` records this.

**Going down** (`Engine::knock_down(actor, impulse, cause)`) happens in this
order:

1. Refuse if the actor is dead, ethereal, in furniture or mounted (step 0
   decides these two), has no `RagdollDesc` or no get-up state in its graph,
   or its race has "No Knockdowns" and the cause isn't paralysis. A refused
   push with magnitude becomes a stagger (UESP's note 2).
2. Spawn the ragdoll from the current pose and the actor's velocity plus the
   impulse, and disable the capsule.
3. Send `Ragdoll` to the graph. The player in first person is switched to
   third person first (the transition's `i1stPerson == 0`).
4. Stop movement, the package's procedure (paused, not ended), attacks,
   casting and voice lines in progress. Combat stays and so does the target.
   The actor keeps its weapons (step 0 checks).

**While down** the actor is posed from the bodies. Bones without a body keep
the graph's pose relative to their parent, so the head and fingers aren't
frozen. `exec.pos` follows the pelvis on the ground. While the graph says
`PoweredRagdollNoMatching` is active, joint motors pull gently toward the
graph's pose with that modifier's tau and damping, scaled by its bone
weights. Paralysis instead holds the pose at the moment it struck (stiff
motors), as an inference from the "falls over" descriptions. Hits, damage,
magic and detection still apply.

**Getting up** happens in this order:

1. Once `down_for ≥` a minimum time, the bodies have been nearly still for a
   while (`rest_for`), and no paralysis holds the actor, the engine sets
   `iGetUpType` (1, or 2 when the pelvis is under water) and sends
   `GetUpBegin`. The thresholds are provisional; step 9 measures them.
2. `MatchAndSendGetup` runs half a second of matching, then the graph sends
   `GetUpStart` and enters `GetUpFromRagdoll`.
3. The pose matcher picks the child whose first frame best matches the
   ragdoll on its root, other, another and pelvis bones (lying face up or
   face down). Its world-from-model rotation sets the actor's heading, and
   the actor's position becomes the pelvis on the ground.
4. `GetUpMod` blends from the last ragdoll pose to the clip over its
   duration (1.0 s), turning the root to the ground over 0.25 s. The bodies
   become kinematic (keyframed to the animated pose, as `DriveRagdollRB` and
   the keyframe-bones modifier ask) and are removed when the blend ends.
5. `AddCharacterControllerToWorld` re-enables the capsule at the new spot,
   sliding it out of anything it overlaps.
6. `GetUpEnd` clears `knock`: AI, package and combat resume, and Havok
   constraints on the actor break (Papyrus Index). `GetUpExit` returns the
   graph to `Root` by itself.

**Dying while down** hands the living ragdoll to the corpse: it is not
spawned again, `RagdollPose` is kept, and the bodies stay limp.
`kill_actor_lying` takes an existing ragdoll when there is one.

**Unloading** works as follows:

- An away actor that was down stands up at once where its pelvis lay.
  `ActorExec` gets no knock state, because nothing away is ragdolled.
- A save of a loaded actor that is down keeps its bodies and knock state
  (`KNCK` chunk), and it loads lying down and then gets up. Whether SE does
  this is unknown (`known_ambiguities/ragdolls.md`).

### The pushes

- **`PushActorAway(target, force)`** knocks `target` down with an impulse
  away from the caller's position (horizontal, plus an upward share), equal
  to `force × K`. A negative force pulls toward the caller. `K` and the
  upward share are provisional until step 9 measures thrown distances for
  forces 5, 10 and 15. Corpses get the same impulse, applied to their
  existing ragdoll, as autotest21 showed.
- **`KnockAreaEffect(magnitude, radius)`**: each living actor within the
  radius of the caller (the player included) is staggered with
  `staggerMagnitude = magnitude`. A magnitude of 1.0 or more knocks down,
  pushing away from the source. This 1.0 threshold is an inference from the
  Stagger archetype's 0..1 range and vanilla's 1.0 calls; step 9 settles it.
  The caller itself is excluded.
- **`ForceAddRagdollToWorld()`**: a dead actor already has its ragdoll, so
  nothing happens. A living actor goes `Down` (`KnockCause::Script`) and
  stays down until `ForceRemoveRagdollFromWorld()` lets it get up. Both are
  provisional; the only vanilla callers act on placed corpses.
- **Constraints:** `havok_constraints::body_of` resolves a living actor's
  node to its living ragdoll's body while it is `Down`. Getting up removes
  every constraint held on its bodies.

### Other sources of knockdowns

- **Paralysis:** an NPC whose `Paralysis` value turns positive goes down
  (`KnockCause::Paralysis`, stiff, ignoring No Knockdowns). The UESP page
  says targets immune to paralysis are only staggered, so immunity is
  handled by the effect, not here. The actor gets up once the value returns
  to 0, and takes no fall damage. This is the largest consumer: every
  paralysis poison, spell and enchantment.
- **Explosions** (`EXPL` "Knock Down - Always / By Formula"): explosions
  aren't implemented. This plan only provides the call they will make,
  `knock_down` with the explosion's force, and records the open "By
  Formula" rule (`fKnockdownCurrentHealthThreshold`?).
- **Attack data's "Knockdown" float:** step 0 surveys vanilla values. If
  they are used (giants, mammoths, power bashes), blows that land with a
  nonzero knockdown knock the target down. That is gated on the survey and
  the step 9 observation.
- **Falls:** fall damage from a ragdoll's landing, using `crate::falling`'s
  formula with the NPC settings (`fJumpFall*NPC`), from the drop between
  `peak_z` and the resting pelvis height. This also covers part of the
  ROADMAP's "falls hurting NPCs" item, but only for ragdolled falls.

### The player

The player goes down through the same `knock_down` path (draugr shouts,
`KnockAreaEffect` at dragon landings, paralysis). The player body's graph
receives `Ragdoll`, after a forced switch to third person when in first
person. Their capsule controller stops, input is ignored except the menu
and, during get-up, jumping (UESP). The camera follows the pelvis. Getting
up moves the player's capsule to the pelvis, then the earlier view comes
back. This is its own step, because the player's capsule, camera and
first-person rig differ from NPCs'.

### Physics

- Living ragdolls use the same collision group as corpses: no collision with
  capsules. "Allow Ragdoll Collision" (race flag) is recorded by step 0. It
  may mean living ragdolls do hit capsules; we choose not to until evidence
  says so.
- Joint motors use rapier's `set_motor_position` on each joint's angular
  axes, with stiffness and damping from the powered modifier's tau and
  damping. Kinematic driving (`RigidBodyType::KinematicPositionBased`) is
  used for the get-up blend.
- A cap on simultaneous living ragdolls (say 16) keeps a dragon landing in a
  crowded city bounded. Beyond it, distant actors stagger instead. This is
  provisional and recorded.

## Steps

### 0. Census and decoding (research, no behaviour change)

- `vrm-tool ragdoll-census <data>`, which lists:
  - races with "No Knockdowns" and "Allow Ragdoll Collision", and whether
    their skeleton has a ragdoll and their project a get-up state;
  - `ATKD` entries with a nonzero knockdown, by race;
  - `EXPL` records with each knockdown flag;
  - `VoicePushEffectScript` / `werewolfPushEffectScript` users and their
    `PushForce`;
  - the vanilla `KnockAreaEffect` calls' constant arguments.
- Extend `hkb-obj` (or the parser, see step 3) to decode the six ragdoll
  classes by the Behavior Editor's member order. Record every instance in
  the humanoid and creature graphs, with variable bindings. Check
  `iGetUpType`'s child order against each creature project.
- Write `research/living-ragdolls.md` with the findings and cite them here.

### 1. Physics: living ragdolls

- `Physics::spawn_ragdoll` takes a `RagdollMode` (`Limp`, `Powered { tau,
  damping }`). Add `apply_ragdoll_impulse(r, impulse, at)`,
  `set_ragdoll_targets(r, local rotations, weights)` (joint motors),
  `keyframe_ragdoll(r, body transforms, dt)` (kinematic), `ragdoll_at_rest(r,
  lin, ang) -> bool`, and `ragdoll_into_corpse(r)` (motors off, bodies
  dynamic).
- Unit tests:
  - a powered one-joint ragdoll returns toward its target;
  - a ragdoll keyframed to a pose reaches it;
  - at-rest detection after settling on a plane;
  - an impulse throws a ragdoll a distance that grows with the force;
  - turning a ragdoll into a corpse keeps its bodies.

### 2. The knock state and going down

- `Knock`, `KnockState`, `KnockCause` in a new `src/knock.rs`, and
  `Engine::knock_down` as designed.
- The AI loop has a branch for knocked actors beside the dead one. It poses
  them from the bodies (`RagdollPose` with the graph's pose for undriven
  bones), follows the pelvis and skips package movement. Combat, detection
  and magic keep updating, but the actor can't attack, cast, talk or
  activate.
- `kill_actor_lying` adopts a living ragdoll.
- Console: `knock [ref] [force]` (push away from the player) and `getup
  [ref]` (send `GetUpBegin` now).
- Engine fixture tests: a knocked NPC's capsule is disabled; its package
  pauses and resumes; killing it while down keeps the same bodies.

### 3. The graph: ragdoll modifiers and pose matching

- Parse `hkbPoseMatchingGenerator` as its own generator (children, bone
  indices, mode, world-from-model rotation). Parse `hkbGetUpModifier`,
  `hkbKeyframeBonesModifier`, `hkbPoweredRagdollControlsModifier`,
  `hkbRigidBodyRagdollControlsModifier` and
  `BSRagdollContactListenerModifier` into `Modifier` variants, not `Other`.
- The runtime doesn't simulate physics. Each frame it reports which ragdoll
  controls are active, with their parameters (`FrameOutput::ragdoll`). It
  takes the ragdoll pose as an input for the pose matcher and the get-up
  blend (`set_ragdoll_pose`).
- The pose matcher chooses its child when activated (at `GetUpStart`) by the
  lowest error between the ragdoll pose and each child's first frame on the
  listed bones. It reports the yaw that aligns them.
- `hkbGetUpModifier` blends the input ragdoll pose into the generated pose
  over `duration`, and the root's up axis into the ground normal over
  `alignWithGroundDuration`.
- Tests: a synthetic face-up and face-down ragdoll pose pick the back and
  front clips in the humanoid graph (`hkb-run`-style fixtures). The get-up
  blend starts at the ragdoll pose and ends at the clip's.

### 4. Getting up

- Rest detection: send `GetUpBegin` with `iGetUpType` (1, or 2 in water).
  While `PoweredRagdollNoMatching` / `MatchAndSendGetup` are active, drive
  motors from the graph's outputs.
- Handle the raised events:
  - `GetUpStart` (state `GetUp`): place `exec.pos` and heading from the
    matcher, then keyframe the bodies for the blend and remove them after.
  - `AddCharacterControllerToWorld`: enable and move the capsule.
  - `GetUpEnd`: clear `knock`, resume the AI, break constraints.
- The water case: `SwimStartFromRagdoll` puts the actor into swimming.
- Offscreen check: `knock` an NPC in Whiterun, burst screenshots through
  the fall and the get-up (`remote-testing` recipes). Then check
  front/back selection and that nothing pops at the hand-over.

### 5. Natives, conditions and constraints

- `PushActorAway`, `KnockAreaEffect`, `ForceAddRagdollToWorld` and
  `ForceRemoveRagdollFromWorld` as designed. `PushActorAway` on a corpse
  pushes its ragdoll.
- `GetKnockedState` (107) and `GetKnockedStateEnum` (725). 725 returns the
  state number. 107 returns 0 / 1 for normal / not normal, which is
  provisional: CommonLibSSE-NG names no separate scale. Vanilla only uses
  725 `== 0`, in the idle roots.
- `havok_constraints` resolves living `Down` actors' bodies; getting up
  removes their constraints.
- Remove these natives from ROADMAP's "Natives left" and update the
  `research/pex-natives` choices.
- Tests:
  - `PushActorAway` on a living synthetic actor knocks it down, and a
    negative force throws it toward the caller;
  - `KnockAreaEffect` staggers below 1.0 and knocks down at 1.0;
  - a No Knockdowns race is staggered;
  - a constraint on a ragdolling living actor holds, then breaks on
    `GetUpEnd`;
  - Unrelenting Force's third word, through its real script, ragdolls an
    NPC and staggers one with `ImmuneStrongUnrelentingForce`.

### 6. Paralysis and other sources

- Paralysis knocks NPCs down stiff and holds them until the value returns to
  0; then they get up. Paralysed NPCs already can't act, so this mostly
  connects existing pieces.
- Attack data knockdown (if step 0 finds vanilla uses) when such a blow
  lands.
- Leave a documented `knock_down` call site for explosions (ROADMAP's
  Destruction / explosions item).
- Note the pickpocket opening while getting up (UESP) in
  `known_ambiguities/ragdolls.md` for the pickpocket code to read
  `GetKnockedStateEnum`. Implement it only if it is cheap.

### 7. Falls

- Track `peak_z`. On coming to rest, apply fall damage from the drop with
  the NPC settings, and none under paralysis. The player's ragdoll landings
  go through the same rule with the player settings.
- Update ROADMAP's "falls hurting NPCs" bullet, which stays open for NPCs
  falling on their feet.

### 8. Unloading and saves

- An unloading cell stands its knocked actors up at their pelvis.
- `KNCK` save chunk: state, cause, timers and body transforms. On load the
  ragdoll is rebuilt from them (as `kill_actor_lying` does for corpses).
  Older saves load without the chunk.
- Save round-trip test.

### 9. The player

- The forced third-person switch, the controller stopped, the camera
  following the pelvis, the player's graph through `FullyRagdoll` /
  `GetUpFromRagdoll`, the capsule placed at `AddCharacterControllerToWorld`
  and the view restored at `GetUpEnd`. Jumping is allowed during get-up
  (UESP).
- The HUD and menus keep working. Saving while down follows step 8.
- Wards block knockdowns from voice effects on the player (UESP). Check
  whether `src/wards.rs` already stops the effect before its script runs.

### 10. SE probe: `test-mod ragdolls`

Build with `vrm-tool test-mod ragdolls`; the user runs it in SE. Results go
to `research/ragdolls-se.md` and the choices above are corrected. Keep each
report page to about 20 lines (split into consecutive tests), and drive the
steps with `set VrmRagRun to <n>` (no `cqf` in vanilla SE).

- **Push:** `PushActorAway` 5, 10, 15, −10 on a standing NPC on flat
  ground. Record its position every 0.25 s until `GetUpEnd`, the final
  distance and direction, and `GetKnockedStateEnum` over time.
- **Timing:** `RegisterForAnimationEvent` on `AddCharacterControllerToWorld`,
  `GetUpEnd`, `Getup` and `GetUpExit`. Record the time from the push to each
  of them.
- **Area:** `KnockAreaEffect` at 0.25, 0.5, 0.75, 0.99, 1.0 and 1.5 with
  NPCs at several distances. For each, record whether it staggered, knocked
  down, or did nothing, and whether the caller was hit.
- **Immunity:** a mammoth or dwarven centurion and a No Knockdowns race NPC
  pushed with 15.
- **Paralysis:** a 1 s and a 5 s paralysis. Record the down time and the
  get-up time.
- **Constraints:** `AddHavokBallAndSocketConstraint` on a living NPC
  standing, then while down, then after it gets up.
- **Scripts:** `ForceAddRagdollToWorld` and `ForceRemoveRagdollFromWorld` on
  a living NPC.
- **Kill while down:** record whether the corpse keeps the pose.
- **Unload / save while down:** record position and state after
  reloading.
- **Weapons:** whether a knocked NPC keeps its drawn weapon.

### 11. Documentation

- ROADMAP: move the done parts to Done. Keep the open questions:
  - the explode / out states;
  - "By Formula";
  - the SE batch's pending readings.
- `known_ambiguities/ragdolls.md` records every provisional choice:
  - the get-up thresholds;
  - the push factor `K` and the area threshold;
  - which states are used, and `GetKnockedState`'s scale;
  - collision with capsules;
  - the stiffness of paralysis;
  - the ragdoll cap;
  - saving while down;
  - `ForceAddRagdollToWorld` on the living.

## Order and dependencies

- Steps 0 → 1 → 2 → 3 → 4 are the core and must go in order.
- Step 5 needs step 4.
- Steps 6–8 can go in any order after step 5.
- Step 9 needs step 4 and the player body.
- Step 10 can be written as soon as step 5 exists, so the user can run it
  while 6–9 are built.

This unblocks:

- ROADMAP's Havok constraints on living actors;
- paralysis for NPCs;
- the knockback effects (Unrelenting Force, werewolves, shield charge, the
  Dragonborn cyclone);
- dragon and giant landings knocking people over;
- part of NPC fall damage;
- later, explosions' knockdowns, and the reanimate path (`iGetUpType` 0)
  when reanimation comes.

## Left out

- Death animations into ragdolls (`AnimateToRagdoll`, `DeathAnim`): deaths
  ragdoll at once now. That is separate work with killmoves.
- Ragdolls with breakable limbs (dismemberment, `BPTD`).
- Creature skins' and armor addons' own ragdolls (the dragon, which never
  gets up, keeps its current handling).

## Open questions

- When SE sends `GetUpBegin`: rest thresholds, a minimum time down, and
  whether health matters (`fKnockdownCurrentHealthThreshold`).
- `PushActorAway`'s force units and upward share. Whether it knocks down
  every time or depends on mass or health.
- `KnockAreaEffect`'s magnitude: stagger only, or a knockdown above a
  threshold.
- What "Allow Ragdoll Collision" and the attack data's "Knockdown" do.
- The explode states (1, 2) and the out states (3, 4): perhaps explosions
  and unconsciousness. No source yet.
