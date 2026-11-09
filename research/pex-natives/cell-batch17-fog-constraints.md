# Cell fog, Havok constraints and dependent animated objects

## Sources

The Papyrus Index (papyrus.bellcube.dev, consulted 2026-11-09), descriptions
attributed to the Creation Kit wiki (CC BY-SA 4.0):

- `Cell.SetFogColor(near r, g, b, far r, g, b)`: 0–255, clipped; only in
  interiors that aren't sky-lit; no visible effect where a lighting template
  gives the fog colours; in memory only, not saved.
- `Cell.SetFogPlanes(afNear, afFar)`: swapped if given the wrong way round;
  only in non-sky-lit interiors. `SetFogPower` is its kin.
- `Game.AddHavokBallAndSocketConstraint(refA, nodeA, refB, nodeB, offsets A,
  offsets B)`: a ball-and-socket constraint between the two references'
  rigid bodies on those nodes (each a `bhkCollisionObject` with a
  `bhkRigidBody`), offsets in each body's space. Works only on dead actors or
  ragdolling living ones; a living actor getting up breaks it.
  `RemoveHavokConstraints` removes them.
- `AddDependentAnimatedObjectReference(akDependent)`: "sets up a dependent
  animated object"; undocumented beyond that ("can cause dangling pointers").

## Game-asset observations (`vrm-tool pex-calls`)

- Fog: `DBSanctuaryDB10LoadCheck` recolours the Dark Brotherhood sanctuary.
- Constraints: `DefaultRagdollAttachPointSCRIPT` and
  `DefaultWerewolfAttachPointSCRIPT` hang corpses' hands and feet from
  attach points' `AttachDummy`; `hangedManScript` and `hangingKeyattachOnLoad`
  remove them.
- Dependent animated objects: the head-chopping blocks
  (`HeadChopBlockHookupSCRIPT`, executioner and guard) and
  `SpiderWebAnimationsSCRIPT`.

## Choices

- Fog overrides apply in the player's interior when it isn't sky-lit (cell
  `DATA` 0x80), each value only where the cell's own lighting gives it (not
  inherited from its template, `XCLL` inherit flags 0x4, 0x8, 0x10, 0x100).
  Not saved, per the source.
- Constraints: a dead actor's node is the bone its ragdoll body drives; a
  loose object's is the body on that node (else its root body); any other
  placed object's is a fixed point at that node of its model. Offsets are in
  game units. Living actors have no ragdoll here, so constraints on them fail
  (false). Constraints aren't saved: the attach points' scripts make them as
  their cells load.
- Dependent animated objects are only recorded (true when added); what the
  dependency does in SE is unknown.
