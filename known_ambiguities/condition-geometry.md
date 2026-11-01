# Condition geometry (positions, angles, distances, line of sight)

Implemented in `src/condition/mod.rs` (`function_value`) and `Engine::line_of_sight`
in `src/detection.rs`; tests in `src/condition/geometry_tests.rs`.

## What is known

- Axis parameters (`GetPos`, `GetAngle`, `GetStartingPos`, `GetStartingAngle`,
  `GetRelativeAngle`) are the ASCII character of the axis: 'X' 88, 'Y' 89, 'Z' 90.
  Vanilla uses 90 throughout. Sources: UESP Oblivion Mod:Mod File Format/SCPT
  (oldid 3089174, "An axis value (setpos/getpos) is simply output as a single
  uppercase character"); UESP Skyrim Mod:Mod File Format/Raw Function Data
  (oldid 3411473) for the function signatures.
- Positions and angles of a loaded or moved reference come from the same state
  Papyrus `GetPositionX` and `GetAngleZ` read (`ref_position`, `ref_rotation`).
  Angles are degrees, as `GetAngle` gives them in Morrowind (UESP Morrowind
  Mod:GetAngle, oldid 3109667: "orientation (in degrees)").
- `GetHeadingAngle(ref)` is the bearing from the subject to the reference less
  the subject's yaw, wrapped into [-180, 180). CommonLibSSE-NG
  `TESObjectREFR::GetHeadingAngle` (`src/RE/T/TESObjectREFR.cpp`) computes it as
  `atan2(dx, dy) - angleZ` with the wrap; this is the reading used.
- `GetStartingPos` and `GetStartingAngle` on placed references read the editor
  placement (the record's `DATA`), not the current one. For references created
  in the world during play, the implementation captures their actual position
  and rotation at creation, retains that origin through movement, and saves it
  in the optional `ORIG` section. Inventory-created references have no world
  origin and remain unsupported.

## Confirmed in the game (`test-mod autotest22`, 2026-11-01)

- The condition `GetHeadingAngle` gives the native's value (90, 180, -90, 0
  for a reference east, south, west and north of a subject facing north).
- `GetWithinDistance(ref, 300)` holds at 300 and at 300 straight up: `<=`,
  in 3D.
- `IsCloserToAThanB` is false at a tie and true when strictly nearer.
- `GetLineOfSight` from an NPC: true for one 200 ahead, false for one 200
  behind in the open, as `HasLOS`. So an actor's facing counts (below).
- `GetStartingPos` of an actor created by `PlaceActorAtMe` and then moved
  twice is the marker it was created at.

## Created-reference source limits

The local UESP [Skyrim Mod function index, revision 3566882](https://en.uesp.net/w/index.php?oldid=3566882)
lists CTDA functions 10 and 11 as `GetStartingPos` and `GetStartingAngle`.
UESP's [raw function data, revision 3411473](https://en.uesp.net/w/index.php?oldid=3411473)
gives `Ref.GetStartingPos(Axis)` and `Ref.GetStartingAngle(Axis)` signatures.
Neither page defines what those functions return for a reference created in
play, nor what older saves should do when no original transform was stored.
Capturing the transform at the moment a world reference is created is therefore
an explicit implementation inference, not verified Skyrim SE parity. On old
saves without `ORIG`, this implementation leaves the created reference's
starting transform unknown and the condition fails rather than using its current
transform. The `ORIG` extension does not change the existing `WRLD` layout.

## Unconfirmed choices

- A null reference (`0`) in `GetHeadingAngle`, `GetRelativeAngle` and
  `GetLineOfSight` names the subject's combat target (`combat_target`: the
  one it fights, or searches for after losing them), and fails for a subject
  with none. Vanilla combat idles (`GiantCombatIdleStagger`,
  `KillMoveFrontSideRoot`...) pass null and test `GetHeadingAngle` between
  -45 and 45, which suggests the null is the combat target. No public source
  says so; the substitution is inference, and the idle's own context carries
  no target, so an idle chosen before combat starts still fails.
- `GetRelativeAngle` returns the reference's angle less the subject's, in
  [0, 360). Vanilla kill moves test `>= 90` and `<= 270`, symmetric about 180,
  so the sign does not matter there; the wrap does. No source gives the formula.
- References in different interiors or worldspaces are never within
  `GetWithinDistance`.
- In package conditions (flag 0x08 on the first parameter), vanilla
  `GetWithinDistance` second parameters are small integers that resolve as
  package float inputs (for example UseWeaponMultiTarget's conditions); the
  engine reads the distance from that input. Outside packages the second
  parameter is the float itself. Whether flag 0x08 also governs the second
  parameter is inferred from those records, not documented.
- `GetLineOfSight(ref)` is a clear line between eye points (actors at their
  eye height, the player at its eye, others at their position), using the same
  physics ray as detection's visual check, with the target's own geometry
  allowed at the end. From an NPC the target must also be within its view
  cone, detection's 95° either side of its facing (the game's detection saw
  at 90° and not at 135°; behind failed `GetLineOfSight`). The player's
  line ignores facing: untested. Unloaded references have no geometry, so the
  line is clear. The exact eye point and whether it counts the target's body
  are not public.
