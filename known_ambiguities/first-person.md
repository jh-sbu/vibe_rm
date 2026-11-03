# First-person character choices

Supporting research: [the asset survey](../research/first-person.md) and
[the implementation plan](../plans/first-person.md). These choices are pending
comparison with vanilla Skyrim SE, not deliberate deviations.

- **Armor without MOD4/MOD5.** Use only explicit first-person models, falling
  across sex when necessary. Omit addons with neither, even though
  [UESP ARMA revision 3089062](https://en.uesp.net/w/index.php?oldid=3089062)
  describes MOD2/3 as serving all viewpoints. The survey finds headgear,
  jewelry, feet and creature parts among the 866 omitted addons, but also
  body/hand parts. Drawing those third-person meshes around the eye is not
  justified by the separate arms rig. Exact vanilla suppression is unresolved;
  the original-game probe asks about helmets and sheathed arms.

- **Placement and aim.** The NIF's Camera1st and NPC LookNode origins are at
  model Z=120.5; the camera is not a Havok bone. Align Camera1st to Player::eye
  with race/body scale and view yaw, and rotate the LookNode subtree about its
  local X axis for pitch. The existing eye is 120 units above the feet
  (capsule half-height 42 + radius 22 + EYE_OFFSET 56), close to the rig at
  unit scale. Retain that controller eye height and align the scaled rig to
  it, including crouch, rather than changing collision or save coordinates.
  AimPitchCurrent is degrees, heading zero relative to the view, with no
  additional bow/magic pitch offset. Asset bindings expose current offsets
  as outputs; this does not prove the game's camera steering convention.
  Camera Control and camerafrom variables remain unverified.

- **Notifications.** Both graphs run and receive the same inputs in both
  views. Gameplay and script notifications come only from the visible rig;
  the hidden rig's output is drained while its equipment state follows its
  own draw/sheath events. Existing HitFrame hand deduplication and the shared
  pending bow release prevent duplicate damage/shots across a view switch.
  Public assets establish two graphs and differing trigger times, but not
  which graph vanilla script listeners hear. The probe must settle routing.

- **Projection and depth.** Draw arms after the world with fresh reverse-Z
  depth, near=1 unit and vertical FOV=65 degrees. The STEP INI reference
  documents the number, not its SE vertical conversion or the depth
  algorithm. This keeps the arms from intersecting walls or being clipped
  by the world's five-unit near plane; visual parity is still unmeasured.
