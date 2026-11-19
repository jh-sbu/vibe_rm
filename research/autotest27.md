# autotest27 in Skyrim SE (run 2026-11-19)

`test-mod autotest27` (`crates/tools/src/test_mods/autotest27.rs`):
autotest24's faults and open items, autotest20's charge, and readings
earlier batches took on held actors, repeated with the actor free. A
throwaway save (a level 6 character outdoors in Whiterun Hold); Survival
Mode's and the Curios plugin were loaded. Every test reported except the
last, KillerObjectDead, which crashed to desktop (below). The game had saved
after the test before it, so nothing else was lost.

*Settled*, *Partial* and *Fault* as in `research/autotest23.md`. Form ids
in the readings are decimal: 20 is the player; 117443108 our Disarm,
117443116 / 117 / 118 our Calm / Fear / Fury, 117442644 our fire spell;
77495 the iron sword.

## The crash

KillerObjectDead ran `IsKillerObject` on the dead Victim, with a list of
Alpha's base and then of Alpha's reference, in a test of its own after
the kill (KillVictim) and both `GetTimeDead` reads had gone through. It
crashed SE, as the tests that combined the kill and this probe did in
autotest23 and autotest24. *Settled*: `IsKillerObject` evaluated on a dead
reference with a form list crashes SE. What it returns there can't be read.

## Readings

**KillVictim**: `Kill(Alpha)`: dead, `GetKiller` Alpha.

**TimeDead**, **TimeDeadLater** (`GetTimeDead >= ProbeValue` bisected, 17
probes of about 2.6 s each over the window):

| window after the kill (real s) | game hours | reading |
| --- | --- | --- |
| 4.2 to 48.6 | 0.025 to 0.284 | in [0, 0.061) |
| 82.8 to 127.2, after timescale 100 for 30 s | 1.18 to 1.44 | in [0.977, 1.038) |

*Partial*. The value was a whole number both times: 0, then 1. Ruled out:
game hours as they are (1.18 or more would hold at 1.04); tenths of a game
hour (2.8 by the first window's end); real minutes as they are (0.7 at the
first window's end, 2.1 at the second's). What fits, with autotest22
(`>= 1` and not `>= 10` 70 to 85 s, 0.41 to 0.47 game hours, after a death):
- **Hour boundaries on the clock passed since the death** (whole game hours
  of `GameHour`, not elapsed): fits all three if the deaths fell at suitable
  times of the hour. The hour at the kill wasn't recorded.
- **Whole real minutes**: fits autotest22 and the first window. It fits the
  second only if the death was counted at least about 6 s after `Kill`.
  The last probes ran 120 to 126 s after the call and read below 2.
  (The Vigilants below took about 1.8 s from `dying` to `death`.)

A follow-up that reads `GetTimeDead >= 1` every few seconds, with
`GameHour` noted at the kill, would tell these apart.

**Holds** (target-actor copies of the vanilla spells' main effects, cast
by script from 140 to 185 units; autotest24's vanilla casts did nothing):
- Our Fear (9) on a level 1 fighter and a level 8 NPC: the effect event,
  confidence 3 → 0, both ran from 314–364 to about 920 units off their
  held target, `IsFleeing` yes, combat state still 1. *Settled*: Fear
  lands as a target-actor spell; its targets flee in combat.
- Our Calm (9) on a level 1 fighter: the effect event, combat state 0 at
  once (`OnCombatStateChanged` 0), and Aggression read -1 for the 8 s
  (it was 1). *Settled*.
- Our Fury (6) on a level 1 unaggressive NPC: Aggression 0 → 3, combat
  state 1 at once, its target **the player** (the caster, 139 off), not
  the other NPC 200 off. *Settled* for this layout. Which target a fury
  picks (the nearest one, or the caster) is open.

**Disarm** (our target-actor Disarm at 12, by script from the player):

| case | disarmed |
| --- | --- |
| level 1, alerted, drawn, out of combat | 0 of 4 |
| level 1 fighter, in combat, mid-swing (`IsAttacking` true) | 4 of 4 |
| level 1 fighter, in combat, between swings | 0 of 4 |
| level 15, alerted, drawn, out of combat, at 12 | 0 of 3 |
| level 15, the same, at 30 | 0 of 2 |

Each disarm came with an `OnHit` from the player carrying the spell, and
the sword ended up on the floor (`GetEquippedWeapon` none, none carried).
No miss came with an `OnHit`. Every cast gave the effect event.
*Settled*: Disarm takes the weapon only from a target that is attacking.
That explains autotest24's misses within the cap, which were cast at no
particular moment of a fight. The level cap is still untested: the level
15 casts were made on idle NPCs, which never lose their weapon. Test it
again mid-swing. One "between swings" case read the weapon sheathed at the
cast.

**Disease**: *Fault*. The skeever never fought: 0 hits in 120 s, combat
state 0 at the end. The player had DiseaseResist 0 and no Ataxia before and
after; `GetDisease` no throughout. Why the skeever didn't attack (it was
`StartCombat`ed on the player, 300 off, in the warehouse) is unknown.

**HealerHurt** (the Vigilant, free, at 40% of 90 health and HealRate 0,
fighting a level 1 fighter with 5000 health): it cast Oakflesh
(`OakfleshLeftHand`) as the fight began, then Lesser Ward
(`WardLesserLeftHand`) at about 22% health while being struck. It never
cast a heal, and it died about 20 s in. Its health fraction read 0.31,
0.22, 0.22, 0.22, then dead.
**WardsFree** (the same, against a fire warlock): it dropped to about 2%
within 4 s (bleedout at 3.6 s), recovered slowly in bleedout (0.022,
0.036, 0.047 at 4, 8 and 12 s), cast Healing (`HealingLeftHand`) at about
5%, 12.9 s in, and died 3.6 s later. No `OnWardHit`.
*Partial*: a hurt Vigilant wards against melee blows at 22% rather than
healing, and heals in bleedout under fire. Both died too soon to show
more. The setup needs more health than 90 at 40%.

**HostileSpellFree**: a free, lawful, unarmed NPC hit by a script cast of
the player's fire spell went to combat state 1 at once, targeting the
player. It moved away from 306 to about 875 units off and stayed there, not
drawn. No crime gold. *Settled*: a free target fights the caster (state 1,
not autotest21's held target's search). Unarmed, it kept its distance.

**SheatheFree**: *Fault*, and autotest21's AlertFlag reading with it.
`IsWeaponDrawn` never read true in any case. Each loop stopped at its
0.5 s floor (0.56 to 0.66 s) because the weapon wasn't drawn *yet*, not
because it had been put away. The NPCs moved 50 to 100 units meanwhile.
autotest21's loop was the same, so its "sheathed 0.62 s later" is this
artefact too. What is known is autotest19's: unalerted NPCs read drawn
at 1 s and sheathed by 3 s. Whether a repeated `DrawWeapon` restarts the
delay is unread.

**RestrainedFights** (a level 1 fighter and an enemy 100 off,
`StartCombat` both ways, 12 s):

| held by | state | swings | damage dealt | taken | moved |
| --- | --- | ---: | ---: | ---: | ---: |
| `SetRestrained` | 0 | 0 | 0 | 54.6 | 161.9 |
| `SetDontMove` | 1 | 5 | 21.5 | 33.5 | 0 |
| free | 1 | 1 | 28.6 | 15.3 | 81.7 |

*Settled*: a restrained NPC doesn't fight (it isn't even in combat) and
can be moved by blows. A `SetDontMove` NPC fights in place.

**AttackOnSightLong** (our `IsGuard` guard, our faction with Arrest and
Attack on Sight, bounty 1000, no location): stood about 400 off for 41 s.
It was never hostile and never drew, and it detected the player
throughout. **AttackOnSightHold** (a voiceless `IsGuard` guard of
`CrimeFactionWhiterun`, outdoors in Whiterun Hold, Whiterun bounty 1000):
walked from 467 to 352 units, then stood for the rest of the 44 s, never
hostile. *Partial*: a bounty of 1000 with Attack on Sight doesn't make a
guard attack within 40 s, in its own hold or not. Open: whether 1000 is
below the threshold, or whether the attack comes through the arrest
dialogue a voiced guard would start (ours have no voice).

**Charge1500**, **Charge2500** (two level 15 fighters with swords, 5000
health, stamina 50, fighting each other; the second moved away once
engaged): the first ran (`R`) back from about 1180 and 2320; the second
walked or ran. **No sprint (`S`) at any distance.** At 2500, both stopped
about 1040 apart for a few seconds, then closed to about 380 and stood
there, the second searching (state 2), until the end. *Settled* for these
fighters: NPCs don't sprint to close a gap of 1500 or 2500 in combat
(autotest20's table: "drop the rule").

## Faults and open items, for a follow-up batch

1. `GetTimeDead`: `>= 1` read every few seconds from the kill, `GameHour`
   recorded at the kill (hour boundaries against whole real minutes).
2. Disarm's level cap mid-swing: level 15 at 12 and 30, cast when
   `IsAttacking`.
3. `GetDisease`: a creature that does attack (read its combat state and
   distance early); several hits.
4. The Vigilant with health enough to survive (Fortify Health, or a weaker
   attacker), healing against warding by health.
5. The sheathe delay: poll from `DrawWeapon` until drawn, then until
   sheathed, 10 s at most; a second `DrawWeapon` after it has drawn.
6. Attack on Sight: bounty above 1000 (2000, 5000), and a voiced guard
   copy, if the run can stay hands-off.
7. Fury's target: the caster farther than the other NPC.

## Compared with ours (2026-11-19)

- **Changed to agree**:
  - Disarm only takes the weapon from a target that is attacking
    (`src/disarm.rs`, `known_ambiguities/disarm.md`).
  - NPCs run, not sprint, to charge, and so make no sprint attacks
    (`known_ambiguities/combat-movement.md`).
  - A restrained actor doesn't fight (`Engine::start_combat`,
    `known_ambiguities/actor-holds.md`).
  - The idle sheathe is 1.5 s drawn, not 0.6 s: a choice between
    autotest19's 1 and 3 s.
  - `GetTimeDead` counts the hours struck on the game clock since the death
    (`known_ambiguities/death.md`). This is a choice: whole real minutes
    also fit.
- **Agree already**: Calm holds Aggression at -1 and ends the fight; Fear
  holds Confidence at 0 and the target flees, still in combat; Fury holds
  Aggression at 3 and attacks the nearest (here the caster); a free target
  of a hostile spell fights its caster (`src/magic.rs`).
- **Differ, left open**: a restrained actor pushed by blows (ours holds it
  still); `SetRestrained` on an actor already fighting (unread; ours lets it
  fight on).
