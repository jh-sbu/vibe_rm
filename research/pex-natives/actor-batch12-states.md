# Actor and reference states (batch 12)

Natives: `Actor.IsInKillMove`, `IsBeingRidden`, `IsPlayersLastRiddenHorse`,
`Game.GetPlayersLastRiddenHorse`, `IsAllowedToFly`, `CanFlyHere`,
`ObjectReference.HasEffectKeyword`, `InterruptCast`, `SetActorCause`,
`Actor.StartSneaking`, `UnlockOwnedDoorsInCell`, `SendTrespassAlarm`,
`StartDeferredKill`, `EndDeferredKill`, `ClearArrested`,
`SendVampirismStateChanged`, `SendLycanthropyStateChanged`.

## Sources

The Papyrus Index (papyrus.bellcube.dev, consulted 2026-11-09), descriptions
attributed to the Creation Kit wiki (CC BY-SA 4.0):

- `HasEffectKeyword(akKeyword)`: true when an active effect whose magic effect
  has the keyword is applied, "even when the spell or magic effect's condition
  isn't met".
- `InterruptCast()`: "Interrupts and stops any spell-casting this object might
  be doing."
- `SetActorCause(akActor)`: sets the object's actor cause; None clears it.
- `StartSneaking()`: forces the actor to sneak; on an actor already sneaking it
  cancels the sneak; it fails while falling, swimming or attacking.
- `UnLockOwnedDoorsInCell()`: the actor "will unlock all the doors that he
  qualifies for ownership in his current parentcell".
- `SendTrespassAlarm(akCriminal)`: the actor pretends it caught the criminal
  trespassing, with no "time to go" warning first.
- `StartDeferredKill()` / `EndDeferredKill()`: the actor takes damage but
  can't die until the deferred kill ends.
- `IsAllowedToFly()`: whether the actor is allowed to fly (`SetAllowFlying`).
  `CanFlyHere()`: "Can this actor fly here?"
- `ClearArrested()`: "Clears this actor's arrested state."
- `SendVampirismStateChanged(abIsVampire)` /
  `SendLycanthropyStateChanged(abIsWerewolf)`: tell "anyone who cares"; the
  related events are `OnVampirismStateChanged` / `OnLycanthropyStateChanged`.

## Game-asset observations (`vrm-tool pex-calls`, `pex-events`)

- `SetActorCause` is called by traps (`TrapOilPool`, `TrapExplosiveGas`,
  `TrapFallingOilLampColLayer`) and `DB05FallingStatueActivate`, passing the
  actor who set them off.
- `StartDeferredKill` / `EndDeferredKill` frame the Harkon and Miraak boss
  fights (`DLC1dunHarkonBossBattle`, `DLC2MQ06MiraakBossBattle`).
- `InterruptCast` stops trap casters (`MagicTrap`, `DLC1TrapSpotLight`) and
  boss casters.
- `OnVampirismStateChanged` and `OnLycanthropyStateChanged` are handled by
  `Survival_VampirismLycanthropy` (a reference alias) among others.
- `ClearArrested` is called once, by `DGArrestQuest`, on its criminal alias.

Choices are in `known_ambiguities/remaining-natives.md`, "Natives added
2026-11-09".
