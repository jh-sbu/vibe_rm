# Living ragdolls and recovery

Choices made after the permitted research in
[research/living-ragdolls.md](../research/living-ragdolls.md), 2026-11-09.
These implement functioning recovery; they are not observations of Skyrim SE.
The generated vanilla probe is described in
[research/ragdolls-se.md](../research/ragdolls-se.md).

- **Knock states:** normal 0, queued 5 (until the next physical update), getting
  up 6, down 7, from CommonLibSSE-NG's public enum. Nothing establishes the
  circumstances for 1–4 or 8. `GetKnockedStateEnum` returns that number;
  `GetKnockedState` returns 0/1. The probe measures both condition functions.
- **Recovery:** wait at least 1 second down and 0.5 seconds with every body's
  linear speed <=35 units/s and angular speed <=2 rad/s. These thresholds
  have no SE source. An offscreen Sinmir capture on the Bannered Mare fire
  surround still had limb jitter of 4–12 units/s and 0.5–1.1 rad/s after
  8–18 seconds; a 0.5 rad/s cutoff indefinitely restarted its rest timer. Then set iGetUpType=1 (2 when submerged) and send GetUpBegin;
  the authored graph determines the matching delay and clip events. Creature
  selectors without swimming clamp to their final authored child.
- **iGetUpType:** 0 reanimate, 1 ordinary, 2 swimming is inferred from the
  order of the humanoid selector's children (reanimate, get-up, swimming
  matchers); no source names the values. Automatic recovery and
  ForceRemoveRagdollFromWorld both choose 2 when the first body is under
  water, else 1.
- **Push forces:** velocity change is force times 35 units/s, horizontally away
  from the source plus 0.35 upwards, clamped to +/-128. Negative force reverses
  the horizontal component only: a pull still lifts the body rather than
  driving it into the ground. Coincident source/target uses +Y. All bodies receive the
  same velocity change (impulse proportional to mass). The corpse movement in
  autotest21 establishes that corpses can be pushed, not this force mapping.
- **Area:** magnitude below 1 staggers; >=1 also tries to knock down, using
  magnitude times push-force 10. Radius is a hard 3D distance limit; the caller
  is excluded. Refused knockdowns stagger. No Knockdowns blocks pushes/area,
  but not paralysis; existing spell conditions handle magical immunities.
- **Refused pushes:** a PushActorAway that can't knock its target down (No
  Knockdowns, furniture or mount, no ragdoll or get-up state, or the living
  ragdoll budget is full) staggers it at full magnitude. UESP's Unrelenting
  Force note 2 gives 1.5 for creatures immune to its ragdoll; that is the only
  source, and it is applied to every refused push. The humanoid stagger
  graph blends its clips over staggerMagnitude 0, 0.25, 0.5, 0.75 and 1
  (`staggerbehavior.hkx`), so scripted staggers clamp to 1.0, as the Stagger
  archetype and blows already do.
- **Furniture/mounts:** refuse while in furniture, a passenger, or mounted.
  The UESP shout note describes falling off a dragon, but this implementation
  has no tested dismount-to-ragdoll handover. This remains an approximation,
  not a deliberate scope deviation. Weapons stay attached; active attacks,
  casting and voices stop while combat targets and package runs are retained.
- **Collision and budget:** living bodies use corpse groups, excluding capsules.
  The meaning of Allow Ragdoll Collision is unconfirmed. At most 16 loaded NPCs
  enter living ragdolls at once (the player has a separate slot); area effects
  consider closer actors first. Overflow actors stagger. No SE cap is claimed.
- **Control approximation:** zero authored maxForce remains limp. Powered
  targets use joint-frame scaled-axis rotation, tau times 40 and damping times
  8 in Rapier acceleration motors, capped by maxForce and authored bone weight.
  This is not a measured Havok gain conversion. Paralysis holds the struck pose
  with tau 1, damping 2, force 1000 until the actor value clears. The struck
  pose is the physical pose on the first paralysed frame, so an actor already
  lying from a push stiffens as it lies rather than straining toward its
  standing pose. A new paralysis or push during recovery returns the same
  bodies to simulation.
- **Matching/get-up:** choose the first frame with the lowest position/rotation
  error on the authored matching bones after least-squares yaw alignment.
  Bones the bodies don't carry are left out: the humanoid's matching bones are
  Havok 0/11/10/0 (NPC Root, R Foot, R Calf, NPC Root), and the root, having
  no body, would otherwise stay where the actor stood. Positions are relative
  to the pelvis bone when it is carried, else to the compared bones' centroid;
  with only one bone, its axes give the yaw. The authored world-from-model
  rotation (identity, or 180° about Z on two creature matchers) is not
  applied: under an unconstrained yaw search a rotation about Z only shifts
  the found yaw by the same angle, and how Havok uses it is undocumented. All
  of a graph's matchers share one playing/matching flag, set by any
  matcher's start-playing event and cleared by any start-matching event;
  vanilla's matchers share theirs (GetUpStart / Ragdoll). Matching holds that frame; the play event advances only the
  selected clip. Local transforms blend for the authored get-up duration;
  the root aligns over its authored shorter duration. Rigid/keyframe controls
  are decoded; recovery drives all bodies kinematically rather than claiming
  an exact reconstruction of Havok's rigid-body controller gains. Bodies remain
  owned until recovery ends, permitting death/save handoffs from that pose.
- **Ground/controller:** use static ground below the body and resolve capsule
  overlaps by contact normals on controller insertion. Root motion of ordinary
  walking is still paused until recovery ends; the camera follows the player's
  first body and restores its earlier view. Jump is allowed once the recovery
  graph has reintroduced the controller.
- **Falls:** from peak to resting first-body height, through the existing
  fall-damage formula and perk entry. Each landing (0.5 s at rest) counts the
  drop since the previous one, so a body that rests on a ledge and slides off
  it is hurt again; a push mid-fall keeps the height already fallen. Water, ethereal state and any
  paralysis during the knock prevent it. UESP documents delayed damage just
  before recovery; the precise SE threshold remains unmeasured.
- **ForceAdd/Remove:** dead actors are unchanged. Living ForceAdd starts/holds
  a scripted knock; ForceRemove releases that hold even during paralysis,
  requesting recovery immediately only when paralysis permits it.
  Vanilla callers only attach placed corpses, so living semantics need probing.
- **Persistence:** unloading stands living actors up at their current position.
  Native saves retain transforms, velocities, timers, cause, state and view in
  KNCK. A saved recovery restarts matching/get-up from its saved pose rather
  than serializing graph internals/clip phase. The original game's persistence
  rule is unknown. Old saves omit KNCK and have no knocked actors. A saved
  actor whose cell hasn't loaded yet waits for it; the entry is used when that
  cell loads with the actor alive, and dropped when it loads with the actor
  dead, when the actor is then more than 256 units (horizontally) from its
  saved first body (moved meanwhile), or when everything unloads.

The pickpocket opening follows
[UESP Paralyze revision 3648133](https://en.uesp.net/w/index.php?oldid=3648133):
100% chance while getting up if out of combat and unaware of the player. Existing
item/perk restrictions still apply. Whether all non-paralysis get-ups share that
opening is unverified.
