# Carry trees and sawmill cycle (2026-11-05)

Examined external vanilla PACK, PEX, NIF and HKX assets using `pack-tree`,
`pex-dump`, `esp-dump`, `nif-dump`, `hkb-vars` and `hkb-events`. No asset was
copied into the repository and no game executable was inspected or run.

## Carrying

- CarryAndDropItem (`00079ABF`) repeats conditional Activate Carry Start,
  Travel Carry End, conditional Activate Carry End, Travel Carry Start.
  Both conditions read the graph variable **isCarryItem**, not iObjectToGrab.
- FaendalCarryFirewood11x2 (`00079ADB`) names its own linked-reference
  keywords for Carry Start/End, one activation and a three-second Wait after
  Carry End input. The shipped Drop tree never consumes that wait input or
  StopMovement: inserting a wait here would change the authored tree.
- CarryAndKeepItem (`00108E3C`) does consume those inputs in its final Wait.
  Its eleven users are fishery/bucket packages; bucket animation details
  remain blocked as the plan already specifies.
- The humanoid wood pickup clip raises AddToInventory and an arm-offset
  event. The pickup's three drawn anim objects survive the one-shot exit.
  isCarryItem becomes true while the carry offset is active. Put-down raises
  RemoveFromInventory, clears the anim objects and makes isCarryItem false.
- CarryFurnitureScript registers against its activating actor's inventory
  animation notifications. CarryActorScript and the package fragments manage
  the carried inventory on package/combat changes. These existing scripts and
  natives require no new hardcoded inventory grants for the carry tree.

## Sawmill

- Riverwood's resource `00071DFC` uses ResourceObjectSawMillScript on base
  ResourceObjectSawmill (`00071C47`). The base model is MillLogPile.nif; its
  BSBehaviorGraphExtraData names MillLogPileAnimating/MillLogPile_Project.hkx.
- The worker's IdleMillLoadStart playback raises MillLogPileLoadStart at its
  beginning. The log pile graph consumes that event to enter MillLogLoadStart.
  At that clip's end it raises MillLogChuteIdle and enters the ready pose.
- ResourceObjectSawMillScript registers for those notifications on itself:
  load start changes ResourceState to busy (1), chute idle to ready (2), and
  MillLogIdleReset to waiting (0). ResourceObjectScript.ChangeState asks its
  Worker actor to evaluate packages.
- SawLeverRiverwoodRef (`0001B752`) uses LeverPushAnimatingMetal (`00074A4E`)
  with TrapLever. The resource names the lever as an activate parent with a
  one-second delay. A parent-triggered activation supplies a non-actor to the
  resource's ready-state OnActivate, which calls WorkResource.
- WorkResource plays MillLogChuteCut and waits for MillLogIdleReset. The object
  graph's cut clip raises that reset at its end. Existing activate-parent,
  PlayAnimationAndWait, registration and package-evaluation natives already
  supply the rest of the chain.

The missing integration was delivery of a furniture user's raised event to
the paired furniture's graph and registered scripts. The policy chosen for
that bridge is in `known_ambiguities/furniture.md`. The runtime fixture runs
the worker's real load graph, paired object graph, resource scripts and delayed
lever activation through a complete cycle, without a game process. A separate
fixture verifies wood graph conditions and the carry tree's ordered endpoints.
