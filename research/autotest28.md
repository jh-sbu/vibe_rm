# autotest28 in Skyrim SE (run 2026-11-27)

`test-mod autotest28` (`crates/tools/src/test_mods/autotest28.rs`):
autotest27's faults and open items (`research/autotest27.md`). A throwaway
save (a level 6 character, the run begun outdoors); Survival Mode's and the
Curios plugin were loaded. All 13 tests reported; nothing crashed.

*Settled*, *Partial* and *Fault* as in `research/autotest23.md`. Form ids
in the readings are decimal: 20 is the player; 117443108 / 109 our Disarm
at 12 / 30, 117443118 our Fury, 117442644 our fire spell; 77495 the iron
sword. The Vigilant's casts: 467733 `OakfleshLeftHand`, 467737
`WardLesserLeftHand`, 216120 `HealingLeftHand`, 256733 the potion
`RestoreHealth01`. Event times are hundredths of a second of real time.

## Readings

**TimeDeadBeforeHour**, **TimeDeadAfterHour** (`GetTimeDead >= 1` polled
from a script `Kill`, timescale 20):

| killed at (clock) | first `>= 1` | game hours after | the clock then |
| --- | --- | --- | --- |
| 17.902 | 15.9 s | 0.108 | 18.011 |
| 18.050 | 161.1 s | 0.956 | 19.006 |

`>= 2` read no both times. *Settled*: `GetTimeDead` counts the hours struck
on the game clock since the death. Whole real minutes (both near 60 s) and
whole game hours elapsed (both near 180 s) are ruled out.

**DisarmCap** (our Disarm cast by script from the player at a fighter
caught mid-swing, `IsAttacking` true at every cast):

| target, magnitude | disarmed |
| --- | --- |
| level 1, 12 (control) | 1 of 1 |
| level 15, 12 | 0 of 3 |
| level 15, 30 | 3 of 3 |

Each disarm came with an `OnHit` from the player carrying the spell; the
level 15 casts at 12 gave only the effect event. *Settled*: the level cap
holds as UESP has it (magnitude against the target's level).

**Disease** (outdoors; a creature `StartCombat`ed on the player, who has
DiseaseResist 0 and doesn't fight back, for 90 s or until `HasSpell` of its
race's attack spell; the player's health kept up):

| creature, disease | at 2 s | hits | `HasSpell` | then `GetDisease` | after `RemoveSpell` |
| --- | --- | ---: | --- | --- | --- |
| skeever (level 1), Ataxia | fighting, 239 off | 19 | no | no | no |
| wolf (level 2), Rockjoint | fighting, 188 off | 69 | no | no | no |
| bear (level 12), Bone Break Fever | fighting, 146 off | 35 | no | **yes** | no |

The races' `ATKD` attack spell is the disease itself (the wolf's
`DiseaseRockjoint`, here Survival Mode's override). *Partial*:
- `GetDisease` read 1 after the bear's blows and 0 once `RemoveSpell(Bone
  Break Fever)` had run, so the player had caught Bone Break Fever. After
  autotest22 to 24 (0 for diseases added by `AddSpell` or cast), a disease
  caught from a creature's blow counts and an added one doesn't.
- `HasSpell(Bone Break Fever)` read false at the 90 s end, an instant before
  combat was stopped. Either the disease was caught in the second after
  that (a swing under way), or `HasSpell` doesn't see a caught disease
  while `RemoveSpell` cures it. Not told apart.
- 19 skeever and 69 wolf hits didn't infect. The chance per hit isn't read.

**Healer** (the Vigilant with 600 health, `HealRate` 0, free, against a
level 1 fighter with a sword and 5000 health; health fraction / magicka /
combat state every 4 s, 24 s each):

| from | casts (s after the fight began) | health at the 4 s reads |
| --- | --- | --- |
| 0.70 | Oakflesh 5.3 | 0.699, 0.699, 0.699, 0.686, 0.661, 0.661 |
| 0.50 | Oakflesh 2.5, Lesser Ward 23.4 | 0.473, 0.473, 0.450, 0.409, 0.409, 0.395 |
| 0.30 | Oakflesh 14.9 | 0.300, 0.259, 0.232, 0.218, 0.218, 0.218 |

It never healed against melee, even at 0.218 for 8 s with 53 to 60
magicka.

**HealerWarlock** (the same against a fire warlock, 28 s each):

| from | casts (s after the fight began) | health at the 4 s reads |
| --- | --- | --- |
| 0.70 | Lesser Ward 12.8, Oakflesh 22.4, **Healing 24.8**, potion 30.6 | 0.643, 0.568, 0.408, 0.302, 0.172, 0.155, 0.131 |
| 0.40 | **Healing 14.4**, Lesser Ward 22.9, 25.8, 28.7 | 0.326, 0.237, 0.150, 0.220, 0.222, 0.115 |

From 0.70, Healing came at about 0.16 and lasted about 1.5 s; magicka ran
out (0 at 29.4 s) and it drank `RestoreHealth01` at 0.131. From 0.40,
Healing came below 0.150 and lasted about 3.9 s, bringing it back to about
0.22; it then warded (`OnWardHit` 0 to 2 throughout) and at 0.115, with 28
magicka, warded rather than healed. *Partial*: the Vigilant heals at about
0.16 or below and not at 0.218 or above (autotest24's 20%, held, didn't
either); a heal stops short of full health, here near 0.25
(`fCombatRestoreStopCastThreshold` is 0.25). It wards against fire, and
rarely against melee. With no magicka left it drank a restore-health
potion.

**SheatheDelay** (an unalerted, free NPC told to `DrawWeapon`, polled every
0.05 s):

| case | `IsWeaponDrawn` true | false again |
| --- | ---: | ---: |
| once | 0.710 | 2.576 |
| once | 0.719 | 2.613 |
| once | 0.589 | 2.460 |
| `DrawWeapon` again at 1.275 | 0.708 | 2.584 |
| `DrawWeapon` again at 1.244 | 0.688 | 2.524 |

*Settled*: `IsWeaponDrawn` reads true 0.59 to 0.72 s after `DrawWeapon`, and
false again 1.86 to 1.90 s after that (2.46 to 2.61 s after the call). A
second `DrawWeapon` while drawn doesn't restart the delay.

**AttackOnSightHigh** (our `IsGuard` guard, our faction with Arrest and
Attack on Sight, about 400 off): at bounty 0, not hostile, detecting the
player, sheathed. At 2000 it drew within 5 s and moved to 357 off; at 2000
and 5000, 25 s each, it stayed there drawn, never hostile, combat state 0,
not targeting the player. *Partial*: a bounty of up to 5000 doesn't make a
guard of a faction that arrests attack on sight without the arrest
dialogue (ours has no voice). From 2000 (not at 1000: autotest27) it draws.
Still open: a voiced guard.

**FuryTarget** (our Fury at 6 on a level 1 unaggressive NPC; the other NPC
150 off and held, the player 437 off): Aggression 3, combat state 1, its
target the player at all four reads (`OnCombatStateChanged` 1, 0, 1 within
0.11 s). The other NPC lost no health. *Settled*: a Fury cast at an NPC
sends it at its caster, even with another actor nearer.

**RestrainedMidFight** (a level 1 fighter engaged with another, then
`SetRestrained(true)`, 10 s): combat state 1 and drawn throughout; 0 swings,
0 damage dealt, 36.4 taken, moved 75 units. *Settled*: restraining an NPC
already fighting leaves it in combat, drawn, but it doesn't attack; blows
still move it.

**HostileSpellHeld** (the player's fire spell cast by script at a lawful NPC
held by `SetDontMove`, 400 off): combat state 2 at 2, 4, 6 and 8 s, its
combat target the player throughout; no crime gold. *Settled*: as
autotest21's held target, it searches (state 2) rather than fights.
autotest27's free one fought (state 1); autotest27's `SetDontMove` fighter,
in reach of its enemy, fought in place (state 1).

## Open, for a follow-up batch

1. The disease chance per blow, and whether `HasSpell` reads a caught
   disease (`HasSpell` polled through the bear's blows, `GetDisease`
   alongside).
2. Attack on Sight by a voiced guard (its dialogue would stop the script).
3. Why a held target of a hostile spell searches: out of reach of its
   attacker, or because it is held (`SetDontMove` at 100 and at 400 off).
4. The Vigilant's heal threshold between 0.16 and 0.22, and whether it
   needs a spell attacker (melee at 0.18).
