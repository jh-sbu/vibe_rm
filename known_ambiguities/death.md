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

## Confirmed in the game (`test-mod autotest22`, 2026-11-01)

- `IsKiller(Actor)` runs on the victim and names the killer: true run on the
  victim with the killer as parameter, false run on the killer with the
  victim. So MS07's uses ask whether the alias was killed by the player.
- `GetCauseofDeath` was -1 for a script `Kill(killer)`.
- `GetTimeDead` is not in game hours: 70–85 s after the death (0.41–0.47
  game hours at timescale 20) it was `>= 1` and not `>= 10`. Real minutes or
  tenths of a game hour fit; the unit stays open (`plans/ROADMAP.md`).

## Unconfirmed

- `GetTimeDead` unit: game hours since death (`Engine::game_hours_total`) is
  still what we return, though the game's isn't game hours (above); kept until
  a follow-up reading tells real minutes from tenths of a game hour. No
  vanilla comparison settles it (see above). Living subjects, and those dead before the game (placed dead, or
  dead in a loaded save), have no value and fail the condition.
- `IsKillerObject(FormList)`: the subject's killer is an actor whose
  reference or base the list names. In the game (`test-mod autotest22`,
  `research/autotest22.md`) it was false run on the killer with a list of
  the victim's base or reference, which rules out "the subject killed one of
  these". The reading as `IsKiller` (run on the victim) is inferred, not
  observed.
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
