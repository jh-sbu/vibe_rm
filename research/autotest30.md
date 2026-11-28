# autotest30 in Skyrim SE (run 2026-11-27)

`test-mod autotest30` (`crates/tools/src/test_mods/autotest30.rs`): seven
open items, each in the nine test pens at once. A throwaway save (a level 6
character); Survival Mode's and the Curios plugin were loaded, Survival Mode
off. All 8 tests reported; nothing crashed.

*Settled*, *Partial* and *Fault* as in `research/autotest23.md`. Form ids in
the readings are decimal: 467733 `OakfleshLeftHand`, 216120
`HealingLeftHand`, 256733 the potion `RestoreHealth01`; 77495 the iron
sword, 80254 the iron dagger.

## Readings

**NpcRegen** (a level 10 NPC per pen, held, out of combat; percent of the
most a second, 1.5 to 5.5 s after the drop):

| value | rate | multiplier 100 | 0 | 300 |
| --- | ---: | ---: | ---: | ---: |
| Health (`HealRate` set to 0.7) | 0.7 | 0.762 | 0 | 2.338 |
| Stamina | 5 | 5.628 | 0 | 17.083 |
| Magicka | 3 | 3.489 | 0 | 10.690 |

*Settled*: NPCs' `HealRateMult`, `StaminaRateMult` and `MagickaRateMult`
count: none at 0, about three times at 300 (3.07, 3.04, 3.06), against the
rate at 100. The reads at 100 run 9 to 16% above the rate, from the reads'
own timing (the ratios hold).

**KnockdownHolds** ("Always", `fakeForceBallLite1024`, 50 west of an NPC in
each pen):

| hold | knocked (the probe) | moved |
| --- | --- | ---: |
| `SetDontMove` | 3 of 3, at 0.47 to 0.60 s | 75 to 91 |
| free | 3 of 3, at 0.66 to 0.79 s | 70 to 113 |
| `SetRestrained` | 0 of 3 | 0 to 4.8 |

*Settled*: `SetDontMove` doesn't keep a knockdown off (explosions round 1's
held NPCs that didn't move were otherwise set up); `SetRestrained` does.

**HostileHeld** (our fire spell cast by script from an unaggressive NPC at a
lawful NPC): in every pen, held or free, unarmed or with a sword, the
caster 100, 250 or 400 off, the target was in combat state 1 with the
caster as its target at 2, 4, 6 and 8 s, drawn; no crime gold. *Settled*
for an NPC caster: the target fights it, held or not, in reach or not.
autotest21's and autotest28's held targets, which searched (state 2), were
hit by the player's cast: the searching comes with the player as caster
(why is open).

**FuryUnseen** (our Fury on a level 1 NPC; Aggression read 3 throughout in
every pen, so it landed):

| caster | others in the pen | target |
| --- | --- | --- |
| the player, in the booth (unseen, through its floor) | an NPC 150 off | the other NPC, from 2 s to 16 s |
| the player, in the booth | none | none: combat state 0 throughout |
| an NPC 400 off, held, taken away after 6 s | an NPC 150 off | the other NPC throughout |

*Settled*: a frenzied NPC goes for the nearest NPC, not its NPC caster, and
nobody unseen; with nobody to find it stays out of combat. With
autotest27 and autotest28 (the player, seen, the caster: it went for the
player, even with the other NPC nearer, 150 against 437): the player as
caster, seen, is the exception.

**Healing** (a Vigilant, 600 health, against a level 1 fighter (melee) or a
fire warlock (fire); health fraction / magicka every 4 s; casts in seconds):

| pen | from | heals (`HealingLeftHand`) at | other |
| --- | --- | --- | --- |
| 0 | melee 0.20 | 18.2 (about 0.157) | Oakflesh 10.3 |
| 1 | melee 0.18 | 4.4 (about 0.178), 24.0, 28.7 | Oakflesh 17.6 |
| 2 | melee 0.15 | 13.1, 19.7, 28.1 (0.09 to 0.11) | |
| 3 | fire 0.20 | 7.8, 9.8 (about 0.12); rose to 0.239 and stopped | |
| 4 | fire 0.18 | 11.6 (about 0.06) | bleedout 12.2, stayed in it |
| 5 | fire 0.15 | 2.4, 4.7, 8.9, 15.0; rose to 0.250 and stopped | |
| 6 | melee 0.10 | 8.0, 18.3, 23.9, 25.9 | |
| 7 | fire 0.10 | none | bleedout 5.0, stayed in it |
| 8 | fire 0.15, no magicka | 17.5, in bleedout | the potion at 0.2; bleedout 14.5 and 17.9 |

*Settled*: the Vigilant heals against melee as against fire (autotest28's
melee runs never fell below 0.218); it heals below about 0.18 and stops
near 0.25. *Partial*: the start isn't immediate (pen 0 sat at 0.171 for
about 10 s first) and isn't a fixed line (0.178 in pen 1). With no magicka
it drank a restore-health potion at once (pen 8). It cast a heal in
bleedout once (pen 8, with 14.5 magicka; autotest27 too), but two others
stayed in bleedout with 56 and 57 magicka.

**DisarmDagger** (a level 1 fighter with a sword, disarmed mid-swing by our
Disarm cast by script from its opponent; one cast each, all disarmed):

| pen | setup | armed again |
| --- | --- | --- |
| 0-2 | a dagger carried | with the dagger, after 2.75 to 2.79 s; the sword left lying |
| 3-5 | no dagger | with its sword, picked up, after 11.2, 27.0 and 26.7 s |
| 6-8 | a dagger, the opponent taken away | not in 60 s; out of combat, the sword lying |

*Settled*: a disarmed fighter draws a weapon it carries about 2.8 s after;
with none, it picks its weapon up from the floor (in 11 to 27 s); out of
combat it does neither.

**DiseaseResist** (Disease Resist 0, 50, 100; a bear until `HasMagicEffect`
showed Bone Break Fever, then max stamina, 50, read every second for about
110 s):

| pen | resist | caught at blow | max stamina after | changes |
| --- | ---: | ---: | --- | ---: |
| 0 | 0 | 3 | 41.504 (8.5 drained) | 0 |
| 1 | 0 | 11 | 25 | 0 |
| 2 | 0 | 28 | 25 | 0 |
| 3 | 50 | 2 | 50 (none) | 0 |
| 4 | 50 | 21 | 49.826 (0.17) | 0 |
| 5 | 50 | 2 | 25 | 0 |
| 6 | 100 | never (3 blows) | | |
| 7 | 100 | never (145 blows) | | |
| 8 | 100 | never (108 blows) | | |

`GetDisease` (the probe) read yes on all six infected NPCs, with the drain
or without. *Settled*: a caught disease isn't rolled again: whatever drain
it had, it kept for 110 s without a change. Disease Resist 100 keeps it off
(253 blows). `GetDisease` holds for an NPC with a caught disease.
*Partial*: the drain isn't always 25. With autotest29: 25 seven times, 0
four times, about 9 twice, 0.17 once. What decides it is open (UESP's
confirmed bug: "rarely drains 0"). Resist 50: caught at blows 2, 21 and 2,
against 3, 11 and 28 at 0; too few to show the halving.

## Compared with ours (2026-11-27)

- **Engine gaps found by the dry runs, fixed**: scripts read our swings as
  the graph's `IsAttacking` (ours never set it, so ours disarmed nobody);
  `OnSpellCast` is sent, once a cast (nothing sent it).
- **Changed to agree**:
  - NPCs' `HealRateMult` and `StaminaRateMult` count
    (`known_ambiguities/actor-values.md`); `MagickaRateMult` already did.
  - Explosions don't knock restrained actors down
    (`known_ambiguities/explosions.md`).
  - Fury goes for the player who cast it only if it finds them, else the
    nearest it finds (`known_ambiguities/magic.md`).
  - A disarmed fighter wields a weapon it carries 2.8 s after, while still
    fighting (`known_ambiguities/disarm.md`).
- **Agree already**: an NPC caster's hostile spell starts a fight, held or
  not; a caught disease isn't rolled again; Disease Resist 100 keeps it
  off; `GetDisease` on an infected NPC; the Vigilant heals against melee
  too, below about 0.2, stopping near 0.25.
- **Differ, left open** (`plans/ROADMAP.md`): picking a dropped weapon up;
  the drain's size (25 or less); the Vigilant's potion, healing in
  bleedout and its delay before a first heal; the player's cast searching
  in autotest21 and 28. In our dry run a frenzied NPC alone in its pen
  found an actor in another pen within 2 s (SE: none).
