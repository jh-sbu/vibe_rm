# Six continuity and behaviour items

Advances Roadmap Next 9 (saves), Next 5 (crime, combat) and Next 1 (object
graphs), with one commit per item. Native policies don't establish Skyrim SE
or original `.ess` parity. Synthetic fixtures only; no game assets or engine
binaries were examined. Per the user's direction, open questions were added
to `known_ambiguities/` rather than settled by a test mod for now.

| Item | Completed behavior | Section / ambiguity |
| --- | --- | --- |
| 1 | Active magic effects survive saves without restarting; the old session's effects go without releasing their holds into the loaded values; summons command again | `AEFF`; `saves.md` |
| 2 | Scripts, variables and threads of saved effects stay in `VM`; orphaned effect scripts are refused | `VM`; `saves.md` |
| 3 | Caught-pickpocket victims forget after 48 game hours once they have unloaded | `PKTM`; `crime.md` |
| 4 | Searchers give up a lost target that fled after `fCombatDetectionFleeingLostRemoveTime` unseen | `detection.md` |
| 5 | Tapering value changes run out after loading | `TAPR`; `saves.md` |
| 6 | Behaviour-graph open states survive saves; rebuilt graphs replay them | `GOPN`; `saves.md` |

Before item 1 a load kept the old session's player effects (later releasing
their holds into the new values), and a load into a fresh engine left
effect-held modifiers in `AVAL` with no effect, while abilities were applied
again on top. Legacy saves still behave this way; they can't say what
was held.

## Tests

- Effects: a fortify ability and a timed fortify spell keep their value
  across loads into a fresh and a used engine, without the ability doubling;
  the timed one expires on its saved clock; ids don't collide. Wrong item
  indices are refused before anything changes; payload prefixes, duplicates,
  trailing bytes, bad clocks, unsorted ids and shared summons are refused.
  Restored summons rebuild `commanded`.
- Effect scripts: `OnEffectStart` ran once before the save and not again;
  `OnEffectFinish` reaches the restored script; finished effects' scripts
  aren't written; scripts without their effect are refused.
- Pickpocket: the gate holds while the victim stays loaded, lifts after two
  days away; `PKTM` must cover exactly `PKCT`'s victims; legacy `PKCT` alone
  is caught at the loaded hour.
- Searches: pure unit tests of the fled-target timer.
- Tapers: a drain tapering for 2 s continues after loading and stops at its
  total; legacy saves stop it.
- Graph open states: replay events reach each saved state; `GOPN` round trip,
  bad codes and `GetOpenState` after load.

## Final validation

- `cargo test --workspace`: all pass (vibe_rm 407 passed, 202 ignored).
- `cargo test --bin vibe_rm save:: -- --include-ignored`: 137 passed.
- Search, pickpocket, graph and magic filters with ignored tests: all pass.
- `cargo fmt` and `git diff --check` are clean.
