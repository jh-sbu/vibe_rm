# Melee reach

Implemented in `src/ai/combat.rs` (`ActorRuntime::reach`, `resolve_swings`,
`chase`) with actors' heights in `src/world/skeleton.rs`. Sources: the game
settings named below, the race and attack records (UESP and xEdit's
definitions of RACE `DATA` and `ATKD`) and the vanilla skeletons' `BSBound`.
How the game tests whether a blow connects in three dimensions isn't public.

## Settled

- **Reach**: `fCombatDistance` (141) times the weapon's reach, or the race's
  unarmed reach (RACE `DATA`); `fCombatBashReach` for bashes. All are scaled
  by the actor's scale. Giants' come to 512.
- **Arc**: each attack's strike angle (`ATKD`), at least 25 degrees either
  side of straight ahead (35 without attack data).
- **Heights**: the top of a skeleton's root `BSBound` (centre plus extent in
  Z): humans 128, giants 269, wolves 79.

## Open (choices made here)

- **From the chest to the body.** A blow reaches from three quarters of the
  attacker's height to the nearest point of the target's body: its feet to its
  height, so a target level with the attacker's chest is a flat distance away.
  The game aims melee with the attacker's pitch, but where that starts and how
  far up and down a swing sweeps aren't public. Three quarters is chest
  height, where a swing starts. Consequences:
  - A wolf can't reach a target up on a ledge, but a giant, with 512 reach,
    still strikes one about 420 units above its chest.
  - Nothing tells low swings from overhead ones: every attack sweeps the same
    height.
- **The 1.3 margin**: a swing still lands up to 1.3 times its reach, because
  the target can move between the swing's start and the frame it strikes. No
  source gives it.
- **Heights without a `BSBound`**: 128, a human's.
- **The player's body** is its capsule (radius 22 plus half-height, doubled),
  not a skeleton.
- **Unreachable targets**: an actor that can't path to its target waits
  where the navmesh comes nearest it, facing it, and swings when the target is
  in reach by distance across the ground. Those swings miss when the height
  puts the target out of reach. In the game, actors that can't reach their
  target are commonly reported to back away and return; that isn't done.
