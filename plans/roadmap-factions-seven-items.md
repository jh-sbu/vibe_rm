# Seven faction items

Advances Roadmap Next 5 (crime, combat) and Next 9 (saves), with one commit
per item. Vanilla scripts call these Faction and Actor natives often
(`SetEnemy` 131 call sites, `SetAlly` 66, `SetPlayerEnemy` in the werewolf,
vampire, Dark Brotherhood and Civil War scripts: `vrm-tool pex-calls`), and
before this they did nothing. Per the user's direction, open questions were
added to `known_ambiguities/factions.md` rather than settled by a test mod for
now. Synthetic fixtures only.

| Item | Completed behavior | Section / ambiguity |
| --- | --- | --- |
| 1 | `SetEnemy` / `SetAlly` change both factions' group combat reactions; combat, `GetFactionCombatReaction` and `GetFactionRelation` follow | `FRCT`; `factions.md` |
| 2 | `SetReaction` / `ModReaction` / `GetReaction` on the `XNAM` modifier | `FRCT`; `factions.md` |
| 3 | Player expelled / enemy flags from `DATA`, `SetPlayerExpelled`, `SetPlayerEnemy`, `IsPlayerExpelled`, `GetPCExpelled`, `GetPCEnemyofFaction` | `FPLR`; `factions.md`, `crime.md` |
| 4 | Aggressive members (or crime-faction members) of a faction the player is an enemy of attack the player; `GetActorCrimePlayerEnemy` | `factions.md` |
| 5 | `SetAttackActorOnSight`: aggressive actors who don't like the marked actor attack it | `AOSG`; `factions.md` |
| 6 | `IsFactionInCrimeGroup` reads the faction's `CRGR` list | `factions.md` |
| 7 | `GetStolenItemValueNoCrime` / `GetStolenItemValueCrime` | `crime.md` |

## Tests

- `factions::tests`: reactions both ways and cache invalidation through
  `hostile_to`; modifiers one way; player-enemy hostility (aggression,
  teammates, crime factions); attacked-on-sight actors and the player; crime
  group lookups; every native through `natives::call`.
- `condition::crime_tests`: `GetPCExpelled`, `GetPCEnemyofFaction` and
  `GetActorCrimePlayerEnemy` from `DATA` and after scripts; the remaining
  unsupported crime conditions still fail.
- `save::tests`: `FRCT`, `FPLR` and `AOSG` round trips, every truncated
  prefix, duplicates, trailing bytes, null forms, bad combat reactions and
  flag bits, plus replace-on-load and legacy clearing in a running engine.

## Final validation

- `cargo test --workspace`: all pass (vibe_rm 412 passed, 211 ignored).
- `cargo test --bin vibe_rm -- --include-ignored`: 623 passed.
- `cargo fmt` and `git diff --check` are clean.
