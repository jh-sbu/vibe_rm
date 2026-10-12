# Casting

Implemented in `src/casting.rs` (the player's hands, powers, magicka,
NPCs' combat spells) and `src/ai/casters.rs` (casters keeping their
distance and casting), with spell projectiles flying as arrows do in
`src/ai/archery.rs` and their impacts in `src/impacts.rs`. Sources: UESP's Magic Overview, Magicka and
Spells pages; the records (`SPEL` `SPIT`, `MGEF` `DATA`, `PROJ` `DATA`,
`EXPL`); the game settings in Skyrim.esm.

## Settled

- **Charge times** are the spell's `SPIT` charge time (Firebolt 0.5 s,
  Flames 0); fire-and-forget spells let go sooner aren't cast.
- **Cost** is `crate::spells::spell_cost` (paid when cast; concentration
  spells by the second while held).
- **Magicka comes back** at the race's `MagickaRate` (percent of the most a
  second) times `MagickaRateMult` (base 100, as `SpeedMult`), times
  `fCombatMagickaRegenRateMult` (0.33) while fighting.
- **Projectiles**: speed, gravity and range from the effect's `PROJ`
  (Firebolt: a missile, 2500 units/s, range 10000). Flames are a flame-type
  projectile with a range of 768.
- **Area**: an effect's area is in feet; Fireball's 15 feet match its
  explosion's radius of 320 units (21.3 units a foot).
- **Messages**: `sMagicCastInsufficientMagicka`, `sMagicCastPowerUsed`.
- **Casters' combat styles**: `CSGD` holds the melee and magic score
  multipliers after the offensive, defensive and group offensive ones
  (csWarlock: melee 0.25, magic 2.15; csHumanMeleeLvl1: 1 and 1).
- **NPCs' magicka** doesn't come back while they cast
  (`bRegenNPCMagickaDuringCast` false).

## Open (choices made here)

- **Regeneration delay**: magicka waits `fDamagedMagickaRegenDelay` (0.5 s)
  after being spent, and doesn't come back while a spell is held ready or
  flowing (`bRegenNPCMagickaDuringCast` is false for NPCs; assumed for the
  player too).
- **Starting a spell**: a fire-and-forget spell needs its whole cost to start
  charging and when cast; a concentration spell starts with 1 magicka or
  more and stops when a frame's cost can't be paid. Whether the game checks
  the cost at the press or the release isn't published.
- **Concentration landings**: a concentration spell lands on what it reaches
  every 0.25 s. Its effects (Flames: 8 a second for 1 s) are replaced each
  time, so the damage runs on at the effect's rate; how often the game
  applies them isn't known.
- **Flames and beams** reach at once along the view to their projectile's
  range; nothing flies.
- **Target actor** spells reach the spell's `SPIT` range (4096 without one);
  contact spells 160 units.
- **Target location** spells without a projectile land where the view meets
  the world (within 4096 units), on the actors in their area. Runes aren't
  placed.
- **Area** spells strike every live actor whose body's middle is within the
  radius (the player too when someone else casts it), never the caster; a
  hostile spell delivered on oneself with an area (Battle Cry) lands on the
  others around but not on the caster.
- **Skill XP**: the spell's base cost (`SPIT`) times its school's effect's
  skill usage multiplier, through the skill's `AVSK`; hostile spells give
  it for each actor they land on, the rest when cast; concentration spells a
  quarter of it each landing (a second's worth a second). UESP says the
  base cost decides; whether it is the cost before or after the record's
  auto-calculation isn't clear.
- **Greater powers** come back a game day after they were used (a day of
  game time, not the next day); lesser powers whenever.
- **Who casts in combat**: an actor whose combat style's magic multiplier
  is above its melee one (or equal to it, holding no weapon: a flame
  atronach's are both 1), with a hostile aimed or target actor spell it has
  the magicka for (a second's worth of a concentration spell), fights with
  the costliest of them; otherwise as everyone else. The game weighs every
  weapon and spell by its scores; that isn't public.
- **How casters fight**: they close to 85% of the spell's range (at most
  2000 units, as archers) with a clear line, stand facing the target and
  throw a spell every 1.5 to 3 seconds, or channel a concentration spell
  for 1.5 to 3 seconds, aimed at the middle of the target's body; no charge
  time. They play no casting animation yet (`BeginCastRight` / `CastStop`
  go to the graph, which doesn't take them without its magic state).
- **The player is struck** by flames, beams and target actor spells that
  pass within their capsule (they aren't in the physics world).
- **The Story Manager's spell cast event** (`CAST`) is sent for the player's
  spells with the caster as `R1` and the spell as `F1`; which members the
  game fills isn't published.
