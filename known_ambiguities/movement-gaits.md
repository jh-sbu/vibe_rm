# How fast a package's "fast walk" is

`FAST_WALK` in `src/ai/mod.rs` (2026-11-08): a fast walk goes a tenth of the
way from the actor's walk speed to its run speed (it was a quarter, a guess).

## Evidence

- Packages' preferred speed (`PKDT` byte 6: 0 walk, 1 jog, 2 run, 3 fast
  walk) names the gait; movement types (`MOVT`) give only walk and run
  speeds. No public source found gives a fast walk's speed (the UESP dump
  has nothing on it).
- MQ101's cart horses, General Tullius and Hadvar ride fast-walk patrol
  packages. Timed in SE by the user (2026-11-08): from the opening starting
  to fade in until the lead cart stops in Helgen took 3:30 to 3:40. Cart
  horse 1's routes (`Patrol1` to `Patrol5`) are about 28,600 units, so
  about 126 units/s over the ride, turns included; the horses' walk is 125
  and run 450. Tullius, not pulling a cart, keeps the same distance ahead of
  the lead cart the whole way, so the carts aren't slowed by their load.
- At a quarter (206 units/s) the ride was much too fast and the cart
  horses overtook Tullius; at walk speed it was too slow (about 285 s).

## Choices made

- A tenth (about 157 units/s for these horses) fits the measured time with
  the slowing in turns. Fitted to this one ride; humans' fast walk hasn't
  been measured.
