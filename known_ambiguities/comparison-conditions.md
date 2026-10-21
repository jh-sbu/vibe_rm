# Actor comparison and status conditions

Implemented in `src/condition/mod.rs` (`function_value`, with `same_faction`,
`same_race`, `same_sex`, `class_of`, `is_member`) and tested in
`src/condition/compare_tests.rs`. Covers `SameFaction`, `SameRace`, `SameSex`
and their `...AsPC` forms, `GetPCInFaction`, `GetIsClass`, `GetPCIsClass`,
`GetGold`, `GetStaminaPercentage`, `GetHealthPercentage`, `IsOverEncumbered`
and `GetPCMiscStat`.

## What is known

- Function indices and names: UESP Skyrim Mod:Function Indices (oldid 3566882,
  https://en.uesp.net/w/index.php?oldid=3566882): 42-44, 48, 68, 128, 129,
  132-135, 312, 430.
- `GetPCMiscStat` takes its statistic's name as a string (`CIS1`); UESP
  Skyrim:Console (oldid 3628513, https://en.uesp.net/w/index.php?oldid=3628513)
  gives `GetPCMS "barters"` as the example. The count is read through
  `Engine::query_stat`, so a name nothing counted is 0.
- Over-encumbrance: UESP Skyrim:Carry Weight (oldid 3630670,
  https://en.uesp.net/w/index.php?oldid=3630670) describes the player's carry
  limit; the engine's `player_overencumbered` is the only state kept.
- `GetPCInFaction`, `GetPCIsClass`, `GetPCIsRace` and the `...AsPC` forms
  ignore the subject, as their names say.
- UESP has no Skyrim page for `SameFaction`. The Morrowind page (rev 3109642,
  https://en.uesp.net/w/index.php?oldid=3109642) says it returns 1 when the
  player is in the faction; that is Morrowind's function, not evidence for Skyrim.

## Choices (unconfirmed)

- Rank -1 is not membership, on either side of `SameFaction` / `SameFactionAsPC`,
  as `GetInFaction` and `GetPCInFaction` leave it out. A potential follower's
  `CurrentFollowerFaction` rank is -1 (see `known_ambiguities/crime.md` and
  `known_ambiguities/followers.md`); the game's own treatment of rank -1 in
  `SameFaction` is not confirmed.
- `SameRace` and `SameRaceAsPC` use the race the templates give (`TRAITS` `RNAM`,
  as `Engine::race_of` does), where `GetIsRace` reads the base record's `RNAM`.
  They agree for NPCs without a template.
- `SameSex` compares the bases' `ACBS` female flag, as `GetIsSex` does.
- `GetGold` is the subject's inventory's gold, as `GetItemCount` reads it.
  Gold is `crime::GOLD` (`FormId(0xF)`).
- `GetStaminaPercentage` and `GetHealthPercentage` are the current value over the
  most it can be (0 to 1), as `GetActorValuePercent` is here. The UESP index
  lists `GetStaminaPercentage` as `GetStamina` with no description; the 0-1 scale
  is inferred from the vanilla `GetHealthPercentage` uses (`>= 0.75`, `< 0.25`).
- `IsOverEncumbered` is the player's only. NPCs have no carry-weight state the
  engine acts on (their race's carry weight and carried items exist, but whether
  an NPC over its carry weight is slowed or otherwise changed is unknown);
  for other actors the condition fails. Tracked in ROADMAP Next, AI depth.
- `GetPCMiscStat` with no name is unsupported (the condition fails).

## Tests

Synthetic records only (`compare_tests.rs`): potential followers left out of
`SameFaction`, `GetPCInFaction` on the player's membership, race and sex against
the player and another actor, class, gold, health fraction, the player's
carry-weight test and the named journal count.
