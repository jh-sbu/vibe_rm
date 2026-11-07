# Actor values

Implemented in `src/actor_values.rs` (names and indices in
`crates/esp/src/actor_value.rs`). Based on the CK wiki's Papyrus pages
(`GetActorValue`, `SetActorValue`, `ModActorValue`, `ForceActorValue`,
`DamageActorValue`, `RestoreActorValue`, `GetActorValuePercentage`), the actor value
list (UESP / CommonLibSSE `ActorValue` order) and how the base game's conditions
use them (`vrm-tool av-conditions <data>`).

## What is known

- Condition parameters (`ptActorValue`) are indices in the `AVIF` order: AI data
  0-5, the 18 skills 6-23 in `NPC_` `DNAM` order, health / magicka / stamina 24-26,
  `Variable01`-`10` at 68-77, `WaitingForPlayer` 95.
- The base game's conditions: player skill checks (`GetBaseActorValue` in perks,
  `GetActorValue Speechcraft` in persuasion lines), health / stamina percentages
  (0..1) on lines, magic effects and idles, aggression / confidence on packages
  and lines, and above all `Variable01`, `06`, `07`, `09` and `WaitingForPlayer`
  that scripts set (followers, quests).
- Base values come from the records: `AIDT` bytes for the AI data, `DNAM` for
  skills, the race's `DATA` for regen rates, carry weight and mass.
  Everything else starts at 0 (`SpeedMult` 100).
- Health, magicka and stamina are the race's starting values plus the `ACBS`
  offsets (health at byte 20, magicka 4, stamina 6), and with "Auto Calc
  Stats" (which "PC Level Mult" forces on, CK wiki's Stats Tab) the class's
  share for each level past the first: UESP's CLAS page (10 attribute
  points a level by the class's weights, 5 more health) and NPC_ page
  (`DNAM`'s health / magicka / stamina are "calculated health (if auto-calc
  stats is on, otherwise seems to be random)"). The player: Nord's 50 and
  the offsets of 50 give the known 100 each. Auto-calculated skills: 15,
  the race's bonuses and 8 points a level by the class's skill weights.
  `esp::auto_calc` gives exactly what the Creation Kit stored in `DNAM` for
  all 1583 vanilla auto-calc NPCs at a fixed level (health / magicka /
  stamina, read signed) and for the skills of 1582 (`vrm-tool npc-stats`;
  skills past 100 below).
- Checked in Skyrim SE (`test-mod autotest`, 2026-10-12): `SetActorValue`
  sets the base and keeps the permanent modifier; `ModActorValue` changes
  the permanent modifier, which `GetBaseActorValue` doesn't see;
  `ForceActorValue` makes the current value what it's given through the
  permanent modifier, leaving damage as it is; restoring heals only damage.
  Papyrus's `GetActorValuePercentage` is current over base plus permanent
  modifier, 0..1, and 1 when that is 0.

- The `GetActorValuePercent` condition is 0..1 as Papyrus's is (`test-mod
  autotest5`): on a full-health target, `>= 0.5` passed and `>= 50` didn't;
  at 40% neither. The few vanilla conditions comparing it against 30, 60, 75
  never pass.
- NPCs' `HealRate` reads 0 (as does `CombatHealthRegenMult`), though their
  race's `DATA` has 0.7: that is the player's. Our NPCs at 90 of 300 health
  didn't heal in 30 s, free, held or fighting (`test-mod autotest5`).
- The player's health comes back at most x `HealRate`% x `HealRateMult`% a
  second, times `CombatHealthRegenMult` in combat (UESP Skyrim:Health,
  <https://en.uesp.net/w/index.php?oldid=3636643>). The 0.7 in combat is
  the Player record's ability `PCHealRateCombat` (a value modifier on
  `CombatHealthRegenMult`, base 0). Skyrim SE (`test-mod regen`,
  2026-11-04): 0.69%, 1.39% at `HealRateMult` 200, the combat multiplier
  unused out of combat; in combat 0.49%, 0.97%, 0.24% at
  `CombatHealthRegenMult` 0.35, so `fCombatHealthRegenRateMult` (0) doesn't
  apply to the player. Stamina is `StaminaRate`% x `StaminaRateMult`%,
  x `fCombatStaminaRegenRateMult` (0.35) in combat, after a wait of about
  `fDamagedStaminaRegenDelay` (0.26–0.44 s seen) from each damage or spend.

- Checked in Skyrim SE (`test-mod autotest21`, 2026-11-01;
  `research/autotest21.md`):
  - Ties in the leftover skill points go to the higher actor value index
    first: Dervenin read OneHanded 17, Marksman 27, Block 17, Enchanting 18,
    Alteration 18, Alchemy 23, as `esp::auto_calc` gives.
  - Non-auto-calc NPCs are race plus `ACBS` offset: Khayla read base health
    50, her race's 50 plus her offset of 0. (This file once gave her offset as
    105. Her `ACBS` has 0.)
  - Skill offsets are added past 100: a deer's Sneak read 85 (15 + 70), and
    `MGArnielSummon`'s Alteration 115.
  - The class comes with the stats template: an actor placed from a base
    whose stats template is a leveled list has the rolled bandit's class.

## Choices made without a source

- **Health's wait after a hit**: health showed no wait after
  `DamageActorValue`, even with hits 0.58 s apart (Skyrim SE, `test-mod
  regen`), though `fDamagedHealthRegenDelay` is 0.5. The probe's real hit
  (a Firebolt from an actor with its AI off) never landed, so whether a
  blow or spell starts that wait is unseen. Ours never waits.

- **Skills past 100**: UESP's CLAS page (revision 2512770,
  https://en.uesp.net/w/index.php?oldid=2512770, "Weight Calculations")
  says that once a skill reaches 100 "the full details of the equations
  used at this point have not yet been worked out", that the total is no
  longer 8 points a level, and that the points are "a fixed function of
  Sets". Skyrim SE (autotest21): a "PC level mult" copy of
  `EncDremoraMelee06` at level 46 read OneHanded 100, TwoHanded 100,
  HeavyArmor 84, Block 84, Destruction 15: the stored `DNAM`'s values, so
  the game shares the excess as the Creation Kit does.
  - Choice: the rule fitted to the 81 vanilla auto-calc NPCs whose whole
    sets take a skill to 100 (`esp::auto_calc::skills`), which gives 1582
    of the 1583 auto-calc NPCs' `DNAM`s (2026-11-07). With `sets` whole
    sets (8 points a level over the sum of the weights), the excess `x` is
    each capped skill's base + weight × sets − 100 (negative for skills
    capped only later). The points left after the sets plus `x` give the
    skills still under 100 at most one more set by weight, in the usual
    rounds (higher weight, then higher index). Then each gets
    `floor((x − z) / n)`, `z` the number of skills with no weight, `n` the
    number still under 100. A skill this takes past 100 (exactly 100
    doesn't count) is capped too, and the shares are worked out again from
    the sets.
  - Evidence for its parts: results depend only on the whole sets
    (`EncDraugr05TemplateBossEbony` at level 45 and `EncDremoraMelee06` at
    46 have the same weights, 32 sets and the same skills); every uncapped
    skill gains exactly weight × (sets + 1) plus the same amount; the
    constant taken off is 13 for the five-skill classes, 14 for the
    four-skill ones (`EncDragon04Fire`, `DLC2EncNetchBetty`) and at most 6
    for `DLC2dunKarstaag`, whose 18 skills all have weight.
  - Uncertain: the rule is a fit, not a documented algorithm, and how the
    game arrives at it is unknown. `DLC2EbonyWarrior` (level 80, five
    skills capped, one only after the excess) differs: Alteration 89 and
    Conjuration 95 stored, 92 and 98 ours; its other skills agree. Only
    "PC level mult" NPCs (and the player at level 1) use the rule; NPCs at
    their own level take `DNAM`.

