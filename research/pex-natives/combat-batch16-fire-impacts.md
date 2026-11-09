# Weapon.Fire, PlayImpactEffect and the pushes

## Sources

The Papyrus Index (papyrus.bellcube.dev, consulted 2026-11-09), descriptions
attributed to the Creation Kit wiki (CC BY-SA 4.0):

- `Weapon.Fire(akSource, akAmmo = None)`: fires the weapon from the source.
  With no ammo given, the source's equipped ammo (a warning if the source
  isn't an actor). No equipped ammo and none given: nothing. Given ammo with
  none equipped: fired without using any. Equipped ammo and a different one
  given: the given one fires, the equipped one is used up. The shot goes the
  way the object faces (an actor's heading).
- `PlayImpactEffect(akImpactEffect, asNodeName = "", afPickDirX = 0,
  afPickDirY = 0, afPickDirZ = -1, afPickLength = 512, abApplyNodeRotation =
  false, abUseNodeLocalRotation = false)`: a pick from the reference (or node)
  along the direction for the length; true when the effect plays.
- `KnockAreaEffect(afMagnitude, afRadius)`: knocks back actors in the radius.
  `PushActorAway(akActorToPush, aiKnockbackForce)`: pushes the actor away as an
  explosion at the reference would (negative pulls).

## Game-asset observations (`vrm-tool pex-calls ... fire`)

Traps (`DartTrap`, `TrapDweBallista`, `DLC1TrapCrossbow`, `SpringTrap`,
`DLC1ArkngthamzBallista`), `defaultFireWeaponScript` and the Civil War's
volleys and catapults fire weapons from markers.

## Choices

- Fire: from an object, at its position along its rotated +Y; from an actor,
  at eye height along its heading; from the player, along their aim. Damage is
  the weapon's base damage plus the ammo's; the shooter is the source (its
  `SetActorCause` actor takes the blame). NPC sources fall back to their best
  or iron arrows, as NPC archers here do; only the player's worn ammo is used
  up.
- PlayImpactEffect: the node's rest transform in the model, as `MoveToNode`;
  "apply node rotation" turns the direction by the node's world rotation, or
  its local one with "use node local rotation". The pick hits the static
  world. Whether SE's pick also hits actors is unknown.
- KnockAreaEffect, PushActorAway and ForceAddRagdollToWorld /
  ForceRemoveRagdollFromWorld need ragdolls for living actors (and getting up
  from them), which don't exist yet: open in `plans/ROADMAP.md`.
