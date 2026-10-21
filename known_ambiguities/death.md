# Death conditions

Implemented in `src/condition/mod.rs` (`GetTimeDead`, `IsKiller`,
`IsKillerObject`) over `WorldState::deaths` (`src/world_state.rs`), which
`Engine::kill_actor_by` fills with the killer and the game hour of each death
in play. Papyrus `Actor.GetKiller` exposes that attribution through the native
dispatcher. Tests: `src/condition/death_tests.rs`.

The local UESP index was searched first (`GetKiller`, all namespaces); it has
no matching page. The public [Papyrus Index entry for Actor.GetKiller](https://papyrus.bellcube.dev/skyrimse/script/actor/function/getkiller/)
reproduces the CK Wiki contract: return the actor that killed this actor, or
`None` if this actor is alive. It also relays a CK Wiki caveat that the player
may be returned when a follower, and possibly a summoned actor, delivers the
kill. This project returns the killer already recorded by
`Engine::kill_actor_by`; follower and summon attribution differences remain
unverified in Skyrim SE.

## What is known

- Records: CommonLibSSE (`refs/CommonLibSSE-NG`) numbers the functions
  (`kGetTimeDead = 361`, `kGetCauseofDeath = 396`, `kIsKiller = 408`,
  `kIsKillerObject = 409`, `kGetKillingBlowLimb = 495`). The actor runtime keeps
  `myKiller` (`Actor.h`, a handle) and `AIProcess::deathTime` (a float at 0x94,
  `AIProcess.h`); neither's unit is documented there.
- UESP (`Skyrim Mod:Function Indices`, revision 3566882) names 408, 409 and 396
  and 495 only by index. No UESP page documents `GetTimeDead`, `IsKiller`,
  `IsKillerObject`, `GetCauseofDeath` or `GetKillingBlowLimb` (searched all
  namespaces).
- Vanilla data (`vrm-tool ctda-uses`, which covers INFO, PACK, IDLE, QUST, PERK,
  MGEF, SPEL, SCEN, FACT, DIAL, MESG, SMQN, SMBN, SMEN, COBJ): no condition calls
  `GetTimeDead`, `GetCauseofDeath`, `IsKillerObject` or `GetKillingBlowLimb`.
  `IsKiller` is called three times, all in INFO (MS07 Vald and Jaree-Ra topics),
  on a quest alias (run on 5) with the player reference as its parameter.

## Unconfirmed

- `GetTimeDead` unit: unconfirmed; game hours since death (`Engine::game_hours_total`)
  chosen. Days or seconds are equally possible. No vanilla comparison settles it
  (see above). Living subjects, and those dead before the game (placed dead, or
  dead in a loaded save), have no value and fail the condition.
- `IsKiller(Actor)` direction: unconfirmed; the subject killed the actor named
  (the parameter is the victim). The reverse (the actor named killed the subject)
  is possible from the name. The three vanilla uses don't settle it from the
  record alone (their dialogue lines don't show which way the test runs).
- `IsKillerObject(FormList)`: unconfirmed; the subject killed an actor whose
  reference or base the list names. Whether the list is of bases only is open.
- `GetCauseofDeath`: unsupported; the meaning of its values is not in any source
  checked (no UESP page, no cause enum in CommonLibSSE).
- `GetKillingBlowLimb`: unsupported; the body part of the killing blow isn't kept
  (the hit's limb values are not recorded and their numbering is unconfirmed).
- `Actor.GetKiller`: implemented from the durable death record. Placed-dead
  actors and deaths from saves predating `DETH` have no recorded killer. Whether
  the game substitutes the player for a follower or summon as the CK Wiki note
  says remains unverified here.
- Deaths are saved (`DETH`); saves from before it load without them. Placed-dead actors (killed quietly on
  cell load, `kill_actor_quietly`) get no record, so they have no death time or
  killer.
