# UseMagic packages (casting practice)

Implemented in `src/ai/magic_practice.rs` (the cast), `src/casting.rs`
(`practice_spell`, the spell put in the hands, the payment) and `src/ai/package.rs`
(the template's inputs). No public source describing the UseMagic template was
reachable (the Creation Kit wiki is blocked), so everything below is read from the
input names and the 15 packages that use it in the game's data (`vrm-tool
pack-templates <data>` with `SHOW=UseMagic`, `pack-tree <data> <package>`).

## What is known

- The template `UseMagic` (Skyrim.esm `000504F5`) has the inputs `Location`,
  `Spell` (a `TargetSelector`), `Target` (a `SingleRef`), `HoldWhenBlocked`,
  `CastTimeMin` / `CastTimeMax`, `CooldownTimeMin` / `CooldownTimeMax`,
  `NumToCastMin` / `NumToCastMax` and `DualCast`, and one procedure, `UseMagic`.
- Its users: the College's practice (Onmund's Flames, Frostbite, Sparks and
  Turn Undead against a practice target; Colette's buff, heal, detect and circle
  spells on herself), Neloth's and Talvas's candlelight, calm and heal, the
  Shimmermist Falmer boss's cast, a Labyrinthian thrall's channelled spell
  (cast time 100000, cast count 10000000) and a Creation Club boss's. The target
  is a reference (type 0) or the actor itself (type 6); the spell is always a
  specific form (type 1).

- J'zargo's package uses a second template, `UseMagicRepeat`, with the same
  procedure; it is read the same.
- Run on the game's data (`vibe_rm --cell WinterholdCollegeHallofTheElements
  --hour 13`): Onmund walks to his duel marker and casts Flames, Frostbite,
  Sparks and Turn Undead at the marker he is given, one package after another
  (the packages choose by `GetRandomPercent` and a quest variable), and his
  graph goes through its casting state; Colette casts Oakflesh and Detect Life
  on herself.

## Choices made without a source

- **Cast time** is how long a concentration spell is held (Onmund's Flames:
  2 to 3 seconds). A fire-and-forget spell ignores it and is let go after its
  charge time, as a caster's in a fight.
- **Cooldown** is the time between casts, picked from the range after each.
- **Number to cast** is how many casts the package is run for before it counts as
  done (then it is skipped while it still applies, as a finished UseWeapon
  package). Onmund's 1 to 1000 therefore keeps him casting for the package's whole
  schedule, the Falmer boss's 1 to 1 casts once. The thrall's bounds are the
  wrong way round (10000000 and 0): the lower is taken as the least (1).
- **Hold when blocked**: waits while a ray from the caster's eye to the target's
  bounds centre hits something other than the target more than 100 units short
  of it (the slack because the ray ends inside the target's own model).
  Unchecked, the caster fires whatever is in the way.
- **Dual cast** casts with both hands when the package says so and neither hand
  holds a one-handed weapon or staff; the dual cast perk isn't needed. The
  magicka cost is the fight's (`fMagicDualCastingCostMult`).
- The spell is paid for in magicka like a caster's in a fight, and the actor
  need not know it. Delivery: on the caster, aimed, or at an actor; touch and
  at-a-location spells aren't cast. A hostile spell cast at a practice target
  (not an actor) is aimed at its bounds centre; what the projectile does to a
  static target is the projectile's.
- Each package (Onmund has four, picked by chance) counts its casts afresh.
- The actor draws its hands (`WeapEquip`) at its place and puts them away
  when the package ends. A package's actor that is fighting does neither.
- Concentration spells' magicka doesn't come back while they are held, as in a fight.
