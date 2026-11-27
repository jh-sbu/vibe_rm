# autotest29 in Skyrim SE (run 2026-11-27)

`test-mod autotest29` (`crates/tools/src/test_mods/autotest29.rs`): whether
NPCs catch a creature's disease, the first batch in the test pens
(`crates/tools/src/test_mods/pens.rs`). A throwaway save (a level 6
character); Survival Mode's and the Curios plugin were loaded, Survival Mode
off. Both tests reported; nothing crashed.

*Settled*, *Partial* and *Fault* as in `research/autotest23.md`.

## The pens

**PensEnter**: the player in the booth: `IsInInterior` true, 1100 above the
middle pen, the pens 1100 to 2265 off. Every bear was in combat with its
own NPC at 3 s (66 to 220 units off), and eight of nine landed 71 to 133
blows in 150 s. *Settled*: the hand-written navmeshes and their `NAVI`
entries load, and creatures path and fight on them. Pen 5's bear landed
only 8 blows (why is unread: it was 220 off at 3 s, as pen 0's, which
landed 94).

## NpcDisease

Nine level 10 unaggressive NPCs (DiseaseResist 0, held by `SetDontMove`,
health kept up), each attacked by a level 12 bear (`EncBear`; Bone Break
Fever, `DisDamageStamina` with `GetRandomPercent <= 5`, on every attack).
Blows counted by `OnHit`; the blow at which each reading first showed the
disease checked as each hit arrived.

| pen | blows | `HasMagicEffect` from blow | max stamina 50 → 25 from blow | max stamina at the end |
| ---: | ---: | ---: | ---: | ---: |
| 0 | 94 | 10 | never | 40.806 |
| 1 | 88 | 2 | 2 | 25 |
| 2 | 83 | 7 | 7 | 25 |
| 3 | 72 | 3 | 3 | 25 |
| 4 | 99 | 10 | 10 | 25 |
| 5 | 8 | never | never | 50 |
| 6 | 133 | 13 | never | 50 |
| 7 | 91 | 58 | never | 50 |
| 8 | 96 | 32 | never | 50 |

`HasSpell(DiseaseBoneBreakFever)` read false throughout, in every pen. No
hit had a spell as its source.

- *Settled*: **NPCs catch creature diseases.** Eight of the nine got
  `DisDamageStamina`, four of them with the full 25-point drain.
  UESP's "Followers are not affected by diseases" (*Skyrim:Followers*,
  revision 3646982) doesn't hold for these NPCs.
- *Settled*: **each blow rolls.** The first blow to infect was 2, 3, 7,
  10, 10, 13, 32 and 58 (pen 5's 8 blows, none). That is 8 infections in
  143 blows at risk, 5.6% a blow, against the condition's 6%
  (`GetRandomPercent <= 5`, 0 to 99).
- *Settled*: the disease's delivery sends no `OnHit` of its own; only the
  blow's.
- *Partial*: **`HasSpell` doesn't see a caught disease.** It read false
  with the drain in force (pens 1 to 4). With autotest28 (the player's
  `GetDisease` 1 while `HasSpell` false, 0 after `RemoveSpell`), a caught
  disease looks like an active effect of the spell, not a spell the actor
  has; `RemoveSpell` still ends it. Inferred, not read directly.
- *Partial*: **the drain doesn't always follow the effect.** In pens 6, 7
  and 8, `HasMagicEffect` read true but max stamina never fell to 30 or
  below at a blow, and read 50 at the end. In pen 0 it ended at 40.806 (a
  drain of 9.194). `HasMagicEffect` is known to report an effect whose
  condition fails (autotest19, autotest21), so these may be the effect
  present with its `GetRandomPercent` condition failing on a later check.
  UESP's confirmed bug "Rarely, Bone Break Fever drains 0 points from
  stamina instead of 25" (*Skyrim:Disease*, revision 3495507) may be the
  same thing. Why pen 0 drained a part is unexplained.

## Open, for a follow-up batch

1. Whether a caught disease's effect re-checks its `GetRandomPercent`
   condition after landing: max stamina polled every second after the
   infecting blow, in each pen, for a few minutes (with the bears gone).
2. `GetDisease` on an infected NPC (through the probe), and on the player
   after a caught disease, against `HasMagicEffect`.
3. Resist Disease: pens at DiseaseResist 0, 50 and 100, three each.
4. Pen 5's bear: landed 8 blows in 150 s.
