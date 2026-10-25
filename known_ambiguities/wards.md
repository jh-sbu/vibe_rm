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
  broke it). The player's hand casting it stops (to be cast again). The
  caster's graph gets `WardBreak` (the master graph's stagger state, the
  `WardBroken` idle's event), or `staggerStart` at 0.5 where it has none;
  an absorbed spell sends `wardAbsorb` (the ward loop's own transition,
  the `WardAbsorbed` idle's event). The game picks these through the
  `ActionWardHit` idles; sent straight here.
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
  after its last landing. Landing again keeps them
  rather than starting them over, unless it changes between dual and not.
- **Restoration XP**: wards give their cast's XP as other self spells (a
  quarter of the base cost each landing); absorbing gives nothing more.

## NPCs

Public sources: CommonLibSSE's `CombatMagicCasterWard` (its own
`CheckStartCast` / `CheckStopCast` and a cooldown timer, nothing about
what they check), `MagicSystem::WardState`, the `ActionWardHit` idles
(`WardAbsorbed` / `WardBroken` and draugr, dragon priest and Hermaeus
Mora's daedra variants, each by `IsWardState`), and who knows a ward
(`vrm-tool item-users WardLesser`...: Vigilants of Stendarr, warlocks,
ghosts, Thalmor, battlemages, Delphine, the College's teachers; the
`...LeftHand` copies are equipped in the left). No public source says
when the game raises them; here:

- **When**: fighting, while a hostile spell is being cast at it: the
  player charging, holding or letting go a hostile non-self spell (or
  staff) while aiming within 30 degrees of its body and fighting it, or
  an NPC caster whose target it is charging, throwing or channelling one.
  Spells already flying (the caster done) don't count beyond the keep
  below. Every NPC that knows a ward does it, whatever its combat style.
- **Before others**: after an emergency heal, before offensive spells and
  staffs; a melee NPC stops fighting to hold it. The strongest ward it
  can pay a second of.
- **Keeping it**: held facing its enemy until 1 s after the last spell
  seen being cast at it, at most 10 s at once, then 2 to 4 s before
  another; a broken ward stops the hand and starts the same wait. The
  graph's first refusal of `MLh_WardStart` while it is still entering the
  magic state is retried half a second later, as other casts.
- **`IsWardState`** is a ward hit's outcome for 1 s after it (or until
  the next); 0 otherwise. The reaction played is the first passing
  `ActionWardHit` idle for the actor's graph; `staggerStart` ones at
  magnitude 0.5.

## Native save policy

Optional `WSTA` data preserves each actor's last ward-hit outcome (absorbed 1
or broken 2) and the remaining duration of the native runtime's one-second
`IsWardState` hold. A load replaces this map; a legacy save clears it. This
does not preserve the active ward spell/effect, its accumulated power, or graph
animation progress. UESP documents ward depletion, regeneration and stagger
when a ward breaks, but does not document this timer or save persistence
([Skyrim:Ward, revision 3473630](https://en.uesp.net/w/index.php?oldid=3473630));
the one-second hold and its persistence here are native runtime policy, not a
claim about Skyrim SE save compatibility.

## Still open

- Whether combat styles weigh NPCs' warding.
- `WardDeflection` (actor value 67): nothing in the records sets it.
