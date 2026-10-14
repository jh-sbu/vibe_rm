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
  stamina, read signed) and for the skills of 1502 (`vrm-tool npc-stats`):
  the rest have a skill past 100.
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

## Choices made without a source

- **Ties in the leftover skill points**: UESP's CLAS page has equal weights
  served by lower actor value index first; the stored `DNAM`s fit only the
  other way (higher index first, as UESP has it for attributes): Dervenin's
  three leftover points went to Enchanting, Alteration and Alchemy, not to
  One-Handed, Archery and Block. Taken from the data.
- **Skills past 100**: UESP says the shares are worked out otherwise once a
  skill reaches 100 (not how). The stored `DNAM`s give more to the other
  skills than an even share of the excess (Dremora at level 46: the two
  capped skills lost 26 points, the others gained 10). NPCs at their own
  level take `DNAM`'s skills, which have it right; "PC level mult" NPCs
  are worked out at their level, the skills held at 100 and the excess
  lost.
- **Non-auto-calc NPCs**: race plus `ACBS` offset, as the CK wiki's
  description and modders have it ("just race+offset"), not `DNAM`
  (garbage in many: Khayla's 16686). UESP's NPC pages disagree for some:
  Khayla (non-auto, health offset 105) is listed at 50, the race's alone.
- **Skill offsets** (`DNAM` 18..36; 12 vanilla NPCs, none auto-calc: prey
  animals' Sneak +70, Jorgen, a wisp mother) are added to the skill
  values, past 100 too (`MGArnielSummon`'s Alteration).
- The class (`CNAM`) comes with the stats template, as on the Creation
  Kit's Stats tab.
