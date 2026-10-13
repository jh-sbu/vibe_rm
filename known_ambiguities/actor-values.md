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
- Checked in Skyrim SE (`test-mod autotest`, 2026-10-12): `SetActorValue`
  sets the base and keeps the permanent modifier; `ModActorValue` changes
  the permanent modifier, which `GetBaseActorValue` doesn't see;
  `ForceActorValue` makes the current value what it's given through the
  permanent modifier, leaving damage as it is; restoring heals only damage.
  Papyrus's `GetActorValuePercentage` is current over base plus permanent
  modifier, 0..1, and 1 when that is 0.

## Open questions

- A few conditions compare `GetActorValuePercent` against 30, 60, 75, which
  then always pass; whether the condition function scales differently from
  Papyrus's is unknown.
