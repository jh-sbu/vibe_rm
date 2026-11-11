# autotest24 in Skyrim SE (run 2026-11-10)

`test-mod autotest24` (`crates/tools/src/test_mods/autotest24.rs`):
autotest23's faults and open items (`research/autotest23.md`). Same
throwaway save as autotest23 (a level 6 Nord). The game saved after each
test. Test 10 (KillerObject) crashed to desktop, and the run went on with
`set VrmAt24SkipTest to 10`.

*Settled*, *Partial* and *Fault* as in `research/autotest23.md`.

## The crash

KillerObject killed Victim with `Kill(Alpha)` and read two probes run on
the dead Victim: `IsKillerObject` with a list of Alpha's base, then with a
list of Alpha's reference. It crashed SE. autotest23's TimeDead, which
crashed too, began the same way; its timescale of 1000 came later. In
autotest22 the same kill with `IsKillerObject` run on the live killer, and
`GetTimeDead` run on the dead Victim, ran without trouble. So the likely
cause, in both batches, is `IsKillerObject` evaluated on a dead reference
with a form list. The kill and the probes weren't separated, so this is
inference.

With KillerObject skipped, Victim was never killed, so TimeDead and
TimeDeadLater read an actor that was alive (`GetTimeDead` 0; their "game
hours after the kill" count from the game's start). Both are *Fault*.
`GetTimeDead`'s unit and the timescale question stay open.

## Readings

**ViewCone** (observer in the middle of the floor facing north, the player
sneaking 300 units off, light 31.9, both Sneak 15; every read 300 units
off as placed): seen at 80° and 90°, not at 100° to 180°, at 2 s and 5 s
alike. Ahead at 300: seen. Behind at 150: not. *Settled*: the view
half-angle is between 90° and 100° for this case. Ours (95°) **agrees**.

**Disarm** (our target-actor copy, cast by script from the player; every
cast gave the effect event):

| magnitude | level | weapon | disarmed |
| ---: | ---: | --- | --- |
| 12 | 1 | sword | no, then yes |
| 12 | 1 | greatsword | yes |
| 12 | 8 | sword | no |
| 12 | 10 | sword | yes, then no |
| 30 | 10 | sword | yes |
| 30 | 20 | its own greatsword (the given sword wasn't drawn) | no |

A disarm always came with `OnHit` and an unequip; a failure with neither.
*Partial*. Every disarm was within the cap (level 1 and 10 at 12, level 10
at 30), but casts within the cap failed too, at level 1 and level 10 alike.
So something besides level sometimes stops it. Greatswords drop like
swords. The level 20 case doesn't count: that NPC drew its own greatsword,
not the sword it was given.

What the sources say: UESP's *Skyrim:Disarm* (revision 3246655) gives a plain
cap ("Disarms enemies up to level 12 / 20 / 30") and no chance. Its talk
page (revision 1455046) has one unconfirmed claim of being disarmed above
level 30. The only chance-based disarm UESP describes is the Block perk
Disarming Bash (*Skyrim:Block*, revision 3648127: "Chance to disarm when
power bashing"). CommonLibSSE-NG declares the archetype's effect class as
`DisarmEffect : StaggerEffect`, with its own `Update` and `Start`. It gives
no rules, but suggests that Disarm works like a stagger.

Inference, not settled: the cap holds, and the misses depend on what the
target was doing when the effect landed (a stagger the target's current
action doesn't take, such as a swing). These casts landed on NPCs mid-fight
at no particular moment. Ours (the cap, always) stays as UESP has it, until
a probe separates the two: casts on NPCs drawn but idle against casts during
attacks, and casts above the cap.

**Disease**: Ataxia cast on the player the way the game's traps do
(`TrapDiseaseAtaxia`), cast directly, and the trap spell cast by an NPC:
none gave the player Ataxia (`HasSpell` False), and `GetDisease` read no.
Sanguinare Vampiris added by `AddSpell` (`HasSpell` True): `GetDisease` no.
*Partial*: a script cast of a disease spell doesn't infect, and an added
disease (Ataxia in autotest23, vampirism here) doesn't make `GetDisease`
true. What does is still open.

**Holds**, cast from 140 to 180 units:
- Fear on a level 1 fighter and on a level 8 NPC, both fighting a held
  target: the effect event, but confidence stayed 3, both kept fighting for
  8 s, and `IsFleeing` was no.
- Calm on a level 1 fighter, Fury on a level 1 unaggressive NPC: no event,
  no change (as at 550 units in autotest23).
- *Partial*: script casts of the vanilla Calm and Fury spells from the
  player don't land, close or far. Fear's lands without making the target
  flee. Our target-actor copy of Disarm worked where the shout's spell
  didn't, so the next probe should use target-actor copies of the Calm,
  Fear and Fury effects.

**Healer** (the Vigilant: `OakfleshLeftHand`, `HealingLeftHand` and
`WardLesserLeftHand` in its spell list; magicka 330, health set to 90%, 70%,
55%, 45% and 20%, 8 s each): it cast Oakflesh once at the start
(`cast#OakfleshLeftHand`), then held Lesser Ward in its left hand and cast
nothing more. Its health never rose. *Partial*: this Vigilant never cast
its heal, at any health, with magicka to spare. Ours (heals below 60%)
**differs**. The held target doesn't hurt it, so it may heal only when it
is being hurt; open.

**AttackOnSightGuard** (a guard by `IsGuard`, of our faction with Arrest and
Attack on Sight): at bounty 0 it stood 500 units off. At 999 and at 1000 it
came to 180–240 units from the player but never turned hostile or fought
in 6 s. *Partial*: an `IsGuard` guard of an Attack on Sight faction walks
up at a bounty of 999 and 1000 (an arrest approach) rather than attacking.
Whether it attacks later, or only once it sees the player in a hold of its
own, is open. Our NPC has no voice, so it couldn't speak to arrest.

**RelationshipHelp**: a helper of friends and allies joined at rank 4 (twice)
and at rank 3. With autotest23: it joins at every rank from 1 to 4; a helper
of allies only joins from rank 2. *Settled*.

**EnterWarehouse**, **LeaveWarehouse**: as before.

## Faults and open items, for a follow-up batch

1. `IsKillerObject` run on the victim: kill in one test, probe in the next,
   so the crash can be pinned on the probe (and skip it if so). Then
   `GetTimeDead` read twice around timescale 100.
2. Calm, Fear and Fury as target-actor copies of their effects.
3. Disarm: casts on NPCs drawn but not attacking (alerted, out of combat)
   against casts during attacks, and casts above the cap, several each.
4. `GetDisease`: what makes it true.
5. The Vigilant's heal when it is being hurt (a fighter that strikes it).
6. Attack on Sight: a longer watch, and a guard in its own hold.

## Compared with ours (2026-11-10)

- **Changed to agree**: Disarm's `OnHit` only with a disarm (`disarm.md`);
  relationship ranks for helpers (with autotest23); `GetDisease` 0.
- **Agree already**: the view cone's edge (ours 95°); the level cap
  (every disarm was within it).
- **ViewCone in ours** (run again after the player's `MoveTo` kept its
  offsets, and after `SetDontMove` held the observer): seen at 80° and
  90°, not at 100° to 180°, ahead at 300 seen, each 300 off, as in SE.
  Behind at 150: seen in 3 of 11 runs (SE: not), when the observer barked
  and turned to the player. Light 113.7 (SE: 31.9).
- **Differ, left open**: Disarm's misses within the cap (`disarm.md`); the
  Vigilant's healing (`npc-healing.md`); Fear not making its target flee,
  and script casts of Calm and Fury not landing (no change: the probe is at
  fault until target-actor copies are tried).
