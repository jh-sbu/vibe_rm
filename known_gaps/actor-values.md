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
  skills and the health / magicka / stamina offsets, the race's `DATA` for
  starting health / magicka / stamina, regen rates, carry weight and mass.
  Everything else starts at 0 (`SpeedMult` 100).

## Open questions

- Which modifier each call touches is inferred from the wiki's wording:
  `SetActorValue` sets the base, `ModActorValue` and `ForceActorValue` change a
  permanent modifier (so `GetBaseActorValue` doesn't see them), damage stays at or
  below 0 and restoring heals only damage. There is no temporary (magic) modifier
  yet.
- `GetActorValuePercent` is answered as current over base plus permanent
  modifier, 0..1. A few conditions compare it against 30, 60, 75, which then
  always pass; whether the game scales those differently is unknown.
- NPCs' health uses the race's starting health plus the `DNAM` offset, as combat
  did; level-scaled NPCs' class-based health and skills (PC level mult) are not
  worked out. The player's health is a fixed 100 base.
- Health and stamina damage done to unloaded actors is kept but not carried into
  their live values when they load (they come in at full health, as before).
- Values past `WaitingForPlayer` (the skill modifiers, `DragonSouls`, indices 153
  and 161 that a few conditions use) have no names here; scripts naming them get
  a plain per-name number.
- Magicka isn't spent by anything yet; `Paralysis`, `Invisibility` and the resists
  are plain numbers with no effect.
