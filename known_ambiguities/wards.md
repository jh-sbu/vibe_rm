# Wards

Implemented in `src/wards.rs`. Sources: UESP's Skyrim:Ward (wards charge
for about a second, dual cast ones are whole at once, a damaged ward
"will immediately start regenerating it at a constant rate even if it's
still taking more damage", only the front in "around 90 degrees", broken
at 0 with the caster staggered and no damage from the spell that broke
it; spells, shouts, dragon breath and sprayed poisons blocked, arrows and
blows not), Skyrim:Lesser Ward ("negates up to 40 points of spell damage
or effects"), Skyrim:Restoration (Ward Absorb: 25% of the spell's base
magnitude as magicka), the records (`WardConcSelf0` / `25` / `50`:
archetype 32 on `WardPower`, duration 0, Recover; `ShieldConcSelf` beside
them; the `wardAbsorb` perk's entry point 71 set to 0.25;
`fMagicWardPowerMaxBase` 1.3, `fMagicWardPowerMaxMult` 0;
`DA13SpellbreakerwardEffect`: a value modifier on `WardPower`), the
`WardAbsorbed` / `WardBroken` idles (`IsWardState` 1 / 2), CommonLibSSE's
`AccumulatingValueModifierEffect` (accumulated and maximum magnitude, a
hold timer) and `MagicSystem::WardState` (none, absorb, break), and
`MG01PlayerScript`'s `OnWardHit(akCaster, akSpell, aiStatus)`.

## Choices made without a source

- **Building**: at the effect's magnitude a second (Lesser Ward 40 a
  second: "about a second" to its 40) up to the magnitude times
  `fMagicWardPowerMaxBase` (52). Whether the most is that, the magnitude
  itself, or `fMagicWardPowerMaxBase` is something else (`...MaxMult` is
  0, so whatever it multiplies doesn't show) isn't published. What the
  hold timer holds isn't known; nothing here waits before building again.
- **Dual cast** wards start at their most (2.2 times the magnitude, times
  1.3).
- **The arc**: 45 degrees either side of where the warded actor faces
  (the player: the camera), measured level, to the caster's position, not
  where a projectile or explosion meets them. A caster not in the world
  (unloaded, or a script's) is taken to be in front.
- **A spell's strength** against a ward: the sum of its hostile effects'
  magnitudes, dual casting's power on those not flagged for duration,
  before the caster's perks and the warded actor's resistances; a
  concentration spell a quarter of that each landing (four a second).
  Durations don't count: a spell of 10 a second for 5 s is 10.
- **What wards stop**: spells, shouts, scrolls and staffs (`SPIT` types 0,
  11, 13, 12); not weapon enchantments, poisons, potions or diseases.
  Friendly spells go through with `OnWardHit` status 0.
- **Breaking**: a hostile spell stronger than the ward breaks the cast
  wards and is stopped all the same (UESP: no damage from the spell that
  broke it). The player's hand casting it stops (to be cast again); NPCs
  are staggered (`staggerMagnitude` 0.5, a choice); the player isn't (the
  body has no stagger yet).
- **Stopped hostile spells still count as a blow**: `OnHit` and the fight
  or assault they start, as if they had landed. Whether the game sends
  `OnHit` for a warded spell isn't known.
- **Gear wards** (a constant `WardPower`, Spellbreaker while blocking) add
  to the cast ward's strength but aren't worn down; alone, a spell too
  strong for them goes through, with no `OnWardHit`.
- **Ward Absorb** gives `strength x 0.25` magicka for each absorbed spell
  while a cast ward is up; none for breaking ones or gear wards alone.
- **Channelled effects**: a concentration spell's effects with no
  duration (the ward and its armor, telekinesis) last while it is cast:
  the player's while a hand holds the spell, anyone else's half a second
  after its last landing (NPCs don't ward yet). Landing again keeps them
  rather than starting them over, unless it changes between dual and not.
- **Restoration XP**: wards give their cast's XP as other self spells (a
  quarter of the base cost each landing); absorbing gives nothing more.

## Still open

- NPCs raising wards in combat (`CombatMagicCasterWard`), their
  `WardAbsorbed` / `WardBroken` idles (`ActionWardHit`) and `IsWardState`.
- The ward's sounds (`WardAbsorbSound`, `WardBreakSound`,
  `WardDeflectSound` default objects) and visuals.
- `WardDeflection` (actor value 67): nothing in the records sets it.
