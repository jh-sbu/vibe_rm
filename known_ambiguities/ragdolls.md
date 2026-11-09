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
- **Push forces:** velocity change is force times 35 units/s, horizontally away
  from the source plus 0.35 upwards, clamped to +/-128. Negative force reverses
  both components. Coincident source/target uses +Y. All bodies receive the
  same velocity change (impulse proportional to mass). The corpse movement in
  autotest21 establishes that corpses can be pushed, not this force mapping.
- **Area:** magnitude below 1 staggers; >=1 also tries to knock down, using
  magnitude times push-force 10. Radius is a hard 3D distance limit; the caller
  is excluded. Refused knockdowns stagger. No Knockdowns blocks pushes/area,
  but not paralysis; existing spell conditions handle magical immunities.
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
  with tau 1, damping 2, force 1000 until the actor value clears. A new paralysis
  or push during recovery returns the same bodies to simulation.
- **Matching/get-up:** choose the first frame with the lowest pelvis-relative
  position/rotation error on the authored matching bones after least-squares
  yaw alignment. Matching holds that frame; the play event advances only the
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
- **Falls:** from peak to resting first-body height, once per knock, through the
  existing fall-damage formula and perk entry. Water, ethereal state and any
  paralysis during the knock prevent it. UESP documents delayed damage just
  before recovery; the precise SE threshold remains unmeasured.
- **ForceAdd/Remove:** dead actors are unchanged. Living ForceAdd starts/holds
  a scripted knock; ForceRemove requests recovery without bypassing paralysis.
  Vanilla callers only attach placed corpses, so living semantics need probing.
- **Persistence:** unloading stands living actors up at their current position.
  Native saves retain transforms, velocities, timers, cause, state and view in
  KNCK. A saved recovery restarts matching/get-up from its saved pose rather
  than serializing graph internals/clip phase. The original game's persistence
  rule is unknown. Old saves omit KNCK and have no knocked actors.

The pickpocket opening follows
[UESP Paralyze revision 3648133](https://en.uesp.net/w/index.php?oldid=3648133):
100% chance while getting up if out of combat and unaware of the player. Existing
item/perk restrictions still apply. Whether all non-paralysis get-ups share that
opening is unverified.
